import type { CDPSession, Page } from "@playwright/test"
import { writeFile } from "node:fs/promises"
import path from "node:path"
import { deliveryTimeline, installDeliveryObserver } from "./frame-delivery"
import { captureProcessMemory } from "./process-memory"
import { drainTraceStream, retainCompositorEvidence } from "./compositor-evidence"
import { TRACE_START, TRACE_END, summarizeCompositorTruth } from "./compositor-truth"

export function requireSustainedBudget(remaining: number) {
  // Do not silently shorten the soak or spend the deep-sample/retention budget.
  if (!Number.isFinite(remaining) || remaining < 120_000) throw new Error(`Sustained KOTH needs 120000 ms after live admission (90000 uninterrupted + deep sample/retention); only ${remaining} ms remain`)
}

/** One live map generation; no forced collection, restart, or clock changes.
 * Full instrumentation stays inactive until the existing late deep sample. */
export async function observeSustainedKoth(options: {
  page: Page; browserCdp: CDPSession; directory: string; checkNativeWindow: () => Promise<void>;
  sourceFingerprint: string; sourceCommit: string
}) {
  const { page, browserCdp, directory, checkNativeWindow } = options
  const file = (name: string) => path.join(directory, `sustained-${name}`)
  const records: any[] = [], memory: any[] = []
  const read = () => page.evaluate(() => {
    const p = (globalThis as any).__playsrcProfile, main = document.querySelector<HTMLElement>("main")!
    return { at: performance.now(), epoch: performance.timeOrigin + performance.now(), generation: main.dataset.generation,
      tick: main.dataset.snapshotTick, round: p.round, bots: p.bots, points: p.controlPoints,
      camera: p.player?.camera, quality: p.videoQuality, heap: (performance as any).memory ?? null,
      gpuApi: (globalThis as any).__playsrcGpuTextureAccounting, assets: p.memoryAssets,
      failures: p.failure, losses: (globalThis as any).__playsrcFrameProfiler.losses, audio: p.audio?.stats() }
  })
  const retain = () => writeFile(file("history.json"), JSON.stringify({ records, memory,
    scope: "One-second logical texture/API, JS heap, roster and resource observations. Missed seconds remain gaps, never interpolated. API live bytes are not physical GPU residency; heap drops alone do not prove GC. No long-term leak-freedom claim." }))
  const processSnapshot = async () => {
    const processes = await browserCdp.send("SystemInfo.getProcessInfo")
    memory.push({ at: Date.now(), processes: await captureProcessMemory(processes.processInfo),
      workers: await Promise.all(page.workers().map(worker => worker.evaluate(() => ({ url: location.href,
        memory: (globalThis as any).__playsrcWorkerMemory ?? null })).catch(error => ({ error: String(error) })))) })
  }
  // Preserve waiting-for-players and setup. Admission is not included in the
  // 90 seconds and cannot be accelerated to make the workflow fit its cap.
  await page.waitForFunction(() => { const p = (globalThis as any).__playsrcProfile
    return p.round?.state === 4 && !p.round.waitingForPlayers && !p.round.inSetup && p.bots?.length === 23
  }, undefined, { timeout: 45_000 })
  records.push(await read()); await retain()
  requireSustainedBudget(Number(process.env.PLAYSRC_PROFILE_DEADLINE) - Date.now())
  // Observe the authored objective from a stable test camera without moving,
  // freezing, replacing or retiring any player/bot/effect entity.
  await page.evaluate(() => {
    const p = (globalThis as any).__playsrcProfile, point = p.controlPoints.points[0]
    if (!point) throw new Error("KOTH objective camera is unavailable")
    p.displacementCameraOverride = { ...p.player.camera,
      position: [point.position[0] - 300, point.position[1], point.position[2] + 160], yawDegrees: 0, pitchDegrees: 20 }
  })
  await page.evaluate(installDeliveryObserver)
  await checkNativeWindow()
  await page.locator("canvas.world-canvas").screenshot({ path: file("early.png") })
  await processSnapshot()
  const categories = ["disabled-by-default-display.framedisplayed", "blink.user_timing", "v8.gc"]
  const available = (await browserCdp.send("Tracing.getCategories")).categories
  if (categories.some(category => !available.includes(category))) throw new Error("Native sustained display/GC trace categories unavailable")
  await browserCdp.send("Tracing.start", { transferMode: "ReturnAsStream", streamFormat: "json", streamCompression: "gzip",
    traceConfig: { recordMode: "recordUntilFull", traceBufferSizeInKb: 8192, includedCategories: categories } })
  let failure: unknown, sample: any
  const started = await page.evaluate(start => { const at = performance.now(); (globalThis as any).__playsrcDeliveryObserver.start(at); performance.mark(start, { startTime: at }); return at }, TRACE_START)
  try {
    for (let index = 0; index < 92; index++) {
      await checkNativeWindow()
      const state = await read(); records.push(state)
      if (state.bots.length !== 23 || state.generation !== records[0].generation || state.round.state !== 4 || state.round.waitingForPlayers || state.round.inSetup) throw new Error("Sustained active full roster/generation was interrupted")
      if (state.failures || state.losses.length) throw new Error("Sustained rendering reported a resource failure")
      if (index % 5 === 0) await retain()
      if (index === 45) await processSnapshot()
      if (state.at - started >= 90_000) break
      await page.waitForTimeout(Math.min(1000, 90_000 - (state.at - started)))
    }
  } catch (error) { failure = error }
  finally {
    sample = await page.evaluate(end => { const at = performance.now(); performance.mark(end, { startTime: at }); return (globalThis as any).__playsrcDeliveryObserver.stop(at) }, TRACE_END)
    const completion = new Promise<{ stream?: string; dataLossOccurred?: boolean }>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Sustained trace completion exceeded 10 seconds")), 10_000)
      browserCdp.once("Tracing.tracingComplete", value => { clearTimeout(timer); resolve(value) })
    })
    await browserCdp.send("Tracing.end")
    const complete = await completion
    if (!complete.stream) throw new Error("Sustained display trace stream unavailable")
    const raw = await drainTraceStream(browserCdp, complete.stream)
    const evidence = await retainCompositorEvidence({ directory, raw: raw.bytes, complete: raw.complete, dataLossOccurred: Boolean(complete.dataLossOccurred), categories,
      identity: { sourceCommit: options.sourceCommit, sourceFingerprint: options.sourceFingerprint, instrumentation: "Bounded display/GC events only; no CPU/allocation sampler during soak" },
      probes: { started: sample.started, ended: sample.ended, joins: sample.frames.map((frame: any) => ({ kind: "completed-submission", at: frame.at })), dropped: sample.dropped + sample.missedPublications } })
    await writeFile(file("delivery.json"), JSON.stringify({ sample, evidence: evidence.artifact, complete: evidence.manifest.complete,
      completed: deliveryTimeline(sample.started, sample.ended, sample.frames.map((frame: any) => frame.at)), raf: deliveryTimeline(sample.started, sample.ended, sample.raf),
      compositor: summarizeCompositorTruth(evidence.events, sample.ended - sample.started, evidence.analysis.window ?? undefined),
      nativeDelivery: evidence.analysis, gcEvents: evidence.events.filter(event => event.cat?.split(",").includes("v8.gc")),
      gcScope: "Only recorded V8 GC events are observations of collection. An empty event list is unobserved/inconclusive, not absence of GC.", failure: failure ? String(failure) : null }))
    await processSnapshot(); await retain()
    if (!evidence.manifest.complete) throw new Error("Sustained presentation evidence incomplete")
  }
  if (failure) throw failure
  if (sample.ended - sample.started < 90_000 || sample.dropped || sample.missedPublications || sample.lifecycle.length) throw new Error("Sustained window incomplete or interrupted")
  await checkNativeWindow()
  await page.locator("canvas.world-canvas").screenshot({ path: file("late.png") })
}
