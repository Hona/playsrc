import { randomUUID } from "node:crypto"
import { closeSync, openSync, unlinkSync, watch, writeFileSync } from "node:fs"
import { mkdir, readFile, readdir, rename, stat, unlink, writeFile } from "node:fs/promises"
import path from "node:path"

const MAX_WAIT = 180_000
let admissionSequence = 0
export function processIsAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true }
  catch (error) { return (error as NodeJS.ErrnoException).code !== "ESRCH" }
}

type Ticket = { token: string; pid: number; profile: string; repository: string; startedAt: string; sequence?: number }
export type LockObservation = Readonly<{
  elapsedMilliseconds: number; position: number; waiting: number
  holder: Ticket | null; holderAlive: boolean | null; holderAgeMilliseconds: number | null
  recovered: number
}>
export class ProfileQueueTimeout extends Error {
  constructor(readonly observation: LockObservation) {
    super(`Headed profile deferred after ${observation.elapsedMilliseconds} ms: queue position ${observation.position}, holder ${observation.holder?.profile ?? "unpublished"} pid=${observation.holder?.pid ?? "unknown"} alive=${observation.holderAlive}. No holder was killed; retry this command when capacity is available.`)
  }
}

async function readTicket(filename: string): Promise<Ticket | null> {
  try {
    const value = JSON.parse(await readFile(filename, "utf8")) as Ticket
    if (!Number.isSafeInteger(value.pid) || value.pid < 1 || typeof value.token !== "string") throw new Error(`Malformed headed profile ownership: ${filename}`)
    return value
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null
    throw error
  }
}

/** Tickets are published atomically. Only the oldest live ticket may contend
 * for the existing lock file, so old runners remain exclusive during migration.
 * Nothing expires a live PID, even when it has exceeded its advertised deadline.
 */
