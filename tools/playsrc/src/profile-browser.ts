import { spawn } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import { closeSync, openSync } from "node:fs"
import { readFile, rename, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { repositoryRoot } from "./config"
import { fileFingerprint } from "./file-fingerprint"
import { acquireHeadedProfileLock, releaseHeadedProfileLock, processIsAlive } from "./profile-lock"

type BrowserOwner = { token: string; pid: number; endpoint: string; identity: string; executable: string; executableSha256: string }
export type BrowserLaunch = { channel?: string; args?: string[] }

async function optionalJson(filename: string): Promise<any> {
  try { return JSON.parse(await readFile(filename, "utf8")) }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT" || error instanceof SyntaxError) return null
    throw error
  }
}

export async function browserLaunchIdentity(launch: BrowserLaunch): Promise<string> {
  return createHash("sha256").update(JSON.stringify(launch))
    .update(await fileFingerprint(import.meta.filename))
    .update(await fileFingerprint(path.join(repositoryRoot, "node_modules/@playwright/test/package.json")))
    .update(await fileFingerprint(path.join(repositoryRoot, "bun.lock"))).digest("hex")
}

export async function browserLease(filename: string, token: string, milliseconds: number, closeUnderLockToken?: string): Promise<void> {
  const temporary = `${filename}.lease.${process.pid}.${randomUUID()}.tmp`
  try {
    await writeFile(temporary, JSON.stringify({ token, expiresAt: Date.now() + milliseconds, closeUnderLockToken }))
    await rename(temporary, `${filename}.lease`)
  } finally { await rm(temporary, { force: true }) }
}

export async function acquireBrowserRetirementLock(filename: string, token: string) {
  const lockPath = path.join(path.dirname(filename), "chromium-profile.lock")
  const delegation = new AbortController()
  let delegated = false
  // A runner may reach the front before idle eviction. Accept its checked
  // token handoff rather than deadlocking behind a runner awaiting retirement.
  const handoff = setInterval(() => {
    void (async () => {
      const lease = await optionalJson(`${filename}.lease`)
      const holder = await optionalJson(lockPath)
      if (lease?.token === token && lease.closeUnderLockToken && holder?.token === lease.closeUnderLockToken) {
        delegated = true
        delegation.abort()
      }
    })().catch(error => delegation.abort(error))
  }, 100)
  try { return await acquireHeadedProfileLock(lockPath, "idle-browser-retirement", 175_000, { signal: delegation.signal }) }
  catch (error) { if (!delegated) throw error; return undefined }
  finally { clearInterval(handoff) }
}

export async function prepareProfileBrowser(filename: string, launch: BrowserLaunch, remaining: () => number, lockToken?: string): Promise<BrowserOwner & { reused: boolean }> {
  const identity = await browserLaunchIdentity(launch)
  let previous: BrowserOwner | undefined
  try { previous = JSON.parse(await readFile(filename, "utf8")) } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  }
  if (previous && processIsAlive(previous.pid)) {
    const lease = await optionalJson(`${filename}.lease`)
    if (lease?.token === previous.token && lease.expiresAt > Date.now() + 1_000
      && previous.identity === identity && await fileFingerprint(previous.executable) === previous.executableSha256) {
      await browserLease(filename, previous.token, remaining())
      return { ...previous, reused: true }
    }
    // Ask our checked lease owner to retire, never kill a browser PID belonging
    // to a different task or an arbitrary CDP endpoint.
    await browserLease(filename, previous.token, 0, lockToken)
    while (processIsAlive(previous.pid) && remaining() > 0) await Bun.sleep(50)
    if (processIsAlive(previous.pid)) throw new Error("Previous headed browser is still retiring at the command deadline")
  }
  if (remaining() <= 0) throw new Error("No command budget remains for headed browser startup")
  const token = randomUUID()
  await browserLease(filename, token, remaining())
  const logPath = `${filename}.${token}.log`
  const log = openSync(logPath, "wx", 0o600)
  const child = spawn(process.execPath, [import.meta.filename, filename, token, JSON.stringify(launch)], {
    cwd: repositoryRoot, detached: true, stdio: ["ignore", log, log],
  })
  closeSync(log)
  if (!child.pid) throw new Error("Headed browser owner failed to start")
  child.unref()
  while (remaining() > 0) {
    if (!processIsAlive(child.pid)) throw new Error(`Headed browser owner exited before readiness: ${(await readFile(logPath, "utf8")).slice(-4_000)}`)
    try {
      const owner = JSON.parse(await readFile(filename, "utf8")) as BrowserOwner
      if (owner.token === token) return { ...owner, reused: false }
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error }
    await Bun.sleep(50)
  }
  await browserLease(filename, token, 0, lockToken)
  throw new Error("Headed browser startup exceeded the command deadline")
}

if (import.meta.main) {
  const [filename, token, encoded] = process.argv.slice(2)
  if (!filename || !path.isAbsolute(filename) || !token || !encoded) throw new Error("Missing headed browser lease")
  const launch = JSON.parse(encoded) as BrowserLaunch
  const { chromium } = await import("@playwright/test")
  const server = await chromium.launchServer({ ...launch, host: "127.0.0.1", headless: false, timeout: 20_000 })
  const executable = server.process().spawnfile
  const owner: BrowserOwner = { token, pid: process.pid, endpoint: server.wsEndpoint(),
    identity: await browserLaunchIdentity(launch), executable, executableSha256: await fileFingerprint(executable) }
  let stopping = false
  const stop = async (underLock = false) => {
    if (stopping) return
    stopping = true
    clearInterval(monitor)
    const started = Date.now()
    const lockPath = path.join(path.dirname(filename), "chromium-profile.lock")
    const lock = underLock ? undefined : await acquireBrowserRetirementLock(filename, token)
    try {
      await server.close()
      try {
        if (JSON.parse(await readFile(filename, "utf8")).token === token) await rm(filename, { force: true })
      } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error }
      await writeFile(`${filename}.${token}.retirement.json`, JSON.stringify({ token, startedAt: started,
        elapsedMilliseconds: Date.now() - started, queueMilliseconds: lock?.milliseconds ?? 0, delegated: !lock }), { flag: "wx" })
    } finally { if (lock) await releaseHeadedProfileLock(lockPath, lock.token) }
  }
  const monitor = setInterval(() => {
    void (async () => {
      try {
        const lease = JSON.parse(await readFile(`${filename}.lease`, "utf8"))
        if (lease.token !== token || lease.expiresAt <= Date.now()) {
          const holder = await optionalJson(path.join(path.dirname(filename), "chromium-profile.lock"))
          // An explicit retirement request is made by the current exclusive
          // runner. Autonomous idle eviction queues like every other GUI task.
          await stop(Boolean(lease.closeUnderLockToken && holder?.token === lease.closeUnderLockToken))
        }
      } catch { await stop() }
    })()
  }, 500)
  process.once("SIGTERM", () => { void stop() })
  process.once("SIGINT", () => { void stop() })
  server.on("close", () => { void stop() })
  const temporary = `${filename}.${token}.tmp`
  const lease = await optionalJson(`${filename}.lease`)
  if (lease?.token !== token || lease.expiresAt <= Date.now()) {
    const holder = await optionalJson(path.join(path.dirname(filename), "chromium-profile.lock"))
    await stop(Boolean(lease?.closeUnderLockToken && holder?.token === lease.closeUnderLockToken))
  }
  else {
    await writeFile(temporary, JSON.stringify(owner))
    await rename(temporary, filename)
  }
}
