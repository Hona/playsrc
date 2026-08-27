import type { CpuProfile } from "./gameui-profile"

export const CPU_PROFILE_LIMITS = Object.freeze({ nodes: 64_000, samples: 64_000, depth: 256 })
export type CpuWindow = Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>

function require(value: unknown, message: string): asserts value {
  if (!value) throw new Error(`Invalid CPU profile: ${message}`)
}

export function validateCpuWindow(window: CpuWindow) {
  require(window && Number.isFinite(window.startedMicroseconds) && Number.isFinite(window.endedMicroseconds)
    && window.startedMicroseconds >= 0 && window.endedMicroseconds <= Number.MAX_SAFE_INTEGER
    && window.endedMicroseconds >= window.startedMicroseconds, "window bounds")
}

/** CDP Profiler.Profile timeDeltas encode signed differences between timestamps,
 * with the first relative to startTime. They are NOT execution durations.
 * https://chromedevtools.github.io/devtools-protocol/tot/Profiler/#type-Profile
 * DevTools CPUProfileDataModel.convertTimeDeltas/sortSamples likewise accumulate
 * https://github.com/ChromeDevTools/devtools-frontend/blob/be2aa079c50adf729e11cf286b2a85e7c3d4e113/front_end/models/cpu_profile/CPUProfileDataModel.ts
 * before sorting paired samples. Keep raw order/absolute times here; sort only a
 * bounded index copy. Never rewrite a negative delta or a sampled stack.
 */
export function reconstructCpuProfile(profile: CpuProfile) {
  require(profile && Number.isSafeInteger(profile.startTime) && Number.isSafeInteger(profile.endTime)
    && profile.startTime >= 0 && profile.endTime >= profile.startTime, "start/end timestamps")
  require(Array.isArray(profile.nodes) && profile.nodes.length <= CPU_PROFILE_LIMITS.nodes, "node bound")
  require((profile.samples === undefined && profile.timeDeltas === undefined)
    || (Array.isArray(profile.samples) && Array.isArray(profile.timeDeltas)
      && profile.samples.length === profile.timeDeltas.length), "samples/timeDeltas must be paired")
  const ids = profile.samples ?? [], deltas = profile.timeDeltas ?? []
  require(ids.length <= CPU_PROFILE_LIMITS.samples, "sample bound")
  const nodes = new Map<number, CpuProfile["nodes"][number]>()
  for (const node of profile.nodes) {
    require(node && Number.isSafeInteger(node.id) && node.id > 0 && !nodes.has(node.id), "duplicate/invalid node id")
    const frame = node.callFrame
    require(frame && typeof frame.functionName === "string" && typeof frame.url === "string"
      && Number.isSafeInteger(frame.lineNumber) && frame.lineNumber >= -1
      && Number.isSafeInteger(frame.columnNumber) && frame.columnNumber >= -1, "call frame")
    require(node.children === undefined || (Array.isArray(node.children) && node.children.length <= CPU_PROFILE_LIMITS.nodes), "children bound")
    nodes.set(node.id, node)
  }
  const parents = new Map<number, number>()
  for (const node of nodes.values()) for (const child of node.children ?? []) {
    require(nodes.has(child) && !parents.has(child), "unknown child or multiple parents")
    parents.set(child, node.id)
  }
  const roots = [...nodes.keys()].filter(id => !parents.has(id))
  require(nodes.size === 0 || roots.length === 1, "expected one root")
  const pending = roots.map(id => ({ id, depth: 0 }))
  let visited = 0
  while (pending.length) {
    const { id, depth } = pending.pop()!
    require(depth <= CPU_PROFILE_LIMITS.depth, "stack depth bound")
    visited++
    for (const child of nodes.get(id)!.children ?? []) pending.push({ id: child, depth: depth + 1 })
  }
  require(visited === nodes.size, "cyclic node tree")
  let timestamp = profile.startTime, negativeDeltaCount = 0, maximumTimestamp = profile.startTime, maximumReorderMicroseconds = 0
  const rawSamples = Array.from(ids, (nodeId, rawIndex) => {
    const delta = deltas[rawIndex]!
    require(Number.isSafeInteger(delta), `delta at ${rawIndex}`)
    timestamp += delta
    require(Number.isSafeInteger(timestamp) && timestamp >= profile.startTime && timestamp <= profile.endTime, `timestamp outside profile at ${rawIndex}`)
    require(nodes.has(nodeId), `unknown sampled node at ${rawIndex}`)
    if (delta < 0) negativeDeltaCount++
    maximumReorderMicroseconds = Math.max(maximumReorderMicroseconds, maximumTimestamp - timestamp)
    maximumTimestamp = Math.max(maximumTimestamp, timestamp)
    return Object.freeze({ rawIndex, nodeId, timestampMicroseconds: timestamp })
  })
  const chronological = rawSamples.toSorted((a, b) => a.timestampMicroseconds - b.timestampMicroseconds || a.rawIndex - b.rawIndex)
  // Like the DevTools flamechart, a sample estimates the interval until the next
  // chronological sample. Unlike normalizeTimestamps, do not extrapolate a final
  // average interval, change profile bounds, or fabricate missing samples. Equal
  // timestamps retain raw order: all are counted; only the last gets the next gap.
  // These non-overlapping weights are estimates, not measured exclusive CPU wall.
  const intervals = chronological.map((sample, index) => Object.freeze({ ...sample,
    endedMicroseconds: chronological[index + 1]?.timestampMicroseconds ?? sample.timestampMicroseconds,
  }))
  return Object.freeze({
    nodes, parents, rawSamples: Object.freeze(rawSamples), intervals: Object.freeze(intervals),
    startedMicroseconds: profile.startTime, endedMicroseconds: profile.endTime,
    signedElapsedMicroseconds: timestamp - profile.startTime, negativeDeltaCount, maximumReorderMicroseconds,
    equalTimestampCount: chronological.filter((sample, index) => index > 0 && sample.timestampMicroseconds === chronological[index - 1]!.timestampMicroseconds).length,
  })
}

