import type { ChromiumTraceEvent } from "./compositor-truth"
import { summarizeCpuProfile, summarizeDistribution } from "./gameui-profile"
import { reconstructCpuProfile, selectCpuWindow, validateCpuWindow, type CpuTimeline } from "./cpu-profile-time"
import type { WorkerCpuCapture } from "./worker-cpu-profiler"
import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import path from "node:path"
import { loadCompositorEvidence, loadCapturePlan, TRACE_LIMITS } from "./compositor-evidence"
import { activeGameplayTraceWindow, summarizeActivePresentationSilence } from "./compositor-truth"
import { assertWorkerInstrumentation } from "./upward-profile-gates"

function sampledStacks(timeline: CpuTimeline, started: number, ended: number) {
  const { nodes, parents } = timeline
  const counts = new Map<number, { samples: number; estimatedMilliseconds: number }>()
  for (const sample of selectCpuWindow(timeline, { startedMicroseconds: started, endedMicroseconds: ended }).samples) {
    const value = counts.get(sample.nodeId) ?? { samples: 0, estimatedMilliseconds: 0 }
    value.samples += sample.samples
    value.estimatedMilliseconds += sample.estimatedMicroseconds / 1000
    counts.set(sample.nodeId, value)
  }
  return [...counts].sort((a, b) => b[1].samples - a[1].samples).slice(0, 12).map(([id, value]) => {
    const frames = []
    let node = nodes.get(id)
    while (node && frames.length < 24) {
      frames.push(node.callFrame)
      node = nodes.get(parents.get(node.id) ?? -1)
    }
    return { ...value, frames }
  })
}

export async function replayWorkerIncidents(filename: string) {
  return loadWorkerIncidents(filename, await loadCompositorEvidence(filename))
}

export async function loadWorkerIncidents(filename: string, { manifest, events, probes }: Awaited<ReturnType<typeof loadCompositorEvidence>>) {
  const artifact = manifest.identity.workerCpu
  if (!artifact || !/^[0-9a-f]{64}\.workers\.json$/u.test(artifact.file) || !artifact.file.startsWith(artifact.sha256)) throw new Error("Worker evidence identity is missing or invalid")
  const file = path.join(path.dirname(filename), artifact.file)
  const size = (await stat(file)).size
  if (size > TRACE_LIMITS.probeBytes || size !== artifact.bytes) throw new Error("Worker evidence byte bound or identity mismatch")
  const bytes = await readFile(file)
  if (createHash("sha256").update(bytes).digest("hex") !== artifact.sha256) throw new Error("Worker evidence hash mismatch")
  const value = JSON.parse(bytes.toString("utf8"))
  if (value.schema !== "playsrc-worker-cpu-v1" || !Array.isArray(value.captures) || value.error) throw new Error(`Worker CPU evidence is incomplete: ${value.error ?? "invalid schema"}`)
  const capturePlan = await loadCapturePlan(filename, manifest)
  const window = activeGameplayTraceWindow(events)
  const analyses = attributeWorkerIncidents(events, value.captures, window, {
    requests: probes.joins.filter(probe => probe.kind === "worker").map(probe => probe.detail as { id: number }),
    publications: probes.joins.filter(probe => probe.kind === "simulation-publication").map(probe => probe.detail as { requestId: number }),
  })
  if (capturePlan?.workerCpu === "required") assertWorkerInstrumentation(analyses.map(analysis => ({
    deadlineStopped: analysis.deadlineStopped, sampleCount: analysis.activeCpu.sampleCount, captureComplete: analysis.captureComplete,
  })))
  return { window, compositorComplete: manifest.complete, compositorErrors: manifest.errors, capturePlan,
    workerInstrumentation: { requested: capturePlan?.workerCpu ?? "unknown", capturedTargets: analyses.length },
    compositorSilence: summarizeActivePresentationSilence(events, window),
    workerArtifact: artifact, unsampledTargets: value.unsampledTargets ?? [], analyses }
}

if (import.meta.main) {
  const filename = process.argv[2]
  if (!filename) throw new Error("Usage: bun tools/playsrc/profile/worker-incident-attribution.ts <sha256.manifest.json>")
  console.log(JSON.stringify(await replayWorkerIncidents(filename), null, 2))
}

