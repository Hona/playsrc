export class RetainedModelCache<Value> {
  readonly #entries = new Map<string, Value>()
  readonly #capacity: number
  readonly #dispose: (value: Value) => void

  constructor(capacity: number, dispose: (value: Value) => void) {
    if (!Number.isSafeInteger(capacity) || capacity < 1) throw new TypeError("retained model cache capacity is invalid")
    this.#capacity = capacity
    this.#dispose = dispose
  }

  take(identity: string): Value | undefined {
    const value = this.#entries.get(identity)
    if (value !== undefined) this.#entries.delete(identity)
    return value
  }

  retain(identity: string, value: Value): void {
    const previous = this.#entries.get(identity)
    if (previous !== undefined) {
      this.#entries.delete(identity)
      if (previous !== value) this.#dispose(previous)
    }
    this.#entries.set(identity, value)
    while (this.#entries.size > this.#capacity) {
      const oldest = this.#entries.keys().next().value as string
      const expired = this.#entries.get(oldest)!
      this.#entries.delete(oldest)
      this.#dispose(expired)
    }
  }

  clear(): void {
    for (const value of this.#entries.values()) this.#dispose(value)
    this.#entries.clear()
  }

  get size(): number { return this.#entries.size }
}
