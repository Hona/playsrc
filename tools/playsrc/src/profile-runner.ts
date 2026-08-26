import { spawn } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import { closeSync, openSync } from "node:fs"
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot, type LocalConfig } from "./config"
import { headedProfileTarget, type HeadedProfileTarget } from "../profile/profile-target"
import { requireWindowsProfileConsole } from "../profile/windows-desktop"
import { acquireHeadedProfileLock, releaseHeadedProfileLock, processIsAlive as isAlive, ProfileQueueTimeout, type LockObservation } from "./profile-lock"
import { configuredProfileIdentity, generatedProfileIdentity } from "./profile-identity"
import { browserLease, prepareProfileBrowser, profileNodeExecutable } from "./profile-browser"
import { applicationBuildIdentity } from "./build-identity"
export { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"

const MAX_RUN_MILLISECONDS = 175_000
const OWNER_IDLE_MILLISECONDS = 60_000
const HEARTBEAT_MILLISECONDS = 2_000

export class ProfileCapacityDeferred extends Error {}

// Admission is outside Ready and the sample. A queued cold build can consume
// almost the entire command cap; retain that prepared build rather than launch
// a browser with too little time for startup, map admission and extraction.
export function requireBrowserBudget(milliseconds: number): void {
  if (milliseconds < 30_000) throw new ProfileCapacityDeferred(`Only ${milliseconds} ms remain after queue/build; reserve 30000 ms for the headed workflow. Exact preparation is retained; retry without --fresh.`)
}

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
  "2fort-startup": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_SCENARIOS: "upward-training-bots", PROFILE_STARTUP_CREATE_SERVER: "1", PROFILE_UPWARD_TRAINING_WARM_RELOAD: "1" } },
  bots: { config: "playwright.bot-profile.config.ts", target: "pl_upward" },
  "2fort-bots": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_CTF_BOTS: "1" } },
  "2fort-visual": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_2FORT_VISUAL: "1" } },
  "2fort-match": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_SCENARIOS: "2fort-match" } },
  "local-practice": { config: "playwright.profile.config.ts", target: "ctf_2fort", environment: { PROFILE_SCENARIOS: "local-practice" } },
  engineer: { config: "playwright.engineer-profile.config.ts", target: "pl_upward" },
  "integrated-lifecycle": { config: "playwright.profile.config.ts", target: "pl_upward", environment: { PROFILE_SCENARIOS: "integrated-lifecycle" } },
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
  generatedIdentity?: string
}>

type OwnerState = Readonly<{ metadata: OwnerMetadata; reused: boolean; milliseconds: number }>

export function parseHeadedProfile(arguments_: readonly string[]): Readonly<{ profile: HeadedProfile; fresh: boolean; playwright: readonly string[] }> {
  const [profile, ...options] = arguments_
  if (!profile || !Object.hasOwn(PROFILES, profile)) {
    throw new Error(`Usage: bun run profile:<${Object.keys(PROFILES).join("|")}> [--fresh] [Playwright options]`)
  }
  let fresh = false
  const playwright: string[] = []
  for (let index = 0; index < options.length; index++) {
    const option = options[index]!
    if (option === "--workers" && options[index + 1] !== "1" || option.startsWith("--workers=") && option !== "--workers=1"
      || option === "--fully-parallel" || option === "--ui" || option === "--debug") {
      throw new Error("Headed profiles require one bounded noninteractive sampling worker")
    }
  }
  for (const option of options) {
    if (option === "--fresh") fresh = true
    else if (option === "--headless") throw new Error("headed TF2 profiles never accept headless browser execution")
    else if (option !== "--headed") playwright.push(option)
  }
  return Object.freeze({ profile: profile as HeadedProfile, fresh, playwright: Object.freeze(playwright) })
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
      || typeof value.target !== "string" || typeof value.repository !== "string" || !Number.isSafeInteger(value.pid) || value.pid < 1
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
  if (!isAlive(metadata.pid) || metadata.identity !== identity || metadata.target !== target || metadata.repository !== repositoryRoot) return false
  try {
    if (metadata.generatedIdentity !== await generatedProfileIdentity()) return false
    return await ownerEndpointMatches(metadata)
  } catch { return false }
}

