import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { sanityViewAngles, selectSanityCheckpoints, type SanityCheckpoint, type SanityLandmarks } from "./map-sanity"
import { divideProfileWindow, profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

const TARGETS = ["jump_beef", "ctf_2fort", "pl_upward"] as const

type WorkerRecord = { kind: string; started: number; finished?: number; bytes: number; timings?: Record<string, number> }
type FrameRecord = {
  interval: number
  displayFrame: number
  detail: Record<string, number>
  pvsSurfaces: number
  drawSurfaces: number
  leaves: number
  staticProps: number
  skySurfaces: number
  skyProps: number
}
type GpuCounters = { submissions: number; commandBuffers: number; buffers: number; textures: number; uploadBytes: number }
type ProfileState = {
  active: boolean
  workers: WorkerRecord[]
  longTasks: number[]
  gpu: GpuCounters
  coverageSamples?: SanityLandmarks["samples"]
  materialAnimation?: { volumes?: SanityLandmarks["water"]; skyController?: { area: number } }
  objectives?: { position: SanityLandmarks["spawn"] }[]
  pickups?: { origin: SanityLandmarks["spawn"] }[]
  bots?: { shots: number; hits: number }[]
  displacementVisibility?: { surfaces: number[]; drawSurfaces: number[]; leaves: number[] }
}

async function command(page: Page, entry: Locator, value: string): Promise<void> {
  if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await expect(entry).toBeVisible()
  await entry.fill(value)
  await entry.press("Enter")
}

function relevantCheckpoints(target: string, values: readonly SanityCheckpoint[]): readonly SanityCheckpoint[] {
  const selected = target === "ctf_2fort"
    ? ["spawn", "bridge", "objective"]
    : target === "pl_upward" ? ["spawn", "outdoor-terrain"] : ["spawn", "water"]
  return selected.map((kind) => {
    const checkpoint = values.find((value) => value.kind === kind)
    if (!checkpoint) throw new Error(`${target} has no authored ${kind} frame-budget viewpoint`)
    return checkpoint
  })
}

function summarizeWorkers(records: readonly WorkerRecord[]) {
  return Object.fromEntries([...new Set(records.map((record) => record.kind))].sort().map((kind) => {
    const values = records.filter((record) => record.kind === kind)
    const timingNames = [...new Set(values.flatMap((record) => Object.keys(record.timings ?? {})))].sort()
    return [kind, {
      calls: values.length,
      completed: values.filter((record) => record.finished !== undefined).length,
      bytes: values.reduce((total, record) => total + record.bytes, 0),
      milliseconds: summarizeDistribution(values.flatMap((record) => record.finished === undefined ? [] : [record.finished - record.started])),
      timings: Object.fromEntries(timingNames.map((name) => [name,
        summarizeDistribution(values.flatMap((record) => typeof record.timings?.[name] === "number" ? [record.timings[name]!] : [])),
      ])),
    }]
  }))
}

test("headed authored three-map Soldier, bots, water, HUD, combat, and full-prop frame budget", async ({ page, context, browser }, testInfo) => {
  const seconds = profileSampleSeconds()
  const mapWindows = divideProfileWindow(seconds, TARGETS.length)
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "all-maps-frame-budget")
  await mkdir(output, { recursive: true })

  await page.addInitScript(() => {
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", {
      configurable: true,
      value(this: Element) {
        locked = this
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    Object.defineProperty(document, "exitPointerLock", {
      configurable: true,
      value() {
        locked = null
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })

    const state: ProfileState = {
      active: false,
      workers: [],
      longTasks: [],
      gpu: { submissions: 0, commandBuffers: 0, buffers: 0, textures: 0, uploadBytes: 0 },
    }
    ;(globalThis as typeof globalThis & { __playsrcProfile?: ProfileState }).__playsrcProfile = state
    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, WorkerRecord>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const record = this.records.get(event.data?.id)
          if (!record) return
          record.finished = performance.now()
          if (event.data?.timings) record.timings = event.data.timings
          this.records.delete(event.data.id)
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (state.active && Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const record = {
            kind: message.kind,
            started: performance.now(),
            bytes: message.command?.byteLength ?? message.batch?.byteLength ?? 0,
          }
          this.records.set(message.id, record)
          state.workers.push(record)
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    const instrument = (owner: any, method: string, update: (...arguments_: any[]) => void) => {
      const original = owner?.prototype?.[method]
      if (typeof original !== "function") return
      Object.defineProperty(owner.prototype, method, {
        configurable: true,
        writable: true,
        value(this: unknown, ...arguments_: any[]) {
          if (state.active) update(...arguments_)
          return original.apply(this, arguments_)
        },
      })
    }
    instrument((globalThis as any).GPUQueue, "submit", (buffers: unknown[]) => { state.gpu.submissions += buffers.length })
    instrument((globalThis as any).GPUCommandEncoder, "finish", () => { state.gpu.commandBuffers += 1 })
    instrument((globalThis as any).GPUDevice, "createBuffer", () => { state.gpu.buffers += 1 })
    instrument((globalThis as any).GPUDevice, "createTexture", () => { state.gpu.textures += 1 })
    instrument((globalThis as any).GPUQueue, "writeBuffer", (_buffer: unknown, _offset: number, data: ArrayBufferView | ArrayBuffer, dataOffset?: number, size?: number) => {
      const bytesPerElement = ArrayBuffer.isView(data) && "BYTES_PER_ELEMENT" in data ? data.BYTES_PER_ELEMENT : 1
      state.gpu.uploadBytes += size === undefined ? data.byteLength - (dataOffset ?? 0) * bytesPerElement : size * bytesPerElement
    })
    instrument((globalThis as any).GPUQueue, "writeTexture", (_destination: unknown, data: ArrayBufferView | ArrayBuffer) => {
      state.gpu.uploadBytes += data.byteLength
    })
    try {
      new PerformanceObserver((list) => {
        if (state.active) state.longTasks.push(...list.getEntries().map((entry) => entry.duration))
      }).observe({ entryTypes: ["longtask"] })
    } catch {}
  })

  const cdp = await context.newCDPSession(page)
  const browserCdp = await browser.newBrowserCDPSession()
  const system = await browserCdp.send("SystemInfo.getInfo") as { gpu?: { devices?: unknown[]; featureStatus?: unknown } }
  await cdp.send("Performance.enable")
  await cdp.send("Profiler.enable")
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })

  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  const root = page.locator("main")
  const canvas = page.locator("canvas.world-canvas")
  const entry = page.locator("[aria-label='Console command']")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  const catalog = await (await page.request.get("/playsrc-config.json")).json() as { targets: { target: string }[] }
  expect(catalog.targets.map((value) => value.target).toSorted()).toEqual([...TARGETS].toSorted())

  const maps: Record<string, unknown>[] = []
  const intervals: number[] = []
  const work: number[] = []
  for (const [index, target] of TARGETS.entries()) {
    if (index > 0) {
      const origin = new URL(page.url()).origin
      await page.goto("about:blank", { waitUntil: "load", timeout: 30_000 })
      await cdp.send("Storage.clearDataForOrigin", { origin, storageTypes: "indexeddb" })
      await page.goto("/", { waitUntil: "load", timeout: 30_000 })
      await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
    }
    await command(page, entry, `map ${target}`)
    await page.waitForFunction((identity) => {
      const main = document.querySelector<HTMLElement>("main")
      return main?.dataset.phase === "Failed" || main?.dataset.teamSelectionVisible === "true"
        || main?.dataset.phase === "Ready" && main.dataset.detail === `Playing ${identity}`
    }, target, { timeout: 120_000, polling: 20 })
    if (await root.getAttribute("data-team-selection-visible") === "true") {
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await chooseTf2Team(page, target === "pl_upward" ? "blue" : "red")
    }
    await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 45_000 })
    await command(page, entry, "joinclass soldier")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("3")
    if (target !== "jump_beef") {
      await command(page, entry, "tf_bot_add red soldier normal")
      await expect(root).toHaveAttribute("data-bot-count", "1", { timeout: 20_000 })
      await command(page, entry, "tf_bot_add blue scout normal")
      await expect(root).toHaveAttribute("data-bot-count", "2", { timeout: 20_000 })
    }
    await command(page, entry, "noclip")
    await expect(root).toHaveAttribute("data-movement-mode", "1")
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.keyboard.press("Digit2")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("7")
    await page.waitForFunction(() => {
      const state = (globalThis as any).__playsrcProfile as ProfileState
      return (state.coverageSamples?.length ?? 0) > 0 && Array.isArray(state.objectives) && Array.isArray(state.pickups)
    }, undefined, { timeout: 20_000 })
    const landmarks = await page.evaluate((identity) => {
      const main = document.querySelector<HTMLElement>("main")!
      const state = (globalThis as any).__playsrcProfile as ProfileState
      const camera = (main.dataset.cameraPosition ?? "").split(",").map(Number)
      camera[2] = camera[2]! - Number((main.dataset.viewOffset ?? "0,0,68").split(",")[2])
      return {
        target: identity,
        spawn: camera,
        samples: state.coverageSamples,
        water: state.materialAnimation?.volumes ?? [],
        skyArea: state.materialAnimation?.skyController?.area ?? null,
        objectives: state.objectives?.map((objective) => objective.position) ?? [],
        pickups: state.pickups?.map((pickup) => pickup.origin) ?? [],
      }
    }, target) as SanityLandmarks
    const checkpoints = relevantCheckpoints(target, selectSanityCheckpoints(landmarks))
    const heapBefore = await cdp.send("Runtime.getHeapUsage")
    const metricsBefore = await cdp.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }
    await cdp.send("HeapProfiler.startSampling", { samplingInterval: 32_768 })
    await cdp.send("Profiler.start")

    const observations: Record<string, unknown>[] = []
    for (const checkpoint of checkpoints) {
      if (checkpoint.kind !== "spawn") {
        const offset = Number(((await root.getAttribute("data-view-offset")) ?? "0,0,68").split(",")[2])
        await command(page, entry, `setpos ${checkpoint.position[0]} ${checkpoint.position[1]} ${checkpoint.position[2] - offset}`)
        await page.waitForFunction((position) => {
          const main = document.querySelector<HTMLElement>("main")
          const current = (main?.dataset.cameraPosition ?? "").split(",").map(Number)
          return main?.dataset.phase === "Failed" || current.length === 3 && Math.hypot(current[0]! - position[0], current[1]! - position[1]) < 2
        }, checkpoint.position, { timeout: 12_000, polling: 10 })
      }
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await canvas.click()
      await expect(root).toHaveAttribute("data-pointer-locked", "true")
      const angles = sanityViewAngles(checkpoint.position, checkpoint.focus)
      await page.evaluate(({ yaw, pitch }) => {
        const main = document.querySelector<HTMLElement>("main")!
        const currentYaw = Number(main.dataset.cameraYaw ?? 0)
        const currentPitch = Number(main.dataset.cameraPitch ?? 0)
        const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
        const event = new MouseEvent("mousemove", { bubbles: true })
        Object.defineProperties(event, { movementX: { value: wrap(currentYaw - yaw) / 0.066 }, movementY: { value: (pitch - currentPitch) / 0.066 } })
        dispatchEvent(event)
      }, angles)
      await page.waitForFunction(() => {
        const state = (globalThis as any).__playsrcProfile as ProfileState
        return (state.displacementVisibility?.drawSurfaces.length ?? 0) > 0
      }, undefined, { timeout: 8_000 })

      const screenshot = await canvas.screenshot()
      const pixels = decodeScreenshot(screenshot)
      let authoredPixels = 0
      for (let pixel = 0; pixel < pixels.pixels.length; pixel += pixels.channels) {
        if (pixels.pixels[pixel]! > 8 || pixels.pixels[pixel + 1]! > 8 || pixels.pixels[pixel + 2]! > 8) authoredPixels += 1
      }
      expect(authoredPixels).toBeGreaterThan(20_000)
      await writeFile(path.join(output, `${target}-${checkpoint.kind}.png`), screenshot)

      const sample = await page.evaluate(async ({ milliseconds, fire }) => {
        const main = document.querySelector<HTMLElement>("main")!
        const surface = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
        const state = (globalThis as any).__playsrcProfile as ProfileState
        const firstTick = Number(main.dataset.snapshotTick)
        const firstDisplay = Number(surface.dataset.displayFrame)
        const firstFire = Number(main.dataset.fireEvents ?? 0)
        const firstWorker = state.workers.length
        const firstTask = state.longTasks.length
        const firstGpu = { ...state.gpu }
        const frames: FrameRecord[] = []
        const latencies: number[] = []
        let pendingInput: { revision: number; at: number } | undefined
        const observer = new MutationObserver(() => {
          if (pendingInput && Number(surface.dataset.displayMouseRevision) >= pendingInput.revision) {
            latencies.push(performance.now() - pendingInput.at)
            pendingInput = undefined
          }
        })
        observer.observe(surface, { attributes: true, attributeFilter: ["data-display-mouse-revision"] })
        const started = performance.now()
        let previous = started
        let nextInput = started + 80
        let attacked = false
        state.active = true
        try {
          await new Promise<void>((resolve, reject) => {
            const frame = (now: number): void => {
              if (main.dataset.phase !== "Ready") { reject(new Error(`gameplay entered ${main.dataset.phase}: ${main.dataset.detail}`)); return }
              if (now <= previous) { requestAnimationFrame(frame); return }
              const raw = main.dataset.performanceDetail
              const detail = raw ? JSON.parse(raw) as Record<string, number> : {}
              const sky = surface.dataset.sky3dPass ? JSON.parse(surface.dataset.sky3dPass) as { skySurfaces: number; skyProps: number } : undefined
              frames.push({
                interval: now - previous,
                displayFrame: Number(surface.dataset.displayFrame),
                detail,
                pvsSurfaces: state.displacementVisibility?.surfaces.length ?? 0,
                drawSurfaces: state.displacementVisibility?.drawSurfaces.length ?? 0,
                leaves: state.displacementVisibility?.leaves.length ?? 0,
                staticProps: surface.dataset.visibleMainStaticProps ? JSON.parse(surface.dataset.visibleMainStaticProps).length : 0,
                skySurfaces: sky?.skySurfaces ?? 0,
                skyProps: sky?.skyProps ?? 0,
              })
              previous = now
              if (now >= nextInput && !pendingInput) {
                const revision = Number(surface.dataset.displayMouseRevision ?? 0) + 1
                pendingInput = { revision, at: performance.now() }
                const event = new MouseEvent("mousemove", { bubbles: true })
                Object.defineProperties(event, { movementX: { value: 1 }, movementY: { value: 0 } })
                dispatchEvent(event)
                nextInput = now + 140
              }
              if (fire && !attacked && now - started >= milliseconds * 0.2) {
                attacked = true
                surface.dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
              }
              if (attacked && now - started >= milliseconds * 0.45) {
                dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
                attacked = false
                fire = false
              }
              if (now - started >= milliseconds) resolve()
              else requestAnimationFrame(frame)
            }
            requestAnimationFrame(frame)
          })
        } finally {
          if (attacked) dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
          state.active = false
          observer.disconnect()
        }
        const elapsed = performance.now() - started
        return {
          elapsed,
          firstTick,
          lastTick: Number(main.dataset.snapshotTick),
          firstDisplay,
          lastDisplay: Number(surface.dataset.displayFrame),
          fireEvents: Number(main.dataset.fireEvents ?? 0) - firstFire,
          bots: Number(main.dataset.botCount ?? 0),
          botShots: state.bots?.reduce((total, bot) => total + bot.shots, 0) ?? 0,
          botHits: state.bots?.reduce((total, bot) => total + bot.hits, 0) ?? 0,
          hud: main.dataset.hudProbe,
          water: main.dataset.waterPlan,
          particles: Number(main.dataset.particleItems ?? 0),
          frames,
          latencies,
          workers: state.workers.slice(firstWorker),
          longTasks: state.longTasks.slice(firstTask),
          gpu: Object.fromEntries(Object.entries(state.gpu).map(([key, value]) => [key, value - firstGpu[key as keyof GpuCounters]])) as GpuCounters,
          staticProps: JSON.parse(surface.dataset.staticProps ?? "{}") as Record<string, number>,
        }
      }, { milliseconds: mapWindows[index]! * 1_000 / checkpoints.length, fire: checkpoint.kind !== "spawn" })

      const unique = [...new Map(sample.frames.filter((frame) => frame.displayFrame > sample.firstDisplay && Number.isFinite(frame.detail.total))
        .map((frame) => [frame.displayFrame, frame])).values()]
      const frameIntervals = sample.frames.map((frame) => frame.interval)
      const frameWork = unique.map((frame) => frame.detail.total!)
      intervals.push(...frameIntervals)
      work.push(...frameWork)
      const simulationHz = (sample.lastTick - sample.firstTick) * 1_000 / sample.elapsed
      expect(simulationHz).toBeGreaterThan(53)
      expect(unique.length).toBeGreaterThan(10)
      expect(sample.gpu.submissions).toBeGreaterThan(0)
      expect(sample.hud?.split(":")[1]).toBe("3")
      if (target !== "jump_beef") expect(sample.bots).toBe(2)
      observations.push({
        viewpoint: checkpoint.kind,
        elapsedMilliseconds: Number(sample.elapsed.toFixed(3)),
        achievedFps: Number(((sample.lastDisplay - sample.firstDisplay) * 1_000 / sample.elapsed).toFixed(2)),
        browserRefreshHz: Number((sample.frames.length * 1_000 / sample.elapsed).toFixed(2)),
        simulationHz: Number(simulationHz.toFixed(2)),
        frames: summarizeFrameTimes(frameIntervals),
        frameWork: summarizeFrameTimes(frameWork),
        inputLatency: summarizeDistribution(sample.latencies),
        timings: Object.fromEntries(["models", "projectiles", "visibility", "particleWorker", "particleDecode", "audio", "dynamicItems", "world", "viewmodel", "render", "total"]
          .map((name) => [name, summarizeDistribution(unique.flatMap((frame) => Number.isFinite(frame.detail[name]) ? [frame.detail[name]!] : []))])),
        visibility: Object.fromEntries(["pvsSurfaces", "drawSurfaces", "leaves", "staticProps", "skySurfaces", "skyProps"]
          .map((name) => [name, summarizeDistribution(sample.frames.map((frame) => frame[name as keyof FrameRecord] as number))])),
        worker: summarizeWorkers(sample.workers),
        gpu: { ...sample.gpu, submissionsPerDisplayedFrame: Number((sample.gpu.submissions / unique.length).toFixed(2)) },
        longTasks: summarizeDistribution(sample.longTasks),
        gameplay: { bots: sample.bots, botShots: sample.botShots, botHits: sample.botHits, fireEvents: sample.fireEvents, particles: sample.particles, hud: sample.hud, water: sample.water, staticProps: sample.staticProps },
        pixels: { width: pixels.width, height: pixels.height, authored: authoredPixels, sha256: createHash("sha256").update(screenshot).digest("hex") },
      })
    }

    const cpu = (await cdp.send("Profiler.stop") as { profile: CpuProfile }).profile
    const allocations = (await cdp.send("HeapProfiler.stopSampling") as {
      profile: { head: { callFrame: { functionName: string; url: string }; selfSize: number; children: any[] } }
    }).profile
    const allocationRows: { function: string; url: string; bytes: number }[] = []
    const visit = (node: typeof allocations.head): void => {
      if (node.selfSize > 0) allocationRows.push({ function: node.callFrame.functionName || "(anonymous)", url: node.callFrame.url, bytes: node.selfSize })
      for (const child of node.children) visit(child)
    }
    visit(allocations.head)
    allocationRows.sort((left, right) => right.bytes - left.bytes)
    const heapAfter = await cdp.send("Runtime.getHeapUsage")
    const metricsAfter = await cdp.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }
    const baseline = new Map(metricsBefore.metrics.map((metric) => [metric.name, metric.value]))
    maps.push({
      target,
      activeMilliseconds: Number(observations.reduce((sum, value) => sum + Number(value.elapsedMilliseconds), 0).toFixed(3)),
      viewpoints: observations,
      cpu: summarizeCpuProfile(cpu),
      allocations: { heapBefore, heapAfter, sampledBytes: allocationRows.reduce((sum, value) => sum + value.bytes, 0), top: allocationRows.slice(0, 20) },
      browserMetrics: Object.fromEntries(metricsAfter.metrics.map((metric) => [metric.name, Number((metric.value - (baseline.get(metric.name) ?? 0)).toFixed(6))])),
    })
    console.log(`PLAYSRC_FRAME_BUDGET ${JSON.stringify({ target, viewpoints: observations.map((value) => ({
      viewpoint: value.viewpoint,
      fps: value.achievedFps,
      simulationHz: value.simulationHz,
      frames: value.frames,
      work: value.frameWork,
      timings: Object.fromEntries(Object.entries(value.timings as Record<string, { p95: number }>).map(([name, timing]) => [name, timing.p95])),
      visibleProps: (value.visibility as Record<string, { p95: number }>).staticProps!.p95,
      gpu: value.gpu,
    })) })}`)
  }

  const report = {
    schema: "playsrc-tf2-headed-all-maps-frame-budget-v1",
    headed: true,
    sampleSeconds: seconds,
    targetP95Milliseconds: 1_000 / 165,
    fallbackP95Milliseconds: 1_000 / 120,
    gpu: system.gpu,
    frameIntervals: summarizeFrameTimes(intervals),
    frameWork: summarizeFrameTimes(work),
    maps,
  }
  const serialized = `${JSON.stringify(report, null, 2)}\n`
  await writeFile(path.join(output, "report.json"), serialized)
  await testInfo.attach("headed-three-map-gameplay-frame-budget", { body: Buffer.from(serialized), contentType: "application/json" })
})
