import path from "node:path"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { expect, test, guardStartupInput } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { applicationBuildIdentity } from "../src/build-identity"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { chooseTf2Team } from "./team-selection-evidence"
import { installDeliveryRpcObserver } from "./delivery-rpc"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { sustainedFreezes } from "./sustained-freezes"
import { SUSTAINED_KOTH, sustainedKothTarget, requireSustainedBudget, installSustainedObservation, summarizeSustainedWindow, sustainedRunIssues } from "./sustained-koth"
import { windowsProcessMemory } from "./windows-process-memory"
import { prepareWorkerCpuCapture } from "./worker-cpu-profiler"
import { drainTraceStream, retainCompositorEvidence, retainEvidenceBlob } from "./compositor-evidence"
import { TRACE_START, TRACE_END, summarizeCompositorTruth, chromiumPresentationEventName } from "./compositor-truth"
import { startGameplayReplayJournal, parseGameplayReplay } from "./gameplay-replay"

test("sustained natural full-roster KOTH with whole-interval delivery and late CPU evidence", async ({ page, context }) => {
  if (process.platform !== "win32") throw new Error("Sustained acceptance requires the approved native Windows local job")
  const target = sustainedKothTarget(process.env.PROFILE_MAP_TARGET)
  const setupOnly = process.env.PROFILE_KOTH_SETUP_ONLY === "1"
  const diagnostic = process.env.PROFILE_KOTH_DIAGNOSTIC === "1"
  const soakMilliseconds = diagnostic ? 0 : SUSTAINED_KOTH.soakMilliseconds
  const captureRemainder = soakMilliseconds + SUSTAINED_KOTH.sampleMilliseconds + SUSTAINED_KOTH.extractionMilliseconds
  let setupComplete = false
  let setupState: any
  let firstSimulationObserveMilliseconds: number | undefined
  const { sourceCacheDir } = await loadLocalConfig(process.cwd())
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked bounded profile runner")
  await mkdir(directory, { recursive: true })
  const deadline = Number(process.env.PLAYSRC_PROFILE_DEADLINE)
  if (!Number.isFinite(deadline)) throw new Error("Missing runner deadline")
  const fingerprint = await applicationBuildIdentity(process.cwd())
  const commit = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim()
  const native = await startupNativeReader(page, sourceCacheDir)
  const check = async () => { const observation = await native.read(); requireStartupNative(observation); return observation }
  guardStartupInput(page, async () => { await check() })
  const cdp = await context.newCDPSession(page), browser = await context.browser()!.newBrowserCDPSession()
  const memory = windowsProcessMemory(sourceCacheDir)
  const records: any[] = [], captures: any[] = [], inputPlan: any[] = []
  let started = 0, lateStarted = 0, lateEnded = 0, sampled: any, error: string | null = null
  let traceStarted = false, observerStarted = false, worker: ReturnType<typeof page.workers>[number] | undefined
  let cpu: Awaited<ReturnType<typeof prepareWorkerCpuCapture>> | undefined
  let cpuStarted = false, mainCpuStarted = false, workerCpu: any, mainCpu: any, replayArtifact: any, compositor: any
  let allocationTracking = false, allocationStart: any, allocationEnd: any
  let lateSimulation: any
  const entropyDirectory = path.join(sourceCacheDir, "profiles/sustained-koth/entropy")
  const entropyIdentity = process.env.PROFILE_SUSTAINED_ENTROPY
  let entropy: Buffer | undefined
  if (entropyIdentity) {
    if (!/^[0-9a-f]{64}$/.test(entropyIdentity)) throw new Error("Malformed entropy identity")
    entropy = await readFile(path.join(entropyDirectory, `${entropyIdentity}.bin`))
    if (createHash("sha256").update(entropy).digest("hex") !== entropyIdentity) throw new Error("Retained entropy changed")
  }
  const replay = await startGameplayReplayJournal(page, directory, "startup", 1, 2, false, entropy?.toString("hex"))
  const snapshot = () => page.evaluate(() => {
    const main = document.querySelector<HTMLElement>("main")!, profile = (globalThis as any).__playsrcProfile
    return { at: performance.now(), epoch: performance.timeOrigin + performance.now(), data: { ...main.dataset }, round: structuredClone(profile?.round),
      points: structuredClone(profile?.controlPoints), bots: structuredClone(profile?.bots), quality: structuredClone(profile?.videoQuality),
      application: structuredClone(profile?.applicationGeneration), viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio } }
  })
  const pixels = async (name: string) => {
    await check()
    const bytes = await page.screenshot({ path: path.join(directory, `${name}.png`) })
    await check()
    captures.push({ name: `${name}.png`, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), state: await snapshot() })
  }
  const pulse = async () => {
    const requestedEpoch = Date.now()
    await check()
    const key = inputPlan.length % 2 === 0 ? "a" : "d", sent = await page.evaluate(() => performance.now())
    const downStartedEpoch = Date.now()
    await page.keyboard.down(key)
    const downCompletedEpoch = Date.now()
    try { await page.waitForTimeout(150) } finally { await page.keyboard.up(key) }
    inputPlan.push({ key, sent, requestedEpoch, downStartedEpoch, downCompletedEpoch, releasedEpoch: Date.now() })
  }
  await page.addInitScript({ content: `globalThis.__playsrcProfile={};(${installDeliveryRpcObserver.toString()})(globalThis,undefined,65536);(${installSustainedObservation.toString()})();(${installBrowserFrameProfiler.toString()})(globalThis,"lifecycle");` })
  try {
    const admission = await check()
    if (admission.idleMilliseconds < 2000) throw new Error("Sustained KOTH requires two seconds of real interactive-console idle")
    await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
    const main = page.locator("main")
    await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 35_000 })
    await check()
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await check()
    await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
    const dialog = page.getByRole("dialog", { name: "CREATE SERVER" })
    await check()
    await dialog.locator("[data-vgui-name='MapList']").click()
    await check()
    await page.getByRole("option", { name: target, exact: true }).click()
    await check()
    await dialog.getByRole("tab", { name: "GAME" }).click()
    await check()
    await dialog.locator("[data-vgui-name='GameplayPage'] [data-vgui-name='NumPlayersTextEntry']").fill("23")
    await check()
    await dialog.getByRole("button", { name: "Start", exact: true }).click()
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
    await chooseTf2Team(page, "red", async () => { await check() })
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 20_000 })
    await expect(main).toHaveAttribute("data-bot-count", "23", { timeout: 15_000 })
    setupState = await snapshot()
    const settings = JSON.parse(await main.getAttribute("data-local-match-settings") ?? "null")
    expect(settings).toMatchObject({ entry: "create-server", mapIdentity: target,
      configuration: { quota: 23, difficulty: 1, mode: "normal", offlinePractice: false } })
    // Server capacity comes from the configured map catalog;23 bots plus the
    // local player do not imply a24-slot server (Harvest is configured for32).
    expect(settings.configuration.maximumPlayers).toBeGreaterThanOrEqual(24)
    await check()
    // Element focus inside an already native-admitted window; no OS activation.
    await page.locator("canvas.world-canvas").focus()
    await replay.ready(); await replay.mark(0); await replay.mark(1); replayArtifact = await replay.stop()
    if (!replayArtifact.complete || !replayArtifact.entropy) throw new Error("Authenticated startup/entropy capture incomplete")
    const startupJournal = parseGameplayReplay(await readFile(path.join(directory, replayArtifact.file)))
    const firstObserve = startupJournal.records.find(record => record.kind === 1)
    if (!firstObserve) throw new Error("Startup journal has no simulation clock baseline")
    firstSimulationObserveMilliseconds = firstObserve.bytes.readDoubleLE(0) * 1000
    const capturedEntropy = await readFile(path.join(directory, replayArtifact.entropy.file))
    if (createHash("sha256").update(capturedEntropy).digest("hex") !== replayArtifact.entropy.sha256) throw new Error("Captured entropy differs")
    await mkdir(entropyDirectory, { recursive: true })
    await writeFile(path.join(entropyDirectory, `${replayArtifact.entropy.sha256}.bin`), capturedEntropy, { flag: "wx" }).catch(async failure => {
      if (failure.code !== "EEXIST" || !capturedEntropy.equals(await readFile(path.join(entropyDirectory, `${replayArtifact.entropy.sha256}.bin`)))) throw failure
    })
    worker = page.workers().find(worker => worker.url().includes("gameplay-worker"))
    if (!worker) throw new Error("Gameplay Worker missing")
    if (!await worker.evaluate(() => (globalThis as any).__playsrcWorkerMemoryTracking(false))) throw new Error("Allocation accounting unavailable")
    cpu = await prepareWorkerCpuCapture(browser, cdp, page, 10_000)
    await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 1000 })
    await pixels("natural-setup")
    if (setupOnly) { setupComplete = true; return }
    await check()
    await page.locator("canvas.world-canvas").click()
    await page.waitForFunction(() => document.pointerLockElement?.matches("canvas.world-canvas"))
    await page.waitForTimeout(2100)
    await page.waitForFunction(() => {
      const p = (globalThis as any).__playsrcProfile
      return p?.bots?.length === 23 && p.round?.state === 4 && !p.round.waitingForPlayers && !p.round.inSetup
    }, undefined, { timeout: Math.max(1, Math.min(40_000, deadline - Date.now() - captureRemainder)) })
    if (!diagnostic) requireSustainedBudget(deadline - Date.now())
    else if (deadline - Date.now() < captureRemainder) throw new Error("Insufficient diagnostic capture/extraction budget")
    await check()
    const categories = ["disabled-by-default-display.framedisplayed", "blink.user_timing", "disabled-by-default-v8.gc"]
    const available = (await browser.send("Tracing.getCategories")).categories
    expect(categories.every(category => available.includes(category))).toBe(true)
    await browser.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
      traceConfig: { recordMode: "recordUntilFull", includedCategories: categories, traceBufferSizeInKb: 8192 } })
    traceStarted = true
    started = await page.evaluate(mark => { const started = (globalThis as any).__playsrcSustained.start(); performance.mark(mark, { startTime: started }); return started }, TRACE_START)
    observerStarted = true
    const processIds = (await browser.send("SystemInfo.getProcessInfo")).processInfo.map(process => process.id)
    const collect = async () => {
      const began = Date.now()
      const [state, processMemory, heap, wasm, workerHeap] = await Promise.all([snapshot(), memory.read(processIds), cdp.send("Runtime.getHeapUsage"),
        worker!.evaluate(() => ({ ...((globalThis as any).__playsrcWorkerMemory ?? {}) })), cpu!.heapUsage()])
      records.push({ began, ended: Date.now(), state, processMemory, heap, wasm, workerHeap, requestedAllocationTracking: allocationTracking })
      if (state.data.phase !== "Ready" || state.bots.length !== 23 || state.round.state !== 4 || state.round.waitingForPlayers || state.round.inSetup) throw new Error("Natural active full-roster gameplay was interrupted")
      if (records.length > 128) throw new Error("Sustained telemetry exceeded its bound")
      return state.at
    }
    let at = await collect(), nextMemory = started + 2000, nextInput = started, nextNative = Date.now(), agedPixels = false
    while (at - started < soakMilliseconds) {
      if (Date.now() >= deadline - SUSTAINED_KOTH.sampleMilliseconds - SUSTAINED_KOTH.extractionMilliseconds) throw new Error("Sustained soak exhausted its reserved budget; no shortening")
      if (Date.now() >= nextNative) { await check(); nextNative = Date.now() + 500 }
      if (!agedPixels && at - started >= 85_000) { await pixels("aged-before-sample"); agedPixels = true }
      if (at >= nextInput) {
        await pulse(); nextInput += 10_000
      }
      await page.waitForTimeout(250)
      at = await page.evaluate(() => performance.now())
      if (at >= nextMemory) { at = await collect(); nextMemory += 2000 }
    }
    if (diagnostic) await pixels("diagnostic-before-sample")
    lateStarted = await page.evaluate(() => performance.now())
    await page.evaluate(() => { (globalThis as any).__playsrcFrameProfiler.active = true })
    allocationStart = await worker.evaluate(() => {
      const memory = (globalThis as any).__playsrcWorkerMemory
      if (!(globalThis as any).__playsrcWorkerMemoryTracking(true)) throw new Error("Allocation accounting rejected")
      return { at: performance.now(), timeOrigin: performance.timeOrigin, memory }
    })
    allocationTracking = true
    await cpu.start(); cpuStarted = true
    await cdp.send("Profiler.start"); mainCpuStarted = true
    const end = Date.now() + SUSTAINED_KOTH.sampleMilliseconds
    await pulse()
    while (Date.now() < end) { await check(); await collect(); await page.waitForTimeout(Math.min(1000, Math.max(0, end - Date.now()))) }
    lateEnded = await page.evaluate(() => performance.now())
    lateSimulation = await page.evaluate(() => {
      const profiler = (globalThis as any).__playsrcFrameProfiler
      profiler.active = false
      return { publications: profiler.simulation, dropped: profiler.simulationDropped, frames: profiler.completedFrames, counters: profiler.counters }
    })
    allocationEnd = await worker.evaluate(() => {
      if (!(globalThis as any).__playsrcWorkerMemoryTracking(false)) throw new Error("Allocation accounting stop rejected")
      return { at: performance.now(), timeOrigin: performance.timeOrigin, memory: (globalThis as any).__playsrcWorkerMemory }
    })
    allocationTracking = false
    if (allocationEnd.at - allocationStart.at > 10_000) throw new Error("Allocation accounting exceeded its late capture bound")
    if (lateEnded - lateStarted < 5000 || lateEnded - lateStarted > 10_000) throw new Error("Detailed sample outside5–10seconds")
    workerCpu = await cpu.stop(); cpuStarted = false
    mainCpu = (await cdp.send("Profiler.stop")).profile; mainCpuStarted = false
    sampled = await page.evaluate(mark => { const sample = (globalThis as any).__playsrcSustained.stop(); performance.mark(mark, { startTime: sample.ended }); return sample }, TRACE_END)
    observerStarted = false
    await pixels("aged-after-sample")
    expect(sampled.dropped + sampled.rpc.dropped + sampled.missedFrames).toBe(0)
    expect(sampled.lifecycle).toEqual([])
    expect(sampled.inputs.map((input: any) => input.code)).toEqual(inputPlan.map(input => input.key === "a" ? "KeyA" : "KeyD"))
    if (!diagnostic) expect(lateStarted - started).toBeGreaterThanOrEqual(90_000)
  } catch (failure) { error = String(failure) }
  finally {
    if (!replayArtifact) replayArtifact = await replay.stop(false).catch(failure => ({ failure: String(failure) }))
    if (cpuStarted) workerCpu = await cpu?.stop().catch(failure => ({ failure: String(failure) }))
    if (mainCpuStarted) mainCpu = await cdp.send("Profiler.stop").catch(failure => ({ failure: String(failure) }))
    if (observerStarted) sampled = await page.evaluate(mark => { const sample = (globalThis as any).__playsrcSustained.stop(); performance.mark(mark, { startTime: sample.ended }); return sample }, TRACE_END).catch(() => null)
    if (traceStarted) {
      try {
        const finished = new Promise<any>((resolve, reject) => { const timeout = setTimeout(() => reject(new Error("Trace completion exceeded10seconds")), 10_000); browser.once("Tracing.tracingComplete", result => { clearTimeout(timeout); resolve(result) }) })
        await browser.send("Tracing.end")
        const complete = await finished
        const raw = await drainTraceStream(browser, complete.stream)
        compositor = await retainCompositorEvidence({ directory, raw: raw.bytes, complete: raw.complete, dataLossOccurred: Boolean(complete.dataLossOccurred),
          identity: { sourceCommit: commit, sourceFingerprint: fingerprint, sourceUnchanged: fingerprint === await applicationBuildIdentity(process.cwd()), target, lateStarted, lateEnded },
          categories: ["disabled-by-default-display.framedisplayed", "blink.user_timing", "disabled-by-default-v8.gc"],
          probes: { started: sampled?.started ?? 0, ended: sampled?.ended ?? 0, joins: [], dropped: sampled ? sampled.dropped + sampled.rpc.dropped : 1 } })
      } catch (failure) { error ??= String(failure) }
    }
    await worker?.evaluate(() => (globalThis as any).__playsrcWorkerMemoryTracking(false)).catch(() => {})
    await cpu?.close().catch(() => {})
    memory.close(); await native.close(); await Promise.all([cdp.detach(), browser.detach()])
    const linkedCpu = { worker: workerCpu ? await retainEvidenceBlob(directory, Buffer.from(JSON.stringify(workerCpu)), "workers.json") : null,
      main: mainCpu ? await retainEvidenceBlob(directory, Buffer.from(JSON.stringify(mainCpu)), "main.cpuprofile") : null }
    const window = compositor?.analysis.window
    const issues = setupOnly ? [] : sustainedRunIssues(sampled, lateStarted, lateEnded, !diagnostic)
    if (lateSimulation?.dropped) issues.push("Late simulation publication evidence was dropped")
    if (!setupOnly && !compositor?.manifest.complete) issues.push("Compositor evidence is incomplete")
    if (issues.length) error ??= issues.join("; ")
    const phases = sampled ? Object.fromEntries([["whole", sampled.started, sampled.ended], ["early", sampled.started, Math.min(sampled.started + 10_000, sampled.ended)], ["late", lateStarted, lateEnded]]
      .filter(([, from, to]) => Number(to) > Number(from)).map(([name, from, to]) => [name, { ...summarizeSustainedWindow(sampled, Number(from), Number(to)),
        compositor: window ? summarizeCompositorTruth(compositor.events, Number(to) - Number(from), { startedMicroseconds: Number(from) * 1000 + window.offsetMicroseconds, endedMicroseconds: Number(to) * 1000 + window.offsetMicroseconds }) : null }])) : null
    const presentationName = compositor ? chromiumPresentationEventName(compositor.events) : undefined
    const presentationStreams = new Map<string, number[]>()
    if (window && presentationName) for (const event of compositor.events) {
      if (event.name !== presentationName || !Number.isFinite(event.ts)) continue
      const stream = `${event.pid}:${event.tid}`, times = presentationStreams.get(stream) ?? []
      times.push((event.ts - window.offsetMicroseconds) / 1000); presentationStreams.set(stream, times)
    }
    const freezeWindows = sampled ? [["whole", sampled.started, sampled.ended], ["early", sampled.started, Math.min(sampled.started + 10_000, sampled.ended)], ["late", lateStarted, lateEnded]] : []
    const freezes = Object.fromEntries(freezeWindows.filter(([, from, to]) => Number(to) > Number(from)).map(([name, from, to]) => [name, {
      submissions: sustainedFreezes(sampled, Number(from), Number(to), sampled.frames.map((frame: any) => frame.at), Array.isArray(workerCpu) ? workerCpu : []),
      tickPublications: sustainedFreezes(sampled, Number(from), Number(to), sampled.ticks.map((tick: any) => tick.at), Array.isArray(workerCpu) ? workerCpu : []),
      presented: [...presentationStreams].map(([stream, times]) => ({ stream, eventName: presentationName,
        ...sustainedFreezes(sampled, Number(from), Number(to), [...new Set(times)].sort((a, b) => a - b), Array.isArray(workerCpu) ? workerCpu : []) })),
    }]))
    await writeFile(path.join(directory, "sustained-koth.json"), JSON.stringify({ schema: 1, target, commit, fingerprint, error, setupOnly, setupComplete, setupState, diagnostic,
      acceptanceEligible: !setupOnly && !diagnostic, started, lateStarted, lateEnded, plan: SUSTAINED_KOTH,
      age: { firstSimulationObserveMilliseconds, deepStartedMilliseconds: lateStarted,
        realMillisecondsSinceFirstSimulationObserve: firstSimulationObserveMilliseconds === undefined ? null : lateStarted - firstSimulationObserveMilliseconds,
        activeSoakMilliseconds: lateStarted ? lateStarted - started : null,
        firstRecordedTick: records[0]?.state.data.snapshotTick, lastRecordedTick: records.at(-1)?.state.data.snapshotTick },
      records, sampled, phases, freezes, lateSimulation, inputPlan, captures, nativeAdmission: native.records, replayArtifact, suppliedEntropy: entropyIdentity ?? null, cpu: linkedCpu, compositor: compositor?.artifact,
      allocations: { start: allocationStart, end: allocationEnd, scope: "Requested-allocation accounting is enabled only for the late detailed capture; ordinary soak records observe live/high-water/linear memory, not requested allocation rates. Counters outside the enabled interval can retain earlier startup-journal totals." },
      gc: { events: compositor?.events.filter((event: any) => /GC|GarbageCollect/.test(event.name ?? "")), scope: "Only explicit captured V8 GC events are observed GC. Heap drops without those events are inferred/unobserved, not Rust allocator frees or WASM growth. No forced collection." } }))
  }
  if (error) throw new Error(error)
  expect(compositor?.manifest.complete).toBe(true)
  expect(records.at(-1).state.bots.reduce((sum: number, bot: any) => sum + bot.shots, 0)).toBeGreaterThan(records[0].state.bots.reduce((sum: number, bot: any) => sum + bot.shots, 0))
})