async function ownerEndpointMatches(metadata: OwnerMetadata): Promise<boolean> {
  try {
    const response = await fetch(new URL("/__playsrc/profile-owner", metadata.url), { cache: "no-store", signal: AbortSignal.timeout(2_000) })
    if (!response.ok) return false
    const value = await response.json() as Partial<OwnerMetadata>
    return value.schema === "playsrc-profile-owner-v1" && value.token === metadata.token && value.identity === metadata.identity
      && value.target === metadata.target && value.repository === metadata.repository
  } catch { return false }
}

export async function stopOwner(metadataPath: string, metadata: OwnerMetadata, maximumMilliseconds = 5_000): Promise<void> {
  if (!Number.isSafeInteger(metadata.pid) || metadata.pid < 1 || metadata.pid === process.pid) throw new Error("Refusing invalid development service PID")
  if (isAlive(metadata.pid)) {
    console.error(`[performance] retiring development owner pid=${metadata.pid} target=${metadata.target} by checked lease`)
    const deadline = Date.now() + maximumMilliseconds
    let announcement = Date.now()
    let nextInterrupt = Date.now() + 1_000
    while (isAlive(metadata.pid) && Date.now() < deadline) {
      const current = await readOwner(metadataPath)
      if (current && current.token !== metadata.token) throw new Error("Shared development owner changed during retirement")
      // Older runners may have an in-flight heartbeat after releasing the
      // machine lock. Keep the checked retirement request authoritative.
      await writeLease(metadataPath, metadata.token, 0)
      // Some older services hang while closing their Vite sockets. This PID is
      // the leased development service, not the agent or the lock holder. Only
      // interrupt that single PID after its live endpoint proves the exact
      // token/identity/repository again; never signal a foreign process group.
      if (Date.now() >= nextInterrupt && await ownerEndpointMatches(metadata)) {
        nextInterrupt = Date.now() + 1_000
        console.error(`[performance] interrupting verified idle development service pid=${metadata.pid}`)
        try { process.kill(metadata.pid, "SIGTERM") }
        catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error }
      }
      if (Date.now() - announcement >= 5_000) {
        announcement = Date.now()
        console.error(`[performance] waiting for development owner retirement pid=${metadata.pid} target=${metadata.target} alive=true`)
      }
      await Bun.sleep(50)
    }
    if (isAlive(metadata.pid)) throw new Error(`Shared headed profile development owner pid=${metadata.pid} remained live through its ${maximumMilliseconds} ms retirement budget; no process was killed`)
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
  const lease = JSON.parse(await readFile(`${metadataPath}.lease`, "utf8").catch(() => "null"))
  if (current && !fresh && lease?.token === current.token && lease.expiresAt > Date.now() + 1_000 && await verifyOwner(current, identity, target)) {
    await writeLease(metadataPath, current.token, MAX_RUN_MILLISECONDS)
    return Object.freeze({ metadata: current, reused: true, milliseconds: Date.now() - started })
  }
  if (current) await stopOwner(metadataPath, current, Math.max(1, remaining()))
  const token = randomUUID()
  await writeLease(metadataPath, token, MAX_RUN_MILLISECONDS)
  const logPath = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance", `profile-owner-${token}.log`)
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
  const cleanupDeadline = Date.now() + 4_000
  while (isAlive(pid) && Date.now() < cleanupDeadline) await Bun.sleep(25)
  if (isAlive(pid)) process.kill(process.platform === "win32" ? pid : -pid, "SIGKILL")
  throw new Error("Shared headed profile development owner exceeded the bounded profile runtime")
}

export function profileLockWaitBudget(elapsedMilliseconds: number, minimumRunMilliseconds = 0): number {
  if (!Number.isSafeInteger(minimumRunMilliseconds) || minimumRunMilliseconds < 0 || minimumRunMilliseconds >= MAX_RUN_MILLISECONDS) throw new Error("Headed scenario runtime reservation is invalid")
  return MAX_RUN_MILLISECONDS - elapsedMilliseconds - minimumRunMilliseconds
}