export function attributeWorkerIncidents(
  events: readonly ChromiumTraceEvent[],
  captures: readonly WorkerCpuCapture[],
  window: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>,
  browser: Readonly<{ requests: readonly { id: number; [key: string]: unknown }[]; publications: readonly { requestId: number; [key: string]: unknown }[] }> = { requests: [], publications: [] },
) {
  validateCpuWindow(window)
  if (captures.length > 32) throw new Error("Worker capture target bound exceeded")
  return captures.map(capture => {
    const timeline = reconstructCpuProfile(capture.profile)
    if (capture.execution.clocks.length !== 2 || capture.execution.tasks.length > 16_384) throw new Error("Worker CPU/task evidence bounds are invalid")
    const clocks = capture.execution.clocks.map(clock => {
      if (!Number.isFinite(clock.before) || !Number.isFinite(clock.after) || clock.after < clock.before) throw new Error("Invalid Worker clock interval")
      const matches = events.filter(event => event.name === clock.name && Number.isFinite(event.ts))
      if (matches.length !== 1) throw new Error(`Worker clock mark ${clock.name} is missing or ambiguous`)
      const mark = matches[0]!
      return {
        ...clock, traceMicroseconds: mark.ts!, pid: mark.pid, tid: mark.tid,
        offsetMinimumMicroseconds: mark.ts! - clock.after * 1000,
        offsetMaximumMicroseconds: mark.ts! - clock.before * 1000,
      }
    })
    if (clocks.length !== 2 || clocks.some(clock => clock.pid !== clocks[0]!.pid || clock.tid !== clocks[0]!.tid)) throw new Error("Worker clock thread identity changed")
    const minimum = Math.max(...clocks.map(clock => clock.offsetMinimumMicroseconds))
    const maximum = Math.min(...clocks.map(clock => clock.offsetMaximumMicroseconds))
    // Allow timestamp rounding, but never repair a mismatched clock by guessing.
    if (minimum - maximum > 100) throw new Error("Worker monotonic clock calibration does not intersect")
    const offset = (minimum + maximum) / 2
    if (capture.profile.startTime < clocks[0]!.traceMicroseconds - 1_000
      || capture.profile.endTime > clocks[1]!.traceMicroseconds + 1_000
      || capture.profile.startTime >= capture.profile.endTime) throw new Error("Worker CPU profile is outside calibrated trace clock bounds")
    const thread = events.filter(event => event.pid === clocks[0]!.pid && event.tid === clocks[0]!.tid)
    for (const task of capture.execution.tasks) {
      if (!Number.isFinite(task.started) || !Number.isFinite(task.finished) || task.finished < task.started) throw new Error("Invalid Worker task interval")
    }
    const tasks = capture.execution.tasks.filter(task => task.started * 1000 + offset < window.endedMicroseconds
      && task.finished * 1000 + offset > window.startedMicroseconds)
    const slow = tasks.filter(task => task.finished - task.started >= 20)
    const slowTasks = slow.toSorted((left, right) => (right.finished - right.started) - (left.finished - left.started)).slice(0, 64).map(task => {
      const started = task.started * 1000 + offset
      const ended = task.finished * 1000 + offset
      // Duration-bearing containment plus native args is correlation, not a
      // claim that HandlePostMessage itself spent this time serializing.
      const nativeTasks = thread.filter(event => /RunTask|HandlePostMessage|FunctionCall/u.test(event.name ?? "")
        && (event.dur ?? 0) > 0 && event.ts! <= started + 100 && event.ts! + event.dur! >= ended - 100)
      const garbageCollection = thread.filter(event => /^(MinorGC|MajorGC|Scavenge|MarkCompact)$/u.test(event.name ?? "")
        && event.ts! < ended && event.ts! + (event.dur ?? 0) > started)
      const before = task.memory[0]
      const after = task.memory.at(-1)
      return {
        ...task, startedMicroseconds: started, endedMicroseconds: ended,
        scope: started >= window.startedMicroseconds && ended <= window.endedMicroseconds ? "sample" : "sample-boundary",
        sampleOverlapMilliseconds: Math.max(0, Math.min(ended, window.endedMicroseconds) - Math.max(started, window.startedMicroseconds)) / 1000,
        milliseconds: task.finished - task.started, nativeTasks, garbageCollection,
        linearMemoryGrowthBytes: before && after ? after.linearBytes - before.linearBytes : null,
        browserTransactions: task.responses.map(response => ({
          request: browser.requests.find(request => request.id === response.requestId) ?? null,
          publication: browser.publications.find(publication => publication.requestId === response.requestId) ?? null,
        })),
        // Counter deltas bound memory.grow to this task, not to an instruction.
        stacks: sampledStacks(timeline, started, ended),
        activeStacks: sampledStacks(timeline, Math.max(started, window.startedMicroseconds), Math.min(ended, window.endedMicroseconds)),
      }
    })
    return {
      target: capture.target, samplingIntervalMicroseconds: capture.samplingIntervalMicroseconds,
      cpu: summarizeCpuProfile(capture.profile), activeCpu: summarizeCpuProfile(capture.profile, window),
      clock: { domain: "Chromium monotonic microseconds", workerTimeOrigin: capture.execution.timeOrigin, offsetMicroseconds: offset, clocks },
      samples: capture.profile.samples?.length ?? 0, profileStarted: capture.profile.startTime, profileEnded: capture.profile.endTime,
      captureComplete: capture.execution.dropped === 0 && capture.deadlineStopped !== true, droppedTasks: capture.execution.dropped,
      deadlineStopped: capture.deadlineStopped === true, slowTaskCount: slow.length, detailsTruncated: slow.length > 64,
      taskMilliseconds: summarizeDistribution(tasks.map(task => task.finished - task.started)),
      postMessageMilliseconds: summarizeDistribution(tasks.flatMap(task => task.responses.filter(response => response.transport !== "atomic").map(response => response.finished - response.started))),
      atomicPublishMilliseconds: summarizeDistribution(tasks.flatMap(task => task.responses.filter(response => response.transport === "atomic").map(response => response.finished - response.started))),
      slowTasks,
    }
  })
}
