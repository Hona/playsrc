import { spawn } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import { closeSync, openSync, watch } from "node:fs"
import { mkdir, open, readFile, rename, rm, stat, unlink, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot, type LocalConfig } from "./config"
import { headedProfileTarget, type HeadedProfileTarget } from "../profile/profile-target"

const MAX_WAIT_MILLISECONDS = 180_000
const MAX_RUN_MILLISECONDS = 175_000
const OWNER_IDLE_MILLISECONDS = 8_000
const HEARTBEAT_MILLISECONDS = 2_000

const PROFILES = Object.freeze({
  gameplay: { config: "playwright.profile.config.ts", target: "jump_beef" },
  "frame-budget": { config: "playwright.profile.config.ts", target: "jump_beef", environment: { PROFILE_SCENARIOS: "frame-budget" } },
  "map-load": { config: "playwright.profile.config.ts", target: "jump_beef" },
  "cold-map": { config: "playwright.cold-map-profile.config.ts", target: "jump_beef" },
  "three-map-load": { config: "playwright.three-map-load.config.ts", target: "jump_beef" },
  "map-memory": { config: "playwright.map-memory-profile.config.ts", target: "jump_beef" },
  "map-sanity": { config: "playwright.map-sanity-profile.config.ts", target: "jump_beef" },
  "map-coverage": { config: "playwright.coverage-profile.config.ts", target: "jump_beef" },
  "upward-outdoors": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_UPWARD_OUTDOORS: "1" } },
  "upward-training-bots": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_SCENARIOS: "upward-training-bots" } },
  "class-switch-high-dpi": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_SCENARIOS: "upward-training-bots", PROFILE_UPWARD_CLASS_SWITCH: "1" } },
  "particle-combat": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_SCENARIOS: "upward-training-bots", PROFILE_PARTICLE_COMBAT: "1" } },
  "sky-coherence": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_SKY_COHERENCE: "1" } },
  "2fort": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_2FORT_MEMORY: "1" } },
  "2fort-full-match": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_2FORT_MEMORY: "1", PROFILE_2FORT_FULL_ROSTER: "1" } },
  bots: { config: "playwright.bot-profile.config.ts", target: "pl_upward" },
  "2fort-bots": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_CTF_BOTS: "1" } },
  "2fort-visual": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_2FORT_VISUAL: "1" } },
  "2fort-match": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_SCENARIOS: "2fort-match" } },
  "local-practice": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_SCENARIOS: "local-practice" } },
  engineer: { config: "playwright.engineer-profile.config.ts", target: "pl_upward" },
  gameui: { config: "playwright.gameui-profile.config.ts", target: "jump_beef" },
  "main-menu": { config: "playwright.profile.config.ts", target: "jump_beef", environment: { PROFILE_SCENARIOS: "main-menu" } },
  hud: { config: "playwright.hud-profile.config.ts", target: "jump_beef" },
  "class-hud": { config: "playwright.class-hud-profile.config.ts", target: "jump_beef" },
  "class-selection": { config: "playwright.class-selection-profile.config.ts", target: "jump_beef" },
  "application-lifecycle": { config: "playwright.profile.config.ts", target: "jump_beef", arguments: ["--grep", "TF2 application generation lifecycle"] },
  "application-upgrade": { config: "playwright.profile.config.ts", target: "jump_beef", environment: { PROFILE_SCENARIOS: "application-upgrade" } },
  "startup-browser": { config: "playwright.startup-profile.config.ts", target: "jump_beef" },
} satisfies Record<string, { config: string; target: HeadedProfileTarget; environment?: Record<string, string>; arguments?: readonly string[] }>)

export type HeadedProfile = keyof typeof PROFILES

type OwnerMetadata = Readonly<{
  schema: "playsrc-profile-owner-v1"
  token: string
  identity: string
  target: string
  repository: string
  pid: number
  url: string
  startup: Readonly<Record<string, unknown>>
}>

