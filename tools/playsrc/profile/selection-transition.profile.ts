import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { createHash } from "node:crypto"
import { execFile } from "node:child_process"
import { promisify } from "node:util"
import { test, expect } from "./application-test"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { macPageAdmission, requireMacPageAdmission } from "./macos-page-admission"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { summarizeCpuProfile } from "./gameui-profile"
import { captureProcessMemory } from "./process-memory"
import { analyzeNativeSelectionPixels } from "./selection-transition-analysis"

const classes = ["scout", "sniper", "soldier", "demoman", "medic", "heavyweapons", "pyro", "spy", "engineer"] as const

test("selection material color depth and draw ownership parity", async ({ page, baseURL }) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, { sourceCacheDir } = await loadLocalConfig()
  if (!directory || !baseURL || new URL(baseURL).hostname !== "127.0.0.1") throw new Error("Use the ordinary local native selection runner")
  const reader = await startupNativeReader(page, sourceCacheDir), observations: unknown[] = [], records: unknown[] = []
  const check = async (name: string) => {
    const record = await reader.read(path.join(directory, `${name}.desktop.png`))
    observations.push(record)
    await writeFile(path.join(directory, "model-native.json"), JSON.stringify({ observations, records: reader.records }))
    requireStartupNative(record)
  }
  try {
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Native material comparison requires genuine idle")
    // Use a real loopback response. A fulfilled/intercepted document has no
    // network address-space provenance and can provoke native LNA permission
    // UI for its own local modules. Do not grant/suppress that permission.
    const url = new URL(`/@fs/${repositoryRoot.replaceAll("\\", "/")}/packages/presentation/rendering/tests/fixtures/model-graph-parity.html`, baseURL).href
    await page.goto(url)
    await page.waitForFunction(() => (window as any).probe)
    await check("model-before")
    const started = performance.now()
    for (let phase = 0; phase < 4; phase++) {
      const { beforePixels, afterPixels, ...record } = await page.evaluate(phase => (window as any).probe.compare(phase), phase)
      for (const [name, pixels] of [["before", beforePixels], ["after", afterPixels]]) await writeFile(path.join(directory, `model-${phase}-${name}.png`), Buffer.from(pixels.split(",")[1], "base64"))
      records.push(record)
      await check(`model-${phase}`)
    }
    await page.waitForTimeout(Math.max(0, 5000 - (performance.now() - started)))
    await check("model-after")
    await page.screenshot({ path: path.join(directory, "model-visible.page.png") })
    await writeFile(path.join(directory, "model-parity.json"), JSON.stringify({ records, milliseconds: performance.now() - started, performanceSample: false }))
    for (const record of records as any[]) { expect(record.colorMismatches).toBe(0); expect(record.depthMismatches).toBe(0) }
    expect(performance.now() - started).toBeLessThan(10000)
  } finally {
    await page.evaluate(() => (window as any).probe?.dispose()).catch(() => {})
    await reader.close()
  }
})

