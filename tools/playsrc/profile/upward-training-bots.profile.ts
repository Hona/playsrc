import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

test("profile authored headed Upward offline-practice default roster and actual completed gameplay frames", async ({ page, context }, testInfo) => {
  const wallStarted = Date.now()
  const seconds = profileSampleSeconds()
  const label = process.env.PROFILE_UPWARD_TRAINING_LABEL ?? "latest"
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "profiles", "upward-training-bots")
  await mkdir(directory, { recursive: true })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { ;(globalThis as any).__playsrcProfile = {} })
  const root = page.locator("main")
  const layer = page.locator(".local-match-layer")
  await page.goto("/")
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
  await cdp.send("Performance.enable")
  await cdp.send("Profiler.enable")
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  const heapBefore = await cdp.send("Runtime.getHeapUsage")
  await cdp.send("Profiler.start")
  await page.keyboard.down("w")
  const measurement = await page.evaluate(async (duration) => {
    const main = document.querySelector<HTMLElement>("main")!
    const surface = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const instrumentation = (globalThis as any).__playsrcFrameProfiler
    const firstTick = Number(main.dataset.snapshotTick)
    const firstFrame = Number(surface.dataset.displayFrame)
    const firstPosition = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    const started = performance.now()
    instrumentation.active = true
    try {
      while (performance.now() - started < duration * 1000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        if (main.dataset.phase !== "Ready") throw new Error(`Upward training left gameplay: ${main.dataset.phase}: ${main.dataset.detail}`)
      }
    } finally { instrumentation.active = false }
    const elapsed = performance.now() - started
    const position = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    return {
      elapsed, firstTick, lastTick: Number(main.dataset.snapshotTick), firstFrame,
      lastFrame: Number(surface.dataset.displayFrame), traveled: Math.hypot(...position.map((value, index) => value - firstPosition[index]!)),
      roster: structuredClone((globalThis as any).__playsrcProfile.bots), scoreboard: JSON.parse(main.dataset.scoreboardProbe ?? "{}"),
      frames: instrumentation.completedFrames, worker: instrumentation.worker, counters: instrumentation.counters,
      longTasks: instrumentation.longTasks.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
      longAnimationFrames: instrumentation.longAnimationFrames.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
    }
  }, seconds)
  await page.keyboard.up("w")
  const cpuProfile = (await cdp.send("Profiler.stop") as { profile: CpuProfile }).profile
  const heapAfter = await cdp.send("Runtime.getHeapUsage")
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
    completedFrames: actualFrames, presentedFramesPerSecond: Number((actualFrames / measurement.elapsed * 1000).toFixed(3)),
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
  await testInfo.attach("headed-upward-default-training-bots", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(`PLAYSRC_UPWARD_TRAINING_BOTS ${JSON.stringify({
    label, activeBots: report.activeBots, teams: report.teams,
    completedFrames: report.completedFrames, presentedFramesPerSecond: report.presentedFramesPerSecond,
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
    expect(report.presentedFramesPerSecond).toBeGreaterThanOrEqual(55)
    expect(report.simulation.hertz).toBeGreaterThanOrEqual(60)
    expect(report.frameIntervals.p95Milliseconds).toBeLessThan(35)
  }
})
