import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
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
  await page.addInitScript(() => { ;(globalThis as any).__playsrcProfile = {} })
  const network = { requests: 0, failed: 0, responseBytes: 0 }
  page.on("request", () => network.requests += 1)
  page.on("requestfailed", () => network.failed += 1)
  page.on("response", response => network.responseBytes += Number(response.headers()["content-length"] ?? 0))
  const root = page.locator("main")
  const layer = page.locator(".local-match-layer")
  await page.goto(process.env.PLAYSRC_PROFILE_ORIGIN ? "/tf2" : "/")
  await page.bringToFront()
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 100_000 })
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
  await mapPanel.locator("[data-vgui-name='StartOfflinePracticeButton']").click({ timeout: 5_000 })
  await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 110_000 })
  await chooseTf2Team(page, "red")
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const readyMilliseconds = Date.now() - mapStarted
  const expectedBots = playerCount - 1
  await expect(root).toHaveAttribute("data-bot-count", String(expectedBots), { timeout: 70_000 })
  const launch = JSON.parse(await root.getAttribute("data-local-match-settings") ?? "null")
  expect(launch).toMatchObject({ entry: "training", mapIdentity: "pl_upward", configuration: { quota: expectedBots, offlinePractice: true } })

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
      frames: instrumentation.completedFrames, worker: instrumentation.worker, counters: instrumentation.counters,
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
  const storage = await page.evaluate(async () => ({ serviceWorkerControlled: Boolean(navigator.serviceWorker?.controller), indexedDatabases: typeof indexedDB.databases === "function" ? await indexedDB.databases() : null, estimate: navigator.storage?.estimate ? await navigator.storage.estimate() : null, navigation: performance.getEntriesByType("navigation").map(entry => entry.toJSON()), resourceCount: performance.getEntriesByType("resource").length }))
  const after = await canvas.screenshot({ timeout: 20_000 })
  const completed = measurement.frames as Array<{ at: number; tick: number; detail: Record<string, number>; renderer: { passes: Array<{ submissions: number }> } }>
  const intervals = completed.slice(1).map((frame, index) => frame.at - completed[index]!.at)
  const workers = measurement.worker as Array<{ kind: string; started: number; finished?: number; bytes: number; receivedBytes?: number; timings?: Record<string, number> }>
  const worker = Object.fromEntries([...new Set(workers.map(item => item.kind))].sort().map(kind => {
    const records = workers.filter(item => item.kind === kind)
    return [kind, { calls: records.length, bytes: records.reduce((sum, item) => sum + item.bytes, 0), receivedBytes: records.reduce((sum, item) => sum + (item.receivedBytes ?? 0), 0), milliseconds: summarizeDistribution(records.flatMap(item => item.finished === undefined ? [] : [item.finished - item.started])), timings: Object.fromEntries([...new Set(records.flatMap(item => Object.keys(item.timings ?? {})))].map(key => [key, summarizeDistribution(records.flatMap(item => typeof item.timings?.[key] === "number" ? [item.timings[key]!] : []))])) }]
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
    elapsedMilliseconds: Number(measurement.elapsed.toFixed(3)), readyMilliseconds, totalWallMilliseconds: Date.now() - wallStarted,
    animationCallbacks: measurement.animationCallbacks, completedFrames: actualFrames, applicationCompletedFramesPerSecond: Number((actualFrames / measurement.elapsed * 1000).toFixed(3)),
    compositor: summarizeCompositorTruth(traceEvents, measurement.elapsed, clockBefore !== undefined && clockAfter !== undefined ? { startedMicroseconds: clockBefore * 1_000_000, endedMicroseconds: clockAfter * 1_000_000 } : undefined),
    browser: { platform: process.platform, origin: new URL(page.url()).origin, channel: process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? "playwright-chromium", viewport: measurement.viewport, visible: measurement.visible, focused: measurement.focused, gpu: system?.gpu ?? null, processes: { before: processBefore?.processInfo ?? null, after: processAfter?.processInfo ?? null, residentBefore, residentAfter }, network, storage, userMachineEvidence: false },
    frameIntervals: summarizeFrameTimes(intervals), frameWork: summarizeFrameTimes(completed.map(frame => frame.detail.total)),
    simulation: { ticks: measurement.lastTick - measurement.firstTick, hertz: Number(((measurement.lastTick - measurement.firstTick) / measurement.elapsed * 1000).toFixed(3)) },
    botWork: summarizeDistribution(completed.map(frame => frame.detail.models)), worker,
    gpu: { ...measurement.counters, submissionsPerCompletedFrame: Number((measurement.counters.submissions / Math.max(1, actualFrames)).toFixed(3)) },
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
    browser: { ...report.browser, gpu: { devices: report.browser.gpu?.devices ?? null, featureStatus: report.browser.gpu?.featureStatus ?? null }, storage: { ...report.browser.storage, navigation: undefined } },
    frameIntervals: report.frameIntervals, frameWork: report.frameWork, simulation: report.simulation,
    worker: Object.fromEntries(Object.entries(worker).map(([kind, value]) => [kind, {
      calls: value.calls, maximumMilliseconds: value.milliseconds.max,
      transactMaximumMilliseconds: value.timings.transactMilliseconds?.max ?? 0,
    }])),
    gpuSubmissionsPerCompletedFrame: report.gpu.submissionsPerCompletedFrame,
    memory: report.memory, readyMilliseconds, totalWallMilliseconds: report.totalWallMilliseconds,
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
