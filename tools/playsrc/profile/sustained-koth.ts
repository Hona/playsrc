import type { CDPSession, Page } from "@playwright/test"
import { writeFile } from "node:fs/promises"
import path from "node:path"
import { deliveryTimeline, installDeliveryObserver } from "./frame-delivery"
import { captureProcessMemory } from "./process-memory"
import { sustainedWorkerMemory } from "./sustained-worker-memory"
import { drainTraceStream, retainCompositorEvidence } from "./compositor-evidence"
import { TRACE_START, TRACE_END, summarizeCompositorTruth } from "./compositor-truth"
import { SUSTAINED_KOTH, requireSustainedBudget, checkSustainedObservation, sustainedTrends, sustainedGcEvidence, liveSustainedAttachments } from "./sustained-koth-evidence"
export { requireSustainedBudget } from "./sustained-koth-evidence"

async function stopLightTrace(browser: CDPSession) {
  let timer: ReturnType<typeof setTimeout> | undefined
  const completion = new Promise<{ stream?: string; dataLossOccurred?: boolean }>((resolve, reject) => {
    browser.once("Tracing.tracingComplete", resolve)
    timer = setTimeout(() => reject(new Error("KOTH lightweight trace completion exceeded 10 seconds")), 10_000)
  })
  try {
    await browser.send("Tracing.end")
    const complete = await completion
    if (!complete.stream) throw new Error("KOTH lightweight trace stream unavailable")
    return { complete, raw: await drainTraceStream(browser, complete.stream, SUSTAINED_KOTH.traceBytes) }
  } finally { clearTimeout(timer) }
}

/** Keep the actual Ready-to-live frames when the later 90-second admission
 * fails. This is a labelled prelude, never counted toward the required soak. */
async function observePrelude(options: Parameters<typeof observeSustainedKoth>[0]) {
  const { page, browserCdp, directory, checkNativeWindow } = options
  const categories = [...SUSTAINED_KOTH.categories]
  await page.evaluate(installDeliveryObserver)
  await checkNativeWindow()
  await browserCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
    traceConfig: { recordMode: "recordUntilFull", traceBufferSizeInKb: SUSTAINED_KOTH.traceKilobytes, includedCategories: categories } })
  await page.evaluate(start => { const at = performance.now(); (globalThis as any).__playsrcDeliveryObserver.start(at); performance.mark(start, { startTime: at }) }, TRACE_START)
  let pending = Promise.resolve(), busy = false, failure: unknown
  let rejectNative!: (error: unknown) => void
  const nativeFailure = new Promise<never>((_, reject) => { rejectNative = reject })
  const monitor = setInterval(() => {
    if (busy) return
    busy = true
    pending = checkNativeWindow().catch(error => { failure ??= error; rejectNative(error) }).finally(() => { busy = false })
  }, 500)
  try {
    await Promise.race([nativeFailure, page.waitForFunction(() => { const p = (globalThis as any).__playsrcProfile
      return p.round?.state === 4 && !p.round.waitingForPlayers && !p.round.inSetup && p.bots?.length === 23
    }, undefined, { timeout: 45_000 })])
  } catch (error) { failure = error }
  finally {
    clearInterval(monitor); await pending
    const sample = await page.evaluate(end => { const at = performance.now(); performance.mark(end, { startTime: at }); return (globalThis as any).__playsrcDeliveryObserver.stop(at) }, TRACE_END)
    const { complete, raw } = await stopLightTrace(browserCdp)
    const evidence = await retainCompositorEvidence({ directory, raw: raw.bytes, complete: raw.complete && !failure,
      dataLossOccurred: Boolean(complete.dataLossOccurred), categories,
      identity: { sourceCommit: options.sourceCommit, sourceFingerprint: options.sourceFingerprint, scope: "Ready-to-live prelude only; not the 90-second sustained sample" },
      probes: { started: sample.started, ended: sample.ended, joins: sample.frames.map((frame: any) => ({ kind: "completed-submission", at: frame.at })), dropped: sample.dropped + sample.missedPublications } })
    await writeFile(path.join(directory, "sustained-prelude.json"), JSON.stringify({ sample, evidence: evidence.artifact,
      complete: evidence.manifest.complete, failure: failure ? String(failure) : null, sustainedAcceptance: false,
      completed: deliveryTimeline(sample.started, sample.ended, sample.frames.map((frame: any) => frame.at)),
      raf: deliveryTimeline(sample.started, sample.ended, sample.raf),
      compositor: summarizeCompositorTruth(evidence.events, sample.ended - sample.started, evidence.analysis.window ?? undefined),
      gc: sustainedGcEvidence(evidence.events, evidence.analysis.window, evidence.manifest.complete),
      scope: "All observed prelude gaps retained, including500–1500ms stalls and zero buckets. Producer tick is the latest DOM publication at submission, not a displayed-tick or Worker-progress certificate. Presented content identity/input-to-photon remain separate evidence." }))
    if (!evidence.manifest.complete && !failure) failure = new Error("KOTH prelude trace incomplete")
    if (sample.dropped || sample.missedPublications || sample.lifecycle.length) failure ??= new Error("KOTH prelude observation interrupted or overflowed")
  }
  if (failure) throw failure
}

