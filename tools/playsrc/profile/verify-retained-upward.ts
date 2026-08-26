import { createHash } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadCompositorEvidence, TRACE_LIMITS } from "./compositor-evidence"
import { summarizeCompositorTruth, summarizeActivePresentationSilence, summarizeCompositorStages, analyzeCompositorStalls } from "./compositor-truth"
import { attributeWorkerIncidents } from "./worker-incident-attribution"
import { assertUpwardProfile } from "./upward-profile-gates"

/** Validate an unchanged real capture after bounded offline native decoding. */
export async function verifyRetainedUpward(reportPath: string, manifestPath: string) {
  const sourceBytes = await readFile(reportPath)
  if (sourceBytes.length > TRACE_LIMITS.probeBytes) throw new Error("Source report byte bound exceeded")
  const report = JSON.parse(sourceBytes.toString("utf8"))
  const { manifest, events, probes, analysis } = await loadCompositorEvidence(manifestPath)
  if (!manifest.complete || !analysis.window || report.schema !== "playsrc-tf2-upward-training-bots-profile-v1" || report.target !== "pl_upward"
    || !report.headed || report.label !== manifest.identity.label
    || `${report.compositorEvidence.sha256}.manifest.json` !== (manifest.identity.reanalysis?.parentManifest ?? path.basename(manifestPath))) throw new Error("Capture/report linkage is invalid")
  const elapsed = (analysis.window.endedMicroseconds - analysis.window.startedMicroseconds) / 1000
  if (elapsed < 5000 || Math.abs(elapsed - report.elapsedMilliseconds) > 1) throw new Error("Retained active duration changed")
  const worker = manifest.identity.workerCpu
  if (!worker || !/^[0-9a-f]{64}\.workers\.json$/u.test(worker.file)) throw new Error("Worker identity absent")
  const workerBytes = await readFile(path.join(path.dirname(manifestPath), worker.file))
  if (workerBytes.length !== worker.bytes || workerBytes.length > TRACE_LIMITS.probeBytes || createHash("sha256").update(workerBytes).digest("hex") !== worker.sha256) throw new Error("Worker hash mismatch")
  const capture = JSON.parse(workerBytes.toString("utf8"))
  if (capture.error) throw new Error("Worker capture failed")
  report.compositor = { ...summarizeCompositorTruth(events, elapsed, analysis.window), stages: summarizeCompositorStages(events, analysis.window),
    stalls: analyzeCompositorStalls(events, analysis.window, probes.joins.filter(value => value.kind === "class-lifecycle").map(value => value.detail as { at: number; phase: string; playerClass?: number })) }
  report.compositorSilence = summarizeActivePresentationSilence(events, analysis.window)
  report.workerIncidents = attributeWorkerIncidents(events, capture.captures, analysis.window, {
    requests: probes.joins.filter(value => value.kind === "worker").map(value => value.detail as { id: number }),
    publications: probes.joins.filter(value => value.kind === "simulation-publication").map(value => value.detail as { requestId: number }),
  })
  report.compositorEvidence = { file: path.basename(manifestPath), complete: manifest.complete, errors: manifest.errors, analysis }
  assertUpwardProfile(report, { expectedBots: 15, playerCount: 16, classes: true, classPasses: 2, smooth: true, compositor: true,
    sourceUnchanged: manifest.identity.sourceFingerprint === manifest.identity.sourceFingerprintAfter, workerCaptures: capture.captures })
  const value = { schema: "playsrc-retained-upward-verification-v1", newCapture: false, gatesPassed: true,
    sourceReportSha256: createHash("sha256").update(sourceBytes).digest("hex"), nativeManifest: path.basename(manifestPath),
    target50Met: Math.max(report.compositorSilence.maximumActiveSilenceMilliseconds, report.compositorSilence.maximumCensoredBoundaryMilliseconds) < 50,
    report }
  const bytes = Buffer.from(JSON.stringify(value))
  const file = `${createHash("sha256").update(bytes).digest("hex")}.upward-verification.json`
  const destination = path.join(path.dirname(manifestPath), file)
  await writeFile(destination, bytes, { flag: "wx" }).catch(async error => {
    if (error.code !== "EEXIST" || !bytes.equals(await readFile(destination))) throw error
  })
  return { file, gatesPassed: true, target50Met: value.target50Met, platform: report.browser.platform, compositor: report.compositor, silence: report.compositorSilence }
}

if (import.meta.main) {
  const [report, manifest] = process.argv.slice(2)
  if (!report || !manifest) throw new Error("Usage: bun tools/playsrc/profile/verify-retained-upward.ts <original-report.json> <sha256.manifest.json>")
  console.log(JSON.stringify(await verifyRetainedUpward(report, manifest), null, 2))
}