export async function acquireHeadedProfileLock(lockPath: string, profile: string, maximumWaitMilliseconds = MAX_WAIT,
  options: { signal?: AbortSignal; onProgress?: (state: LockObservation) => void } = {},
): Promise<Readonly<{ token: string; milliseconds: number; observation: LockObservation }>> {
  if (!Number.isSafeInteger(maximumWaitMilliseconds) || maximumWaitMilliseconds < 1 || maximumWaitMilliseconds > MAX_WAIT) {
    throw new Error("Machine-wide headed profile lock wait is outside its three-minute bound")
  }
  const started = Date.now()
  const sequence = ++admissionSequence
  const token = randomUUID()
  const queue = `${lockPath}.queue`
  await mkdir(queue, { recursive: true })
  const name = `${String(started).padStart(24, "0")}-${String(process.pid).padStart(10, "0")}-${String(sequence).padStart(10, "0")}-${token}.json`
  const ticketPath = path.join(queue, name)
  const ticket: Ticket = { token, pid: process.pid, profile, repository: process.cwd(), startedAt: new Date(started).toISOString(), sequence }
  const temporary = `${ticketPath}.tmp`
  let awaken: (() => void) | undefined
  let revision = 0
  const changed = () => { revision++; awaken?.() }
  let eligible = false
  let claimed = false
  let handedOff = false
  let claimError: unknown
  const tryClaim = () => {
    if (!eligible || claimed || claimError || options.signal?.aborted || Date.now() - started >= maximumWaitMilliseconds) return
    let file: number | undefined
    try {
      file = openSync(lockPath, "wx", 0o600)
      writeFileSync(file, `${JSON.stringify({ ...ticket, startedAt: new Date().toISOString() })}\n`)
      claimed = true
      changed()
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") {
        claimError = error
        if (file !== undefined) unlinkSync(lockPath)
        changed()
      }
    } finally { if (file !== undefined) closeSync(file) }
  }
  // Only the selected head performs this one O_EXCL syscall. Directory-watch
  // delivery is coalesced on macOS; re-scanning tickets before claiming lets
  // older non-queued runners repeatedly barge ahead. No build, hashing, or
  // source scan runs in this handoff path, and an existing lock is never stolen.
  const handoff = setInterval(tryClaim, 10)
  const observers = [watch(queue, changed), watch(path.dirname(lockPath), (_event, filename) => {
    if (filename?.toString() === path.basename(lockPath)) { tryClaim(); changed() }
  })]
  options.signal?.addEventListener("abort", changed)
  let observation: LockObservation = { elapsedMilliseconds: 0, position: 1, waiting: 1, holder: null, holderAlive: null, holderAgeMilliseconds: null, recovered: 0 }
  let lastAnnouncement = -Infinity
  try {
    await writeFile(temporary, JSON.stringify(ticket), { flag: "wx", mode: 0o600 })
    await rename(temporary, ticketPath)
    while (Date.now() - started < maximumWaitMilliseconds) {
      options.signal?.throwIfAborted()
      if (claimError) throw claimError
      if (claimed) {
        handedOff = true
        return { token, milliseconds: Date.now() - started, observation }
      }
      const observed = revision
      const tickets: Array<{ name: string; started: number; pid: number; sequence: number }> = []
      for (const entry of (await readdir(queue)).filter(name => name.endsWith(".json")).sort()) {
        const candidate = await readTicket(path.join(queue, entry))
        if (!candidate) continue
        if (!processIsAlive(candidate.pid)) { await unlink(path.join(queue, entry)).catch(() => undefined); continue }
        const timestamp = Date.parse(candidate.startedAt)
        if (!Number.isFinite(timestamp)) throw new Error("Queued profile ticket has no valid admission time")
        tickets.push({ name: entry, started: timestamp, pid: candidate.pid, sequence: candidate.sequence ?? 0 })
      }
      // Bun and Node do not share an hrtime epoch. The published wall-clock
      // admission time, not a per-process timer embedded in a filename, owns
      // FIFO order for every existing ticket.
      // Same-process requests can publish within one clock millisecond on fast
      // filesystems. Keep call order rather than randomly sorting their UUIDs.
      const live = tickets.sort((left, right) => left.started - right.started || left.pid - right.pid || left.sequence - right.sequence || left.name.localeCompare(right.name)).map(ticket => ticket.name)
      eligible = live[0] === name
      tryClaim()
      if (claimed || claimError) continue
      let holder: Ticket | null = null
      try { holder = await readTicket(lockPath) } catch (error) {
        // Legacy writers publish into an empty O_EXCL file. Allow that small
        // publication window, but never interpret malformed metadata as dead.
        const metadata = await stat(lockPath).catch(() => null)
        if (metadata && Date.now() - metadata.mtimeMs > 10_000) throw error
      }
      observation = { ...observation, elapsedMilliseconds: Date.now() - started, position: live.indexOf(name) + 1, waiting: live.length,
        holder, holderAlive: holder ? processIsAlive(holder.pid) : null,
        holderAgeMilliseconds: holder?.startedAt ? Date.now() - Date.parse(holder.startedAt) : null }
      if (live[0] === name) {
        if (holder && !observation.holderAlive) {
          // Only the queue head reclaims; re-read identity before touching a
          // legacy holder. Its dead PID cannot release or hand off this file.
          if ((await readTicket(lockPath))?.token === holder.token) {
            await unlink(lockPath)
            observation = { ...observation, recovered: observation.recovered + 1 }
          }
          continue
        }
      }
      if (Date.now() - lastAnnouncement >= 10_000) {
        lastAnnouncement = Date.now()
        if (options.onProgress) options.onProgress(observation)
        else console.error(`[performance] queued ${profile} position=${observation.position}/${observation.waiting} holder=${holder?.profile ?? "publishing"} pid=${holder?.pid ?? "unknown"} alive=${observation.holderAlive} age=${observation.holderAgeMilliseconds ?? "unknown"}ms wait=${observation.elapsedMilliseconds}ms`)
      }
      if (revision !== observed) continue
      await new Promise<void>(resolve => {
        const timer = setTimeout(resolve, Math.min(250, Math.max(1, maximumWaitMilliseconds - (Date.now() - started))))
        awaken = () => { clearTimeout(timer); resolve() }
        if (revision !== observed) awaken()
      })
      awaken = undefined
    }
    throw new ProfileQueueTimeout({ ...observation, elapsedMilliseconds: Date.now() - started })
  } finally {
    clearInterval(handoff)
    options.signal?.removeEventListener("abort", changed)
    observers.forEach(observer => observer.close())
    await unlink(ticketPath).catch(() => undefined)
    await unlink(temporary).catch(() => undefined)
    if (claimed && !handedOff) await releaseHeadedProfileLock(lockPath, token)
  }
}

export async function releaseHeadedProfileLock(lockPath: string, token: string): Promise<void> {
  if ((await readTicket(lockPath))?.token !== token) throw new Error("Machine-wide headed profile lock ownership changed")
  await unlink(lockPath)
}
