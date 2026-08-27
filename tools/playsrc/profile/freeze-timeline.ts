import { summarizeFrameTimes } from "./profile-window"
import { chromiumPresentationEventName, type ChromiumTraceEvent } from "./compositor-truth"

export type FreezeWindow = Readonly<{ startedMilliseconds: number; endedMilliseconds: number }>
export type PresentedContent = Readonly<{ at: number; contentIdentity: string | null }>
const THRESHOLDS = [50, 100, 250, 500, 1000] as const

function validateWindow(window: FreezeWindow): void {
  if (!Number.isFinite(window.startedMilliseconds) || !Number.isFinite(window.endedMilliseconds)
    || window.endedMilliseconds <= window.startedMilliseconds) throw new Error("Freeze timeline requires finite ordered sample boundaries")
}

/** One clock and one stream only. Context observations outside the sample are
 * retained so a freeze crossing a boundary cannot disappear from the report. */
export function summarizeFreezeTimeline(timestamps: readonly number[], window: FreezeWindow) {
  const { startedMilliseconds: start, endedMilliseconds: end } = window
  validateWindow(window)
  if (timestamps.some(at => !Number.isFinite(at))) throw new Error("Freeze timeline requires finite timestamps")
  const times = [...new Set(timestamps)].sort((a, b) => a - b)
  const inside = times.filter(at => at >= start && at < end)
  const previous = times.findLast(at => at < start)
  const following = times.find(at => at >= end)
  const points = [previous ?? null, ...inside, following ?? null]
  const gaps = points.slice(1).flatMap((right, index) => {
    const left = points[index]!
    const a = left ?? start, b = right ?? end
    const activeStart = Math.max(start, a), activeEnd = Math.min(end, b)
    if (activeEnd <= activeStart) return []
    return [{
      startedMilliseconds: a, endedMilliseconds: b,
      observedMilliseconds: left === null || right === null ? null : b - a,
      minimumMilliseconds: b - a,
      activeStartMilliseconds: activeStart, activeEndMilliseconds: activeEnd,
      activeMilliseconds: activeEnd - activeStart,
      leftWindowCensored: a < start || left === null,
      rightWindowCensored: b > end || right === null,
      missingPreviousObservation: left === null, missingFollowingObservation: right === null,
    }]
  })
  const rolling = (size: number) => {
    const output = []
    let first = 0, last = 0
    for (let at = start; at < end; at += 250) {
      const until = Math.min(at + size, end)
      while (first < inside.length && inside[first]! < at) first += 1
      last = Math.max(last, first)
      while (last < inside.length && inside[last]! < until) last += 1
      const count = last - first
      output.push({ startedMilliseconds: at, endedMilliseconds: until, partial: until - at !== size,
        frames: count, framesPerSecond: count * 1000 / (until - at) })
    }
    return output
  }
  return {
    frames: inside.length, framesPerSecond: inside.length * 1000 / (end - start),
    completeObservedIntervals: summarizeFrameTimes(gaps.flatMap(gap => gap.observedMilliseconds === null ? [] : [gap.observedMilliseconds])),
    maximumActiveSilenceMilliseconds: gaps.reduce((max, gap) => Math.max(max, gap.activeMilliseconds), 0),
    maximumObservedGapMilliseconds: gaps.reduce<number | null>((max, gap) => gap.observedMilliseconds === null ? max : Math.max(max ?? 0, gap.observedMilliseconds), null),
    boundaryCensoredIntervals: gaps.filter(gap => gap.leftWindowCensored || gap.rightWindowCensored),
    rolling250Milliseconds: rolling(250), rolling1000Milliseconds: rolling(1000),
    stalls: THRESHOLDS.map(threshold => {
      const long = gaps.filter(gap => gap.minimumMilliseconds > threshold)
      const covered = long.reduce((sum, gap) => sum + gap.activeMilliseconds, 0)
      // "Freeze" here is silence after the stated threshold, not an inferred
      // CPU/GPU duration. Missing context makes this a conservative lower bound.
      const excess = long.reduce((sum, gap) => sum + Math.max(0, gap.activeEndMilliseconds - Math.max(gap.activeStartMilliseconds, gap.startedMilliseconds + threshold)), 0)
      return { thresholdMilliseconds: threshold, count: long.length,
        cumulativeLongGapMilliseconds: covered, longGapRatio: covered / (end - start),
        freezeMillisecondsBeyondThreshold: excess, freezeRatioBeyondThreshold: excess / (end - start),
        uncertainCensoredMilliseconds: gaps.filter(gap => gap.observedMilliseconds === null && gap.minimumMilliseconds <= threshold).reduce((sum, gap) => sum + gap.activeMilliseconds, 0),
        intervals: long,
        interStallStartMilliseconds: long.slice(1).map((gap, index) => gap.startedMilliseconds - long[index]!.startedMilliseconds),
      }
    }),
  }
}

/** Requires an observed, versioned game-content identity at presentation (for
 * example a pixel fingerprint or an exact game-layer resource/version join).
 * A nearest earlier JS completion, RAF count, or unversioned texture ID is NOT
 * evidence of changed visible content. Unknown identities never become frames. */
export function summarizePresentedContent(observations: readonly PresentedContent[], window: FreezeWindow) {
  validateWindow(window)
  if (observations.some(observation => !Number.isFinite(observation.at))) throw new Error("Content presentation timestamp is invalid")
  const ordered = observations.toSorted((a, b) => a.at - b.at)
  const updates: number[] = []
  let previous: string | null = null
  let repeats = 0, unknown = 0
  for (const observation of ordered) {
    const active = observation.at >= window.startedMilliseconds && observation.at < window.endedMilliseconds
    if (observation.contentIdentity === null) { if (active) unknown += 1; previous = null; continue }
    if (previous !== observation.contentIdentity) updates.push(observation.at)
    else if (active) repeats += 1
    previous = observation.contentIdentity
  }
  const evidenceAvailable = ordered.some(observation => observation.contentIdentity !== null)
  return {
    evidence: evidenceAvailable && unknown === 0 ? "observed-content-presentation" : "incomplete-content-presentation",
    repeatedPresentations: repeats, unknownPresentations: unknown,
    // Incomplete identity coverage must not be turned into an apparent freeze.
    changingContent: evidenceAvailable && unknown === 0 ? summarizeFreezeTimeline(updates, window) : null,
  }
}

export function summarizeCompositorFreezes(events: readonly ChromiumTraceEvent[], window: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>) {
  const bounds = { startedMilliseconds: 0, endedMilliseconds: (window.endedMicroseconds - window.startedMicroseconds) / 1000 }
  validateWindow(bounds)
  const eventName = chromiumPresentationEventName(events)
  const streams = new Map<string, number[]>()
  for (const event of events) {
    if (!eventName || event.name !== eventName || !Number.isFinite(event.ts)) continue
    const key = `${event.pid}:${event.tid}`, timestamps = streams.get(key) ?? []
    timestamps.push((event.ts! - window.startedMicroseconds) / 1000)
    streams.set(key, timestamps)
  }
  return { eventName: eventName ?? null,
    // Different native presentation streams must never be interleaved into
    // artificially shorter gaps. Surface/content identity is separate evidence.
    streams: [...streams].map(([stream, timestamps]) => ({ stream, ...summarizeFreezeTimeline(timestamps, bounds) })),
    gameContentPresentation: null,
    gameContentEvidence: "unavailable-without-versioned-game-content-join" as const,
  }
}
