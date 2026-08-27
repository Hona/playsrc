import { summarizeDistribution, type CpuProfile, type TraceEvent } from "./gameui-profile"
import { reconstructCpuProfile, selectCpuWindow } from "./cpu-profile-time"

type DisplayFrame = Readonly<{ at: number; displayFrame: number; mouseRevision?: number; detail?: Record<string, number>; workerPending?: number }>
type WorkerRecord = Readonly<{ kind: string; started: number; finished?: number; timings?: Record<string, number> }>
type InputRecord = Readonly<{ at: number; revision: number; kind: string }>

export function attributeFrameTails(options: Readonly<{
  frames: readonly DisplayFrame[]
  workers: readonly WorkerRecord[]
  inputs: readonly InputRecord[]
  longAnimationFrames: readonly Readonly<{ at: number; duration: number; scripts?: readonly unknown[]; styleAndLayoutMilliseconds?: number }>[]
  trace: readonly TraceEvent[]
  cpu: CpuProfile
  traceOffsetMicroseconds: number
}>) {
  const gc = options.trace.filter(event => event.ph === "X" && typeof event.dur === "number"
    && /^(?:MinorGC|MajorGC|Scavenge|MarkCompact)$/iu.test(event.name ?? ""))
  const collections = gc.map(event => ({
    kind: /(?:MinorGC|Scavenge)/iu.test(event.name ?? "") ? "minor" as const
      : /(?:MajorGC|MarkCompact)/iu.test(event.name ?? "") ? "major" as const : "other" as const,
    at: (event.ts! - options.traceOffsetMicroseconds) / 1_000,
    milliseconds: event.dur! / 1_000,
    name: event.name!,
    beforeBytes: Number(event.args?.usedHeapSizeBefore ?? event.args?.data?.usedHeapSizeBefore ?? 0),
    afterBytes: Number(event.args?.usedHeapSizeAfter ?? event.args?.data?.usedHeapSizeAfter ?? 0),
  }))
  const timeline = reconstructCpuProfile(options.cpu)
  const { nodes, parents } = timeline
  const tails = options.frames.flatMap((frame, index) => {
    if (index === 0) return []
    const previous = options.frames[index - 1]!
    const milliseconds = frame.at - previous.at
    if (milliseconds <= 20) return []
    const selected = selectCpuWindow(timeline, {
      startedMicroseconds: previous.at * 1000 + options.traceOffsetMicroseconds,
      endedMicroseconds: frame.at * 1000 + options.traceOffsetMicroseconds,
    })
    const counts = new Map<number, { samples: number; estimatedMilliseconds: number }>()
    for (const sample of selected.samples) {
      const value = counts.get(sample.nodeId) ?? { samples: 0, estimatedMilliseconds: 0 }
      value.samples += sample.samples
      value.estimatedMilliseconds += sample.estimatedMicroseconds / 1000
      counts.set(sample.nodeId, value)
    }
    const stacks = [...counts].sort((left, right) => right[1].samples - left[1].samples).slice(0, 3).map(([identity, value]) => {
      const frames: string[] = []
      let current = nodes.get(identity)
      while (current && frames.length < 8) {
        frames.push(current.callFrame.functionName || "(anonymous)")
        current = nodes.get(parents.get(current.id) ?? -1)
      }
      return { ...value, frames }
    })
    return [{
      displayFrame: frame.displayFrame, at: frame.at, milliseconds: Number(milliseconds.toFixed(3)),
      cpuSampling: { sampleCount: selected.sampleCount, estimatedMilliseconds: selected.estimatedMicroseconds / 1000,
        unattributedMilliseconds: selected.unattributedMicroseconds / 1000, outsideProfileMilliseconds: selected.outsideProfileMicroseconds / 1000 },
      work: frame.detail ?? null, workerPending: frame.workerPending ?? 0, stacks,
      workers: options.workers.filter(worker => worker.started <= frame.at && (worker.finished ?? Infinity) >= previous.at)
        .map(worker => ({ kind: worker.kind, milliseconds: Number(((worker.finished ?? frame.at) - worker.started).toFixed(3)), queueMilliseconds: worker.timings?.queueMilliseconds ?? null })),
      garbageCollection: collections.filter(collection => collection.at < frame.at && collection.at + collection.milliseconds > previous.at),
      longAnimationFrames: options.longAnimationFrames.filter(animation => animation.at < frame.at && animation.at + animation.duration > previous.at),
    }]
  })
  const latency = options.inputs.flatMap(input => {
    const frame = options.frames.find(candidate => candidate.at >= input.at && (candidate.mouseRevision ?? 0) >= input.revision)
    return frame ? [frame.at - input.at] : []
  })
  return Object.freeze({
    frames: tails,
    garbageCollection: {
      count: collections.length,
      minor: collections.filter(collection => collection.kind === "minor").length,
      major: collections.filter(collection => collection.kind === "major").length,
      milliseconds: summarizeDistribution(collections.map(collection => collection.milliseconds)),
      reclaimedBytes: collections.reduce((total, collection) => total + Math.max(0, collection.beforeBytes - collection.afterBytes), 0),
    },
    inputToVisibleMilliseconds: summarizeDistribution(latency),
  })
}
