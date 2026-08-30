import { spawn } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import { closeSync, openSync } from "node:fs"
import { readFile, rm, writeFile } from "node:fs/promises"
import { replaceProfileLeaseFile } from "./profile-lease-rename"
import path from "node:path"
import { repositoryRoot } from "./config"
import { fileFingerprint } from "./file-fingerprint"
import { acquireHeadedProfileLock, releaseHeadedProfileLock, processIsAlive } from "./profile-lock"

type BrowserOwner = { token: string; pid: number; browserPid: number; arguments: string[]; endpoint: string; identity: string; executable: string; executableSha256: string }
export type BrowserLaunch = { channel?: string; args?: string[]; executablePath?: string }
export type PreparedBrowser = { launch: BrowserLaunch; identity: string; executable: string; executableSha256: string }

export async function prepareBrowserLaunch(launch: BrowserLaunch): Promise<PreparedBrowser> {
  const child = spawn(profileNodeExecutable(), [path.join(import.meta.dir, "profile-browser-server.cjs"), "prepare", JSON.stringify(launch)], { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] })
  const timer = setTimeout(() => child.kill(), 10_000)
  try {
    let output = "", diagnostic = ""
    child.stdout.on("data", bytes => { output += bytes })
    child.stderr.on("data", bytes => { diagnostic += bytes })
    const code = await new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve) })
    if (code !== 0) throw new Error(`Browser preparation failed: ${diagnostic}`)
    const { executable } = JSON.parse(output)
    return { launch, identity: await browserLaunchIdentity(launch), executable, executableSha256: await fileFingerprint(executable) }
  } finally { clearTimeout(timer) }
}

export function effectiveProfileBrowserLaunch(use: { channel?: string; launchOptions?: BrowserLaunch }, environment: Readonly<NodeJS.ProcessEnv>): BrowserLaunch {
  const channel = environment.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? use.channel
  return { ...use.launchOptions, ...(channel ? { channel } : {}) }
}

async function optionalJson(filename: string): Promise<any> {
  try { return JSON.parse(await readFile(filename, "utf8")) }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT" || error instanceof SyntaxError) return null
    throw error
  }
}

export function profileNodeExecutable(): string {
  const node = Bun.which("node")
  if (!node) throw new Error("The supported Playwright WebSocket server requires Node on PATH")
  return node
}

export async function browserLaunchIdentity(launch: BrowserLaunch): Promise<string> {
  return createHash("sha256").update(JSON.stringify(launch))
    .update(await fileFingerprint(import.meta.filename))
    .update(await fileFingerprint(path.join(import.meta.dir, "profile-browser-server.cjs")))
    .update(await fileFingerprint(path.join(import.meta.dir, "profile-lease-rename.ts")))
    .update(await fileFingerprint(profileNodeExecutable()))
    .update(await fileFingerprint(path.join(repositoryRoot, "node_modules/@playwright/test/package.json")))
    .update(await fileFingerprint(path.join(repositoryRoot, "bun.lock"))).digest("hex")
}

