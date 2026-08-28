import type { WorkerResponse, WorkerTransactionTimings } from "./protocol"

// One bounded FIFO for both ready shared replies and structured-clone controls.
// The ring owns metadata only. Rust owns each immutable byte lease until the
// reader has copied it (or the existing model decoder releases its own lease).
export const REPLY_CAPACITY = 64
const HEADER = 16, STRIDE = 128
export const REPLY_BYTES = HEADER + STRIDE * REPLY_CAPACITY
export type ReplyControl = Readonly<{ kind: "reply-control"; sequence: number; response: WorkerResponse }>
export type ReplyMemory = Readonly<{ mailbox: SharedArrayBuffer; memory: WebAssembly.Memory; modelOwnership: SharedArrayBuffer }>
export type ReplyRange = Readonly<{ pointer: number; length: number }>
export type SharedReply = Readonly<{
  id: number; generation: number; kind: "simulation" | "particles" | "models" | "visibility" | "acoustics"
  ranges: readonly ReplyRange[]; timings: WorkerTransactionTimings
  acoustic?: boolean
  slot?: number; replayAttack?: Readonly<{ hostTick: bigint; playerClass: number; weapon: number; lifecycle: number }>
}>
const tags = ["simulation", "particles", "models", "visibility", "acoustics"] as const
const rangeOffsets = [16, 24, 40] as const
const timingKeys = ["queueMilliseconds", "inputCopyMilliseconds", "transactMilliseconds", "outputCopyMilliseconds", "totalMilliseconds"] as const
const memoryKeys = ["wasmLinearMemoryBytes", "wasmAllocatorLiveBytes", "wasmAllocatorHighWaterBytes"] as const

function validate(mailbox: SharedArrayBuffer): void {
  if (!(mailbox instanceof SharedArrayBuffer) || mailbox.byteLength !== REPLY_BYTES) throw new Error("Invalid reply mailbox")
}

