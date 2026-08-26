import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
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
  renderer?: { drawCalls: number; frameCalls: number; triangles: number; memory: Record<string, number>; passes: { identity: string; submissions: number; commandBuffers: number; renderPasses: number; drawCalls: number; milliseconds: number; renderPipelines: number; nodeBuilderMisses: number }[]; poseUploadBytes: number; indexUploadBytes: number; indirectUploadBytes: number; bundleInvalidations: number; bundleEncodes: number; bundleEncodeMilliseconds: number; timestampMilliseconds: number | null }
}

test("profile headed grounded BLU Upward gameplay and completed multi-pass frames", async ({ page, context }, testInfo) => {
  const seconds = profileSampleSeconds()
  await page.addInitScript(installBrowserFrameProfiler)
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
    }
    ;(window as any).__playsrcProfile = state
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
  expect(await page.locator("main").getAttribute("data-movement-mode")).not.toBe("1")

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
      coverageSamples: CoverageSample[]
    }
    const instrumentation = (window as any).__playsrcFrameProfiler
    const root = document.querySelector<HTMLElement>("main")!
    const readPosition = () => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    const distance = (left: readonly number[], right: readonly number[]) => Math.hypot(...left.map((value, index) => value - right[index]!))
    const start = readPosition()
    const candidates = state.coverageSamples
      .map((sample) => ({ ...sample, distance: distance(start, sample.position) }))
      .filter((sample) => sample.distance > 450 && sample.distance < durationSeconds * 320 && Math.abs(sample.position[2] - start[2]) < 160)
      .sort((left, right) => right.distance - left.distance)
    const target = candidates[0]
    if (!target) throw new Error("authored grounded Upward gameplay route is unavailable")

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
    state.active = true
    instrumentation.active = true
    let lastTurn = state.started
    try {
      while (performance.now() - state.started < durationSeconds * 1_000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        if (root.dataset.phase !== "Ready") throw new Error(`Upward outdoor route entered ${root.dataset.phase}: ${root.dataset.detail}`)
        if (root.dataset.movementMode === "1") throw new Error("grounded Upward profile unexpectedly entered noclip")
        const now = performance.now()
        if (now - lastTurn > 160 && distance(readPosition(), target.position) > 96) {
          turn(target.position)
          lastTurn = now
        }
      }
    } finally {
      state.active = false
      instrumentation.active = false
    }
    const end = readPosition()
    return {
      startedMilliseconds: state.started,
      elapsedMilliseconds: Number((performance.now() - state.started).toFixed(3)),
      initial: start,
      target,
      final: end,
      traveled: Number(distance(start, end).toFixed(3)),
      terminalTick: Number(root.dataset.snapshotTick ?? 0),
      movementMode: root.dataset.movementMode,
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
    const profile = (window as any).__playsrcFrameProfiler
    return {
      frames: profile.completedFrames, animationCallbacks: profile.animationCallbacks, rpcs: profile.worker,
      longTasks: profile.longTasks, longAnimationFrames: profile.longAnimationFrames,
      gpu: profile.counters, capabilities: profile.capabilities, losses: profile.losses, gpuTimestamps: profile.gpuTimestamps,
    }
  }) as { frames: FrameRecord[]; animationCallbacks: number[]; rpcs: RpcRecord[]; longTasks: { at: number; duration: number }[]; longAnimationFrames: Record<string, any>[]; gpu: Record<string, number>; capabilities: Record<string, boolean>; losses: Record<string, unknown>[]; gpuTimestamps: { frame: number; milliseconds: number }[] }

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
  const displayedFrames = state.frames.filter((frame) => Number.isFinite(frame.detail.total))
  const callbackIntervals = state.animationCallbacks.slice(1).map((value, index) => value - state.animationCallbacks[index]!)
  const completedIntervals = displayedFrames.slice(1).map((frame, index) => frame.at - displayedFrames[index]!.at)
  const frameTimes = summarizeFrameTimes(callbackIntervals)
  const presentedFramesPerSecond = Number((displayedFrames.length / route.elapsedMilliseconds * 1_000).toFixed(3))
  const frameWork = summarizeFrameTimes(displayedFrames.map((frame) => frame.detail.total!))
  const fields = ["models", "projectiles", "visibility", "particleWorker", "particleDecode", "audio", "dynamicItems", "world", "viewmodel", "render", "total"]
  const timings = Object.fromEntries(fields.map((name) => [name, summarizeDistribution(displayedFrames.flatMap((frame) => Number.isFinite(frame.detail[name]) ? [frame.detail[name]!] : []))]))
  const ticksPerSecond = Number(((route.terminalTick - initial.tick) / route.elapsedMilliseconds * 1_000).toFixed(3))
  const metricValues = new Map((metricsBefore.metrics as { name: string; value: number }[]).map((metric) => [metric.name, metric.value]))
  const metricDeltas = Object.fromEntries((metricsAfter.metrics as { name: string; value: number }[]).map((metric) => [metric.name, Number((metric.value - (metricValues.get(metric.name) ?? 0)).toFixed(6))]))

  const report = {
    schema: "playsrc-tf2-upward-outdoor-profile-v2",
    sampleSeconds: seconds,
    route,
    ticksPerSecond,
    frameIntervals: frameTimes,
    completedFrameIntervals: summarizeFrameTimes(completedIntervals),
    presentedFrameIntervals: summarizeFrameTimes(completedIntervals),
    presentedFramesPerSecond,
    inputToDisplayMilliseconds: displayedFrames[0] ? Number((displayedFrames[0].at - route.startedMilliseconds).toFixed(3)) : null,
    workerQueueMaximum: state.gpu.workerMaximumPending,
    animationCallbacks: state.animationCallbacks.length,
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
    renderer: {
      drawCalls: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.drawCalls ?? 0)),
      frameCalls: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.frameCalls ?? 0)),
      triangles: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.triangles ?? 0)),
      poseUploadBytes: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.poseUploadBytes ?? 0)),
      indexUploadBytes: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.indexUploadBytes ?? 0)),
      indirectUploadBytes: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.indirectUploadBytes ?? 0)),
      bundleInvalidations: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.bundleInvalidations ?? 0)),
      bundleEncodes: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.bundleEncodes ?? 0)),
      bundleEncodeMilliseconds: summarizeDistribution(displayedFrames.map(frame => frame.renderer?.bundleEncodeMilliseconds ?? 0)),
      memory: Object.fromEntries(Object.keys(displayedFrames[0]?.renderer?.memory ?? {}).map(key =>
        [key, summarizeDistribution(displayedFrames.map(frame => frame.renderer?.memory[key] ?? 0))])),
      passes: Object.fromEntries([...new Set(displayedFrames.flatMap(frame => frame.renderer?.passes.map(pass => pass.identity) ?? []))].map(identity => {
        const passes = displayedFrames.flatMap(frame => frame.renderer?.passes.filter(pass => pass.identity === identity) ?? [])
        return [identity, {
          count: passes.length, submissions: passes.reduce((sum, pass) => sum + pass.submissions, 0),
          commandBuffers: passes.reduce((sum, pass) => sum + pass.commandBuffers, 0),
          renderPasses: passes.reduce((sum, pass) => sum + pass.renderPasses, 0),
          renderPipelines: passes.reduce((sum, pass) => sum + pass.renderPipelines, 0),
          nodeBuilderMisses: passes.reduce((sum, pass) => sum + pass.nodeBuilderMisses, 0),
          drawCalls: summarizeDistribution(passes.map(pass => pass.drawCalls)),
          milliseconds: summarizeDistribution(passes.map(pass => pass.milliseconds)),
        }]
      })),
      timestamps: {
        supported: state.capabilities.timestampQuery,
        milliseconds: summarizeDistribution(state.gpuTimestamps.map(sample => sample.milliseconds)),
        samples: state.gpuTimestamps,
      },
    },
    allocations: {
      heapBefore,
      heapAfter,
      sampledBytes: allocationRows.reduce((total, row) => total + row.bytes, 0),
      top: allocationRows.slice(0, 30),
      heapSamples: summarizeDistribution([]),
    },
    longTasks: summarizeDistribution(state.longTasks.map((task) => task.duration)),
    longAnimationFrames: {
      supported: state.capabilities.longAnimationFrame,
      duration: summarizeDistribution(state.longAnimationFrames.map(frame => frame.duration)),
      blocking: summarizeDistribution(state.longAnimationFrames.map(frame => frame.blockingDuration)),
      frames: state.longAnimationFrames,
    },
    losses: state.losses,
    browserMetrics: metricDeltas,
    cpu: summarizeCpuProfile(cpuProfile),
    pixels: {
      beforeSha256: createHash("sha256").update(before).digest("hex"),
      afterSha256: createHash("sha256").update(after).digest("hex"),
    },
    worstFrames: displayedFrames.toSorted((left, right) => right.detail.total - left.detail.total).slice(0, 10),
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
    completedFrameIntervals: report.completedFrameIntervals,
    displayedFrames: displayedFrames.length,
    outdoorFrames: report.visibility.outdoorFrames,
    staticProps: report.visibility.staticProps,
    gpu: state.gpu,
    drawCalls: report.renderer.drawCalls,
    worldBundles: { encodes: report.renderer.bundleEncodes, invalidations: report.renderer.bundleInvalidations, indexUploadBytes: report.renderer.indexUploadBytes, indirectUploadBytes: report.renderer.indirectUploadBytes },
    gpuMemoryBytes: report.renderer.memory.total,
    longAnimationFrames: report.longAnimationFrames.duration,
    worker: Object.fromEntries(Object.entries(worker).map(([kind, value]) => [kind, { calls: value.calls, p95: value.milliseconds.p95 }])),
    longTasks: report.longTasks,
    cpu: report.cpu.topSelf.slice(0, 8),
  })}`)

  expect(route.elapsedMilliseconds).toBeGreaterThanOrEqual(seconds * 1_000)
  expect(route.traveled).toBeGreaterThan(120)
  expect(route.movementMode).not.toBe("1")
  expect(ticksPerSecond).toBeGreaterThan(55)
  expect(state.animationCallbacks.length).toBeGreaterThan(20)
  expect(displayedFrames.length).toBeGreaterThan(20)
  expect(report.visibility.outdoorFrames).toBeGreaterThan(0)
  expect(report.visibility.drawSurfaces.max).toBeGreaterThan(0)
  expect(report.gpu.submissions).toBeGreaterThan(0)
  expect(report.renderer.drawCalls.max).toBeGreaterThan(0)
  expect(report.renderer.frameCalls.max).toBeGreaterThan(0)
  expect(report.renderer.memory.textures.max).toBeGreaterThan(0)
  if(report.renderer.timestamps.supported)expect(report.renderer.timestamps.milliseconds.count).toBeGreaterThan(0)
  expect(report.gpu.validationErrors).toBe(0)
  expect(report.losses).toEqual([])
  expect(report.worker.visibility?.calls ?? 0).toBeGreaterThan(0)
  expect(report.pixels.beforeSha256).not.toBe(report.pixels.afterSha256)
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
})
