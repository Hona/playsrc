import { expect, test } from "@playwright/test"

type RpcRecord = { kind: string; started: number; finished?: number; bytes?: number; workerTimings?: Record<string, number> }
type PresentationRecord = { at: number; detail: string; performance: string | undefined }

const percentile = (values: number[], fraction: number): number =>
  values.length === 0 ? 0 : values[Math.min(values.length - 1, Math.floor(values.length * fraction))]!

test("profile startup and input latency", async ({ page }) => {
  const rafHz = Number(process.env.PROFILE_RAF_HZ ?? 0)
  const scenarioMode = process.env.PROFILE_SCENARIOS ?? ""
  const runScenarios = scenarioMode !== ""
  const shouldRunScenario = (name: string) => scenarioMode === "1" || scenarioMode === name
  await page.addInitScript(({ rafHz }) => {
    if (rafHz > 0) {
      const interval = 1_000 / rafHz
      Object.defineProperty(window, "requestAnimationFrame", {
        configurable: true,
        value: (callback: FrameRequestCallback) => window.setTimeout(() => callback(performance.now()), interval),
      })
      Object.defineProperty(window, "cancelAnimationFrame", {
        configurable: true,
        value: (handle: number) => clearTimeout(handle),
      })
    }
    const state = {
      created: performance.now(),
      rpcs: [] as RpcRecord[],
      phases: [] as { at: number; phase: string; frames: number }[],
      longTasks: [] as { at: number; duration: number }[],
      rafGaps: [] as { at: number; duration: number }[],
      presentations: [] as PresentationRecord[],
      frames: 0,
    }
    ;(window as any).__playsrcProfile = state

    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, RpcRecord>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const id = event.data?.id
          if (Number.isSafeInteger(id)) {
            const record = this.records.get(id)
            if (record) {
              record.finished = performance.now()
              if (event.data?.timings) record.workerTimings = event.data.timings
            }
          }
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const record: RpcRecord = {
            kind: message.kind,
            started: performance.now(),
            bytes: message.command?.byteLength ?? message.batch?.byteLength ?? message.bsp?.byteLength,
          }
          this.records.set(message.id, record)
          state.rpcs.push(record)
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) state.longTasks.push({ at: entry.startTime, duration: entry.duration })
      }).observe({ entryTypes: ["longtask"] })
    } catch {}

    let previous = performance.now()
    const frame = (now: number) => {
      const gap = now - previous
      if (gap > 20) state.rafGaps.push({ at: now, duration: gap })
      previous = now
      state.frames += 1
      requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)

    addEventListener("DOMContentLoaded", () => {
      const main = document.querySelector("main")
      if (!main) return
      const capture = () => state.phases.push({
        at: performance.now(),
        phase: (main as HTMLElement).dataset.phase ?? "",
        frames: state.frames,
      })
      const capturePresentation = () => {
        const detail = (main as HTMLElement).dataset.performanceDetail
        if (detail) state.presentations.push({
          at: performance.now(),
          detail,
          performance: (main as HTMLElement).dataset.performance,
        })
      }
      capture()
      new MutationObserver((records) => {
        if (records.some((record) => record.attributeName === "data-phase")) capture()
        if (records.some((record) => record.attributeName === "data-performance-detail")) capturePresentation()
      }).observe(main, { attributes: true, attributeFilter: ["data-phase", "data-performance-detail"] })
    })
  }, { rafHz })

  const wallStarted = Date.now()
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "Ready" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  const startupMilliseconds = Date.now() - wallStarted

  const initial = await page.locator("main").evaluate((main) => ({ ...((main as HTMLElement).dataset) }))
  const input: Record<string, unknown> = {}
  if (initial.phase === "Ready") {
    await page.waitForTimeout(5_000)
    const settledPhase = await page.locator("main").getAttribute("data-phase")
    if (settledPhase !== "Ready") {
      input.skipped = `phase became ${settledPhase} during settle`
    } else {
    const keyDownAt = await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
      return performance.now()
    })
    try {
      await page.waitForFunction(() => Number(document.querySelector("main")?.getAttribute("data-wish-speed")) > 0, undefined, {
        timeout: 30_000,
        polling: 10,
      })
      input.keyDownMilliseconds = await page.evaluate((started) => performance.now() - started, keyDownAt)
    } catch {
      input.keyDownMilliseconds = ">30000"
    }

    const keyUpAt = await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
      return performance.now()
    })
    try {
      await page.waitForFunction(() => Number(document.querySelector("main")?.getAttribute("data-wish-speed")) === 0, undefined, {
        timeout: 30_000,
        polling: 10,
      })
      input.keyUpMilliseconds = await page.evaluate((started) => performance.now() - started, keyUpAt)
    } catch {
      input.keyUpMilliseconds = ">30000"
    }

    const fireEvents = Number(await page.locator("main").getAttribute("data-fire-events"))
    try {
      await page.locator(".world-canvas").click()
      await page.waitForFunction(() => document.pointerLockElement?.classList.contains("world-canvas"), undefined, {
        timeout: 5_000,
      })
      const fireAt = await page.evaluate(() => performance.now())
      await page.mouse.down({ button: "left" })
      await page.waitForFunction((baseline) => Number(document.querySelector("main")?.getAttribute("data-fire-events")) > baseline, fireEvents, {
        timeout: 30_000,
        polling: 10,
      })
      input.fireMilliseconds = await page.evaluate((started) => performance.now() - started, fireAt)
      await page.mouse.up({ button: "left" })
      if (!runScenarios) await page.keyboard.press("Escape")
    } catch {
      input.fireMilliseconds = ">30000"
      await page.mouse.up({ button: "left" }).catch(() => {})
    }
    }
  }

  const steadyState: {
    elapsedMilliseconds: number
    phase: string | undefined
    performance: string | undefined
    tick: string | undefined
    frames: number
    rpcStarted: Record<string, number>
    rpcCompleted: Record<string, number>
  }[] = []
  const captureRuntime = () => page.locator("main").evaluate((main) => {
    const profile = (window as any).__playsrcProfile as { frames: number; rpcs: RpcRecord[] }
    const rpcCompleted: Record<string, number> = {}
    for (const rpc of profile.rpcs) {
      if (rpc.finished !== undefined) rpcCompleted[rpc.kind] = (rpcCompleted[rpc.kind] ?? 0) + 1
    }
    const dataset = (main as HTMLElement).dataset
    return {
      at: performance.now(),
      frames: profile.frames,
      rpcCompleted,
      tick: Number(dataset.snapshotTick ?? 0),
      cameraPosition: dataset.cameraPosition,
      wishSpeed: Number(dataset.wishSpeed ?? 0),
      grounded: dataset.grounded,
      sweepQueries: Number(dataset.sweepQueries ?? 0),
      pointQueries: Number(dataset.pointQueries ?? 0),
      movementContacts: Number(dataset.movementContacts ?? 0),
      movementEvents: Number(dataset.movementEvents ?? 0),
      fireEvents: Number(dataset.fireEvents ?? 0),
      particleItems: Number(dataset.particleItems ?? 0),
      performance: dataset.performance,
      performanceDetail: dataset.performanceDetail,
      phase: dataset.phase,
    }
  })
  const steadyStarted = Date.now()
  for (let second = 0; second < 10; second += 1) {
    await page.waitForTimeout(1_000)
    const sample = await page.locator("main").evaluate((main) => {
      const profile = (window as any).__playsrcProfile as { frames: number; rpcs: RpcRecord[] }
      const rpcStarted: Record<string, number> = {}
      const rpcCompleted: Record<string, number> = {}
      for (const rpc of profile.rpcs) {
        rpcStarted[rpc.kind] = (rpcStarted[rpc.kind] ?? 0) + 1
        if (rpc.finished !== undefined) rpcCompleted[rpc.kind] = (rpcCompleted[rpc.kind] ?? 0) + 1
      }
      return { ...((main as HTMLElement).dataset), frames: profile.frames, rpcStarted, rpcCompleted }
    })
    steadyState.push({
      elapsedMilliseconds: Date.now() - steadyStarted,
      phase: sample.phase,
      performance: sample.performance,
      tick: sample.snapshotTick,
      frames: sample.frames,
      rpcStarted: sample.rpcStarted,
      rpcCompleted: sample.rpcCompleted,
    })
    if (sample.phase === "Failed") break
  }

  const scenarios: {
    name: string
    samples: Awaited<ReturnType<typeof captureRuntime>>[]
  }[] = []
  const runScenario = async (name: string, seconds: number, start: () => Promise<void>, stop: () => Promise<void>) => {
    await start()
    const samples = [await captureRuntime()]
    for (let second = 0; second < seconds; second += 1) {
      await page.waitForTimeout(1_000)
      samples.push(await captureRuntime())
      if (samples.at(-1)?.phase === "Failed") break
    }
    await stop()
    scenarios.push({ name, samples })
  }
  if (runScenarios && initial.phase === "Ready") {
    if (shouldRunScenario("jump")) await runScenario("repeated-jump", 8, async () => {
      await page.evaluate(() => {
        const press = () => {
          dispatchEvent(new KeyboardEvent("keydown", { code: "Space", key: " ", bubbles: true }))
          setTimeout(() => dispatchEvent(new KeyboardEvent("keyup", { code: "Space", key: " ", bubbles: true })), 35)
        }
        press()
        ;(window as any).__playsrcJumpInterval = setInterval(press, 100)
      })
    }, async () => {
      await page.evaluate(() => {
        clearInterval((window as any).__playsrcJumpInterval)
        dispatchEvent(new KeyboardEvent("keyup", { code: "Space", key: " ", bubbles: true }))
      })
    })
    if (shouldRunScenario("wall")) await page.waitForTimeout(1_000)
    if (shouldRunScenario("wall")) await runScenario("held-forward", 15, async () => {
      await page.evaluate(() => dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true })))
    }, async () => {
      await page.evaluate(() => dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true })))
    })
    if (shouldRunScenario("fire")) await page.waitForTimeout(1_000)
    if (shouldRunScenario("fire")) await runScenario("held-primary-fire", 10, async () => {
      await page.evaluate(() => {
        const canvas = document.querySelector(".world-canvas")
        if (!canvas) throw new Error("world canvas is unavailable")
        if (document.pointerLockElement !== canvas) {
          Object.defineProperty(document, "pointerLockElement", { configurable: true, value: canvas })
        }
        dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
      })
    }, async () => {
      await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
    })
  }

  const raw = await page.evaluate(() => {
    const state = (window as any).__playsrcProfile
    return {
      now: performance.now(),
      state,
      dataset: { ...((document.querySelector("main") as HTMLElement | null)?.dataset ?? {}) },
      navigation: performance.getEntriesByType("navigation").map((entry) => ({
        duration: entry.duration,
        responseEnd: (entry as PerformanceNavigationTiming).responseEnd,
        domInteractive: (entry as PerformanceNavigationTiming).domInteractive,
        loadEventEnd: (entry as PerformanceNavigationTiming).loadEventEnd,
      })),
    }
  })

  const completed = (raw.state.rpcs as RpcRecord[]).filter((record) => record.finished !== undefined)
  const kinds = [...new Set(completed.map((record) => record.kind))].sort()
  const rpcSummary = Object.fromEntries(kinds.map((kind) => {
    const records = completed.filter((record) => record.kind === kind)
    const durations = records.map((record) => record.finished! - record.started).sort((a, b) => a - b)
    return [kind, {
      count: durations.length,
      totalMilliseconds: Number(durations.reduce((sum, value) => sum + value, 0).toFixed(3)),
      meanMilliseconds: Number((durations.reduce((sum, value) => sum + value, 0) / durations.length).toFixed(3)),
      p50Milliseconds: Number(percentile(durations, 0.5).toFixed(3)),
      p95Milliseconds: Number(percentile(durations, 0.95).toFixed(3)),
      p99Milliseconds: Number(percentile(durations, 0.99).toFixed(3)),
      maxMilliseconds: Number(durations.at(-1)!.toFixed(3)),
      maxBytes: Math.max(0, ...records.map((record) => record.bytes ?? 0)),
      workerTimings: Object.fromEntries(
        [...new Set(records.flatMap((record) => Object.keys(record.workerTimings ?? {})))].map((timing) => {
          const values = records.flatMap((record) => record.workerTimings?.[timing] ?? []).sort((a, b) => a - b)
          return [timing, {
            meanMilliseconds: Number((values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3)),
            p50Milliseconds: Number(percentile(values, 0.5).toFixed(3)),
            p95Milliseconds: Number(percentile(values, 0.95).toFixed(3)),
            maxMilliseconds: Number((values.at(-1) ?? 0).toFixed(3)),
          }]
        }),
      ),
    }]
  }))
  const rafGapRecords = raw.state.rafGaps as { at: number; duration: number }[]
  const rafGaps = rafGapRecords.map((entry) => entry.duration).sort((a, b) => a - b)
  const longTasks = (raw.state.longTasks as { duration: number }[]).map((entry) => entry.duration).sort((a, b) => a - b)
  const presentationRecords = raw.state.presentations as PresentationRecord[]
  const timingKeys = ["models", "projectiles", "visibility", "particleWorker", "particleDecode", "audio", "dynamicItems", "world", "viewmodel", "render", "total"] as const
  const presentationSummary = (started: number, finished: number) => {
    const entries = presentationRecords
      .filter((record) => record.at >= started && record.at <= finished)
      .map((record) => ({ record, detail: JSON.parse(record.detail) as Record<string, number | string> }))
    const distributions = Object.fromEntries(timingKeys.map((key) => {
      const values = entries.map((entry) => Number(entry.detail[key] ?? 0)).sort((a, b) => a - b)
      return [key, {
        p50Milliseconds: Number(percentile(values, 0.5).toFixed(3)),
        p95Milliseconds: Number(percentile(values, 0.95).toFixed(3)),
        p99Milliseconds: Number(percentile(values, 0.99).toFixed(3)),
        maxMilliseconds: Number((values.at(-1) ?? 0).toFixed(3)),
      }]
    }))
    const worst = entries
      .toSorted((left, right) => Number(right.detail.total) - Number(left.detail.total))
      .slice(0, 10)
      .map(({ record, detail }) => {
        const frameStarted = record.at - Number(detail.total)
        return {
          at: Number(record.at.toFixed(3)),
          detail,
          overlappingWorkerRpc: completed
            .filter((rpc) => rpc.started <= record.at && rpc.finished! >= frameStarted)
            .map((rpc) => ({
              kind: rpc.kind,
              started: Number(rpc.started.toFixed(3)),
              duration: Number((rpc.finished! - rpc.started).toFixed(3)),
              workerTimings: rpc.workerTimings,
            })),
          overlappingLongTasks: (raw.state.longTasks as { at: number; duration: number }[])
            .filter((task) => task.at <= record.at && task.at + task.duration >= frameStarted),
          overlappingRafGaps: rafGapRecords
            .filter((gap) => gap.at >= frameStarted && gap.at - gap.duration <= record.at),
        }
      })
    return { count: entries.length, distributions, worst }
  }
  const firstSteady = steadyState[Math.floor((steadyState.length - 1) / 2)]
  const lastSteady = steadyState.at(-1)
  const steadySeconds = firstSteady && lastSteady
    ? (lastSteady.elapsedMilliseconds - firstSteady.elapsedMilliseconds) / 1_000
    : 0
  const rate = (last: number, first: number): number =>
    steadySeconds > 0 ? Number(((last - first) / steadySeconds).toFixed(3)) : 0
  const completedRate = (kind: string): number => rate(
    lastSteady?.rpcCompleted[kind] ?? 0,
    firstSteady?.rpcCompleted[kind] ?? 0,
  )
  const report = {
    requestedRafHz: rafHz || "native",
    startupMilliseconds,
    terminalPhase: raw.dataset.phase,
    terminalDetail: raw.dataset.detail,
    input,
    steadyState,
    scenarios: scenarios.map((scenario) => ({
      name: scenario.name,
      presentationTrace: presentationSummary(scenario.samples[0]!.at, scenario.samples.at(-1)!.at),
      intervals: scenario.samples.slice(1).map((sample, index) => {
        const previous = scenario.samples[index]!
        const seconds = (sample.at - previous.at) / 1_000
        const rate = (kind: string) => Number((((sample.rpcCompleted[kind] ?? 0) - (previous.rpcCompleted[kind] ?? 0)) / seconds).toFixed(3))
        return {
          seconds: Number(seconds.toFixed(3)),
          rafCallbacksPerSecond: Number(((sample.frames - previous.frames) / seconds).toFixed(3)),
          observeCompletedPerSecond: rate("observe"),
          presentationsCompletedPerSecond: rate("models"),
          simulationTicksPerSecond: Number(((sample.tick - previous.tick) / seconds).toFixed(3)),
          tick: sample.tick,
          cameraPosition: sample.cameraPosition,
          wishSpeed: sample.wishSpeed,
          grounded: sample.grounded,
          sweepQueries: sample.sweepQueries,
          pointQueries: sample.pointQueries,
          movementContacts: sample.movementContacts,
          movementEvents: sample.movementEvents,
          fireEvents: sample.fireEvents,
          particleItems: sample.particleItems,
          performance: sample.performance,
          performanceDetail: sample.performanceDetail,
          phase: sample.phase,
        }
      }),
    })),
    runtimePerformanceProbe: raw.dataset.performance,
    snapshotTick: raw.dataset.snapshotTick,
    elapsedBrowserMilliseconds: raw.now,
    phases: raw.state.phases,
    workerRpc: rpcSummary,
    steadyRates: {
      measurementSeconds: Number(steadySeconds.toFixed(3)),
      rafCallbacksPerSecond: rate(lastSteady?.frames ?? 0, firstSteady?.frames ?? 0),
      observeStartedPerSecond: rate(
        lastSteady?.rpcStarted.observe ?? 0,
        firstSteady?.rpcStarted.observe ?? 0,
      ),
      observeCompletedPerSecond: completedRate("observe"),
      presentationsCompletedPerSecond: completedRate("models"),
      visibilityCompletedPerSecond: completedRate("visibility"),
      particlesCompletedPerSecond: completedRate("particles"),
      simulationTicksPerSecond: rate(Number(lastSteady?.tick ?? 0), Number(firstSteady?.tick ?? 0)),
    },
    mainThread: {
      frames: raw.state.frames,
      rafGapCount: rafGaps.length,
      rafGapP95Milliseconds: Number(percentile(rafGaps, 0.95).toFixed(3)),
      rafGapMaxMilliseconds: Number((rafGaps.at(-1) ?? 0).toFixed(3)),
      longTaskCount: longTasks.length,
      longTaskTotalMilliseconds: Number(longTasks.reduce((sum, value) => sum + value, 0).toFixed(3)),
      longTaskP95Milliseconds: Number(percentile(longTasks, 0.95).toFixed(3)),
      longTaskMaxMilliseconds: Number((longTasks.at(-1) ?? 0).toFixed(3)),
    },
    navigation: raw.navigation,
  }
  console.log(`PLAYSRCPROFILE ${JSON.stringify(report)}`)
  expect(raw.dataset.phase).not.toBe("Loading")
})