type OwnerState = Readonly<{ metadata: OwnerMetadata; reused: boolean; milliseconds: number }>

export function parseHeadedProfile(arguments_: readonly string[]): Readonly<{ profile: HeadedProfile; fresh: boolean; playwright: readonly string[] }> {
  const [profile, ...options] = arguments_
  if (!profile || !Object.hasOwn(PROFILES, profile)) {
    throw new Error(`Usage: bun run profile:<${Object.keys(PROFILES).join("|")}> [--fresh] [Playwright options]`)
  }
  let fresh = false
  const playwright: string[] = []
  for (const option of options) {
    if (option === "--fresh") fresh = true
    else if (option === "--headless") throw new Error("headed TF2 profiles never accept headless browser execution")
    else if (option !== "--headed") playwright.push(option)
  }
  return Object.freeze({ profile: profile as HeadedProfile, fresh, playwright: Object.freeze(playwright) })
}

export async function profileSourceIdentity(root = repositoryRoot): Promise<string> {
  const command = async (arguments_: string[]): Promise<string> => {
    const child = Bun.spawn(["git", ...arguments_], { cwd: root, stdout: "pipe", stderr: "pipe" })
    const [output, errors, status] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    if (status !== 0) throw new Error(`Headed profile source identity failed: ${errors.trim()}`)
    return output
  }
  const [head, changes, untracked] = await Promise.all([
    command(["rev-parse", "HEAD"]),
    command(["diff", "--binary", "HEAD", "--", ".", ":(exclude)**/*.md", ":(exclude)*.md"]),
    command(["ls-files", "--others", "--exclude-standard"]),
  ])
  const hash = createHash("sha256").update("playsrc-headed-profile-source-v1\0").update(head).update("\0").update(changes)
  for (const name of untracked.split("\n").filter((value) => value && !value.endsWith(".md")).sort()) {
    hash.update("\0").update(name).update("\0").update(await readFile(path.join(root, name)))
  }
  return hash.digest("hex")
}

function isAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true } catch (error) { return (error as NodeJS.ErrnoException).code !== "ESRCH" }
}

export async function acquireHeadedProfileLock(
  lockPath: string,
  profile: string,
  maximumWaitMilliseconds = MAX_WAIT_MILLISECONDS,
): Promise<Readonly<{ token: string; milliseconds: number }>> {
  if (!Number.isSafeInteger(maximumWaitMilliseconds) || maximumWaitMilliseconds < 1 || maximumWaitMilliseconds > MAX_WAIT_MILLISECONDS) {
    throw new Error("Machine-wide headed profile lock wait is outside its three-minute bound")
  }
  const started = Date.now()
  const token = randomUUID()
  let announcement = started
  let revision = 0
  let awaken: (() => void) | undefined
  const observer = watch(path.dirname(lockPath), (_event, filename) => {
    if (filename?.toString() !== path.basename(lockPath)) return
    revision += 1
    awaken?.()
  })
  try {
    while (Date.now() - started < maximumWaitMilliseconds) {
      const observed = revision
      try {
        const file = await open(lockPath, "wx", 0o600)
        try { await file.writeFile(`${JSON.stringify({ token, pid: process.pid, profile, startedAt: new Date().toISOString() })}\n`) }
        finally { await file.close() }
        return Object.freeze({ token, milliseconds: Date.now() - started })
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error
        try {
          const owner = JSON.parse(await readFile(lockPath, "utf8")) as { pid?: number }
          if (Number.isSafeInteger(owner.pid) && !isAlive(owner.pid!)) { await unlink(lockPath).catch(() => undefined); continue }
        } catch (failure) {
          if ((failure as NodeJS.ErrnoException).code !== "ENOENT") {
            const metadata = await stat(lockPath).catch(() => null)
            if (metadata && Date.now() - metadata.mtimeMs > 10_000) throw new Error("Machine-wide headed profile lock is malformed")
          }
        }
        if (Date.now() - announcement >= 10_000) {
          announcement = Date.now()
          console.error(`[performance] waiting for exclusive headed profile: ${profile}`)
        }
        if (revision !== observed) continue
        await new Promise<void>((resolve) => {
          const timer = setTimeout(resolve, Math.min(250, Math.max(1, maximumWaitMilliseconds - (Date.now() - started))))
          awaken = () => { clearTimeout(timer); resolve() }
        })
        awaken = undefined
      }
    }
  } finally {
    awaken = undefined
    observer.close()
  }
  throw new Error(`Timed out waiting for the machine-wide headed profile lock after ${maximumWaitMilliseconds} ms`)
}

