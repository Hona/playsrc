export type TeamModelPlayback = Readonly<{
  modelRevision: number
  animationRevision: number
  sequence: string
  startedSeconds: number
  previousSeconds: number
  lastPaintSeconds: number
  sampledSeconds: number | null
  transitioning: boolean
}>

/** Presentation scheduling only; the studio owner resolves cycle and pose. */
export function teamModelPlayback(
  prior: TeamModelPlayback | undefined,
  panel: Readonly<{ modelRevision: number; animationRevision: number; sequence: string }>,
  now: number,
  duration: number,
): Readonly<{ state: TeamModelPlayback; elapsed: number; previousElapsed: number; frameTime: number; sample: boolean; reset: boolean }> {
  const reset = !prior || prior.modelRevision !== panel.modelRevision
  const changed = reset || prior?.animationRevision !== panel.animationRevision
  const current: TeamModelPlayback = changed
    ? { ...panel, startedSeconds: now, previousSeconds: 0, lastPaintSeconds: now, sampledSeconds: null, transitioning: false }
    : prior!
  const elapsed = Math.min(Math.max(0, now - current.startedSeconds), duration)
  return {
    state: { ...current, previousSeconds: elapsed, lastPaintSeconds: now },
    elapsed,
    previousElapsed: Math.min(current.previousSeconds, elapsed),
    frameTime: Math.max(0, now - (prior?.lastPaintSeconds ?? now)),
    sample: current.sampledSeconds === null || current.sampledSeconds < duration || current.transitioning,
    reset,
  }
}
