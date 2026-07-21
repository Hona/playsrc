export class RequiredParticleDisplayQueue<T> {
  readonly #limit: number
  #frames: T[] = []
  #effects = new Set<number>()

  constructor(limit: number) {
    if (!Number.isSafeInteger(limit) || limit < 1) throw new Error("Required Particle display limit is invalid")
    this.#limit = limit
  }

  admit(frame: T, effectIdentities: readonly number[]): void {
    if (effectIdentities.some((identity) => !Number.isSafeInteger(identity) || identity < 1)) {
      throw new Error("Required Particle display identity is invalid")
    }
    const next = new Set(effectIdentities)
    if ([...next].some((identity) => !this.#effects.has(identity))) {
      if (this.#frames.length >= this.#limit) throw new Error("Required Particle display queue reached its explicit limit")
      this.#frames.push(frame)
    }
    this.#effects = next
  }

  peek(): T | undefined { return this.#frames[0] }

  complete(frame: T): void {
    if (this.#frames[0] === frame) this.#frames.shift()
  }

  reset(): void {
    this.#frames = []
    this.#effects.clear()
  }
}
