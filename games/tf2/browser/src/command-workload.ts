import type { WorkerRequest, WorkerResponse } from "./protocol"

type WithoutId<T> = T extends unknown ? Omit<T, "id"> : never
type Request = WithoutId<WorkerRequest>
export type WorkloadMutation = Readonly<{ kind: 4 | 5 | 6 | 9 | 10; hex: string }>
export type WorkloadObserve = Readonly<{ nowSeconds: number; suspended: boolean; snapshotTick: string; command: string; mutations: readonly WorkloadMutation[] }>
export type CommandWorkload = Readonly<{
  schema: 1; journalSha256: string; bspSha256: string; configurationSha256: string; configurationBytes: number
  profile: 0 | 1; generation: number; sampleStarted: number; sampleEnded: number
  observes: readonly WorkloadObserve[]
  clientFrames?: readonly import("./client-render-frame").RecordedClientRenderFrame[]
  presentations?: readonly Readonly<{ atSeconds: number; firstHostTick: string; lastHostTick: string; selectedTicks: number }>[]
}>
export function workloadBytes(hex: string): Uint8Array<ArrayBuffer> {
  if (!/^(?:[0-9a-f]{2})+$/.test(hex) || hex.length > 131192) throw new Error("Invalid workload bytes")
  return Uint8Array.from(hex.match(/../g)!, value => parseInt(value, 16))
}
export function validateWorkload(plan: CommandWorkload) {
  if (plan.schema !== 1 || ![plan.journalSha256, plan.bspSha256, plan.configurationSha256].every(v => /^[0-9a-f]{64}$/.test(v))
    || !Number.isSafeInteger(plan.configurationBytes) || plan.configurationBytes < 12
    || ![0, 1].includes(plan.profile) || !Number.isSafeInteger(plan.generation) || plan.generation < 1
    || !Number.isFinite(plan.sampleStarted) || !Number.isFinite(plan.sampleEnded)
    || plan.sampleEnded - plan.sampleStarted < 5000 || plan.sampleEnded - plan.sampleStarted > 11000
    || plan.observes.length < 2 || plan.observes.length > 16384) throw new Error("Invalid workload identity/phase")
  let previous = -Infinity, records = 0, inputBytes = 0
  for (const entry of plan.observes) {
    if (!Number.isFinite(entry.nowSeconds) || entry.nowSeconds < previous || !/^[0-9]+$/.test(entry.snapshotTick)
      || typeof entry.suspended !== "boolean" || workloadBytes(entry.command).length < 84 || entry.mutations.length > 1024) throw new Error("Invalid workload observe")
    previous = entry.nowSeconds
    records += 1 + entry.mutations.length
    inputBytes += entry.command.length / 2
    for (const mutation of entry.mutations) {
      if (![4, 5, 6, 9, 10].includes(mutation.kind)) throw new Error("Unsupported workload mutation")
      inputBytes += workloadBytes(mutation.hex).length
    }
  }
  if (records > 16384 || inputBytes > 4 * 1024 * 1024) throw new Error("Workload record/input bound exceeded")
  if (plan.presentations) {
    if (!plan.presentations.length || plan.presentations.length > 16384) throw new Error("Invalid presentation workload count")
    let previousTick: bigint | undefined, previousTime = -Infinity
    for (const entry of plan.presentations) {
      if (!/^\d+$/.test(entry.firstHostTick) || !/^\d+$/.test(entry.lastHostTick) || !Number.isFinite(entry.atSeconds)
        || entry.atSeconds < previousTime || !Number.isSafeInteger(entry.selectedTicks) || entry.selectedTicks < 1) throw new Error("Invalid presentation workload input")
      const first = BigInt(entry.firstHostTick), last = BigInt(entry.lastHostTick)
      if (last < first || last - first + 1n !== BigInt(entry.selectedTicks) || previousTick !== undefined && first !== previousTick + 1n) throw new Error("Presentation workload tick coverage differs")
      previousTick = last; previousTime = entry.atSeconds
    }
  }
  if (plan.sampleStarted < plan.observes[0]!.nowSeconds * 1000 || plan.sampleEnded > previous * 1000) throw new Error("Workload does not cover its measured phase")
}

