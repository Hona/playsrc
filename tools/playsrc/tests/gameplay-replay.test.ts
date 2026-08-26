import { expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import path from "node:path"
import { parseGameplayReplay, startGameplayReplayJournal } from "../profile/gameplay-replay"
import { drainTraceStream } from "../profile/compositor-evidence"
import { summarizeActivePresentationSilence } from "../profile/compositor-truth"

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
  const header = Buffer.alloc(88); header.write("PGRP"); header.writeUInt32LE(1, 4); header.writeBigUInt64LE(1n, 80)
  const observe = Buffer.alloc(100); observe.writeDoubleLE(2); observe.writeUInt32LE(84, 12)
  const tick = Buffer.alloc(136); tick.writeBigUInt64LE(1n); tick.writeUInt32LE(84, 48)
  const one = Buffer.from([1, 0, 0, 0])
  return Buffer.concat([header, record(7, Buffer.alloc(4)), record(1, observe), record(2, tick), record(3, Buffer.alloc(32)), record(7, one), record(8, one)])
}
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
      return { checkpoint, offset: args.offset, length: bytes.length, complete: args.stop, base64: bytes.subarray(args.offset).toString("base64") }
    } })
    const [first, second] = await Promise.all([journal.stop(), journal.stop()])
    expect(first).toEqual(second)
    expect(readCount).toBe(1)
    expect(first.complete).toBe(true)
    expect((await readFile(path.join(directory, first.file))).equals(bytes)).toBe(true)
    expect(JSON.parse(await readFile(path.join(directory, "test.replay-progress.json"), "utf8")).complete).toBe(true)
  } finally { await rm(directory, { recursive: true, force: true }) }
})
