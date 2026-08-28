import { spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { loadLocalConfig } from "../src/config"

type BrowserProcess = Readonly<{ type: string; id: number; cpuTime: number }>
type RpcRecord = { kind: string; stage: string; started: number; finished?: number; sentBytes: number; receivedBytes: number; transferredBytes: number; views?: number; sharedDispatch?: boolean; timings?: Record<string, number> }
type FrameRecord = { at: number; interval: number; displayFrame: number; tick: number; surfaces: number; props: number; skyProps: number; drawCalls: number; bufferCreations: number; uploadBytes: number; submissions: number; heapBytes: number | null; detail: Record<string, number> }

function intervalUnion(intervals: readonly (readonly [number, number])[]): number {
  let total = 0
  let started = Number.NaN
  let finished = Number.NaN
  for (const [nextStarted, nextFinished] of [...intervals].sort((left, right) => left[0] - right[0])) {
    if (!(nextFinished > nextStarted)) continue
    if (!Number.isFinite(started)) { started = nextStarted; finished = nextFinished; continue }
    if (nextStarted <= finished) finished = Math.max(finished, nextFinished)
    else { total += finished - started; started = nextStarted; finished = nextFinished }
  }
  return total + (Number.isFinite(started) ? finished - started : 0)
}

function processMemory(processes: readonly BrowserProcess[]) {
  const ids = [...new Set(processes.map((process) => process.id).filter((id) => Number.isSafeInteger(id) && id > 0))]
  if (ids.length === 0) return { processes: [], roles: {}, residentBytes: 0 }
  const command = process.platform === "win32"
    ? spawnSync("powershell", ["-NoProfile", "-Command", `Get-Process -Id ${ids.join(",")} -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64 | ConvertTo-Json -Compress`], { encoding: "utf8", timeout: 10_000, windowsHide: true })
    : spawnSync("ps", ["-o", "pid=,rss=", "-p", ids.join(",")], { encoding: "utf8", timeout: 10_000 })
  if (command.status !== 0) return { processes: [], roles: {}, residentBytes: 0, error: command.stderr.trim() }
  const values = process.platform === "win32"
    ? [JSON.parse(command.stdout)].flat().map((entry: any) => ({ id: entry.Id, residentBytes: entry.WorkingSet64, privateBytes: entry.PrivateMemorySize64 }))
    : command.stdout.trim().split("\n").filter(Boolean).map((line) => {
      const [id, rss] = line.trim().split(/\s+/u).map(Number)
      return { id: id!, residentBytes: rss! * 1024, privateBytes: null }
    })
  const roles = new Map(processes.map((entry) => [entry.id, entry.type]))
  const records = values.map((entry) => ({ ...entry, role: roles.get(entry.id) ?? "unknown" }))
  return {
    processes: records,
    roles: Object.fromEntries([...new Set(records.map((entry) => entry.role))].sort().map((role) => [role, {
      count: records.filter((entry) => entry.role === role).length,
      residentBytes: records.filter((entry) => entry.role === role).reduce((total, entry) => total + entry.residentBytes, 0),
    }])),
    residentBytes: records.reduce((total, entry) => total + entry.residentBytes, 0),
  }
}

test("profiles real headed 2Fort load, Soldier spawn, bots, outdoor visible frames and residency", async ({ page, context, browser }, testInfo) => {
  test.setTimeout(150_000)
  const wallStarted = Date.now()
  const seconds = profileSampleSeconds()
  const fullRoster = process.env.PROFILE_2FORT_FULL_ROSTER === "1"
  const label = process.env.PROFILE_2FORT_LABEL ?? "latest"
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "ctf-2fort-performance")
  await mkdir(directory, { recursive: true })
  const checkpoint = async (stage: string, values: Record<string, unknown> = {}) => {
    console.log(`PLAYSRC_2FORT_STAGE ${JSON.stringify({ label, stage, ...values })}`)
    await writeFile(path.join(directory, `${label}-progress.json`), `${JSON.stringify({ stage, ...values }, null, 2)}\n`)
  }
  await page.addInitScript(() => {
    const state = {
      stage: "startup",
      rpcs: [] as RpcRecord[],
      frames: [] as FrameRecord[],
      completedDisplays: [] as number[],
      longTasks: [] as { at: number; duration: number; stage: string }[],
      gpu: { buffers: 0, bufferBytes: 0, textures: 0, textureBytes: 0, residentTextureBytes: 0, compressedTextureBytes: 0, compressedTextures: 0, textureMipLevels: 0, pipelines: 0, submissions: 0, commandBuffers: 0, drawCalls: 0, writes: 0, writeBytes: 0 },
      transfer: { sentBytes: 0, receivedBytes: 0, transferredBytes: 0, sharedBytes: 0, receivedSharedBytes: 0, structuredCloneBytes: 0, messages: 0 },
    }
    ;(window as any).__playsrcProfile = state
    const payloadBytes = (value: any, depth = 0, visited = new Set<object>()): number => {
      if (!value || typeof value !== "object" || depth > 3 || visited.has(value)) return 0
      visited.add(value)
      if (ArrayBuffer.isView(value)) return value.byteLength
      if (value instanceof ArrayBuffer || typeof SharedArrayBuffer !== "undefined" && value instanceof SharedArrayBuffer) return value.byteLength
      return Object.values(value).reduce((total: number, item) => total + payloadBytes(item, depth + 1, visited), 0)
    }
    const sharedPayloadBytes = (value: any, depth = 0, visited = new Set<object>()): number => {
      if (!value || typeof value !== "object" || depth > 3 || visited.has(value)) return 0
      visited.add(value)
      if (value instanceof SharedArrayBuffer) return value.byteLength
      if (ArrayBuffer.isView(value)) return value.buffer instanceof SharedArrayBuffer ? value.byteLength : 0
      if (value instanceof ArrayBuffer) return 0
      return Object.values(value).reduce((total: number, item) => total + sharedPayloadBytes(item, depth + 1, visited), 0)
    }
    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, RpcRecord>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        const previous = (this as any).__playsrcProfileReply?.bind(this)
        ;(this as any).__playsrcProfileReply = (response: any) => {
          previous?.(response)
          const event = { data: response }
          const sharedSection = event.data?.kind === "resources" && event.data.bytes instanceof SharedArrayBuffer
            && Number.isSafeInteger(event.data.byteLength)
          const bytes = sharedSection ? event.data.byteLength : payloadBytes(event.data)
          state.transfer.receivedBytes += bytes
          state.transfer.receivedSharedBytes += sharedSection ? event.data.byteLength : sharedPayloadBytes(event.data)
          state.transfer.messages += 1
          const record = this.records.get(event.data?.id)
          if (!record) return
          record.finished = performance.now()
          record.receivedBytes = bytes
          if (event.data.timings) record.timings = event.data.timings
          this.records.delete(event.data.id)
        }
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        const sentBytes = payloadBytes(message)
        const transfers = Array.isArray(transferOrOptions) ? transferOrOptions : transferOrOptions?.transfer ?? []
        const transferredBytes = transfers.reduce((total, value) => total + (value instanceof ArrayBuffer ? value.byteLength : 0), 0)
        const sharedBytes = sharedPayloadBytes(message)
        state.transfer.sentBytes += sentBytes
        state.transfer.transferredBytes += transferredBytes
        state.transfer.sharedBytes += sharedBytes
        state.transfer.structuredCloneBytes += Math.max(0, sentBytes - transferredBytes - sharedBytes)
        state.transfer.messages += 1
        if (Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const started = performance.now()
          const views = Array.isArray(message.views) ? message.views.length : undefined
          const record: RpcRecord = { kind: message.kind, stage: state.stage, started, sentBytes, receivedBytes: 0, transferredBytes, ...(views === undefined ? {} : { views }) }
          this.records.set(message.id, record)
          state.rpcs.push(record)
          if (message.kind === "models" && Number.isSafeInteger(message.visibility?.id)) {
            const companionViews = Array.isArray(message.visibility.views) ? message.visibility.views.length : 0
            const companion: RpcRecord = { kind: "visibility", stage: state.stage, started, sentBytes: companionViews * 56, receivedBytes: 0, transferredBytes: 0, views: companionViews, sharedDispatch: true }
            this.records.set(message.visibility.id, companion)
            state.rpcs.push(companion)
          }
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    const intercept = (owner: any, method: string, before: (...arguments_: any[]) => void) => {
      const original = owner?.prototype?.[method]
      if (typeof original !== "function") return
      Object.defineProperty(owner.prototype, method, {
        configurable: true,
        writable: true,
        value(this: unknown, ...arguments_: any[]) {
          before(...arguments_)
          return original.apply(this, arguments_)
        },
      })
    }
    intercept((globalThis as any).GPUDevice, "createBuffer", (descriptor: any) => { state.gpu.buffers += 1; state.gpu.bufferBytes += descriptor?.size ?? 0 })
    intercept((globalThis as any).GPUDevice, "createTexture", (descriptor: any) => {
      state.gpu.textures += 1
      const size = descriptor?.size
      let width = Array.isArray(size) ? size[0] : size?.width ?? 1
      let height = Array.isArray(size) ? size[1] : size?.height ?? 1
      const layers = Array.isArray(size) ? size[2] ?? 1 : size?.depthOrArrayLayers ?? 1
      const levels = descriptor?.mipLevelCount ?? 1
      const format = String(descriptor?.format ?? "")
      const bytesPerPixel = format.includes("rgba32") ? 16 : format.includes("rgba16") ? 8 : format.includes("rg32") ? 8 : format.includes("r8") ? 1 : 4
      const blockBytes = /^bc1-/u.test(format) ? 8 : /^bc[2-7]-/u.test(format) ? 16 : 0
      if (blockBytes) state.gpu.compressedTextures += 1
      state.gpu.textureMipLevels += levels
      for (let level = 0; level < levels; level += 1) {
        const uncompressed = Math.max(1, width) * Math.max(1, height) * layers * bytesPerPixel
        const resident = blockBytes ? Math.max(1, Math.ceil(width / 4)) * Math.max(1, Math.ceil(height / 4)) * layers * blockBytes : uncompressed
        state.gpu.textureBytes += uncompressed
        state.gpu.residentTextureBytes += resident
        if (blockBytes) state.gpu.compressedTextureBytes += resident
        width = Math.floor(width / 2)
        height = Math.floor(height / 2)
      }
    })
    intercept((globalThis as any).GPUDevice, "createRenderPipeline", () => { state.gpu.pipelines += 1 })
    intercept((globalThis as any).GPUDevice, "createRenderPipelineAsync", () => { state.gpu.pipelines += 1 })
    intercept((globalThis as any).GPUQueue, "submit", (buffers: any[]) => { state.gpu.submissions += buffers?.length ?? 0 })
    intercept((globalThis as any).GPUQueue, "writeBuffer", (_target: unknown, _offset: unknown, data: any, dataOffset?: number, size?: number) => {
      state.gpu.writes += 1
      const elementBytes = ArrayBuffer.isView(data) && !(data instanceof DataView) ? data.BYTES_PER_ELEMENT : 1
      state.gpu.writeBytes += size === undefined
        ? Math.max(0, (data?.byteLength ?? 0) - (dataOffset ?? 0) * elementBytes)
        : size * elementBytes
    })
    intercept((globalThis as any).GPUQueue, "writeTexture", (_target: unknown, data: any) => { state.gpu.writes += 1; state.gpu.writeBytes += data?.byteLength ?? 0 })
    intercept((globalThis as any).GPUCommandEncoder, "finish", () => { state.gpu.commandBuffers += 1 })
    intercept((globalThis as any).GPURenderPassEncoder, "draw", () => { state.gpu.drawCalls += 1 })
    intercept((globalThis as any).GPURenderPassEncoder, "drawIndexed", () => { state.gpu.drawCalls += 1 })
    try {
      new PerformanceObserver((entries) => {
        for (const entry of entries.getEntries()) state.longTasks.push({ at: entry.startTime, duration: entry.duration, stage: state.stage })
      }).observe({ entryTypes: ["longtask"] })
    } catch {}

    let lastDisplayFrame = -1
    let lastDisplayedAt = 0
    let lastDrawCalls = 0
    let lastBuffers = 0
    let lastUploadBytes = 0
    let lastSubmissions = 0
    const sample = (now: number): void => {
      if (state.stage === "outdoor") {
        const root = document.querySelector<HTMLElement>("main")
        const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
        const displayFrame = Number(canvas?.dataset.displayFrame ?? -1)
        if (displayFrame !== lastDisplayFrame) {
          const sky = canvas?.dataset.sky3dPass ? JSON.parse(canvas.dataset.sky3dPass) : null
          const detail = root?.dataset.performanceDetail ? JSON.parse(root.dataset.performanceDetail) : {}
          if (lastDisplayedAt > 0) state.frames.push({
            at: now,
            interval: now - lastDisplayedAt,
            displayFrame,
            tick: Number(root?.dataset.snapshotTick ?? 0),
            surfaces: (window as any).__playsrcProfile.displacementVisibility?.drawSurfaces?.length ?? 0,
            props: canvas?.dataset.visibleMainStaticProps ? JSON.parse(canvas.dataset.visibleMainStaticProps).length : 0,
            skyProps: sky?.skyProps ?? 0,
            drawCalls: state.gpu.drawCalls - lastDrawCalls,
            bufferCreations: state.gpu.buffers - lastBuffers,
            uploadBytes: state.gpu.writeBytes - lastUploadBytes,
            submissions: state.gpu.submissions - lastSubmissions,
            heapBytes: (performance as any).memory?.usedJSHeapSize ?? null,
            detail,
          })
          lastDisplayedAt = now
          lastDisplayFrame = displayFrame
          lastDrawCalls = state.gpu.drawCalls
          lastBuffers = state.gpu.buffers
          lastUploadBytes = state.gpu.writeBytes
          lastSubmissions = state.gpu.submissions
        }
      } else {
        lastDisplayedAt = 0
        lastDisplayFrame = -1
      }
      requestAnimationFrame(sample)
    }
    requestAnimationFrame(sample)
  })

  const browserCdp = await browser.newBrowserCDPSession()
  const pageCdp = await context.newCDPSession(page)
  await pageCdp.send("Performance.enable")
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
  await checkpoint("main-menu")
  const configuration = await (await page.request.get("/playsrc-config.json")).json() as { assetOrigin: string; targets: { target: string; objects: { bsp: { sha256: string; byteLength: string } } }[] }
  const target = configuration.targets.find((entry) => entry.target === "ctf_2fort")
  if (!target) throw new Error("configured ctf_2fort target is unavailable")
  const bspUrl = `${configuration.assetOrigin}/objects/sha256/${target.objects.bsp.sha256}`
  const bsp = await (await page.request.get(bspUrl)).body()
  expect(bsp.byteLength).toBe(Number(target.objects.bsp.byteLength))
  let memoryBspRequests = 0
  await page.route(bspUrl, async (route) => {
    memoryBspRequests += 1
    await route.fulfill({ status: 200, body: bsp, headers: { "access-control-allow-origin": "*", "cache-control": "no-store", "content-length": String(bsp.byteLength), "content-type": "application/octet-stream", etag: `"${target.objects.bsp.sha256}"` } })
  })
  const processBefore = processMemory((await browserCdp.send("SystemInfo.getProcessInfo") as { processInfo: BrowserProcess[] }).processInfo)
  const heapBefore = await pageCdp.send("Runtime.getHeapUsage")

  const entry = page.locator("[aria-label='Console command']")
  let started: number
  if (fullRoster) {
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
    const dialog = page.locator(".local-match-layer").getByRole("dialog", { name: "CREATE SERVER" })
    await dialog.locator("[data-vgui-name='MapList']").click()
    await page.getByRole("option", { name: "ctf_2fort" }).click()
    await dialog.getByRole("tab", { name: "GAME" }).click()
    await dialog.locator("[data-vgui-name='GameplayPage'] [data-vgui-name='NumPlayersTextEntry']").fill("23")
    started = await page.evaluate(() => { performance.clearResourceTimings(); (window as any).__playsrcProfile.stage = "load"; return performance.now() })
    await dialog.getByRole("button", { name: "Start" }).click()
  } else {
    await page.keyboard.press("Backquote")
    await entry.fill("map ctf_2fort")
    started = await page.evaluate(() => { performance.clearResourceTimings(); (window as any).__playsrcProfile.stage = "load"; return performance.now() })
    await entry.press("Enter")
  }
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true" || main?.dataset.phase === "Ready" || main?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000, polling: 50 })
  const teamMenuAt = await page.evaluate(() => performance.now())
  await checkpoint("team-selection", { milliseconds: teamMenuAt - started })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
    await page.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      return main?.dataset.classSelectionVisible === "true" || main?.dataset.phase === "Failed"
    }, undefined, { timeout: 90_000 })
  }
  const soldierStarted = await page.evaluate(() => performance.now())
  if (await root.getAttribute("data-class-selection-visible") === "true") await page.keyboard.press("Digit2")
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 90_000 })
  await expect(root).toHaveAttribute("data-class-selection-visible", "false", { timeout: 90_000 })
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1], { timeout: 90_000 }).toBe("3")
  const playableAt = await page.evaluate(() => { (window as any).__playsrcProfile.stage = "setup"; return performance.now() })
  const downloads = await page.evaluate(({ started, finished, bsp }) => performance.getEntriesByType("resource")
    .map((entry) => entry as PerformanceResourceTiming)
    .filter((entry) => entry.name !== bsp && entry.transferSize > 0 && entry.responseEnd > started && entry.startTime < finished)
    .map((entry) => ({ url: entry.name, started: Math.max(started, entry.startTime), finished: Math.min(finished, entry.responseEnd), bytes: entry.transferSize })), { started, finished: playableAt, bsp: bspUrl })
  const networkDownloadMilliseconds = intervalUnion(downloads.map((download) => [download.started, download.finished] as const))
  const loadedMemory = processMemory((await browserCdp.send("SystemInfo.getProcessInfo") as { processInfo: BrowserProcess[] }).processInfo)
  const heapLoaded = await pageCdp.send("Runtime.getHeapUsage")
  await checkpoint("soldier-playable", { milliseconds: playableAt - started, residentBytes: loadedMemory.residentBytes, heapBytes: heapLoaded.usedSize, phases: JSON.parse(await root.getAttribute("data-load-performance") ?? "null") })

  if (fullRoster) {
    await expect(root).toHaveAttribute("data-bot-count", "23", { timeout: 45_000 })
    await page.waitForFunction(() => (window as any).__playsrcProfile.bots?.length === 23, undefined, { timeout: 30_000 })
    const scoreboard = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
    expect(scoreboard.red.playerCount).toBe(12)
    expect(scoreboard.blue.playerCount).toBe(12)
    await checkpoint("bots", { count: 23, red: scoreboard.red.playerCount, blue: scoreboard.blue.playerCount })
  }
  if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  if (!fullRoster) {
    await command("tf_bot_add red soldier normal")
    await expect(root).toHaveAttribute("data-bot-count", "1", { timeout: 30_000 })
    await page.waitForFunction(() => (window as any).__playsrcProfile.bots?.length === 1, undefined, { timeout: 30_000 })
    await command("tf_bot_add blue scout normal")
    await expect(root).toHaveAttribute("data-bot-count", "2", { timeout: 30_000 })
    await page.waitForFunction(() => (window as any).__playsrcProfile.bots?.length === 2, undefined, { timeout: 30_000 })
    await checkpoint("bots", { count: 2 })
  }
  await command("noclip")
  await command("setpos 523 -439 250")
  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "false")
  const canvas = page.locator("canvas.world-canvas")
  await expect(canvas).toBeVisible()
  await page.evaluate(() => {
    ;(window as any).__playsrcProfile.displacementCameraOverride = {
      position: [523, -439, 318],
      yawDegrees: 90,
      pitchDegrees: -8,
    }
  })
  await page.waitForFunction(() => document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayCameraPosition === "523,-439,318", undefined, { timeout: 30_000 })
  await page.waitForFunction(() => Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0) > 0, undefined, { timeout: 30_000 })
  await checkpoint("outdoor-positioned")
  const beforePixels = await canvas.screenshot({ timeout: 15_000 })
  const props = await canvas.evaluate((element) => JSON.parse(element.dataset.staticProps ?? "{}"))

  await pageCdp.send("Profiler.enable")
  await pageCdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  await pageCdp.send("HeapProfiler.startSampling", { samplingInterval: 32_768 })
  await pageCdp.send("Profiler.start")
  await checkpoint("sampling")
  const route = await page.evaluate(async (duration) => {
    const main = document.querySelector<HTMLElement>("main")!
    const state = (window as any).__playsrcProfile
    const firstTick = Number(main.dataset.snapshotTick)
    const firstGpu = { ...state.gpu }
    const firstTransfer = { ...state.transfer }
    const firstUploads = { ...state.modelParticleUploads }
    const firstDisplay = Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0)
    const began = performance.now()
    state.stage = "outdoor"
    try {
      while (performance.now() - began < duration * 1000) {
        if (main.dataset.phase !== "Ready") throw new Error(`2Fort left Ready: ${main.dataset.phase} ${main.dataset.detail}`)
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
      }
    } finally {
      state.stage = "finished"
    }
    return {
      elapsedMilliseconds: performance.now() - began,
      firstTick,
      finalTick: Number(main.dataset.snapshotTick),
      firstDisplay,
      finalDisplay: Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0),
      firstGpu,
      firstTransfer,
      firstUploads,
      bots: (window as any).__playsrcProfile.bots ?? [],
    }
  }, seconds)
  await checkpoint("sampled", { milliseconds: route.elapsedMilliseconds, displayFrames: route.finalDisplay - route.firstDisplay })
  const cpu = (await pageCdp.send("Profiler.stop") as { profile: CpuProfile }).profile
  const sampled = (await pageCdp.send("HeapProfiler.stopSampling") as { profile: { head: { callFrame: { functionName: string; url: string }; selfSize: number; children: any[] } } }).profile
  const heapAfter = await pageCdp.send("Runtime.getHeapUsage")
  const afterProcesses = processMemory((await browserCdp.send("SystemInfo.getProcessInfo") as { processInfo: BrowserProcess[] }).processInfo)
  const afterPixels = await canvas.screenshot({ timeout: 15_000 })
  const outdoorImage = decodeScreenshot(afterPixels)
  let outdoorSkyPixels = 0
  let outdoorVisiblePixels = 0
  for (let index = 0; index < outdoorImage.pixels.length; index += outdoorImage.channels) {
    const red = outdoorImage.pixels[index]!
    const green = outdoorImage.pixels[index + 1]!
    const blue = outdoorImage.pixels[index + 2]!
    if (red > 8 || green > 8 || blue > 8) outdoorVisiblePixels += 1
    if (Math.abs(red - 94) <= 2 && Math.abs(green - 104) <= 2 && Math.abs(blue - 140) <= 2) outdoorSkyPixels += 1
  }
  const workers = await Promise.all(page.workers().map(async (worker) => ({
    url: worker.url(),
    ...await Promise.race([
      worker.evaluate(() => ({ heapBytes: (performance as any).memory?.usedJSHeapSize ?? null, memory: (globalThis as any).__playsrcWorkerMemory ?? null })).catch(() => ({ heapBytes: null, memory: null })),
      new Promise<{ heapBytes: null; memory: null }>((resolve) => setTimeout(() => resolve({ heapBytes: null, memory: null }), 1_000)),
    ]),
  })))
  const state = await page.evaluate(() => {
    const profile = (window as any).__playsrcProfile
    const main = document.querySelector<HTMLElement>("main")!
    return { frames: profile.frames, completedDisplays: profile.completedDisplays, rpcs: profile.rpcs, longTasks: profile.longTasks, gpu: profile.gpu, transfer: profile.transfer, modelParticleUploads: profile.modelParticleUploads, load: JSON.parse(main.dataset.loadPerformance ?? "null"), hud: JSON.parse(main.dataset.hudPresentationProbe ?? "null"), panels: document.querySelectorAll("[data-vgui-name]").length }
  }) as { frames: FrameRecord[]; completedDisplays: number[]; rpcs: RpcRecord[]; longTasks: { at: number; duration: number; stage: string }[]; gpu: Record<string, number>; transfer: Record<string, number>; modelParticleUploads: Record<string, number>; load: unknown; hud: any; panels: number }

  const allocationRows: { function: string; url: string; bytes: number }[] = []
  const visit = (node: { callFrame: { functionName: string; url: string }; selfSize: number; children: any[] }) => {
    if (node.selfSize > 0) allocationRows.push({ function: node.callFrame.functionName || "(anonymous)", url: node.callFrame.url, bytes: node.selfSize })
    for (const child of node.children) visit(child)
  }
  visit(sampled.head)
  allocationRows.sort((left, right) => right.bytes - left.bytes)
  const summarizeWorkerCalls = (entries: readonly RpcRecord[]) => Object.fromEntries([...new Set(entries.map((record) => record.kind))].sort().map((kind) => {
    const records = entries.filter((record) => record.kind === kind)
    return [kind, { calls: records.length, views: records.reduce((total, record) => total + (record.views ?? 0), 0), sharedDispatches: records.filter(record => record.sharedDispatch).length, sentBytes: records.reduce((total, record) => total + record.sentBytes, 0), receivedBytes: records.reduce((total, record) => total + record.receivedBytes, 0), transferredBytes: records.reduce((total, record) => total + record.transferredBytes, 0), latency: summarizeDistribution(records.flatMap((record) => record.finished === undefined ? [] : [record.finished - record.started])), timings: Object.fromEntries([...new Set(records.flatMap(record => Object.keys(record.timings ?? {})))].map(key => [key, summarizeDistribution(records.flatMap(record => typeof record.timings?.[key] === "number" ? [record.timings[key]!] : []))])) }]
  }))
  const workerCalls = summarizeWorkerCalls(state.rpcs)
  const workerCallsByStage = Object.fromEntries([...new Set(state.rpcs.map(record => record.stage))].sort()
    .map(stage => [stage, summarizeWorkerCalls(state.rpcs.filter(record => record.stage === stage))]))
  const gpuOutdoor = Object.fromEntries(Object.entries(state.gpu).map(([name, value]) => [name, value - ((route.firstGpu as Record<string, number>)[name] ?? 0)]))
  const transferOutdoor = Object.fromEntries(Object.entries(state.transfer).map(([name, value]) => [name, value - ((route.firstTransfer as Record<string, number>)[name] ?? 0)]))
  const uploadOutdoor = Object.fromEntries(Object.entries(state.modelParticleUploads ?? {}).map(([name, value]) => [name,
    name === "retainedParticleBatches" ? value : value - ((route.firstUploads as Record<string, number>)[name] ?? 0),
  ]))
  const ticksPerSecond = (route.finalTick - route.firstTick) / route.elapsedMilliseconds * 1000
  const report = {
    schema: "playsrc-tf2-2fort-performance-v1",
    headed: true,
    target: "ctf_2fort",
    sampleSeconds: seconds,
    totalWallMilliseconds: Date.now() - wallStarted,
    loading: { bspBytes: bsp.byteLength, prefetchedBspRequests: memoryBspRequests, networkDownloadMilliseconds, initializationExcludingDownloadMilliseconds: playableAt - started - networkDownloadMilliseconds, downloadedBytes: downloads.reduce((total, download) => total + download.bytes, 0), mapToTeamMenuMilliseconds: teamMenuAt - started, soldierSelectionMilliseconds: playableAt - soldierStarted, mapToSoldierPlayableMilliseconds: playableAt - started, phases: state.load },
    simulation: { ticksPerSecond, firstTick: route.firstTick, finalTick: route.finalTick, bots: route.bots },
    actualVisibleFrames: {
      ...summarizeFrameTimes(state.completedDisplays.slice(1).map((at, index) => at - state.completedDisplays[index]!)),
      firstDisplay: route.firstDisplay, finalDisplay: route.finalDisplay,
      sampledAnimationFrames: state.frames.length, completedPresentations: state.completedDisplays.length,
    },
    visibility: { surfaces: summarizeDistribution(state.frames.map((frame) => frame.surfaces)), props: summarizeDistribution(state.frames.map((frame) => frame.props)), skyProps: summarizeDistribution(state.frames.map((frame) => frame.skyProps)), totalStaticProps: props.total, configuration: props },
    drawCalls: summarizeDistribution(state.frames.map((frame) => frame.drawCalls)),
    timings: Object.fromEntries(["models", "bots", "visibility", "world", "viewmodel", "render", "total"].map((name) => [name, summarizeDistribution(state.frames.flatMap((frame) => Number.isFinite(frame.detail[name]) ? [frame.detail[name]!] : []))])),
    browserResident: { before: processBefore, loaded: loadedMemory, outdoor: afterProcesses },
    javascript: { before: heapBefore, loaded: heapLoaded, outdoor: heapAfter, frameSamples: summarizeDistribution(state.frames.flatMap((frame) => frame.heapBytes === null ? [] : [frame.heapBytes])), workers },
    gpu: {
      total: state.gpu,
      outdoor: gpuOutdoor,
      perVisibleFrame: {
        uploadBytes: summarizeDistribution(state.frames.map((frame) => frame.uploadBytes)),
        bufferCreations: summarizeDistribution(state.frames.map((frame) => frame.bufferCreations)),
        submissions: summarizeDistribution(state.frames.map((frame) => frame.submissions)),
      },
      modelParticleUploads: uploadOutdoor,
    },
    worker: { calls: workerCalls, stages: workerCallsByStage, transfer: { total: state.transfer, outdoor: transferOutdoor } },
    allocations: { sampledBytes: allocationRows.reduce((total, entry) => total + entry.bytes, 0), perVisibleFrameBytes: allocationRows.reduce((total, entry) => total + entry.bytes, 0) / Math.max(1, state.frames.length), top: allocationRows.slice(0, 25) },
    longTasks: summarizeDistribution(state.longTasks.filter((task) => task.stage === "outdoor").map((task) => task.duration)),
    panels: { count: state.panels, classModel: state.hud?.classModel ?? null },
    cpu: summarizeCpuProfile(cpu),
    pixels: { beforeSha256: createHash("sha256").update(beforePixels).digest("hex"), afterSha256: createHash("sha256").update(afterPixels).digest("hex"), visible: outdoorVisiblePixels, authoredSky: outdoorSkyPixels },
  }
  await Promise.all([
    writeFile(path.join(directory, `${label}.json`), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, `${label}.cpuprofile`), JSON.stringify(cpu)),
    writeFile(path.join(directory, `${label}.png`), afterPixels),
  ])
  await testInfo.attach("headed-2fort-performance", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(`PLAYSRC_2FORT_PERFORMANCE ${JSON.stringify({ label, loadMilliseconds: report.loading.mapToSoldierPlayableMilliseconds, initializationMilliseconds: report.loading.initializationExcludingDownloadMilliseconds, soldierMilliseconds: report.loading.soldierSelectionMilliseconds, visibleFrames: report.actualVisibleFrames, ticksPerSecond, residentMiB: Math.round(afterProcesses.residentBytes / 1048576), jsMiB: Math.round(heapAfter.usedSize / 1048576), textureMiB: Math.round(state.gpu.residentTextureBytes / 1048576), wasm: workers, staticProps: props.total, authoredSkyPixels: outdoorSkyPixels, drawCalls: report.drawCalls, outdoorGpu: gpuOutdoor, perVisibleFrame: report.gpu.perVisibleFrame, modelParticleUploads: uploadOutdoor, topCpu: report.cpu.topSelf.slice(0, 8) })}`)

  expect(memoryBspRequests).toBe(1)
  expect(props.total).toBe(2265)
  expect(route.bots).toHaveLength(fullRoster ? 23 : 2)
  expect(outdoorVisiblePixels).toBeGreaterThan(20_000)
  expect(outdoorSkyPixels).toBeGreaterThan(5_000)
  expect(ticksPerSecond).toBeGreaterThan(55)
  expect(report.actualVisibleFrames.frames).toBeGreaterThan(seconds * 20)
  expect(report.actualVisibleFrames.p95Milliseconds).toBeLessThan(25)
  expect(report.loading.initializationExcludingDownloadMilliseconds).toBeLessThan(12_000)
  expect(report.browserResident.loaded.residentBytes).toBeLessThan(6 * 1024 ** 3)
  expect(workers.find((worker) => worker.memory)?.memory?.linearBytes).toBeLessThan(2 * 1024 ** 3)
  await expect(root).toHaveAttribute("data-phase", "Ready")
})