/** Local development replay input owner. It never advances clocks or simulation:
 * original timestamps/commands reach the normal observe API at their real-time
 * deadlines. A late caller stays late; no fast-forward, resampling or reset. */
export class CommandWorkloadPlayer {
  #cursor = 0
  #epoch: number | undefined
  #closed = false
  #pending = false
  #mutations: Array<{ value: WorkloadMutation; claimed: boolean; response?: WorkerResponse }>
  #mutationCursor = 0
  #prefix: Promise<void> | undefined
  readonly receipt: { journalSha256: string; epoch?: number; scheduled: number; released: number; lateMilliseconds: number; cursor: number; yaw: number; pitch: number; ignoredLiveInputs: number;
    publicationQueueHighWater: number; publicationQueueBytesHighWater: number; consumedPackets: number;
    handoff?: { realMilliseconds: number; sourceSeconds: number; unadmittedLiveSamples: number };
    observations: Array<{ index: number; scheduled: number; released: number; waitMilliseconds: number; lateMilliseconds: number }> } = {
    journalSha256: "", scheduled: 0, released: 0, lateMilliseconds: 0, cursor: 0, yaw: 0, pitch: 0, ignoredLiveInputs: 0, observations: [],
    publicationQueueHighWater: 0, publicationQueueBytesHighWater: 0, consumedPackets: 0,
  }
  constructor(readonly plan: CommandWorkload, readonly generation: number,
    private readonly send: (request: Request, transfer?: Transferable[]) => Promise<WorkerResponse>,
    private readonly now: () => number = () => performance.now(),
    private readonly wait: (ms: number) => Promise<void> = ms => new Promise(resolve => setTimeout(resolve, ms))) {
    validateWorkload(plan); this.receipt.journalSha256 = plan.journalSha256
    this.#mutations = plan.observes.flatMap(entry => entry.mutations.map(value => ({ value, claimed: false })))
  }
  close() { this.#closed = true }
  get ended() { return this.#cursor === this.plan.observes.length }
  get epoch() { return this.#epoch }
  #initialMutations() {
    return this.#prefix ??= (async () => {
      for (const mutation of this.plan.observes[0]!.mutations) {
        if (this.#closed) throw new Error("Workload cancelled")
        await this.#mutate(mutation)
      }
    })()
  }
  async next(snapshotTick: bigint, sampledAt = this.now()): Promise<Readonly<{ command: ArrayBuffer; nowSeconds: number; suspended: boolean; due: number }>> {
    if (this.#closed || this.#pending) throw new Error("Workload closed or concurrently consumed")
    const entry = this.plan.observes[this.#cursor]
    if (!entry) throw new Error("Authenticated workload exhausted")
    if (BigInt(entry.snapshotTick) !== snapshotTick) throw new Error("Workload publication acknowledgement differs")
    this.#pending = true
    try {
      await this.#initialMutations()
      this.#epoch ??= sampledAt - entry.nowSeconds * 1000
      const due = this.#epoch + entry.nowSeconds * 1000
      const began = this.now()
      while (!this.#closed && this.now() < due) await this.wait(Math.min(20, due - this.now()))
      const waited = this.now() - began
      if (this.#closed) throw new Error("Workload cancelled")
      for (const mutation of this.#cursor === 0 ? [] : entry.mutations) {
        if (this.#closed) throw new Error("Workload cancelled")
        await this.#mutate(mutation)
      }
      if (this.#closed) throw new Error("Workload cancelled")
      const bytes = workloadBytes(entry.command), view = new DataView(bytes.buffer)
      const released = this.now()
      Object.assign(this.receipt, { epoch: this.#epoch, scheduled: due, released, lateMilliseconds: released - due,
        cursor: ++this.#cursor, yaw: view.getFloat32(20, true), pitch: view.getFloat32(24, true) })
      this.receipt.observations.push({ index: this.#cursor - 1, scheduled: due, released, waitMilliseconds: waited, lateMilliseconds: released - due })
      return { command: bytes.buffer, nowSeconds: entry.nowSeconds, suspended: entry.suspended, due }
    } finally { this.#pending = false }
  }
  /** The recorded workload, not live UI timing, owns mutations during playback.
   * Reads still reach the real owner. No fake map/session state is returned. */
  redirect(request: Request): Request | WorkerResponse | Promise<WorkerResponse> {
    if (!("generation" in request) || request.generation !== this.generation) return request
    let intent: WorkloadMutation | undefined
    const hex = (bytes: Uint8Array) => Array.from(bytes, b => b.toString(16).padStart(2, "0")).join("")
    if (request.kind === "team-selection" && request.choice !== null) {
      const bytes = new Uint8Array(4)
      new DataView(bytes.buffer).setUint32(0, { red: 2, blue: 3, spectate: 1, auto: 4 }[request.choice], true)
      intent = { kind: 4, hex: hex(bytes) }
    }
    if (request.kind === "equipment" && request.mutation) intent = { kind: 9, hex: hex(new Uint8Array(request.mutation)) }
    if (request.kind === "configure-course") intent = { kind: 6, hex: hex(new Uint8Array(request.definition)) }
    if (request.kind === "set-position") {
      const bytes = new Uint8Array(12), view = new DataView(bytes.buffer)
      request.position.forEach((v, i) => view.setFloat32(i * 4, v, true))
      intent = { kind: 5, hex: hex(bytes) }
    }
    if (request.kind === "entity-input") {
      const text = new TextEncoder().encode(`${request.target}\0${request.input}\0${request.value}`)
      const bytes = new Uint8Array(4 + text.length); new DataView(bytes.buffer).setFloat32(0, request.delay, true); bytes.set(text, 4)
      intent = { kind: 10, hex: hex(bytes) }
    }
    if (intent) {
      const next = this.#mutations.find(record => !record.claimed && record.value.kind === intent!.kind && record.value.hex === intent!.hex)
      if (!next) throw new Error("Live mutation intent differs from authenticated workload")
      next.claimed = true; this.receipt.ignoredLiveInputs++
      return (async () => {
        // Pre-baseline mutations have ordering but no recorded host-clock
        // timestamp. Apply that exact prefix at the ordinary live admission,
        // otherwise a team-selection acknowledgement would deadlock map startup.
        if (this.#epoch === undefined && this.plan.observes[0]!.mutations.includes(next.value)) await this.#initialMutations()
        while (!next.response && !this.#closed && !this.ended) await this.wait(10)
        if (!next.response) throw new Error("Recorded mutation acknowledgement unavailable")
        return next.response
      })()
    }
    return request
  }
  async #mutate(mutation: WorkloadMutation) {
    const bytes = workloadBytes(mutation.hex), view = new DataView(bytes.buffer), generation = this.generation
    const vector = (at: number) => [view.getFloat32(at, true), view.getFloat32(at + 4, true), view.getFloat32(at + 8, true)] as const
    let request: Request
    if (mutation.kind === 4 && bytes.length === 4) {
      const choice = [null, "spectate", "red", "blue", "auto"][view.getUint32(0, true)] as "red" | "blue" | "spectate" | "auto" | undefined
      if (!choice) throw new Error("Invalid workload team")
      request = { kind: "team-selection", generation, choice }
    } else if (mutation.kind === 5 && bytes.length === 12) request = { kind: "set-position", generation, position: vector(0) }
    else if (mutation.kind === 6) request = { kind: "configure-course", generation, definition: bytes.buffer }
    else if (mutation.kind === 9) request = { kind: "equipment", generation, mutation: bytes.buffer }
    else if (mutation.kind === 10) {
      const [target, input, value] = new TextDecoder().decode(bytes.subarray(4)).split("\0")
      if (!target || !input || value === undefined) throw new Error("Invalid workload entity input")
      request = { kind: "entity-input", generation, target, input, value, delay: view.getFloat32(0, true) }
    } else throw new Error("Unsupported workload mutation")
    const response = await this.send(request)
    const record = this.#mutations[this.#mutationCursor++]
    if (!record || record.value.hex !== mutation.hex || record.value.kind !== mutation.kind) throw new Error("Workload mutation order differs")
    record.response = response
  }
}
