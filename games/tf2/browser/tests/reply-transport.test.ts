import { expect, test } from "bun:test"
import { ReplyReader, ReplyWriter, REPLY_BYTES, REPLY_CAPACITY, type ReplyControl, type SharedReply } from "../src/reply-transport"
import type { WorkerResponse } from "../src/protocol"

const timings = { queueMilliseconds: 0.1, inputCopyMilliseconds: 0.2, transactMilliseconds: 0.3, outputCopyMilliseconds: 0.4, totalMilliseconds: 1 }
function fixture() {
  const shared = { mailbox: new SharedArrayBuffer(REPLY_BYTES), memory: new WebAssembly.Memory({ initial: 1, maximum: 4, shared: true }), modelOwnership: new SharedArrayBuffer(64 * 4) }
  const received: WorkerResponse[] = [], released: number[] = []
  let resolve: (response: WorkerResponse) => void
  let next = new Promise<WorkerResponse>(r => { resolve = r })
  const reader = new ReplyReader(shared, response => { received.push(response); resolve(response) })
  const writer = new ReplyWriter(shared.mailbox)
  const reply = (id: number, kind: SharedReply["kind"] = "particles", pointer = 16) => {
    new Uint8Array(shared.memory.buffer, pointer, 4).set([id & 255, 2, 3, 4])
    return { id, kind, generation: 9, ranges: [{ pointer, length: 4 }], timings }
  }
  return { shared, reader, writer, received, released, reply, next: () => {
    const waiting = next
    return waiting.then(response => { next = new Promise(r => { resolve = r }); return response })
  } }
}

test("a notification before or after wait delivers ordered immutable copies and releases once", async () => {
  const f = fixture()
  f.writer.shared(f.reply(1), () => f.released.push(1))
  const run = f.reader.run()
  await f.next()
  f.writer.reclaim()
  expect(f.released).toEqual([1])
  f.writer.shared(f.reply(2), () => f.released.push(2))
  await f.next()
  f.writer.reclaim()
  const first = f.received[0] as Extract<WorkerResponse, { kind: "particles" }>
  expect([...new Uint8Array(first.output)]).toEqual([1, 2, 3, 4])
  expect(f.received.map(r => r.id)).toEqual([1, 2])
  expect(f.released).toEqual([1, 2])
  f.writer.reclaim()
  expect(f.released).toEqual([1, 2])
  f.reader.close(); await run
})

test("structured clone controls and failures are FIFO barriers, never overtaken by shared replies", async () => {
  const f = fixture()
  let control!: ReplyControl
  f.writer.control({ id: 1, kind: "failure", code: "TransitionFailed", detail: 813, reason: "original failure" }, value => { control = value })
  f.writer.shared(f.reply(2), () => f.released.push(2))
  const run = f.reader.run()
  await Promise.resolve()
  expect(f.received).toHaveLength(0)
  f.reader.accept(control)
  await f.next(); await f.next()
  expect(f.received.map(r => r.id)).toEqual([1, 2])
  expect(f.received[0]).toEqual(control.response)
  f.writer.reclaim(); expect(f.released).toEqual([2])
  f.reader.close(); await run
})

test("main/sky leases publish together and shared memory growth is read through the Memory owner", async () => {
  const f = fixture()
  const old = f.shared.memory.buffer
  f.shared.memory.grow(1)
  expect(old.byteLength).toBe(65536)
  const reply = f.reply(4, "visibility", 65536 + 16)
  new Uint8Array(f.shared.memory.buffer, 65536 + 32, 3).set([5, 6, 7])
  f.writer.shared({ ...reply, ranges: [...reply.ranges, { pointer: 65536 + 32, length: 3 }] }, () => f.released.push(4))
  const run = f.reader.run()
  const response = await f.next() as Extract<WorkerResponse, { kind: "visibility" }>
  expect(response.generation).toBe(9)
  expect(response.outputs.map(output => [...new Uint8Array(output)])).toEqual([[4, 2, 3, 4], [5, 6, 7]])
  f.writer.reclaim(); expect(f.released).toEqual([4])
  f.reader.close(); await run
})

test("model decoder ownership survives ring acknowledgement and replay tick stays exact", async () => {
  const f = fixture()
  const owner = new Int32Array(f.shared.modelOwnership)
  Atomics.store(owner, 3, 0xffff_fffe | 0)
  f.writer.shared({ ...f.reply(0xffff_fffe, "models"), slot: 3 }, () => f.released.push(1))
  const run = f.reader.run()
  const model = await f.next() as Extract<WorkerResponse, { kind: "models" }>
  f.writer.reclaim()
  expect(model.ownership).toBe(f.shared.modelOwnership)
  expect(Atomics.load(owner, model.slot) >>> 0).toBe(model.lease)
  expect(model.output).toBe(f.shared.memory.buffer)
  Atomics.store(owner, 3, 0)
  const replayAttack = { hostTick: 0xffff_ffff_ffff_fffen, playerClass: 3, weapon: 9, lifecycle: 2 }
  f.writer.shared({ ...f.reply(8, "simulation"), replayAttack }, () => {})
  const simulation = await f.next() as Extract<WorkerResponse, { kind: "simulation" }>
  expect(simulation.replayAttack).toEqual(replayAttack)
  f.reader.close(); await run
})