/** One live map generation; no forced collection, restart, or clock changes.
 * Full instrumentation stays inactive until the existing late deep sample. */
export async function observeSustainedKoth(options: {
  page: Page; browserCdp: CDPSession; directory: string; checkNativeWindow: () => Promise<void>;
  sourceFingerprint: string; sourceCommit: string
  diagnosticSeconds?: 15
}) {
  const { page, browserCdp, directory, checkNativeWindow } = options
  const file = (name: string) => path.join(directory, `sustained-${name}`)
  const records: any[] = [], memory: any[] = []
  const transitions: ReturnType<typeof checkSustainedObservation>[] = []
  const seconds = options.diagnosticSeconds ?? SUSTAINED_KOTH.seconds
  const budget = () => {
    const remaining = Number(process.env.PLAYSRC_PROFILE_DEADLINE) - Date.now()
    if (!options.diagnosticSeconds) requireSustainedBudget(remaining)
    else if (!Number.isFinite(remaining) || remaining < 45_000) throw new Error(`Freeze diagnosis needs45000ms for15s context,5s deep sample and retention; only ${remaining}ms remain`)
  }
  const read = () => page.evaluate(() => {
    const p = (globalThis as any).__playsrcProfile, main = document.querySelector<HTMLElement>("main")!
    return { at: performance.now(), epoch: performance.timeOrigin + performance.now(), generation: main.dataset.generation,
      tick: main.dataset.snapshotTick, round: p.round, bots: p.bots, points: p.controlPoints,
      camera: p.displacementCameraOverride ?? p.player?.camera, quality: p.videoQuality,
      heap: (performance as any).memory ? { usedJSHeapSize: (performance as any).memory.usedJSHeapSize,
        totalJSHeapSize: (performance as any).memory.totalJSHeapSize, jsHeapSizeLimit: (performance as any).memory.jsHeapSizeLimit } : null,
      gpuApi: (globalThis as any).__playsrcGpuTextureAccounting, assets: p.memoryAssets,
      ownership: p.rendererOwnership?.() ?? null,
      failures: p.failure, losses: (globalThis as any).__playsrcFrameProfiler.losses, audio: p.audio?.stats() }
  })
  const retain = () => writeFile(file("history.json"), JSON.stringify({ records, memory, transitions, seconds, sustainedAcceptance: !options.diagnosticSeconds,
    scope: "One-second logical texture/API, JS heap, roster and resource observations. Missed seconds remain gaps, never interpolated. API live bytes are not physical GPU residency; heap drops alone do not prove GC. No long-term leak-freedom claim." }))
  const processSnapshot = async () => {
    const record: any = { at: Date.now(), phase: "process-info" }
    memory.push(record); await retain()
    try {
      const processes = await browserCdp.send("SystemInfo.getProcessInfo")
      record.phase = "process-memory"; await retain()
      record.processes = await captureProcessMemory(processes.processInfo)
      record.phase = "worker-memory"; await retain()
      record.workers = await sustainedWorkerMemory(page.workers())
      record.phase = "complete"
    } catch (error) { record.error = String(error); throw error }
    finally { record.ended = Date.now(); await retain() }
  }
  // Preserve waiting-for-players and setup. Admission is not included in the
  // 90 seconds and cannot be accelerated to make the workflow fit its cap.
  await observePrelude(options)
  records.push(await read()); await retain()
  budget()
  // Observe the authored objective from a stable test camera without moving,
  // freezing, replacing or retiring any player/bot/effect entity.
  if (!options.diagnosticSeconds) await page.evaluate(() => {
    const p = (globalThis as any).__playsrcProfile, point = p.controlPoints.points[0]
    if (!point) throw new Error("KOTH objective camera is unavailable")
    p.displacementCameraOverride = { ...p.player.camera,
      position: [point.position[0] - 300, point.position[1], point.position[2] + 160], yawDegrees: 0, pitchDegrees: 20 }
  })
  await page.evaluate(installDeliveryObserver)
  await checkNativeWindow()
  await page.locator("canvas.world-canvas").screenshot({ path: file("early.png") })
  await processSnapshot()
  budget()
  const categories = [...SUSTAINED_KOTH.categories]
  const available = (await browserCdp.send("Tracing.getCategories")).categories
  if (categories.some(category => !available.includes(category))) throw new Error("Native sustained display/GC trace categories unavailable")
  await browserCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
    traceConfig: { recordMode: "recordUntilFull", traceBufferSizeInKb: SUSTAINED_KOTH.traceKilobytes, includedCategories: categories } })
  let failure: unknown, sample: any
  const started = await page.evaluate(start => { const at = performance.now(); (globalThis as any).__playsrcDeliveryObserver.start(at); performance.mark(start, { startTime: at }); return at }, TRACE_START)
  try {
    for (let index = 0; index < SUSTAINED_KOTH.historyRecords - 1; index++) {
      await checkNativeWindow()
      const previous = records.at(-1)!, state = await read(); records.push(state)
      const transition = checkSustainedObservation(state, records[0], previous)
      if (transition.classes.length || transition.counterResets.length) transitions.push(transition)
      if (index % 5 === 0) await retain()
      // OS process snapshots stay outside active gameplay. Spawning a shell
      // halfway through would contaminate the interval being measured.
      if (state.at - started >= seconds * 1000) break
      await page.waitForTimeout(Math.min(1000, seconds * 1000 - (state.at - started)))
    }
  } catch (error) { failure = error }
  finally {
    sample = await page.evaluate(end => { const at = performance.now(); performance.mark(end, { startTime: at }); return (globalThis as any).__playsrcDeliveryObserver.stop(at) }, TRACE_END)
    const { complete, raw } = await stopLightTrace(browserCdp)
    const evidence = await retainCompositorEvidence({ directory, raw: raw.bytes, complete: raw.complete, dataLossOccurred: Boolean(complete.dataLossOccurred), categories,
      identity: { sourceCommit: options.sourceCommit, sourceFingerprint: options.sourceFingerprint, instrumentation: "Bounded display/GC events only; no CPU/allocation sampler during soak" },
      probes: { started: sample.started, ended: sample.ended, joins: sample.frames.map((frame: any) => ({ kind: "completed-submission", at: frame.at })), dropped: sample.dropped + sample.missedPublications } })
    await writeFile(file("delivery.json"), JSON.stringify({ sample, seconds, sustainedAcceptance: !options.diagnosticSeconds, evidence: evidence.artifact, complete: evidence.manifest.complete,
      completed: deliveryTimeline(sample.started, sample.ended, sample.frames.map((frame: any) => frame.at)), raf: deliveryTimeline(sample.started, sample.ended, sample.raf),
      compositor: summarizeCompositorTruth(evidence.events, sample.ended - sample.started, evidence.analysis.window ?? undefined),
      nativeDelivery: evidence.analysis, gc: sustainedGcEvidence(evidence.events, evidence.analysis.window, evidence.manifest.complete),
      trends: failure ? null : sustainedTrends(records, sample.started, sample.ended),
      changingPresentedFrames: null, changingPixelEvidence: "Not established by display events or two endpoint screenshots; requires native changing-surface evidence",
      failure: failure ? String(failure) : null }))
    await processSnapshot(); await retain()
    if (!evidence.manifest.complete) throw new Error("Sustained presentation evidence incomplete")
  }
  if (failure) throw failure
  if (sample.ended - sample.started < seconds * 1000 || sample.dropped || sample.missedPublications || sample.lifecycle.length) throw new Error("KOTH window incomplete or interrupted")
  const trend = sustainedTrends(records, sample.started, sample.ended)
  if (!(trend.whole.shots! > 0 && (trend.whole.hits! > 0 || trend.whole.deaths! > 0))) throw new Error("Sustained KOTH lacks observed real combat activity")
  await checkNativeWindow()
  await page.locator("canvas.world-canvas").screenshot({ path: file("late.png") })
}

