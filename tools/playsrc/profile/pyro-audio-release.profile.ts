import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

test("real Mouse1 Pyro release retires the running WebAudio flame graph", async ({ page }) => {
  const output = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "pyro-audio-release", process.env.PROFILE_AUDIO_BASELINE === "1" ? "before" : "after")
  await mkdir(output, { recursive: true })
  await page.addInitScript(() => {
    const probe = { voices: [] as any[], edges: [] as any[], commands: [] as any[], frames: [] as number[], chunks: [] as Blob[], recording: false, recorder: null as MediaRecorder | null, context: null as AudioContext | null }
    ;(globalThis as any).__flameAudio = probe
    const post = Worker.prototype.postMessage
    Worker.prototype.postMessage = function (message: any, transfer?: any) {
      if (message?.command instanceof ArrayBuffer && message.command.byteLength >= 32) {
        const flags = new DataView(message.command).getUint32(28, true)
        if (probe.commands.at(-1)?.flags !== flags) probe.commands.push({ flags, at: performance.now(), audio: probe.context?.currentTime, kind: message.kind })
      }
      return post.call(this, message, transfer)
    }
    const create = AudioContext.prototype.createBufferSource
    AudioContext.prototype.createBufferSource = function () {
      const source = create.call(this)
      const voice: any = { started: null, stopped: null, ended: null, disconnected: null }
      probe.voices.push(voice)
      const start = source.start.bind(source), stop = source.stop.bind(source), disconnect = source.disconnect.bind(source)
      source.start = (...args: Parameters<typeof source.start>) => {
        Object.assign(voice, { started: this.currentTime, when: args[0] ?? this.currentTime, duration: source.buffer?.duration, loop: source.loop, loopStart: source.loopStart, loopEnd: source.loopEnd })
        start(...args)
      }
      source.stop = (...args: Parameters<typeof source.stop>) => { voice.stopped = this.currentTime; stop(...args) }
      source.disconnect = () => { voice.disconnected = this.currentTime; disconnect() }
      source.addEventListener("ended", () => { voice.ended = this.currentTime })
      return source
    }
    const connect = AudioNode.prototype.connect
    const streams = new WeakMap<AudioContext, MediaStreamAudioDestinationNode>()
    AudioNode.prototype.connect = function (...args: any[]) {
      const result = (connect as any).apply(this, args)
      if (args[0] === this.context.destination && this.context instanceof AudioContext) {
        const context = this.context
        let stream = streams.get(context)
        if (!stream) {
          stream = context.createMediaStreamDestination()
          streams.set(context, stream)
          // Keep the recording clock continuous even when the audible graph is
          // empty. This zero-valued node feeds only the recorder, never speakers.
          const clock = context.createConstantSource()
          clock.offset.value = 0
          clock.connect(stream)
          clock.start()
          probe.context = context
          probe.recorder = new MediaRecorder(stream.stream)
          probe.recorder.ondataavailable = event => probe.chunks.push(event.data)
          if (probe.recording) probe.recorder.start()
        }
        connect.call(this, stream)
      }
      return result
    } as typeof connect
    for (const type of ["mousedown", "mouseup", "blur", "pointerlockchange"]) {
      ;(type === "pointerlockchange" ? document : window).addEventListener(type, (event: Event) => {
        probe.edges.push({ type, trusted: event.isTrusted, button: (event as MouseEvent).button, at: performance.now(), audio: probe.context?.currentTime, locked: !!document.pointerLockElement })
      }, true)
    }
  })
  await page.goto("/", { waitUntil: "domcontentloaded" })
  const main = page.locator("main")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
  await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
  const dialog = page.locator(".local-match-layer").getByRole("dialog", { name: "CREATE SERVER" })
  await dialog.locator("[data-vgui-name='MapList']").click()
  await page.getByRole("option", { name: "jump_beef", exact: true }).click()
  await dialog.getByRole("button", { name: "Start", exact: true }).click()
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit3")
  await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("7")
  await page.bringToFront()
  await page.evaluate(() => { window.focus(); document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!.focus() })
  await page.locator("canvas.world-canvas").click()
  await page.waitForFunction(() => document.pointerLockElement !== null || document.querySelector<HTMLElement>("main")?.dataset.detail?.startsWith("Pointer lock failed:"), undefined, { timeout: 5_000 })
  if (await main.getAttribute("data-pointer-locked") !== "true") {
    await page.screenshot({ path: path.join(output, "pointer-failure.png") })
    throw new Error(`Native pointer lock required: ${await main.getAttribute("data-detail")}`)
  }
  await page.evaluate(() => {
    const p = (globalThis as any).__flameAudio
    p.recording = true
    p.recorder?.start()
    p.startTick = Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick)
    p.startTime = performance.now()
    const frame = (now: number) => { p.frames.push(now); if (!p.done) requestAnimationFrame(frame) }
    requestAnimationFrame(frame)
  })
  await page.mouse.down()
  await page.waitForTimeout(process.env.PROFILE_AUDIO_BASELINE === "1" ? 1_000 : 4_000)
  await page.screenshot({ path: path.join(output, "holding.png") })
  await page.mouse.up()
  await page.waitForTimeout(process.env.PROFILE_AUDIO_BASELINE === "1" ? 4_000 : 1_200)
  await page.mouse.down()
  await page.waitForTimeout(500)
  await page.mouse.up()
  await page.waitForTimeout(1_200)
  if (process.env.PROFILE_AUDIO_BASELINE !== "1") {
    await page.keyboard.press("Digit2")
    await page.waitForTimeout(600)
    await page.mouse.down()
    await page.waitForTimeout(60)
    await page.mouse.up()
    await page.waitForTimeout(500)
  }
  const evidence = await page.evaluate(async () => {
    const p = (globalThis as any).__flameAudio
    p.done = true
    await new Promise<void>(resolve => { p.recorder.onstop = () => resolve(); p.recorder.stop() })
    const bytes = new Uint8Array(await new Blob(p.chunks).arrayBuffer())
    let binary = ""
    for (const byte of bytes) binary += String.fromCharCode(byte)
    return { voices: p.voices, edges: p.edges, commands: p.commands, frames: p.frames, startTick: p.startTick, endTick: Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick), milliseconds: performance.now() - p.startTime, now: p.context.currentTime, audio: btoa(binary), state: { ...document.querySelector<HTMLElement>("main")!.dataset } }
  })
  const { audio, ...report } = evidence
  await writeFile(path.join(output, "output.webm"), Buffer.from(audio, "base64"))
  await writeFile(path.join(output, "graph.json"), JSON.stringify(report, null, 2))
  await page.screenshot({ path: path.join(output, "released.png") })
  expect(report.edges.filter((edge: any) => edge.type === "mouseup").every((edge: any) => edge.trusted)).toBe(true)
  if (process.env.PROFILE_AUDIO_BASELINE !== "1") {
    const flame = report.voices.filter((voice: any) => Math.abs(voice.duration - 160064 / 44100) < 0.001)
    expect(flame.length).toBeGreaterThanOrEqual(4)
    expect(flame.every((voice: any) => (voice.stopped !== null || voice.ended !== null) && voice.disconnected !== null)).toBe(true)
    const loops = flame.filter((voice: any) => voice.loop)
    expect(loops).toHaveLength(2)
    const releases = report.edges.filter((edge: any) => edge.type === "mouseup" && edge.locked)
    for (let index = 0; index < loops.length; index++) {
      expect(loops[index].stopped - releases[index].audio).toBeGreaterThanOrEqual(0)
      expect(loops[index].stopped - releases[index].audio).toBeLessThan(0.15)
    }
    const tails = report.voices.filter((voice: any) => Math.abs(voice.duration - 36096 / 44100) < 0.001)
    expect(tails).toHaveLength(2)
    expect(tails.every((voice: any) => voice.stopped === null && voice.ended - voice.started >= voice.duration)).toBe(true)
    expect(report.state.audioStarts).toContain("Weapon_Shotgun.Single")
    expect(report.voices.filter((voice: any) => voice.loop && voice.stopped === null && voice.ended === null)).toHaveLength(0)
  }
})
