export class DisplayBackpressure {
  #frame = 0
  #startedFrame = -1
  #pending = false

  get frame(): number { return this.#frame }
  get pending(): boolean { return this.#pending }

  advance(): void { this.#frame += 1 }

  begin(): void {
    if (this.#startedFrame !== -1) throw new Error("Display presentation is already in progress")
    this.#startedFrame = this.#frame
    this.#pending = false
  }

  defer(): boolean {
    if (this.#startedFrame === -1) return false
    const first = !this.#pending
    this.#pending = true
    return first
  }

  complete(): boolean {
    if (this.#startedFrame === -1) return false
    const retry = this.#pending && this.#frame !== this.#startedFrame
    this.#startedFrame = -1
    this.#pending = false
    return retry
  }

  reset(): void {
    this.#startedFrame = -1
    this.#pending = false
  }
}