export async function runHeadedProfile(arguments_: readonly string[], minimumRunMilliseconds = 0): Promise<number> {
  const started = Date.now()
  const { profile, fresh, playwright } = parseHeadedProfile(arguments_)
  const configurationStarted = Date.now()
  const config = await loadLocalConfig()
  const evidence = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance")
  const runId = `${profile}-${new Date(started).toISOString().replaceAll(":", "-")}-${randomUUID()}`
  const runDirectory = path.join(evidence, "runs", runId)
  await mkdir(runDirectory, { recursive: true })
  const configurationMilliseconds = Date.now() - configurationStarted
  const lockPath = path.join(evidence, "chromium-profile.lock")
  const cancellation = new AbortController()
  // Five seconds of the unchanged total cap belong to cleanup, not sampling.
  const remaining = () => cancellation.signal.aborted ? 0 : Math.max(0, MAX_RUN_MILLISECONDS - 5_000 - (Date.now() - started))
  const metadataPath = path.join(evidence, "development-owner.json")
  const browserPath = path.join(evidence, "headed-browser.json")
  const plan = PROFILES[profile]
  const environment = "environment" in plan ? plan.environment : {}
  const target = headedProfileTarget({ ...process.env, ...environment }, plan.target)
  let lock: Awaited<ReturnType<typeof acquireHeadedProfileLock>> | undefined
  const observations: LockObservation[] = []
  let identity: string | null = null
  let configuredIdentity: string | null = null
  let generatedIdentity: string | null = null
  let currentPhase = "queue"
  const attempts: Array<{ phase: string; durationMilliseconds: number; complete: boolean }> = []
  const measure = async <T>(phase: string, action: () => Promise<T>): Promise<T> => {
    const began = Date.now()
    currentPhase = phase
    let complete = false
    try { const result = await action(); complete = true; return result }
    finally { attempts.push({ phase, durationMilliseconds: Date.now() - began, complete }) }
  }
  let sourceIdentityMilliseconds = 0
  let sourceVerificationMilliseconds = 0
  let owner: OwnerState | undefined
  let browser: Awaited<ReturnType<typeof prepareProfileBrowser>> | undefined
  let browserOwnerMilliseconds = 0
  let heartbeat: ReturnType<typeof setInterval> | undefined
  let heartbeatWrites = Promise.resolve()
  let progress: ReturnType<typeof setInterval> | undefined
  let exitCode = 1
  let timedOut = false
  let failure: string | null = null
  let outcome = "failed"
  let child: ReturnType<typeof spawn> | undefined
  let childExited = false
  let browserMilliseconds = 0
  let browserStarted: number | undefined
  let playwrightPhases: unknown = null
  let windowsConsole: ReturnType<typeof requireWindowsProfileConsole> = null
  const timingPath = path.join(runDirectory, "playwright-phases.json")
  const terminate = (signal: NodeJS.Signals) => {
    if (child?.pid && !childExited) {
      try { process.kill(process.platform === "win32" ? child.pid : -child.pid, signal) }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error }
    }
  }
  const cancel = () => { cancellation.abort(new Error("Headed profile cancelled")); terminate("SIGTERM") }
  process.once("SIGINT", cancel)
  process.once("SIGTERM", cancel)
  const deadline = setTimeout(() => { timedOut = true; cancel() }, remaining())
  const hardDeadline = setTimeout(() => terminate("SIGKILL"), Math.max(1, MAX_RUN_MILLISECONDS - (Date.now() - started)))
  try {
    windowsConsole = requireWindowsProfileConsole(remaining())
    // Preserve the full release matrix's earlier admission deadline.
    const maximumWait = Math.min(remaining(), profile === "application-upgrade" && !playwright.includes("--grep") ? 45_000 : MAX_RUN_MILLISECONDS)
    lock = await acquireHeadedProfileLock(lockPath, profile, Math.max(1, maximumWait), {
      signal: cancellation.signal,
      onProgress: state => {
        observations.push(state)
        console.error(`[performance] queued ${profile} position=${state.position}/${state.waiting} holder=${state.holder?.profile ?? "publishing"} pid=${state.holder?.pid ?? "unknown"} alive=${state.holderAlive} age=${state.holderAgeMilliseconds ?? "unknown"}ms wait=${state.elapsedMilliseconds}ms; no build/browser started`)
      },
    })
    progress = setInterval(() => console.error(`[performance] ${profile} phase=${currentPhase} total=${Date.now() - started}ms queued=${lock!.milliseconds}ms remaining=${remaining()}ms`), 10_000)
    const identityStarted = Date.now()
    identity = await measure("source-identity", () => applicationBuildIdentity())
    configuredIdentity = await measure("configured-content-identity", () => configuredProfileIdentity(config, target))
    sourceIdentityMilliseconds = Date.now() - identityStarted
    const ownerIdentity = createHash("sha256").update(identity).update(configuredIdentity).update(process.env.PLAYSRC_DEV_PORT ?? "4173").digest("hex")
    if (!process.env.PLAYSRC_PROFILE_ORIGIN) {
      owner = await measure("development-owner", () => prepareOwner(config, ownerIdentity, target, fresh, metadataPath, remaining))
      generatedIdentity = await measure("generated-wasm-identity", () => generatedProfileIdentity())
    }
    if (process.platform === "win32") windowsConsole = requireWindowsProfileConsole(remaining())
    requireBrowserBudget(remaining())
    if (!process.env.PLAYSRC_PROFILE_CDP_ENDPOINT && !process.env.PLAYSRC_PROFILE_BROWSER_ENDPOINT) {
      const began = Date.now()
      const { default: configuration } = await import(path.join(repositoryRoot, plan.config))
      const use = configuration.use ?? {}
      browser = await measure("browser-owner", () => prepareProfileBrowser(browserPath, { ...use.launchOptions, ...(use.channel ? { channel: use.channel } : {}) }, remaining, lock!.token))
      browserOwnerMilliseconds = Date.now() - began
    }
    heartbeat = setInterval(() => {
      heartbeatWrites = heartbeatWrites.then(async () => {
        if (owner) await writeLease(metadataPath, owner.metadata.token, Math.max(1, remaining()))
        if (browser) await browserLease(browserPath, browser.token, Math.max(1, remaining()))
      }).catch(error => { failure = String(error); cancel() })
    }, HEARTBEAT_MILLISECONDS)
    cancellation.signal.throwIfAborted()
    browserStarted = Date.now()
    currentPhase = "headed-browser"
    const command = [
      process.env.PLAYSRC_PROFILE_PLAYWRIGHT_EXECUTABLE ?? profileNodeExecutable(),
      path.join(repositoryRoot, "node_modules", "@playwright", "test", "cli.js"),
      "test",
      `--config=${plan.config}`,
      "--headed",
      ...(playwright.some((value) => value === "--output" || value.startsWith("--output="))
        ? []
        : ["--output", path.join(runDirectory, "results")]),
      ...("arguments" in plan ? plan.arguments : []),
      ...playwright,
    ]
    child = spawn(command[0]!, command.slice(1), {
      cwd: repositoryRoot,
      detached: true,
      env: {
        ...process.env,
        ...environment,
        npm_lifecycle_event: `profile:${profile}`,
        PLAYSRC_PROFILE_MANAGED: "1",
        PLAYSRC_PROFILE_SOURCE_IDENTITY: identity,
        PLAYSRC_PROFILE_TIMING_PATH: timingPath,
        PLAYSRC_PROFILE_PROCESS_STARTED: String(browserStarted),
        PLAYSRC_PROFILE_RUN_DIRECTORY: runDirectory,
        PLAYSRC_PROFILE_SOURCE_FINGERPRINT: identity!,
        PLAYSRC_PROFILE_BROWSER_ENDPOINT: browser?.endpoint ?? process.env.PLAYSRC_PROFILE_BROWSER_ENDPOINT,
      },
      stdio: ["ignore", "inherit", "inherit"],
    })
    exitCode = await new Promise<number>((resolve, reject) => {
      child!.once("error", reject)
      child!.once("exit", (code) => { childExited = true; resolve(code ?? 1) })
    })
    browserMilliseconds = Date.now() - browserStarted
    try { playwrightPhases = JSON.parse(await readFile(timingPath, "utf8")) } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
    }
    cancellation.signal.throwIfAborted()
    const verificationStarted = Date.now()
    if (identity !== await applicationBuildIdentity() || configuredIdentity !== await configuredProfileIdentity(config, target)
      || generatedIdentity !== null && generatedIdentity !== await generatedProfileIdentity()) throw new Error("Source/configuration/generated WASM changed during the command; evidence is not executable-current")
    sourceVerificationMilliseconds = Date.now() - verificationStarted
    outcome = exitCode === 0 ? "passed" : "failed"
    return exitCode
  } catch (error) {
    exitCode = 1
    failure ??= error instanceof Error ? error.message : String(error)
    if (error instanceof ProfileCapacityDeferred || !lock && (error instanceof ProfileQueueTimeout || timedOut)) {
      outcome = "deferred"
      exitCode = 75
      console.error(`[performance] capacity deferred (not a dead holder or failed gameplay): ${failure}. Retry when the queue has capacity.`)
    } else {
      outcome = timedOut ? "timed-out" : cancellation.signal.aborted ? "cancelled" : "failed"
      console.error(failure)
    }
    return exitCode
  } finally {
    const cleanupStarted = Date.now()
    if (progress) clearInterval(progress)
    if (heartbeat) clearInterval(heartbeat)
    if (browserStarted && !childExited) browserMilliseconds = Date.now() - browserStarted
    await heartbeatWrites
    try {
      if (owner) await writeLease(metadataPath, owner.metadata.token, outcome === "passed" || outcome === "deferred" ? OWNER_IDLE_MILLISECONDS : 0)
      if (browser) await browserLease(browserPath, browser.token, OWNER_IDLE_MILLISECONDS)
    } finally {
      if (lock) await releaseHeadedProfileLock(lockPath, lock.token)
      clearTimeout(deadline)
      clearTimeout(hardDeadline)
      process.off("SIGINT", cancel)
      process.off("SIGTERM", cancel)
        const finished = Date.now()
        const report = Object.freeze({
          schema: "playsrc-browser-profile-run-v4",
          runId,
          profile,
          command: ["bun", "tools/playsrc/src/profile-runner.ts", ...arguments_],
          repository: repositoryRoot,
          sourceFingerprint: identity,
          configuredIdentity,
          generatedIdentity,
          outcome,
          failure,
          queue: observations,
          attempts,
          startedAt: new Date(started).toISOString(),
          finishedAt: new Date(finished).toISOString(),
          elapsedMilliseconds: finished - started,
          exitCode,
          timedOut,
          windowsConsole,
          phases: Object.freeze({
            configurationMilliseconds,
            sourceIdentityMilliseconds,
            sourceVerificationMilliseconds,
            lockWaitMilliseconds: lock?.milliseconds ?? cleanupStarted - started - configurationMilliseconds,
            ownerMilliseconds: owner?.milliseconds ?? 0,
            ownerReused: owner?.reused ?? false,
            ownerStartup: owner?.metadata.startup ?? null,
            origin: process.env.PLAYSRC_PROFILE_ORIGIN ?? "development-owner",
            headedBrowserMilliseconds: browserMilliseconds,
            browserOwnerMilliseconds,
            browserReused: browser?.reused ?? false,
            browserRetention: browser ? { token: browser.token, idleMilliseconds: OWNER_IDLE_MILLISECONDS, contexts: "fresh-per-test", retirementReport: `${browserPath}.${browser.token}.retirement.json` } : null,
            playwright: playwrightPhases,
            cleanupMilliseconds: finished - cleanupStarted,
          }),
        })
        const exportStarted = Date.now()
        await writeFile(path.join(runDirectory, "command.json"), `${JSON.stringify(report, null, 2)}\n`, { flag: "wx" })
        console.error(`[performance] ${profile} ${outcome} total=${Date.now() - started}ms queue=${report.phases.lockWaitMilliseconds}ms owner=${owner?.milliseconds ?? 0}ms reused=${owner?.reused ?? false} headed=${browserMilliseconds}ms cleanup=${report.phases.cleanupMilliseconds}ms reportExport=${Date.now() - exportStarted}ms report=${path.join(runDirectory, "command.json")}`)
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