export class ReplyWriter {
  readonly #control: Int32Array
  readonly #bytes: DataView
  readonly #releases = new Map<number, () => void>()
  #written = 0
  #reclaimed = 0
  constructor(readonly mailbox: SharedArrayBuffer) {
    validate(mailbox)
    this.#control = new Int32Array(mailbox, 0, 4)
    this.#bytes = new DataView(mailbox)
  }
  reclaim(): void {
    const read = Atomics.load(this.#control, 1) >>> 0
    if (((read - this.#reclaimed) >>> 0) > REPLY_CAPACITY) throw new Error("Invalid reply acknowledgement")
    while (this.#reclaimed !== read) {
      this.#reclaimed = (this.#reclaimed + 1) >>> 0
      this.#releases.get(this.#reclaimed)?.()
      this.#releases.delete(this.#reclaimed)
    }
  }
  #reserve(): { at: number; sequence: number } {
    this.reclaim()
    if (Atomics.load(this.#control, 2) !== 0) throw new Error("Reply reader closed")
    if (((this.#written - this.#reclaimed) >>> 0) >= REPLY_CAPACITY) throw new Error("Reply mailbox bound exceeded")
    return { at: HEADER + (this.#written % REPLY_CAPACITY) * STRIDE, sequence: (this.#written + 1) >>> 0 }
  }
  #publish(sequence: number): void {
    this.#written = sequence
    Atomics.store(this.#control, 0, sequence | 0)
    Atomics.notify(this.#control, 0)
  }
  control(response: WorkerResponse, send: (message: ReplyControl) => void): void {
    const { at, sequence } = this.#reserve()
    this.#bytes.setUint32(at, 0, true)
    this.#bytes.setUint32(at + 4, response.id, true)
    send({ kind: "reply-control", sequence, response })
    this.#publish(sequence)
  }
  shared(reply: SharedReply, release: () => void): void {
    const { at, sequence } = this.#reserve()
    const acoustic = reply.kind === "visibility" && reply.acoustic === true
    if (reply.ranges.length < 1 + Number(acoustic) || reply.ranges.length > (reply.kind === "visibility" ? 2 + Number(acoustic) : 2)
      || (reply.kind !== "visibility" && reply.kind !== "particles" && reply.ranges.length !== 1)) throw new Error("Invalid reply ranges")
    this.#bytes.setUint32(at, tags.indexOf(reply.kind) + 1, true)
    this.#bytes.setUint32(at + 4, reply.id, true)
    this.#bytes.setUint32(at + 8, reply.generation, true)
    this.#bytes.setUint32(at + 12, reply.ranges.length, true)
    for (let index = 0; index < reply.ranges.length; index++) {
      const range = reply.ranges[index]!
      if (!Number.isSafeInteger(range.pointer) || range.pointer < 1 || range.pointer > 0xffff_ffff
        || !Number.isSafeInteger(range.length) || range.length < 1 || range.length > 512 * 1024 * 1024) throw new Error("Invalid reply lease")
      this.#bytes.setUint32(at + rangeOffsets[index]!, range.pointer, true)
      this.#bytes.setUint32(at + rangeOffsets[index]! + 4, range.length, true)
    }
    this.#bytes.setUint32(at + 32, reply.slot ?? 0, true)
    const attack = reply.replayAttack
    this.#bytes.setUint32(at + 36, reply.kind === "visibility" ? Number(acoustic) : attack ? 1 : 0, true)
    if (reply.kind !== "visibility") this.#bytes.setUint32(at + 40, attack ? attack.playerClass | attack.weapon << 8 | attack.lifecycle << 16 : 0, true)
    this.#bytes.setBigUint64(at + 48, attack?.hostTick ?? 0n, true)
    timingKeys.forEach((key, index) => this.#bytes.setFloat64(at + 56 + index * 8, reply.timings[key], true))
    const hasMemory = reply.timings.wasmLinearMemoryBytes !== undefined
    if (hasMemory ? memoryKeys.some(key => !Number.isSafeInteger(reply.timings[key]) || reply.timings[key]! < 0)
      : reply.timings.wasmAllocatorLiveBytes !== undefined || reply.timings.wasmAllocatorHighWaterBytes !== undefined) throw new Error("Invalid reply memory gauges")
    this.#bytes.setUint32(at + 120, hasMemory ? 1 : 0, true)
    if (hasMemory) memoryKeys.forEach((key, index) => this.#bytes.setFloat64(at + 96 + index * 8, reply.timings[key]!, true))
    this.#releases.set(sequence, release)
    this.#publish(sequence)
  }
}

export class ReplyReader {
  readonly #control: Int32Array
  readonly #bytes: DataView
  readonly #native = new Map<number, WorkerResponse>()
  #read = 0
  #closed = false
  #controlReady?: () => void
  constructor(readonly shared: ReplyMemory, readonly receive: (response: WorkerResponse) => void) {
    validate(shared.mailbox)
    if (typeof Atomics.waitAsync !== "function") throw new Error("Asynchronous atomic replies unavailable")
    if (!(shared.memory instanceof WebAssembly.Memory) || !(shared.memory.buffer instanceof SharedArrayBuffer)
      || !(shared.modelOwnership instanceof SharedArrayBuffer) || shared.modelOwnership.byteLength !== REPLY_CAPACITY * 4) throw new Error("Invalid reply memory")
    this.#control = new Int32Array(shared.mailbox, 0, 4)
    this.#bytes = new DataView(shared.mailbox)
  }
  accept(message: ReplyControl): void {
    if (this.#closed) return
    const distance = (message.sequence - this.#read) >>> 0
    if (!Number.isSafeInteger(message.sequence) || message.sequence < 0 || message.sequence > 0xffff_ffff
      || distance < 1 || distance > REPLY_CAPACITY || this.#native.has(message.sequence)) throw new Error("Invalid reply control sequence")
    this.#native.set(message.sequence, message.response)
    this.#controlReady?.()
  }
  close(): void {
    if (this.#closed) return
    this.#closed = true
    this.#native.clear()
    Atomics.store(this.#control, 2, 1)
    // Change the expected word as well as notifying: closes the load/wait race.
    Atomics.add(this.#control, 0, 1)
    Atomics.notify(this.#control, 0)
    this.#controlReady?.()
  }
  async run(): Promise<void> {
    while (!this.#closed) {
      const written = Atomics.load(this.#control, 0) >>> 0
      if (((written - this.#read) >>> 0) > REPLY_CAPACITY) throw new Error("Invalid reply publication sequence")
      if (written === this.#read) {
        const wait = Atomics.waitAsync(this.#control, 0, written | 0)
        if (wait.async) await wait.value
        continue
      }
      const sequence = (this.#read + 1) >>> 0
      const at = HEADER + (this.#read % REPLY_CAPACITY) * STRIDE
      let response: WorkerResponse
      if (this.#bytes.getUint32(at, true) === 0) {
        if (!this.#native.has(sequence)) await new Promise<void>(resolve => { this.#controlReady = resolve })
        this.#controlReady = undefined
        if (this.#closed) return
        const value = this.#native.get(sequence)
        if (!value || value.id !== this.#bytes.getUint32(at + 4, true)) throw new Error("Missing reply control")
        response = value
        this.#native.delete(sequence)
      } else response = this.#decode(at)
      // Copy non-model payloads before releasing the ring. Model ranges have
      // their own atomic read lease, released by the synchronous pose decoder.
      this.#read = sequence
      Atomics.store(this.#control, 1, sequence | 0)
      this.receive(response)
      // Let synchronous decoder continuations run between FIFO deliveries.
      // This is bounded draining of published replies, never periodic polling.
      await Promise.resolve()
    }
  }
  #decode(at: number): WorkerResponse {
    const tag = this.#bytes.getUint32(at, true)
    const kind = tags[tag - 1]
    const id = this.#bytes.getUint32(at + 4, true), generation = this.#bytes.getUint32(at + 8, true)
    const count = this.#bytes.getUint32(at + 12, true)
    const acousticFlag = kind === "visibility" ? this.#bytes.getUint32(at + 36, true) : 0
    if (!kind || id === 0 || acousticFlag > 1 || count < 1 + acousticFlag || count > (kind === "visibility" ? 2 + acousticFlag : 2)
      || (kind !== "visibility" && kind !== "particles" && count !== 1)) throw new Error("Invalid shared reply")
    const memoryFlag = this.#bytes.getUint32(at + 120, true)
    if (memoryFlag > 1) throw new Error("Invalid reply memory flag")
    const timings: { -readonly [K in keyof WorkerTransactionTimings]: WorkerTransactionTimings[K] } & { mainCopyMilliseconds: number } = {
      queueMilliseconds: this.#bytes.getFloat64(at + 56, true), inputCopyMilliseconds: this.#bytes.getFloat64(at + 64, true),
      transactMilliseconds: this.#bytes.getFloat64(at + 72, true), outputCopyMilliseconds: this.#bytes.getFloat64(at + 80, true),
      totalMilliseconds: this.#bytes.getFloat64(at + 88, true), mainCopyMilliseconds: 0,
    }
    if (memoryFlag) memoryKeys.forEach((key, index) => {
      const value = this.#bytes.getFloat64(at + 96 + index * 8, true)
      if (!Number.isSafeInteger(value) || value < 0) throw new Error("Invalid reply memory gauges")
      timings[key] = value
    })
    if (Object.values(timings).some(value => !Number.isFinite(value) || value < 0)) throw new Error("Invalid reply timings")
    // A shared Memory object, not a cached SAB, observes growth by its Worker.
    const memory = this.shared.memory.buffer
    const ranges = Array.from({ length: count }, (_, index) => {
      const pointer = this.#bytes.getUint32(at + rangeOffsets[index]!, true), length = this.#bytes.getUint32(at + rangeOffsets[index]! + 4, true)
      if (pointer === 0 || length === 0 || length > 512 * 1024 * 1024 || pointer > memory.byteLength - length) throw new Error("Reply lease outside memory")
      return { pointer, length }
    })
    const first = ranges[0]!
    if (kind === "models") return { id, kind, generation, timings, output: memory as SharedArrayBuffer,
      byteOffset: first.pointer, byteLength: first.length, lease: id,
      ownership: this.shared.modelOwnership, slot: this.#bytes.getUint32(at + 32, true) }
    const copyStarted = performance.now()
    const outputs = ranges.map(range => new Uint8Array(memory, range.pointer, range.length).slice().buffer)
    timings.mainCopyMilliseconds = performance.now() - copyStarted
    if (kind === "visibility") return { id, kind, generation, timings, outputs: acousticFlag ? outputs.slice(0, -1) : outputs, ...(acousticFlag ? { acoustic: outputs.at(-1)! } : {}) }
    if (kind === "acoustics") return { id, kind, generation, timings, output: outputs[0]! }
    if (kind === "particles") return { id, kind, generation, timings, output: outputs[0]!, ...(outputs[1] ? { visualOutput: outputs[1] } : {}) }
    const player = this.#bytes.getUint32(at + 40, true)
    const replayAttack = this.#bytes.getUint32(at + 36, true) ? { hostTick: this.#bytes.getBigUint64(at + 48, true),
      playerClass: player & 255, weapon: player >>> 8 & 255, lifecycle: player >>> 16 & 255 } : undefined
    return { id, kind, generation, timings, output: outputs[0]!, ...(replayAttack ? { replayAttack } : {}) }
  }
}