export async function releaseHeadedProfileLock(lockPath: string, token: string): Promise<void> {
  const current = JSON.parse(await readFile(lockPath, "utf8")) as { token?: string }
  if (current.token !== token) throw new Error("Machine-wide headed profile lock ownership changed")
  await unlink(lockPath)
}

async function writeLease(metadataPath: string, token: string, milliseconds: number): Promise<void> {
  const destination = `${metadataPath}.lease`
  const temporary = `${destination}.${process.pid}.${randomUUID()}.tmp`
  try {
    await writeFile(temporary, `${JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token, expiresAt: Date.now() + milliseconds })}\n`)
    await rename(temporary, destination)
  } finally {
    await rm(temporary, { force: true })
  }
}

async function readOwner(metadataPath: string): Promise<OwnerMetadata | null> {
  try {
    const value = JSON.parse(await readFile(metadataPath, "utf8")) as OwnerMetadata
    if (value.schema !== "playsrc-profile-owner-v1" || typeof value.token !== "string" || typeof value.identity !== "string"
      || typeof value.target !== "string" || typeof value.repository !== "string" || !Number.isSafeInteger(value.pid)
      || typeof value.url !== "string" || !value.startup || typeof value.startup !== "object") {
      throw new Error("Shared headed profile development owner metadata is malformed")
    }
    return value
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null
    throw error
  }
}

async function verifyOwner(metadata: OwnerMetadata, identity: string, target: string): Promise<boolean> {
  if (!isAlive(metadata.pid) || metadata.identity !== identity || metadata.target !== target) return false
  try {
    const response = await fetch(new URL("/__playsrc/profile-owner", metadata.url), { cache: "no-store", signal: AbortSignal.timeout(2_000) })
    if (!response.ok) return false
    const value = await response.json() as Partial<OwnerMetadata>
    return value.schema === "playsrc-profile-owner-v1" && value.token === metadata.token && value.identity === identity
      && value.target === target && value.repository === metadata.repository
  } catch { return false }
}

async function stopOwner(metadataPath: string, metadata: OwnerMetadata): Promise<void> {
  if (isAlive(metadata.pid)) {
    process.kill(process.platform === "win32" ? metadata.pid : -metadata.pid, "SIGTERM")
    const deadline = Date.now() + 5_000
    while (isAlive(metadata.pid) && Date.now() < deadline) await Bun.sleep(50)
    if (isAlive(metadata.pid)) throw new Error("Shared headed profile development owner did not stop within 5000 ms")
  }
  const current = await readOwner(metadataPath)
  if (current?.token === metadata.token) await rm(metadataPath, { force: true })
  try {
    const lease = JSON.parse(await readFile(`${metadataPath}.lease`, "utf8")) as { token?: string }
    if (lease.token === metadata.token) await rm(`${metadataPath}.lease`, { force: true })
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  }
}

