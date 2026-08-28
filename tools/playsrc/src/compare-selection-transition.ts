import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "./config"
import { analyzeNativeSelectionPixels } from "../profile/selection-transition-analysis"
import { compareSelectionIntervals } from "../profile/selection-comparison"

const [beforeDirectory, afterDirectory] = process.argv.slice(2), { sourceCacheDir } = await loadLocalConfig()
if (process.argv.length !== 4 || [beforeDirectory, afterDirectory].some(value => !value || !path.resolve(value).startsWith(path.resolve(sourceCacheDir) + path.sep))) {
  throw new Error("Usage: compare-selection-transition.ts <configured-cache before directory> <configured-cache after directory>")
}
const read = async (directory: string) => JSON.parse(await readFile(path.join(directory, "selection-measurement.json"), "utf8"))
const [before, after] = await Promise.all([read(beforeDirectory!), read(afterDirectory!)])
if (before.team !== after.team || before.identity !== after.identity || before.warm !== after.warm
  || before.cpuAttributionEnabled !== after.cpuAttributionEnabled) throw new Error("Selection workloads or CPU attribution modes differ")
const beforePixels = await analyzeNativeSelectionPixels(beforeDirectory!), afterPixels = await analyzeNativeSelectionPixels(afterDirectory!)
const intervals = compareSelectionIntervals(beforePixels, afterPixels)
const owners = (measurement: any) => {
  const entries = measurement.evidence.owners
  const end = entries.find((entry: any) => entry.kind === "pipeline-end"), start = entries.find((entry: any) => entry.kind === "model-poses-ready")
  if (!start || !end) throw new Error("Complete model preparation ownership is required")
  return end.at - start.at
}
const modelBefore = owners(before), modelAfter = owners(after)
if (modelAfter >= modelBefore || intervals[0]!.disposition !== "proven-reduction") throw new Error("No proven reduction of the measured team-selection blocking work")
if (JSON.stringify(before.evidence.modelPreparation.models) !== JSON.stringify(after.evidence.modelPreparation.models)
  || before.evidence.gpu.preparedModelVariants !== after.evidence.gpu.preparedModelVariants) throw new Error("Prepared authored model/pass coverage changed")
if (after.evidence.losses.length || after.evidence.gpuOperationsDropped || before.evidence.gpuOperationsDropped) throw new Error("Incomplete or failed GPU evidence")
const memory = { before: { heap: before.heapAfter.usedSize, resident: before.residentAfter.residentBytes },
  after: { heap: after.heapAfter.usedSize, resident: after.residentAfter.residentBytes } }
if (memory.after.heap > memory.before.heap || memory.after.resident === null || memory.before.resident === null || memory.after.resident > memory.before.resident) {
  throw new Error(`Selection settled memory regression or missing evidence: ${JSON.stringify(memory)}`)
}
const peak = (value: any) => {
  const samples = value.evidence.memorySamples.map((sample: any) => sample.usedJSHeapSize)
  if (!samples.length || samples.some((sample: any) => !Number.isFinite(sample))) throw new Error("Sampled peak heap evidence is unavailable")
  return Math.max(...samples)
}
const sampledPeakHeap = { before: peak(before), after: peak(after) }
if (sampledPeakHeap.after > sampledPeakHeap.before) throw new Error("Sampled peak heap regressed")
const loadingBefore = JSON.parse(before.evidence.loading), loadingAfter = JSON.parse(after.evidence.loading)
if (loadingBefore.mapBytes !== loadingAfter.mapBytes || loadingBefore.presentationBytes !== loadingAfter.presentationBytes) throw new Error("Configured loaded content differs")
if (loadingAfter.totalMilliseconds > loadingBefore.totalMilliseconds) throw new Error("Initial loading regressed; selection work cannot be moved before the measured input")
const report = { status: "matched-reduction", beforeDirectory, afterDirectory, intervals,
  modelPreparationWallMilliseconds: { before: modelBefore, after: modelAfter }, memory, sampledPeakHeap,
  initialLoadingMilliseconds: { before: loadingBefore.totalMilliseconds, after: loadingAfter.totalMilliseconds },
  limitation: "Overlapping native capture intervals are not called a latency improvement. This does not certify an absolute250ms budget or steady60FPS." }
await writeFile(path.join(afterDirectory!, "selection-comparison.json"), JSON.stringify(report, null, 2))
console.log(JSON.stringify(report))
