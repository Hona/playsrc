import { mkdir, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import type { CDPSession } from "@playwright/test"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { metricDelta, summarizeCpuProfile, summarizeDistribution, summarizeTrace, type CpuProfile, type TraceEvent } from "./gameui-profile"
import { divideProfileWindow, profileSampleSeconds } from "./profile-window"

const TARGET = "jump_beef"
const [MENU_SAMPLE_SECONDS, OPTIONS_SAMPLE_SECONDS] = divideProfileWindow(profileSampleSeconds(), 2)
const SAMPLE_MILLISECONDS = MENU_SAMPLE_SECONDS! * 1_000
const OPTIONS_SAMPLE_MILLISECONDS = OPTIONS_SAMPLE_SECONDS! * 1_000
const MAX_CDP_TRACE_BYTES = 512 * 1024 * 1024
const TRACE_CATEGORIES = "devtools.timeline,disabled-by-default-devtools.timeline,v8,blink.user_timing,loading,toplevel"

async function readCdpStream(client: CDPSession, handle: string): Promise<string> {
  let output = ""
  while (true) {
    const chunk = await client.send("IO.read", { handle, size: 4 * 1024 * 1024 }) as { data: string; base64Encoded?: boolean; eof?: boolean }
    output += chunk.base64Encoded ? Buffer.from(chunk.data, "base64").toString("utf8") : chunk.data
    if (Buffer.byteLength(output) > MAX_CDP_TRACE_BYTES) throw new Error("CDP trace exceeded 512 MiB")
    if (chunk.eof) break
  }
  await client.send("IO.close", { handle })
  return output
}

async function startCdpCapture(client: CDPSession) {
  const metrics = (await client.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }).metrics
  const traceComplete = new Promise<{ stream: string }>((resolve) => client.once("Tracing.tracingComplete", resolve))
  await client.send("Tracing.start", { categories: TRACE_CATEGORIES, options: "record-continuously", transferMode: "ReturnAsStream" })
  await client.send("Profiler.start")
  return { metrics, traceComplete }
}

async function stopCdpCapture(client: CDPSession, started: Awaited<ReturnType<typeof startCdpCapture>>) {
  const cpuProfile = (await client.send("Profiler.stop") as { profile: CpuProfile }).profile
  const metrics = (await client.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }).metrics
  await client.send("Tracing.end")
  const trace = await started.traceComplete
  const traceText = await readCdpStream(client, trace.stream)
  const traceEvents = (JSON.parse(traceText) as { traceEvents?: TraceEvent[] }).traceEvents ?? []
  return { cpuProfile, startMetrics: started.metrics, metrics, traceText, traceEvents }
}

