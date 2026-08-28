import { createHash } from "node:crypto"
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Page, Worker } from "@playwright/test"
import { MAX_ADMISSION_EVENTS } from "../../../games/tf2/browser/src/admission-metrics"

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
export function replayTickCommand(data:Buffer,version:number):Buffer{
  if(![2,3,4].includes(version))throw new Error("Unknown replay command version")
  const offset=version===4?56:52;return data.subarray(offset,offset+data.readUInt32LE(48))
}
export function replayWorkClockBytes(records:readonly ReplayRecord[],version:number):Buffer{
  if(version!==4)throw new Error("Replay does not contain work-clock inputs")
  return Buffer.concat(records.filter(record=>record.kind===2).map(record=>record.bytes.subarray(56+record.bytes.readUInt32LE(48))))
}
export function parseGameplayReplay(bytes: Buffer, requireComplete = true, expectedMarks: 0 | 2 = 2) {
  if (bytes.length < 88 || bytes.length > REPLAY_BYTES || bytes.toString("ascii", 0, 4) !== "PGRP" || ![2,3,4].includes(bytes.readUInt32LE(4))
    || bytes.readBigUInt64LE(72) !== 0n || bytes.readBigUInt64LE(80) !== 1n) throw new Error("Replay initial checkpoint is invalid")
  const version = bytes.readUInt32LE(4), headerBytes = version >= 3 ? 780 : 88
  if (bytes.length < headerBytes || (version >= 3 && !bytes.subarray(88, 96).equals(Buffer.from("TFEQ\x01\0\0\0")))) throw new Error("Replay equipment checkpoint is invalid")
  const initialEquipment = version >= 3 ? bytes.subarray(88, headerBytes) : undefined
  const records: ReplayRecord[] = []
  let at = headerBytes, observing = false, complete = false, tick = -1n, marks = 0
  let lastClock=0
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
      const commandOffset=version===4?56:52
      if (!observing || data.length < commandOffset+84 || data.readUInt32LE(48)<84 || data.readBigUInt64LE(0) <= tick) throw new Error("Invalid authoritative tick order")
      const clocks=version===4?data.readUInt32LE(52):0
      if(clocks>4096||data.readUInt32LE(48)+commandOffset+clocks*8!==data.length)throw new Error("Invalid authoritative tick order")
      for(let offset=commandOffset+data.readUInt32LE(48);offset<data.length;offset+=8){const value=data.readDoubleLE(offset);if(!Number.isFinite(value)||value<lastClock)throw new Error("Invalid replay work clock");lastClock=value}
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
  if (requireComplete && (!complete || observing || marks !== expectedMarks)) throw new Error("Replay is incomplete")
  return { version, headerBytes, initialEquipment, bspSha256: bytes.subarray(8, 40).toString("hex"), worldSha256: bytes.subarray(40, 72).toString("hex"), records, complete, marks }
}

