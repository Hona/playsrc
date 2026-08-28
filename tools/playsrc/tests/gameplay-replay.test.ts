import { expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import path from "node:path"
import { bindReplayGeneration, parseGameplayReplay, startGameplayReplayJournal, startGameplayReplayLifecycle, validateReplayLifecycle } from "../profile/gameplay-replay"
import { drainTraceStream } from "../profile/compositor-evidence"
import { summarizeActivePresentationSilence } from "../profile/compositor-truth"
import { parseReplayArguments, replayMutation, verifyReplayCheckpoint, verifyReplayHash } from "../profile/replay-gameplay"

test("offline replay accepts only explicit map/root selections and unambiguous options", () => {
  const identity = "a".repeat(64), resourceRoot = "b".repeat(64)
  expect(parseReplayArguments([identity])).toEqual({ identity, baseline: undefined, resourceRoot: undefined,
    target: "pl_upward", ticksOnly: false, displacement: false })
  expect(parseReplayArguments([identity, "--target=ctf_2fort", `--resource-root=${resourceRoot}`, "--baseline-wasm=before.wasm", "--ticks", "--displacement"]))
    .toEqual({ identity, baseline: "before.wasm", resourceRoot, target: "ctf_2fort", ticksOnly: true, displacement: true })
  for (const options of [["--target=other"], ["--resource-root=bad"], ["--baseline-wasm="],
    ["--target=pl_upward", "--target=ctf_2fort"], ["--ticks", "--ticks"],
    [`--resource-root=${resourceRoot}`, `--resource-root=${identity}`]]) {
    expect(() => parseReplayArguments([identity, ...options])).toThrow()
  }
})

test("historical replay stays strict and explicit two-build comparisons report historical mismatches", () => {
  expect(verifyReplayHash("a", "a", "tick:1")).toBe(false)
  expect(() => verifyReplayHash("b", "a", "tick:1")).toThrow("diverged")
  const hashes = new Map<string, string>()
  expect(verifyReplayHash("b", "a", "tick:1", hashes, true)).toBe(true)
  expect(verifyReplayHash("b", "a", "tick:1", hashes)).toBe(true)
  expect(() => verifyReplayHash("a", "a", "tick:1", hashes)).toThrow("diverged")
  expect(() => verifyReplayHash("b", "a", "tick:2", hashes)).toThrow("diverged")
})

test("replay checkpoint cannot substitute another generation or installed BSP", () => {
  const checkpoint = { configurationSha256: "a".repeat(64), configurationBytes: 4096, profile: 0, generation: 2 }
  const installed = { mapGeneration: 2, bsp: "b".repeat(64) }
  verifyReplayCheckpoint(checkpoint, installed, installed.bsp)
  expect(() => verifyReplayCheckpoint(checkpoint, { ...installed, mapGeneration: 1 }, installed.bsp)).toThrow("generation")
  expect(() => verifyReplayCheckpoint(checkpoint, installed, "c".repeat(64))).toThrow("BSP")
  for (const changed of [{ generation: 0 }, { generation: 1.5 }, { configurationBytes: -1 }, { configurationSha256: "bad" }, { profile: 2 }]) {
    expect(() => verifyReplayCheckpoint({ ...checkpoint, ...changed }, installed, installed.bsp)).toThrow("checkpoint")
  }
})

test("a missing final native presentation cannot hide boundary silence", () => {
  const value = summarizeActivePresentationSilence([
    { name: "PresentationFeedback", ts: 0, pid: 1, tid: 1 },
    { name: "PresentationFeedback", ts: 10_000, pid: 1, tid: 1 },
  ], { startedMicroseconds: 0, endedMicroseconds: 400_000 })
  expect(value.maximumActiveSilenceMilliseconds).toBe(10)
  expect(value.maximumCensoredBoundaryMilliseconds).toBe(390)
  expect(value.censoredBoundaries).toEqual([{ stream: "1:1", side: "end", milliseconds: 390 }])
})

function record(kind: number, bytes: Buffer) {
  const header = Buffer.alloc(8)
  header.writeUInt32LE(bytes.length + 8); header.writeUInt32LE(kind, 4)
  return Buffer.concat([header, bytes])
}
function fixture() {
  const header = Buffer.alloc(88); header.write("PGRP"); header.writeUInt32LE(2, 4); header.writeBigUInt64LE(1n, 80)
  const observe = Buffer.alloc(108); observe.writeDoubleLE(2); observe.writeUInt32LE(84, 20)
  const tick = Buffer.alloc(136); tick.writeBigUInt64LE(1n); tick.writeUInt32LE(84, 48)
  const one = Buffer.from([1, 0, 0, 0])
  return Buffer.concat([header, record(7, Buffer.alloc(4)), record(1, observe), record(2, tick), record(3, Buffer.alloc(32)), record(7, one), record(8, one)])
}

test("v3 mutations have disjoint wire operations; historical ambiguous records fail closed", () => {
  const old = fixture(), header = Buffer.alloc(780)
  old.copy(header, 0, 0, 88); header.writeUInt32LE(3, 4); header.write("TFEQ\x01\0\0\0", 88, "latin1")
  const position = Buffer.alloc(12); position.writeFloatLE(-12.5); position.writeFloatLE(3, 4); position.writeFloatLE(96, 8)
  const course = Buffer.alloc(52); course.write("PJMP"); course.writeUInt32LE(1, 4)
  const entity = Buffer.concat([Buffer.alloc(4), Buffer.from("door\0Open\0")])
  const equip = Buffer.from([1, 3, 0, 18, 0, 0, 0]), botEquip = Buffer.from([2, 2, 0, 0, 0, 255, 255, 255, 255])
  const restore = Buffer.concat([Buffer.from([0]), header.subarray(88)])
  const mutations = [[4, Buffer.from([2, 0, 0, 0])], [5, position], [6, course], [9, equip], [9, botEquip], [9, restore], [10, entity]] as const
  const parsed = parseGameplayReplay(Buffer.concat([header, ...mutations.map(([kind, bytes]) => record(kind, bytes)), old.subarray(88)]))
  expect(parsed.version).toBe(3); expect(parsed.initialEquipment?.length).toBe(692)
  expect(parsed.records.slice(0, mutations.length).map(r => r.kind)).toEqual([4, 5, 6, 9, 9, 9, 10])
  for (const [kind, data] of [[5, position], [5, botEquip], [7, entity]] as const) {
    expect(() => parseGameplayReplay(Buffer.concat([old.subarray(0, 88), record(kind, data), old.subarray(88)]))).toThrow("historical")
  }
  const calls: unknown[] = [], memory = new WebAssembly.Memory({ initial: 1, maximum: 3 })
  const exports: Record<string, any> = { memory, playsrc_alloc(size: number) { memory.grow(1); return 65536 }, playsrc_free(pointer: number, size: number) { calls.push(["free", pointer, size]) },
    playsrc_team_select: (...args: unknown[]) => { calls.push(["team", ...args]); return 1 },
    playsrc_player_set_position: (...args: unknown[]) => { calls.push(["position", ...args]); return 1 } }
  for (const [name, kind, bytes] of [["playsrc_jump_configure", 6, course], ["playsrc_equipment_update", 9, botEquip], ["playsrc_entity_fire", 10, entity]] as const) {
    exports.playsrc_alloc = () => 65536
    if (memory.buffer.byteLength === 65536) memory.grow(1)
    exports[name] = (handle: number, pointer: number, length: number) => {
      expect(handle).toBe(0x20001); expect(Buffer.from(memory.buffer, pointer, length).equals(bytes)).toBe(true); return 1
    }
    replayMutation(exports, 0x20001, { kind, bytes })
  }
  replayMutation(exports, 0x20001, { kind: 4, bytes: mutations[0][1] })
  replayMutation(exports, 0x20001, { kind: 5, bytes: position })
  expect(calls.slice(-2)).toEqual([["team", 0x20001, 2], ["position", 0x20001, -12.5, 3, 96]])
  exports.playsrc_entity_fire = () => 0
  expect(() => replayMutation(exports, 0x20001, { kind: 10, bytes: entity })).toThrow("mutation 10 failed")
  expect(calls.at(-1)).toEqual(["free", 65536, entity.length])
  expect(() => replayMutation(exports, 1, { kind: 7, bytes: Buffer.alloc(4) })).toThrow("Invalid gameplay mutation")
  position.writeFloatLE(NaN)
  expect(() => replayMutation(exports, 1, { kind: 5, bytes: position })).toThrow("Invalid gameplay mutation")
})
test("authoritative replay retains full merged commands and rejects incomplete/order-corrupt evidence", () => {
  const bytes = fixture()
  expect(parseGameplayReplay(bytes).complete).toBe(true)
  expect(parseGameplayReplay(bytes).records.filter(record => record.kind === 2)[0]!.bytes.subarray(52).length).toBe(84)
  expect(() => parseGameplayReplay(bytes.subarray(0, -1))).toThrow("Partial")
  expect(parseGameplayReplay(bytes.subarray(0, -1), false).complete).toBe(false)
  const failed = Buffer.from(bytes); failed.writeUInt32LE(0, failed.length - 4)
  expect(() => parseGameplayReplay(failed)).toThrow("incomplete")
  const badHeader = Buffer.from(bytes); badHeader.writeBigUInt64LE(1n, 72)
  expect(() => parseGameplayReplay(badHeader)).toThrow("checkpoint")
  const badOrder = Buffer.concat([bytes.subarray(0, 88), record(2, Buffer.alloc(136))])
  expect(() => parseGameplayReplay(badOrder)).toThrow("tick order")
})
test("streaming trace persistence preserves partial bytes when CDP draining fails", async () => {
  const persisted: Buffer[] = []
  let reads = 0
  const cdp = { async send(method: string) {
    if (method === "IO.close") return {}
    if (reads++) throw new Error("lost browser")
    return { data: Buffer.from("partial native trace").toString("base64"), base64Encoded: true, eof: false }
  } }
  const result = await drainTraceStream(cdp as any, "native", 1024, async bytes => { persisted.push(bytes) })
  expect(result.complete).toBe(false)
  expect(Buffer.concat(persisted).equals(result.bytes)).toBe(true)
  expect(result.bytes.toString()).toBe("partial native trace")
})
test("replay capture is bounded, incremental, durable and stop is idempotent", async () => {
  const directory = await mkdtemp(path.join(process.cwd(), ".replay-test-"))
  let attach: ((worker: any) => void) | undefined
  const page = { on(_event: string, fn: typeof attach) { attach = fn }, off() {} }
  const bytes = fixture()
  const checkpoint = { configurationSha256: "a".repeat(64), configurationBytes: 12, profile: 1, generation: 1 }
  let readCount = 0
  try {
    const journal = await startGameplayReplayJournal(page as any, directory, "test")
    attach!({ url: () => "http://local/gameplay-worker.ts", async evaluate(_fn: unknown, args: any) {
      if (!args || args.offset === undefined) return
      readCount++
      return { checkpoint, mapOrdinal: 1, offset: args.offset, length: bytes.length, complete: args.stop, base64: bytes.subarray(args.offset).toString("base64") }
    } })
    const [first, second] = await Promise.all([journal.stop(), journal.stop()])
    expect(first).toEqual(second)
    expect(readCount).toBe(1)
    expect(first.complete).toBe(true)
    expect((await readFile(path.join(directory, first.file))).equals(bytes)).toBe(true)
    expect(JSON.parse(await readFile(path.join(directory, "test.replay-progress.json"), "utf8")).complete).toBe(true)
  } finally { await rm(directory, { recursive: true, force: true }) }
})

test("the owner journal waits for Ready and serializes boundary marks before final drain", async () => {
  const directory = await mkdtemp(path.join(process.cwd(), ".replay-test-"))
  let attach: ((worker: any) => void) | undefined
  const page = { on(_event: string, fn: typeof attach) { attach = fn }, off() {} }
  const bytes = fixture(), calls: string[] = []
  let release: (() => void) | undefined
  try {
    const journal = await startGameplayReplayJournal(page as any, directory, "ordered", 2)
    attach!({ url: () => "http://local/gameplay-worker.ts", async evaluate(_fn: unknown, args: any) {
      if (args?.mapOrdinal) { expect(args.mapOrdinal).toBe(2); return }
      if (typeof args === "number") { calls.push("mark"); await new Promise<void>(resolve => { release = resolve }); return }
      if (!args) return
      calls.push(args.stop ? "stop" : "read")
      return { checkpoint: { configurationSha256: "a".repeat(64), configurationBytes: 12, profile: 1, generation: 5 }, mapOrdinal: 2, offset: args.offset, length: bytes.length, complete: args.stop, base64: bytes.subarray(args.offset).toString("base64") }
    } })
    expect(calls).toEqual([])
    await journal.ready()
    const marked = journal.mark(0)
    await Promise.resolve()
    const stopped = journal.stop()
    expect(calls).toEqual(["read", "mark"])
    release!()
    await marked
    expect((await stopped).complete).toBe(true)
    expect(calls).toEqual(["read", "mark", "stop"])
  } finally { await rm(directory, { recursive: true, force: true }) }
})

test("failed incremental reads can retain a final recovered prefix but never pass", async () => {
  const directory = await mkdtemp(path.join(process.cwd(), ".replay-test-"))
  let attach: ((worker: any) => void) | undefined
  const page = { on(_event: string, fn: typeof attach) { attach = fn }, off() {} }
  const bytes = fixture()
  try {
    const journal = await startGameplayReplayJournal(page as any, directory, "failed")
    attach!({ url: () => "http://local/gameplay-worker.ts", async evaluate(_fn: unknown, args: any) {
      if (!args || args.mapOrdinal) return
      if (!args.stop) throw new Error("transport failed")
      return { checkpoint: { configurationSha256: "a".repeat(64), configurationBytes: 12, profile: 1, generation: 1 }, mapOrdinal: 1, offset: 0, length: bytes.length, complete: true, base64: bytes.toString("base64") }
    } })
    await expect(journal.ready()).rejects.toThrow("transport failed")
    const retained = await journal.stop()
    expect(retained.complete).toBe(false)
    expect((await readFile(path.join(directory, retained.file))).equals(bytes)).toBe(true)
  } finally { await rm(directory, { recursive: true, force: true }) }
})

test("requested warm reload authenticates two distinct Worker journals and preserves navigation order", async () => {
  const directory = await mkdtemp(path.join(process.cwd(), ".replay-test-"))
  const listeners = new Set<(worker: any) => void>()
  const page = { on(_event: string, fn: (worker: any) => void) { listeners.add(fn) }, off(_event: string, fn: (worker: any) => void) { listeners.delete(fn) } }
  const full = fixture()
  const setup = Buffer.concat([full.subarray(0, 88), ...parseGameplayReplay(full).records.filter(record => record.kind !== 7).map(value => record(value.kind, value.bytes))])
  const identity = { target: "pl_upward", resourceRoot: "a".repeat(64), bsp: "0".repeat(64), mapGeneration: 1, contentBuild: "24245096" }
  function worker(bytes: Buffer) {
    let closed: (() => void) | undefined
    return { url: () => "http://local/gameplay-worker.ts", on(_event: string, fn: () => void) { closed = fn }, close() { closed!() },
      async evaluate(_fn: unknown, args: any) {
        if (!args || args.mapOrdinal || typeof args === "number") return
        return { checkpoint: { configurationSha256: "b".repeat(64), configurationBytes: 12, profile: 1, generation: 1 },
          mapOrdinal: 1, offset: args.offset, length: bytes.length, complete: args.stop, base64: bytes.subarray(args.offset).toString("base64"),
          admission: { schema: 1, timeOrigin: 123, dropped: 0, events: [] } }
      } }
  }
  try {
    const capture = await startGameplayReplayLifecycle(page as any, directory, "warm", true)
    const cold = worker(setup); for (const fn of listeners) fn(cold)
    await expect(capture.ready()).rejects.toThrow("navigation missing")
    await capture.beforeReload(identity)
    cold.close()
    const warm = worker(full); for (const fn of listeners) fn(warm)
    await capture.ready(); await capture.mark(0); await capture.mark(1)
    const result = await capture.stop(identity)
    expect(await capture.stop(identity)).toBe(result)
    const manifest = JSON.parse(await readFile(path.join(directory, result.lifecycle.file), "utf8"))
    validateReplayLifecycle(manifest)
    expect(manifest.generations.map((entry: any) => [entry.workerOrdinal, entry.journal.expectedMarks, entry.scope])).toEqual([
      [1, 0, "checkpoint-to-pre-navigation"], [2, 2, "checkpoint-through-sample"],
    ])
    expect(manifest.generations[0].journal.sha256).not.toBe(manifest.generations[1].journal.sha256)
    expect(manifest.generations[0].journal.admission.file).toMatch(/^[0-9a-f]{64}\.admission\.json$/)
    expect(manifest.generations.every((entry: any) => entry.applicationGeneration.resourceRoot === identity.resourceRoot)).toBe(true)
    const broken = structuredClone(manifest); broken.generations.reverse()
    expect(() => validateReplayLifecycle(broken)).toThrow("generation order")
    broken.generations.reverse(); broken.generations[1].transition.previousClosedAt = 0
    expect(() => validateReplayLifecycle(broken)).toThrow("generation order")
    const wrongRoot = { ...identity, resourceRoot: "not-a-root" }
    expect(() => bindReplayGeneration(manifest.generations[0].journal, wrongRoot, identity.bsp)).toThrow("identity mismatch")
    expect(() => bindReplayGeneration(manifest.generations[0].journal, { ...identity, mapGeneration: 2 }, identity.bsp)).toThrow("identity mismatch")
    expect(() => parseGameplayReplay(setup)).toThrow("incomplete")
    expect(parseGameplayReplay(setup, true, 0).complete).toBe(true)
    const unclosed = await startGameplayReplayLifecycle(page as any, directory, "unclosed", true)
    const predecessor = worker(setup); for (const fn of listeners) fn(predecessor)
    await unclosed.beforeReload(identity)
    for (const fn of listeners) fn(worker(full))
    await expect(unclosed.ready()).rejects.toThrow("did not close")
    const rejected = await unclosed.stop(identity)
    expect(rejected.lifecycle.complete).toBe(false)
    const incomplete = JSON.parse(await readFile(path.join(directory, rejected.lifecycle.file), "utf8"))
    expect(incomplete.generations).toHaveLength(2)
    expect(() => validateReplayLifecycle(incomplete)).toThrow("Incomplete replay lifecycle")
  } finally { await rm(directory, { recursive: true, force: true }) }
})

test("unexpected replacement remains a failure and keeps the recoverable journal", async () => {
  const directory = await mkdtemp(path.join(process.cwd(), ".replay-test-"))
  let attach: ((worker: any) => void) | undefined
  const page = { on(_event: string, fn: typeof attach) { attach = fn }, off() {} }
  const bytes = fixture()
  const worker = () => ({ url: () => "http://local/gameplay-worker.ts", async evaluate(_fn: unknown, args: any) {
    if (!args || args.mapOrdinal) return
    return { checkpoint: { configurationSha256: "a".repeat(64), configurationBytes: 12, profile: 1, generation: 1 }, mapOrdinal: 1,
      offset: args.offset, length: bytes.length, complete: args.stop, base64: bytes.subarray(args.offset).toString("base64") }
  } })
  try {
    const journal = await startGameplayReplayJournal(page as any, directory, "unexpected")
    attach!(worker()); attach!(worker())
    await expect(journal.ready()).rejects.toThrow("owner changed")
    const result = await journal.stop(false)
    expect(result.complete).toBe(false); expect(result.error).toContain("owner changed")
    expect((await readFile(path.join(directory, result.file))).equals(bytes)).toBe(true)
  } finally { await rm(directory, { recursive: true, force: true }) }
})