test("profile TF2 Main Menu startup and steady state", async ({ page, context, browserName }) => {
  const local = await loadLocalConfig()
  const outputDirectory = path.join(local.sourceCacheDir, "profiles", "gameui", TARGET)
  await mkdir(outputDirectory, { recursive: true })
  for (const name of ["report.json", "playwright-trace.zip", "startup-cdp-trace.json", "steady-cdp-trace.json", "options-open-cdp-trace.json", "options-steady-cdp-trace.json", "startup-cpu-profile.cpuprofile", "steady-cpu-profile.cpuprofile", "options-open-cpu-profile.cpuprofile", "options-steady-cpu-profile.cpuprofile", "main-menu.png", "options.png", "loading-presentation.png", "loading-presentation.json"]) {
    await rm(path.join(outputDirectory, name), { force: true })
  }

  await page.addInitScript(() => {
    const limit = 20_000
    const boundedPush = <T>(values: T[], value: T) => {
      if (values.length < limit) values.push(value)
    }
    const cardinalities = new WeakMap<object, number>()
    const targetIdentities = new WeakMap<Element, string>()
    const counts = (values: Record<string, number>, key: string) => {
      if (!(key in values)) {
        const cardinality = cardinalities.get(values) ?? 0
        if (cardinality >= 2_048) return
        cardinalities.set(values, cardinality + 1)
      }
      values[key] = (values[key] ?? 0) + 1
    }
    const targetIdentity = (target: Node): string => {
      const element = target instanceof Element ? target : target.parentElement
      if (!element) return target.nodeName
      const prior = targetIdentities.get(element)
      if (prior) return prior
      const runtime = element.getAttribute("data-vgui-runtime") ?? element.closest("[data-vgui-runtime]")?.getAttribute("data-vgui-runtime") ?? "none"
      const control = element.getAttribute("data-vgui-control") ?? element.getAttribute("data-vgui-raster") ?? element.className
      const identity = `${runtime}:${element.tagName.toLowerCase()}:${String(control).slice(0, 120)}`
      targetIdentities.set(element, identity)
      return identity
    }
    const state = {
      createdMilliseconds: performance.now(),
      rafIntervals: [] as { at: number; duration: number }[],
      rafCallbackDurations: [] as { at: number; duration: number }[],
      longTasks: [] as { start: number; duration: number }[],
      eventLoopLags: [] as { at: number; duration: number }[],
      memory: [] as { at: number; used: number; total: number }[],
      dom: [] as { at: number; nodes: number; gameUiNodes: number; visibleGameUiNodes: number }[],
      phases: [] as { at: number; phase: string; detail: string }[],
      mutations: [] as { at: number; records: number; addedNodes: number; removedNodes: number; attributes: number; characterData: number }[],
      mutationAttributes: {} as Record<string, number>,
      mutationTargets: {} as Record<string, number>,
      mutationBatches: 0,
      mutationRecords: 0,
      addedNodes: 0,
      removedNodes: 0,
      attributeMutations: 0,
      characterDataMutations: 0,
      droppedSamples: 0,
    }
    ;(window as any).__playsrcGameUiProfile = state

    const nativeRequestAnimationFrame = window.requestAnimationFrame.bind(window)
    let priorFrame = performance.now()
    const monitor = (now: number) => {
      boundedPush(state.rafIntervals, { at: now, duration: now - priorFrame })
      priorFrame = now
      nativeRequestAnimationFrame(monitor)
    }
    nativeRequestAnimationFrame(monitor)
    Object.defineProperty(window, "requestAnimationFrame", {
      configurable: true,
      value: (callback: FrameRequestCallback) => nativeRequestAnimationFrame((now) => {
        const started = performance.now()
        try { callback(now) }
        finally { boundedPush(state.rafCallbackDurations, { at: started, duration: performance.now() - started }) }
      }),
    })

    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (state.longTasks.length < limit) state.longTasks.push({ start: entry.startTime, duration: entry.duration })
          else state.droppedSamples += 1
        }
      }).observe({ type: "longtask", buffered: true })
    } catch {}

    addEventListener("DOMContentLoaded", () => {
      new MutationObserver((records) => {
        const sample = { at: performance.now(), records: records.length, addedNodes: 0, removedNodes: 0, attributes: 0, characterData: 0 }
        state.mutationBatches += 1
        state.mutationRecords += records.length
        for (const record of records) {
          counts(state.mutationTargets, targetIdentity(record.target))
          if (record.type === "attributes") {
            state.attributeMutations += 1
            sample.attributes += 1
            counts(state.mutationAttributes, record.attributeName ?? "(unknown)")
          }
          else if (record.type === "characterData") {
            state.characterDataMutations += 1
            sample.characterData += 1
          }
          else {
            state.addedNodes += record.addedNodes.length
            state.removedNodes += record.removedNodes.length
            sample.addedNodes += record.addedNodes.length
            sample.removedNodes += record.removedNodes.length
          }
        }
        boundedPush(state.mutations, sample)
      }).observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true })
    }, { once: true })

    let expected = performance.now() + 20
    let priorPhase = ""
    setInterval(() => {
      const now = performance.now()
      boundedPush(state.eventLoopLags, { at: now, duration: Math.max(0, now - expected) })
      expected = now + 20
      const memory = (performance as any).memory
      if (memory && state.memory.length < 2_000) state.memory.push({ at: now, used: memory.usedJSHeapSize, total: memory.totalJSHeapSize })
      const main = document.querySelector<HTMLElement>("main")
      const gameUi = document.querySelector<HTMLElement>(".gameui-layer")
      if (state.dom.length < 2_000) {
        const visible = gameUi ? [...gameUi.querySelectorAll<HTMLElement>("*")].filter((element) => {
          const style = getComputedStyle(element)
          return style.display !== "none" && style.visibility !== "hidden"
        }).length : 0
        state.dom.push({ at: now, nodes: document.querySelectorAll("*").length, gameUiNodes: gameUi?.querySelectorAll("*").length ?? 0, visibleGameUiNodes: visible })
      }
      const phase = main?.dataset.phase ?? "Absent"
      if (phase !== priorPhase) {
        state.phases.push({ at: now, phase, detail: main?.dataset.detail ?? "" })
        priorPhase = phase
      }
    }, 20)
  })

  const cdp = await context.newCDPSession(page)
  await cdp.send("Performance.enable", { timeDomain: "timeTicks" })
  await cdp.send("Profiler.enable")
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true, title: "TF2 Main Menu performance" })

  const wallStarted = Date.now()
  const startupCapture = await startCdpCapture(cdp)
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "MainMenu" || main?.dataset.phase === "Failed"
      || ["Playing", "AwaitingGesture"].includes(main?.dataset.startupState ?? "")
  }, undefined, { timeout: 180_000, polling: 50 })
  if (await page.locator("main").getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "MainMenu" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  const mainMenuMilliseconds = await page.evaluate(() => performance.now())
  const startupWallMilliseconds = Date.now() - wallStarted
  const startup = await stopCdpCapture(cdp, startupCapture)
  await writeFile(path.join(outputDirectory, "startup-cdp-trace.json"), startup.traceText)
  await writeFile(path.join(outputDirectory, "startup-cpu-profile.cpuprofile"), JSON.stringify(startup.cpuProfile))

  const steadyCapture = await startCdpCapture(cdp)
  const steadyStartedMilliseconds = await page.evaluate(() => performance.now())
  await page.waitForTimeout(SAMPLE_MILLISECONDS)
  const steadyFinishedMilliseconds = await page.evaluate(() => performance.now())
  const steady = await stopCdpCapture(cdp, steadyCapture)
  await writeFile(path.join(outputDirectory, "steady-cdp-trace.json"), steady.traceText)
  await writeFile(path.join(outputDirectory, "steady-cpu-profile.cpuprofile"), JSON.stringify(steady.cpuProfile))
  await page.screenshot({ path: path.join(outputDirectory, "main-menu.png") })
  for (const name of [
    "EventPromo", "FriendsContainer", "ShowPromoCodesButton", "CharacterSetupButton", "GeneralStoreButton",
    "Notifications_ShowButtonPanel", "MOTD_ShowButtonPanel", "WatchStreamButton", "QuestLogButton",
    "NoGCMessage", "NoGCImage", "RankBorder", "SettingsButtonSDK", "TF2SettingsButtonSDK",
  ]) await expect(page.locator(`[data-vgui-name="${name}"]`)).toBeHidden()
  await expect(page.locator('[data-vgui-name="SettingsButton"]')).toBeVisible()
  await expect(page.locator('[data-vgui-name="TF2SettingsButton"]')).toBeVisible()

  const optionsStartedMilliseconds = await page.evaluate(() => performance.now())
  const optionsOpenCapture = await startCdpCapture(cdp)
  await page.click("[data-vgui-name='SettingsButton']")
  await page.waitForFunction(() => document.querySelector("main")?.getAttribute("data-options-visible") === "true", undefined, { timeout: 180_000, polling: 50 })
  const optionsVisibleMilliseconds = await page.evaluate(() => performance.now())
  const optionsOpen = await stopCdpCapture(cdp, optionsOpenCapture)
  await writeFile(path.join(outputDirectory, "options-open-cdp-trace.json"), optionsOpen.traceText)
  await writeFile(path.join(outputDirectory, "options-open-cpu-profile.cpuprofile"), JSON.stringify(optionsOpen.cpuProfile))
  const optionsSteadyCapture = await startCdpCapture(cdp)
  const optionsSteadyStartedMilliseconds = await page.evaluate(() => performance.now())
  await page.waitForTimeout(OPTIONS_SAMPLE_MILLISECONDS)
  const optionsSteadyFinishedMilliseconds = await page.evaluate(() => performance.now())
  const optionsSteady = await stopCdpCapture(cdp, optionsSteadyCapture)
  await writeFile(path.join(outputDirectory, "options-steady-cdp-trace.json"), optionsSteady.traceText)
  await writeFile(path.join(outputDirectory, "options-steady-cpu-profile.cpuprofile"), JSON.stringify(optionsSteady.cpuProfile))
  await page.screenshot({ path: path.join(outputDirectory, "options.png") })
  const wallMilliseconds = Date.now() - wallStarted

  const raw = await page.evaluate(() => {
    const state = (window as any).__playsrcGameUiProfile
    const main = document.querySelector<HTMLElement>("main")
    const resources = performance.getEntriesByType("resource").map((entry) => {
      const value = entry as PerformanceResourceTiming
      return { name: value.name, initiatorType: value.initiatorType, duration: value.duration, transferSize: value.transferSize, decodedBodySize: value.decodedBodySize }
    })
    return {
      ...state,
      finishedMilliseconds: performance.now(),
      terminal: main ? { phase: main.dataset.phase, detail: main.dataset.detail, gameUi: main.dataset.gameui, startupState: main.dataset.startupState, blockers: JSON.parse(main.dataset.blockers ?? "[]") } : null,
      runtimeFrameWork: [...document.querySelectorAll<HTMLElement>("[data-vgui-runtime]")].map((element) => ({ runtime: element.dataset.vguiRuntime, frameWork: element.dataset.vguiFrameWork ?? "unreported" })),
      resources,
      navigation: performance.getEntriesByType("navigation").map((entry) => {
        const value = entry as PerformanceNavigationTiming
        return { duration: value.duration, responseEnd: value.responseEnd, domInteractive: value.domInteractive, domContentLoaded: value.domContentLoadedEventEnd, loadEventEnd: value.loadEventEnd }
      }),
    }
  })

  await context.tracing.stop({ path: path.join(outputDirectory, "playwright-trace.zip") })
  const domFinal = raw.dom.at(-1) ?? { nodes: 0, gameUiNodes: 0, visibleGameUiNodes: 0 }
  const memoryUsed = raw.memory.map((sample: { used: number }) => sample.used / (1024 * 1024))
  const resources = raw.resources.toSorted((left: { duration: number }, right: { duration: number }) => right.duration - left.duration)
  const topCounts = (values: Record<string, number>) => Object.freeze(Object.entries(values)
    .map(([identity, count]) => Object.freeze({ identity, count }))
    .sort((left, right) => right.count - left.count)
    .slice(0, 100))
  const windowReport = (
    start: number,
    end: number,
    capture: Awaited<ReturnType<typeof stopCdpCapture>>,
  ) => {
    const within = <T extends { at: number }>(values: readonly T[]) => values.filter((value) => value.at >= start && value.at <= end)
    const intervals = within(raw.rafIntervals)
    const callbacks = within(raw.rafCallbackDurations)
    const longTasks = raw.longTasks.filter((value: { start: number }) => value.start >= start && value.start <= end)
    const lags = within(raw.eventLoopLags)
    const mutations = within(raw.mutations)
    const memory = within(raw.memory)
    const dom = within(raw.dom)
    const mutationTotals = mutations.reduce((total: { records: number; addedNodes: number; removedNodes: number; attributes: number; characterData: number }, value: { records: number; addedNodes: number; removedNodes: number; attributes: number; characterData: number }) => ({
      records: total.records + value.records,
      addedNodes: total.addedNodes + value.addedNodes,
      removedNodes: total.removedNodes + value.removedNodes,
      attributes: total.attributes + value.attributes,
      characterData: total.characterData + value.characterData,
    }), { records: 0, addedNodes: 0, removedNodes: 0, attributes: 0, characterData: 0 })
    return Object.freeze({
      startMilliseconds: Number(start.toFixed(3)),
      endMilliseconds: Number(end.toFixed(3)),
      durationMilliseconds: Number((end - start).toFixed(3)),
      mainThread: Object.freeze({
        rafIntervalsMilliseconds: summarizeDistribution(intervals.map((value: { duration: number }) => value.duration)),
        rafCallbackMilliseconds: summarizeDistribution(callbacks.map((value: { duration: number }) => value.duration)),
        longTaskMilliseconds: summarizeDistribution(longTasks.map((value: { duration: number }) => value.duration)),
        eventLoopLagMilliseconds: summarizeDistribution(lags.map((value: { duration: number }) => value.duration)),
        estimatedRafHz: Number((intervals.length * 1_000 / Math.max(1, end - start)).toFixed(3)),
      }),
      dom: Object.freeze({
        final: dom.at(-1) ?? null,
        mutationBatches: mutations.length,
        ...mutationTotals,
      }),
      memory: Object.freeze({
        usedHeapMiB: summarizeDistribution(memory.map((value: { used: number }) => value.used / (1024 * 1024))),
        totalHeapMiB: summarizeDistribution(memory.map((value: { total: number }) => value.total / (1024 * 1024))),
      }),
      cdpMetrics: metricDelta(capture.startMetrics, capture.metrics),
      cpu: summarizeCpuProfile(capture.cpuProfile),
      trace: summarizeTrace(capture.traceEvents),
    })
  }
  const startupReport = windowReport(raw.createdMilliseconds, mainMenuMilliseconds, startup)
  const steadyReport = windowReport(steadyStartedMilliseconds, steadyFinishedMilliseconds, steady)
  const optionsOpenReport = windowReport(optionsStartedMilliseconds, optionsVisibleMilliseconds, optionsOpen)
  const optionsSteadyReport = windowReport(optionsSteadyStartedMilliseconds, optionsSteadyFinishedMilliseconds, optionsSteady)
  const sustainedWindows = [steadyReport, optionsSteadyReport]
  const frameCallbackP95 = Math.max(...sustainedWindows.map((value) => value.mainThread.rafCallbackMilliseconds.p95))
  const frameCallbackMax = Math.max(...sustainedWindows.map((value) => value.mainThread.rafCallbackMilliseconds.max))
  const report = Object.freeze({
    schema: "playsrc-gameui-profile-v2",
    target: TARGET,
    browserName,
    browserVersion: await page.evaluate(() => navigator.userAgent),
    wallMilliseconds,
    startupWallMilliseconds,
    terminal: raw.terminal,
    runtimeFrameWork: raw.runtimeFrameWork,
    phases: raw.phases,
    budget: Object.freeze({
      maximumFrameCallbackMilliseconds: 5,
      observedP95Milliseconds: frameCallbackP95,
      observedMaximumMilliseconds: frameCallbackMax,
      transientMaximumMilliseconds: Math.max(startupReport.mainThread.rafCallbackMilliseconds.max, optionsOpenReport.mainThread.rafCallbackMilliseconds.max),
      passed: frameCallbackMax < 5,
    }),
    startup: startupReport,
    steady: steadyReport,
    optionsOpen: optionsOpenReport,
    optionsSteady: optionsSteadyReport,
    dom: Object.freeze({
      finalNodes: domFinal.nodes,
      finalGameUiNodes: domFinal.gameUiNodes,
      finalVisibleGameUiNodes: domFinal.visibleGameUiNodes,
      samples: raw.dom,
      mutationBatches: raw.mutationBatches,
      mutationRecords: raw.mutationRecords,
      addedNodes: raw.addedNodes,
      removedNodes: raw.removedNodes,
      attributeMutations: raw.attributeMutations,
      characterDataMutations: raw.characterDataMutations,
      topAttributes: topCounts(raw.mutationAttributes),
      topTargets: topCounts(raw.mutationTargets),
    }),
    memory: Object.freeze({ usedHeapMiB: summarizeDistribution(memoryUsed), samples: raw.memory }),
    network: Object.freeze({
      requests: raw.resources.length,
      transferBytes: raw.resources.reduce((sum: number, resource: { transferSize: number }) => sum + resource.transferSize, 0),
      decodedBytes: raw.resources.reduce((sum: number, resource: { decodedBodySize: number }) => sum + resource.decodedBodySize, 0),
      topDuration: resources.slice(0, 30),
      navigation: raw.navigation,
    }),
    instrumentation: Object.freeze({ droppedSamples: raw.droppedSamples, maximumRetainedSamplesPerFamily: 20_000, cpuSamplingIntervalMicroseconds: 1_000 }),
    artifacts: Object.freeze({
      report: "report.json",
      playwrightTrace: "playwright-trace.zip",
      startupCdpTrace: "startup-cdp-trace.json",
      steadyCdpTrace: "steady-cdp-trace.json",
      optionsOpenCdpTrace: "options-open-cdp-trace.json",
      optionsSteadyCdpTrace: "options-steady-cdp-trace.json",
      startupCpuProfile: "startup-cpu-profile.cpuprofile",
      steadyCpuProfile: "steady-cpu-profile.cpuprofile",
      optionsOpenCpuProfile: "options-open-cpu-profile.cpuprofile",
      optionsSteadyCpuProfile: "options-steady-cpu-profile.cpuprofile",
      mainMenuScreenshot: "main-menu.png",
      optionsScreenshot: "options.png",
      loadingScreenshot: "loading-presentation.png",
      loadingEvidence: "loading-presentation.json",
    }),
  })
  await writeFile(path.join(outputDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  const compact = (value: typeof report.startup) => ({
    milliseconds: value.durationMilliseconds,
    frames: value.mainThread.rafIntervalsMilliseconds.count,
    frameP95Milliseconds: value.mainThread.rafIntervalsMilliseconds.p95,
    callbackP95Milliseconds: value.mainThread.rafCallbackMilliseconds.p95,
    mutations: value.dom.records,
    heapMiB: value.memory.usedHeapMiB.p95,
  })
  console.log(`PLAYSRCGAMEUIPROFILE ${JSON.stringify({
    target: report.target,
    sampleSeconds: profileSampleSeconds(),
    startupSkipped: report.terminal?.startupState === "Skipped",
    startup: compact(report.startup),
    menu: compact(report.steady),
    optionsOpen: compact(report.optionsOpen),
    options: compact(report.optionsSteady),
    budget: report.budget,
    nodes: { total: report.dom.finalNodes, menu: report.dom.finalGameUiNodes },
    report: path.join(outputDirectory, "report.json"),
  })}`)

  expect(report.terminal?.phase).toBe("MainMenu")
  expect(report.terminal?.startupState).toBe("Skipped")
  expect(report.steady.durationMilliseconds + report.optionsSteady.durationMilliseconds).toBeGreaterThanOrEqual(profileSampleSeconds() * 1_000)
  expect(report.startup.cpu.sampleCount).toBeGreaterThan(0)
  expect(report.steady.cpu.sampleCount).toBeGreaterThan(0)
  expect(report.startup.trace.eventCount).toBeGreaterThan(0)
  expect(report.steady.trace.eventCount).toBeGreaterThan(0)
  expect(report.optionsOpen.cpu.sampleCount).toBeGreaterThan(0)
  expect(report.optionsSteady.cpu.sampleCount).toBeGreaterThan(0)
  expect(report.optionsOpen.trace.eventCount).toBeGreaterThan(0)
  expect(report.optionsSteady.trace.eventCount).toBeGreaterThan(0)
  expect(report.dom.finalGameUiNodes).toBeGreaterThan(0)

  await page.keyboard.press("Escape")
  await expect(page.locator("main")).toHaveAttribute("data-options-visible", "false")
  const configuration = await (await page.request.get("/playsrc-config.json")).json() as {
    targets: ReadonlyArray<{ target: string; objects: { bsp: { sha256: string } } }>
  }
  const upward = configuration.targets.find((target) => target.target === "pl_upward")
  if (!upward) throw new Error("Configured Upward map is missing")
  let releaseMap: (() => void) | undefined
  const withheldMap = new Promise<void>((resolve) => { releaseMap = resolve })
  await page.route(`**/objects/sha256/${upward.objects.bsp.sha256}`, async (route) => {
    await withheldMap
    await route.abort().catch((error) => {
      if (!(error instanceof Error) || !error.message.includes("Route is already handled")) throw error
    })
  })
  try {
    await page.keyboard.press("Backquote")
    await page.getByLabel("Console command").fill("map pl_upward")
    await page.getByLabel("Console command").press("Enter")
    await expect(page.locator("main")).toHaveAttribute("data-gameui", "loading", { timeout: 30_000 })
    await expect(page.locator('.loading-layer [data-vgui-name="OnYourWayLabel"]')).toHaveText("You're on your way to:")
    await expect(page.locator('.loading-layer [data-vgui-name="MapLabel"]')).toHaveText("Upward")
    await expect(page.locator('.loading-layer [data-vgui-name="MapType"]')).toHaveText("Payload")
    const mapImage = page.locator('.loading-layer [data-vgui-name="MapImage"]')
    await expect(mapImage).toBeVisible()
    await expect(page.locator("main")).toHaveAttribute("data-loading-background", "map-photo")
    const imageBounds = await mapImage.boundingBox()
    if (!imageBounds) throw new Error("Upward map photograph has no visible bounds")
    const screenshot = await page.screenshot({ path: path.join(outputDirectory, "loading-presentation.png") })
    const pixels = await page.evaluate(async ({ encoded, bounds }) => {
      const bytes = Uint8Array.from(atob(encoded), (value) => value.charCodeAt(0))
      const image = await createImageBitmap(new Blob([bytes], { type: "image/png" }))
      const canvas = new OffscreenCanvas(image.width, image.height)
      const context = canvas.getContext("2d")
      if (!context) throw new Error("Loading screenshot pixel context is unavailable")
      context.drawImage(image, 0, 0)
      return [0.25, 0.5, 0.75].map((fraction) => {
        const x = Math.floor(bounds.x + bounds.width * fraction)
        const y = Math.floor(bounds.y + bounds.height * fraction)
        return { x, y, rgba: [...context.getImageData(x, y, 1, 1).data] }
      })
    }, { encoded: screenshot.toString("base64"), bounds: imageBounds })
    expect(new Set(pixels.map((pixel) => pixel.rgba.slice(0, 3).join(","))).size).toBeGreaterThan(1)
    const evidence = { map: "pl_upward", displayName: "Upward", type: "Payload", imageBounds, pixels }
    await writeFile(path.join(outputDirectory, "loading-presentation.json"), `${JSON.stringify(evidence, null, 2)}\n`)
    console.log(`PLAYSRCLOADINGPRESENTATION ${JSON.stringify(evidence)}`)
  } finally {
    releaseMap?.()
    await page.unroute(`**/objects/sha256/${upward.objects.bsp.sha256}`)
  }
  expect(report.budget.observedMaximumMilliseconds).toBeLessThan(5)
})