/** Ordinary Disconnect retires the renderer, not just a map's texture set.
 * Run strictly after the uninterrupted soak and late sample/trace collection. */
export async function retireSustainedKoth(page: Page, directory: string, checkNativeWindow: () => Promise<void>, label = "sustained") {
  const snapshot = () => page.evaluate(() => ({ at: performance.now(),
    accounting: structuredClone((globalThis as any).__playsrcGpuTextureAccounting),
    owners: structuredClone((globalThis as any).__playsrcTextureOwners),
    losses: structuredClone((globalThis as any).__playsrcFrameProfiler.losses) }))
  await checkNativeWindow()
  const before = await snapshot(), old = new Set(liveSustainedAttachments(before.owners.records).map(record => record.id))
  const root = page.locator("main")
  if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.keyboard.press("Escape")
  await page.locator('[data-vgui-name="DisconnectButton"]').click({ timeout: 5000 })
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase === "MainMenu", undefined, { timeout: 10000 })
  const after = await snapshot(), survivors = liveSustainedAttachments(after.owners.records).filter(record => old.has(record.id))
  await writeFile(path.join(directory, `${label}-retirement.json`), JSON.stringify({ before, after, survivors,
    scope: "Actual native API create/destroy calls for render attachments across ordinary Disconnect; new MainMenu allocations excluded by identity. Logical API bytes, not physical residency or inferred GC." }))
  await checkNativeWindow()
  await page.screenshot({ path: path.join(directory, `${label}-disconnected.png`) })
  if (before.owners.dropped || after.owners.dropped) throw new Error("Native attachment lifetime evidence overflowed")
  if (after.losses.length) throw new Error("Native attachment retirement reported a GPU failure")
  if (!old.size || survivors.length) throw new Error(`Native attachment retirement incomplete: ${survivors.length}/${old.size} old attachments remain`)
}
