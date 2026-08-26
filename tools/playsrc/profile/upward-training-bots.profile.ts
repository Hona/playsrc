import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { applicationBuildIdentity } from "../src/build-identity"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { summarizeClassSwitchLifecycle } from "./class-switch-lifecycle"
import { TRACE_START, TRACE_END, analyzeCompositorStalls, assertVisibleGameplayTruth, summarizeCompositorStages, summarizeCompositorTruth, summarizeActivePresentationSilence, type ChromiumTraceEvent } from "./compositor-truth"
import { COMPOSITOR_TRACE_CATEGORIES, TRACE_LIMITS, drainTraceStream, retainCompositorEvidence, retainEvidenceBlob, type TraceJoin } from "./compositor-evidence"
import { attributeFrameTails } from "./frame-tail-attribution"
import { summarizeCpuProfile, summarizeDistribution, type CpuProfile } from "./gameui-profile"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"
import { startWorkerCpuCapture } from "./worker-cpu-profiler"
import { attributeWorkerIncidents } from "./worker-incident-attribution"
import { captureProcessMemory } from "./process-memory"
import { acceptStockLoadouts } from "./stock-loadout-acceptance"

test("profile authored headed Upward offline-practice default roster and actual completed gameplay frames", async ({ page, context, profilePhases }, testInfo) => {
  const wallStarted = Date.now()
  const seconds = profileSampleSeconds()
  const createServer = process.env.PROFILE_STARTUP_CREATE_SERVER === "1"
  const target = createServer ? "ctf_2fort" : "pl_upward"
  const entry = createServer ? "create-server" : "training"
  const exerciseClasses = process.env.PROFILE_UPWARD_CLASS_SWITCH === "1"
  const acceptance = process.env.PROFILE_INTEGRATED_ACCEPTANCE === "1"
  const label = process.env.PROFILE_UPWARD_TRAINING_LABEL ?? "latest"
  const evidenceLabel = `${label}-${wallStarted}`
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY ?? path.join(sourceCacheDir, "profiles", createServer ? "2fort-startup" : "upward-training-bots", crypto.randomUUID())
  await mkdir(directory, { recursive: true })
  const sourceFingerprint = process.env.PLAYSRC_PROFILE_SOURCE_FINGERPRINT ?? await applicationBuildIdentity()
  const sourceCommit = spawnSync("git", ["rev-parse", "HEAD"], { cwd: repositoryRoot, encoding: "utf8" })
  if (sourceCommit.status !== 0) throw new Error("Cannot establish profiler source commit")
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
    await page.bringToFront()
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
      if (process.env.PROFILE_UPWARD_TRAINING_PLAYERS) await playerEntry.fill(process.env.PROFILE_UPWARD_TRAINING_PLAYERS)
      playerCount = Number(await playerEntry.inputValue())
      start = mapPanel.locator("[data-vgui-name='StartOfflinePracticeButton']")
    }
    if (process.env.PROFILE_INTEGRATED_ACCEPTANCE === "1") expect(playerCount).toBe(createServer ? 24 : 16)
    else expect(playerCount).toBeGreaterThanOrEqual(12)
    const mapStarted = Date.now()
    networkStage = `${cache}-map`
    await start.click({ timeout: 5_000 })
    await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 110_000 })
    const teamAdmissionMilliseconds = Date.now() - mapStarted
    await chooseTf2Team(page, process.env.PROFILE_ACCEPTANCE_STOCK_TEAM === "blue" || process.env.PROFILE_INTEGRATED_ACCEPTANCE === "1" && cache === "warm" ? "blue" : "red")
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
  if (process.env.PROFILE_UPWARD_TRAINING_WARM_RELOAD === "1") await loadPractice("warm")
  if (process.env.PROFILE_ACCEPTANCE_STOCK_ONLY === "1") {
    const stock = await acceptStockLoadouts(page, directory, label)
    const losses = await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)
    await writeFile(path.join(directory, `${label}-correctness.json`), JSON.stringify({ headed: true, loads, stock, losses, team: process.env.PROFILE_ACCEPTANCE_STOCK_TEAM, performanceSample: false }, null, 2))
    expect(stock).toHaveLength(27)
    expect(losses).toEqual([])
    return
  }
  const finalLoad = loads.at(-1)!
  const { readyMilliseconds, playerCount, launch } = finalLoad
  profilePhases.enter("pre-sample")
  const expectedBots = playerCount - 1

  const combat = process.env.PROFILE_PARTICLE_COMBAT === "1"
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
  const before = await canvas.screenshot({ timeout: 20_000 })
  if (process.env.PROFILE_UPWARD_TRAINING_INTERACTION === "1" && !exerciseClasses) await canvas.click({ position: { x: 300, y: 250 } })
  const cdp = await context.newCDPSession(page)
  const browserCdp = await context.browser()!.newBrowserCDPSession()
  const system = await browserCdp.send("SystemInfo.getInfo").catch(() => null)
  const processBefore = await browserCdp.send("SystemInfo.getProcessInfo").catch(() => null)
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
  const workerCpu = exerciseClasses || acceptance ? await startWorkerCpuCapture(browserCdp, cdp, page) : undefined
  const nativeScreenshot = process.env.PROFILE_NATIVE_SCREENSHOT === "1" ? path.join(directory, `${evidenceLabel}.desktop.png`) : null
  if (nativeScreenshot) {
    if (process.platform !== "darwin") throw new Error("Native desktop screenshot capture requires the configured macOS capture tool")
    const captured = spawnSync("screencapture", ["-x", nativeScreenshot], { timeout: 5_000 })
    if (captured.status !== 0) throw new Error("Native visible desktop capture failed")
  }
  await cdp.send("Performance.enable")
  await cdp.send("Profiler.enable")
  await cdp.send("HeapProfiler.enable")
  await cdp.send("HeapProfiler.startSampling", { samplingInterval: 65_536, includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true })
  await cdp.send("Profiler.setSamplingInterval", { interval: 1_000 })
  const heapBefore = await cdp.send("Runtime.getHeapUsage")
  await cdp.send("Profiler.start")
  if (!exerciseClasses) await page.keyboard.down("w")
  await browserCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
    traceConfig: { recordMode: "recordUntilFull", traceBufferSizeInKb: TRACE_LIMITS.browserKilobytes, includedCategories: [...COMPOSITOR_TRACE_CATEGORIES] } })
  await workerCpu?.start()
  const performanceBefore = (await cdp.send("Performance.getMetrics").catch(() => ({ metrics: [] }))).metrics
  const clockBefore = performanceBefore.find(metric => metric.name === "Timestamp")?.value
  profilePhases.enter("sample-and-readback")
  const measurementPromise = page.evaluate(async ({ duration, startMark, endMark }) => {
    const main = document.querySelector<HTMLElement>("main")!
    const surface = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const instrumentation = (globalThis as any).__playsrcFrameProfiler
    const firstTick = Number(main.dataset.snapshotTick)
    const firstFrame = Number(surface.dataset.displayFrame)
    const firstPosition = (main.dataset.cameraPosition ?? "").split(",").map(Number)
    const firstUploads = structuredClone((globalThis as any).__playsrcProfile.modelParticleUploads ?? {}) as Record<string, number>
    const firstSnapshots = structuredClone((globalThis as any).__playsrcProfile.snapshotTransport ?? {}) as Record<string, number>
    const started = performance.now()
    performance.mark(startMark, { startTime: started })
    const lifecycle: Array<{ at: number; phase: string; playerClass?: number; key?: string; visible?: boolean }> = []
    const mark = (phase: string, detail: { playerClass?: number; key?: string; visible?: boolean } = {}) => {
      lifecycle.push({ at: Number((performance.now() - started).toFixed(3)), phase, ...detail })
    }
    const keydown = (event: KeyboardEvent) => { if (event.code === "Comma" || /^Digit[1-9]$/u.test(event.code)) mark("key-down", { key: event.code }) }
    const pointerdown = () => mark(document.pointerLockElement === surface ? "weapon-fire" : "pointer-capture")
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
    return {
      elapsed, started, ended, browserLifecycle, firstTick, lastTick: Number(main.dataset.snapshotTick), firstFrame,
      visible: document.visibilityState === "visible", focused: document.hasFocus(), animationCallbacks,
      viewport: { width: innerWidth, height: innerHeight, devicePixelRatio, visualViewportScale: visualViewport?.scale ?? null, canvasWidth: surface.width, canvasHeight: surface.height },
      lastFrame: Number(surface.dataset.displayFrame), traveled: Math.hypot(...position.map((value, index) => value - firstPosition[index]!)),
      roster: structuredClone((globalThis as any).__playsrcProfile.bots), scoreboard: JSON.parse(main.dataset.scoreboardProbe ?? "{}"),
      frames: instrumentation.completedFrames, compositorFrames: instrumentation.compositorFrames, particleSamples,
      presentationCallbacks: instrumentation.animationCallbacks, worker: instrumentation.worker, input: instrumentation.input, counters: instrumentation.counters, queueWrites: instrumentation.queueWrites,
      simulationPublications: instrumentation.simulation, simulationPublicationsDropped: instrumentation.simulationDropped,
      classSwitches, lifecycle,
      dom: { mutations, nodes: document.getElementsByTagName("*").length, hudNodes: hudRoot?.getElementsByTagName("*").length ?? 0,
        panels, rasterImages: document.querySelectorAll("img[data-vgui-raster]").length, rasterCanvases: document.querySelectorAll("canvas[data-vgui-raster]").length,
        accessibility: { hudLabels: hudRoot?.querySelectorAll("[aria-label]").length ?? 0, gameView: surface.getAttribute("aria-label") },
        canvas: { alphaMode: gpuContext?.getConfiguration()?.alphaMode ?? null, format: gpuContext?.getConfiguration()?.format ?? null },
      },
      modelUploads: Object.fromEntries(Object.entries((globalThis as any).__playsrcProfile.modelParticleUploads ?? {})
        .map(([key, value]) => [key, typeof value === "number" ? value - (firstUploads[key] ?? 0) : value])),
      snapshotTransport: Object.fromEntries(Object.entries((globalThis as any).__playsrcProfile.snapshotTransport ?? {})
        .map(([key, value]) => [key, typeof value === "number" ? value - (firstSnapshots[key] ?? 0) : value])),
      capabilities: instrumentation.capabilities, gpuTimestamps: instrumentation.gpuTimestamps, losses: instrumentation.losses,
      gpuOperations: instrumentation.gpuOperations, gpuOperationsDropped: instrumentation.gpuOperationsDropped,
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
  }, { duration: seconds, startMark: TRACE_START, endMark: TRACE_END })
  const exercisedClasses: string[] = []
  let visibleScoreboardRows: number | null = null
  const exercise = async () => {
    if (!exerciseClasses) return
    const classes = ["heavyweapons", "pyro", "medic", "spy", "engineer", "sniper", "scout", "demoman", "soldier"] as const
    const digits = [5, 3, 7, 9, 6, 8, 1, 4, 2] as const
    const identities = [6, 7, 5, 8, 9, 2, 1, 4, 3] as const
    const deadline = Date.now() + seconds * 1000
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
    }), Math.max(0, deadline - Date.now()))
    await page.keyboard.up("Tab")
    for (const [position, playerClass] of (acceptance ? classes : [...classes, ...classes]).entries()) {
      const index = position % classes.length
      if (Date.now() >= deadline) break
      if (acceptance && position > 0 && position % 4 === 0) await page.waitForTimeout(2100)
      await page.keyboard.press("Comma")
      if (!await root.evaluate((element, timeout) => new Promise<boolean>(resolve => {
        const started = performance.now()
        const poll = () => element.dataset.classSelectionVisible === "true" ? resolve(true)
          : performance.now() - started >= timeout ? resolve(false) : requestAnimationFrame(poll)
        poll()
      }), Math.max(0, deadline - Date.now()))) break
      await page.keyboard.press(`Digit${digits[index]}`)
      if (!await root.evaluate((element, { identity, timeout }) => new Promise<boolean>(resolve => {
        const started = performance.now()
        const poll = () => (element.dataset.hudProbe ?? "").split(":")[1] === String(identity) ? resolve(true)
          : performance.now() - started >= timeout ? resolve(false) : requestAnimationFrame(poll)
        poll()
      }), { identity: identities[index], timeout: Math.max(0, deadline - Date.now()) })) break
      await page.mouse.click(Math.round(page.viewportSize()!.width / 2), Math.round(page.viewportSize()!.height / 2))
      if (acceptance) {
        await expect(root).toHaveAttribute("data-pointer-locked", "true", { timeout: 2000 })
        await page.mouse.down()
        await page.waitForTimeout(100)
        await page.mouse.up()
      }
      exercisedClasses.push(playerClass)
    }
  }
  const interaction = process.env.PROFILE_UPWARD_TRAINING_INTERACTION === "1" && !exerciseClasses
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
  const sample = await Promise.all([measurementPromise, exercise(), interaction, combatActions])
    .then(values => ({ measurement: values[0], error: null }), error => ({ measurement: null, error: String(error) }))
  profilePhases.enter("trace-drain")
  // Stop the real Worker sampler before ending the trace so its end clock mark
  // remains joinable. Failure here must not discard the native browser trace.
  const workerCapture = await (workerCpu?.stop() ?? Promise.resolve([]))
    .then(captures => ({ captures, error: null }), error => ({ captures: [], error: String(error) }))
  await workerCpu?.close().catch(() => undefined)
  const performanceAfter = (await cdp.send("Performance.getMetrics").catch(() => ({ metrics: [] }))).metrics
  const clockAfter = performanceAfter.find(metric => metric.name === "Timestamp")?.value
  await browserCdp.send("Tracing.end")
  // Stop samplers now, but a detached renderer or failed optional sampler must not discard the browser trace.
  const supplemental = Promise.all([cdp.send("Profiler.stop"), cdp.send("HeapProfiler.stopSampling"),
    cdp.send("Runtime.getHeapUsage"), browserCdp.send("SystemInfo.getProcessInfo")])
    .then(value => ({ value, error: null }), error => ({ value: null, error: String(error) }))
  if (!exerciseClasses) await page.keyboard.up("w").catch(() => undefined)
  const completion = await traceFinished
  const raw = completion.stream ? await drainTraceStream(browserCdp, completion.stream) : { bytes: new Uint8Array(), complete: false }
  const joins: TraceJoin[] = []
  const measured = sample.measurement
  if (measured) {
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
  const sourceFingerprintAfter = await applicationBuildIdentity()
  profilePhases.enter("trace-analysis-retention")
  let workerBytes = Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", ...workerCapture, unsampledTargets: workerCpu?.unsampledTargets ?? [] }))
  if (workerBytes.byteLength > TRACE_LIMITS.probeBytes) {
    workerCapture.error = "Worker CPU evidence exceeds its byte bound"
    workerBytes = Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", captures: [], error: workerCapture.error }))
  }
  const workerArtifact = await retainEvidenceBlob(path.join(directory, "compositor-evidence"), workerBytes, "workers.json")
  const evidence = await retainCompositorEvidence({ directory: path.join(directory, "compositor-evidence"), raw: raw.bytes,
    complete: raw.complete, dataLossOccurred: completion.dataLossOccurred,
    identity: { sourceCommit: sourceCommit.stdout.trim(), sourceFingerprint, sourceFingerprintAfter,
      sourceUnchanged: sourceFingerprint === sourceFingerprintAfter, applicationGeneration, browserVersion,
      gpu: system?.gpu ?? null, availableCategories, viewport: measured?.viewport ?? null,
      origin: new URL(page.url()).origin, localProductionBundle, label, headed: true, target, launch,
      sampleError: sample.error, workerCpu: workerArtifact, nativeScreenshot, beforeScreenshotSha256: createHash("sha256").update(before).digest("hex") },
    probes: { started: measured?.started ?? 0, ended: measured?.ended ?? 0, joins, dropped: measured ? measured.gpuOperationsDropped + measured.simulationPublicationsDropped : 1 } })
  // Reference durable evidence before subsequent CPU/heap extraction, screenshots, or assertions can fail.
  await testInfo.attach("compositor-evidence", { body: JSON.stringify(evidence.artifact), contentType: "application/json" })
  console.log(`PLAYSRC_COMPOSITOR_EVIDENCE ${JSON.stringify(evidence.artifact)}`)
  profilePhases.enter("diagnostics-and-pixels")
  if (workerCapture.error) throw new Error(`Worker CPU capture failed; raw compositor evidence retained: ${workerCapture.error}`)
  if (!measured) throw new Error(`Gameplay sampling failed; compositor evidence retained: ${sample.error}`)
  const collected = await supplemental
  if (!collected.value) throw new Error(`Optional profiling extraction failed; compositor evidence retained: ${collected.error}`)
  const [cpuResult, allocationResult, heapAfter, processAfter] = collected.value
  const cpuProfile = cpuResult.profile as CpuProfile
  const allocationProfile = allocationResult.profile
  const memoryAfter = await captureProcessMemory(processAfter.processInfo, { remote: Boolean(process.env.PLAYSRC_PROFILE_CDP_ENDPOINT) })
  const measurement = measured
  const traceEvents: ChromiumTraceEvent[] = evidence.events
  const exactTraceWindow = evidence.manifest.analysis.window
  const allocations = (node: { selfSize: number; children: any[] }): number => node.selfSize + node.children.reduce((total, child) => total + allocations(child), 0)
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
    schema: "playsrc-tf2-upward-training-bots-profile-v1", label, headed: true, target, entry, launch,
    sourceFingerprint,
    roster: measurement.roster.map((bot: any) => ({ identity: bot.identity, class: bot.class, team: bot.team, difficulty: bot.difficulty })),
    activeBots: measurement.roster.length, teams: { red: measurement.scoreboard.red.playerCount, blue: measurement.scoreboard.blue.playerCount },
    elapsedMilliseconds: Number(measurement.elapsed.toFixed(3)), readyMilliseconds, loads, totalWallMilliseconds: Date.now() - wallStarted,
    animationCallbacks: measurement.animationCallbacks, completedFrames: actualFrames, applicationCompletedFramesPerSecond: Number((actualFrames / measurement.elapsed * 1000).toFixed(3)),
    compositor: { ...summarizeCompositorTruth(exactTraceWindow ? traceEvents : [], measurement.elapsed, exactTraceWindow ?? undefined),
      stages: summarizeCompositorStages(exactTraceWindow ? traceEvents : [], exactTraceWindow ?? undefined),
      stalls: exactTraceWindow ? analyzeCompositorStalls(traceEvents, exactTraceWindow, measurement.lifecycle) : [] },
    compositorIncludingSetupAndCollection: clockBefore !== undefined && clockAfter !== undefined && clockAfter > clockBefore ? {
      elapsedMilliseconds: (clockAfter - clockBefore) * 1_000,
      ...summarizeCompositorTruth(traceEvents, (clockAfter - clockBefore) * 1_000, { startedMicroseconds: clockBefore * 1_000_000, endedMicroseconds: clockAfter * 1_000_000 }),
    } : null,
    compositorEvidence: { ...evidence.artifact, complete: evidence.manifest.complete, errors: evidence.manifest.errors,
      issues: evidence.manifest.analysis.issues, incidents: evidence.manifest.analysis.incidents.map(({ work, joins, ...incident }) => incident) },
    presentationOpportunities:{frames:compositor.length,framesPerSecond:Number((compositor.length/measurement.elapsed*1000).toFixed(3)),animationCallbacks:measurement.presentationCallbacks.length,intervals:summarizeFrameTimes(compositor.slice(1).map((frame,index)=>frame.at-compositor[index]!.at)),submissionLatency:summarizeDistribution(compositor.map(frame=>frame.submissionMilliseconds))},
    settings: await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.videoQuality)),
    browser: { platform: await page.evaluate(() => navigator.platform), userAgent: await page.evaluate(() => navigator.userAgent), controllerPlatform: process.platform, origin: new URL(page.url()).origin, localProductionBundle, channel: process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? "playwright-chromium", viewport: measurement.viewport, visible: measurement.visible, focused: measurement.focused, lifecycle: measurement.browserLifecycle, gpu: system?.gpu ?? null, processes: { before: processBefore?.processInfo ?? null, after: processAfter?.processInfo ?? null, memoryBefore, memoryAfter }, network, storage, userMachineEvidence: false },
    firstPlayableBoundary: "application-completed-frame-not-compositor",
    frameIntervals: summarizeFrameTimes(intervals), frameWork: summarizeFrameTimes(completed.map(frame => frame.detail.total)),
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
    botWork: summarizeDistribution(completed.map(frame => frame.detail.models)), worker,
    classSwitches: { requested: exercisedClasses, lifecycle: measurement.lifecycle, timing: summarizeClassSwitchLifecycle(measurement.lifecycle), observed: measurement.classSwitches.map((item, index, values) => ({
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
      adapter: measurement.capabilities.adapter ?? null,
      chromiumDevices: system?.gpu.devices ?? [],
      backend: system?.gpu.auxAttributes?.displayType ?? system?.gpu.auxAttributes?.glImplementationParts ?? null,
      featureStatus: system?.gpu.featureStatus ?? null,
      timestamps: measurement.gpuTimestamps,
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
    snapshotTransport: measurement.snapshotTransport,
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
      sampledAllocationBytesIncludingCollected: allocations(allocationProfile.head),
      tracedGarbageCollection: summarizeDistribution(traceEvents.filter(event => /^(?:MajorGC|MinorGC)$/u.test(event.name ?? "")
        && exactTraceWindow && (event.ts ?? 0) >= exactTraceWindow.startedMicroseconds && (event.ts ?? 0) < exactTraceWindow.endedMicroseconds && event.dur !== undefined).map(event => event.dur! / 1000)),
    },
    frameTails: tails,
    workerIncidents,
    compositorSilence: exactTraceWindow ? summarizeActivePresentationSilence(traceEvents, exactTraceWindow) : null,
    workerEvidence: workerArtifact,
    traveled: Number(measurement.traveled.toFixed(3)), cpu: summarizeCpuProfile(cpuProfile),
    pixels: { nonBlack, beforeSha256: createHash("sha256").update(before).digest("hex"), afterSha256: createHash("sha256").update(after).digest("hex") },
  }
  profilePhases.enter("export")
  await Promise.all([
    writeFile(path.join(directory, `${label}.json`), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, `${label}.cpuprofile`), `${JSON.stringify(cpuProfile)}\n`),
    writeFile(path.join(directory, `${evidenceLabel}.measurement.json`), `${JSON.stringify(measurement)}\n`),
    writeFile(path.join(directory, `${label}-before.png`), before),
    writeFile(path.join(directory, `${label}-after.png`), after),
  ])
  if (process.env.PROFILE_INTEGRATED_ACCEPTANCE === "1" && exerciseClasses) {
    const stock = await acceptStockLoadouts(page, directory, label)
    await writeFile(path.join(directory, `${label}-stock.json`), JSON.stringify(stock, null, 2))
    const losses = await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)
    expect(losses).toEqual([])
  }
  assertVisibleGameplayTruth({ visible: measurement.visible, focused: measurement.focused, ticks: report.simulation.ticks, displayFrames: actualFrames, submissions: measurement.counters.submissions, beforeSha256: report.pixels.beforeSha256, afterSha256: report.pixels.afterSha256 })
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
    frameIntervals: report.frameIntervals, frameWork: report.frameWork, presentationDom: report.presentationDom, simulation: report.simulation,
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
  expect(report.activeBots).toBe(expectedBots)
  expect(report.teams.red + report.teams.blue).toBe(playerCount)
  if (process.env.PROFILE_INTEGRATED_ACCEPTANCE === "1") expect(report.teams).toEqual({ red: playerCount / 2, blue: playerCount / 2 })
  expect(report.completedFrames).toBeGreaterThan(0)
  expect(report.pixels.nonBlack).toBeGreaterThan(20_000)
  if (exerciseClasses) {
    expect(report.classSwitches.requested.length).toBe(acceptance ? 9 : 18)
    expect(report.classSwitches.observed.length).toBe(acceptance ? 9 : 18)
    expect(report.classSwitches.timing.length).toBe(report.classSwitches.observed.length)
    if (acceptance) expect(report.classSwitches.timing.every(item => item.fireAt !== null)).toBe(true)
    expect(report.classSwitches.timing.filter(item => item.admission === "first").length).toBe(9)
    expect(report.classSwitches.timing.filter(item => item.admission === "retained").length).toBe(acceptance ? 0 : 9)
    expect(report.classSwitches.visibleScoreboardRows).toBe(playerCount)
    expect(report.simulation.hertz).toBeGreaterThanOrEqual(60)
    expect(report.compositor.intervals?.maximumMilliseconds).toBeLessThan(250)
    expect(report.compositorSilence?.maximumActiveSilenceMilliseconds).toBeLessThan(250)
    expect(workerCapture.captures).toHaveLength(1)
    expect(workerCapture.captures[0]!.deadlineStopped).toBe(false)
    expect(workerIncidents[0]!.samples).toBeGreaterThan(0)
    expect(workerIncidents[0]!.captureComplete).toBe(true)
  }
  expect(report.screen.visibility).toBe("visible")
  expect(report.gpu.losses).toHaveLength(0)
  expect(report.compositorEvidence.complete).toBe(true)
  expect(sourceFingerprintAfter).toBe(sourceFingerprint)
  if (process.env.PROFILE_UPWARD_REQUIRE_COMPOSITOR === "1") expect(report.compositor.evidence).toBe("chromium-compositor-presentation-trace")
  if (process.env.PROFILE_UPWARD_TRAINING_REQUIRE_SMOOTH === "1") {
    expect(report.applicationCompletedFramesPerSecond).toBeGreaterThanOrEqual(55)
    expect(report.simulation.hertz).toBeGreaterThanOrEqual(60)
    expect(report.frameIntervals.p95Milliseconds).toBeLessThan(35)
  }
})