/** Durable incremental journal: owner-generated commands, never a heap/checkpoint dump. */
export async function startGameplayReplayJournal(page: Page, directory: string, label: string, mapOrdinal = 1, expectedMarks: 0 | 2 = 2, retainAdmission = false, entropyHex?: string) {
  if (!Number.isSafeInteger(mapOrdinal) || mapOrdinal < 1) throw new Error("Replay map ordinal rejected")
  await mkdir(directory, { recursive: true })
  const partial = path.join(directory, `${label}.replay.partial`)
  const progress = path.join(directory, `${label}.replay-progress.json`)
  await writeFile(partial, Buffer.alloc(0), { flag: "wx" })
  let worker: Worker | undefined, offset = 0, failure: string | null = null, checkpoint: ReplayCheckpoint | undefined
  let pending = Promise.resolve(), stopped = false, closedAt: number | null = null
  let admission: { file: string; sha256: string; bytes: number } | undefined
  let entropy: { file: string; sha256: string; bytes: number } | undefined
  const persistStatus = () => writeFile(progress, JSON.stringify({ schema: "playsrc-gameplay-replay-progress-v1", complete: false, bytes: offset, checkpoint, error: failure }))
  await persistStatus()
  const capture = async (stop = false) => {
    if (!worker || (failure && !stop)) return
    let timeout: ReturnType<typeof setTimeout> | undefined
    try {
      const result = await Promise.race([
        worker.evaluate(({ offset, stop, retainAdmission }) => {
          const owner = (globalThis as any).__playsrcGameplayReplay
          const result = owner.read(offset, stop)
          return result && stop && retainAdmission ? { ...result, admission: owner.admission() } : result
        }, { offset, stop, retainAdmission }),
        new Promise<never>((_, reject) => { timeout = setTimeout(() => reject(new Error("Replay owner read exceeded 3 seconds")), 3000) }),
      ])
      if (!result) return
      if (typeof result.base64 !== "string" || result.base64.length > Math.ceil(REPLAY_BYTES / 3) * 4 || !Number.isSafeInteger(result.length) || result.length > REPLAY_BYTES) throw new Error("Replay stream byte bound exceeded")
      const bytes = Buffer.from(result.base64, "base64")
      if (result.offset !== offset || offset + bytes.length !== result.length || result.length > REPLAY_BYTES) throw new Error("Replay stream identity mismatch")
      if (checkpoint && JSON.stringify(checkpoint) !== JSON.stringify(result.checkpoint)) throw new Error("Replay checkpoint changed")
      if (result.mapOrdinal !== mapOrdinal) throw new Error("Replay captured a different map construction")
      checkpoint = result.checkpoint
      if (stop && result.mapEntropyHex !== undefined) {
        if (!/^(?:[0-9a-f]{2})+$/.test(result.mapEntropyHex) || result.mapEntropyHex.length > 8 * 1024 * 1024) throw new Error("Malformed entropy record")
        const data = Buffer.from(result.mapEntropyHex, "hex"), sha256 = createHash("sha256").update(data).digest("hex"), file = `${sha256}.map-entropy.bin`
        await writeFile(path.join(directory, file), data, { flag: "wx" }).catch(async error => {
          if (error.code !== "EEXIST" || !data.equals(await readFile(path.join(directory, file)))) throw error
        })
        entropy = { file, sha256, bytes: data.length }
      }
      await appendFile(partial, bytes)
      offset += bytes.length
      if (stop && !result.complete) throw new Error("Authoritative replay owner reported incomplete evidence")
      if (stop && retainAdmission) {
        const value = result.admission
        if (value?.schema !== 1 || !Number.isFinite(value.timeOrigin) || !Number.isSafeInteger(value.dropped) || value.dropped < 0
          || !Array.isArray(value.events) || value.events.length > MAX_ADMISSION_EVENTS) throw new Error("Invalid admission evidence")
        const bytes = Buffer.from(JSON.stringify(value))
        if (bytes.length > REPLAY_BYTES) throw new Error("Admission evidence byte bound exceeded")
        const sha256 = createHash("sha256").update(bytes).digest("hex"), file = `${sha256}.admission.json`
        await writeFile(path.join(directory, file), bytes, { flag: "wx" }).catch(async error => {
          if (error.code !== "EEXIST" || !bytes.equals(await readFile(path.join(directory, file)))) throw error
        })
        admission = { file, sha256, bytes: bytes.length }
      }
    } catch (error) { failure = String(error) }
    finally { if (timeout) clearTimeout(timeout); await persistStatus() }
  }
  const attach = (value: Worker) => {
    if (!value.url().includes("gameplay-worker")) return
    if (worker) { failure = "Replay gameplay owner changed"; return }
    worker = value
    value.on?.("close", () => { closedAt = Date.now() })
    pending = value.evaluate(async ({ mapOrdinal, entropyHex }) => {
      const deadline = performance.now() + 5000
      while (!(globalThis as any).__playsrcGameplayReplay && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10))
      ;(globalThis as any).__playsrcGameplayReplay.arm(mapOrdinal, entropyHex)
    }, { mapOrdinal, entropyHex }).catch(error => { failure = String(error) })
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
    owner: () => worker,
    closedAt: () => closedAt,
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
       await pending
       await capture(true)
       page.off("worker", attach)
      const bytes = await readFile(partial)
       if (complete && !failure) { try { parseGameplayReplay(bytes, true, expectedMarks) } catch (error) { failure = String(error) } }
      const sha256 = createHash("sha256").update(bytes).digest("hex")
      const file = `${sha256}.replay.bin`
      await writeFile(path.join(directory, file), bytes, { flag: "wx" }).catch(async error => {
        if (error.code !== "EEXIST" || !bytes.equals(await readFile(path.join(directory, file)))) throw error
      })
        const manifest = { schema: "playsrc-gameplay-replay-v1", file, sha256, bytes: bytes.length, complete: complete && !failure, checkpoint, mapOrdinal, expectedMarks, admission, entropy, error: failure }
      const manifestBytes = Buffer.from(JSON.stringify(manifest))
      const manifestFile = `${createHash("sha256").update(manifestBytes).digest("hex")}.replay.json`
      await writeFile(path.join(directory, manifestFile), manifestBytes, { flag: "wx" }).catch(async error => {
        if (error.code !== "EEXIST" || !manifestBytes.equals(await readFile(path.join(directory, manifestFile)))) throw error
      })
      await writeFile(progress, JSON.stringify({ ...manifest, manifestFile }))
      return { ...manifest, manifestFile }
      })()
      return result
    },
  }
}

