import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { applicationBuildIdentity } from "../src/build-identity"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { installGpuTextureAccounting } from "./gpu-texture-accounting"
import { summarizeClassSwitchLifecycle } from "./class-switch-lifecycle"
import { classInputViolations, prepareClassCapture } from "./class-input-sequence"
import { TRACE_START, TRACE_END, analyzeCompositorStalls, assertVisibleGameplayTruth, summarizeCompositorStages, summarizeCompositorTruth, summarizeActivePresentationSilence, type ChromiumTraceEvent } from "./compositor-truth"
import { summarizeWebGpuTrace } from "./webgpu-trace"
import { summarizeCompositorFreezes, summarizeFreezeTimeline } from "./freeze-timeline"
import { COMPOSITOR_TRACE_CATEGORIES, TRACE_LIMITS, drainTraceStream, retainCompositorEvidence, retainEvidenceBlob, retainCapturePlan, startMainCpuEvidence, type TraceJoin, type TraceProbes } from "./compositor-evidence"
import { attributeFrameTails } from "./frame-tail-attribution"
import { summarizeCpuProfile, summarizeDistribution } from "./gameui-profile"
import { summarizeFrameTimes } from "./profile-window"
import { upwardCapturePlan } from "./upward-capture-plan"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"
import { startWorkerCpuCapture } from "./worker-cpu-profiler"
import { attributeWorkerIncidents } from "./worker-incident-attribution"
import { captureProcessMemory } from "./process-memory"
import { startGpuEngineCapture } from "./process-gpu"
import { acceptStockLoadouts } from "./stock-loadout-acceptance"
import { startGameplayReplayLifecycle } from "./gameplay-replay"
import { assertUpwardProfile, assertWorkerInstrumentation } from "./upward-profile-gates"
import { startAllocationCapture, loadAllocationMemoryEvidence } from "./allocation-memory-evidence"
import { summarizeSnapshotTransport, type SnapshotTransportBoundary } from "./snapshot-transport-memory"
import { macPageAdmission, requireMacPageAdmission, awaitMacBrowserOverlay, type MacPageAdmission } from "./macos-page-admission"
import { auditEngineerMenus } from "./engineer-menu-audit"
import { auditSpriteOrientation } from "./sprite-orientation-audit"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { auditDrawPlaneParity } from "./draw-plane-parity"
import { loadCommandWorkload, compareWorkloadJournal } from "./command-workload"
import { workloadState, assertMatchingWorkloadState, canonicalWorkloadState } from "./workload-state"
import { deliveryTimeline, installDeliveryObserver, summarizeDeliveryMeasurement } from "./frame-delivery"

let retainIncomplete: (() => Promise<unknown>) | undefined
let closeNativeAdmission: (() => Promise<void>) | undefined
test.afterEach(async () => {
  try { await retainIncomplete?.() } finally { retainIncomplete = undefined; await closeNativeAdmission?.(); closeNativeAdmission = undefined }
})

