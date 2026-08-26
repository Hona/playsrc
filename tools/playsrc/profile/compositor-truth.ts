import { summarizeFrameTimes } from "./profile-window"

export type ChromiumTraceEvent = Readonly<{ name?: string; ts?: number; dur?: number; pid?: number; tid?: number; cat?: string; args?: Record<string, any> }>

const PRESENTATION_EVENTS = ["PresentationFeedback", "Display::FrameDisplayed", "FramePresented"] as const

export const TRACE_START = "playsrc-active-gameplay-start"
export const TRACE_END = "playsrc-active-gameplay-end"

export function activeGameplayTraceWindow(events: readonly ChromiumTraceEvent[]): Readonly<{ startedMicroseconds: number; endedMicroseconds: number }> {
  const started = events.find(event => event.name === TRACE_START && Number.isFinite(event.ts))?.ts
  const ended = events.find(event => event.name === TRACE_END && Number.isFinite(event.ts))?.ts
  if (started === undefined || ended === undefined || ended < started) throw new Error("Chromium trace does not contain ordered active gameplay marks")
  return Object.freeze({ startedMicroseconds: started, endedMicroseconds: ended })
}

export function chromiumPresentationEventName(events: readonly ChromiumTraceEvent[]) {
  return PRESENTATION_EVENTS.find(name => events.some(event => event.name === name))
}

export function summarizeCompositorTruth(events: readonly ChromiumTraceEvent[], elapsedMilliseconds: number, window?: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>) {
  if (!Number.isFinite(elapsedMilliseconds) || elapsedMilliseconds <= 0) throw new Error("Compositor sampling duration must be positive")
  if (window && (!Number.isFinite(window.startedMicroseconds) || !Number.isFinite(window.endedMicroseconds) || window.endedMicroseconds < window.startedMicroseconds)) throw new Error("Compositor sampling window is invalid")
  const sampled = events.filter(event => Number.isFinite(event.ts) && (!window || event.ts! >= window.startedMicroseconds && event.ts! <= window.endedMicroseconds))
  const eventName = chromiumPresentationEventName(sampled)
  const presentations = eventName ? sampled.filter(event => event.name === eventName) : []
  const timestamps = [...new Set(presentations.map(event => event.ts!))].sort((left, right) => left - right)
  const intervals = timestamps.slice(1).map((timestamp, index) => (timestamp - timestamps[index]!) / 1_000)
  return Object.freeze({
    evidence: timestamps.length ? "chromium-compositor-presentation-trace" as const : "unavailable" as const,
    presentedFrames: timestamps.length || null,
    presentedFramesPerSecond: timestamps.length ? Number((timestamps.length / elapsedMilliseconds * 1_000).toFixed(3)) : null,
    intervals: timestamps.length > 1 ? summarizeFrameTimes(intervals) : null,
    eventNames: [...new Set(presentations.map(event => event.name!))].sort(),
    traceEvents: events.length,
  })
}

export function analyzeCompositorStalls(
  events: readonly ChromiumTraceEvent[],
  window: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>,
  lifecycle: readonly Readonly<{ at: number; phase: string; playerClass?: number }>[],
  minimumMilliseconds = 50,
) {
  const sampled = events.filter(event => Number.isFinite(event.ts) && event.ts! >= window.startedMicroseconds && event.ts! <= window.endedMicroseconds)
  const name = chromiumPresentationEventName(sampled)
  if (!name) return []
  const timestamps = [...new Set(sampled.filter(event => event.name === name).map(event => event.ts!))].sort((left, right) => left - right)
  return timestamps.slice(1).flatMap((ended, index) => {
    const started = timestamps[index]!
    if (ended - started < minimumMilliseconds * 1_000) return []
    const startedMilliseconds = (started - window.startedMicroseconds) / 1_000
    const endedMilliseconds = (ended - window.startedMicroseconds) / 1_000
    const overlapping = sampled.filter(event => event.ts! < ended && (event.ts! + (event.dur ?? 0)) > started)
    return [{
      milliseconds: Number(((ended - started) / 1_000).toFixed(3)),
      startedMilliseconds: Number(startedMilliseconds.toFixed(3)),
      endedMilliseconds: Number(endedMilliseconds.toFixed(3)),
      classes: lifecycle.filter(event => event.at >= startedMilliseconds && event.at <= endedMilliseconds),
      beginFrames: overlapping.filter(event => /BeginFrame|SurfaceFrame/u.test(event.name ?? "")).length,
      work: overlapping.filter(event => (event.dur ?? 0) >= 1_000).map(event => ({
        name: event.name ?? "unknown",
        milliseconds: Number((event.dur! / 1_000).toFixed(3)),
        overlapMilliseconds: Number((Math.min(ended, event.ts! + event.dur!) - Math.max(started, event.ts!)).toFixed(0)) / 1_000,
        ...(event.pid === undefined ? {} : { pid: event.pid }),
        ...(event.tid === undefined ? {} : { tid: event.tid }),
        ...(event.cat === undefined ? {} : { category: event.cat }),
      })).sort((left, right) => right.overlapMilliseconds - left.overlapMilliseconds).slice(0, 16),
    }]
  }).sort((left, right) => right.milliseconds - left.milliseconds).slice(0, 12)
}

export function summarizeCompositorStages(events: readonly ChromiumTraceEvent[], window?: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>) {
  const stages = ["BeginFrame", "BeginMainFrame", "DrawFrame", "SubmitCompositorFrame", "FramePresented"] as const
  return Object.freeze(Object.fromEntries(stages.map(name => {
    const sampled = events.filter(event => event.name === name && Number.isFinite(event.ts)
      && (!window || event.ts! >= window.startedMicroseconds && event.ts! <= window.endedMicroseconds))
    const timestamps = [...new Set(sampled.map(event => event.ts!))].sort((left, right) => left - right)
    return [name, { count: timestamps.length, intervals: summarizeFrameTimes(timestamps.slice(1).map((timestamp, index) => (timestamp - timestamps[index]!) / 1_000)) }]
  })))
}

export function assertVisibleGameplayTruth(evidence: Readonly<{ visible: boolean; focused: boolean; ticks: number; displayFrames: number; submissions: number; beforeSha256: string; afterSha256: string }>): void {
  if (!evidence.visible) throw new Error("Gameplay evidence rejected: browser document was hidden")
  if (!evidence.focused) throw new Error("Gameplay evidence rejected: browser document was not focused")
  if (evidence.ticks <= 0) throw new Error("Gameplay evidence rejected: simulation did not advance")
  if (evidence.displayFrames <= 0) throw new Error("Gameplay evidence rejected: application did not complete display frames")
  if (evidence.submissions <= 0) throw new Error("Gameplay evidence rejected: no GPU command buffers were submitted")
  if (evidence.beforeSha256 === evidence.afterSha256) throw new Error("Gameplay evidence rejected: visible canvas pixels did not change")
}