export async function browserLease(filename: string, token: string, milliseconds: number, closeUnderLockToken?: string): Promise<void> {
  const temporary = `${filename}.lease.${process.pid}.${randomUUID()}.tmp`
  try {
    await writeFile(temporary, JSON.stringify({ token, expiresAt: Date.now() + milliseconds, closeUnderLockToken }))
    await replaceProfileLeaseFile(temporary, `${filename}.lease`)
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

export async function prepareProfileBrowser(filename: string, prepared: PreparedBrowser, remaining: () => number, lockToken?: string, fresh = false): Promise<BrowserOwner & { reused: boolean }> {
  const { identity, launch } = prepared
  let previous: BrowserOwner | undefined
  try { previous = JSON.parse(await readFile(filename, "utf8")) } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  }
  if (previous && processIsAlive(previous.pid)) {
    const lease = await optionalJson(`${filename}.lease`)
    if (!fresh && lease?.token === previous.token && lease.expiresAt > Date.now() + 1_000
      && previous.identity === identity && previous.executable === prepared.executable && previous.executableSha256 === prepared.executableSha256) {
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
  await browserLease(filename, token, remaining(), lockToken)
  const logPath = `${filename}.${token}.log`
  const log = openSync(logPath, "wx", 0o600)
  const child = spawn(process.execPath, [import.meta.filename, filename, token, JSON.stringify(prepared)], {
    cwd: repositoryRoot, detached: process.platform !== "win32", windowsHide: true, stdio: ["ignore", log, log],
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

export async function retireProfileBrowser(filename: string, owner: BrowserOwner, lockToken: string, remaining: () => number): Promise<void> {
  await browserLease(filename, owner.token, 0, lockToken)
  while (processIsAlive(owner.pid) && remaining() > 0) await Bun.sleep(25)
  if (processIsAlive(owner.pid) || processIsAlive(owner.browserPid)) throw new Error("Owned browser teardown unconfirmed")
}

if (import.meta.main) {
  const [filename, token, encoded] = process.argv.slice(2)
  if (!filename || !path.isAbsolute(filename) || !token || !encoded) throw new Error("Missing headed browser lease")
  const prepared = JSON.parse(encoded) as PreparedBrowser
  const { launch } = prepared
  const child = spawn(profileNodeExecutable(), [path.join(import.meta.dir, "profile-browser-server.cjs"), JSON.stringify(launch)], { windowsHide: true, stdio: ["pipe", "pipe", "inherit"] })
  const exited = new Promise<void>((resolve, reject) => {
    child.once("error", reject)
    child.once("exit", code => code === 0 ? resolve() : reject(new Error(`Headed browser server exited with ${code}`)))
  })
  // Observe rejection during startup as well as during controlled shutdown.
  void exited.catch(() => undefined)
  const { endpoint, executable, browserPid, arguments: browserArguments } = await Promise.race([
    new Promise<{ endpoint: string; executable: string; browserPid: number; arguments: string[] }>((resolve, reject) => {
      let output = ""
      child.stdout!.on("data", chunk => {
        output += chunk.toString()
        if (!output.includes("\n")) return
        try { resolve(JSON.parse(output.split("\n")[0]!)) } catch (error) { reject(error) }
      })
    }),
    exited.then(() => { throw new Error("Headed browser server exited before publishing its endpoint") }),
  ])
  if (!Number.isSafeInteger(browserPid) || browserPid < 1 || !Array.isArray(browserArguments) || browserArguments.some(value => typeof value !== "string")) throw new Error("Owned browser launch receipt differs")
  if (path.resolve(executable) !== path.resolve(prepared.executable)) throw new Error("Launched browser differs from prepared executable")
  const owner: BrowserOwner = { token, pid: process.pid, browserPid, arguments: browserArguments, endpoint,
    identity: prepared.identity, executable, executableSha256: prepared.executableSha256 }
  let stopping = false
  const stop = async (underLock = false) => {
    if (stopping) return
    stopping = true
    clearInterval(monitor)
    const started = Date.now()
    const lockPath = path.join(path.dirname(filename), "chromium-profile.lock")
    const lock = underLock ? undefined : await acquireBrowserRetirementLock(filename, token)
    try {
      if (child.exitCode === null) child.stdin!.write("close\n")
      await exited
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
  child.once("exit", () => { void stop() })
  const temporary = `${filename}.${token}.tmp`
  const lease = await optionalJson(`${filename}.lease`)
  if (lease?.token !== token || lease.expiresAt <= Date.now()) {
    const holder = await optionalJson(path.join(path.dirname(filename), "chromium-profile.lock"))
    await stop(Boolean(lease?.closeUnderLockToken && holder?.token === lease.closeUnderLockToken))
  }
  else {
    try {
      await writeFile(temporary, JSON.stringify(owner))
      await replaceProfileLeaseFile(temporary, filename)
    } catch (error) {
      const holder = await optionalJson(path.join(path.dirname(filename), "chromium-profile.lock"))
      await stop(Boolean(lease?.closeUnderLockToken && holder?.token === lease.closeUnderLockToken))
      throw error
    } finally { await rm(temporary, { force: true }) }
  }
}
