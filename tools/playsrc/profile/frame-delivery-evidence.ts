import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import { loadCompositorEvidence, TRACE_LIMITS } from "./compositor-evidence"
import { summarizeCompositorTruth, summarizeActivePresentationSilence } from "./compositor-truth"
import { retainedDeliveryAttribution, summarizeDeliveryMeasurement } from "./frame-delivery"

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
    attribution: retainedDeliveryAttribution(measurement, { compositor, renderWork: phases.renderSubmissionElapsed }).observe }
}
