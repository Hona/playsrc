import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { headedProfileTarget } from "./profile-target"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { macPageAdmission,requireMacPageAdmission,type MacPageAdmission } from "./macos-page-admission"
import { tf2MapBsp, tf2MapMode } from "@playsrc/game-tf2-browser/maps"
import { loadLocalConfig } from "../src/config"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import {startGameplayReplayJournal} from "./gameplay-replay"

const json = (value: unknown) => JSON.stringify(value, (_, value) => typeof value === "bigint" ? value.toString() : value)
let closeNative:(()=>Promise<void>)|undefined
let nativeRecords:MacPageAdmission[]=[]
let nativeWaitRecords:MacPageAdmission[]=[]
let nativeMonitoring=false,nativeFailure:unknown,nativeMonitor:Promise<void>|undefined
let replayJournal:Awaited<ReturnType<typeof startGameplayReplayJournal>>|undefined

test.afterEach(async ({ page }, testInfo) => {
  nativeMonitoring=false;await nativeMonitor
  if(replayJournal){await replayJournal.stop();replayJournal=undefined}
  await writeFile(testInfo.outputPath("native-admission.json"),json(nativeRecords))
  await writeFile(testInfo.outputPath("native-admission-waits.json"),json(nativeWaitRecords))
  await closeNative?.();closeNative=undefined
  if(nativeFailure&&testInfo.status===testInfo.expectedStatus)throw nativeFailure
  if (testInfo.status === testInfo.expectedStatus || page.isClosed()) return
  const evidence = await page.evaluate(() => ({ failure: (globalThis as any).__playsrcProfile?.failure,
    legacyVisuals: (globalThis as any).__playsrcProfile?.legacyVisualEvidence,
    legacyViews:(globalThis as any).__playsrcProfile?.legacyVisualViews,
    geometry:(globalThis as any).__playsrcProfile?.geometryEvidence,
    console:document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText,
    frames: (globalThis as any).__playsrcFrameProfiler?.completedFrames,
    simulation: (globalThis as any).__playsrcFrameProfiler?.simulation,
    counters:(globalThis as any).__playsrcFrameProfiler?.counters,nodeBuilds:(globalThis as any).__playsrcFrameProfiler?.nodeBuilds,
    dataset: { ...document.querySelector<HTMLElement>("main")?.dataset } })).catch(() => null)
  await writeFile(testInfo.outputPath("map-failure.json"), json(evidence))
})

