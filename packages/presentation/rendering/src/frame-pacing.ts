export type FramePacingRecord = Readonly<{
  ordinal: number
  opportunityMilliseconds: number
  disposition: "accepted" | "coalesced" | "suspended" | "stopped"
  startMilliseconds: number | null
  submitMilliseconds: number | null
  presentMilliseconds: number | null
  error: string | null
}>

export type FramePacingClock = Readonly<{ now(): number }>
export type FramePacingWork = () => Promise<Readonly<{ submitMilliseconds: number; presentMilliseconds: number }>>

export class FramePacingError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "FramePacingError"
  }
}

export class FramePacingController {
  readonly #clock: FramePacingClock
  readonly #maximumRecords: number
  #records: FramePacingRecord[] = []
  #ordinal = 0
  #running = false
  #suspended = false
  #inFlight = false

  constructor(clock: FramePacingClock, maximumRecords: number) {
    if (!clock || typeof clock.now !== "function" || !Number.isSafeInteger(maximumRecords) || maximumRecords < 1 || maximumRecords > 65_536) {
      throw new FramePacingError("frame-pacing configuration is invalid")
    }
    this.#clock = clock
    this.#maximumRecords = maximumRecords
  }

  start(): void {
    if (this.#running) throw new FramePacingError("frame pacing is already active")
    this.#running = true
  }

  stop(): void {
    this.#running = false
  }

  suspend(value: boolean): void {
    this.#suspended = value
  }

  async offer(opportunityMilliseconds: number, work: FramePacingWork): Promise<FramePacingRecord> {
    if (!Number.isFinite(opportunityMilliseconds)) throw new FramePacingError("frame opportunity is invalid")
    const ordinal = ++this.#ordinal
    if (!this.#running) return this.#record({ ordinal, opportunityMilliseconds, disposition: "stopped", startMilliseconds: null, submitMilliseconds: null, presentMilliseconds: null, error: null })
    if (this.#suspended) return this.#record({ ordinal, opportunityMilliseconds, disposition: "suspended", startMilliseconds: null, submitMilliseconds: null, presentMilliseconds: null, error: null })
    if (this.#inFlight) return this.#record({ ordinal, opportunityMilliseconds, disposition: "coalesced", startMilliseconds: null, submitMilliseconds: null, presentMilliseconds: null, error: null })
    this.#inFlight = true
    const startMilliseconds = this.#clock.now()
    try {
      const result = await work()
      if (
        !Number.isFinite(result.submitMilliseconds) || !Number.isFinite(result.presentMilliseconds)
        || result.submitMilliseconds < startMilliseconds || result.presentMilliseconds < result.submitMilliseconds
      ) throw new FramePacingError("frame timing result is invalid")
      return this.#record({ ordinal, opportunityMilliseconds, disposition: "accepted", startMilliseconds, ...result, error: null })
    } catch (error) {
      const record = this.#record({
        ordinal, opportunityMilliseconds, disposition: "accepted", startMilliseconds,
        submitMilliseconds: null, presentMilliseconds: null, error: String(error),
      })
      throw new FramePacingError(`paced frame ${record.ordinal} failed: ${record.error}`)
    } finally {
      this.#inFlight = false
    }
  }

  records(): readonly FramePacingRecord[] {
    return Object.freeze(this.#records.map((record) => Object.freeze({ ...record })))
  }

  #record(record: FramePacingRecord): FramePacingRecord {
    const retained = Object.freeze({ ...record })
    this.#records.push(retained)
    if (this.#records.length > this.#maximumRecords) this.#records.splice(0, this.#records.length - this.#maximumRecords)
    return retained
  }
}
