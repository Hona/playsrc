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

/** Templates already contain sequence zero at cycle zero. Keep those pixels
 * and their static GPU pipeline until an actual pose differs; once deformed,
 * idle still has to restore the retained skinned instance. */
export class TeamModelSkinning {
  readonly #templates = new Map<string, Float32Array>()
  readonly #deformed = new Set<string>()

  observe(name: string, pose: Readonly<{ sequence: number; cycle: number; boneMatrices: Float32Array; flex: readonly unknown[] }>): void {
    if (this.#deformed.has(name)) return
    const template = this.#templates.get(name)
    if (!template && pose.sequence === 0 && pose.cycle === 0 && pose.flex.length === 0) {
      this.#templates.set(name, pose.boneMatrices)
      return
    }
    if (!template || pose.flex.length !== 0 || template.length !== pose.boneMatrices.length
      || template.some((value, index) => value !== pose.boneMatrices[index])) this.#deformed.add(name)
  }

  needsPose(name: string): boolean { return this.#deformed.has(name) }
  clear(): void { this.#templates.clear(); this.#deformed.clear() }
}

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
    ? { ...panel, startedSeconds: reset ? now : prior!.lastPaintSeconds, previousSeconds: 0, lastPaintSeconds: now, sampledSeconds: null, transitioning: false }
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
