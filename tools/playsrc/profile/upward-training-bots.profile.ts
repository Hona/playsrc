import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { assertVisibleGameplayTruth, summarizeCompositorTruth, type ChromiumTraceEvent } from "./compositor-truth"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

function processResidentMemory(processes: Array<{ id: number; type: string }> | undefined) {
  if (!processes?.length || process.platform === "win32") return null
  const result = Bun.spawnSync(["ps", "-o", "pid=,rss=", "-p", processes.map(process => process.id).join(",")], { stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) return null
  const resident = new Map(new TextDecoder().decode(result.stdout).trim().split("\n").map(line => line.trim().split(/\s+/).map(Number) as [number, number]))
  return processes.map(process => ({ id: process.id, type: process.type, residentBytes: resident.has(process.id) ? resident.get(process.id)! * 1024 : null }))
}

test("profile authored headed Upward offline-practice default roster and actual completed gameplay frames", async ({ page, context }, testInfo) => {
  const wallStarted = Date.now()
  const seconds = profileSampleSeconds()
  const label = process.env.PROFILE_UPWARD_TRAINING_LABEL ?? "latest"
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "profiles", "upward-training-bots")
  await mkdir(directory, { recursive: true })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => {
    performance.setResourceTimingBufferSize(4096)
    ;(globalThis as any).__playsrcProfile = {}
  })
  let localProductionBundle = false
  if (process.env.PLAYSRC_PROFILE_LOCAL_PRODUCTION_BUNDLE === "1") {
    if (process.env.PLAYSRC_PROFILE_ORIGIN !== "https://playsrc.online") {
      throw new Error("local production bundle requires the exact deployed application and immutable asset origins")
    }
    const root = path.join(repositoryRoot, "apps", "web", "tf2")
    const { build } = await import("vite")
    await build({ root, configFile: path.join(root, "vite.config.ts"), logLevel: "error" })
    const output = path.join(root, "dist", "cloudflare", "tf2")
    const html = await readFile(path.join(output, "index.html"), "utf8")
    const javascript = /src="\/tf2\/assets\/(index-[^"]+\.js)"/u.exec(html)?.[1]
    const stylesheet = /href="\/tf2\/assets\/(index-[^"]+\.css)"/u.exec(html)?.[1]
    if (!javascript || !stylesheet) throw new Error("local production-equivalent application bundle is incomplete")
    await page.route("https://playsrc.online/tf2/assets/**", async (route) => {
      const requested = path.basename(new URL(route.request().url()).pathname)
      const name = /^index-[^/]+\.js$/u.test(requested) ? javascript
        : /^index-[^/]+\.css$/u.test(requested) ? stylesheet
        : requested
      const file = path.join(output, "assets", name)
      let body: Buffer
      try { body = await readFile(file) }
      catch { throw new Error(`local production-equivalent application asset is absent: ${requested}`) }
      await route.fulfill({
        status: 200,
        body,
        contentType: name.endsWith(".css") ? "text/css" : "text/javascript",
        headers: { "cross-origin-embedder-policy": "require-corp", "cross-origin-opener-policy": "same-origin" },
      })
    })
    localProductionBundle = true
  }
  const network = {
    requests: 0, failed: 0, responseBytes: 0, maximumInflight: 0, duplicateImmutableRequests: 0,
    immutableRequests: 0, immutableBytes: 0, maximumImmutableInflight: 0, wireBytes: 0,
    cacheHits: 0, contentEncodings: {} as Record<string, number>,
    stages: {} as Record<string, { requests: number; bytes: number }>,
    timings: [] as Array<{ url: string; stage: string; bytes: number; ttfbMilliseconds: number; durationMilliseconds: number; protocol: string }>,
  }
  let inflight = 0
  let immutableInflight = 0
  const immutableIdentities = new Set<string>()
  const requestStages = new Map<import("@playwright/test").Request, string>()
  let networkStage = "cold-startup"
  page.on("request", request => {
    network.requests += 1
    network.maximumInflight = Math.max(network.maximumInflight, ++inflight)
    requestStages.set(request, networkStage)
    if (/\/objects\/sha256\/[0-9a-f]{64}$/u.test(request.url())) {
      network.immutableRequests += 1
      network.maximumImmutableInflight = Math.max(network.maximumImmutableInflight, ++immutableInflight)
      if (immutableIdentities.has(request.url())) network.duplicateImmutableRequests += 1
      immutableIdentities.add(request.url())
    }
  })
  page.on("requestfailed", request => {
    network.failed += 1
    inflight = Math.max(0, inflight - 1)
    if (/\/objects\/sha256\/[0-9a-f]{64}$/u.test(request.url())) immutableInflight = Math.max(0, immutableInflight - 1)
  })
  page.on("requestfinished", async request => {
    inflight = Math.max(0, inflight - 1)
    if (/\/objects\/sha256\/[0-9a-f]{64}$/u.test(request.url())) immutableInflight = Math.max(0, immutableInflight - 1)
    const response = await request.response()
    if (!response) return
    const bytes = Number(response.headers()["content-length"] ?? 0)
    const stage = requestStages.get(request) ?? "unknown"
    const timing = request.timing()
    network.timings.push({
      url: request.url(), stage, bytes,
      ttfbMilliseconds: Number(Math.max(0, timing.responseStart - timing.requestStart).toFixed(3)),
      durationMilliseconds: Number(Math.max(0, timing.responseEnd - timing.requestStart).toFixed(3)),
      protocol: response.headers()["alt-svc"]?.includes("h3") ? "h3-advertised" : "http",
    })
  })
  page.on("response", response => {
    const bytes = Number(response.headers()["content-length"] ?? 0)
    network.responseBytes += bytes
    const stage = requestStages.get(response.request()) ?? "unknown"
    const totals = network.stages[stage] ??= { requests: 0, bytes: 0 }
    totals.requests += 1
    totals.bytes += bytes
    if (/\/objects\/sha256\/[0-9a-f]{64}$/u.test(response.url())) network.immutableBytes += bytes
    const encoding = response.headers()["content-encoding"] ?? "identity"
    network.contentEncodings[encoding] = (network.contentEncodings[encoding] ?? 0) + 1
  })
  const networkCdp = await context.newCDPSession(page)
  await networkCdp.send("Network.enable")
  if (localProductionBundle) await networkCdp.send("Network.setCacheDisabled", { cacheDisabled: false })
  networkCdp.on("Network.loadingFinished", ({ encodedDataLength }) => { network.wireBytes += encodedDataLength })
  networkCdp.on("Network.requestServedFromCache", () => { network.cacheHits += 1 })
  const root = page.locator("main")
  const layer = page.locator(".local-match-layer")
  const loads: Array<{
    cache: "cold" | "warm"
    startupMilliseconds: number
    readyMilliseconds: number
    requests: number
    responseBytes: number
    transferredBytes: number
    wireBytes: number
    playerCount: number
    launch: any
  }> = []
  const loadPractice = async (cache: "cold" | "warm") => {
    const started = Date.now()
    const previousRequests = network.requests
    const previousBytes = network.responseBytes
    const previousWireBytes = network.wireBytes
    networkStage = `${cache}-startup`
    if (cache === "cold") await page.goto(process.env.PLAYSRC_PROFILE_ORIGIN ? "/tf2" : "/", { waitUntil: "domcontentloaded", timeout: 30_000 })
    else await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 })
    await page.bringToFront()
    await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 100_000 })
    const startupMilliseconds = Date.now() - started
    networkStage = `${cache}-menu`
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']").click()
    await layer.locator("[data-vgui-name='OfflinePracticePanel'] [data-vgui-name='StartButton']").click()
    const next = layer.locator("[data-vgui-name='OfflinePractice_ModeSelectionPanel'] [data-vgui-name='NextButton']")
    await next.click()
    await next.click()
    await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("Payload")
    await layer.locator("[data-vgui-name='SelectCurrentGameModeButton']").click()
    await expect(layer.locator("[data-vgui-name='MapNameLabel']")).toHaveText("Upward")
    const mapPanel = layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel']")
    const playerCount = Number(await mapPanel.locator("[data-vgui-name='NumPlayersTextEntry']").inputValue())
    expect(playerCount).toBeGreaterThanOrEqual(12)
    const mapStarted = Date.now()
    networkStage = `${cache}-map`
    await mapPanel.locator("[data-vgui-name='StartOfflinePracticeButton']").click({ timeout: 5_000 })
    await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 110_000 })
    await chooseTf2Team(page, "red")
    await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
    const readyMilliseconds = Date.now() - mapStarted
    networkStage = `${cache}-gameplay`
    await expect(root).toHaveAttribute("data-bot-count", String(playerCount - 1), { timeout: 70_000 })
    const launch = JSON.parse(await root.getAttribute("data-local-match-settings") ?? "null")
    expect(launch).toMatchObject({ entry: "training", mapIdentity: "pl_upward", configuration: { quota: playerCount - 1, offlinePractice: true } })
    const transferredBytes = await page.evaluate(() => (performance.getEntriesByType("resource") as PerformanceResourceTiming[]).reduce((total, entry) => total + entry.transferSize, 0))
    loads.push({ cache, startupMilliseconds, readyMilliseconds, requests: network.requests - previousRequests, responseBytes: network.responseBytes - previousBytes, transferredBytes, wireBytes: network.wireBytes - previousWireBytes, playerCount, launch })
  }
  await loadPractice("cold")
  if (process.env.PROFILE_UPWARD_TRAINING_WARM_RELOAD === "1") await loadPractice("warm")
  const finalLoad = loads.at(-1)!
  const { readyMilliseconds, playerCount, launch } = finalLoad
  const expectedBots = playerCount - 1

  const canvas = page.locator("canvas.world-canvas")
  const before = await canvas.screenshot({ timeout: 20_000 })
  const cdp = await context.newCDPSession(page)
  const browserCdp = await context.browser()!.newBrowserCDPSession()
  const system = await browserCdp.send("SystemInfo.getInfo").catch(() => null)
  const processBefore = await browserCdp.send("SystemInfo.getProcessInfo").catch(() => null)
  const residentBefore = processResidentMemory(processBefore?.processInfo)
  const traceEvents: ChromiumTraceEvent[] = []
  cdp.on("Tracing.dataCollected", ({ value }) => traceEvents.push(...value as ChromiumTraceEvent[]))
  await cdp.send("Performance.enable")
  await cdp.send("Profiler.enable")
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  const heapBefore = await cdp.send("Runtime.getHeapUsage")
  await cdp.send("Profiler.start")
  await page.keyboard.down("w")
  await cdp.send("Tracing.start", { categories: "benchmark,viz,gpu,devtools.timeline", options: "record-as-much-as-possible" })
  const clockBefore = (await cdp.send("Performance.getMetrics")).metrics.find(metric => metric.name === "Timestamp")?.value
  const measurement = await page.evaluate(async (duration) => {
    const main = document.querySelector<HTMLElement>("main")!
    const surface = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const instrumentation = (globalThis as any).__playsrcFrameProfiler
    const firstTick = Number(main.dataset.snapshotTick)
    const firstFrame = Number(surface.dataset.displayFrame)
    const firstPosition = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    const firstUploads = structuredClone((globalThis as any).__playsrcProfile.modelParticleUploads ?? {}) as Record<string, number>
    const started = performance.now()
    let animationCallbacks = 0
    instrumentation.active = true
    try {
      while (performance.now() - started < duration * 1000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        animationCallbacks += 1
        if (main.dataset.phase !== "Ready") throw new Error(`Upward training left gameplay: ${main.dataset.phase}: ${main.dataset.detail}`)
      }
    } finally { instrumentation.active = false }
    const elapsed = performance.now() - started
    const position = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    return {
      elapsed, firstTick, lastTick: Number(main.dataset.snapshotTick), firstFrame,
      visible: document.visibilityState === "visible", focused: document.hasFocus(), animationCallbacks,
      viewport: { width: innerWidth, height: innerHeight, devicePixelRatio, visualViewportScale: visualViewport?.scale ?? null, canvasWidth: surface.width, canvasHeight: surface.height },
      lastFrame: Number(surface.dataset.displayFrame), traveled: Math.hypot(...position.map((value, index) => value - firstPosition[index]!)),
      roster: structuredClone((globalThis as any).__playsrcProfile.bots), scoreboard: JSON.parse(main.dataset.scoreboardProbe ?? "{}"),
      frames: instrumentation.completedFrames, compositorFrames: instrumentation.compositorFrames,
      presentationCallbacks: instrumentation.animationCallbacks, worker: instrumentation.worker, counters: instrumentation.counters, queueWrites: instrumentation.queueWrites,
      modelUploads: Object.fromEntries(Object.entries((globalThis as any).__playsrcProfile.modelParticleUploads ?? {})
        .map(([key, value]) => [key, typeof value === "number" ? value - (firstUploads[key] ?? 0) : value])),
      longTasks: instrumentation.longTasks.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
      longAnimationFrames: instrumentation.longAnimationFrames.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
    }
  }, seconds)
  const clockAfter = (await cdp.send("Performance.getMetrics")).metrics.find(metric => metric.name === "Timestamp")?.value
  const traceFinished = new Promise<void>(resolve => cdp.once("Tracing.tracingComplete", () => resolve()))
  await cdp.send("Tracing.end")
  await page.keyboard.up("w")
  await traceFinished
  const cpuProfile = (await cdp.send("Profiler.stop") as { profile: CpuProfile }).profile
  const heapAfter = await cdp.send("Runtime.getHeapUsage")
  const processAfter = await browserCdp.send("SystemInfo.getProcessInfo").catch(() => null)
  const residentAfter = processResidentMemory(processAfter?.processInfo)
  const storage = await page.evaluate(async () => {
    const resources = performance.getEntriesByType("resource") as PerformanceResourceTiming[]
    return {
      serviceWorkerControlled: Boolean(navigator.serviceWorker?.controller),
      indexedDatabases: typeof indexedDB.databases === "function" ? await indexedDB.databases() : null,
      estimate: navigator.storage?.estimate ? await navigator.storage.estimate() : null,
      navigation: performance.getEntriesByType("navigation").map(entry => entry.toJSON()),
      resourceCount: resources.length,
      transferredBytes: resources.reduce((total, entry) => total + entry.transferSize, 0),
      encodedBytes: resources.reduce((total, entry) => total + entry.encodedBodySize, 0),
      decodedBytes: resources.reduce((total, entry) => total + entry.decodedBodySize, 0),
      protocols: Object.fromEntries([...new Set(resources.map(entry => entry.nextHopProtocol))].map(protocol => [protocol || "unavailable", resources.filter(entry => entry.nextHopProtocol === protocol).length])),
    }
  })
  const after = await canvas.screenshot({ timeout: 20_000 })
  const completed = measurement.frames as Array<{ at: number; tick: number; detail: Record<string, number>; renderer: { passes: Array<{ submissions: number }> } }>
  const compositor=measurement.compositorFrames as Array<{at:number;submittedAt:number;submissionMilliseconds:number}>
  const intervals = completed.slice(1).map((frame, index) => frame.at - completed[index]!.at)
  const workers = measurement.worker as Array<{ kind: string; started: number; finished?: number; bytes: number; receivedBytes?: number; views?: number; sharedDispatch?: boolean; timings?: Record<string, number> }>
  const worker = Object.fromEntries([...new Set(workers.map(item => item.kind))].sort().map(kind => {
    const records = workers.filter(item => item.kind === kind)
    return [kind, { calls: records.length, views: records.reduce((sum, item) => sum + (item.views ?? 0), 0), sharedDispatches: records.filter(item => item.sharedDispatch).length, bytes: records.reduce((sum, item) => sum + item.bytes, 0), receivedBytes: records.reduce((sum, item) => sum + (item.receivedBytes ?? 0), 0), milliseconds: summarizeDistribution(records.flatMap(item => item.finished === undefined ? [] : [item.finished - item.started])), timings: Object.fromEntries([...new Set(records.flatMap(item => Object.keys(item.timings ?? {})))].map(key => [key, summarizeDistribution(records.flatMap(item => typeof item.timings?.[key] === "number" ? [item.timings[key]!] : []))])) }]
  }))
  const decoded = decodeScreenshot(after)
  let nonBlack = 0
  for (let index = 0; index < decoded.pixels.length; index += decoded.channels) {
    if (decoded.pixels[index]! > 16 || decoded.pixels[index + 1]! > 16 || decoded.pixels[index + 2]! > 16) nonBlack += 1
  }
  const actualFrames = measurement.lastFrame - measurement.firstFrame
  const report = {
    schema: "playsrc-tf2-upward-training-bots-profile-v1", label, headed: true, target: "pl_upward", entry: "training", launch,
    activeBots: measurement.roster.length, teams: { red: measurement.scoreboard.red.playerCount, blue: measurement.scoreboard.blue.playerCount },
    elapsedMilliseconds: Number(measurement.elapsed.toFixed(3)), readyMilliseconds, loads, totalWallMilliseconds: Date.now() - wallStarted,
    animationCallbacks: measurement.animationCallbacks, completedFrames: actualFrames, applicationCompletedFramesPerSecond: Number((actualFrames / measurement.elapsed * 1000).toFixed(3)),
    compositor: summarizeCompositorTruth(traceEvents, measurement.elapsed, clockBefore !== undefined && clockAfter !== undefined ? { startedMicroseconds: clockBefore * 1_000_000, endedMicroseconds: clockAfter * 1_000_000 } : undefined),
    presentationOpportunities:{frames:compositor.length,framesPerSecond:Number((compositor.length/measurement.elapsed*1000).toFixed(3)),animationCallbacks:measurement.presentationCallbacks.length,intervals:summarizeFrameTimes(compositor.slice(1).map((frame,index)=>frame.at-compositor[index]!.at)),submissionLatency:summarizeDistribution(compositor.map(frame=>frame.submissionMilliseconds))},
    browser: { platform: process.platform, origin: new URL(page.url()).origin, localProductionBundle, channel: process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? "playwright-chromium", viewport: measurement.viewport, visible: measurement.visible, focused: measurement.focused, gpu: system?.gpu ?? null, processes: { before: processBefore?.processInfo ?? null, after: processAfter?.processInfo ?? null, residentBefore, residentAfter }, network, storage, userMachineEvidence: false },
    frameIntervals: summarizeFrameTimes(intervals), frameWork: summarizeFrameTimes(completed.map(frame => frame.detail.total)),
    simulation: { ticks: measurement.lastTick - measurement.firstTick, hertz: Number(((measurement.lastTick - measurement.firstTick) / measurement.elapsed * 1000).toFixed(3)) },
    botWork: summarizeDistribution(completed.map(frame => frame.detail.models)), worker,
    gpu: {
      ...measurement.counters,
      modelUploads: measurement.modelUploads,
      submissionsPerCompletedFrame: Number((measurement.counters.submissions / Math.max(1, actualFrames)).toFixed(3)),
      writes: measurement.queueWrites,
      processCpuSeconds: (processAfter?.processInfo ?? []).map(process => {
        const before = processBefore?.processInfo.find(previous => previous.id === process.id)
        return { id: process.id, type: process.type, seconds: before ? Number((process.cpuTime - before.cpuTime).toFixed(6)) : null }
      }),
    },
    longAnimationFrames: summarizeDistribution(measurement.longAnimationFrames.map((frame: { duration: number }) => frame.duration)),
    longTasks: summarizeDistribution(measurement.longTasks.map((task: { duration: number }) => task.duration)),
    memory: { beforeBytes: heapBefore.usedSize, afterBytes: heapAfter.usedSize, embedderBytes: heapAfter.embedderHeapUsedSize },
    traveled: Number(measurement.traveled.toFixed(3)), cpu: summarizeCpuProfile(cpuProfile),
    pixels: { nonBlack, beforeSha256: createHash("sha256").update(before).digest("hex"), afterSha256: createHash("sha256").update(after).digest("hex") },
  }
  await Promise.all([
    writeFile(path.join(directory, `${label}.json`), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, `${label}.cpuprofile`), `${JSON.stringify(cpuProfile)}\n`),
    writeFile(path.join(directory, `${label}-before.png`), before),
    writeFile(path.join(directory, `${label}-after.png`), after),
  ])
  assertVisibleGameplayTruth({ visible: measurement.visible, focused: measurement.focused, ticks: report.simulation.ticks, displayFrames: actualFrames, submissions: measurement.counters.submissions, beforeSha256: report.pixels.beforeSha256, afterSha256: report.pixels.afterSha256 })
  await testInfo.attach("headed-upward-default-training-bots", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(`PLAYSRC_UPWARD_TRAINING_BOTS ${JSON.stringify({
    label, activeBots: report.activeBots, teams: report.teams,
    animationCallbacks: report.animationCallbacks, completedFrames: report.completedFrames, applicationCompletedFramesPerSecond: report.applicationCompletedFramesPerSecond, compositor: report.compositor,
    browser: {
      ...report.browser,
      network: {
        ...report.browser.network,
        timings: {
          ttfbMilliseconds: summarizeDistribution(report.browser.network.timings.map(request => request.ttfbMilliseconds)),
          durationMilliseconds: summarizeDistribution(report.browser.network.timings.map(request => request.durationMilliseconds)),
        },
      },
      gpu: { devices: report.browser.gpu?.devices ?? null, featureStatus: report.browser.gpu?.featureStatus ?? null },
      storage: { ...report.browser.storage, navigation: undefined },
    },
    frameIntervals: report.frameIntervals, frameWork: report.frameWork, simulation: report.simulation,
    worker: Object.fromEntries(Object.entries(worker).map(([kind, value]) => [kind, {
      calls: value.calls, views: value.views, sharedDispatches: value.sharedDispatches, bytes: value.bytes, receivedBytes: value.receivedBytes,
      queue: value.timings.queueMilliseconds ?? null, maximumMilliseconds: value.milliseconds.max,
      transactMaximumMilliseconds: value.timings.transactMilliseconds?.max ?? 0,
    }])),
    gpuSubmissionsPerCompletedFrame: report.gpu.submissionsPerCompletedFrame,
    modelUploads: report.gpu.modelUploads,
    gpuWrites: { calls: report.gpu.queueWriteCalls, bytes: report.gpu.queueWriteBytes, milliseconds: report.gpu.queueWriteMilliseconds, histogram: report.gpu.writes.histogram, phases: report.gpu.writes.phases, processCpuSeconds: report.gpu.processCpuSeconds },
    memory: report.memory, readyMilliseconds, loads: report.loads, totalWallMilliseconds: report.totalWallMilliseconds,
    traveled: report.traveled, longAnimationFrames: report.longAnimationFrames,
    cpu: report.cpu.topSelf.slice(0, 8), pixels: report.pixels,
  })}`)
  expect(report.activeBots).toBe(expectedBots)
  expect(report.teams.red + report.teams.blue).toBe(playerCount)
  expect(report.completedFrames).toBeGreaterThan(0)
  expect(report.pixels.nonBlack).toBeGreaterThan(20_000)
  if (process.env.PROFILE_UPWARD_TRAINING_REQUIRE_SMOOTH === "1") {
    expect(report.applicationCompletedFramesPerSecond).toBeGreaterThanOrEqual(55)
    expect(report.simulation.hertz).toBeGreaterThanOrEqual(60)
    expect(report.frameIntervals.p95Milliseconds).toBeLessThan(35)
  }
})
