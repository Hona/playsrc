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
import { SUSTAINED_KOTH, sustainedKothTarget, requireSustainedBudget, installSustainedObservation, summarizeSustainedWindow } from "./sustained-koth"
import { windowsProcessMemory } from "./windows-process-memory"
import { prepareWorkerCpuCapture } from "./worker-cpu-profiler"
import { drainTraceStream, retainCompositorEvidence, retainEvidenceBlob } from "./compositor-evidence"
import { TRACE_START, TRACE_END, summarizeCompositorTruth } from "./compositor-truth"
import { startGameplayReplayJournal } from "./gameplay-replay"

test("sustained natural full-roster KOTH with whole-interval delivery and late CPU evidence", async ({ page, context }) => {
  if (process.platform !== "win32") throw new Error("Sustained acceptance requires the approved native Windows local job")
  const target = sustainedKothTarget(process.env.PROFILE_MAP_TARGET)
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
  await page.addInitScript({ content: `globalThis.__playsrcProfile={};(${installDeliveryRpcObserver.toString()})(globalThis,["observe"]);(${installSustainedObservation.toString()})();` })
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
    expect(JSON.parse(await main.getAttribute("data-local-match-settings") ?? "null")).toMatchObject({ entry: "create-server", mapIdentity: target,
      configuration: { quota: 23, maximumPlayers: 24, difficulty: 1, mode: "normal", offlinePractice: false } })
    await replay.ready(); await replay.mark(0); await replay.mark(1); replayArtifact = await replay.stop()
    if (!replayArtifact.complete || !replayArtifact.entropy) throw new Error("Authenticated startup/entropy capture incomplete")
    const capturedEntropy = await readFile(path.join(directory, replayArtifact.entropy.file))
    if (createHash("sha256").update(capturedEntropy).digest("hex") !== replayArtifact.entropy.sha256) throw new Error("Captured entropy differs")
    await mkdir(entropyDirectory, { recursive: true })
    await writeFile(path.join(entropyDirectory, `${replayArtifact.entropy.sha256}.bin`), capturedEntropy, { flag: "wx" }).catch(async failure => {
      if (failure.code !== "EEXIST" || !capturedEntropy.equals(await readFile(path.join(entropyDirectory, `${replayArtifact.entropy.sha256}.bin`)))) throw failure
    })
    worker = page.workers().find(worker => worker.url().includes("gameplay-worker"))
    if (!worker) throw new Error("Gameplay Worker missing")
    if (!await worker.evaluate(() => (globalThis as any).__playsrcWorkerMemoryTracking(true))) throw new Error("Allocation accounting unavailable")
    cpu = await prepareWorkerCpuCapture(browser, cdp, page)
    await pixels("natural-setup")
    await page.waitForFunction(() => {
      const p = (globalThis as any).__playsrcProfile
      return p?.bots?.length === 23 && p.round?.state === 4 && !p.round.waitingForPlayers && !p.round.inSetup
    }, undefined, { timeout: Math.max(1, Math.min(40_000, deadline - Date.now() - 111_000)) })
    requireSustainedBudget(deadline - Date.now())
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
      records.push({ began, ended: Date.now(), state, processMemory, heap, wasm, workerHeap })
      if (state.data.phase !== "Ready" || state.bots.length !== 23 || state.round.state !== 4 || state.round.waitingForPlayers || state.round.inSetup) throw new Error("Natural active full-roster gameplay was interrupted")
      if (records.length > 128) throw new Error("Sustained telemetry exceeded its bound")
      return state.at
    }
    let at = await collect(), nextMemory = started + 2000, nextInput = started, nextNative = Date.now()
    while (at - started < SUSTAINED_KOTH.soakMilliseconds) {
      if (Date.now() >= deadline - SUSTAINED_KOTH.sampleMilliseconds - SUSTAINED_KOTH.extractionMilliseconds) throw new Error("Sustained soak exhausted its reserved budget; no shortening")
      if (Date.now() >= nextNative) { await check(); nextNative = Date.now() + 500 }
      if (at >= nextInput) {
        await check()
        const key = inputPlan.length % 2 === 0 ? "a" : "d"
        const sent = await page.evaluate(() => performance.now()); await page.keyboard.down(key)
        await page.waitForTimeout(150); await page.keyboard.up(key)
        inputPlan.push({ key, sent }); nextInput += 10_000
      }
      await page.waitForTimeout(250)
      at = await page.evaluate(() => performance.now())
      if (at >= nextMemory) { at = await collect(); nextMemory += 2000 }
    }
    await pixels("aged-before-sample")
    await cpu.start(); cpuStarted = true
    await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 1000 }); await cdp.send("Profiler.start"); mainCpuStarted = true
    lateStarted = await page.evaluate(() => performance.now())
    const end = Date.now() + SUSTAINED_KOTH.sampleMilliseconds
    while (Date.now() < end) { await check(); await collect(); await page.waitForTimeout(Math.min(1000, Math.max(0, end - Date.now()))) }
    lateEnded = await page.evaluate(() => performance.now())
    if (lateEnded - lateStarted < 5000 || lateEnded - lateStarted > 10_000) throw new Error("Detailed sample outside5–10seconds")
    workerCpu = await cpu.stop(); cpuStarted = false
    mainCpu = (await cdp.send("Profiler.stop")).profile; mainCpuStarted = false
    sampled = await page.evaluate(mark => { const sample = (globalThis as any).__playsrcSustained.stop(); performance.mark(mark, { startTime: sample.ended }); return sample }, TRACE_END)
    observerStarted = false
    await pixels("aged-after-sample")
    expect(sampled.dropped + sampled.rpc.dropped + sampled.missedFrames).toBe(0)
    expect(sampled.lifecycle).toEqual([])
    expect(sampled.inputs.map((input: any) => input.code)).toEqual(inputPlan.map(input => input.key === "a" ? "KeyA" : "KeyD"))
    expect(lateStarted - started).toBeGreaterThanOrEqual(90_000)
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
    const phases = sampled ? Object.fromEntries([["whole", sampled.started, sampled.ended], ["early", sampled.started, Math.min(sampled.started + 10_000, sampled.ended)], ["late", lateStarted, lateEnded]]
      .filter(([, from, to]) => Number(to) > Number(from)).map(([name, from, to]) => [name, { ...summarizeSustainedWindow(sampled, Number(from), Number(to)),
        compositor: window ? summarizeCompositorTruth(compositor.events, Number(to) - Number(from), { startedMicroseconds: Number(from) * 1000 + window.offsetMicroseconds, endedMicroseconds: Number(to) * 1000 + window.offsetMicroseconds }) : null }])) : null
    await writeFile(path.join(directory, "sustained-koth.json"), JSON.stringify({ schema: 1, target, commit, fingerprint, error, plan: SUSTAINED_KOTH,
      records, sampled, phases, inputPlan, captures, nativeAdmission: native.records, replayArtifact, suppliedEntropy: entropyIdentity ?? null, cpu: linkedCpu, compositor: compositor?.artifact,
      gc: { events: compositor?.events.filter((event: any) => /GC|GarbageCollect/.test(event.name ?? "")), scope: "Only explicit captured V8 GC events are observed GC. Heap drops without those events are inferred/unobserved, not Rust allocator frees or WASM growth. No forced collection." } }))
  }
  if (error) throw new Error(error)
  expect(compositor?.manifest.complete).toBe(true)
  expect(records.at(-1).state.bots.reduce((sum: number, bot: any) => sum + bot.shots, 0)).toBeGreaterThan(records[0].state.bots.reduce((sum: number, bot: any) => sum + bot.shots, 0))
})
