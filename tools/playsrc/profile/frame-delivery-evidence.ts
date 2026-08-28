import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import { loadCompositorEvidence, TRACE_LIMITS } from "./compositor-evidence"
import { summarizeCompositorTruth, summarizeActivePresentationSilence } from "./compositor-truth"
import { retainedDeliveryAttribution, summarizeDeliveryMeasurement } from "./frame-delivery"

type CpuEvent = { name?: string; ts?: number; dur?: number; tdur?: number; pid?: number; tid?: number; args?: any }

/** Select the gameplay isolate by its recorded script, then count only its
 * non-nested message tasks. Never add FunctionCall/RunTask CPU a second time or
 * prorate thread CPU across a censored boundary. */
export function gameplayWorkerCpu(events: readonly CpuEvent[], window: { startedMicroseconds: number; endedMicroseconds: number }) {
  const owners = new Set(events.filter(event => event.name === "FunctionCall" && event.args?.data?.url?.includes("gameplay-worker")
    && event.ts! >= window.startedMicroseconds && event.ts! < window.endedMicroseconds).map(event => `${event.pid}:${event.tid}`))
  return [...owners].map(thread => {
    const tasks = events.filter(event => event.name === "HandlePostMessage" && `${event.pid}:${event.tid}` === thread && Number.isFinite(event.ts) && Number.isFinite(event.dur))
      .sort((left, right) => left.ts! - right.ts!)
    const interior = tasks.filter(event => event.ts! >= window.startedMicroseconds && event.ts! + event.dur! <= window.endedMicroseconds)
    if (interior.some((event, index) => index > 0 && event.ts! < interior[index - 1]!.ts! + interior[index - 1]!.dur!)) throw new Error("Overlapping Worker message tasks cannot be summed as CPU")
    const missingCpuClocks = interior.filter(event => !Number.isFinite(event.tdur)).length
    return { thread, interiorCallbacks: interior.length, missingCpuClocks,
      wallMilliseconds: interior.reduce((sum, event) => sum + event.dur!, 0) / 1000,
      cpuMilliseconds: missingCpuClocks ? null : interior.reduce((sum, event) => sum + event.tdur!, 0) / 1000,
      boundaryCallbacks: tasks.filter(event => event.ts! < window.endedMicroseconds && event.ts! + event.dur! > window.startedMicroseconds && !interior.includes(event))
        .map(event => ({ startedMicroseconds: event.ts, endedMicroseconds: event.ts! + event.dur!, wallMilliseconds: event.dur! / 1000,
          fullCpuMilliseconds: event.tdur === undefined ? null : event.tdur / 1000, activeCpuMilliseconds: null })),
    }
  })
}

/** Reuse complete immutable native evidence when only analysis changed. */
export async function analyzeRetainedDelivery(measurementPath: string, manifestPath: string) {
  if ((await stat(measurementPath)).size > TRACE_LIMITS.probeBytes) throw new Error("Measurement exceeds its byte bound")
  const bytes = await readFile(measurementPath), measurement = JSON.parse(bytes.toString("utf8"))
  const { manifest, events, probes, analysis } = await loadCompositorEvidence(manifestPath)
  if (!manifest.complete || !analysis.window || !manifest.identity.sourceUnchanged || probes.dropped
    || measurement.started !== probes.started || measurement.ended !== probes.ended
    || JSON.stringify(measurement.frames) !== JSON.stringify(probes.joins.filter(join => join.kind === "completed-frame").map(join => join.detail))
    || JSON.stringify(measurement.worker) !== JSON.stringify(probes.joins.filter(join => join.kind === "worker").map(join => join.detail))) throw new Error("Retained delivery measurement/trace linkage differs")
  const elapsed = measurement.ended - measurement.started
  if (elapsed < 5000 || elapsed > 10000 || Math.abs(elapsed - (analysis.window.endedMicroseconds - analysis.window.startedMicroseconds) / 1000) > 1) throw new Error("Retained active boundary differs")
  const phases = summarizeDeliveryMeasurement(measurement)
  const compositor = summarizeCompositorTruth(events, elapsed, analysis.window)
  return { schema: "playsrc-frame-delivery-analysis-v1", newCapture: false, performanceAccepted: false,
    measurementSha256: createHash("sha256").update(bytes).digest("hex"), identity: manifest.identity,
    ...phases, compositor, compositorSilence: summarizeActivePresentationSilence(events, analysis.window),
    gameplayWorkerCpu: gameplayWorkerCpu(events, analysis.window),
    attribution: retainedDeliveryAttribution(measurement, { compositor, renderWork: phases.renderSubmissionElapsed }).observe }
}
