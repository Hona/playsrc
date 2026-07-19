export type Tf2PresentationRandomSnapshot = Readonly<{
  seed: number
  state: number
  current: number
  shuffle: readonly number[]
  draws: number
}>

export type Tf2PresentationRandom = Readonly<{
  nextUnit(): number
  nextInteger(minimum: number, maximum: number): number
  snapshot(): Tf2PresentationRandomSnapshot
  restore(snapshot: Tf2PresentationRandomSnapshot): void
}>

const MODULUS = 2_147_483_647
const MULTIPLIER = 16_807
const TABLE_SIZE = 32
const DIVISOR = 1 + Math.trunc((MODULUS - 1) / TABLE_SIZE)
const MAXIMUM_UNIT = 1 - 1.2e-7

class PresentationRandom implements Tf2PresentationRandom {
  readonly #seed: number
  #state: number
  #current = 0
  #shuffle = new Array<number>(TABLE_SIZE).fill(0)
  #draws = 0

  constructor(seed: number) {
    this.#seed = seed
    this.#state = seed < 0 ? seed : -seed
  }

  #advanceState(): number {
    this.#state = Number((BigInt(MULTIPLIER) * BigInt(this.#state)) % BigInt(MODULUS))
    if (this.#state < 0) this.#state += MODULUS
    return this.#state
  }

  #initialize(): void {
    let state = Math.max(Math.abs(this.#state), 1)
    this.#state = state
    const generated: number[] = []
    for (let index = 0; index < TABLE_SIZE + 8; index += 1) {
      state = Number((BigInt(MULTIPLIER) * BigInt(state)) % BigInt(MODULUS))
      generated.push(state)
    }
    this.#state = state
    for (let index = 0; index < TABLE_SIZE; index += 1) this.#shuffle[index] = generated[TABLE_SIZE + 7 - index]!
    this.#current = this.#shuffle[0]!
  }

  #number(): number {
    if (this.#state <= 0 || this.#current === 0) this.#initialize()
    const next = this.#advanceState()
    const index = Math.max(0, Math.min(TABLE_SIZE - 1, Math.trunc(this.#current / DIVISOR)))
    this.#current = this.#shuffle[index]!
    this.#shuffle[index] = next
    this.#draws += 1
    return this.#current
  }

  nextUnit(): number { return Math.min(this.#number() / MODULUS, MAXIMUM_UNIT) }

  nextInteger(minimum: number, maximum: number): number {
    if (!Number.isSafeInteger(minimum) || !Number.isSafeInteger(maximum) || maximum < minimum) throw new Error("TF2 presentation random interval is invalid")
    const range = maximum - minimum + 1
    if (!Number.isSafeInteger(range) || range < 1 || range > 0x8000_0000) throw new Error("TF2 presentation random interval exceeds its bound")
    if (range === 1) return minimum
    const maximumAccepted = 0x7fff_ffff - ((0x8000_0000) % range)
    let value: number
    do value = this.#number()
    while (value > maximumAccepted)
    return minimum + (value % range)
  }

  snapshot(): Tf2PresentationRandomSnapshot {
    return Object.freeze({ seed: this.#seed, state: this.#state, current: this.#current, shuffle: Object.freeze([...this.#shuffle]), draws: this.#draws })
  }

  restore(snapshot: Tf2PresentationRandomSnapshot): void {
    if (!snapshot || snapshot.seed !== this.#seed || !Number.isSafeInteger(snapshot.state) || snapshot.state < -0x7fff_ffff || snapshot.state >= MODULUS
      || !Number.isSafeInteger(snapshot.current) || snapshot.current < 0 || snapshot.current >= MODULUS
      || !Array.isArray(snapshot.shuffle) || snapshot.shuffle.length !== TABLE_SIZE || snapshot.shuffle.some((value) => !Number.isSafeInteger(value) || value < 0 || value >= MODULUS)
      || !Number.isSafeInteger(snapshot.draws) || snapshot.draws < 0) throw new Error("TF2 presentation random snapshot is invalid")
    this.#current = snapshot.current
    this.#state = snapshot.state
    this.#shuffle = [...snapshot.shuffle]
    this.#draws = snapshot.draws
  }
}

export function createTf2PresentationRandom(seed: number): Tf2PresentationRandom {
  if (!Number.isSafeInteger(seed) || seed < -0x7fff_ffff || seed > 0x7fff_ffff) throw new Error("TF2 presentation random seed is invalid")
  return Object.freeze(new PresentationRandom(seed))
}
