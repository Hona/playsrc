export const MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS = 64

export type SimulationClockSample = Readonly<{
  generation: number
  nowSeconds: number
  suspended: boolean
}>

export class SimulationClockQueue {
  readonly #pending: SimulationClockSample[] = []

  get length(): number {
    return this.#pending.length
  }

  push(sample: SimulationClockSample): "queued" | "coalesced" {
    const previous = this.#pending.at(-1)
    if (previous?.generation === sample.generation && previous.suspended === sample.suspended) {
      this.#pending[this.#pending.length - 1] = sample
      return "coalesced"
    }
    if (this.#pending.length >= MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS) {
      throw new RangeError("Simulation lifecycle transition queue reached its explicit limit")
    }
    this.#pending.push(sample)
    return "queued"
  }

  shift(): SimulationClockSample | undefined {
    return this.#pending.shift()
  }

  clear(): void {
    this.#pending.length = 0
  }
}
