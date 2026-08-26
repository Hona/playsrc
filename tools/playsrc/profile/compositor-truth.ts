import { summarizeFrameTimes } from "./profile-window"

export type ChromiumTraceEvent = Readonly<{ name?: string; ts?: number; dur?: number; args?: Record<string, any> }>

const PRESENTATION_EVENTS = ["PresentationFeedback", "Display::FrameDisplayed", "FramePresented"] as const

export function summarizeCompositorTruth(events: readonly ChromiumTraceEvent[], elapsedMilliseconds: number, window?: Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>) {
  if (!Number.isFinite(elapsedMilliseconds) || elapsedMilliseconds <= 0) throw new Error("Compositor sampling duration must be positive")
  if (window && (!Number.isFinite(window.startedMicroseconds) || !Number.isFinite(window.endedMicroseconds) || window.endedMicroseconds < window.startedMicroseconds)) throw new Error("Compositor sampling window is invalid")
  const sampled = events.filter(event => Number.isFinite(event.ts) && (!window || event.ts! >= window.startedMicroseconds && event.ts! <= window.endedMicroseconds))
  const eventName = PRESENTATION_EVENTS.find(name => sampled.some(event => event.name === name))
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

export function assertVisibleGameplayTruth(evidence: Readonly<{ visible: boolean; focused: boolean; ticks: number; displayFrames: number; submissions: number; beforeSha256: string; afterSha256: string }>): void {
  if (!evidence.visible) throw new Error("Gameplay evidence rejected: browser document was hidden")
  if (!evidence.focused) throw new Error("Gameplay evidence rejected: browser document was not focused")
  if (evidence.ticks <= 0) throw new Error("Gameplay evidence rejected: simulation did not advance")
  if (evidence.displayFrames <= 0) throw new Error("Gameplay evidence rejected: application did not complete display frames")
  if (evidence.submissions <= 0) throw new Error("Gameplay evidence rejected: no GPU command buffers were submitted")
  if (evidence.beforeSha256 === evidence.afterSha256) throw new Error("Gameplay evidence rejected: visible canvas pixels did not change")
}