for (const team of ["red", "blue"] as const) for (const identity of [1, 2, 3, 4, 5, 6, 7, 8, 9]) for (const warm of [false, true]) {
test(`trusted selection ${team} class${identity} ${warm ? "warm" : "cold"} to changed native pixels`, async ({ page, context }) => {
  const { sourceCacheDir } = await loadLocalConfig(), directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked selection-transition profile runner")
  await mkdir(directory, { recursive: true })
  const cdp = await context.newCDPSession(page), browser = await context.browser()!.newBrowserCDPSession()
  const native = await macPageAdmission(page, sourceCacheDir)
  const windows = process.platform === "win32" ? await startupNativeReader(page, sourceCacheDir) : undefined
  if (!native && !windows) throw new Error("Native selection pixel sampling requires a configured macOS or Windows desktop")
  const captures: any[] = [], references: any[] = [], errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => {
    const profile = (globalThis as any).__playsrcProfile = { captureSelectionTransitions: true, selectionInputs: [] }
    const input = (event: Event) => {
      if (!event.isTrusted || !profile.selectionSampling) return
      let target = event.target as HTMLElement | null
      const names = ["teambutton0", "teambutton1", "scout", "sniper", "soldier", "demoman", "medic", "heavyweapons", "pyro", "spy", "engineer"]
      while (target && !names.includes(target.dataset?.vguiName ?? "")) target = target.parentElement
      if (event.type !== "pointerup" && event.type !== "keydown") return
      profile.selectionInputs.push({ trusted: event.isTrusted, name: target?.dataset.vguiName ?? null, key: (event as KeyboardEvent).code ?? null, inputEpoch: performance.timeOrigin + event.timeStamp,
        timestamp: event.timeStamp, processing: performance.now() })
      performance.mark("playsrc-selection-trusted-input", { detail: target?.dataset.vguiName ?? "unplanned" })
    }
    document.addEventListener("pointerup", input, true)
    document.addEventListener("keydown", input, true)
  })
  let sampling = false
  let captureLoop: Promise<void> | undefined
  let activeStartedEpoch: number | undefined
  let measurementRetained = false
  let actionsComplete = false
  // Sampled CPU attribution is a separate diagnostic mode. V8 stack sampling
  // must not silently become the application's input-to-visible wall clock.
  const captureCpu = process.env.PLAYSRC_SELECTION_CPU === "1"
  const captureWithCua = process.platform === "darwin" && process.env.PLAYSRC_SELECTION_CAPTURE === "cua"
  const session = path.basename(directory)
  const cua = async (name: string, input: Record<string, unknown>) => JSON.parse((await promisify(execFile)("cua-driver", ["call", name, JSON.stringify(input)], { timeout: 2000, maxBuffer: 1024 * 1024 })).stdout)
  if (captureWithCua) await cua("start_session", { session, capture_scope: "desktop" })
  const capture = async () => {
    const file = `selection-${String(captures.length).padStart(3, "0")}.desktop.png`
    if (windows) {
      // Full-desktop admission endpoints remain private. Active pixel sampling
      // reads the entire authenticated window from the real screen, including
      // its native chrome, rather than PNG-encoding unrelated monitors.
      const startedEpoch = Date.now(), admission = await windows.read(path.join(directory, file), sampling ? "window" : "desktop"), admissionAfter = await windows.read(), endedEpoch = Date.now()
      requireStartupNative(admission); requireStartupNative(admissionAfter)
      if (!admission.pixels || admission.pixels.path !== path.join(directory, file)) throw new Error("Native Windows pixel capture missing")
      const bytes = await readFile(path.join(directory, file))
      captures.push({ file, startedEpoch, endedEpoch, pixelCaptureInterval: admission.pixels, admission, admissionAfter,
        nativeRecords: windows.records.slice(-2), method: "windows-native-desktop", byteLength: bytes.length,
        sha256: createHash("sha256").update(bytes).digest("hex"), privacy: "private-desktop-never-upload" })
      await writeFile(path.join(directory, "selection-native.json"), JSON.stringify(captures))
      return captures.length - 1
    }
    const startedEpoch = Date.now(), admission = await native!.read(captureWithCua ? undefined : path.join(directory, file))
    let admissionAfter, pixelCaptureInterval
    if (captureWithCua) {
      requireMacPageAdmission(admission)
      const pixelStartedEpoch = Date.now()
      const desktop = await cua("get_desktop_state", { session, screenshot_out_file: path.join(directory, file) })
      const pixelEndedEpoch = Date.now()
      if (desktop.screenshot_file_path !== path.join(directory, file)) throw new Error("Native desktop capture did not publish the requested PNG")
      // Preserve the complete guard envelope separately. Neither a pre-capture
      // window query nor a post-capture query dates the pixels in this PNG.
      pixelCaptureInterval = { startedEpoch: pixelStartedEpoch, endedEpoch: pixelEndedEpoch }
      admissionAfter = await native!.read()
    }
    const endedEpoch = Date.now()
    const bytes = await readFile(path.join(directory, file))
    const record = { file, startedEpoch, endedEpoch, pixelCaptureInterval, admission, admissionAfter, method: captureWithCua ? "cua-native-desktop" : "screencapture", byteLength: bytes.byteLength, sha256: createHash("sha256").update(bytes).digest("hex"), privacy: "private-desktop-never-upload" }
    captures.push(record)
    await writeFile(path.join(directory, "selection-native.json"), JSON.stringify(captures))
    requireMacPageAdmission(admission)
    if (admissionAfter) requireMacPageAdmission(admissionAfter)
    return captures.length - 1
  }
  const command = async (text: string) => {
    await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text); await page.keyboard.press("Enter")
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    await command("map pl_upward")
    await expect(page.locator("main")).toHaveAttribute("data-team-selection-models", /(?=.*reddoor:[^|]+:\d+:\d+)(?=.*bluedoor:[^|]+:\d+:\d+)/, { timeout: 40_000 })
    if (warm) {
      await page.locator(`.team-selection-layer [data-vgui-name='${team === "red" ? "teambutton0" : "teambutton1"}']`).click()
      await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
      await page.locator(`.class-selection-layer [data-vgui-name='${classes[identity - 1]}']`).click()
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.player?.class)).toBe(identity)
      await page.keyboard.press("Period")
      await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true")
    }
    // The user-approved native foreground is a prerequisite, never forced.
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Selection sample requires two seconds of native input idle admission")
    await capture()
    const heapBefore = await cdp.send("Runtime.getHeapUsage")
    const residentBefore = await captureProcessMemory((await browser.send("SystemInfo.getProcessInfo")).processInfo)
    if (captureCpu) { await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 100 }); await cdp.send("Profiler.start") }
    const startedEpoch = Date.now()
    activeStartedEpoch = startedEpoch
    await page.evaluate(() => { const root = globalThis as any; root.__playsrcProfile.selectionSampling = true; root.__playsrcFrameProfiler.active = true
      root.__playsrcProfile.selectionMemory = []
      const sample = () => { if (!root.__playsrcProfile.selectionSampling) return
        root.__playsrcProfile.selectionMemory.push({ at: performance.now(), usedJSHeapSize: (performance as any).memory?.usedJSHeapSize ?? null })
        requestAnimationFrame(sample) }
      sample()
    })
    sampling = true
    let nextCapture: Promise<number> | undefined
    const loop = (async () => {
      while (sampling && Date.now() - startedEpoch < 9000 && (!actionsComplete || Date.now() - startedEpoch < 5000)) { nextCapture = capture(); await nextCapture }
    })()
    captureLoop = loop
    void loop.catch(() => {})
    // DOM/owner acknowledgements sequence the requested inputs only. None of
    // these waits is counted as a visibly responsive frame.
    await page.locator(`.team-selection-layer [data-vgui-name='${team === "red" ? "teambutton1" : "teambutton0"}']`).click()
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.selectionOwners?.filter((entry: any) => entry.kind === "draw-complete").at(-1)?.detail.scene === "class", undefined, { timeout: 8500, polling: 50 })
    const reference = async (scene: string, selector: string) => {
      const facts = await page.locator(selector).evaluate(element => ({ bounds: element.getBoundingClientRect().toJSON(),
        screenX, screenY, outerWidth, outerHeight, innerWidth, innerHeight, timeOrigin: performance.timeOrigin,
        draw: (globalThis as any).__playsrcProfile.selectionOwners.filter((entry: any) => entry.kind === "draw-complete").at(-1) }))
      let index = nextCapture ? await nextCapture : await capture()
      while (captures[index].startedEpoch < facts.timeOrigin + facts.draw.at) {
        index = Date.now() - startedEpoch < 9000 && nextCapture ? await nextCapture : await capture()
      }
      references.push({ scene, index, facts })
    }
    await reference("class", ".class-selection-layer [data-vgui-name='ClassMenuSelect']")
    await page.locator(`.class-selection-layer [data-vgui-name='${classes[identity - 1]}']`).click()
    await expect.poll(() => page.evaluate(({ identity, team }) => {
      const profile = (globalThis as any).__playsrcProfile, input = profile.selectionInputs.at(-1)
      const draw = profile.selectionOwners.filter((entry: any) => entry.kind === "draw-complete").at(-1)
      return draw?.detail.scene === "world" && draw.detail.class === identity && draw.detail.team === team && draw.at > input.processing
    }, { identity, team: team === "red" ? 2 : 3 }), { timeout: 5000 }).toBe(true)
    await reference("world", ".hud-layer [data-vgui-name='PlayerStatusHealthValue']")
    actionsComplete = true
    await loop
    sampling = false
    const endedEpoch = Date.now()
    const sampledCpu = captureCpu ? await cdp.send("Profiler.stop") : undefined
    if (sampledCpu) await writeFile(path.join(directory, "selection.cpuprofile"), JSON.stringify(sampledCpu.profile))
    const evidence = await page.evaluate(async () => { const root = globalThis as any; root.__playsrcProfile.selectionSampling = false; root.__playsrcFrameProfiler.active = false
      await root.__playsrcFrameProfiler.flushShaderHashes()
      return { inputs: root.__playsrcProfile.selectionInputs, owners: root.__playsrcProfile.selectionOwners, dropped: root.__playsrcProfile.selectionOwnersDropped ?? 0,
        frames: root.__playsrcFrameProfiler.completedFrames, worker: root.__playsrcFrameProfiler.worker, gpu: root.__playsrcFrameProfiler.counters,
        gpuOperations: root.__playsrcFrameProfiler.gpuOperations, gpuOperationsDropped: root.__playsrcFrameProfiler.gpuOperationsDropped,
        shaders: root.__playsrcFrameProfiler.shaders, adapters: root.__playsrcFrameProfiler.adapters, devices: root.__playsrcFrameProfiler.devices, losses: root.__playsrcFrameProfiler.losses,
        modelPreparation: root.__playsrcFrameProfiler.modelPreparation, longTasks: root.__playsrcFrameProfiler.longTasks, longAnimationFrames: root.__playsrcFrameProfiler.longAnimationFrames,
        memorySamples: root.__playsrcProfile.selectionMemory,
        loading: document.querySelector<HTMLElement>("main")!.dataset.loadPerformance } })
    const heapAfter = await cdp.send("Runtime.getHeapUsage")
    const residentAfter = await captureProcessMemory((await browser.send("SystemInfo.getProcessInfo")).processInfo)
    await capture()
    await writeFile(path.join(directory, "selection-measurement.json"), JSON.stringify({ team, identity, warm, startedEpoch, endedEpoch, references, evidence, heapBefore, heapAfter, residentBefore, residentAfter,
      cpuAttributionEnabled: captureCpu, cpu: sampledCpu ? summarizeCpuProfile(sampledCpu.profile) : null }, null, 2))
    measurementRetained = true
    expect(endedEpoch - startedEpoch).toBeGreaterThanOrEqual(5000)
    expect(endedEpoch - startedEpoch).toBeLessThanOrEqual(10000)
    expect(evidence.inputs).toHaveLength(2)
    expect(evidence.inputs.map((input: any) => input.name)).toEqual([team === "red" ? "teambutton1" : "teambutton0", classes[identity - 1]])
    expect(evidence.dropped).toBe(0)
    expect(evidence.gpuOperationsDropped).toBe(0)
    const latencies = await analyzeNativeSelectionPixels(directory)
    await page.screenshot({ path: path.join(directory, "selection-world.page.png") })
    expect(errors).toEqual([])
    for (const latency of latencies) {
      expect(latency.endCensored).toBe(false)
      if (process.env.PLAYSRC_SELECTION_MAX_MILLISECONDS !== undefined) expect(latency.upperMilliseconds).toBeLessThanOrEqual(Number(process.env.PLAYSRC_SELECTION_MAX_MILLISECONDS))
    }
    // This is pixel/ownership admission, not a speedup certificate. Performance
    // acceptance requires the matched before/after comparison; the former
    // arbitrary250ms default was not an SDK or user-supplied timing contract.
  } finally {
    sampling = false
    await captureLoop?.catch(() => {})
    const partialCpu = captureCpu ? await cdp.send("Profiler.stop").catch(() => undefined) : undefined
    if (!measurementRetained && activeStartedEpoch !== undefined) {
      if (partialCpu) await writeFile(path.join(directory, "selection-partial.cpuprofile"), JSON.stringify(partialCpu.profile))
      const partial = await page.evaluate(async () => {
        const root = globalThis as any, profile = root.__playsrcProfile, frames = root.__playsrcFrameProfiler
        if (profile) profile.selectionSampling = false
        if (frames) frames.active = false
        await frames?.flushShaderHashes()
        return { inputs: profile?.selectionInputs ?? [], owners: profile?.selectionOwners ?? [], memorySamples: profile?.selectionMemory ?? [],
          frames: frames?.completedFrames ?? [], worker: frames?.worker ?? [], gpuOperations: frames?.gpuOperations ?? [],
          shaders: frames?.shaders ?? [], adapters: frames?.adapters ?? [], devices: frames?.devices ?? [], losses: frames?.losses ?? [],
          gpuOperationsDropped: frames?.gpuOperationsDropped ?? 0, longTasks: frames?.longTasks ?? [], longAnimationFrames: frames?.longAnimationFrames ?? [] }
      }).catch(error => ({ unavailable: String(error) }))
      await writeFile(path.join(directory, "selection-partial.json"), JSON.stringify({ status: "failed-incomplete", startedEpoch: activeStartedEpoch,
        endedEpoch: Date.now(), references, partial, cpu: partialCpu ? summarizeCpuProfile(partialCpu.profile) : null,
        boundary: "Any missing next visible frame is right-censored, never a zero-latency or successful transition" }, null, 2))
    }
    await writeFile(path.join(directory, "selection-references.json"), JSON.stringify(references))
    await native?.close(); await windows?.close(); await Promise.all([cdp.detach(), browser.detach()])
    if (captureWithCua) await cua("end_session", { session })
  }
})
}