test("profile authored headed Upward offline-practice default roster and actual completed gameplay frames", async ({ page, context, profilePhases }, testInfo) => {
  const wallStarted = Date.now()
  const applicationRoot = process.cwd()
  const deliveryMode = testInfo.project.metadata.frameDeliveryMode as "ordinary" | "presentation" | "traced" | undefined
  if (deliveryMode && !["ordinary", "presentation", "traced"].includes(deliveryMode)) throw new Error("Unknown delivery comparison mode")
  const passiveDelivery = deliveryMode === "ordinary" || deliveryMode === "presentation"
  const capturePlan = upwardCapturePlan(process.env)
  const { target, entry, exerciseClasses, acceptance, combat } = capturePlan
  const seconds = capturePlan.sampleSeconds ?? 0 // Stock-only returns before sampling.
  const createServer = entry === "create-server"
  const label = process.env.PROFILE_UPWARD_TRAINING_LABEL ?? "latest"
  const evidenceLabel = `${label}-${wallStarted}`
  const { sourceCacheDir } = await loadLocalConfig(applicationRoot)
  const authorWorkload = process.env.PROFILE_AUTHOR_WORKLOAD === "1"
  const workload = process.env.PROFILE_COMMAND_WORKLOAD
    ? await loadCommandWorkload(path.join(sourceCacheDir, "profiles/command-workloads"), process.env.PROFILE_COMMAND_WORKLOAD) : undefined
  if ((authorWorkload || workload) && (!capturePlan.warmReload || exerciseClasses || combat)) throw new Error("Workload requires the declared warm ordinary roster/route")
  if ((authorWorkload || workload) && deliveryMode) throw new Error("Recorded workload controls and frame-delivery comparison projects are separate capture plans")
  // This workflow already owns the checked machine-wide lock. Admit genuine
  // physical-console idle before any gameplay input, also on the local desktop.
  if (process.platform === "darwin") {
    const idleMilliseconds = await startupConsoleIdle(sourceCacheDir)
    console.log(`PLAYSRC_NATIVE_IDLE ${JSON.stringify({ at: Date.now(), idleMilliseconds })}`)
    if (!Number.isFinite(idleMilliseconds) || idleMilliseconds < 2000) throw new Error("Gameplay profiling requires two seconds of genuine physical-console idle")
  }
  const nativeReader = await macPageAdmission(page, sourceCacheDir)
  const windowsReader = process.platform === "win32" ? await startupNativeReader(page, sourceCacheDir) : null
  closeNativeAdmission = async () => { await nativeReader?.close(); await windowsReader?.close() }
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY ?? path.join(sourceCacheDir, "profiles", createServer ? "2fort-startup" : "upward-training-bots", crypto.randomUUID())
  const evidenceDirectory = path.join(sourceCacheDir, "profiles", createServer ? "2fort-startup" : "upward-training-bots", "compositor-evidence")
  await mkdir(directory, { recursive: true })
  const nativeAdmission: MacPageAdmission[] = []
  const nativeRecords = () => windowsReader?.records ?? nativeAdmission
  const checkNativeWindow = async (desktopScreenshot?: string) => {
    try {
      if (nativeReader) nativeAdmission.push(await nativeReader.read(desktopScreenshot))
      if (windowsReader) requireStartupNative(await windowsReader.read())
    } catch (error) {
      // Preserve the rejecting native observation too; it is not a passing
      // sample and must not be guessed to be human input or a rendering fault.
      await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
      throw error
    }
  }
  const capturePlanArtifact = await retainCapturePlan(evidenceDirectory, capturePlan)
  await testInfo.attach("capture-plan", { body: JSON.stringify(capturePlanArtifact), contentType: "application/json" })
  console.log(`PLAYSRC_CAPTURE_PLAN ${JSON.stringify(capturePlanArtifact)}`)
  const replay = deliveryMode === "traced" || exerciseClasses || capturePlan.gameplayReplay === "required"
    ? await startGameplayReplayLifecycle(page, evidenceDirectory, evidenceLabel, capturePlan.warmReload, capturePlan.replacement ? 2 : 1, workload?.entropyHex, workload?.workClock) : undefined
  const replayIdentity = () => page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.applicationGeneration))
  const stopReplay = async (complete: boolean) => replay?.stop(await replayIdentity().catch(error => { if (complete) throw error; return null }), complete)
  retainIncomplete = () => stopReplay(false)
  const sourceFingerprint = process.env.PLAYSRC_PROFILE_SOURCE_FINGERPRINT ?? await applicationBuildIdentity(applicationRoot)
  const sourceCommit = spawnSync("git", ["rev-parse", "HEAD"], { cwd: applicationRoot, encoding: "utf8" })
  if (sourceCommit.status !== 0) throw new Error("Cannot establish profiler source commit")
  if (deliveryMode) expect(await applicationBuildIdentity(applicationRoot)).toBe(sourceFingerprint)
  if (deliveryMode) await page.addInitScript({ content: `(${installDeliveryObserver.toString()})();` })
  if (!passiveDelivery) await page.addInitScript({ content: `(${installGpuTextureAccounting.toString()})();(${installBrowserFrameProfiler.toString()})();${capturePlan.renderOwners
    ? `globalThis.__playsrcFrameProfiler.renderOwnerPlan=${JSON.stringify(capturePlan.renderOwners)};` : ""}` })
  if (!passiveDelivery) await page.addInitScript(captureClientFrames => {
    performance.setResourceTimingBufferSize(4096)
    ;(globalThis as any).__playsrcProfile = { captureClientFrames }
  }, authorWorkload)
  if (workload) await page.addInitScript(plan => {
    const ordinal = Number(sessionStorage.getItem("playsrc-command-workload-navigation") ?? "0") + 1
    sessionStorage.setItem("playsrc-command-workload-navigation", String(ordinal))
    if (ordinal > 2) throw new Error("Unexpected workload application generation")
    if (ordinal === 2) (globalThis as any).__playsrcCommandWorkload = plan
  }, workload.plan)
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
  const cdp = await context.newCDPSession(page)
  const browserCdp = await context.browser()!.newBrowserCDPSession()
  let profilerPreparation: Promise<unknown> = Promise.resolve()
  let profilerPreparationError: unknown
  const preparationTimings: Record<string, number> = {}
  const prepare = async <T>(name: string, action: () => Promise<T>) => {
    const began = performance.now()
    try { return await action() } finally { preparationTimings[name] = performance.now() - began }
  }
  await networkCdp.send("Network.enable")
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
    immutableRequests: number
    browserCacheHits: number
    immutableCache: Readonly<Record<string, number>>
    criticalPath: Readonly<{
      application: Readonly<Record<string, number>>
      worker: Readonly<Record<string, number>>
      spans: readonly Readonly<Record<string, unknown>>[]
      resource: Readonly<Record<string, number>>
      teamAdmissionMilliseconds: number
      firstPlayableFrameMilliseconds: number
      classSelectionMilliseconds: number
    }>
    persistentStorage: Readonly<{ records: number; bytes: number; usageBytes: number | null; quotaBytes: number | null }>
    watchdogFailures: number
    playerCount: number
    launch: any
  }> = []
  const loadPractice = async (cache: "cold" | "warm") => {
    profilePhases.enter(`map-${cache}`)
    const started = Date.now()
    const previousRequests = network.requests
    const previousBytes = network.responseBytes
    const previousWireBytes = network.wireBytes
    const previousImmutableRequests = network.immutableRequests
    const previousCacheHits = network.cacheHits
    networkStage = `${cache}-startup`
    if (cache === "cold") await page.goto(process.env.PLAYSRC_PROFILE_ORIGIN ? "/tf2" : "/", { waitUntil: "domcontentloaded", timeout: 30_000 })
    else await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 })
    // Enable domains while ordinary startup is already in flight, without
    // delaying or moving its Ready observation. No sampling/heap read starts
    // here; their existing pre-sample boundaries below remain authoritative.
    if (workload) profilerPreparation = prepare(`domains-${cache}`, () => Promise.all([
      cdp.send("Performance.enable"), cdp.send("HeapProfiler.enable"),
      cdp.send("Profiler.enable").then(() => cdp.send("Profiler.setSamplingInterval", { interval: 1000 })),
    ])).catch(error => { profilerPreparationError = error })
    await page.bringToFront()
    if (windowsReader) await checkNativeWindow()
    await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 100_000 })
    const startupMilliseconds = Date.now() - started
    networkStage = `${cache}-menu`
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    let playerCount: number
    let start: import("@playwright/test").Locator
    if (createServer) {
      await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
      const dialog = layer.getByRole("dialog", { name: "CREATE SERVER" })
      await dialog.locator("[data-vgui-name='MapList']").click()
      await page.getByRole("option", { name: "ctf_2fort" }).click()
      await dialog.getByRole("tab", { name: "GAME" }).click()
      await dialog.locator("[data-vgui-name='GameplayPage'] [data-vgui-name='NumPlayersTextEntry']").fill("23")
      playerCount = 24
      start = dialog.getByRole("button", { name: "Start" })
    } else {
      await page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']").click()
      await layer.locator("[data-vgui-name='OfflinePracticePanel'] [data-vgui-name='StartButton']").click()
      const next = layer.locator("[data-vgui-name='OfflinePractice_ModeSelectionPanel'] [data-vgui-name='NextButton']")
      await next.click()
      await next.click()
      await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("Payload")
      await layer.locator("[data-vgui-name='SelectCurrentGameModeButton']").click()
      await expect(layer.locator("[data-vgui-name='MapNameLabel']")).toHaveText("Upward")
      const mapPanel = layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel']")
      const playerEntry = mapPanel.locator("[data-vgui-name='NumPlayersTextEntry']")
      if (capturePlan.playersOverride) await playerEntry.fill(capturePlan.playersOverride)
      playerCount = Number(await playerEntry.inputValue())
      start = mapPanel.locator("[data-vgui-name='StartOfflinePracticeButton']")
    }
    if (acceptance) expect(playerCount).toBe(createServer ? 24 : 16)
    else expect(playerCount).toBeGreaterThanOrEqual(12)
    const mapStarted = Date.now()
    networkStage = `${cache}-map`
    await start.click({ timeout: 5_000 })
    await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 110_000 })
    const teamAdmissionMilliseconds = Date.now() - mapStarted
    await chooseTf2Team(page, cache === "warm" ? capturePlan.warmTeam : capturePlan.coldTeam)
    await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
    const readyMilliseconds = Date.now() - mapStarted
    const classSelectionMilliseconds = Date.now() - mapStarted
    const firstPlayableFrameMilliseconds = await page.evaluate(async (elapsed) => {
      const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
      if (!canvas) throw new Error("authored gameplay canvas is unavailable")
      const started = performance.now()
      const initial = Number(canvas.dataset.displayFrame ?? 0)
      await new Promise<void>((resolve, reject) => {
        const poll = () => Number(canvas.dataset.displayFrame ?? 0) > initial ? resolve()
          : performance.now() - started > 10_000 ? reject(new Error("authored playable frame was not presented"))
          : requestAnimationFrame(poll)
        requestAnimationFrame(poll)
      })
      return elapsed + performance.now() - started
    }, Date.now() - mapStarted)
    networkStage = `${cache}-gameplay`
    await expect(root).toHaveAttribute("data-bot-count", String(playerCount - 1), { timeout: 70_000 })
    const launch = JSON.parse(await root.getAttribute("data-local-match-settings") ?? "null")
    expect(launch).toMatchObject({ entry, mapIdentity: target, configuration: { quota: playerCount - 1, offlinePractice: !createServer } })
    const persistence = await page.evaluate(async () => {
      const request = indexedDB.open("playsrc-derived-v3")
      const database = await new Promise<IDBDatabase>((resolve, reject) => {
        request.onsuccess = () => resolve(request.result)
        request.onerror = () => reject(request.error)
      })
      try {
        const inventory = database.transaction("metadata", "readonly").objectStore("metadata").getAll()
        const records = await new Promise<Array<{ byteLength: number }>>((resolve, reject) => {
          inventory.onsuccess = () => resolve(inventory.result)
          inventory.onerror = () => reject(inventory.error)
        })
        const estimate = await navigator.storage.estimate()
        return {
          transferredBytes: (performance.getEntriesByType("resource") as PerformanceResourceTiming[]).reduce((total, entry) => total + entry.transferSize, 0),
          immutableCache: structuredClone((globalThis as any).__playsrcProfile?.immutableCache ?? {}),
          spans: structuredClone((globalThis as any).__playsrcProfile?.startupSpans ?? []),
          persistentStorage: { records: records.length, bytes: records.reduce((total, record) => total + record.byteLength, 0), usageBytes: estimate.usage ?? null, quotaBytes: estimate.quota ?? null },
          watchdogFailures: document.querySelector<HTMLElement>("main")?.dataset.phase === "Failed" ? 1 : 0,
        }
      } finally { database.close() }
    })
    const loadPerformance = JSON.parse(await root.getAttribute("data-load-performance") ?? "null") as {
      application: Record<string, number>
      client: Record<string, number>
    } | null
    loads.push({
      cache, startupMilliseconds, readyMilliseconds,
      requests: network.requests - previousRequests,
      responseBytes: network.responseBytes - previousBytes,
      transferredBytes: persistence.transferredBytes,
      wireBytes: network.wireBytes - previousWireBytes,
      immutableRequests: network.immutableRequests - previousImmutableRequests,
      browserCacheHits: network.cacheHits - previousCacheHits,
      immutableCache: persistence.immutableCache,
      criticalPath: (() => {
        const spans = persistence.spans as Array<{ kind: string; roles?: string; started: number; finished: number; bytes?: number }>
        const gameplay = spans.filter((span) => span.roles === "gameplay")
        const totals = (kind: string) => gameplay.filter((span) => span.kind === kind)
          .reduce((total, span) => total + span.finished - span.started, 0)
        const fetch = gameplay.filter((span) => span.kind === "chunk-acquire")
        const decode = gameplay.filter((span) => span.kind === "resource-decode")
        return {
          application: loadPerformance?.application ?? {},
          worker: loadPerformance?.client ?? {},
          spans,
          resource: {
            chunks: fetch.length,
            groups: decode.length,
            acquireAggregateMilliseconds: totals("chunk-acquire"),
            decodeAggregateMilliseconds: totals("resource-decode"),
            indexAggregateMilliseconds: totals("resource-index"),
            finalizeMilliseconds: totals("resource-finalize"),
            acquisitionWallMilliseconds: fetch.length ? Math.max(...fetch.map((span) => span.finished)) - Math.min(...fetch.map((span) => span.started)) : 0,
            decodeWallMilliseconds: decode.length ? Math.max(...decode.map((span) => span.finished)) - Math.min(...decode.map((span) => span.started)) : 0,
          },
          teamAdmissionMilliseconds,
          firstPlayableFrameMilliseconds,
          classSelectionMilliseconds,
        }
      })(),
      persistentStorage: persistence.persistentStorage,
      watchdogFailures: persistence.watchdogFailures,
      playerCount, launch,
    })
  }
  await loadPractice("cold")
  // Establish the actual Page -> CDP window -> native drawing window before
  // warm navigation/replacement/resize, not while the new window is animating.
  if (nativeReader || windowsReader) {
    await page.bringToFront()
    await checkNativeWindow()
    await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
    if (nativeReader) requireMacPageAdmission(nativeAdmission[0]!)
  }
  if (capturePlan.warmReload) {
    await replay?.beforeReload(await replayIdentity())
    await loadPractice("warm")
  }
  const replacement: Array<Record<string, unknown>> = []
  if (capturePlan.replacement) {
    const prior = await root.getAttribute("data-generation")
    await page.keyboard.press("Backquote")
    const command = page.locator("[aria-label='Console command']")
    await command.fill(`map ${target}`)
    await command.press("Enter")
    await expect(root).not.toHaveAttribute("data-generation", prior!, { timeout: 60_000 })
    await expect(root).toHaveAttribute("data-phase", "Ready")
    replacement.push(await page.evaluate(() => ({ ...document.querySelector<HTMLElement>("main")!.dataset })))
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    if (await root.getAttribute("data-team-selection-visible") === "true") await chooseTf2Team(page, "red")
    if (await root.getAttribute("data-class-selection-visible") === "true") await page.keyboard.press("Digit3")
    await page.keyboard.press("Backquote")
    await command.fill(`tf_bot_quota ${loads.at(-1)!.playerCount - 1}`)
    await command.press("Enter")
    await page.keyboard.press("Backquote")
    await expect(root).toHaveAttribute("data-bot-count", String(loads.at(-1)!.playerCount - 1))
    const original = page.viewportSize()!
    for (const size of [{ width: 1024, height: 700 }, original]) {
      await page.setViewportSize(size)
      await page.waitForFunction(({ width, height }) => {
        const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
        return innerWidth === width && innerHeight === height && canvas.width === width * devicePixelRatio && canvas.height === height * devicePixelRatio
      }, size)
      const frame = Number(await page.locator("canvas.world-canvas").getAttribute("data-display-frame"))
      await expect.poll(async () => Number(await page.locator("canvas.world-canvas").getAttribute("data-display-frame"))).toBeGreaterThan(frame + 1)
      replacement.push(await page.evaluate(() => ({ width: innerWidth, height: innerHeight, ...document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!.dataset })))
    }
  }
  if (capturePlan.stockOnly) {
    const stock = await acceptStockLoadouts(page, directory, label)
    const losses = await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)
    await writeFile(path.join(directory, `${label}-correctness.json`), JSON.stringify({ headed: true, capturePlan, capturePlanArtifact, loads, stock, losses, team: capturePlan.warmReload ? capturePlan.warmTeam : capturePlan.coldTeam, performanceSample: false }, null, 2))
    expect(stock).toHaveLength(27)
    expect(losses).toEqual([])
    return
  }
  const finalLoad = loads.at(-1)!
  const { readyMilliseconds, playerCount, launch } = finalLoad
  profilePhases.enter("pre-sample")
  const expectedBots = playerCount - 1

  const combatCommand = async (value: string): Promise<void> => {
    await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value)
    await entry.press("Enter")
    await page.keyboard.press("Backquote")
  }
  if (combat) {
    await combatCommand("joinclass pyro")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("7")
    await combatCommand("+attack")
  }

  const canvas = page.locator("canvas.world-canvas")
  const canvasBox = await canvas.boundingBox()
  if (!canvasBox) throw new Error("Visible class input surface is absent")
  const capturePoint = { x: canvasBox.x + canvasBox.width / 2, y: canvasBox.y + canvasBox.height / 2 }
  await replay?.ready()
  await page.bringToFront()
  await canvas.focus()
  expect(await page.evaluate(() => document.hasFocus())).toBe(true)
  const before = await canvas.screenshot({ timeout: 20_000 })
  if (deliveryMode) {
    if (exerciseClasses || combat || capturePlan.interaction !== "forward-movement" || expectedBots !== 15) throw new Error("Delivery comparison requires the same 15-bot forward-movement scenario")
    const boundary = await page.evaluate(() => ({ userAgent: navigator.userAgent, viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio,
      canvasWidth: document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!.width, canvasHeight: document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!.height },
      state: { ...document.querySelector<HTMLElement>("main")!.dataset }, storage: { ...localStorage },
      instrumentation: { app: Boolean((globalThis as any).__playsrcProfile), frame: Boolean((globalThis as any).__playsrcFrameProfiler) } }))
    const configuration = await (await page.request.get("/playsrc-config.json")).json()
    await writeFile(path.join(directory, "delivery-boundary.json"), JSON.stringify({ mode: deliveryMode, applicationCommit: sourceCommit.stdout.trim(),
      sourceFingerprint, harnessRoot: repositoryRoot, harnessFingerprint: process.env.PLAYSRC_PROFILE_HARNESS_FINGERPRINT,
      browserVersion: context.browser()!.version(), configuration, capturePlan, boundary }, null, 2))
    await writeFile(path.join(directory, "delivery-before.png"), before)
  }
  if (passiveDelivery) {
    expect(await page.evaluate(() => Boolean((globalThis as any).__playsrcProfile || (globalThis as any).__playsrcFrameProfiler))).toBe(false)
    const presentationCdp = deliveryMode === "presentation" ? await context.browser()!.newBrowserCDPSession() : undefined
    const presentationCategories = ["disabled-by-default-display.framedisplayed", "blink.user_timing"]
    const processBefore = await presentationCdp?.send("SystemInfo.getProcessInfo")
    const processBoundaryStarted = performance.now()
    const memoryBefore = processBefore ? await captureProcessMemory(processBefore.processInfo) : undefined
    const gpuEngines = processBefore && process.platform === "win32" ? await startGpuEngineCapture(processBefore.processInfo, seconds) : undefined
    if (presentationCdp) {
      const available = (await presentationCdp.send("Tracing.getCategories")).categories
      expect(presentationCategories.every(category => available.includes(category))).toBe(true)
      await presentationCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
        traceConfig: { recordMode: "recordUntilFull", includedCategories: presentationCategories, traceBufferSizeInKb: 8192 } })
    }
    await page.keyboard.down("w")
    let monitoring = true, nativeFailure: unknown
    const monitor = (async () => { while (monitoring) { await new Promise(resolve => setTimeout(resolve, 500)); if (monitoring) try { await checkNativeWindow() } catch (error) { nativeFailure = error; break } } })()
    let sample: any
    try {
      sample = await page.evaluate(async ({ seconds, presented, startMark, endMark }) => {
        const owner = (globalThis as any).__playsrcDeliveryObserver
        const started = performance.now(), startedEpoch = performance.timeOrigin + started
        owner.start(started)
        if (presented) performance.mark(startMark, { startTime: started })
        await new Promise(resolve => setTimeout(resolve, seconds * 1000))
        const ended = performance.now()
        if (presented) performance.mark(endMark, { startTime: ended })
        return { ...owner.stop(ended), startedEpoch, endedEpoch: performance.timeOrigin + ended }
      }, { seconds, presented: Boolean(presentationCdp), startMark: TRACE_START, endMark: TRACE_END })
    } finally { monitoring = false; await page.keyboard.up("w"); await monitor }
    await checkNativeWindow()
    await writeFile(path.join(directory, "delivery-after.png"), await canvas.screenshot())
    await writeFile(path.join(directory, "delivery.json"), JSON.stringify({ mode: deliveryMode, applicationCommit: sourceCommit.stdout.trim(), sourceFingerprint,
      sample, completed: deliveryTimeline(sample.started, sample.ended, sample.frames.map((frame: any) => frame.at)),
      raf: deliveryTimeline(sample.started, sample.ended, sample.raf), nativeAdmission: nativeRecords(), nativeFailure: nativeFailure ? String(nativeFailure) : null }, null, 2))
    if (presentationCdp) {
      const processAfter = await presentationCdp.send("SystemInfo.getProcessInfo")
      const processBoundaryEnded = performance.now()
      const memoryAfter = await captureProcessMemory(processAfter.processInfo)
      let timer: ReturnType<typeof setTimeout> | undefined
      const complete = new Promise<{ stream?: string; dataLossOccurred?: boolean }>((resolve, reject) => {
        presentationCdp.once("Tracing.tracingComplete", resolve)
        timer = setTimeout(() => reject(new Error("Presentation trace completion exceeded 15 seconds")), 15_000)
      })
      let completion
      try { await presentationCdp.send("Tracing.end"); completion = await complete }
      finally { clearTimeout(timer) }
      expect(completion.stream).toBeTruthy()
      const raw = await drainTraceStream(presentationCdp, completion.stream!)
      const evidence = await retainCompositorEvidence({ directory, raw: raw.bytes, complete: raw.complete, dataLossOccurred: Boolean(completion.dataLossOccurred),
        categories: presentationCategories, identity: { sourceCommit: sourceCommit.stdout.trim(), sourceFingerprint, sourceUnchanged: await applicationBuildIdentity(applicationRoot) === sourceFingerprint,
          mode: deliveryMode, nativeAdmission: nativeRecords(), applicationGeneration: (await (await page.request.get("/playsrc-config.json")).json()),
          instrumentation: "Read-only submission/RAF observer and native display/user-timing trace only; no application, Worker, CPU or heap sampler" },
        probes: { started: sample.started, ended: sample.ended, joins: sample.frames.map((frame: any) => ({ kind: "completed-submission", at: frame.at })), dropped: sample.missedPublications } })
      const compositor = summarizeCompositorTruth(evidence.events, sample.ended - sample.started, evidence.analysis.window ?? undefined)
      await writeFile(path.join(directory, "delivery-presentation.json"), JSON.stringify({ evidence: evidence.artifact, complete: evidence.manifest.complete, compositor,
        nativeDelivery: evidence.analysis, processes: { before: processBefore, after: processAfter, started: processBoundaryStarted, ended: processBoundaryEnded,
          scope: "Unprorated process CPU counters bracket sampling and boundary readback; not active-only CPU or GPU device time" },
        memory: { before: memoryBefore, after: memoryAfter, scope: "Boundary process residency/commit, not active peaks or deduplicated physical RAM" },
        gpuEngines: gpuEngines ? { ...await gpuEngines.finished, scope: gpuEngines.scope, startedEpoch: sample.startedEpoch, endedEpoch: sample.endedEpoch } : null }, null, 2))
      expect(evidence.manifest.complete).toBe(true)
      expect(compositor.evidence).toBe("chromium-compositor-presentation-trace")
      await presentationCdp.detach()
    }
    expect(nativeFailure).toBeUndefined()
    expect(sample.lifecycle).toEqual([])
    expect(sample.missedPublications).toBe(0)
    expect(sample.ended - sample.started).toBeGreaterThanOrEqual(5000)
    expect(sample.ended - sample.started).toBeLessThanOrEqual(10000)
    expect(sample.lastFrame).toBeGreaterThan(sample.firstFrame)
    expect(await applicationBuildIdentity(applicationRoot)).toBe(sourceFingerprint)
    return
  }
  if (capturePlan.interaction === "movement-weapon") await canvas.click({ position: { x: 300, y: 250 } })
  await profilerPreparation
  if (profilerPreparationError) throw profilerPreparationError
  const system = await browserCdp.send("SystemInfo.getInfo").catch(() => null)
  const processBefore = await browserCdp.send("SystemInfo.getProcessInfo").catch(() => null)
  const nativeScreenshot = nativeReader ? path.join(directory, `${evidenceLabel}.desktop.png`) : null
  if (!nativeReader && process.env.PROFILE_NATIVE_SCREENSHOT === "1") throw new Error("Native desktop screenshot capture requires the configured macOS capture tool")
  if (nativeReader && capturePlan.interaction === "movement-weapon") {
    const wait: MacPageAdmission[] = []
    try { await awaitMacBrowserOverlay(() => nativeReader.read(), wait) }
    finally { await writeFile(path.join(directory, `${label}-native-overlay-wait.json`), JSON.stringify({ scope: "before-sample", records: wait })) }
  }
  await checkNativeWindow(nativeScreenshot ?? undefined)
  await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
  if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
  const auditParity = () => auditDrawPlaneParity(page, canvas, directory, label, process.env.PROFILE_DRAW_LIGHTING_PARITY === "1"
    || process.env.PROFILE_DRAW_LIGHTING_PARITY_ONLY === "1", async phase => {
      await checkNativeWindow(nativeReader ? path.join(directory, `${label}-parity-${phase}.desktop.png`) : undefined)
      await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
      if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
    })
  if (process.env.PROFILE_DRAW_LIGHTING_PARITY_ONLY === "1") { await auditParity(); return }
  if (process.env.PROFILE_ENGINEER_UI_ONLY === "1") {
    await auditEngineerMenus(page, root, directory, label, combatCommand)
    await checkNativeWindow(nativeReader ? path.join(directory, `${label}-ui-after.desktop.png`) : undefined)
    await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
    if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
    if (process.env.PROFILE_PARTICLE_ORIENTATION_AUDIT === "1") {
      await auditSpriteOrientation(page, directory, label, async file => {
        await checkNativeWindow(nativeReader ? file : undefined)
        if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
      })
    }
    return
  }
  const memoryBefore = await captureProcessMemory(processBefore?.processInfo, { remote: Boolean(process.env.PLAYSRC_PROFILE_CDP_ENDPOINT) })
  const browserVersion = await browserCdp.send("Browser.getVersion")
  const applicationGeneration = await page.evaluate(() => (globalThis as any).__playsrcProfile.applicationGeneration ?? null)
  const availableCategories = (await browserCdp.send("Tracing.getCategories")).categories
  const traceFinished = new Promise<{ stream?: string; dataLossOccurred: boolean }>(resolve => browserCdp.once("Tracing.tracingComplete", resolve))
  let compositorLayers: Array<{ layerId: string; backendNodeId?: number; width: number; height: number; drawsContent: boolean; paintCount: number }> = []
  let layerTreeChanges = 0
  const layerPaints: Array<{ layerId: string; x: number; y: number; width: number; height: number }> = []
  cdp.on("LayerTree.layerTreeDidChange", ({ layers }) => {
    layerTreeChanges += 1
    if (layers) compositorLayers = layers
  })
  cdp.on("LayerTree.layerPainted", ({ layerId, clip }) => layerPaints.push({ layerId, ...clip }))
  await cdp.send("LayerTree.enable")
  const workerCpu = capturePlan.workerCpu === "required" ? await startWorkerCpuCapture(browserCdp, cdp, page) : undefined
  // Never enter an active sample that the shared runner's total deadline would
  // necessarily truncate. The construction/command journal is already durable.
  const totalDeadline = Number(process.env.PLAYSRC_PROFILE_DEADLINE ?? Date.now() + 175_000)
  if (!Number.isFinite(totalDeadline)) throw new Error("Invalid bounded profile deadline")
  if (totalDeadline - Date.now() < seconds * 1000 + 20_000) throw new Error("Insufficient bounded capture/retention time after lock and startup; partial replay retained")
  if (workload) {
    // Arm native collection no more than three seconds before the authenticated
    // phase, keeping collection plus the five-second sample within ten seconds.
    await page.waitForFunction(start => {
      const owner = (globalThis as any).__playsrcProfile.commandWorkload
      return owner?.epoch !== undefined && owner.epoch + start - performance.now() <= 3000
    }, workload.plan.sampleStarted, { timeout: 40_000 })
    expect(await page.evaluate(start => (globalThis as any).__playsrcProfile.commandWorkload.epoch + start - performance.now(), workload.plan.sampleStarted)).toBeGreaterThan(0)
  }
  const allocationCapture = await prepare("allocation-start", () => startAllocationCapture(cdp, evidenceDirectory))
  const rawPartial = path.join(directory, "compositor-evidence", `${evidenceLabel}.trace.partial.gz`)
  await mkdir(path.dirname(rawPartial), { recursive: true })
  await writeFile(rawPartial, Buffer.alloc(0), { flag: "wx" })
  const mainCpu = await prepare("main-cpu-start", () => startMainCpuEvidence(cdp, evidenceDirectory, { sourceCommit: sourceCommit.stdout.trim(), sourceFingerprint }))
  let interrupted = false
  let traceStarted = false
  const collectNative = async () => {
    const mainCapture = await mainCpu.stop()
    const collectionErrors: string[] = []
    const workerCapture = await (workerCpu?.stop() ?? Promise.resolve([]))
      .then(captures => ({ captures, error: null as string | null }), error => ({ captures: [], error: String(error) }))
    await workerCpu?.close().catch(() => undefined)
    let workerBytes = Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", ...workerCapture, unsampledTargets: workerCpu?.unsampledTargets ?? [] }))
    if (workerBytes.byteLength > TRACE_LIMITS.probeBytes) {
      workerCapture.error = "Worker CPU evidence exceeds its byte bound"
      workerBytes = Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", captures: [], error: workerCapture.error }))
    }
    const workerArtifact = await retainEvidenceBlob(evidenceDirectory, workerBytes, "workers.json")
    if (workerCapture.error) collectionErrors.push(workerCapture.error)
    if (capturePlan.workerCpu === "required") {
      try { assertWorkerInstrumentation(workerCapture.captures.map(capture => ({
        deadlineStopped: capture.deadlineStopped, sampleCount: capture.profile.samples?.length ?? 0,
        captureComplete: capture.execution.dropped === 0,
      }))) } catch (error) { collectionErrors.push(String(error)) }
    }
    let completion: { stream?: string; dataLossOccurred: boolean } = { dataLossOccurred: true }
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      if (!traceStarted) throw new Error("Native trace did not start; main CPU diagnostics retained")
      completion = await Promise.race([(async () => { await browserCdp.send("Tracing.end"); return traceFinished })(),
        new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Native trace completion exceeded 5 seconds")), 5000) })])
    } catch (error) { collectionErrors.push(String(error)) }
    finally { clearTimeout(timer) }
    const raw = completion.stream ? await drainTraceStream(browserCdp, completion.stream, TRACE_LIMITS.compressedBytes, chunk => appendFile(rawPartial, chunk)) : { bytes: new Uint8Array(), complete: false }
    await retainEvidenceBlob(evidenceDirectory, raw.bytes, "trace.json.gz")
    // Keep the existing setup/collection clock endpoint before heap extraction.
    const performanceAfter = (await cdp.send("Performance.getMetrics").catch(() => ({ metrics: [] }))).metrics
    // Begin extraction without delaying the authored input-release edge. Await
    // its completion only when binding the already stopped capture to a manifest.
    const memory = (async () => {
      const allocation = await allocationCapture.stop()
      const processAfter = await browserCdp.send("SystemInfo.getProcessInfo").catch(() => null)
      const memoryAfter = await captureProcessMemory(processAfter?.processInfo, { remote: Boolean(process.env.PLAYSRC_PROFILE_CDP_ENDPOINT) })
      return { allocation, processAfter, memoryAfter }
    })()
    return { workerCapture, workerArtifact, completion, raw, mainCapture, collectionErrors, performanceAfter, memory }
  }
  let nativeResult: ReturnType<typeof collectNative> | undefined
  const finishNative = () => nativeResult ??= collectNative()
  const persistNativeEvidence = async (probes: TraceProbes, details: Record<string, unknown>) => {
    const { raw, completion, workerArtifact, mainCapture, collectionErrors, memory } = await finishNative()
    const { allocation, memoryAfter } = await memory
    const sourceFingerprintAfter = await applicationBuildIdentity(applicationRoot).catch(error => `unavailable: ${String(error)}`)
    return retainCompositorEvidence({ directory: evidenceDirectory, raw: raw.bytes,
      complete: raw.complete && !interrupted, dataLossOccurred: completion.dataLossOccurred, mainCpu: mainCapture, collectionErrors,
      memory: { schema: "playsrc-allocation-memory-v1", main: allocation,
        snapshotTransport: snapshotBoundaries, processes: { before: memoryBefore, after: memoryAfter } },
      identity: { sourceCommit: sourceCommit.stdout.trim(), sourceFingerprint, sourceFingerprintAfter,
        sourceUnchanged: sourceFingerprint === sourceFingerprintAfter, applicationGeneration, browserVersion,
        gpu: system?.gpu ?? null, availableCategories, origin: new URL(page.url()).origin,
        label, headed: true, target, entry, launch, interrupted, workerCpu: workerArtifact, capturePlan: capturePlanArtifact,
        nativeScreenshot, beforeScreenshotSha256: createHash("sha256").update(before).digest("hex"), ...details }, probes })
  }
  let retained: ReturnType<typeof persistNativeEvidence> | undefined
  let snapshotBoundaries: { before: SnapshotTransportBoundary; after: SnapshotTransportBoundary } | null = null
  const retainNativeEvidence = (probes: TraceProbes, details: Record<string, unknown>) => retained ??= persistNativeEvidence(probes, details)
  const retainInterrupted = () => retainNativeEvidence({ started: 0, ended: 0, joins: [], dropped: 1 }, { sampleError: "Capture interrupted before measurement retention" })
  const interrupt = () => {
    interrupted = true
    void Promise.allSettled([retainInterrupted(), stopReplay(false)])
  }
  let captureDeadline: ReturnType<typeof setTimeout> | undefined
  process.once("SIGTERM", interrupt)
  retainIncomplete = async () => {
    clearTimeout(captureDeadline)
    process.off("SIGTERM", interrupt)
    interrupted = true
    await Promise.allSettled([finishNative(), stopReplay(false)])
    const evidence = await retainInterrupted()
    console.log(`PLAYSRC_COMPOSITOR_EVIDENCE ${JSON.stringify(evidence.artifact)}`)
  }
  // Preserve the authored order: sampler setup before movement input. A slow
  // Profiler.start must not advance the player before the original input edge.
  if (authorWorkload) {
    // Select a real, authoritative weapon-draw phase. Initial idle animation can
    // depend on the first presentation read during loading; a declared ordinary
    // slot transition supplies its exact activity tick without resetting clocks,
    // bots, simulation, rendering, or existing recorded input bytes.
    await page.keyboard.press("Digit2")
    await expect.poll(async () => {
      const weapon = (await root.getAttribute("data-hud-probe"))?.split(":")[2]
      return Boolean(weapon && weapon !== "unavailable" && weapon !== "1")
    }).toBe(true)
    // Use the real secondary draw completion, not an arbitrary delay, to give
    // replay preparation a stable prelude before the measured primary draw.
    await expect.poll(async () => await root.getAttribute("data-viewmodel-activity")).toContain("IDLE")
    await page.keyboard.press("Digit1")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("1")
  }
  if (!exerciseClasses) await page.keyboard.down("w")
  await replay?.mark(0)
  await prepare("trace-start", () => browserCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
    traceConfig: { recordMode: "recordUntilFull", traceBufferSizeInKb: TRACE_LIMITS.browserKilobytes, includedCategories: [...COMPOSITOR_TRACE_CATEGORIES] } }))
  traceStarted = true
  captureDeadline = setTimeout(interrupt, Math.min(seconds * 1000 + 5000, Math.max(1, totalDeadline - Date.now() - 5000)))
  await workerCpu?.start()
  const performanceBefore = (await cdp.send("Performance.getMetrics").catch(() => ({ metrics: [] }))).metrics
  const clockBefore = performanceBefore.find(metric => metric.name === "Timestamp")?.value
  profilePhases.enter("sample-and-readback")
  const measurementPromise = page.evaluate(async ({ duration, startMark, endMark, workloadStart, captureState, workloadTick }) => {
    const profile = (globalThis as any).__playsrcProfile
    let phase: { at: number; snapshotAt: number; owner: any; values: Record<string, number>; frameCompletedAt: number } | undefined
    if (captureState) {
      profile.workloadFrame = undefined; profile.workloadTargetTick = workloadTick
      phase = await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`Authenticated displayed phase missing: ${JSON.stringify(profile.workloadProgress)}`)), 4000)
        profile.workloadStateReady = (frameCompletedAt: number) => {
          clearTimeout(timer)
          const owner = profile.snapshotTransport, values = structuredClone(owner ?? {})
          const snapshotAt = performance.now(), at = performance.now()
          resolve({ at, snapshotAt, owner, values, frameCompletedAt })
        }
        profile.captureWorkloadState = true
      })
    }
    const main = document.querySelector<HTMLElement>("main")!
    const surface = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const instrumentation = (globalThis as any).__playsrcFrameProfiler
    const workloadInitial = { frame: structuredClone((globalThis as any).__playsrcProfile.workloadFrame),
      round: structuredClone((globalThis as any).__playsrcProfile.round), producerTick: main.dataset.snapshotTick }
    ;(globalThis as any).__playsrcProfile.captureWorkloadState = false
    const firstTick = Number(main.dataset.snapshotTick)
    const firstFrame = Number(surface.dataset.displayFrame)
    const firstPosition = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    const firstUploads = structuredClone((globalThis as any).__playsrcProfile.modelParticleUploads ?? {}) as Record<string, number>
    const firstSnapshotOwner = phase?.owner ?? (globalThis as any).__playsrcProfile.snapshotTransport
    const firstSnapshots = phase?.values ?? structuredClone(firstSnapshotOwner ?? {}) as Record<string, number>
    const firstSnapshotAt = phase?.snapshotAt ?? performance.now()
    const started = phase?.at ?? performance.now()
    ;(globalThis as any).__playsrcDeliveryObserver?.start(started)
    ;(globalThis as any).__playsrcProfile.classInputSampleStarted = started
    performance.mark(startMark, { startTime: started })
    const lifecycle: Array<{ at: number; phase: string; playerClass?: number; key?: string; visible?: boolean; button?: number; trusted?: boolean; controllerAction?: string | null }> = []
    const mark = (phase: string, detail: { playerClass?: number; key?: string; visible?: boolean; button?: number; trusted?: boolean; controllerAction?: string | null } = {}) => {
      lifecycle.push({ at: Number((performance.now() - started).toFixed(3)), phase, ...detail })
    }
    const controllerAction = () => (globalThis as any).__playsrcProfile.classInputAction ?? null
    const keydown = (event: KeyboardEvent) => mark("key-down", { key: event.code, trusted: event.isTrusted, controllerAction: controllerAction() })
    const pointerdown = (event: PointerEvent) => mark(event.button !== 0 ? "other-pointer-button" : document.pointerLockElement === surface ? "weapon-fire" : "pointer-capture",
      { button: event.button, trusted: event.isTrusted, controllerAction: controllerAction() })
    document.addEventListener("keydown", keydown, true)
    surface.addEventListener("pointerdown", pointerdown, true)
    let ended = started
    const browserLifecycle: Array<{ at: number; kind: string; visible: boolean; focused: boolean; width: number; height: number; dpr: number }> = []
    const state = (kind: string) => browserLifecycle.push({ at: performance.now(), kind, visible: document.visibilityState === "visible", focused: document.hasFocus(), width: innerWidth, height: innerHeight, dpr: devicePixelRatio })
    const visibility = () => state("visibility")
    const focus = () => state("focus")
    const resize = () => state("resize")
    document.addEventListener("visibilitychange", visibility)
    window.addEventListener("focus", focus)
    window.addEventListener("blur", focus)
    window.addEventListener("resize", resize)
    state("start")
    const classSwitches: Array<{ at: number; playerClass: number; completedFrames: number; textures: number; pipelines: number; buffers: number; queueWriteBytes: number; workerCalls: number }> = []
    let previousClass = Number((main.dataset.hudProbe ?? "").split(":")[1])
    const mutations = { total: 0, attributes: 0, text: 0, children: 0, rootAttributes: 0, hud: 0, style: 0 }
    const hudRoot = document.querySelector<HTMLElement>(".hud-layer")
    const mutationObserver = new MutationObserver((records) => {
      for (const record of records) {
        mutations.total += 1
        if (record.type === "attributes") {
          mutations.attributes += 1
          if (record.target === main) mutations.rootAttributes += 1
          if (record.attributeName === "style") mutations.style += 1
        } else if (record.type === "characterData") mutations.text += 1
        else mutations.children += 1
        if (hudRoot?.contains(record.target)) mutations.hud += 1
      }
    })
    mutationObserver.observe(main, { attributes: true, characterData: true, childList: true, subtree: true })
    let previousVisibility = main.dataset.classSelectionVisible === "true"
    const classObserver = new MutationObserver(() => {
      const visible = main.dataset.classSelectionVisible === "true"
      if (visible !== previousVisibility) { previousVisibility = visible; mark("class-panel", { visible }) }
      const playerClass = Number((main.dataset.hudProbe ?? "").split(":")[1])
      if (!Number.isSafeInteger(playerClass) || playerClass === previousClass) return
      previousClass = playerClass
      mark("selected", { playerClass })
      classSwitches.push({ at: performance.now() - started, playerClass, completedFrames: instrumentation.counters.completedFrames,
        textures: instrumentation.counters.textures, pipelines: instrumentation.counters.renderPipelines, buffers: instrumentation.counters.buffers,
        queueWriteBytes: instrumentation.counters.queueWriteBytes, workerCalls: instrumentation.worker.length })
    })
    classObserver.observe(main, { attributes: true, attributeFilter: ["data-hud-probe", "data-class-selection-visible"] })
    let animationCallbacks = 0
    const particleSamples: Array<{ at: number; classIdentity: number; items: number; particleUploadBytes: number; particleUploadWrites: number; queueWriteBytes: number; textureWrites: number }> = []
    let previousParticleUploadBytes = firstUploads.particleUploadBytes ?? 0
    let previousParticleUploadWrites = firstUploads.particleUploadWrites ?? 0
    let previousQueueWriteBytes = 0
    let previousTextureWrites = 0
    instrumentation.active = true
    try {
      while (performance.now() - started < duration * 1000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        animationCallbacks += 1
        const particleUploadBytes = Number((globalThis as any).__playsrcProfile.modelParticleUploads?.particleUploadBytes ?? 0)
        const particleUploadWrites = Number((globalThis as any).__playsrcProfile.modelParticleUploads?.particleUploadWrites ?? 0)
        const queueWriteBytes = instrumentation.counters.queueWriteBytes
        const textureWrites = instrumentation.counters.textureWrites
        particleSamples.push({
          at: performance.now() - started,
          classIdentity: Number(main.dataset.hudProbe?.split(":")[1] ?? 0),
          items: Number(main.dataset.particleItems ?? 0),
          particleUploadBytes: particleUploadBytes - previousParticleUploadBytes,
          particleUploadWrites: particleUploadWrites - previousParticleUploadWrites,
          queueWriteBytes: queueWriteBytes - previousQueueWriteBytes,
          textureWrites: textureWrites - previousTextureWrites,
        })
        previousParticleUploadBytes = particleUploadBytes
        previousParticleUploadWrites = particleUploadWrites
        previousQueueWriteBytes = queueWriteBytes
        previousTextureWrites = textureWrites
        if (main.dataset.phase !== "Ready") throw new Error(`Upward training left gameplay: ${main.dataset.phase}: ${main.dataset.detail}`)
      }
    } finally {
      ended = performance.now()
      performance.mark(endMark, { startTime: ended })
      state("end")
      instrumentation.active = false; classObserver.disconnect(); mutationObserver.disconnect()
      document.removeEventListener("visibilitychange", visibility)
      window.removeEventListener("focus", focus)
      window.removeEventListener("blur", focus)
      window.removeEventListener("resize", resize)
      document.removeEventListener("keydown", keydown, true)
      surface.removeEventListener("pointerdown", pointerdown, true)
    }
    const elapsed = ended - started
    const position = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    const panels = [...document.querySelectorAll<HTMLElement>(".vgui-layer, .playsrc-vgui-runtime")].map(element => {
      const bounds = element.getBoundingClientRect()
      const style = getComputedStyle(element)
      return { className: element.className, width: bounds.width, height: bounds.height, visible: style.display !== "none" && style.visibility !== "hidden", background: style.backgroundColor, opacity: style.opacity, filter: style.filter, isolation: style.isolation, contain: style.contain }
    })
    const gpuContext = surface.getContext("webgpu")
    const output = {
      elapsed, started, ended, workloadInitial,
      workloadClock: structuredClone((globalThis as any).__playsrcProfile.commandWorkload ?? null),
      workloadPhaseAdmission: workloadStart === null ? null : { expected: profile.commandWorkload.epoch + workloadStart,
        actual: started, delayMilliseconds: started - profile.commandWorkload.epoch - workloadStart },
      clientFrames: structuredClone((globalThis as any).__playsrcProfile.clientFrames ?? null),
      clientFrameWorkload: structuredClone((globalThis as any).__playsrcProfile.clientFrameWorkload ?? null),
      browserLifecycle, firstTick, lastTick: Number(main.dataset.snapshotTick), firstFrame,
      visible: document.visibilityState === "visible", focused: document.hasFocus(), animationCallbacks,
      viewport: { width: innerWidth, height: innerHeight, devicePixelRatio, visualViewportScale: visualViewport?.scale ?? null, canvasWidth: surface.width, canvasHeight: surface.height },
      lastFrame: Number(surface.dataset.displayFrame), traveled: Math.hypot(...position.map((value, index) => value - firstPosition[index]!)),
      roster: structuredClone((globalThis as any).__playsrcProfile.bots), scoreboard: JSON.parse(main.dataset.scoreboardProbe ?? "{}"),
      frames: instrumentation.completedFrames, compositorFrames: instrumentation.compositorFrames, particleSamples,
      renderOwners: instrumentation.renderOwners ?? [],
      presentationCallbacks: instrumentation.animationCallbacks, worker: instrumentation.worker, input: instrumentation.input, counters: instrumentation.counters, queueWrites: instrumentation.queueWrites,
      simulationPublications: instrumentation.simulation, simulationPublicationsDropped: instrumentation.simulationDropped,
      classSwitches, lifecycle, nodeBuilds: instrumentation.nodeBuilds,
      pipelinePreparation: { firstStaticPropUses: instrumentation.firstStaticPropUses ?? [], models: instrumentation.modelPreparation, retainedTemplates: instrumentation.counters.retainedModelTemplates ?? 0,
        reusedPreparedModels: instrumentation.counters.reusedPreparedModels ?? 0 },
      dom: { mutations, nodes: document.getElementsByTagName("*").length, hudNodes: hudRoot?.getElementsByTagName("*").length ?? 0,
        panels, rasterImages: document.querySelectorAll("img[data-vgui-raster]").length, rasterCanvases: document.querySelectorAll("canvas[data-vgui-raster]").length,
        accessibility: { hudLabels: hudRoot?.querySelectorAll("[aria-label]").length ?? 0, gameView: surface.getAttribute("aria-label") },
        canvas: { alphaMode: gpuContext?.getConfiguration()?.alphaMode ?? null, format: gpuContext?.getConfiguration()?.format ?? null },
      },
      modelUploads: Object.fromEntries(Object.entries((globalThis as any).__playsrcProfile.modelParticleUploads ?? {})
        .map(([key, value]) => [key, typeof value === "number" ? value - (firstUploads[key] ?? 0) : value])),
      snapshotTransport: {
        // Tokens denote reference equality of the actual stream.metrics object
        // within this sample, not an inferred application/map generation.
        before: { at: firstSnapshotAt, ownerToken: firstSnapshotOwner ? 0 : null, values: firstSnapshots },
        after: { at: performance.now(), ownerToken: !(globalThis as any).__playsrcProfile.snapshotTransport ? null
          : firstSnapshotOwner === (globalThis as any).__playsrcProfile.snapshotTransport ? 0 : 1,
          values: structuredClone((globalThis as any).__playsrcProfile.snapshotTransport ?? {}) as Record<string, number> },
      },
      capabilities: instrumentation.capabilities, gpuTimestamps: instrumentation.gpuTimestamps, losses: instrumentation.losses,
      textureAllocation: (globalThis as any).__playsrcGpuTextureAccounting,
      gpuOperations: instrumentation.gpuOperations, gpuOperationsDropped: instrumentation.gpuOperationsDropped,
      deviceEvidence: { adapters: instrumentation.adapters, devices: instrumentation.devices, shaders: instrumentation.shaders,
        identitiesDropped: instrumentation.gpuIdentitiesDropped, shadersDropped: instrumentation.shadersDropped },
      screen: {
        css: { width: surface.clientWidth, height: surface.clientHeight },
        physical: { width: surface.width, height: surface.height },
        devicePixelRatio: devicePixelRatio,
        visualViewportScale: visualViewport?.scale ?? null,
        visibility: document.visibilityState,
      },
      longTasks: instrumentation.longTasks.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
      longAnimationFrames: instrumentation.longAnimationFrames.filter((entry: { at: number }) => entry.at >= started && entry.at < started + elapsed),
    }
    const delivery = (globalThis as any).__playsrcDeliveryObserver?.stop(output.ended) ?? null
    await instrumentation.flushShaderHashes()
    return { ...output, delivery }
  }, { duration: workload ? (workload.plan.sampleEnded - workload.plan.sampleStarted) / 1000 : seconds,
    startMark: TRACE_START, endMark: TRACE_END, workloadStart: workload?.plan.sampleStarted ?? null,
    captureState: Boolean(authorWorkload || workload), workloadTick: workload?.initialState?.frame.tick ?? null })
  const exercisedClasses: string[] = []
  const admittedAttacks: Array<{ playerClass: number; weapon: number; lifecycle: number; hostTick: string; at: number; requestId: number; playerTick: string; firstPrimaryTick: string; nextPrimaryTick: string; primaryActivities: unknown[] }> = []
  let visibleScoreboardRows: number | null = null
  const exercise = async () => {
    if (!exerciseClasses) return
    const classes = ["heavyweapons", "pyro", "medic", "spy", "engineer", "sniper", "scout", "demoman", "soldier"] as const
    const digits = [5, 3, 7, 9, 6, 8, 1, 4, 2] as const
    const identities = [6, 7, 5, 8, 9, 2, 1, 4, 3] as const
    const now = () => performance.now()
    const deadline = now() + seconds * 1000
    const action = (value: string) => page.evaluate(value => { (globalThis as any).__playsrcProfile.classInputAction = value }, value)
    await action("scoreboard")
    await page.keyboard.down("Tab")
    visibleScoreboardRows = await root.evaluate((element, timeout) => new Promise<number | null>(resolve => {
      const started = performance.now()
      const poll = () => {
        if (element.dataset.scoreboardVisible === "true") {
          resolve(element.querySelectorAll("[data-vgui-name='RedPlayerList'] [data-vgui-item], [data-vgui-name='BluePlayerList'] [data-vgui-item]").length)
        } else if (performance.now() - started >= timeout) resolve(null)
        else requestAnimationFrame(poll)
      }
      poll()
    }), Math.max(0, deadline - now()))
    await page.keyboard.up("Tab")
    await action("none")
    let lastNativeCapture = 0
    for (const [position, playerClass] of (acceptance ? classes : [...classes, ...classes]).entries()) {
      const index = position % classes.length
      if (now() >= deadline) break
      // Blink's native limiter starts at one, then resets to zero after two
      // seconds since the last successful lock: four first admissions, then
      // five per reset. Respect the real wall-clock policy; never emulate lock.
      const cooling = acceptance ? position > 0 && position % 4 === 0 : position >= 4 && (position - 4) % 5 === 0
      if (!await prepareClassCapture({
        earliestCapture: cooling ? lastNativeCapture + 2100 : 0, deadline, now,
        delay: milliseconds => page.waitForTimeout(milliseconds),
        select: async () => {
          await action("select")
          try {
            exercisedClasses.push(playerClass)
            await page.keyboard.press("Comma")
            if (!await root.evaluate((element, timeout) => new Promise<boolean>(resolve => {
              const started = performance.now()
              const poll = () => element.dataset.classSelectionVisible === "true" ? resolve(true)
                : performance.now() - started >= timeout ? resolve(false) : requestAnimationFrame(poll)
              poll()
            }), Math.max(0, deadline - now()))) return false
            await page.keyboard.press(`Digit${digits[index]}`)
            if (!await root.evaluate((element, { identity, timeout }) => new Promise<boolean>(resolve => {
              const started = performance.now()
              const poll = () => (element.dataset.hudProbe ?? "").split(":")[1] === String(identity) ? resolve(true)
                : performance.now() - started >= timeout ? resolve(false) : requestAnimationFrame(poll)
              poll()
            }), { identity: identities[index], timeout: Math.max(0, deadline - now()) })) return false
            // The grenade/sticky rigid-body path is deliberately unavailable.
            // Exercise the Demoman's authored Bottle via a real slot key instead
            // of counting a mouse press which the application must reject.
            if (identities[index] === 4) await page.keyboard.press("Digit3")
            return await root.evaluate((element, { identity, timeout }) => new Promise<boolean>(resolve => {
              const started = performance.now()
              let selectedTick: number | undefined
              const poll = () => {
                const hud = (element.dataset.hudProbe ?? "").split(":")
                if (hud[1] === String(identity) && (identity !== 4 || hud[2] === "17") && element.dataset.classSelectionVisible === "false") {
                  selectedTick ??= Number(element.dataset.snapshotTick)
                  const frames = (globalThis as any).__playsrcFrameProfiler.completedFrames
                  if (frames.length && Number(frames.at(-1).tick) >= selectedTick!) { resolve(true); return }
                }
                if (performance.now() - started >= timeout) resolve(false)
                else requestAnimationFrame(poll)
              }
              poll()
            }), { identity: identities[index], timeout: Math.max(0, deadline - now()) })
          } finally { await action("none") }
        },
      })) break
      // A lock acquired during the idle/setup interval was not this controller's
      // capture. Do not manufacture exclusivity by unlocking or ignoring input.
      if (await page.evaluate(() => document.pointerLockElement !== null)) break
      await action("capture")
      await page.bringToFront()
      await canvas.focus()
      // Visibility, class state and a completed frame were acknowledged above.
      // Send native coordinates directly: locator.click's two stability frames
      // are not another Source deploy or a prerequisite for this fixed viewport.
      await page.mouse.click(capturePoint.x, capturePoint.y)
      // The first native click requests capture; it is not a weapon-fire edge.
      // Both scenario modes must admit capture before sending actual held fire.
      await expect(root).toHaveAttribute("data-pointer-locked", "true", { timeout: Math.max(1, Math.min(2000, deadline - now())) })
      lastNativeCapture = now()
      const beforeAttack = await page.evaluate(() => {
        const records = (globalThis as any).__playsrcFrameProfiler.simulation
        return { index: records.length, hostTick: records.at(-1)?.replayAttack?.hostTick ?? "0" }
      })
      const minimumHold = acceptance ? 100 : 20
      if (now() + minimumHold >= deadline) break
      await action("attack")
      try {
        await page.mouse.down()
        const pressedAt = now()
        const admitted = await page.evaluate(({ before, identity, timeout }) => new Promise<any>(resolve => {
          const started = performance.now()
          let cursor = before.index
          const poll = () => {
            const profile = (globalThis as any).__playsrcFrameProfiler
            const sampleStart = (globalThis as any).__playsrcProfile.classInputSampleStarted
            for (; cursor < profile.simulation.length; cursor++) {
              const record = profile.simulation[cursor], attack = record.replayAttack
              if (!attack || attack.playerClass !== identity || attack.lifecycle !== 1 || BigInt(attack.hostTick) <= BigInt(before.hostTick)) continue
              for (const publication of record.publications) {
                if (!publication.selectedTicks || BigInt(attack.hostTick) < BigInt(publication.firstHostTick) || BigInt(attack.hostTick) > BigInt(publication.lastHostTick)) continue
                const weapon = publication.weapons.find((weapon: any) => weapon.weapon === attack.weapon)
                if (!weapon) continue
                resolve({ playerClass: attack.playerClass, weapon: attack.weapon, lifecycle: attack.lifecycle, hostTick: attack.hostTick,
                  at: record.at - sampleStart, requestId: record.requestId, playerTick: publication.player.tick,
                  firstPrimaryTick: weapon.firstPrimaryTick, nextPrimaryTick: weapon.nextPrimaryTick,
                  primaryActivities: publication.activities.filter((activity: any) => activity.weapon === attack.weapon && activity.activity === 2) })
                return
              }
            }
            if (performance.now() - started >= timeout) resolve(null)
            else requestAnimationFrame(poll)
          }
          poll()
        }), { before: beforeAttack, identity: identities[index], timeout: Math.max(0, deadline - now()) })
        if (!admitted || admitted.at >= seconds * 1000) break
        const remainingHold = Math.max(0, pressedAt + minimumHold - now())
        if (now() + remainingHold >= deadline) break
        if (remainingHold) await page.waitForTimeout(remainingHold)
        admittedAttacks.push(admitted)
      } finally { await page.mouse.up(); await action("none") }
    }
  }
  const interaction = capturePlan.interaction === "movement-weapon"
    ? (async () => {
      await page.waitForTimeout(300)
      for (let index = 0; index < 6; index += 1) {
        await page.mouse.move(310 + index * 14, 255 + index * 3)
        if (index === 1) await page.keyboard.down("a")
        if (index === 2) await page.keyboard.press("Digit2")
        if (index === 3) await page.keyboard.press("Digit1")
        if (index === 4) { await page.mouse.down(); await page.mouse.up() }
        if (index === 5) await page.keyboard.up("a")
        await page.waitForTimeout(250)
      }
    })()
    : Promise.resolve()
  const combatActions = combat ? (async () => {
    for (const identity of ["heavyweapons", "medic", "soldier"]) {
      await new Promise(resolve => setTimeout(resolve, seconds * 250))
      await combatCommand(`joinclass ${identity}`)
    }
  })() : Promise.resolve()
  let sampling = true
  let nativeFailure: string | null = null
  const nativeMonitor = (async () => {
    while (sampling && (nativeReader || windowsReader)) {
      await new Promise(resolve => setTimeout(resolve, 500))
      if (!sampling) break
      try { await checkNativeWindow() } catch (error) { nativeFailure = String(error); break }
      if (nativeAdmission.at(-1)?.error) break
      if (nativeRecords().length >= 32) break
    }
  })()
  const sample = await Promise.all([measurementPromise.finally(() => { sampling = false }), exercise(), interaction, combatActions])
    .then(values => ({ measurement: values[0], error: null }), error => ({ measurement: null, error: String(error) }))
  sampling = false
  await nativeMonitor
  sample.error ??= nativeFailure
  if (deliveryMode === "traced" && sample.measurement?.delivery) {
    const delivery = sample.measurement.delivery
    expect(delivery.lifecycle).toEqual([])
    expect(delivery.missedPublications).toBe(0)
    await writeFile(path.join(directory, "delivery.json"), JSON.stringify({ mode: deliveryMode, applicationCommit: sourceCommit.stdout.trim(), sourceFingerprint,
      sample: delivery, completed: deliveryTimeline(delivery.started, delivery.ended, delivery.frames.map((frame: any) => frame.at)),
      raf: deliveryTimeline(delivery.started, delivery.ended, delivery.raf), nativeAdmission: nativeRecords() }, null, 2))
  }
  await checkNativeWindow(nativeScreenshot ? path.join(directory, `${evidenceLabel}.after.desktop.png`) : undefined)
  await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
  profilePhases.enter("trace-drain")
  clearTimeout(captureDeadline)
  process.off("SIGTERM", interrupt)
  await replay?.mark(1).catch(() => { interrupted = true })
  if (authorWorkload) await replay?.stopAdmission()
  const replayPending = authorWorkload ? undefined : stopReplay(!interrupted && sample.error === null)
  // Stop the real Worker sampler before ending the trace so its end clock mark
  // remains joinable. Failure here must not discard the native browser trace.
  const { workerCapture, workerArtifact, mainCapture, performanceAfter, memory } = await finishNative()
  const clockAfter = performanceAfter.find(metric => metric.name === "Timestamp")?.value
  if (!exerciseClasses) await page.keyboard.up("w").catch(() => undefined)
  // Author a genuine live tail for playback retention, after the original trace,
  // memory and input-release boundaries. These ticks are never sampled as FPS.
  if (authorWorkload) await page.waitForTimeout(4000)
  const replayCapture = await (replayPending ?? stopReplay(!interrupted && sample.error === null))
  const replayArtifact = replayCapture?.artifact
  const clientFrameInputs = authorWorkload ? await page.evaluate(() => (globalThis as any).__playsrcProfile.clientFrames ?? null) : null
  const presentationInputs = authorWorkload ? await page.evaluate(() => (globalThis as any).__playsrcProfile.presentationInputs ?? null) : null
  const joins: TraceJoin[] = []
  const measured = sample.measurement
  const initialWorkloadState = (workload || authorWorkload) && measured
    ? workloadState(measured.workloadInitial.frame) : null
  if (measured) {
    // The private capture may contain BigInt/typed model inputs. Persist their
    // lossless canonical form, never a second non-JSON raw copy in the report.
    const producerAtStart = canonicalWorkloadState({ tick: measured.workloadInitial.producerTick, round: measured.workloadInitial.round })
    delete (measured as Partial<typeof measured>).workloadInitial
    Object.assign(measured, { workloadState: initialWorkloadState, producerAtStart })
  }
  snapshotBoundaries = measured?.snapshotTransport ?? null
  if (measured) {
    for (const record of measured.renderOwners) joins.push({ kind: "render-owners", at: measured.started, end: measured.ended, detail: record })
    for (const record of measured.gpuOperations) joins.push({ kind: "gpu", at: record.at, end: record.end ?? measured.ended, detail: record })
    for (const record of measured.lifecycle) joins.push({ kind: "class-lifecycle", at: measured.started + record.at, detail: record })
    for (const record of measured.browserLifecycle) joins.push({ kind: "browser-lifecycle", at: record.at, detail: record })
    for (const record of measured.classSwitches) joins.push({ kind: "class", at: measured.started + record.at, detail: record })
    for (const record of measured.particleSamples) joins.push({ kind: "particles", at: measured.started + record.at, detail: record })
    for (const record of measured.frames) joins.push({ kind: "completed-frame", at: record.at, detail: record })
    for (const record of measured.worker) joins.push({ kind: "worker", at: record.started, end: record.finished ?? measured.ended, detail: record })
    for (const record of measured.simulationPublications) joins.push({ kind: "simulation-publication", at: record.at, end: record.at + record.decodeMilliseconds, detail: record })
    for (const record of measured.longAnimationFrames) joins.push({ kind: "long-animation-frame", at: record.at, end: record.at + record.duration, detail: record })
  }
  profilePhases.enter("trace-analysis-retention")
  const evidence = await retainNativeEvidence(
    { started: measured?.started ?? 0, ended: measured?.ended ?? 0, joins, dropped: measured ? measured.gpuOperationsDropped + measured.simulationPublicationsDropped + measured.renderOwners.reduce((n: number, r: any) => n + r.dropped, 0) : 1 },
    { viewport: measured?.viewport ?? null, sampleError: sample.error, gameplayReplay: replayArtifact,
      gameplayReplayLifecycle: replayCapture?.lifecycle, nativeAdmission: nativeRecords(), replacement,
      workloadState: initialWorkloadState,
      workloadIdentity: process.env.PROFILE_COMMAND_WORKLOAD ?? null, authorWorkload, preparationTimings, clientFrameInputs, presentationInputs })
  const sourceFingerprintAfter = evidence.manifest.identity.sourceFingerprintAfter
  // Reference durable evidence before subsequent CPU/heap extraction, screenshots, or assertions can fail.
  await testInfo.attach("compositor-evidence", { body: JSON.stringify(evidence.artifact), contentType: "application/json" })
  console.log(`PLAYSRC_COMPOSITOR_EVIDENCE ${JSON.stringify(evidence.artifact)}`)
  profilePhases.enter("diagnostics-and-pixels")
  retainIncomplete = undefined
  if (replayArtifact && !replayArtifact.complete) throw new Error(`Gameplay replay incomplete; diagnostics retained: ${replayArtifact.error}`)
  if (interrupted) throw new Error("Capture interrupted; partial diagnostics retained, not passing evidence")
  if (workerCapture.error) throw new Error(`Worker CPU capture failed; raw compositor evidence retained: ${workerCapture.error}`)
  if (!measured) throw new Error(`Gameplay sampling failed; compositor evidence retained: ${sample.error}`)
  if (workload) {
    let failure: string | null = null
    try {
      assertMatchingWorkloadState(workload.initialState, evidence.manifest.identity.workloadState)
      if (!replayArtifact?.file) throw new Error("Actual workload journal is absent")
      compareWorkloadJournal(await readFile(workload.journalFile), await readFile(path.join(evidenceDirectory, replayArtifact.file)), workload.plan.sampleEnded / 1000)
    } catch (error) { failure = String(error) }
    await writeFile(path.join(directory, `${label}-workload-validation.json`), JSON.stringify({ identity: process.env.PROFILE_COMMAND_WORKLOAD,
      accepted: failure === null, failure, expected: workload.initialState, actual: evidence.manifest.identity.workloadState }, null, 2))
    if (failure) throw new Error(failure)
  }
  if (evidence.manifest.mainCpu?.errors.length || !mainCapture.profile) throw new Error(`Main CPU capture failed; diagnostics retained: ${evidence.manifest.mainCpu?.errors.join("; ")}`)
  const { allocation, processAfter, memoryAfter } = await memory
  const memoryEvidence = await loadAllocationMemoryEvidence(path.join(evidenceDirectory, evidence.artifact.file), evidence)
  if (allocation.errors.length || !allocation.heapBefore || !allocation.heapAfter) throw new Error(`Allocation capture failed; diagnostics retained: ${allocation.errors.join("; ")}`)
  const heapBefore = allocation.heapBefore.value, heapAfter = allocation.heapAfter.value
  const cpuProfile = mainCapture.profile
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = 1 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.worldLighting?.revision === 1)
  const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.worldLighting)
  const measurement = measured
  const traceEvents: ChromiumTraceEvent[] = evidence.events
  const exactTraceWindow = evidence.manifest.analysis.window
  const layerDetails = await Promise.all(compositorLayers.filter(layer => layer.drawsContent).map(async layer => {
    const [reasons, node] = await Promise.all([
      cdp.send("LayerTree.compositingReasons", { layerId: layer.layerId }).catch(() => null),
      layer.backendNodeId ? cdp.send("DOM.describeNode", { backendNodeId: layer.backendNodeId }).catch(() => null) : null,
    ])
    return {
      id: layer.layerId, width: layer.width, height: layer.height, paintCount: layer.paintCount,
      estimatedRasterBytes: Math.ceil(layer.width) * Math.ceil(layer.height) * 4,
      node: node?.node.nodeName ?? null, attributes: node?.node.attributes ?? null,
      reasons: reasons?.compositingReasons ?? [], reasonIds: reasons?.compositingReasonIds ?? [],
    }
  }))
  const wasmWorkers = await Promise.all(page.workers().map(async (worker) => ({
    url: worker.url(),
    ...await (worker.url().includes("gameplay-worker") ? worker.evaluate(() => ({
      heapBytes: (performance as any).memory?.usedJSHeapSize ?? null,
      memory: (globalThis as any).__playsrcWorkerMemory ?? null,
    })).catch(() => ({ heapBytes: null, memory: null })) : { heapBytes: null, memory: null }),
  })))
  const loadPerformance = JSON.parse(await root.getAttribute("data-load-performance") ?? "null")
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
  const workers = measurement.worker as Array<{ kind: string; started: number; finished?: number; bytes: number; receivedBytes?: number; sharedBytes?: number; views?: number; sharedDispatch?: boolean; timings?: Record<string, number> }>
  const worker = Object.fromEntries([...new Set(workers.map(item => item.kind))].sort().map(kind => {
    const records = workers.filter(item => item.kind === kind)
    return [kind, { calls: records.length, views: records.reduce((sum, item) => sum + (item.views ?? 0), 0), sharedDispatches: records.filter(item => item.sharedDispatch).length, bytes: records.reduce((sum, item) => sum + item.bytes, 0), receivedBytes: records.reduce((sum, item) => sum + (item.receivedBytes ?? 0), 0), sharedBytes: records.reduce((sum, item) => sum + (item.sharedBytes ?? 0), 0), milliseconds: summarizeDistribution(records.flatMap(item => item.finished === undefined ? [] : [item.finished - item.started])), timings: Object.fromEntries([...new Set(records.flatMap(item => Object.keys(item.timings ?? {})))].map(key => [key, summarizeDistribution(records.flatMap(item => typeof item.timings?.[key] === "number" ? [item.timings[key]!] : []))])) }]
  }))
  const decoded = decodeScreenshot(after)
  let nonBlack = 0
  for (let index = 0; index < decoded.pixels.length; index += decoded.channels) {
    if (decoded.pixels[index]! > 16 || decoded.pixels[index + 1]! > 16 || decoded.pixels[index + 2]! > 16) nonBlack += 1
  }
  const actualFrames = measurement.lastFrame - measurement.firstFrame
  const workerIncidents = exactTraceWindow ? attributeWorkerIncidents(traceEvents, workerCapture.captures, exactTraceWindow, { requests: measurement.worker, publications: measurement.simulationPublications }) : []
  const tails = attributeFrameTails({
    frames: completed, workers, inputs: measurement.input, longAnimationFrames: measurement.longAnimationFrames,
    trace: traceEvents, cpu: cpuProfile,
    traceOffsetMicroseconds: exactTraceWindow?.offsetMicroseconds ?? 0,
  })
  const gpuProcessBefore = processBefore?.processInfo.find(process => process.type === "GPU")
  const gpuProcessAfter = processAfter?.processInfo.find(process => process.type === "GPU")
  const performanceDelta = (name: string): number => Number(((performanceAfter.find(metric => metric.name === name)?.value ?? 0)
    - (performanceBefore.find(metric => metric.name === name)?.value ?? 0)).toFixed(6))
  const paint = Object.fromEntries(["Paint", "Layout", "UpdateLayoutTree", "RasterTask", "CompositeLayers", "Commit"].map((name) => {
    const events = traceEvents.filter(event => event.name === name && typeof event.dur === "number")
    return [name, summarizeDistribution(events.map(event => event.dur! / 1_000))]
  }))
  const report = {
    nativeAdmission: nativeRecords(), replacement, nodeBuilds: measurement.nodeBuilds ?? [], geometry, pipelinePreparation: measurement.pipelinePreparation,
    schema: "playsrc-tf2-upward-training-bots-profile-v4", label, headed: true, target, entry, launch, capturePlan, capturePlanArtifact,
    sourceFingerprint,
    roster: measurement.roster.map((bot: any) => ({ identity: bot.identity, class: bot.class, team: bot.team, difficulty: bot.difficulty })),
    activeBots: measurement.roster.length, teams: { red: measurement.scoreboard.red.playerCount, blue: measurement.scoreboard.blue.playerCount },
    elapsedMilliseconds: Number(measurement.elapsed.toFixed(3)), readyMilliseconds, loads, totalWallMilliseconds: Date.now() - wallStarted,
    animationCallbacks: measurement.animationCallbacks, completedFrames: actualFrames, applicationCompletedFramesPerSecond: Number((actualFrames / measurement.elapsed * 1000).toFixed(3)),
    compositor: { ...summarizeCompositorTruth(exactTraceWindow ? traceEvents : [], measurement.elapsed, exactTraceWindow ?? undefined),
      stages: summarizeCompositorStages(exactTraceWindow ? traceEvents : [], exactTraceWindow ?? undefined),
      stalls: exactTraceWindow ? analyzeCompositorStalls(traceEvents, exactTraceWindow, measurement.lifecycle) : [] },
    nativeWebGpu: exactTraceWindow ? summarizeWebGpuTrace(traceEvents, exactTraceWindow) : null,
    freezes: {
      completedGameSubmissions: summarizeFreezeTimeline(completed.map(frame => frame.at - measurement.started), { startedMilliseconds: 0, endedMilliseconds: measurement.elapsed }),
      compositor: exactTraceWindow ? summarizeCompositorFreezes(traceEvents, exactTraceWindow) : null,
    },
    compositorIncludingSetupAndCollection: clockBefore !== undefined && clockAfter !== undefined && clockAfter > clockBefore ? {
      elapsedMilliseconds: (clockAfter - clockBefore) * 1_000,
      ...summarizeCompositorTruth(traceEvents, (clockAfter - clockBefore) * 1_000, { startedMicroseconds: clockBefore * 1_000_000, endedMicroseconds: clockAfter * 1_000_000 }),
    } : null,
    compositorEvidence: { ...evidence.artifact, complete: evidence.manifest.complete, errors: evidence.manifest.errors,
      issues: evidence.manifest.analysis.issues, incidents: evidence.manifest.analysis.incidents.map(({ work, joins, ...incident }) => incident) },
    presentationOpportunities:{frames:compositor.length,framesPerSecond:Number((compositor.length/measurement.elapsed*1000).toFixed(3)),animationCallbacks:measurement.presentationCallbacks.length,intervals:summarizeFrameTimes(compositor.slice(1).map((frame,index)=>frame.at-compositor[index]!.at)),submissionLatency:summarizeDistribution(compositor.map(frame=>frame.submissionMilliseconds))},
    settings: await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.videoQuality)),
    browser: { platform: await page.evaluate(() => navigator.platform), userAgent: await page.evaluate(() => navigator.userAgent), controllerPlatform: process.platform, origin: new URL(page.url()).origin, channel: process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? "playwright-chromium", viewport: measurement.viewport, visible: measurement.visible, focused: measurement.focused, lifecycle: measurement.browserLifecycle, gpu: system?.gpu ?? null, processes: { before: processBefore?.processInfo ?? null, after: processAfter?.processInfo ?? null, memoryBefore, memoryAfter }, network, storage, userMachineEvidence: false },
    firstPlayableBoundary: "application-completed-frame-not-compositor",
    frameIntervals: summarizeFrameTimes(intervals), ...summarizeDeliveryMeasurement(measurement),
    presentationDom: {
      ...measurement.dom,
      layers: {
        count: compositorLayers.length, drawing: layerDetails.length, treeChanges: layerTreeChanges,
        estimatedRasterBytes: layerDetails.reduce((total, layer) => total + layer.estimatedRasterBytes, 0),
        painted: { count: layerPaints.length, cssPixels: layerPaints.reduce((total, paint) => total + paint.width * paint.height, 0), rectangles: layerPaints.slice(0, 32) },
        details: layerDetails,
      },
      layout: { count: performanceDelta("LayoutCount"), milliseconds: Number((performanceDelta("LayoutDuration") * 1000).toFixed(3)) },
      style: { count: performanceDelta("RecalcStyleCount"), milliseconds: Number((performanceDelta("RecalcStyleDuration") * 1000).toFixed(3)) },
      paint,
    },
    simulation: { ticks: measurement.lastTick - measurement.firstTick, hertz: Number(((measurement.lastTick - measurement.firstTick) / measurement.elapsed * 1000).toFixed(3)) },
    worker,
    classSwitches: { requested: exercisedClasses, attacks: admittedAttacks,
      inputGuard: { unplanned: classInputViolations(measurement.lifecycle), captures: measurement.lifecycle.filter(event => event.phase === "pointer-capture").length, presses: measurement.lifecycle.filter(event => event.phase === "weapon-fire").length },
      lifecycle: measurement.lifecycle, timing: summarizeClassSwitchLifecycle(measurement.lifecycle), observed: measurement.classSwitches.map((item, index, values) => ({
      ...item,
      millisecondsSincePrevious: Number((item.at - (values[index - 1]?.at ?? 0)).toFixed(3)),
      createdTextures: item.textures - (values[index - 1]?.textures ?? 0),
      createdPipelines: item.pipelines - (values[index - 1]?.pipelines ?? 0),
      createdBuffers: item.buffers - (values[index - 1]?.buffers ?? 0),
      uploadedBytes: item.queueWriteBytes - (values[index - 1]?.queueWriteBytes ?? 0),
      workerCallsSincePrevious: item.workerCalls - (values[index - 1]?.workerCalls ?? 0),
    })), visibleScoreboardRows },
    particleCombat: {
      enabled: combat,
      classes: Object.fromEntries([...new Set(measurement.particleSamples.map(sample => sample.classIdentity))].sort((left, right) => left - right).map(identity => {
        const samples = measurement.particleSamples.filter(sample => sample.classIdentity === identity)
        const intervals = samples.slice(1).flatMap((sample, index) => {
          const previous = samples[index]!
          return sample.at - previous.at < 250 ? [sample.at - previous.at] : []
        })
        const particleWorkers = workers.filter(record => {
          if (record.kind !== "particles") return false
          const at = record.started - measurement.started
          const playerClass = measurement.classSwitches.findLast(change => change.at <= at)?.playerClass
            ?? measurement.particleSamples[0]?.classIdentity
          return playerClass === identity
        })
        return [identity, {
          frames: samples.length,
          liveParticles: summarizeDistribution(samples.map(sample => sample.items)),
          frameIntervals: summarizeFrameTimes(intervals),
          particleUploadBytes: samples.reduce((total, sample) => total + sample.particleUploadBytes, 0),
          particleUploadWrites: samples.reduce((total, sample) => total + sample.particleUploadWrites, 0),
          gpuUploadBytes: samples.reduce((total, sample) => total + sample.queueWriteBytes, 0),
          textureWrites: samples.reduce((total, sample) => total + sample.textureWrites, 0),
          particleWorkerInputBytes: particleWorkers.reduce((total, record) => total + record.bytes, 0),
          particleWorkerOutputBytes: particleWorkers.reduce((total, record) => total + (record.receivedBytes ?? 0), 0),
          simultaneousBurstFrames: samples.filter(sample => sample.items > 0).length,
        }]
      })),
      simulation: summarizeDistribution(completed.map(frame => frame.detail.particleWorker)),
      decode: summarizeDistribution(completed.map(frame => frame.detail.particleDecode)),
      renderItems: summarizeDistribution(completed.map(frame => frame.detail.particleItems)),
      batches: summarizeDistribution(completed.map(frame => frame.detail.particleBatches)),
    },
    gpu: {
      ...measurement.counters,
      modelUploads: measurement.modelUploads,
      writes: measurement.queueWrites,
      processCpuSeconds: (processAfter?.processInfo ?? []).map(process => {
        const before = processBefore?.processInfo.find(previous => previous.id === process.id)
        return { id: process.id, type: process.type, seconds: before ? Number((process.cpuTime - before.cpuTime).toFixed(6)) : null }
      }),
      deviceEvidence: measurement.deviceEvidence,
      chromiumDevices: system?.gpu.devices ?? [],
      compositorBackend: system?.gpu.auxAttributes?.displayType ?? system?.gpu.auxAttributes?.glImplementationParts ?? null,
      featureStatus: system?.gpu.featureStatus ?? null,
      timestamps: measurement.gpuTimestamps,
      textureAllocation: measurement.textureAllocation,
      losses: measurement.losses,
      process: {
        id: gpuProcessAfter?.id ?? null,
        cpuMilliseconds: gpuProcessBefore && gpuProcessAfter
          ? Number(((gpuProcessAfter.cpuTime - gpuProcessBefore.cpuTime) * 1_000).toFixed(3))
          : null,
      },
      submissionsPerCompletedFrame: Number((measurement.counters.submissions / Math.max(1, actualFrames)).toFixed(3)),
      commandBuffersPerSubmission: Number((measurement.counters.commandBuffers / Math.max(1, measurement.counters.submissions)).toFixed(3)),
    },
    screen: measurement.screen,
    snapshotTransport: summarizeSnapshotTransport(measurement.snapshotTransport.before, measurement.snapshotTransport.after),
    longAnimationFrames: { ...summarizeDistribution(measurement.longAnimationFrames.map((frame: { duration: number }) => frame.duration)), events: measurement.longAnimationFrames },
    longTasks: { ...summarizeDistribution(measurement.longTasks.map((task: { duration: number }) => task.duration)), events: measurement.longTasks },
    memory: {
      beforeBytes: heapBefore.usedSize,
      afterBytes: heapAfter.usedSize,
      embedderBytes: heapAfter.embedderHeapUsedSize,
      residentBeforeBytes: memoryBefore.residentBytes,
      residentAfterBytes: memoryAfter.residentBytes,
      privateBeforeBytes: memoryBefore.privateBytes,
      privateAfterBytes: memoryAfter.privateBytes,
      wasm: wasmWorkers,
      load: loadPerformance,
      evidence: memoryEvidence,
      tracedGarbageCollection: summarizeDistribution(traceEvents.filter(event => /^(?:MajorGC|MinorGC)$/u.test(event.name ?? "")
        && exactTraceWindow && (event.ts ?? 0) >= exactTraceWindow.startedMicroseconds && (event.ts ?? 0) < exactTraceWindow.endedMicroseconds && event.dur !== undefined).map(event => event.dur! / 1000)),
    },
    frameTails: tails,
    workerIncidents,
    compositorSilence: exactTraceWindow ? summarizeActivePresentationSilence(traceEvents, exactTraceWindow) : null,
    workerEvidence: workerArtifact,
    traveled: Number(measurement.traveled.toFixed(3)), cpu: summarizeCpuProfile(cpuProfile),
    activeCpu: exactTraceWindow ? summarizeCpuProfile(cpuProfile, exactTraceWindow) : null,
    pixels: { nonBlack, beforeSha256: createHash("sha256").update(before).digest("hex"), afterSha256: createHash("sha256").update(after).digest("hex") },
  }
  profilePhases.enter("export")
  await Promise.all([
    writeFile(path.join(directory, `${label}.json`), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, `${evidenceLabel}.measurement.json`), `${JSON.stringify(measurement)}\n`),
    writeFile(path.join(directory, `${label}-before.png`), before),
    writeFile(path.join(directory, `${label}-after.png`), after),
  ])
  if (process.env.PROFILE_UPWARD_SKINNING_PARITY === "1" || process.env.PROFILE_DRAW_LIGHTING_PARITY === "1") {
    // Strictly after sampling and memory extraction; never rerun an unchanged
    // expensive sample to iterate on the differential correctness oracle.
    await auditParity()
  }
  if (acceptance && exerciseClasses) {
    const stock = await acceptStockLoadouts(page, directory, label)
    await writeFile(path.join(directory, `${label}-stock.json`), JSON.stringify(stock, null, 2))
    const losses = await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)
    expect(losses).toEqual([])
  }
  assertVisibleGameplayTruth({ visible: measurement.visible, focused: measurement.focused, ticks: report.simulation.ticks, displayFrames: actualFrames, submissions: measurement.counters.submissions, beforeSha256: report.pixels.beforeSha256, afterSha256: report.pixels.afterSha256 })
  expect(nativeAdmission.filter(value => value.error || (value.occluders as unknown[])?.length)).toEqual([])
  expect(geometry.geometry.samples.some((sample: any) => sample.modelDepth > 0)).toBe(true)
  await testInfo.attach("headed-upward-default-training-bots", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(`PLAYSRC_UPWARD_TRAINING_BOTS ${JSON.stringify({
    label, activeBots: report.activeBots, teams: report.teams,
    animationCallbacks: report.animationCallbacks, completedFrames: report.completedFrames, applicationCompletedFramesPerSecond: report.applicationCompletedFramesPerSecond, compositor: report.compositor,
    compositorSilence: report.compositorSilence, workerEvidence: report.workerEvidence,
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
    frameIntervals: report.frameIntervals, renderSubmissionElapsed: report.renderSubmissionElapsed, presentationDom: report.presentationDom, simulation: report.simulation,
    classSwitches: report.classSwitches,
    snapshotTransport: report.snapshotTransport,
    particleCombat: report.particleCombat,
    worker: Object.fromEntries(Object.entries(worker).map(([kind, value]) => [kind, {
      calls: value.calls, views: value.views, sharedDispatches: value.sharedDispatches, bytes: value.bytes, receivedBytes: value.receivedBytes,
      queue: value.timings.queueMilliseconds ?? null, maximumMilliseconds: value.milliseconds.max,
      transactMaximumMilliseconds: value.timings.transactMilliseconds?.max ?? 0,
    }])),
    gpuSubmissionsPerCompletedFrame: report.gpu.submissionsPerCompletedFrame,
    modelUploads: report.gpu.modelUploads,
    gpuWrites: { calls: report.gpu.queueWriteCalls, bytes: report.gpu.queueWriteBytes, milliseconds: report.gpu.queueWriteMilliseconds, histogram: report.gpu.writes.histogram, phases: report.gpu.writes.phases, processCpuSeconds: report.gpu.processCpuSeconds },
    memory: report.memory, frameTails: { ...tails, frames: tails.frames.slice(0, 10), totalLongFrames: tails.frames.length }, readyMilliseconds, loads: report.loads, totalWallMilliseconds: report.totalWallMilliseconds,
    traveled: report.traveled, longAnimationFrames: report.longAnimationFrames,
    cpu: report.cpu.topSelf.slice(0, 8), pixels: report.pixels,
  })}`)
  if (acceptance) expect(report.teams).toEqual({ red: playerCount / 2, blue: playerCount / 2 })
  assertUpwardProfile(report, { expectedBots, playerCount, classes: exerciseClasses, classPasses: acceptance ? 1 : 2,
    workerRequired: capturePlan.workerCpu === "required",
    sourceUnchanged: sourceFingerprintAfter === sourceFingerprint, workerCaptures: workerCapture.captures,
    compositor: process.env.PROFILE_UPWARD_REQUIRE_COMPOSITOR === "1", smooth: process.env.PROFILE_UPWARD_TRAINING_REQUIRE_SMOOTH === "1" })
  if (process.env.PROFILE_REQUIRE_CLASS_HANDOFF === "1") {
    expect(report.replacement.length).toBe(3)
    expect(report.pipelinePreparation.retainedTemplates).toBeGreaterThan(0)
    expect(report.pipelinePreparation.reusedPreparedModels).toBeGreaterThan(0)
    expect(report.nodeBuilds.filter((build: any) => build.material.includes("/models/player/"))).toEqual([])
  }
  if (process.env.PROFILE_CLASS_UI_AUDIT === "1") {
    await auditEngineerMenus(page, root, directory, label, combatCommand)
  }
  if (process.env.PROFILE_PARTICLE_ORIENTATION_AUDIT === "1") {
    await auditSpriteOrientation(page, directory, label, async file => {
      await checkNativeWindow(nativeReader ? file : undefined)
      if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
    })
  }
  if (process.env.PROFILE_STATIC_PROP_AUDIT === "1") {
    // This independent pixel/depth fixture runs after the complete gameplay
    // sample and its trace export, on the same visible page and checked lease.
    expect(report.nodeBuilds.filter((build: any) => build.material.includes("/models/props_"))).toEqual([])
    const uses = report.pipelinePreparation.firstStaticPropUses
    expect(uses.filter((use: any) => use.generation === 2 && use.pass === "main").length).toBeGreaterThan(0)
    const url = new URL("/static-prop-graph-audit", page.url()).href
    await page.route(url, route => route.fulfill({ contentType: "text/html", body: `<!doctype html><title>Static prop graph pixel/depth equivalence</title><style>body{margin:0;background:#111;color:white}</style><h3>Independent VHV colors and fades: dedicated vs shared material</h3><script type="module">import {createStaticPropGraphProbe} from '/@fs/${repositoryRoot}/packages/presentation/rendering/tests/fixtures/static-prop-graph-probe.ts';window.probe=await createStaticPropGraphProbe();</script>` }))
    await page.goto(url)
    await page.waitForFunction(() => (window as any).probe)
    const results = []
    for (let phase = 0; phase < 4; phase++) {
      const { beforePixels, afterPixels, ...result } = await page.evaluate(phase => (window as any).probe.compare(phase), phase)
      for (const [side, data] of [["before", beforePixels], ["after", afterPixels]]) await writeFile(path.join(directory, `${label}-static-${phase}-${side}.png`), Buffer.from(data.split(",")[1], "base64"))
      await checkNativeWindow(nativeReader ? path.join(directory, `${label}-static-${phase}.desktop.png`) : undefined)
      await writeFile(path.join(directory, `${label}-native-admission.json`), JSON.stringify(nativeRecords()))
      if (nativeReader) requireMacPageAdmission(nativeAdmission.at(-1)!)
      results.push(result)
    }
    await page.screenshot({ path: path.join(directory, `${label}-static-visible.png`) })
    const retiredDraws = await page.evaluate(() => (window as any).probe.dispose())
    await writeFile(path.join(directory, `${label}-static-graphs.json`), JSON.stringify({ results, retiredDraws, nativeAdmission: nativeRecords().slice(-4), performanceSample: false }, null, 2))
    expect(results.map(result => [result.builds, result.newPrograms, result.colorMismatches, result.depthMismatches])).toEqual(Array.from({ length: 4 }, () => [0, 0, 0, 0]))
    expect(retiredDraws).toBe(0)
    expect(nativeAdmission.filter(value => value.error || (value.occluders as unknown[])?.length)).toEqual([])
  }
})
