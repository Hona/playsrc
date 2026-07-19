import { mkdir, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test, type CDPSession } from "@playwright/test"
import { loadLocalConfig } from "../src/config"
import { metricDelta, summarizeCpuProfile, summarizeDistribution, summarizeTrace, type CpuProfile, type TraceEvent } from "./gameui-profile"

const TARGET = "jump_beef"
const SAMPLE_MILLISECONDS = 15_000
const MAX_CDP_TRACE_BYTES = 512 * 1024 * 1024

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

test("profile TF2 Main Menu startup and steady state", async ({ page, context, browserName }) => {
  const local = await loadLocalConfig()
  const outputDirectory = path.join(local.sourceCacheDir, "profiles", "gameui", TARGET)
  await mkdir(outputDirectory, { recursive: true })
  for (const name of ["report.json", "playwright-trace.zip", "cdp-trace.json", "cpu-profile.cpuprofile", "main-menu.png"]) {
    await rm(path.join(outputDirectory, name), { force: true })
  }

  await page.addInitScript(() => {
    const limit = 20_000
    const boundedPush = (values: number[], value: number) => {
      if (values.length < limit) values.push(value)
    }
    const state = {
      createdMilliseconds: performance.now(),
      rafIntervals: [] as number[],
      rafCallbackDurations: [] as number[],
      longTasks: [] as { start: number; duration: number }[],
      eventLoopLags: [] as number[],
      memory: [] as { at: number; used: number; total: number }[],
      dom: [] as { at: number; nodes: number; gameUiNodes: number; visibleGameUiNodes: number }[],
      phases: [] as { at: number; phase: string; detail: string }[],
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
      boundedPush(state.rafIntervals, now - priorFrame)
      priorFrame = now
      nativeRequestAnimationFrame(monitor)
    }
    nativeRequestAnimationFrame(monitor)
    Object.defineProperty(window, "requestAnimationFrame", {
      configurable: true,
      value: (callback: FrameRequestCallback) => nativeRequestAnimationFrame((now) => {
        const started = performance.now()
        try { callback(now) }
        finally { boundedPush(state.rafCallbackDurations, performance.now() - started) }
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
        state.mutationBatches += 1
        state.mutationRecords += records.length
        for (const record of records) {
          if (record.type === "attributes") state.attributeMutations += 1
          else if (record.type === "characterData") state.characterDataMutations += 1
          else {
            state.addedNodes += record.addedNodes.length
            state.removedNodes += record.removedNodes.length
          }
        }
      }).observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true })
    }, { once: true })

    let expected = performance.now() + 20
    let priorPhase = ""
    setInterval(() => {
      const now = performance.now()
      boundedPush(state.eventLoopLags, Math.max(0, now - expected))
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
  const metricsBefore = (await cdp.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }).metrics
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true, title: "TF2 Main Menu performance" })
  const traceComplete = new Promise<{ stream: string }>((resolve) => cdp.once("Tracing.tracingComplete", resolve))
  await cdp.send("Tracing.start", {
    categories: "devtools.timeline,disabled-by-default-devtools.timeline,v8,blink.user_timing,loading,toplevel",
    options: "record-continuously",
    transferMode: "ReturnAsStream",
  })
  await cdp.send("Profiler.start")

  const wallStarted = Date.now()
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "MainMenu" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  await page.waitForTimeout(SAMPLE_MILLISECONDS)
  const wallMilliseconds = Date.now() - wallStarted
  await page.screenshot({ path: path.join(outputDirectory, "main-menu.png") })

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
      terminal: main ? { phase: main.dataset.phase, detail: main.dataset.detail, gameUi: main.dataset.gameui, blockers: JSON.parse(main.dataset.blockers ?? "[]") } : null,
      resources,
      navigation: performance.getEntriesByType("navigation").map((entry) => {
        const value = entry as PerformanceNavigationTiming
        return { duration: value.duration, responseEnd: value.responseEnd, domInteractive: value.domInteractive, domContentLoaded: value.domContentLoadedEventEnd, loadEventEnd: value.loadEventEnd }
      }),
    }
  })

  const cpuProfile = (await cdp.send("Profiler.stop") as { profile: CpuProfile }).profile
  const metricsAfter = (await cdp.send("Performance.getMetrics") as { metrics: { name: string; value: number }[] }).metrics
  await cdp.send("Tracing.end")
  const trace = await traceComplete
  const traceText = await readCdpStream(cdp, trace.stream)
  await context.tracing.stop({ path: path.join(outputDirectory, "playwright-trace.zip") })
  await writeFile(path.join(outputDirectory, "cdp-trace.json"), traceText)
  await writeFile(path.join(outputDirectory, "cpu-profile.cpuprofile"), JSON.stringify(cpuProfile))

  const traceEvents = (JSON.parse(traceText) as { traceEvents?: TraceEvent[] }).traceEvents ?? []
  const domFinal = raw.dom.at(-1) ?? { nodes: 0, gameUiNodes: 0, visibleGameUiNodes: 0 }
  const memoryUsed = raw.memory.map((sample: { used: number }) => sample.used / (1024 * 1024))
  const resources = raw.resources.toSorted((left: { duration: number }, right: { duration: number }) => right.duration - left.duration)
  const frameCallbacks = summarizeDistribution(raw.rafCallbackDurations)
  const report = Object.freeze({
    schema: "playsrc-gameui-profile-v1",
    target: TARGET,
    browserName,
    browserVersion: await page.evaluate(() => navigator.userAgent),
    wallMilliseconds,
    terminal: raw.terminal,
    phases: raw.phases,
    budget: Object.freeze({
      maximumFrameCallbackMilliseconds: 5,
      observedP95Milliseconds: frameCallbacks.p95,
      observedMaximumMilliseconds: frameCallbacks.max,
      passed: frameCallbacks.max < 5,
    }),
    mainThread: Object.freeze({
      rafIntervalsMilliseconds: summarizeDistribution(raw.rafIntervals),
      rafCallbackMilliseconds: frameCallbacks,
      longTaskMilliseconds: summarizeDistribution(raw.longTasks.map((entry: { duration: number }) => entry.duration)),
      eventLoopLagMilliseconds: summarizeDistribution(raw.eventLoopLags),
      estimatedRafHz: Number((raw.rafIntervals.length * 1_000 / Math.max(1, raw.finishedMilliseconds - raw.createdMilliseconds)).toFixed(3)),
      droppedSamples: raw.droppedSamples,
    }),
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
    }),
    memory: Object.freeze({ usedHeapMiB: summarizeDistribution(memoryUsed), samples: raw.memory }),
    network: Object.freeze({
      requests: raw.resources.length,
      transferBytes: raw.resources.reduce((sum: number, resource: { transferSize: number }) => sum + resource.transferSize, 0),
      decodedBytes: raw.resources.reduce((sum: number, resource: { decodedBodySize: number }) => sum + resource.decodedBodySize, 0),
      topDuration: resources.slice(0, 30),
      navigation: raw.navigation,
    }),
    cdpMetrics: metricDelta(metricsBefore, metricsAfter),
    cpu: summarizeCpuProfile(cpuProfile),
    trace: summarizeTrace(traceEvents),
    artifacts: Object.freeze({
      report: "report.json",
      playwrightTrace: "playwright-trace.zip",
      cdpTrace: "cdp-trace.json",
      cpuProfile: "cpu-profile.cpuprofile",
      screenshot: "main-menu.png",
    }),
  })
  await writeFile(path.join(outputDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`PLAYSRCGAMEUIPROFILE ${JSON.stringify({ outputDirectory, terminal: report.terminal ? { phase: report.terminal.phase, detail: report.terminal.detail, gameUi: report.terminal.gameUi, blockerCount: report.terminal.blockers.length } : null, budget: report.budget, mainThread: report.mainThread, dom: { finalNodes: report.dom.finalNodes, finalGameUiNodes: report.dom.finalGameUiNodes, mutationRecords: report.dom.mutationRecords }, cdpMetrics: report.cdpMetrics, topCpuSelf: report.cpu.topSelf.slice(0, 15), topTraceCategories: report.trace.categories.slice(0, 15) })}`)

  expect(report.terminal?.phase).toBe("MainMenu")
  expect(report.cpu.sampleCount).toBeGreaterThan(0)
  expect(report.trace.eventCount).toBeGreaterThan(0)
  expect(report.dom.finalGameUiNodes).toBeGreaterThan(0)
  expect(report.budget.observedMaximumMilliseconds).toBeLessThan(5)
})