test("configured map native traversal, objective roster, visible geometry and cadence", async ({ page }, testInfo) => {
  const target = headedProfileTarget(process.env, "cp_badlands")
  const config = await loadLocalConfig()
  if(process.env.PROFILE_MAP_REPLAY==="1")replayJournal=await startGameplayReplayJournal(page,testInfo.outputPath("gameplay-replay"),target)
  const native=await macPageAdmission(page,config.sourceCacheDir)
  closeNative=native?.close;nativeRecords=[];nativeWaitRecords=[];nativeFailure=undefined;nativeMonitor=undefined
  const checkNative=async(desktop?:string)=>{
    if(!native)return
    const record=await native.read(desktop);nativeRecords.push(record);requireMacPageAdmission(record)
  }
  const worldScreenshot=async(path:string)=>{
    await waitNativeReady();const bytes=await page.locator("canvas.world-canvas").screenshot({path});await checkNative();return bytes
  }
  const waitNativeReady=async()=>{
    if(!native)return
    const deadline=Date.now()+5000
    for(;;){
      const record=await native.read()
      try{requireMacPageAdmission(record);nativeRecords.push(record);return}
      catch(error){nativeWaitRecords.push(record);if(Date.now()>=deadline)throw error}
      await page.waitForTimeout(250)
    }
  }
  const facts = JSON.parse(await readFile(path.join(config.sourceCacheDir, "evidence/map-runtime", `${target}.facts.json`), "utf8"))
  expect(facts.bspSha256).toBe(tf2MapBsp(target).sha256)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.addInitScript(installBrowserFrameProfiler)
  if(process.env.PROFILE_MAP_PIPELINE_PROBE==="1")await page.addInitScript(()=>{(globalThis as any).__playsrcFrameProfiler.nodeKeyMaterial="materials/models/weapons/c_models/c_minigun/c_minigun.vmt"})
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  const sampleWindow=async()=>{
    nativeMonitoring=true;nativeFailure=undefined
    nativeMonitor=(async()=>{
      while(nativeMonitoring&&native&&nativeRecords.length<400){
        await new Promise(resolve=>setTimeout(resolve,500))
        if(!nativeMonitoring)break
        try{await checkNative()}catch(error){nativeFailure=error;break}
      }
    })()
    try{return await page.evaluate(async()=>{
    const root=document.querySelector<HTMLElement>("main")!,profile=(globalThis as any).__playsrcProfile,profiler=(globalThis as any).__playsrcFrameProfiler
    profiler.completedFrames.length=0;profiler.active=true
    const start=performance.now(),tick=Number(root.dataset.snapshotTick)
    const audioBefore=profile.audio?.stats()
    const before=profile.bots.map((bot:any)=>({identity:bot.identity,area:bot.area,position:bot.position}))
    const frames:number[]=[];let previous:number|undefined,firstRafTimestamp:number|undefined,firstRafObserved:number|undefined
    // A RAF timestamp can precede an evaluate() that ran during that frame.
    // Retain the real first callback wait, then only RAF-to-RAF intervals.
    await new Promise<void>(resolve=>{const frame=(now:number)=>{
      if(previous===undefined){firstRafTimestamp=now;firstRafObserved=performance.now();frames.push(firstRafObserved-start)}else frames.push(now-previous)
      previous=now;if(now-start>=5000)resolve();else requestAnimationFrame(frame)
    };requestAnimationFrame(frame)})
    profiler.active=false
    return {seconds:(performance.now()-start)/1000,ticks:Number(root.dataset.snapshotTick)-tick,frames,sampleStarted:start,firstRafTimestamp,firstRafObserved,before,bots:profile.bots,points:profile.controlPoints.points,
      completedFrames:profiler.completedFrames,counters:profiler.counters,nodeBuilds:profiler.nodeBuilds,nodeKeys:profiler.nodeKeys,simulation:profiler.simulation,memoryAssets:profile.memoryAssets,failures:profile.failure,longTasks:profiler.longTasks,round:profile.round,
      audioBefore,audioAfter:profile.audio?.stats(),soundscape:profile.soundscape}
    })}finally{nativeMonitoring=false;await nativeMonitor;await checkNative();if(nativeFailure)throw nativeFailure}
  }
  let revision = 0
  const spawnChecks: unknown[] = []
  const waitPlayer = async (field: "team" | "class", value: number) => {
    try {
      await page.waitForFunction(({ field, value }) => (globalThis as any).__playsrcProfile.player?.[field] === value, { field, value }, { timeout: 5000 })
    } catch (error) {
      const state = await page.evaluate(() => ({ player: (globalThis as any).__playsrcProfile.player, dataset: { ...document.querySelector<HTMLElement>("main")!.dataset }, console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText }))
      await writeFile(testInfo.outputPath(`${target}-spawn-failure.json`), JSON.stringify(state))
      throw error
    }
  }
  const checkSpawn = async (team: number, label: string) => {
    await waitPlayer("team", team)
    const player = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
    const candidates = facts.spawns.filter((spawn: any) => Number(spawn.team) === team && Math.hypot(spawn.position[0] - player.position[0], spawn.position[1] - player.position[1]) < 1 && Math.abs(spawn.position[2] - player.position[2]) < 128)
    expect(candidates.length, `${target}:${label} authored spawn position`).toBeGreaterThan(0)
    const yawError = (left: number, right: number) => Math.abs(((left - right + 540) % 360) - 180)
    expect(candidates.some((spawn: any) => yawError(player.camera.yawDegrees, spawn.angles[1]) < 0.001 && Math.abs(player.camera.pitchDegrees - spawn.angles[0]) < 0.001), `${target}:${label} authored spawn angles`).toBe(true)
    spawnChecks.push({ label, player, candidates })
  }
  const capture = async (label: string) => {
    await closeConsole()
    await waitNativeReady()
    const selected = ++revision
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, selected)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, selected)
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    await writeFile(testInfo.outputPath(`${target}-${label}-audio.json`),json(await page.evaluate(()=>({selection:(globalThis as any).__playsrcProfile.soundscape,stats:(globalThis as any).__playsrcProfile.audio?.stats()}))))
    const imagePath = testInfo.outputPath(`${target}-${label}.png`)
    await page.screenshot({ path: imagePath })
    if (process.platform === "darwin" && label === "spawn") {
      const desktopPath = testInfo.outputPath(`${target}-spawn-desktop.png`)
      await checkNative(desktopPath)
      await testInfo.attach("native-desktop", { path: desktopPath, contentType: "image/png" })
    }
    const image = decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-${label}-world.png`)))
    const depth = geometry.geometry.samples.filter((sample: any) => sample.family !== null && Number.isFinite(sample.depth) && sample.depth > 0).map((sample: any) => {
      const x = Math.max(0, Math.min(image.width - 1, Math.round((sample.x + 1) * image.width / 2)))
      const y = Math.max(0, Math.min(image.height - 1, Math.round((1 - sample.y) * image.height / 2)))
      const offset = (y * image.width + x) * image.channels
      return { ...sample, rgb: [...image.pixels.subarray(offset, offset + 3)] }
    })
    const facts = await page.evaluate(() => ({ points: (globalThis as any).__playsrcProfile.controlPoints, bots: (globalThis as any).__playsrcProfile.bots, round: (globalThis as any).__playsrcProfile.round }))
    const dataPath = testInfo.outputPath(`${target}-${label}.json`)
    await writeFile(dataPath, JSON.stringify({ geometry, depth, facts }))
    await testInfo.attach(label, { path: imagePath, contentType: "image/png" })
    await testInfo.attach(`${label}-depth`, { path: dataPath, contentType: "application/json" })
    expect(depth.some((sample: any) => ["main-world","static-prop","dynamic-prop"].includes(sample.disposition) && sample.rgb.some((channel: number) => channel > 3))).toBe(true)
  }
  await page.bringToFront()
  if(native){
    const deadline=Date.now()+20_000
    let announced=false
    for(;;){
      const record=await native.read()
      if(!announced&&record.linkage){console.error(`[native-window] browser=${record.page?.browserPid} window=${record.linkage.nativeWindowId}`);announced=true}
      await writeFile(testInfo.outputPath("native-window-pending.json"),json(record))
      try{requireMacPageAdmission(record);nativeRecords.push(record);break}
      catch(error){if(Date.now()>=deadline){nativeRecords.push(record);throw error}}
      await page.waitForTimeout(250)
    }
  }
  if(process.env.PROFILE_MAP_LIFECYCLE==="1"){
    const prepared=await page.request.post(`/__playsrc/prepare-target/${process.env.PROFILE_MAP_REPLACEMENT??"koth_viaduct"}`)
    expect(prepared.status()).toBe(200)
  }
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command(`map ${target}`)
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  if(process.env.PROFILE_MAP_AUTONOMOUS==="1"){
    await replayJournal?.ready()
    if(replayJournal)await writeFile(testInfo.outputPath("replay-content.json"),json(await page.evaluate(()=>(globalThis as any).__playsrcProfile.applicationGeneration)))
    expect(target).toBe("cp_granary")
    await command("joinclass soldier")
    await command("tf_bot_add 15 red scout normal");await closeConsole()
    await expect(main).toHaveAttribute("data-bot-count","15")
    await page.locator("canvas.world-canvas").click({force:true})
    await page.waitForFunction(()=>{const p=(globalThis as any).__playsrcProfile;return p.round?.state===4&&!p.round.inSetup&&!p.round.waitingForPlayers},undefined,{timeout:45000})
    const middle=await page.evaluate(()=>(globalThis as any).__playsrcProfile.controlPoints.points.find((point:any)=>point.owner===0)?.identity)
    expect(middle).toBeDefined()
    const started=Date.now(),history:any[]=[],entered=new Set<number>()
    let escaped:number|undefined,sampled=false,captured=false,lowerCaptured=false
    while(Date.now()-started<105000){
      const state=await page.evaluate(()=>{const p=(globalThis as any).__playsrcProfile;return {tick:Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick),round:p.round,points:p.controlPoints,bots:p.bots,camera:p.player.camera,audio:p.audio?.stats()}})
      history.push(state)
      for(const bot of state.bots){
        if(bot.area===6129)entered.add(bot.identity)
        if(entered.has(bot.identity)&&[6137,6138,1223].includes(bot.area)&&escaped===undefined)escaped=bot.identity
      }
      if(entered.size&&!sampled){
        const bot=state.bots.find((bot:any)=>entered.has(bot.identity))
        await page.evaluate(({camera,position})=>{(globalThis as any).__playsrcProfile.displacementCameraOverride={...camera,position:[position[0]-96,position[1]-128,position[2]+112],yawDegrees:53.130102,pitchDegrees:30}}, {camera:state.camera,position:bot.position})
        await capture("autonomous-upper-crossing")
        await replayJournal?.mark(0)
        const sample=await sampleWindow();await replayJournal?.mark(1);await writeFile(testInfo.outputPath("autonomous-cadence.json"),json({...sample,frames:summarizeFrameTimes(sample.frames)}))
        expect.soft(sample.ticks/sample.seconds).toBeGreaterThan(63);expect.soft(sample.audioAfter?.underrunFrames).toBe(0)
        sampled=true
      }
      if(escaped!==undefined&&!lowerCaptured){
        const bot=state.bots.find((bot:any)=>bot.identity===escaped)
        await page.evaluate(({camera,position})=>{(globalThis as any).__playsrcProfile.displacementCameraOverride={...camera,position:[position[0]-64,position[1]-96,position[2]+96],yawDegrees:56.309932,pitchDegrees:30}}, {camera:state.camera,position:bot.position})
        await capture("autonomous-lower-crossing");lowerCaptured=true
      }
      if(escaped!==undefined&&state.bots.some((bot:any)=>bot.captures>0)&&state.points.points.some((point:any)=>point.owner===2&&point.identity===middle)){captured=true;break}
      await page.waitForTimeout(100)
    }
    const result={target,fixture:"normal-spawn-15-red-scouts-crossing-and-capture",entered:[...entered],escaped,captured,history}
    await writeFile(testInfo.outputPath("autonomous-route-capture.json"),json(result))
    expect(escaped,"a bot must physically leave the upper crossing for the lower authored route").toBeDefined()
    expect(captured,"walking bots must capture authored mid after leaving the blocked route").toBe(true)
    await capture("autonomous-mid-captured")
    await page.evaluate(()=>{delete (globalThis as any).__playsrcProfile.displacementCameraOverride})
    return
  }
  if(process.env.PROFILE_MAP_ROUTE_VIEWS){
    const views=JSON.parse(process.env.PROFILE_MAP_ROUTE_VIEWS) as number[][]
    expect(views.length).toBeLessThanOrEqual(8)
    for(const [index,view]of views.entries()){
      expect(view).toHaveLength(5);expect(view.every(Number.isFinite)).toBe(true)
      await command(`setpos ${view.slice(0,3).join(" ")}`)
      await command(`setang ${view[4]} ${view[3]} 0`)
      await closeConsole();await capture(`route-${index}`)
    }
    return
  }
  if(process.env.PROFILE_MAP_LIFECYCLE==="1"){
    const replacement=process.env.PROFILE_MAP_REPLACEMENT??"koth_viaduct"
    expect(replacement).not.toBe(target);tf2MapBsp(replacement)
    const records:any[]=[]
    const state=async(label:string)=>{
      await capture(label)
      const data=await page.evaluate(()=>({generation:Number(document.querySelector<HTMLElement>("main")!.dataset.generation),cache:document.querySelector<HTMLElement>("main")!.dataset.cache,
        geometry:(globalThis as any).__playsrcProfile.geometryEvidence,memory:(globalThis as any).__playsrcProfile.memoryAssets,quality:(globalThis as any).__playsrcProfile.videoQuality,heap:(performance as any).memory?.usedJSHeapSize,
        soundscape:(globalThis as any).__playsrcProfile.soundscape,audio:(globalThis as any).__playsrcProfile.audio?.stats()}))
      const worker=await Promise.all(page.workers().filter(worker=>worker.url().includes("gameplay-worker")).map(worker=>worker.evaluate(()=>(globalThis as any).__playsrcWorkerMemory)))
      records.push({label,...data,worker});await writeFile(testInfo.outputPath("map-generation-lifecycle.json"),json(records))
      expect(data.audio?.underrunFrames).toBe(0)
      expect(worker).toHaveLength(1)
      expect(worker[0].resourceSections.every((section:any)=>section.generation===data.generation&&section.owner==="active")).toBe(true)
      return data
    }
    const initial=await state("generation-initial")
    await page.keyboard.press("Escape")
    await expect(main).toHaveAttribute("data-gameui","pause")
    await page.locator('[data-vgui-name="DisconnectButton"]').click()
    await expect(main).toHaveAttribute("data-phase","MainMenu")
    const prepared=await page.request.post(`/__playsrc/prepare-target/${replacement}`)
    expect(prepared.status()).toBe(200)
    const configuration=await prepared.json()
    const replacementBsp=configuration.targets.find((candidate:any)=>candidate.target===replacement)?.objects.bsp.sha256
    expect(replacementBsp).toMatch(/^[0-9a-f]{64}$/)
    let release!:()=>void,entered!:()=>void,settled!:()=>void,preparationFailure:unknown
    const held=new Promise<void>(resolve=>release=resolve),started=new Promise<void>(resolve=>entered=resolve),finished=new Promise<void>(resolve=>settled=resolve)
    const route=`**/${replacementBsp}`
    await page.route(route,async request=>{
      try{const response=await request.fetch();expect(response.status()).toBe(200);entered();await held;await request.fulfill({response}).catch(()=>{})}
      catch(error){preparationFailure=error;entered()}
      finally{settled()}
    })
    try{
      await command(`map ${replacement}`)
      await Promise.race([started,page.waitForTimeout(20000).then(()=>{throw new Error("Replacement BSP request did not reach the cancellation barrier")})])
      if(preparationFailure)throw preparationFailure
      await expect(main).toHaveAttribute("data-gameui","loading")
      if(await main.getAttribute("data-console-visible")==="true")await closeConsole()
      await page.getByRole("button",{name:"Cancel",exact:true}).click()
      await expect(main).toHaveAttribute("data-phase","MainMenu",{timeout:10000})
      release();await finished
      await expect(main).toHaveAttribute("data-team-selection-visible","false")
      records.push({label:"cancelled-bsp-acquisition",phase:await main.getAttribute("data-phase")})
    }finally{release();await page.unroute(route)}
    for(const [index,next]of [target,replacement,target].entries()){
      const began=performance.now();await command(`map ${next}`)
      await expect(main).toHaveAttribute("data-team-selection-visible","true",{timeout:60000})
      await closeConsole();await chooseTf2Team(page,"red");await expect(main).toHaveAttribute("data-phase","Ready",{timeout:30000})
      const current=await state(`generation-${index+1}`)
      expect(current.geometry.target).toBe(next);expect(current.generation).toBeGreaterThan(initial.generation)
      records.at(-1).loadMilliseconds=performance.now()-began
      if(next===target)for(const field of ["resourceBytes","mapBytes","modelCount","staticProps","textures","planeBytes","particleBytes"])expect(current.memory[field]).toBe(initial.memory[field])
    }
    await writeFile(testInfo.outputPath("map-generation-lifecycle.json"),json(records));expect(errors).toEqual([]);return
  }
  if(process.env.PROFILE_MAP_PIPELINE_PROBE==="1"){
    // Rendering diagnosis only: retain real setup/rules/cadence, but seed real
    // bots into the cold view rather than waiting for them to walk there.
    await command("tf_bot_quota 15");await closeConsole();await expect(main).toHaveAttribute("data-bot-count","15")
    const state=await page.evaluate(()=>{const p=(globalThis as any).__playsrcProfile;return {point:p.controlPoints.points.find((point:any)=>point.owner===0),bots:p.bots}})
    expect(state.point).toBeTruthy()
    for(const [index,bot]of state.bots.entries())await command(`bot_teleport ${bot.identity} ${state.point.position[0]+(index%5-2)*40} ${state.point.position[1]+(Math.floor(index/5)-1)*40} ${state.point.position[2]+8}`)
    await waitNativeReady()
    await command(`setpos ${state.point.position[0]} ${state.point.position[1]} ${state.point.position[2]+8}`);await closeConsole()
    const sample=await sampleWindow()
    await writeFile(testInfo.outputPath(`${target}-pipeline-probe-performance.json`),json({phase:"pipeline-probe-only-not-map-admission",...sample,frames:summarizeFrameTimes(sample.frames)}))
    expect.soft(sample.ticks/sample.seconds).toBeGreaterThan(63)
    await capture("pipeline-probe");expect(errors).toEqual([]);return
  }
  if(process.env.PROFILE_MAP_ROCKET_SMOKE==="1"){
    expect(target).toBe("cp_granary")
    const cameras=[
      {position:[-1632,-6420,-224],yawDegrees:90,pitchDegrees:15},
      {position:[-1664,-6200,-320],yawDegrees:90,pitchDegrees:0},
    ]
    for(const phase of ["active","stopped"]){
      if(phase==="stopped"){await command("ent_fire particle_rocketsteam* Stop");await closeConsole();await page.waitForTimeout(12000)}
      for(const [index,camera]of cameras.entries()){
        await page.evaluate(camera=>{(globalThis as any).__playsrcProfile.displacementCameraOverride=camera},camera)
        await page.waitForTimeout(350)
        if(phase==="active"&&index===0){
          await page.evaluate(()=>{const p=(globalThis as any).__playsrcProfile;p.cosmeticDepthRevision=1;p.particleEvidenceRevision=1})
          await page.waitForFunction(()=>{const p=(globalThis as any).__playsrcProfile;return p.cosmeticDepthCapture?.revision===1&&p.particleEvidence?.revision===1})
          const evidence=await page.evaluate(()=>{const p=(globalThis as any).__playsrcProfile;return {particles:p.particleEvidence,depth:{width:p.cosmeticDepthCapture.buffers.width,height:p.cosmeticDepthCapture.buffers.height,bytes:Array.from(p.cosmeticDepthCapture.buffers.depth)}}})
          const consoleMaterials=evidence.particles.materialDepth.filter((material:any)=>/control_room_consoles/.test(material.identity??""))
          expect(consoleMaterials.length).toBeGreaterThan(0)
          expect(consoleMaterials.every((material:any)=>material.depthWrite&&!material.transparent)).toBe(true)
          expect(evidence.particles.items.some((item:any)=>item.material==="effects/smoke/smokelit.vmt"&&!item.sky)).toBe(true)
          const consoleDepth=Number(evidence.depth.bytes[(390*evidence.depth.width+590)*4+3])*192/255
          expect(consoleDepth).toBeGreaterThan(0);expect(consoleDepth).toBeLessThan(100)
          await writeFile(testInfo.outputPath("rocket-smoke-depth.rgba"),Buffer.from(evidence.depth.bytes as number[]))
          await writeFile(testInfo.outputPath("rocket-smoke-depth.json"),json({...evidence,consoleDepth,depth:{width:evidence.depth.width,height:evidence.depth.height}}))
        }
        await capture(`rocket-smoke-${phase}-${index}`)
      }
    }
    if(target==="cp_granary"){
      const active=decodeScreenshot(await readFile(testInfo.outputPath(`${target}-rocket-smoke-active-0-world.png`)))
      const stopped=decodeScreenshot(await readFile(testInfo.outputPath(`${target}-rocket-smoke-stopped-0-world.png`)))
      let changed=0
      // Opaque console panel, in front of the authored rocket steam emitters.
      for(let y=355;y<425;y++)for(let x=560;x<620;x++){
        const at=(y*active.width+x)*active.channels
        if([0,1,2].some(c=>Math.abs(active.pixels[at+c]-stopped.pixels[at+c])>5))changed++
      }
      await writeFile(testInfo.outputPath("rocket-console-occlusion.json"),json({changed,pixels:4200}))
      expect(changed,"rocket steam must not draw through the opaque console").toBe(0)
      const exposed=decodeScreenshot(await readFile(testInfo.outputPath(`${target}-rocket-smoke-active-1-world.png`)))
      const drained=decodeScreenshot(await readFile(testInfo.outputPath(`${target}-rocket-smoke-stopped-1-world.png`)))
      let exposedChanged=0
      for(let y=280;y<520;y++)for(let x=540;x<740;x++){
        const at=(y*exposed.width+x)*exposed.channels
        if([0,1,2].some(c=>Math.abs(exposed.pixels[at+c]-drained.pixels[at+c])>5))exposedChanged++
      }
      await writeFile(testInfo.outputPath("rocket-visible-steam.json"),json({exposedChanged,pixels:48000}))
      expect(exposedChanged,"unoccluded rocket steam must remain visible").toBeGreaterThan(16)
    }
    expect(errors).toEqual([])
    if(process.env.PROFILE_MAP_FULL!=="1")return
    await command("ent_fire particle_rocketsteam* Start");await closeConsole()
    await page.evaluate(()=>{delete (globalThis as any).__playsrcProfile.displacementCameraOverride})
  }
  if(process.env.PROFILE_MAP_SPOTLIGHT==="1"){
    const beam=facts.legacyVisuals.find((entity:any)=>entity.classname==="point_spotlight"&&Math.hypot(...entity.end.map((v:number,i:number)=>v-entity.position[i]))>1)
    expect(beam).toBeTruthy()
    const delta=beam.end.map((v:number,i:number)=>v-beam.position[i]),length=Math.hypot(...delta),direction=delta.map((v:number)=>v/length)
    let right=[direction[1],-direction[0],0],rightLength=Math.hypot(...right);right=rightLength>0?right.map(v=>v/rightLength):[1,0,0]
    const position=beam.position.map((v:number,i:number)=>v+direction[i]*Math.min(128,length*0.5)+right[i]*Math.max(96,beam.width*2))
    const aim=beam.position.map((v:number,i:number)=>v-position[i])
    const camera={position,yawDegrees:Math.atan2(aim[1],aim[0])*180/Math.PI,pitchDegrees:-Math.atan2(aim[2],Math.hypot(aim[0],aim[1]))*180/Math.PI}
    await page.evaluate(camera=>{const profile=(globalThis as any).__playsrcProfile;profile.legacyVisualProbe=true;profile.displacementCameraOverride=camera},camera)
    await writeFile(testInfo.outputPath(`${target}-spotlight-camera.json`),json({beam,camera}))
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.filter((quad:any)=>quad.source===source).length===2,beam.identity,{timeout:10000})
    const state=await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)
    const before=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-spotlight-on.png`)))
    await command("ent_fire beam Kill");await closeConsole()
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.every((quad:any)=>quad.source!==source),beam.identity)
    const after=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-spotlight-off.png`)))
    let changed=0
    for(let y=Math.floor(before.height/2)-96;y<before.height/2+96;y++)for(let x=Math.floor(before.width/2)-96;x<before.width/2+96;x++){
      const at=(y*before.width+x)*before.channels;if([0,1,2].some(c=>before.pixels[at+c]!==after.pixels[at+c]))changed++
    }
    await writeFile(testInfo.outputPath(`${target}-spotlight.json`),json({beam,camera,state,changed}))
    expect(changed).toBeGreaterThan(16);expect(errors).toEqual([]);return
  }
  if(process.env.PROFILE_MAP_SUN==="1"){
    const sun=facts.legacyVisuals.find((entity:any)=>entity.classname==="env_sun")
    expect(sun).toBeTruthy()
    await page.waitForFunction(()=>(globalThis as any).__playsrcProfile.controlPoints?.points.length>0)
    const point=await page.evaluate(()=>(globalThis as any).__playsrcProfile.controlPoints.points.find((point:any)=>point.owner===0).position)
    const direction=sun.direction
    const camera={position:[point[0],point[1],point[2]+96],yawDegrees:Math.atan2(direction[1],direction[0])*180/Math.PI,pitchDegrees:-Math.atan2(direction[2],Math.hypot(direction[0],direction[1]))*180/Math.PI}
    await page.evaluate(camera=>{const profile=(globalThis as any).__playsrcProfile;profile.legacyVisualProbe=true;profile.displacementCameraOverride=camera},camera)
    await command("ent_fire env_sun TurnOn");await closeConsole()
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.filter((quad:any)=>quad.source===source).length===2,sun.identity,{timeout:10000})
    const state=await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)
    const before=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-sun-on.png`)))
    await command("ent_fire env_sun TurnOff");await closeConsole()
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.every((quad:any)=>quad.source!==source),sun.identity)
    const after=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-sun-off.png`)))
    let changed=0
    for(let y=Math.floor(before.height/2)-96;y<before.height/2+96;y++)for(let x=Math.floor(before.width/2)-96;x<before.width/2+96;x++){
      const at=(y*before.width+x)*before.channels;if([0,1,2].some(c=>before.pixels[at+c]!==after.pixels[at+c]))changed++
    }
    await writeFile(testInfo.outputPath(`${target}-sun.json`),json({sun,camera,state,changed}))
    expect(changed).toBeGreaterThan(16);expect(errors).toEqual([]);return
  }
  if (process.env.PROFILE_MAP_LEGACY_GLOW === "1" || process.env.PROFILE_MAP_LEGACY_SPRITE === "1") {
    const spriteProbe=process.env.PROFILE_MAP_LEGACY_SPRITE === "1",probe=spriteProbe?"sprite":"glow"
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.player?.camera)
    const camera = await page.evaluate(() => (globalThis as any).__playsrcProfile.player.camera)
    const candidates = facts.legacyVisuals.filter((entity: any) => (spriteProbe?["env_sprite","env_sprite_oriented","env_glow"].includes(entity.classname):entity.classname === "env_lightglow")
      && (!process.env.PROFILE_MAP_SPRITE_MODE||Number(entity.renderMode)===Number(process.env.PROFILE_MAP_SPRITE_MODE))).map((entity: any) => {
      const delta = camera.position.map((value: number,axis: number)=>value-entity.position[axis])
      const pitch=entity.angles[0]*Math.PI/180,yaw=entity.angles[1]*Math.PI/180
      const facing=delta[0]*Math.cos(pitch)*Math.cos(yaw)+delta[1]*Math.cos(pitch)*Math.sin(yaw)-delta[2]*Math.sin(pitch)
      return { ...entity,distance:Math.hypot(...delta),facing }
    }).filter((entity: any)=>entity.distance>Number(entity.minimumDistance??0)
      && (spriteProbe||!(Number(entity.spawnflags)&1)||entity.facing>=0)
      && (Number(entity.outerMaximumDistance??0)<=Number(entity.maximumDistance??0)||entity.distance<Number(entity.outerMaximumDistance)))
      .sort((left: any,right: any)=>left.distance-right.distance)
    const glow=candidates[0]
    expect(glow).toBeTruthy()
    const glowPitch=glow.angles[0]*Math.PI/180,glowYaw=glow.angles[1]*Math.PI/180
    camera.position=glow.position.map((value:number,axis:number)=>value+[
      128*Math.cos(glowPitch)*Math.cos(glowYaw)+64*Math.sin(glowYaw),
      128*Math.cos(glowPitch)*Math.sin(glowYaw)-64*Math.cos(glowYaw),
      -128*Math.sin(glowPitch),
    ][axis]!)
    const delta=glow.position.map((value: number,axis: number)=>value-camera.position[axis])
    camera.yawDegrees=Math.atan2(delta[1],delta[0])*180/Math.PI
    camera.pitchDegrees=-Math.atan2(delta[2],Math.hypot(delta[0],delta[1]))*180/Math.PI
    await writeFile(testInfo.outputPath(`${target}-legacy-${probe}-camera.json`),json({glow,camera,candidates}))
    await page.evaluate(camera=>{const profile=(globalThis as any).__playsrcProfile;profile.legacyVisualProbe=true;profile.displacementCameraOverride=camera},camera)
    const selector=JSON.stringify(glow.name||glow.classname)
    if(spriteProbe){await command(`ent_fire ${selector} ShowSprite`);await closeConsole()}
    const attempts:unknown[]=[]
    if(spriteProbe){
      let found=false
      for(const offset of [[128,-64,0],[-128,64,0],[0,128,64],[0,-128,64],[64,0,128],[-64,0,128]]){
        camera.position=glow.position.map((value:number,axis:number)=>value+offset[axis]!)
        const delta=glow.position.map((value:number,axis:number)=>value-camera.position[axis])
        camera.yawDegrees=Math.atan2(delta[1],delta[0])*180/Math.PI;camera.pitchDegrees=-Math.atan2(delta[2],Math.hypot(delta[0],delta[1]))*180/Math.PI
        const selected=++revision
        await page.evaluate(({camera,revision})=>{const profile=(globalThis as any).__playsrcProfile;profile.displacementCameraOverride=camera;profile.geometryEvidenceRevision=revision},{camera,revision:selected})
        await page.waitForFunction(revision=>(globalThis as any).__playsrcProfile.geometryEvidence?.revision===revision,selected)
        found=await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.some((quad:any)=>quad.source===source),glow.identity,{timeout:1500}).then(()=>true,()=>false)
        attempts.push({camera:structuredClone(camera),found,evidence:await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)})
        if(found)break
      }
      await writeFile(testInfo.outputPath(`${target}-legacy-sprite-views.json`),json(attempts))
      expect(found,"an actual raster-visible view of the authored sprite").toBe(true)
    }else await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.some((quad: any)=>quad.source===source),glow.identity,{timeout:10000})
    const beforeState=await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)
    const before=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-legacy-${probe}-on.png`)))
    await command(spriteProbe?`ent_fire ${selector} HideSprite`:'ent_fire env_lightglow Color "0 0 0"');await closeConsole()
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.every((quad:any)=>quad.source!==source),glow.identity,{timeout:5000})
    const after=decodeScreenshot(await worldScreenshot(testInfo.outputPath(`${target}-legacy-${probe}-off.png`)))
    let changed=0
    for(let y=Math.floor(before.height/2)-64;y<before.height/2+64;y++)for(let x=Math.floor(before.width/2)-64;x<before.width/2+64;x++){
      const at=(y*before.width+x)*before.channels
      if([0,1,2].some(channel=>before.pixels[at+channel]!==after.pixels[at+channel]))changed++
    }
    await writeFile(testInfo.outputPath(`${target}-legacy-${probe}-pixels.json`),json({glow,camera,changed,beforeState,afterState:await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)}))
    expect(changed,"authored entity input removes visible effect pixels in the fixed central ROI").toBeGreaterThan(8)
    if(!spriteProbe||[3,9].includes(Number(glow.renderMode)))expect(beforeState[0].queries.some((query: any)=>query.source===glow.identity&&query.possible>0)).toBe(true)
    expect(errors).toEqual([])
    return
  }
  if (process.env.PROFILE_MAP_SKY_PARTICLE) {
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints?.points.length > 0)
    const emitter = facts.particleSystems.find((value: any) => value.name === process.env.PROFILE_MAP_SKY_PARTICLE)
    expect(emitter).toBeTruthy()
    const sky = facts.skyCameras[0]
    expect(sky).toBeTruthy()
    const point = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points.find((point: any) => point.owner === 0).position)
    const position = [point[0], point[1], point[2] + 768]
    const targetPosition = emitter.position.map((value: number, axis: number) => (value - sky.position[axis]) * Number(sky.scale))
    const delta = targetPosition.map((value: number, axis: number) => value - position[axis]!)
    const yawDegrees = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
    const pitchDegrees = -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI
    await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, { position, yawDegrees, pitchDegrees })
    await capture("sky-particle-visible")
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.particleEvidenceRevision = 1 })
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.particleEvidence?.revision === 1)
    const particles = await page.evaluate(() => (globalThis as any).__playsrcProfile.particleEvidence)
    const skyItems = particles.items.filter((item: any) => item.sky)
    expect(skyItems.length).toBeGreaterThan(0)
    const worldPixels = async (revision: number, name: string) => {
      await page.evaluate(revision => { (globalThis as any).__playsrcProfile.hudPixelEvidenceRevision = revision }, revision)
      await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.hudPixelEvidence?.revision === revision, revision)
      const bytes = Buffer.from(await page.evaluate(() => Array.from((globalThis as any).__playsrcProfile.hudPixelEvidence.before.bytes) as number[]))
      await writeFile(testInfo.outputPath(name), bytes)
      return decodeScreenshot(bytes)
    }
    const before = await worldPixels(1, "sky-particle-before.png")
    if (process.env.PROFILE_MAP_SKY_PARTICLE_DEBUG === "1") {
      const profiler = await page.evaluate(() => ({ uses: (globalThis as any).__playsrcFrameProfiler.firstParticleUses,
        preparation: (globalThis as any).__playsrcFrameProfiler.particlePreparation }))
      await writeFile(testInfo.outputPath("sky-particle-debug.json"), json({ particles, profiler }))
      return
    }
    const lifetime = Math.max(...skyItems.map((item: any) => item.lifetimeSeconds))
    expect(lifetime).toBeLessThanOrEqual(30)
    await command(`ent_fire ${emitter.name} Stop`); await closeConsole()
    await page.waitForTimeout((lifetime + 0.2) * 1000)
    await capture("sky-particle-stopped")
    const after = await worldPixels(2, "sky-particle-after.png")
    let changed = 0
    for (let y = before.height / 4; y < before.height / 2; y++) for (let x = before.width * 3 / 8; x < before.width * 5 / 8; x++) {
      const offset = (y * before.width + x) * before.channels
      if ([0, 1, 2].some(channel => Math.abs(before.pixels[offset + channel]! - after.pixels[offset + channel]!) > 3)) changed++
    }
    await writeFile(testInfo.outputPath("sky-particle-evidence.json"), json({ emitter, sky, particles, changed, lifetime }))
    expect(changed).toBeGreaterThan(10)
    expect(errors).toEqual([])
    return
  }
  await checkSpawn(2, "red-initial")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(2, "red-class-respawn")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await command("jointeam blue")
  await command("joinclass soldier"); await closeConsole()
  await checkSpawn(3, "blue-join")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(3, "blue-class-respawn")
  await capture("blue-spawn")
  await command("jointeam red")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await checkSpawn(2, "red-return")
  if (process.env.PROFILE_SPAWN_ONLY === "1") {
    await capture("red-spawn")
    await writeFile(testInfo.outputPath(`${target}-spawn-checks.json`), JSON.stringify({ target, spawnChecks }))
    return
  }
  await command(process.env.PROFILE_MAP_CAPTURE_ONLY==="1"?"tf_bot_add 15 red scout normal":"tf_bot_quota 15"); await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await capture("spawn")
  const before = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
  for(let attempt=0;attempt<2;attempt++){
    await page.bringToFront();await page.locator("canvas.world-canvas").click({force:true})
    try{await expect(main).toHaveAttribute("data-pointer-locked","true",{timeout:1500});break}
    catch(error){if(attempt===1)throw error}
  }
  await page.keyboard.down("w"); await page.waitForTimeout(1000); await page.keyboard.up("w")
  const after = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
  const yaw = before.camera.yawDegrees * Math.PI / 180
  const forwardDistance = (after.position[0] - before.position[0]) * Math.cos(yaw) + (after.position[1] - before.position[1]) * Math.sin(yaw)
  expect(forwardDistance, "authored forward input moves along the selected spawn facing").toBeGreaterThan(16)
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers
    && !(globalThis as any).__playsrcProfile.round.inSetup
    && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 85_000 })
  const points = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points.map((point: any) => ({ identity: point.identity, position: point.position, owner: point.owner })))
  expect(points).toHaveLength(tf2MapMode(target) === "king-of-the-hill" ? 1 : 5)
  if (tf2MapMode(target) === "king-of-the-hill") await command("ent_fire team_control_point SetUnlockTime 1")
  const point = points.find((point: any) => point.owner === 0) ?? points[Math.floor(points.length / 2)]
  // Finish native browser pointer-lock UI before the cold-view command, never
  // insert an admission/readback delay after setpos and outside its timing.
  if(await main.getAttribute("data-console-visible")!=="true")await page.keyboard.press("Backquote")
  await waitNativeReady()
  const captureOnly=process.env.PROFILE_MAP_CAPTURE_ONLY==="1"
  const cpu=!captureOnly&&process.env.PROFILE_MAP_CPU==="1"?await page.context().newCDPSession(page):null
  if(cpu){await cpu.send("Profiler.enable");await cpu.send("Profiler.start")}
  await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
  await closeConsole()
  if(!captureOnly){
  const sample = await sampleWindow()
  if(cpu){const result=await cpu.send("Profiler.stop");await writeFile(testInfo.outputPath(`${target}-main.cpuprofile`),JSON.stringify(result.profile));await cpu.detach()}
  const resultPath = testInfo.outputPath(`${target}-acceptance.json`)
  await writeFile(resultPath, json({ target,scope:"balanced-roster-traversal-and-cadence", errors, spawnChecks, ...sample, frames: summarizeFrameTimes(sample.frames) }))
  await testInfo.attach("map-acceptance", { path: resultPath, contentType: "application/json" })
  expect(sample.bots).toHaveLength(15)
  expect.soft(sample.audioAfter?.contextState).toBe("running")
  expect.soft(sample.audioAfter?.underrunFrames).toBe(0)
  expect(sample.bots.every((bot: any) => bot.area !== null)).toBe(true)
  expect(sample.bots.some((bot: any) => sample.before.some((prior: any) => prior.identity === bot.identity && Math.hypot(...bot.position.map((value: number, axis: number) => value - prior.position[axis])) > 32))).toBe(true)
  // Retain the failure, but still exercise capture/lifecycle gates so a cold-view
  // pipeline hitch cannot hide an independent gameplay admission failure.
  expect.soft(sample.ticks / sample.seconds).toBeGreaterThan(63)
  if(process.env.PROFILE_MAP_CPU==="1")return
  }else{
    await writeFile(testInfo.outputPath(`${target}-capture-only.json`),json({target,spawnChecks,performanceSample:false}))
  }
  await capture("objective")
  if(captureOnly){
  await page.waitForFunction(()=> (globalThis as any).__playsrcProfile.controlPoints.points.some((point:any)=>!point.locked
    &&((point.owner!==2&&point.mayCapture[0])||(point.owner!==3&&point.mayCapture[1]))),undefined,{timeout:5000})
  // Separate uncontested objective fixture. Its 15 Scouts were created before
  // play; the balanced-roster cadence run is independent evidence.
  const priorRoster=await page.evaluate(()=>({camera:(globalThis as any).__playsrcProfile.player.camera,bots:(globalThis as any).__playsrcProfile.bots}))
  const home = (spawnChecks[0] as any).player.position
  await command(`setpos ${home.join(" ")}`)
  const capturePlan=await page.evaluate(()=>{
    const points=(globalThis as any).__playsrcProfile.controlPoints.points
    const point=points.find((point:any)=>!point.locked&&((point.owner!==2&&point.mayCapture[0])||(point.owner!==3&&point.mayCapture[1])))
    if(!point)throw new Error("No capturable authored point remains for bot acceptance")
    return {point,team:point.owner!==2&&point.mayCapture[0]?2:3}
  })
  expect(capturePlan.team).toBe(2)
  const botCapture = await page.evaluate(({point,team}) => {
    const profile = (globalThis as any).__playsrcProfile
    const roster = JSON.parse(document.querySelector<HTMLElement>("main")!.dataset.scoreboardProbe!).players
    const candidates=profile.bots.filter((bot:any)=>bot.team===team&&bot.health>0&&[1,3,4,6,7].includes(bot.class))
      .sort((a:any,b:any)=>Number(b.class===1)-Number(a.class===1)).slice(0,3)
    return { point, team,fixture:"uncontested-15-bot-capture",
      captureBaseline:profile.bots.filter((bot:any)=>bot.team===team).map((bot:any)=>({identity:bot.identity,captures:bot.captures})),
      bots: candidates
        .map((bot: any) => ({ identity: bot.identity, captures: bot.captures, position: bot.position,
          name: roster.find((player: any) => player.identity === bot.identity).name })) }
  },capturePlan)
  expect(botCapture.bots.length).toBeGreaterThan(0)
  for (const bot of botCapture.bots) await command(`bot_teleport ${JSON.stringify(bot.name)} ${botCapture.point.position[0]} ${botCapture.point.position[1]} ${botCapture.point.position[2] + 8} 0 90 0`)
  await closeConsole()
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, priorRoster.camera)
  const botPath = testInfo.outputPath(`${target}-bot-capture-state.json`)
  const captureTimeout=Math.min(45_000,Math.max(20_000,Math.ceil(botCapture.point.captureTimes[botCapture.team-2]*1000)+2000))
  await writeFile(botPath, json({...botCapture,priorRoster}))
  let captureFailure:unknown
  try{await page.waitForFunction(({ point, team, captureBaseline }) => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.controlPoints.points.find((candidate: any) => candidate.identity === point.identity)?.owner === team
      && captureBaseline.some((before: any) => profile.bots.some((bot: any) => bot.identity === before.identity && bot.captures > before.captures))
  }, botCapture, { timeout: captureTimeout })}catch(error){captureFailure=error}
  const captured = await page.evaluate(() => ({ points: (globalThis as any).__playsrcProfile.controlPoints, bots: (globalThis as any).__playsrcProfile.bots }))
  await writeFile(botPath, json({ ...botCapture,priorRoster,captured }))
  if(captureFailure)throw captureFailure
  await capture("bot-capture")
  await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
  }
  for (const [index, point] of points.entries()) {
    await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
    await closeConsole(); await page.waitForTimeout(300)
    await capture(`point-${index}`)
  }
  expect(errors).toEqual([])
})
