import { createHash } from "node:crypto"
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Page, Worker } from "@playwright/test"

export const REPLAY_BYTES = 4 * 1024 * 1024
export type ReplayCheckpoint = { configurationSha256: string; configurationBytes: number; profile: number; generation: number }
export type ReplayRecord = { kind: number; bytes: Buffer }
export function validateReplayMutation(kind: number, data: Buffer) {
  let valid = false
  if (kind === 4) valid = data.length === 4 && data.readUInt32LE(0) >= 1 && data.readUInt32LE(0) <= 4
  if (kind === 5) valid = data.length === 12 && [0, 4, 8].every(at => Number.isFinite(data.readFloatLE(at)))
  if (kind === 6) valid = data.length >= 52 && data.length <= 65536 && data.toString("ascii", 0, 4) === "PJMP" && data.readUInt32LE(4) === 1
    && 52 + data.readUInt32LE(48) * 16 === data.length
  if (kind === 9) valid = (data.length === 693 && data[0] === 0 && data.subarray(1, 9).equals(Buffer.from("TFEQ\x01\0\0\0")))
    || (data.length === 7 && data[0] === 1 && data[1]! >= 1 && data[1]! <= 9 && data[2]! <= 18)
    || (data.length === 9 && data[0] === 2)
  if (kind === 10 && data.length >= 8 && data.length <= 3078 && Number.isFinite(data.readFloatLE(0))) {
    const fields = data.subarray(4).toString("latin1").split("\0")
    valid = fields.length === 3 && fields[0]!.length > 0 && fields[1]!.length > 0 && fields.every(field => field.length <= 1024)
  }
  if (!valid) throw new Error("Invalid gameplay mutation")
}
export function parseGameplayReplay(bytes: Buffer, requireComplete = true) {
  if (bytes.length < 88 || bytes.length > REPLAY_BYTES || bytes.toString("ascii", 0, 4) !== "PGRP" || ![2, 3].includes(bytes.readUInt32LE(4))
    || bytes.readBigUInt64LE(72) !== 0n || bytes.readBigUInt64LE(80) !== 1n) throw new Error("Replay initial checkpoint is invalid")
  const version = bytes.readUInt32LE(4), headerBytes = version === 3 ? 780 : 88
  if (bytes.length < headerBytes || (version === 3 && !bytes.subarray(88, 96).equals(Buffer.from("TFEQ\x01\0\0\0")))) throw new Error("Replay equipment checkpoint is invalid")
  const initialEquipment = version === 3 ? bytes.subarray(88, headerBytes) : undefined
  const records: ReplayRecord[] = []
  let at = headerBytes, observing = false, complete = false, tick = -1n, marks = 0
  while (at < bytes.length) {
    if (at + 8 > bytes.length) { if (!requireComplete) break; throw new Error("Partial replay record") }
    const length = bytes.readUInt32LE(at), kind = bytes.readUInt32LE(at + 4)
    if (length < 8 || length > 65596 || records.length > 16384) throw new Error("Replay record bound is invalid")
    if (at + length > bytes.length) { if (!requireComplete) break; throw new Error("Partial replay record") }
    const data = bytes.subarray(at + 8, at + length)
    // V2 assigned kind 5 to two mutations and kind 7 to both Entity input and
    // sample marks. Only four-byte marks are unambiguous (Entity input requires
    // at least eight bytes). Never infer a mutation from historical payload shape.
    if (version === 2 && (kind === 5 || (kind === 7 && data.length !== 4) || kind > 8)) throw new Error("Ambiguous or unsupported historical replay operation")
    if (kind === 1) {
      if (observing || data.length < 108 || data.readUInt32LE(20) + 24 !== data.length || !Number.isFinite(data.readDoubleLE(0)) || data.readUInt32LE(8) > 1) throw new Error("Invalid admitted observe command")
      observing = true
    } else if (kind === 2) {
      if (!observing || data.length < 136 || data.readUInt32LE(48) + 52 !== data.length || data.readBigUInt64LE(0) <= tick) throw new Error("Invalid authoritative tick order")
      tick = data.readBigUInt64LE(0)
    } else if (kind === 3) {
      if (!observing || data.length !== 32) throw new Error("Invalid observe publication")
      observing = false
    } else if ([4, 5, 6, 9, 10].includes(kind)) {
      if (observing) throw new Error("Invalid gameplay mutation during observe")
      validateReplayMutation(kind, data)
    } else if (kind === 7) {
      if (observing || data.length !== 4 || data.readUInt32LE(0) !== marks || marks > 1) throw new Error("Invalid replay sample boundary")
      marks++
    } else if (kind === 8) {
      if (data.length !== 4 || at + length !== bytes.length) throw new Error("Invalid replay footer")
      complete = !observing && data.readUInt32LE(0) === 1
    } else throw new Error("Unknown replay operation")
    records.push({ kind, bytes: data })
    at += length
  }
  if (requireComplete && (!complete || observing || marks !== 2)) throw new Error("Replay is incomplete")
  return { version, headerBytes, initialEquipment, bspSha256: bytes.subarray(8, 40).toString("hex"), worldSha256: bytes.subarray(40, 72).toString("hex"), records, complete, marks }
}

