import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"

type RpcRecord = { kind: string; started: number; finished?: number; bytes: number; timings?: Record<string, number> }
type CoverageSample = { leaf: number; cluster: number; area: number; position: readonly [number, number, number] }
type FrameRecord = {
  at: number
  interval: number
  position: readonly number[]
  tick: number
  displayFrame: number
  drawSurfaces: number
  leaves: number
  props: number
  skySurfaces: number
  skyProps: number
  heapBytes: number | null
  gpuSubmissions: number
  gpuCommandBuffers: number
  detail: Record<string, number>
}

test("profile headed BLU spawn-to-outdoor Upward grounded movement", async ({ page, context }, testInfo) => {
  const seconds = profileSampleSeconds()
  await page.addInitScript(() => {
    let pointerLockElement: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => pointerLockElement })
    Object.defineProperty(Element.prototype, "requestPointerLock", {
      configurable: true,
      value(this: Element) {
        pointerLockElement = this
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    Object.defineProperty(document, "exitPointerLock", {
      configurable: true,
      value() {
        pointerLockElement = null
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })

    const state = {
      active: false,
      started: 0,
      startedTick: 0,
      startedDisplayFrame: 0,
      rpcs: [] as RpcRecord[],
      frames: [] as FrameRecord[],
      longTasks: [] as { at: number; duration: number }[],
      input: { started: 0, displayed: 0 },
      workerQueueMaximum: 0,
      gpu: {
        submissions: 0, submitCalls: 0, commandBuffers: 0, buffers: 0, textures: 0,
        destroyedBuffers: 0, destroyedTextures: 0, renderPasses: 0, computePasses: 0,
      },
    }
    ;(window as any).__playsrcProfile = state

    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, RpcRecord>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const record = this.records.get(event.data?.id)
          if (record) {
            record.finished = performance.now()
            if (event.data?.timings) record.timings = event.data.timings
            this.records.delete(event.data.id)
          }
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (state.active && Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const record: RpcRecord = {
            kind: message.kind,
            started: performance.now(),
            bytes: message.command?.byteLength ?? message.batch?.byteLength ?? message.bsp?.byteLength ?? 0,
          }
          this.records.set(message.id, record)
          state.rpcs.push(record)
          state.workerQueueMaximum = Math.max(state.workerQueueMaximum, this.records.size)
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    const instrument = (owner: any, method: string, counter: keyof typeof state.gpu) => {
      const original = owner?.prototype?.[method]
      if (typeof original !== "function") return
      Object.defineProperty(owner.prototype, method, {
        configurable: true,
        writable: true,
        value(this: unknown, ...arguments_: any[]) {
          if (state.active) {
            state.gpu[counter] += counter === "submissions" ? arguments_[0]?.length ?? 0 : 1
            if (counter === "submissions") state.gpu.submitCalls += 1
          }
          return original.apply(this, arguments_)
        },
      })
    }
    instrument((globalThis as any).GPUQueue, "submit", "submissions")
    instrument((globalThis as any).GPUCommandEncoder, "finish", "commandBuffers")
    instrument((globalThis as any).GPUDevice, "createBuffer", "buffers")
    instrument((globalThis as any).GPUDevice, "createTexture", "textures")
    instrument((globalThis as any).GPUBuffer, "destroy", "destroyedBuffers")
    instrument((globalThis as any).GPUTexture, "destroy", "destroyedTextures")
    instrument((globalThis as any).GPUCommandEncoder, "beginRenderPass", "renderPasses")
    instrument((globalThis as any).GPUCommandEncoder, "beginComputePass", "computePasses")

    try {
      new PerformanceObserver((list) => {
        if (!state.active) return
        for (const entry of list.getEntries()) state.longTasks.push({ at: entry.startTime, duration: entry.duration })
      }).observe({ entryTypes: ["longtask"] })
    } catch {}

    let previous = performance.now()
    const sample = (now: number) => {
      const interval = now - previous
      previous = now
      if (state.active) {
        const root = document.querySelector<HTMLElement>("main")
        const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
        const profile = (window as any).__playsrcProfile
        const detail = root?.dataset.performanceDetail
        const displayFrame = Number(canvas?.dataset.displayFrame ?? 0)
        if (state.input.started !== 0 && state.input.displayed === 0 && displayFrame > state.startedDisplayFrame) {
          state.input.displayed = now
        }
        const completedDetail = detail && displayFrame > state.startedDisplayFrame ? JSON.parse(detail) : {}
        const sky = canvas?.dataset.sky3dPass
        const skyPass = sky ? JSON.parse(sky) : null
        state.frames.push({
          at: now,
          interval,
          position: (root?.dataset.cameraPosition ?? "").split(",").map(Number),
          tick: Number(root?.dataset.snapshotTick ?? 0),
          displayFrame,
          drawSurfaces: profile.displacementVisibility?.drawSurfaces?.length ?? 0,
          leaves: profile.displacementVisibility?.leaves?.length ?? 0,
          props: canvas?.dataset.visibleMainStaticProps ? JSON.parse(canvas.dataset.visibleMainStaticProps).length : 0,
          skySurfaces: skyPass?.skySurfaces ?? 0,
          skyProps: skyPass?.skyProps ?? 0,
          heapBytes: (performance as any).memory?.usedJSHeapSize ?? null,
          gpuSubmissions: state.gpu.submissions,
          gpuCommandBuffers: state.gpu.commandBuffers,
          detail: Number(completedDetail.tick ?? -1) >= state.startedTick ? completedDetail : {},
        })
      }
      requestAnimationFrame(sample)
    }
    requestAnimationFrame(sample)
  })

  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 25 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")

  const command = async (value: string) => {
    if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await expect(entry).toBeVisible()
    await entry.fill(value)
    await page.keyboard.press("Enter")
  }
  await command("map pl_upward")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed" || root?.dataset.teamSelectionVisible === "true"
  }, undefined, { timeout: 600_000, polling: 25 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.movementMode !== "1", undefined, { timeout: 30_000, polling: 20 })

  const canvas = page.locator("canvas.world-canvas")
  await expect(canvas).toBeVisible()
  await canvas.click()
  await page.waitForFunction(() => document.pointerLockElement?.classList.contains("world-canvas"), undefined, { timeout: 5_000 })
  await page.mouse.move(420, 320)
  await page.mouse.move(470, 320, { steps: 3 })
  const before = await canvas.screenshot()
  const initial = await page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const profile = (window as any).__playsrcProfile
    return {
      position: (root.dataset.cameraPosition ?? "").split(",").map(Number),
      tick: Number(root.dataset.snapshotTick ?? 0),
      yaw: Number(root.dataset.cameraYaw ?? 0),
      props: JSON.parse(canvas.dataset.staticProps ?? "{}"),
      samples: profile.coverageSamples as CoverageSample[],
    }
  })
  expect(initial.position).toHaveLength(3)
  expect(initial.samples.length).toBeGreaterThan(0)
  expect(initial.props.total).toBeGreaterThan(0)

  const cdp = await context.newCDPSession(page)
  await cdp.send("Performance.enable")
  await cdp.send("Profiler.enable")
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  await cdp.send("HeapProfiler.startSampling", { samplingInterval: 32_768 })
  const heapBefore = await cdp.send("Runtime.getHeapUsage")
  const metricsBefore = await cdp.send("Performance.getMetrics")
  await cdp.send("Profiler.start")

  await page.keyboard.down("w")
  const route = await page.evaluate(async (durationSeconds) => {
    const state = (window as any).__playsrcProfile as {
      active: boolean
      started: number
      startedTick: number
      startedDisplayFrame: number
      frames: FrameRecord[]
      coverageSamples: CoverageSample[]
    }
    const root = document.querySelector<HTMLElement>("main")!
    const readPosition = () => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    const distance = (left: readonly number[], right: readonly number[]) => Math.hypot(...left.map((value, index) => value - right[index]!))
    const start = readPosition()
    const candidates = state.coverageSamples
      .map((sample) => ({ ...sample, distance: distance(start, sample.position) }))
      .filter((sample) => sample.distance > 450 && sample.distance < durationSeconds * 320 && Math.abs(sample.position[2] - start[2]) < 160)
      .sort((left, right) => right.distance - left.distance)
    const target = candidates[0]
    if (!target) throw new Error("authored Upward grounded walking route is unavailable")

    const turn = (goal: readonly number[]) => {
      const position = readPosition()
      const dx = goal[0]! - position[0]!
      const dy = goal[1]! - position[1]!
      const dz = goal[2]! - position[2]!
      const desiredYaw = Math.atan2(dy, dx) * 180 / Math.PI
      const desiredPitch = -Math.atan2(dz, Math.hypot(dx, dy)) * 180 / Math.PI
      const currentYaw = Number(root.dataset.cameraYaw ?? 0)
      const currentPitch = Number(root.dataset.cameraPitch ?? 0)
      const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, {
        movementX: { value: wrap(currentYaw - desiredYaw) / 0.066 },
        movementY: { value: (desiredPitch - currentPitch) / 0.066 },
      })
      dispatchEvent(event)
    }

    turn(target.position)
    state.started = performance.now()
    state.startedTick = Number(root.dataset.snapshotTick ?? 0)
    state.startedDisplayFrame = Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0)
    ;(state as any).input.started = state.started
    state.active = true
    let lastTurn = state.started
    try {
      while (performance.now() - state.started < durationSeconds * 1_000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        if (root.dataset.phase !== "Ready") throw new Error(`Upward outdoor route entered ${root.dataset.phase}: ${root.dataset.detail}`)
        const now = performance.now()
        if (now - lastTurn > 160 && distance(readPosition(), target.position) > 96) {
          turn(target.position)
          lastTurn = now
        }
      }
    } finally {
      state.active = false
    }
    const end = readPosition()
    return {
      elapsedMilliseconds: Number((performance.now() - state.started).toFixed(3)),
      initial: start,
      target,
      final: end,
      traveled: Number(distance(start, end).toFixed(3)),
      terminalTick: Number(root.dataset.snapshotTick ?? 0),
      phase: root.dataset.phase,
    }
  }, seconds)
  await page.keyboard.up("w")

  const cpuProfile = (await cdp.send("Profiler.stop") as { profile: CpuProfile }).profile
  const allocationProfile = (await cdp.send("HeapProfiler.stopSampling") as {
    profile: { head: { callFrame: { functionName: string; url: string }; selfSize: number; children: any[] } }
  }).profile
  const heapAfter = await cdp.send("Runtime.getHeapUsage")
  const metricsAfter = await cdp.send("Performance.getMetrics")
  const after = await canvas.screenshot()
  const state = await page.evaluate(() => {
    const profile = (window as any).__playsrcProfile
    return {
      frames: profile.frames, rpcs: profile.rpcs, longTasks: profile.longTasks, gpu: profile.gpu,
      input: profile.input, workerQueueMaximum: profile.workerQueueMaximum,
    }
  }) as {
    frames: FrameRecord[]; rpcs: RpcRecord[]; longTasks: { at: number; duration: number }[]
    gpu: Record<string, number>; input: { started: number; displayed: number }; workerQueueMaximum: number
  }

  const allocationRows: { function: string; url: string; bytes: number }[] = []
  const visitAllocation = (node: { callFrame: { functionName: string; url: string }; selfSize: number; children: any[] }) => {
    if (node.selfSize > 0) allocationRows.push({ function: node.callFrame.functionName || "(anonymous)", url: node.callFrame.url, bytes: node.selfSize })
    for (const child of node.children) visitAllocation(child)
  }
  visitAllocation(allocationProfile.head)
  allocationRows.sort((left, right) => right.bytes - left.bytes)

  const workerKinds = [...new Set(state.rpcs.map((record) => record.kind))].sort()
  const worker = Object.fromEntries(workerKinds.map((kind) => {
    const values = state.rpcs.filter((record) => record.kind === kind)
    return [kind, {
      calls: values.length,
      completed: values.filter((record) => record.finished !== undefined).length,
      bytes: values.reduce((total, record) => total + record.bytes, 0),
      milliseconds: summarizeDistribution(values.flatMap((record) => record.finished === undefined ? [] : [record.finished - record.started])),
      workerTimings: Object.fromEntries([...new Set(values.flatMap((record) => Object.keys(record.timings ?? {})))].sort().map((name) =>
        [name, summarizeDistribution(values.flatMap((record) => typeof record.timings?.[name] === "number" ? [record.timings[name]!] : []))])),
    }]
  }))
  const displayedFrames = [...new Map(state.frames
    .filter((frame) => Number.isFinite(frame.detail.total))
    .map((frame) => [frame.displayFrame, frame])).values()]
  const frameTimes = summarizeFrameTimes(state.frames.map((frame) => frame.interval))
  const presentedIntervals = summarizeFrameTimes(displayedFrames.map((frame, index) =>
    index === 0 ? frame.at - (state.frames[0]?.at ?? frame.at) : frame.at - displayedFrames[index - 1]!.at))
  const presentedFramesPerSecond = Number((displayedFrames.length / route.elapsedMilliseconds * 1_000).toFixed(3))
  const frameWork = summarizeFrameTimes(displayedFrames.map((frame) => frame.detail.total!))
  const fields = ["models", "projectiles", "visibility", "particleWorker", "particleDecode", "audio", "dynamicItems", "world", "viewmodel", "render", "total"]
  const timings = Object.fromEntries(fields.map((name) => [name, summarizeDistribution(displayedFrames.flatMap((frame) => Number.isFinite(frame.detail[name]) ? [frame.detail[name]!] : []))]))
  const ticksPerSecond = Number(((route.terminalTick - initial.tick) / route.elapsedMilliseconds * 1_000).toFixed(3))
  const metricValues = new Map((metricsBefore.metrics as { name: string; value: number }[]).map((metric) => [metric.name, metric.value]))
  const metricDeltas = Object.fromEntries((metricsAfter.metrics as { name: string; value: number }[]).map((metric) => [metric.name, Number((metric.value - (metricValues.get(metric.name) ?? 0)).toFixed(6))]))

  const report = {
    schema: "playsrc-tf2-upward-outdoor-profile-v1",
    sampleSeconds: seconds,
    route,
    ticksPerSecond,
    frameIntervals: frameTimes,
    presentedFrameIntervals: presentedIntervals,
    presentedFramesPerSecond,
    inputToDisplayMilliseconds: state.input.displayed === 0 ? null : Number((state.input.displayed - state.input.started).toFixed(3)),
    workerQueueMaximum: state.workerQueueMaximum,
    frameWork,
    displayedFrames: displayedFrames.length,
    timings,
    visibility: {
      drawSurfaces: summarizeDistribution(state.frames.map((frame) => frame.drawSurfaces)),
      leaves: summarizeDistribution(state.frames.map((frame) => frame.leaves)),
      staticProps: summarizeDistribution(state.frames.map((frame) => frame.props)),
      skySurfaces: summarizeDistribution(state.frames.map((frame) => frame.skySurfaces)),
      skyProps: summarizeDistribution(state.frames.map((frame) => frame.skyProps)),
      outdoorFrames: state.frames.filter((frame) => frame.skySurfaces > 0).length,
      totalStaticProps: initial.props.total,
      staticPropConfiguration: initial.props,
    },
    particles: {
      items: summarizeDistribution(displayedFrames.flatMap((frame) => Number.isFinite(frame.detail.particleItems) ? [frame.detail.particleItems!] : [])),
      batches: summarizeDistribution(displayedFrames.flatMap((frame) => Number.isFinite(frame.detail.particleBatches) ? [frame.detail.particleBatches!] : [])),
    },
    worker,
    gpu: state.gpu,
    allocations: {
      heapBefore,
      heapAfter,
      sampledBytes: allocationRows.reduce((total, row) => total + row.bytes, 0),
      top: allocationRows.slice(0, 30),
      heapSamples: summarizeDistribution(state.frames.flatMap((frame) => frame.heapBytes === null ? [] : [frame.heapBytes])),
    },
    longTasks: summarizeDistribution(state.longTasks.map((task) => task.duration)),
    browserMetrics: metricDeltas,
    cpu: summarizeCpuProfile(cpuProfile),
    pixels: {
      beforeSha256: createHash("sha256").update(before).digest("hex"),
      afterSha256: createHash("sha256").update(after).digest("hex"),
    },
    worstFrames: state.frames.toSorted((left, right) => right.interval - left.interval).slice(0, 10),
  }
  const local = await loadLocalConfig(process.cwd())
  const directory = path.join(local.sourceCacheDir, "profiles", "upward-outdoors")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "report.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "cpu.cpuprofile"), `${JSON.stringify(cpuProfile)}\n`),
    writeFile(path.join(directory, "spawn.png"), before),
    writeFile(path.join(directory, "outdoors.png"), after),
  ])
  await testInfo.attach("headed-upward-outdoor-performance", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(`PLAYSRC_UPWARD_OUTDOORS ${JSON.stringify({
    sampleSeconds: seconds,
    elapsedMilliseconds: route.elapsedMilliseconds,
    traveled: route.traveled,
    ticksPerSecond,
    frames: frameTimes,
    frameWork,
    displayedFrames: displayedFrames.length,
    presentedFramesPerSecond,
    presentedFrameIntervals: presentedIntervals,
    inputToDisplayMilliseconds: report.inputToDisplayMilliseconds,
    workerQueueMaximum: state.workerQueueMaximum,
    outdoorFrames: report.visibility.outdoorFrames,
    staticProps: report.visibility.staticProps,
    gpu: state.gpu,
    worker: Object.fromEntries(Object.entries(worker).map(([kind, value]) => [kind, { calls: value.calls, p95: value.milliseconds.p95 }])),
    longTasks: report.longTasks,
    cpu: report.cpu.topSelf.slice(0, 8),
  })}`)

  expect(route.elapsedMilliseconds).toBeGreaterThanOrEqual(seconds * 1_000)
  expect(route.traveled).toBeGreaterThan(120)
  expect(ticksPerSecond).toBeGreaterThan(55)
  expect(state.frames.length).toBeGreaterThan(20)
  expect(displayedFrames.length).toBeGreaterThan(20)
  expect(report.inputToDisplayMilliseconds).not.toBeNull()
  expect(report.visibility.outdoorFrames).toBeGreaterThan(0)
  expect(report.visibility.drawSurfaces.max).toBeGreaterThan(0)
  expect(report.gpu.submissions).toBeGreaterThan(0)
  expect(report.worker.visibility?.calls ?? 0).toBeGreaterThan(0)
  expect(report.pixels.beforeSha256).not.toBe(report.pixels.afterSha256)
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
})