async function prepareOwner(config: LocalConfig, identity: string, target: string, fresh: boolean, metadataPath: string, remaining: () => number): Promise<OwnerState> {
  const started = Date.now()
  const current = await readOwner(metadataPath)
  if (current && !fresh && await verifyOwner(current, identity, target)) {
    await writeLease(metadataPath, current.token, MAX_RUN_MILLISECONDS)
    return Object.freeze({ metadata: current, reused: true, milliseconds: Date.now() - started })
  }
  if (current) await stopOwner(metadataPath, current)
  const token = randomUUID()
  await writeLease(metadataPath, token, MAX_RUN_MILLISECONDS)
  const logPath = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance", "profile-owner.log")
  const log = openSync(logPath, "a")
  const child = spawn(process.execPath, [path.join(repositoryRoot, "tools", "playsrc", "src", "profile-owner.ts"), target], {
    cwd: repositoryRoot,
    env: { ...process.env, PLAYSRC_PROFILE_SOURCE_IDENTITY: identity, PLAYSRC_PROFILE_OWNER_TOKEN: token, PLAYSRC_PROFILE_OWNER_PATH: metadataPath },
    detached: true,
    stdio: ["ignore", log, log],
  })
  closeSync(log)
  if (!child.pid) throw new Error("Shared headed profile development owner failed to start")
  const pid = child.pid
  child.unref()
  while (remaining() > 0) {
    if (!isAlive(pid)) {
      const logs = await readFile(logPath, "utf8")
      throw new Error(`Shared headed profile development owner exited before readiness:\n${logs.slice(-6_000)}`)
    }
    const metadata = await readOwner(metadataPath)
    if (metadata?.token === token && await verifyOwner(metadata, identity, target)) {
      return Object.freeze({ metadata, reused: false, milliseconds: Date.now() - started })
    }
    await Bun.sleep(100)
  }
  process.kill(process.platform === "win32" ? pid : -pid, "SIGTERM")
  throw new Error("Shared headed profile development owner exceeded the bounded profile runtime")
}