export type ReplayInstalledIdentity = { target: string; resourceRoot: string; bsp: string; mapGeneration: number; contentBuild: string }
export function bindReplayGeneration(journal: any, installed: ReplayInstalledIdentity, bsp: string) {
  if (!journal.complete || !journal.checkpoint || !installed || !["pl_upward", "ctf_2fort"].includes(installed.target)
    || !/^[0-9a-f]{64}$/.test(installed.resourceRoot) || installed.bsp !== bsp || !/^\d+$/.test(installed.contentBuild)
    || installed.mapGeneration !== journal.checkpoint.generation) throw new Error("Replay generation/resource identity mismatch")
  return { target: installed.target, resourceRoot: installed.resourceRoot, bsp: installed.bsp,
    mapGeneration: installed.mapGeneration, contentBuild: installed.contentBuild }
}

/** Explicit requested navigation boundaries, never automatic replacement recovery.
 * Setup journals end before navigation is requested; they do not claim teardown
 * coverage. Each new Worker must close its predecessor and authenticate anew. */
export async function startGameplayReplayLifecycle(page: Page, directory: string, label: string, warmReload: boolean, mapOrdinal = 1, entropyHex?: string) {
  if (warmReload && mapOrdinal !== 1) throw new Error("Warm-reload replay requires the initial map of each Worker")
  let journal = await startGameplayReplayJournal(page, directory, `${label}-worker-1`, mapOrdinal, warmReload ? 0 : 2, true, warmReload ? undefined : entropyHex)
  let ordinal = 1, previous: typeof journal | undefined, transitionAt: number | undefined
  let stopped = false
  const generations: any[] = []
  const retain = async (installed: ReplayInstalledIdentity, complete: boolean, expectedMarks: 0 | 2) => {
    const artifact = await journal.stop(complete)
    const bytes = await readFile(path.join(directory, artifact.file))
    const parsed = complete ? parseGameplayReplay(bytes, true, expectedMarks) : undefined
    const identity = complete ? bindReplayGeneration(artifact, installed, parsed!.bspSha256) : installed
    const entry = { workerOrdinal: ordinal, mapOrdinal, scope: expectedMarks === 0 ? "checkpoint-to-pre-navigation" : "checkpoint-through-sample",
      applicationGeneration: identity, journal: artifact,
      ...(previous ? { transition: { requestedAt: transitionAt, previousClosedAt: previous.closedAt() } } : {}) }
    generations.push(entry)
    return artifact
  }
  let result: Promise<any> | undefined
  return {
    async beforeReload(installed: ReplayInstalledIdentity) {
      if (!warmReload || ordinal !== 1 || stopped) throw new Error("Unexpected replay navigation")
      await journal.ready()
      await retain(installed, true, 0)
      previous = journal
      transitionAt = Date.now()
      ordinal = 2
      journal = await startGameplayReplayJournal(page, directory, `${label}-worker-2`, mapOrdinal, 2, true, entropyHex)
    },
    async ready() {
      if (ordinal !== (warmReload ? 2 : 1)) throw new Error("Requested replay navigation missing")
      await journal.ready()
      if (previous && (previous.owner() === journal.owner() || previous.closedAt() === null)) throw new Error("Replay predecessor Worker did not close")
    },
    mark: (mark: number) => journal.mark(mark),
    async stopAdmission() {
      const owner = journal.owner()
      if (!owner || stopped) throw new Error("Admission recorder owner absent")
      await owner.evaluate(() => (globalThis as any).__playsrcGameplayReplay.stopAdmission())
    },
    stop(installed: ReplayInstalledIdentity, complete = true) {
      if (result) return result
      stopped = true
      result = (async () => {
        const artifact = await retain(installed, complete, ordinal === 1 && warmReload ? 0 : 2)
        const manifest = { schema: "playsrc-gameplay-replay-lifecycle-v1", requestedWorkers: warmReload ? 2 : 1,
          complete: complete && artifact.complete && generations.every(entry => entry.journal.complete)
            && generations.length === (warmReload ? 2 : 1)
            && (!previous || previous.owner() !== journal.owner() && previous.closedAt() !== null && previous.closedAt()! >= transitionAt!), generations }
        validateReplayLifecycle(manifest, manifest.complete)
        const bytes = Buffer.from(JSON.stringify(manifest)), sha256 = createHash("sha256").update(bytes).digest("hex")
        const file = `${sha256}.replay-lifecycle.json`
        await writeFile(path.join(directory, file), bytes, { flag: "wx" }).catch(async error => {
          if (error.code !== "EEXIST" || !bytes.equals(await readFile(path.join(directory, file)))) throw error
        })
        return { artifact, lifecycle: { file, sha256, bytes: bytes.length, complete: manifest.complete } }
      })()
      return result
    },
  }
}