test("main, sky and acoustic geometry share one immutable publication without overwriting metadata", async () => {
  const f = fixture()
  new Uint8Array(f.shared.memory.buffer, 64, 3).set([5, 6, 7])
  new Uint8Array(f.shared.memory.buffer, 80, 4).set([0x50, 0x53, 0x41, 0x52])
  f.writer.shared({ ...f.reply(1, "visibility"), acoustic: true,
    ranges: [{ pointer: 16, length: 4 }, { pointer: 64, length: 3 }, { pointer: 80, length: 4 }] }, () => f.released.push(1))
  const run = f.reader.run()
  const reply = await f.next() as Extract<WorkerResponse, { kind: "visibility" }>
  expect(reply.outputs.map(bytes => [...new Uint8Array(bytes)])).toEqual([[1, 2, 3, 4], [5, 6, 7]])
  expect([...new Uint8Array(reply.acoustic!)]).toEqual([0x50, 0x53, 0x41, 0x52])
  expect(reply.timings).toMatchObject(timings)
  f.writer.reclaim(); expect(f.released).toEqual([1])
  f.writer.shared(f.reply(2, "acoustics", 80), () => f.released.push(2))
  const audio = await f.next() as Extract<WorkerResponse, { kind: "acoustics" }>
  expect([...new Uint8Array(audio.output)]).toEqual([2, 2, 3, 4])
  expect([...new Uint8Array(reply.acoustic!)]).toEqual([0x50, 0x53, 0x41, 0x52])
  f.writer.reclaim(); expect(f.released).toEqual([1, 2])
  f.reader.close(); await run
})

test("legacy particle and visual ranges publish and release atomically with exact byte gauges", async () => {
  const f = fixture()
  const reply = f.reply(1)
  new Uint8Array(f.shared.memory.buffer, 32, 4).set([0x50, 0x4c, 0x56, 0x46])
  const measured = { ...timings, wasmLinearMemoryBytes: 3_000_000_000, wasmAllocatorLiveBytes: 2_200_012_345, wasmAllocatorHighWaterBytes: 2_500_023_456 }
  f.writer.shared({ ...reply, ranges: [...reply.ranges, { pointer: 32, length: 4 }], timings: measured }, () => f.released.push(1))
  const run = f.reader.run()
  const response = await f.next() as Extract<WorkerResponse, { kind: "particles" }>
  expect([...new Uint8Array(response.output)]).toEqual([1, 2, 3, 4])
  expect([...new Uint8Array(response.visualOutput!)]).toEqual([0x50, 0x4c, 0x56, 0x46])
  expect(response.timings.wasmAllocatorLiveBytes).toBe(2_200_012_345)
  f.writer.reclaim(); expect(f.released).toEqual([1])
  expect(() => f.writer.shared({ ...f.reply(2), timings: { ...measured, wasmAllocatorLiveBytes: 0.5 } }, () => {})).toThrow("memory gauges")
  f.reader.close(); await run
})

test("full ring rejects rather than overwriting a pending lease", () => {
  const f = fixture()
  for (let id = 1; id <= REPLY_CAPACITY; id++) f.writer.shared(f.reply(id), () => {})
  expect(() => f.writer.shared(f.reply(65), () => {})).toThrow("bound exceeded")
  f.reader.close()
  expect(() => f.writer.shared(f.reply(66), () => {})).toThrow("closed")
})

test("reuses every ring slot across successive notifications without leaking retired releases", async () => {
  const f = fixture()
  const run = f.reader.run()
  for (let id = 1; id <= REPLY_CAPACITY * 3; id++) {
    f.writer.shared(f.reply(id), () => f.released.push(id))
    const response = await f.next()
    expect(response.id).toBe(id)
    f.writer.reclaim()
  }
  expect(f.released).toEqual(Array.from({ length: REPLY_CAPACITY * 3 }, (_, index) => index + 1))
  f.reader.close(); await run
})

test("abort wakes both an empty waiter and a pending control without timers or polling", async () => {
  for (const control of [false, true]) {
    const f = fixture()
    if (control) f.writer.control({ id: 1, kind: "shutdown" }, () => {})
    const run = f.reader.run()
    f.reader.close(); f.reader.close()
    await run
    expect(f.received).toHaveLength(0)
  }
})

test("malformed metadata fails closed before acknowledging the Rust byte lease", async () => {
  const f = fixture()
  f.writer.shared(f.reply(1), () => f.released.push(1))
  new DataView(f.shared.mailbox).setUint32(16 + 16, 0xffff_ffff, true)
  await expect(f.reader.run()).rejects.toThrow("outside memory")
  f.writer.reclaim(); expect(f.released).toEqual([])
  f.reader.close()
})