export type CpuTimeline = ReturnType<typeof reconstructCpuProfile>

/** Explicit windows are half-open. Keep a predecessor whose estimated interval
 * overlaps the left boundary, even though its sample point is outside. Full
 * profile summaries also count terminal points at endTime, with zero weight. */
export function selectCpuWindow(timeline: CpuTimeline, window?: CpuWindow) {
  if (window !== undefined) validateCpuWindow(window)
  const selected = window ?? timeline
  validateCpuWindow(selected)
  const started = selected.startedMicroseconds, ended = selected.endedMicroseconds
  const samples = timeline.intervals.flatMap(sample => {
    const point = sample.timestampMicroseconds
    const inWindow = point >= started && (point < ended || (!window && point === ended))
    const microseconds = Math.max(0, Math.min(sample.endedMicroseconds, ended) - Math.max(point, started))
    return inWindow || microseconds > 0 ? [{ ...sample, samples: Number(inWindow), estimatedMicroseconds: microseconds }] : []
  })
  const estimatedMicroseconds = samples.reduce((sum, sample) => sum + sample.estimatedMicroseconds, 0)
  const profileOverlapMicroseconds = Math.max(0, Math.min(ended, timeline.endedMicroseconds) - Math.max(started, timeline.startedMicroseconds))
  const first = timeline.intervals[0]?.timestampMicroseconds
  const last = timeline.intervals.at(-1)?.timestampMicroseconds
  const overlap = (a: number, b: number) => Math.max(0, Math.min(ended, b) - Math.max(started, a))
  return Object.freeze({
    startedMicroseconds: started, endedMicroseconds: ended, samples,
    sampleCount: samples.reduce((sum, sample) => sum + sample.samples, 0), estimatedMicroseconds,
    wallMicroseconds: ended - started, profileOverlapMicroseconds,
    outsideProfileMicroseconds: ended - started - profileOverlapMicroseconds,
    unattributedMicroseconds: ended - started - estimatedMicroseconds,
    leadingUnattributedMicroseconds: first === undefined ? 0 : overlap(timeline.startedMicroseconds, first),
    trailingUnattributedMicroseconds: last === undefined ? 0 : overlap(last, timeline.endedMicroseconds),
    unsampledProfileMicroseconds: first === undefined ? profileOverlapMicroseconds : 0,
  })
}