export function validateReplayLifecycle(value: any, requireComplete = true) {
  if (value?.schema !== "playsrc-gameplay-replay-lifecycle-v1" || ![1, 2].includes(value.requestedWorkers)
    || !Array.isArray(value.generations) || value.generations.length > value.requestedWorkers
    || (requireComplete && (!value.complete || value.generations.length !== value.requestedWorkers))) throw new Error("Incomplete replay lifecycle")
  for (const [index, entry] of value.generations.entries()) {
    const marks = index === value.requestedWorkers - 1 ? 2 : 0
    if (entry.workerOrdinal !== index + 1 || !Number.isSafeInteger(entry.mapOrdinal) || entry.mapOrdinal < 1
      || entry.journal?.expectedMarks !== marks || entry.journal?.mapOrdinal !== entry.mapOrdinal
      || entry.scope !== (marks === 0 ? "checkpoint-to-pre-navigation" : "checkpoint-through-sample")
      || (index === 0 ? entry.transition !== undefined : requireComplete && (!Number.isSafeInteger(entry.transition?.requestedAt)
        || !Number.isSafeInteger(entry.transition?.previousClosedAt) || entry.transition.previousClosedAt < entry.transition.requestedAt))) throw new Error("Invalid replay generation order")
    if (requireComplete) bindReplayGeneration(entry.journal, entry.applicationGeneration, entry.applicationGeneration?.bsp)
  }
}
