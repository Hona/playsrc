import { test } from "@playwright/test"
import { spawn } from "node:child_process"
import { createServer } from "node:net"
import { mkdir, mkdtemp, writeFile, readFile, realpath } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { staticStartupRouter, startupDigest } from "./static-startup-package"
import { captureStaticStartupPhase, compactStaticStartupPhase, assertStaticStartupPhase, staticStartupReceipt, startupPixelEvidence, requireStartupNative, type StartupObservation } from "./static-startup-gate"
import { startupConsoleIdle, startupNativeReader, externalStartupNativeReader, closeStartupNativeProbe } from "./native-startup"
import { WorkerCdpSession } from "./worker-cpu-profiler"
import { admitWorkerExecutionContext } from "./worker-runtime-admission"
import { installStaticPackageRouting } from "./static-package-routing"

test(`exact static package: audible movie, menu and playable frame (${process.env.PLAYSRC_STARTUP_CASE??"cold"})`, async ({ playwright }, testInfo) => {
  if (process.env.PLAYSRC_PROFILE_MANAGED !== "1") throw new Error("Static startup must run under the checked machine-wide profile lock")
  const config = await loadLocalConfig()
  const required = (name: string) => { const value=process.env[name];if(!value||!path.isAbsolute(value))throw new Error(`${name} must name an exact absolute file/directory`);return value }
  const router = await staticStartupRouter({ directory: required("PLAYSRC_STATIC_PACKAGE"), previousDirectory: required("PLAYSRC_PREVIOUS_STATIC_PACKAGE"), wasmFile: required("PLAYSRC_STATIC_WASM"), assetDir: config.assetDir })
  const mode=process.env.PLAYSRC_STARTUP_CASE??"cold"
  if(mode!=="cold"&&mode!=="warm-upgrade")throw new Error("Static startup case must be cold or warm-upgrade")
  let cold:any
  if(mode==="warm-upgrade"){
    const file=required("PLAYSRC_STATIC_COLD_PHASE")
    cold=JSON.parse(await readFile(file,"utf8"))
    if(cold.schema!=="playsrc-static-startup-phase-v1"||cold.mode!=="cold"||cold.packageSha256!==router.admitted.sha256||cold.wasmSha256!==router.admitted.configuration.wasm.sha256)throw new Error("Warm startup requires the exact accepted cold package")
    assertStaticStartupPhase(cold.capture,"cold")
    const profile=await realpath(cold.profileDirectory),parent=await realpath(path.dirname(file))
    if(path.dirname(profile)!==parent||!path.basename(profile).startsWith("native-profile-")||path.relative(config.sourceCacheDir,parent).startsWith(".."))throw new Error("Warm startup may only reopen its own accepted cold profile")
    if(cold.capture.profileSha256!==startupDigest(cold.profileDirectory))throw new Error("Accepted cold profile identity differs")
  }
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  await mkdir(directory, { recursive: true })
  const evidence: any = { package: router.admitted, previous: router.previous, startedAt: Date.now(), idle: [] }
  const externalEndpoint=process.env.PLAYSRC_STARTUP_CDP_ENDPOINT
  if(externalEndpoint&&(!/^http:\/\/127\.0\.0\.1:\d+$/.test(externalEndpoint)||!process.env.PLAYSRC_STARTUP_NATIVE_ENDPOINT||!process.env.PLAYSRC_STARTUP_NATIVE_LOCK_TOKEN))throw new Error("External startup endpoint requires its checked native broker")
  const idleDeadline = Date.now() + 15_000
  if(!externalEndpoint) do {
    let idle:number
    try {idle=await startupConsoleIdle(config.sourceCacheDir)} catch(error) {closeStartupNativeProbe();evidence.admissionFailure=String(error);await writeFile(path.join(directory,"rejected-admission.json"),JSON.stringify(evidence));throw error}
    evidence.idle.push({ at: Date.now(), milliseconds: idle })
    if (idle >= 2000) break
    if (Date.now() >= idleDeadline) { await writeFile(path.join(directory, "rejected-admission.json"), JSON.stringify(evidence)); closeStartupNativeProbe();throw new Error("No genuine two-second idle interval during bounded static startup admission") }
    await new Promise(resolve => setTimeout(resolve, 250))
  } while (true)
  const reservation = createServer()
  await new Promise<void>(resolve => reservation.listen(0, "127.0.0.1", resolve))
  const address = reservation.address(); if (!address || typeof address === "string") throw new Error("Static startup debug port reservation failed")
  await new Promise<void>(resolve => reservation.close(() => resolve()))
  const profile = cold?.profileDirectory??await mkdtemp(path.join(directory, "native-profile-"))
  evidence.profileDirectory=profile
  const executable = process.env.PLAYSRC_STARTUP_BROWSER ?? playwright.chromium.executablePath()
  if (!path.isAbsolute(executable)) throw new Error("Static startup browser must name its exact installed executable")
  const width = process.platform === "win32" ? 1705 : 1280, height = process.platform === "win32" ? 1372 : 800
  const child = externalEndpoint?undefined:spawn(executable, [`--user-data-dir=${profile}`, `--remote-debugging-port=${address.port}`, "--no-first-run", `--window-position=0,${process.platform==="win32"?0:40}`, `--window-size=${width},${height}`, "about:blank"], { stdio: ["ignore", "ignore", "pipe"] })
  let launchError: unknown, browser: Awaited<ReturnType<typeof playwright.chromium.connectOverCDP>> | undefined
  let native: Awaited<ReturnType<typeof startupNativeReader>> | Awaited<ReturnType<typeof externalStartupNativeReader>> | undefined
  let packageRouting: Awaited<ReturnType<typeof installStaticPackageRouting>> | undefined
  let observedPage: import("@playwright/test").Page | undefined
  child?.on("error", error => { launchError=error });child?.stderr!.on("data", data => { evidence.browserDiagnostics=((evidence.browserDiagnostics??"")+data).slice(-8192) })
  const terminate = () => child?.kill("SIGTERM")
  process.once("SIGTERM", terminate)
  try {
    const endpoint = externalEndpoint??`http://127.0.0.1:${address.port}`, ready = Date.now()+12_000
    while (!await fetch(`${endpoint}/json/version`).then(r=>r.ok).catch(()=>false)) {
      if (launchError || child&&child.exitCode !== null || Date.now()>=ready) throw new Error(`Static startup native browser failed: ${String(launchError)}`)
      await new Promise(resolve=>setTimeout(resolve,100))
    }
    browser = await playwright.chromium.connectOverCDP(endpoint, { noDefaults: true, timeout: 10_000 })
    evidence.browserVersion = browser.version()
    const context=browser.contexts()[0]!,page=context.pages()[0]!
    observedPage=page
    page.setDefaultTimeout(5_000)
    if(context.pages().length!==1)throw new Error("Static startup requires its single fresh native page")
    if(!externalEndpoint)await page.bringToFront()
    native=externalEndpoint?await externalStartupNativeReader(page,process.env.PLAYSRC_STARTUP_NATIVE_ENDPOINT!,process.env.PLAYSRC_STARTUP_NATIVE_LOCK_TOKEN!):await startupNativeReader(page,config.sourceCacheDir)
    evidence.console=[];evidence.errors=[];evidence.workers=[];evidence.responses=[]
    evidence.workerContexts=[]
    const workerBrowser=await browser.newBrowserCDPSession(),workerPage=await context.newCDPSession(page)
    let unavailableConfiguration=false
    packageRouting=await installStaticPackageRouting(workerBrowser,async url=>{
      if(unavailableConfiguration&&url==="https://playsrc.online/tf2/playsrc-config.json")return {status:503,headers:{"content-type":"application/problem+json"},body:Buffer.from('{"title":"Startup gate unavailable configuration fixture"}')}
      return router.response(url)
    })
    evidence.packageRouting=packageRouting.records
    await workerPage.send("Page.enable");await workerPage.send("Page.setLifecycleEventsEnabled",{enabled:true})
    evidence.navigations=[]
    const navigate=async(mode:"cold"|"warm-upgrade")=>{
      if(mode==="warm-upgrade")router.warmUpgrade()
      const initial=(await workerPage.send("Page.getFrameTree")).frameTree.frame
      const documents:any[]=[],expected=mode==="cold"?1:2
      let resolve!:()=>void,reject!:(error:Error)=>void
      const admitted=new Promise<void>((yes,no)=>{resolve=yes;reject=no})
      const timer=setTimeout(()=>reject(new Error("Static startup document/upgrade admission exceeded its bound")),20_000)
      const frame=(event:any)=>{
        if(event.frame.parentId||event.frame.loaderId===initial.loaderId)return
        if(!documents.some(value=>value.loaderId===event.frame.loaderId))documents.push({frameId:event.frame.id,loaderId:event.frame.loaderId,url:event.frame.url,at:Date.now(),domContentLoaded:false})
        if(documents.length>expected)reject(new Error("Static startup exceeded its one declared upgrade navigation"))
      }
      const lifecycle=(event:any)=>{
        if(event.name!=="DOMContentLoaded")return
        const document=documents.find(value=>value.frameId===event.frameId&&value.loaderId===event.loaderId)
        if(document)document.domContentLoaded=true
        if(documents.length===expected&&documents.at(-1)?.domContentLoaded)resolve()
      }
      workerPage.on("Page.frameNavigated",frame);workerPage.on("Page.lifecycleEvent",lifecycle)
      try {await Promise.all([page.goto("https://playsrc.online/tf2",{waitUntil:"domcontentloaded",timeout:20_000}),admitted])}
      finally {clearTimeout(timer);workerPage.off("Page.frameNavigated",frame);workerPage.off("Page.lifecycleEvent",lifecycle);evidence.navigations.push({mode,documents})}
    }
    const admittedWorkers=new Set<string>()
    const admitGameplayWorker=async()=>{
      const {targetInfo:owner}=await workerPage.send("Target.getTargetInfo")
      const urls=new Set(page.workers().map(worker=>worker.url()))
      const {targetInfos}=await workerBrowser.send("Target.getTargets")
      const targets=targetInfos.filter(target=>target.type==="worker"&&target.url.includes("gameplay-worker")&&urls.has(target.url)&&target.browserContextId===owner.browserContextId)
      if(targets.length!==1)throw new Error("Static startup gameplay Worker owner is ambiguous or absent")
      const target=targets[0]!
      if(admittedWorkers.has(target.targetId))return
      const {sessionId}=await workerBrowser.send("Target.attachToTarget",{targetId:target.targetId,flatten:false})
      const session=new WorkerCdpSession(workerBrowser,sessionId)
      try {const executionContextId=await admitWorkerExecutionContext(session);evidence.workerContexts.push({target,executionContextId});admittedWorkers.add(target.targetId)}
      finally {await session.close()}
    }
    page.on("console",message=>evidence.console.push({type:message.type(),text:message.text()}))
    page.on("pageerror",error=>evidence.errors.push(String(error)))
    page.on("worker",worker=>evidence.workers.push({url:worker.url(),at:Date.now()}))
    page.on("response",response=>evidence.responses.push({url:response.url(),status:response.status(),headers:response.headers()}))
    evidence.unexpectedInput=[]
    await page.exposeBinding("__playsrcUnexpectedStartupInput",(_source,event)=>{evidence.unexpectedInput.push(event)})
    evidence.gpuFailures=[]
    await page.exposeBinding("__playsrcStartupGpuFailure",(_source,event)=>{evidence.gpuFailures.push(event)})
    await page.addInitScript(()=>{
      ;(globalThis as any).__playsrcProfile={}
      const graphics={devices:[] as any[],losses:[] as any[]};(globalThis as any).__playsrcStartupGraphics=graphics
      const gpuFailure=(value:any)=>{if(graphics.losses.length>=256)return;const detail={at:performance.now(),timeOrigin:performance.timeOrigin,...value};graphics.losses.push(detail);void(globalThis as any).__playsrcStartupGpuFailure(detail)}
      const adapter=(globalThis as any).GPUAdapter?.prototype,requestDevice=adapter?.requestDevice
      if(requestDevice)Object.defineProperty(adapter,"requestDevice",{configurable:true,writable:true,value:function(this:any,...args:any[]){
        const result=requestDevice.apply(this,args)
        void result.then((device:any)=>{
          const info=this.info
          if(graphics.devices.length<16)graphics.devices.push({label:args[0]?.label??"",requiredFeatures:[...(args[0]?.requiredFeatures??[])],features:[...device.features],isFallbackAdapter:this.isFallbackAdapter??info?.isFallbackAdapter??null,info:Object.fromEntries(["vendor","architecture","device","description","backend","driver"].map(key=>[key,info?.[key]??null]))})
          device.addEventListener("uncapturederror",(event:any)=>gpuFailure({kind:"validation",message:String(event.error?.message??event.error)}))
          void device.lost.then((value:any)=>{if(value.reason!=="destroyed")gpuFailure({kind:"device",reason:value.reason,message:value.message})})
        },()=>{})
        return result
      }})
      const state={action:"none",unexpected:[] as unknown[]};(globalThis as any).__playsrcStartupInput=state
      for(const type of ["keydown","pointerdown","wheel","mousemove"])document.addEventListener(type,event=>{
        const key=(event as KeyboardEvent).code,button=(event as PointerEvent).button
        if(type==="mousemove"&&!(event as MouseEvent).movementX&&!(event as MouseEvent).movementY)return
        const planned=state.action!=="none"&&event.isTrusted&&(type==="mousemove"||type==="pointerdown"&&button===0
          ||type==="keydown"&&(state.action==="open-map"&&["Backquote","Enter"].includes(key)||(state.action==="close-console"||state.action==="choose-team")&&key==="Backquote"||state.action==="choose-class"&&key==="Digit2"))
        if(!planned){const detail={at:performance.now(),timeOrigin:performance.timeOrigin,type,key,button,trusted:event.isTrusted,action:state.action};state.unexpected.push(detail);void(globalThis as any).__playsrcUnexpectedStartupInput(detail)}
      },{capture:true,passive:true})
    })
    let admission=0
    const capture=await captureStaticStartupPhase({
      native:()=>native!.read(path.join(directory,`native-${admission++}.png`)),
      navigate,
      read:async()=>{
        packageRouting!.check()
        if(evidence.gpuFailures.length)throw new Error(`Static startup GPU failure: ${evidence.gpuFailures[0].message}`)
        const state=await page.evaluate(()=>{const main=document.querySelector<HTMLElement>("main"),video=document.querySelector<HTMLVideoElement>(".startup-movie"),canvas=document.querySelector<HTMLElement>("canvas.world-canvas");return {phase:main?.dataset.phase??"Absent",detail:main?.dataset.detail??"",startupState:main?.dataset.startupState,visible:document.visibilityState==="visible",focused:document.hasFocus(),timeOrigin:performance.timeOrigin,at:performance.now(),frame:Number(canvas?.dataset.displayFrame??0),cache:main?.dataset.cache,consoleVisible:main?.dataset.consoleVisible==="true",gameUi:main?.dataset.gameui,playerClass:Number(main?.dataset.hudProbe?.split(":")[1]??0),tick:main?.dataset.snapshotTick,teamSelection:main?.dataset.teamSelectionVisible==="true",classSelection:main?.dataset.classSelectionVisible==="true",unexpectedInput:(globalThis as any).__playsrcStartupInput?.unexpected.length??0,movie:video?{time:video.currentTime,paused:video.paused,muted:video.muted,width:video.videoWidth,height:video.videoHeight}:null} as StartupObservation})
        if(state.phase==="MainMenu"||state.phase==="Ready")await admitGameplayWorker()
        state.unexpectedInput=Math.max(state.unexpectedInput,evidence.unexpectedInput.length)
        return state
      },
      screenshot:async label=>{const bytes=await page.screenshot({path:path.join(directory,`${label}.png`)});await testInfo.attach(label,{body:bytes,contentType:"image/png"});return bytes},
      action:async(action,target)=>{
        if(evidence.unexpectedInput.length||!await page.evaluate(()=>document.visibilityState==="visible"&&document.hasFocus()))throw new Error("Static startup input delivery lost its visible admitted document")
        await page.evaluate(action=>{(globalThis as any).__playsrcStartupInput.action=action},action)
        try {
          if(action==="play-intro")await page.getByRole("button",{name:"Play intro",exact:true}).click()
          else if(action==="open-map") {await page.keyboard.press("Backquote");await page.getByRole("textbox",{name:"Console command",exact:true}).click();await page.keyboard.insertText(`map ${target}`);await page.keyboard.press("Enter")}
          else if(action==="close-console"){await page.keyboard.press("Backquote");await page.locator("canvas.world-canvas").focus()}
          else if(action==="choose-team"){
            if(await page.locator("main").getAttribute("data-console-visible")==="true")await page.keyboard.press("Backquote")
            await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
          }
          else await page.keyboard.press("Digit2")
        } finally {await page.evaluate(()=>{(globalThis as any).__playsrcStartupInput.action="none"})}
      },
      wait:milliseconds=>new Promise(resolve=>setTimeout(resolve,milliseconds)),
    },router.admitted.configuration.defaultTarget,mode)
    await router.verifyUnchanged()
    const phase={schema:"playsrc-static-startup-phase-v1",mode,packageSha256:router.admitted.sha256,wasmSha256:router.admitted.configuration.wasm.sha256,profileDirectory:profile,capture:compactStaticStartupPhase(capture,startupDigest(profile))}
    assertStaticStartupPhase(phase.capture,mode)
    await writeFile(path.join(directory,"startup-phase.json"),JSON.stringify(phase))
    evidence.capture=capture
    if(mode==="cold"){console.log(`PLAYSRC_STATIC_COLD_PHASE ${path.join(directory,"startup-phase.json")}`);return}
    const upgradeNavigations=router.upgradeNavigations
    evidence.runtime=await page.evaluate(()=>({graphics:(globalThis as any).__playsrcStartupGraphics,quality:(globalThis as any).__playsrcProfile.videoQuality,generation:(globalThis as any).__playsrcProfile.applicationGeneration,cache:(globalThis as any).__playsrcProfile.immutableCache,state:{...document.querySelector<HTMLElement>("main")?.dataset}}))
    if(evidence.gpuFailures.length)throw new Error("Static startup encountered a GPU validation/device failure")
    // A declared network-failure fixture tests the independent boot UI. It does
    // not edit or relabel any package/WASM byte, and is not a successful boot.
    unavailableConfiguration=true
    await page.goto("https://playsrc.online/tf2",{waitUntil:"domcontentloaded",timeout:20_000})
    await page.waitForFunction(()=>document.querySelector("main")?.getAttribute("data-phase")==="Failed",undefined,{timeout:10_000})
    const failurePanel=page.getByRole("alert",{name:"Application failure",exact:true})
    const failureNative=await native.read(path.join(directory,"native-boot-failure.png"))
    requireStartupNative(failureNative)
    const failurePixels=await page.screenshot({path:path.join(directory,"boot-failure.png")})
    const bootFailure={phase:"Failed",visible:await failurePanel.isVisible(),text:await failurePanel.innerText(),pixels:startupPixelEvidence(failurePixels),native:failureNative}
    await testInfo.attach("boot-failure",{body:failurePixels,contentType:"image/png"})
    evidence.bootFailure=bootFailure
    if(evidence.gpuFailures.length||evidence.unexpectedInput.length)throw new Error("Static startup post-capture GPU/input guard failed")
    await router.verifyUnchanged()
    packageRouting.check();await packageRouting.close();packageRouting=undefined
    await Promise.all([workerPage.detach(),workerBrowser.detach()])
    evidence.capture=capture
    const receipt=staticStartupReceipt({packageSha256:router.admitted.sha256,wasmSha256:router.admitted.configuration.wasm.sha256,previousPackageSha256:router.previous.sha256,previousEntryUsed:router.previousEntryUsed,upgradeNavigations,bootFailure},cold.capture,phase.capture)
    await writeFile(path.join(directory,"startup-receipt.json"),JSON.stringify(receipt))
    console.log(`PLAYSRC_STATIC_STARTUP_RECEIPT ${JSON.stringify(receipt)}`)
  } catch(error) {
    evidence.failure=String(error);evidence.capture=(error as any).startupEvidence
    try {evidence.failureState=await observedPage?.evaluate(()=>({state:{...document.querySelector<HTMLElement>("main")?.dataset},console:document.querySelector<HTMLElement>('[aria-label="Console output"]')?.innerText,profile:(globalThis as any).__playsrcProfile&&{cache:(globalThis as any).__playsrcProfile.immutableCache,generation:(globalThis as any).__playsrcProfile.applicationGeneration}}))}
    catch(readback){evidence.failureStateError=String(readback)}
    // Read only: distinguish OS/window loss from document focus loss without
    // reactivating, clicking, retrying, or collecting a hidden page screenshot.
    try {evidence.failureNative=await native?.read(path.join(directory,"native-failure.png"))}
    catch(readback){evidence.failureNativeError=String(readback)}
    throw error
  }
  finally {
    evidence.reads=router.reads;evidence.native=native?.records;evidence.endedAt=Date.now()
    await writeFile(path.join(directory,"static-startup-evidence.json"),JSON.stringify(evidence))
    try {await packageRouting?.close();await native?.close()} finally {
      try {
        if(browser){const close=await browser.newBrowserCDPSession();await close.send("Browser.close").catch(()=>{});await browser.close().catch(()=>{})}
        if(child&&child.exitCode===null)await Promise.race([new Promise(resolve=>child.once("exit",resolve)),new Promise(resolve=>setTimeout(resolve,2000))])
      } finally {closeStartupNativeProbe();terminate();process.removeListener("SIGTERM",terminate)}
    }
  }
})