/** Durable incremental journal: owner-generated commands, never a heap/checkpoint dump. */
export async function startGameplayReplayJournal(page: Page, directory: string, label: string, mapOrdinal = 1) {
  if (!Number.isSafeInteger(mapOrdinal) || mapOrdinal < 1) throw new Error("Replay map ordinal rejected")
  await mkdir(directory, { recursive: true })
  const partial = path.join(directory, `${label}.replay.partial`)
  const progress = path.join(directory, `${label}.replay-progress.json`)
  await writeFile(partial, Buffer.alloc(0), { flag: "wx" })
  let worker: Worker | undefined, offset = 0, failure: string | null = null, checkpoint: ReplayCheckpoint | undefined
  let pending = Promise.resolve(), stopped = false
  const persistStatus = () => writeFile(progress, JSON.stringify({ schema: "playsrc-gameplay-replay-progress-v1", complete: false, bytes: offset, checkpoint, error: failure }))
  await persistStatus()
  const capture = async (stop = false) => {
    if (!worker || (failure && !stop)) return
    let timeout: ReturnType<typeof setTimeout> | undefined
    try {
      const result = await Promise.race([
        worker.evaluate(({ offset, stop }) => (globalThis as any).__playsrcGameplayReplay.read(offset, stop), { offset, stop }),
        new Promise<never>((_, reject) => { timeout = setTimeout(() => reject(new Error("Replay owner read exceeded 3 seconds")), 3000) }),
      ])
      if (!result) return
      if (typeof result.base64 !== "string" || result.base64.length > Math.ceil(REPLAY_BYTES / 3) * 4 || !Number.isSafeInteger(result.length) || result.length > REPLAY_BYTES) throw new Error("Replay stream byte bound exceeded")
      const bytes = Buffer.from(result.base64, "base64")
      if (result.offset !== offset || offset + bytes.length !== result.length || result.length > REPLAY_BYTES) throw new Error("Replay stream identity mismatch")
      if (checkpoint && JSON.stringify(checkpoint) !== JSON.stringify(result.checkpoint)) throw new Error("Replay checkpoint changed")
      if (result.mapOrdinal !== mapOrdinal) throw new Error("Replay captured a different map construction")
      checkpoint = result.checkpoint
      await appendFile(partial, bytes)
      offset += bytes.length
      if (stop && !result.complete) throw new Error("Authoritative replay owner reported incomplete evidence")
    } catch (error) { failure = String(error) }
    finally { if (timeout) clearTimeout(timeout); await persistStatus() }
  }
  const attach = (value: Worker) => {
    if (!value.url().includes("gameplay-worker")) return
    if (worker) { failure = "Replay gameplay owner changed"; return }
    worker = value
    pending = value.evaluate(async ({ mapOrdinal }) => {
      const deadline = performance.now() + 5000
      while (!(globalThis as any).__playsrcGameplayReplay && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10))
      ;(globalThis as any).__playsrcGameplayReplay.arm(mapOrdinal)
    }, { mapOrdinal }).catch(error => { failure = String(error) })
  }
  page.on("worker", attach)
  let busy = false
  let timer: ReturnType<typeof setInterval> | undefined
  const poll = () => {
    if (busy || stopped) return
    busy = true
    pending = pending.then(() => capture()).finally(() => { busy = false })
  }
  let result: Promise<any> | undefined
  return {
    async ready() {
      // Initial WASM compilation is synchronous and can legitimately outlast a
      // short diagnostic RPC. Keep the owner journal in Rust until Ready; do
      // not queue reads behind compilation and misclassify it as capture loss.
      await pending
      await capture()
      if (!checkpoint || failure) throw new Error(failure ?? "Replay initial checkpoint absent at Ready")
      timer = setInterval(poll, 1000)
    },
    mark(mark: number) {
      const marked = pending.then(async () => {
        if (!worker || failure) throw new Error(failure ?? "Replay owner absent")
        await worker.evaluate(mark => (globalThis as any).__playsrcGameplayReplay.mark(mark), mark)
      })
      pending = marked.catch(error => { failure = String(error) })
      return marked
    },
    stop(complete = true) {
      if (result) return result
      result = (async () => {
      stopped = true
      if (timer) clearInterval(timer)
      page.off("worker", attach)
      await pending
      await capture(true)
      const bytes = await readFile(partial)
      if (complete && !failure) { try { parseGameplayReplay(bytes) } catch (error) { failure = String(error) } }
      const sha256 = createHash("sha256").update(bytes).digest("hex")
      const file = `${sha256}.replay.bin`
      await writeFile(path.join(directory, file), bytes, { flag: "wx" }).catch(async error => {
        if (error.code !== "EEXIST" || !bytes.equals(await readFile(path.join(directory, file)))) throw error
      })
       const manifest = { schema: "playsrc-gameplay-replay-v1", file, sha256, bytes: bytes.length, complete: complete && !failure, checkpoint, mapOrdinal, error: failure }
      const manifestBytes = Buffer.from(JSON.stringify(manifest))
      const manifestFile = `${createHash("sha256").update(manifestBytes).digest("hex")}.replay.json`
      await writeFile(path.join(directory, manifestFile), manifestBytes, { flag: "wx" })
      await writeFile(progress, JSON.stringify({ ...manifest, manifestFile }))
      return { ...manifest, manifestFile }
      })()
      return result
    },
  }
}
