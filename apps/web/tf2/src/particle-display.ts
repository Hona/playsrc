export class RequiredParticleDisplayQueue<T> {
  readonly #limit: number
  readonly #displayCount: number
  #frames: Array<{ frame: T; remaining: number }> = []
  #effects = new Set<number>()

  constructor(limit: number, displayCount: number) {
    if (!Number.isSafeInteger(limit) || limit < 1 || !Number.isSafeInteger(displayCount) || displayCount < 2) throw new Error("Required Particle display limit is invalid")
    this.#limit = limit
    this.#displayCount = displayCount
  }

  admit(frame: T, effectIdentities: readonly number[]): void {
    if (effectIdentities.some((identity) => !Number.isSafeInteger(identity) || identity < 1)) {
      throw new Error("Required Particle display identity is invalid")
    }
    const next = new Set(effectIdentities)
    if ([...next].some((identity) => !this.#effects.has(identity))) {
      if (this.#frames.length >= this.#limit) throw new Error("Required Particle display queue reached its explicit limit")
      this.#frames.push({ frame, remaining: this.#displayCount })
    }
    this.#effects = next
  }

  peek(): T | undefined { return this.#frames[0]?.frame }

  complete(frame: T): void {
    const current = this.#frames[0]
    if (current?.frame !== frame) return
    current.remaining -= 1
    if (current.remaining === 0) this.#frames.shift()
  }

  reset(): void {
    this.#frames = []
    this.#effects.clear()
  }
}
