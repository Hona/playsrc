import { writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { macPageAdmission, requireMacPageAdmission } from "./macos-page-admission"
import { summarizeFrameTimes } from "./profile-window"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { observeSetupObjectiveContacts } from "./setup-objective-evidence"
import { sampleSetupFrames } from "./setup-frame-sample"

test("authentic setup countdown reaches a live local round with configured audio", async ({ page }, testInfo) => {
  const { sourceCacheDir } = await loadLocalConfig()
  if (process.env.PLAYSRC_PROFILE_MANAGED !== "1" || !testInfo.outputPath("evidence").startsWith(`${path.resolve(sourceCacheDir)}${path.sep}`)) throw Error("Use the checked setup-round profile runner with evidence under sourceCacheDir")
  const map = process.env.PROFILE_MAP_TARGET ?? "pl_upward"
  const team = process.env.PROFILE_SETUP_TEAM ?? "blue"
  const reproduceOrigin = process.env.PROFILE_SETUP_FAILURE_ORIGIN === "1"
  const sampleSeconds = process.env.PROFILE_SETUP_SAMPLE_SECONDS === "10" ? 10 : 5
  const aimDown = process.env.PROFILE_SETUP_AIM_DOWN === "1"
  const faceCart = process.env.PROFILE_SETUP_CART_FACING === "1"
  const botQuota = process.env.PROFILE_SETUP_BOT_QUOTA === "16" ? 16 : 15
  if (reproduceOrigin && (map !== "pl_upward" || team !== "blue")) throw Error("The retained failed origin belongs to Upward BLU")
  if (!["pl_upward", "cp_dustbowl", "cp_gorge"].includes(map) || !["blue", "red"].includes(team)) throw Error("Unsupported setup acceptance target/team")
  const native = await macPageAdmission(page, sourceCacheDir)
  const admissions: unknown[] = []
  const checkNative = async () => {
    if (!native) return
    await expect.poll(async () => {
      const record = await native.read(); admissions.push(record)
      try {
        requireMacPageAdmission(record)
        const console = record.snapshot?.console
        if (!console?.onConsole || !console.loginDone || console.locked || console.idleMilliseconds < 2000) throw Error("An unlocked, genuinely idle native console is required")
        return "admitted"
      } catch (error) { return String(error) }
    },{timeout:6000,intervals:[500]}).toBe("admitted")
  }
  const main = page.locator("main")
  const automation = new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>,
    press: key=>page.keyboard.press(key), click: selector=>page.locator(selector).click(),
    focus: selector=>page.locator(selector).focus(), fill: (selector,value)=>page.locator(selector).fill(value),
    waitFor: async (expression,timeout)=>{await page.waitForFunction(expression,undefined,{timeout})},
    activateCurrentTab: ()=>page.bringToFront(),
  })
  const lookBy = async (x: number, y = 0) => {
    expect(await page.evaluate(()=>document.pointerLockElement===document.querySelector("canvas.world-canvas"))).toBe(true)
    await automation.player.lookBy({x,y})
    expect(await page.evaluate(()=>(globalThis as any).__playsrcBrowserTestPointer?.mode??"native")).toBe("native")
  }
  const save = async (name: string, value: unknown) => {
    await writeFile(testInfo.outputPath(`${name}.json`), JSON.stringify(value, null, 2))
  }
  await page.addInitScript(() => {
    ;(globalThis as any).__playsrcProfile ??= {}
    ;(globalThis as any).__playsrcProfile.captureFrameAdmission=true
    const probe = { contexts: [] as any[], states: [] as any[], voices: [] as any[] }
    ;(globalThis as any).__setupEvidence = probe
    const connect = AudioNode.prototype.connect
    const streams = new WeakMap<AudioContext, MediaStreamAudioDestinationNode>()
    AudioNode.prototype.connect = function (...args: any[]) {
      const result = (connect as any).apply(this, args)
      if (args[0] === this.context.destination && this.context instanceof AudioContext) {
        let stream = streams.get(this.context)
        if (!stream) {
          stream = this.context.createMediaStreamDestination(); streams.set(this.context, stream)
          const chunks: Blob[] = [], recorder = new MediaRecorder(stream.stream)
          recorder.ondataavailable = e => chunks.push(e.data)
          recorder.start(1000)
          probe.contexts.push({ context: this.context, recorder, chunks })
        }
        connect.call(this, stream)
      }
      return result
    } as typeof connect
    const create = AudioContext.prototype.createBufferSource
    AudioContext.prototype.createBufferSource = function () {
      const source = create.call(this), start = source.start.bind(source)
      source.start = (...args: Parameters<typeof source.start>) => {
        probe.voices.push({at: performance.now(), audio: this.currentTime, state:this.state, duration:source.buffer?.duration, channels:source.buffer?.numberOfChannels})
        return start(...args)
      }
      return source
    }
    setInterval(() => {
      const p = (globalThis as any).__playsrcProfile, d = document.querySelector<HTMLElement>("main")?.dataset
      if (p.round) probe.states.push({at:performance.now(), tick:d?.snapshotTick, round:p.round, player:p.player, points:p.controlPoints, bots:p.bots, audio:d?.audioStarts, phase:d?.phase, camera:d?.cameraPosition, visibility:document.visibilityState})
    },1000)
  })
  await page.addInitScript(`(${observeSetupObjectiveContacts.toString()})(globalThis.__playsrcProfile ??= {})`)
  await page.addInitScript(`(globalThis.__playsrcProfile ??= {}).setupSampleFrames = (${sampleSetupFrames.toString()})`)
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const input = page.locator("[aria-label='Console command']")
    await input.fill(text); await input.press("Enter"); await page.keyboard.press("Backquote")
  }
  const capture = async (name: string) => {
    await checkNative()
    await page.screenshot({path:testInfo.outputPath(`${name}.png`)})
    await save(name,await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile
      return {data:{...document.querySelector<HTMLElement>("main")!.dataset},round:p.round,player:p.player,bots:p.bots,points:p.controlPoints,frameAdmission:p.frameAdmission}
    }))
  }
  const walkToGorgeExit = async () => {
    // The spawn room has a wall between the spawn and gate. Follow the same
    // open corridor traversed by the authored navigation route, not a teleport.
    const directions = [["KeyW"],["KeyW","KeyA"],["KeyA"],["KeyS","KeyA"],["KeyS"],["KeyS","KeyD"],["KeyD"],["KeyW","KeyD"]]
    let held: string[] = []
    try {
      for (const [x,y] of [[-5436,7685],[-5483,7408],[-5216,7330]]) {
        let reached = false
        let slow = false
        for (let step = 0; step < 40; step++) {
          const state = await page.evaluate(({x,y}) => {
            const d = document.querySelector<HTMLElement>("main")!.dataset
            const [px,py] = d.cameraPosition!.split(",").map(Number)
            const angle = Math.atan2(y-py!,x-px!)*180/Math.PI-Number(d.cameraYaw)
            return {distance:Math.hypot(x-px!,y-py!),direction:((Math.round(angle/45)%8)+8)%8}
          },{x:x!,y:y!})
          if (state.distance<24) { reached=true; break }
          if (!slow && state.distance<180) {
            for (const key of held) await page.keyboard.up(key)
            held=[];slow=true
            await page.waitForTimeout(700)
            continue
          }
          const next = directions[state.direction]!
          for (const key of held) if (!next.includes(key)) await page.keyboard.up(key)
          for (const key of next) if (!held.includes(key)) await page.keyboard.down(key)
          held = next
          await page.waitForTimeout(slow?30:100)
          if (slow) {
            for (const key of held) await page.keyboard.up(key)
            held=[]
            await page.waitForTimeout(300)
          }
        }
        for (const key of held) await page.keyboard.up(key)
        held = []
        expect(reached,`walk to ${x},${y}`).toBe(true)
        await page.waitForTimeout(200)
      }
    } finally { for (const key of held) await page.keyboard.up(key) }
    const delta = await page.evaluate(() => {
      const yaw=Number(document.querySelector<HTMLElement>("main")!.dataset.cameraYaw)
      return ((yaw-270+180)%360+360)%360-180
    })
    if (Math.abs(delta)>0.01) await lookBy(delta/0.066)
  }
  const walkToFailedUpwardOrigin = async () => {
    const directions = [["KeyW"],["KeyW","KeyA"],["KeyA"],["KeyS","KeyA"],["KeyS"],["KeyS","KeyD"],["KeyD"],["KeyW","KeyD"]]
    // Match the retained failed live-round camera using ordinary input during
    // real setup, not a camera override, teleport or authored respawn mutation.
    await page.keyboard.down("ControlLeft")
    try {
      let reached=false
      for(let step=0;step<100;step++) {
        const state=await page.evaluate(()=>{
          const d=document.querySelector<HTMLElement>("main")!.dataset
          const [x,y]=d.cameraPosition!.split(",").map(Number)
          const angle=Math.atan2(-1680-y!,-2592-x!)*180/Math.PI-Number(d.cameraYaw)
          return {distance:Math.hypot(-2592-x!,-1680-y!),direction:((Math.round(angle/45)%8)+8)%8}
        })
        if(state.distance<2){reached=true;break}
        const keys=directions[state.direction]!
        for(const key of keys)await page.keyboard.down(key)
        await page.waitForTimeout(state.distance>30?50:16)
        for(const key of keys)await page.keyboard.up(key)
        await page.waitForTimeout(180)
      }
      expect(reached,"native walk to retained failed Upward origin").toBe(true)
    } finally { await page.keyboard.up("ControlLeft") }
    await page.waitForTimeout(1000)
    const delta=await page.evaluate(()=>{
      const yaw=Number(document.querySelector<HTMLElement>("main")!.dataset.cameraYaw)
      return ((yaw-315+180)%360+360)%360-180
    })
    if(Math.abs(delta)>0.01)await lookBy(delta/0.066)
    await capture("matched-failure-origin")
  }
  try {
    await page.goto("/")
    if (native) {
      const target = await native.read()
      await save("native-focus-target",target)
      console.log(`SETUP_NATIVE_TARGET ${JSON.stringify({pid:target.linkage?.browserPid,window:target.linkage?.nativeWindowId})}`)
    }
    await expect(main).toHaveAttribute("data-phase","MainMenu",{timeout:60_000})
    await command(`map ${map}`)
    await expect(main).toHaveAttribute("data-team-selection-visible","true",{timeout:60_000})
    await page.locator(`.team-selection-layer [data-vgui-name='teambutton${team === "blue" ? 0 : 1}']`).click()
    await expect(main).toHaveAttribute("data-class-selection-visible","true")
    await page.keyboard.press("Digit1")
    await expect(main).toHaveAttribute("data-class-selection-visible","false")
    await expect(main).toHaveAttribute("data-phase","Ready")
    await command(`tf_bot_quota ${botQuota}`)
    await page.bringToFront()
    await page.locator("canvas.world-canvas").click()
    await expect(main).toHaveAttribute("data-pointer-locked","true")
    await checkNative()
    if (map === "cp_gorge" && team === "blue") {
      await walkToGorgeExit()
    }
    await capture("admitted-setup")
    await page.waitForFunction(() => {
      const r = (globalThis as any).__playsrcProfile.round
      return r && r.state === 4 && !r.waitingForPlayers && r.inSetup
    },undefined,{timeout:40_000})
    if (map === "cp_gorge" && team === "blue") {
      // Walk to the authored exit after the waiting-for-players respawn. The
      // real setup timer continues while ordinary native input positions us.
      await walkToGorgeExit()
    }
    if(reproduceOrigin)await walkToFailedUpwardOrigin()
    await capture("authentic-setup")
    await page.waitForFunction(() => {
      const r = (globalThis as any).__playsrcProfile.round
      return r && r.state === 4 && !r.waitingForPlayers && !r.inSetup
    },undefined,{timeout:125_000})
    await capture("live-round")
    if(faceCart) {
      const delta=await page.evaluate(()=>((Number(document.querySelector<HTMLElement>("main")!.dataset.cameraYaw)+180)%360+360)%360-180)
      await lookBy(delta/0.066); await capture("native-cart-facing-aim")
    }
    if(aimDown) { await lookBy(0,45/0.066); await capture("native-downward-aim") }
    await page.keyboard.down("KeyW"); await page.mouse.down()
    const sample = await page.evaluate(async (sampleSeconds) => {
      const d = document.querySelector<HTMLElement>("main")!.dataset, p = (globalThis as any).__playsrcProfile
      const tick = Number(d.snapshotTick), before = d.cameraPosition, bots = structuredClone(p.bots)
      const timing = await p.setupSampleFrames(undefined,undefined,sampleSeconds)
      return {...timing,ticks:Number(d.snapshotTick)-tick,before,after:d.cameraPosition,botsBefore:bots,botsAfter:p.bots,round:p.round,points:p.controlPoints,audio:d.audioStarts}
    },sampleSeconds)
    await page.keyboard.up("KeyW"); await page.mouse.up()
    await save("live-sample-raw",sample)
    await save("live-sample",{...sample,summary:summarizeFrameTimes(sample.frames)})
    // Look back over the traversed route for the endpoint pixels, rather than
    // leaving the camera against the obstacle reached by the forward input.
    await lookBy(180/0.066)
    await capture("movement-firing-bots")
    expect(sample.ticks/sample.seconds).toBeGreaterThan(65)
    expect(sample.after).not.toBe(sample.before)
    expect(sample.botsAfter.length).toBe(botQuota)
    expect(sample.audio).toContain("Weapon_Scatter_Gun.Single")
    for (const seconds of [60,30,10,5,4,3,2,1]) expect(sample.audio).toContain(`Announcer.RoundBegins${seconds}Seconds:sound/vo/announcer_begins_${seconds}sec.mp3`)
    expect(sample.audio).toContain("Ambient.Siren:sound/ambient_mp3/siren.mp3")
    if (map === "cp_gorge") {
      await page.waitForFunction(() => (globalThis as any).__playsrcProfile.bots.some((b: any) => b.team === 3 && b.position[1] < 7000),undefined,{timeout:5000})
      await capture("bots-crossed-setup-gates")
      if (team === "blue") {
        expect(Number(sample.after!.split(",")[1])).toBeLessThan(7000)
        await capture("player-crossed-setup-gate")
      }
    }
    if (map !== "pl_upward") {
      await page.waitForFunction(() => (globalThis as any).__playsrcProfile.setupObjectiveContacts.length > 0,undefined,{timeout:25000})
      await capture("live-objective-interaction")
    }
    if (map === "pl_upward") {
      const blockers = JSON.parse(await main.getAttribute("data-blockers") ?? "[]") as string[]
      await save("objective-admission",{complete:false,blockers:blockers.filter(value=>value.includes("Payload cart"))})
    }
    expect(await main.getAttribute("data-phase")).toBe("Ready")
  } finally {
    await page.keyboard.up("KeyW").catch(()=>{})
    await page.mouse.up().catch(()=>{})
    const evidence = await page.evaluate(async () => {
      const p = (globalThis as any).__setupEvidence
      const audio: string[] = []
      for (const c of p.contexts) {
        if (c.recorder.state !== "inactive") await new Promise<void>(resolve => { c.recorder.onstop=()=>resolve();c.recorder.stop() })
        const bytes = new Uint8Array(await new Blob(c.chunks).arrayBuffer()); let binary=""
        for (const byte of bytes) binary+=String.fromCharCode(byte)
        audio.push(btoa(binary))
      }
      return {states:p.states,voices:p.voices,audio,objectiveContacts:(globalThis as any).__playsrcProfile.setupObjectiveContacts}
    }).catch(error => ({error:String(error),audio:[]}))
    for (const [i, audio] of evidence.audio.entries()) await writeFile(testInfo.outputPath(`countdown-output-${i}.webm`),Buffer.from(audio,"base64"))
    await save("lifecycle",{...evidence,audio:undefined,map,team})
    await save("native-admission",admissions)
    if (native) await save("private-terminal-native",await native.read(testInfo.outputPath("private-terminal-desktop.png")))
    await page.screenshot({path:testInfo.outputPath("terminal.png")}).catch(()=>{})
    await native?.close()
  }
})
