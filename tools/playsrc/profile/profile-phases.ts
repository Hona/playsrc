export class ProfilePhases {
  readonly #started = performance.now()
  readonly #spans: Array<{ name: string; startedMilliseconds: number; durationMilliseconds: number; complete: boolean }> = []
  #name = "setup"
  #entered = this.#started

  enter(name: string): void {
    this.#finish(true)
    this.#name = name
    this.#entered = performance.now()
  }

  #finish(complete: boolean): void {
    this.#spans.push({ name: this.#name, startedMilliseconds: this.#entered - this.#started,
      durationMilliseconds: performance.now() - this.#entered, complete })
  }

  finish(complete: boolean) {
    this.#finish(complete)
    return { elapsedMilliseconds: performance.now() - this.#started, spans: this.#spans }
  }
}