export async function runHeadedProfile(arguments_: readonly string[]): Promise<number> {
  const started = Date.now()
  const { profile, fresh, playwright } = parseHeadedProfile(arguments_)
  const configurationStarted = Date.now()
  const config = await loadLocalConfig()
  const evidence = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance")
  await mkdir(evidence, { recursive: true })
  const configurationMilliseconds = Date.now() - configurationStarted
  const identityStarted = Date.now()
  const identity = await profileSourceIdentity()
  const sourceIdentityMilliseconds = Date.now() - identityStarted
  const lockPath = path.join(evidence, "chromium-profile.lock")
  const maximumWait = MAX_RUN_MILLISECONDS - (Date.now() - started)
  if (maximumWait < 1) throw new Error(`${profile} exhausted its headed profile deadline before acquiring the machine-wide lock`)
  const lock = await acquireHeadedProfileLock(lockPath, profile, maximumWait)
  const locked = Date.now()
  const remaining = () => MAX_RUN_MILLISECONDS - (Date.now() - started)
  const metadataPath = path.join(evidence, "development-owner.json")
  const plan = PROFILES[profile]
  const environment = "environment" in plan ? plan.environment : {}
  const target = headedProfileTarget({ ...process.env, ...environment }, plan.target)
  let owner: OwnerState | undefined
  let heartbeat: ReturnType<typeof setInterval> | undefined
  let progress: ReturnType<typeof setInterval> | undefined
  let deadline: ReturnType<typeof setTimeout> | undefined
  let exitCode = 1
  let timedOut = false
  let heartbeatFailure: Error | undefined
  let child: ReturnType<typeof Bun.spawn> | undefined
  let browserMilliseconds = 0
  let playwrightPhases: unknown = null
  const timingPath = path.join(evidence, "phase-reports", `${profile}-${process.pid}.json`)
  try {
    if (!process.env.PLAYSRC_PROFILE_ORIGIN) {
      owner = await prepareOwner(config, identity, target, fresh, metadataPath, remaining)
      const ownerToken = owner.metadata.token
      heartbeat = setInterval(() => {
        void writeLease(metadataPath, ownerToken, MAX_RUN_MILLISECONDS).catch((error) => {
          heartbeatFailure = error instanceof Error ? error : new Error(String(error))
          child?.kill("SIGTERM")
        })
      }, HEARTBEAT_MILLISECONDS)
    }
    const browserStarted = Date.now()
    progress = setInterval(() => console.error(`[performance] ${profile} running ${Math.round((Date.now() - locked) / 1_000)}s`), 10_000)
    deadline = setTimeout(() => { timedOut = true; child?.kill("SIGTERM") }, Math.max(0, remaining()))
    const command = [
      process.env.PLAYSRC_PROFILE_PLAYWRIGHT_EXECUTABLE ?? process.execPath,
      path.join(repositoryRoot, "node_modules", "@playwright", "test", "cli.js"),
      "test",
      `--config=${plan.config}`,
      "--headed",
      ...(playwright.some((value) => value === "--output" || value.startsWith("--output="))
        ? []
        : ["--output", path.join(evidence, "playwright-results", `${profile}-${process.pid}`)]),
      ...("arguments" in plan ? plan.arguments : []),
      ...playwright,
    ]
    child = Bun.spawn(command, {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        ...environment,
        npm_lifecycle_event: `profile:${profile}`,
        PLAYSRC_PROFILE_MANAGED: "1",
        PLAYSRC_PROFILE_TIMING_PATH: timingPath,
        PLAYSRC_PROFILE_PROCESS_STARTED: String(browserStarted),
      },
      stdin: "ignore",
      stdout: "inherit",
      stderr: "inherit",
    })
    exitCode = await child.exited
    if (heartbeatFailure) throw heartbeatFailure
    browserMilliseconds = Date.now() - browserStarted
    try { playwrightPhases = JSON.parse(await readFile(timingPath, "utf8")) } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
    }
    if (timedOut) throw new Error(`${profile} exceeded the ${MAX_RUN_MILLISECONDS} ms bounded headed profile`)
    return exitCode
  } finally {
    const cleanupStarted = Date.now()
    if (progress) clearInterval(progress)
    if (heartbeat) clearInterval(heartbeat)
    if (deadline) clearTimeout(deadline)
    try {
      if (owner) {
        if (exitCode === 0 && !timedOut) await writeLease(metadataPath, owner.metadata.token, OWNER_IDLE_MILLISECONDS)
        else await stopOwner(metadataPath, owner.metadata)
      }
    } finally {
      await releaseHeadedProfileLock(lockPath, lock.token)
        const finished = Date.now()
        const report = Object.freeze({
          schema: "playsrc-browser-profile-run-v3",
          profile,
          command: `bun run profile:${profile} --headed`,
          repository: repositoryRoot,
          sourceFingerprint: identity,
          startedAt: new Date(started).toISOString(),
          finishedAt: new Date(finished).toISOString(),
          elapsedMilliseconds: finished - started,
          exitCode,
          timedOut,
          phases: Object.freeze({
            configurationMilliseconds,
            sourceIdentityMilliseconds,
            lockWaitMilliseconds: lock.milliseconds,
            ownerMilliseconds: owner?.milliseconds ?? 0,
            ownerReused: owner?.reused ?? false,
            ownerStartup: owner?.metadata.startup ?? null,
            origin: process.env.PLAYSRC_PROFILE_ORIGIN ?? "development-owner",
            headedBrowserMilliseconds: browserMilliseconds,
            playwright: playwrightPhases,
            cleanupMilliseconds: finished - cleanupStarted,
          }),
        })
        const filename = `${profile}-${new Date(started).toISOString().replaceAll(":", "-")}-${process.pid}.json`
        await writeFile(path.join(evidence, filename), `${JSON.stringify(report, null, 2)}\n`)
        console.error(`[performance] ${profile} total=${report.elapsedMilliseconds}ms owner=${owner?.milliseconds ?? 0}ms reused=${owner?.reused ?? false} headed=${browserMilliseconds}ms cleanup=${report.phases.cleanupMilliseconds}ms`)
    }
  }
}

if (import.meta.main) {
  try {
    process.exitCode = await runHeadedProfile(process.argv.slice(2))
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  }
}
