import { spawn } from "node:child_process"
import { randomUUID } from "node:crypto"
import { createWriteStream } from "node:fs"
import { mkdir, open, readFile, rename, rm, writeFile } from "node:fs/promises"
import { finished } from "node:stream/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot, type LocalConfig } from "./config"
import { parseHeadedProfile } from "./profile-runner"
import { TF2_TARGET_NAMES } from "@playsrc/game-tf2-browser/maps"
import { parseLocalPreparationStage } from "./prepare-local-stage"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import { runWindowsNativeJob, type NativeJobReceipt } from "./windows-job-native"

const LIMIT = 175_000
const SHA = /^[0-9a-f]{40}$/
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
type Job = { schema: "playsrc-local-job-v1"; id: string; origin: string; ref: string; commit: string; config: LocalConfig }

/** A task's launch record is its authority, never another run's newer log. */
export async function readLocalTaskResult(directory: string, task: string) {
  const token = task.slice("playsrc-local-job-".length)
  if (!task.startsWith("playsrc-local-job-") || !ID.test(token)) throw new Error("Invalid task identity")
  const bytes = await readFile(path.join(directory, `${token}-launch.log`))
  if (bytes.length > 2 * 1024 * 1024) throw new Error("Task launch record exceeds its bound")
  const text = bytes.toString(bytes[0] === 0xff && bytes[1] === 0xfe ? "utf16le" : "utf8").replace(/^\uFEFF/, "").trim()
  if (!text) return { result: null, launchError: null }
  try {
    const result = JSON.parse(text)
    if (result.schema !== "playsrc-local-job-result-v1" || result.id !== path.basename(directory)
      || result.task !== task || !SHA.test(result.commit) || !["passed", "failed", "denied", "cancelled"].includes(result.outcome) || !Array.isArray(result.command)
      || typeof result.run !== "string" || path.dirname(result.run) !== directory) throw new Error("Malformed task result")
    return { result, launchError: null }
  } catch { return { result: null, launchError: text } }
}

export function validateRevision(ref: string, commit: string): void {
  if (!SHA.test(commit) || !(SHA.test(ref) || /^refs\/(heads|tags)\/[A-Za-z0-9_][A-Za-z0-9_.\/-]*$/.test(ref))
    || ref.includes("..") || ref.includes("//") || ref.endsWith("/") || ref.endsWith(".lock")) {
    throw new Error("Expected an explicit refs/heads/... or refs/tags/... and an exact 40-character commit")
  }
}

export function localJobCommand(args: readonly string[]): { command: string[]; interactive: boolean } {
  const [kind, ...options] = args
  if (kind === "test") {
    if (options.some(value => !/^[A-Za-z0-9_./-]+\.test\.ts$/.test(value) || value.startsWith("-") || value.startsWith("/") || value.split("/").includes(".."))) {
      throw new Error("test accepts only repository-relative .test.ts files")
    }
    return { command: ["test", ...options], interactive: false }
  }
  if (kind === "profile") {
    parseHeadedProfile(options)
    return { command: ["tools/playsrc/src/profile-runner.ts", ...options], interactive: true }
  }
  if (kind === "build" && options.length === 1 && (TF2_TARGET_NAMES as readonly string[]).includes(options[0]!)) {
    return { command: ["tools/playsrc/src/cli.ts", "dev", options[0]!, "--prepare-only"], interactive: false }
  }
  if (kind === "build-stage") {
    parseLocalPreparationStage(options)
    return { command: ["tools/playsrc/src/prepare-local-stage.ts", ...options], interactive: false }
  }
  if (kind === "diagnostic" && options.length === 2 && /^\d{1,5}$/.test(options[0]!) && Number(options[0]) <= 30_000 && /^[01]$/.test(options[1]!)) {
    return { command: ["-e", `console.log('native diagnostic workload');setTimeout(()=>process.exit(${options[1]}),${options[0]})`], interactive: false }
  }
  throw new Error("Expected test [files...], build <map>, build-stage wasm|producer|resources <map>, or profile <normal profile name> [normal profiler options]")
}

/** Remote transport never supplies a browser endpoint, fixture, or asset server.
 * Keep host toolchain variables; let the checked-out normal command own the rest. */
export function localJobEnvironment(source: NodeJS.ProcessEnv, port?: number): NodeJS.ProcessEnv {
  const env = Object.fromEntries(Object.entries(source).filter(([key]) => !/^(PLAYSRC_|PROFILE_|NATIVE_|VITE_|TF2_|npm_lifecycle_event$)/i.test(key)))
  if (port !== undefined) env.PLAYSRC_DEV_PORT = String(port)
  return env
}

async function execute(command: string[], cwd: string, env: NodeJS.ProcessEnv, log?: string, timeout = LIMIT): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command[0]!, command.slice(1), { cwd, env, stdio: ["ignore", "pipe", "pipe"], windowsHide: true })
    let output = "", diagnostic = "", timedOut = false
    const stream = log ? createWriteStream(log, { flags: "wx" }) : undefined
    let logFailure: unknown
    stream?.on("error", error => { logFailure = error })
    child.stdout.on("data", bytes => { output = (output + bytes).slice(-16_384); stream?.write(bytes) })
    child.stderr.on("data", bytes => { diagnostic = (diagnostic + bytes).slice(-16_384); stream?.write(bytes) })
    const timer = setTimeout(() => {
      timedOut = true
      // Only the still-live child created by this invocation, never a shared
      // server, browser-name match, port owner, or user process.
      if (process.platform === "win32" && child.pid && child.exitCode === null) {
        spawn("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore", windowsHide: true })
      } else child.kill("SIGKILL")
    }, timeout)
    child.once("error", error => { clearTimeout(timer); stream?.end(); reject(error) })
    child.once("close", async code => {
      clearTimeout(timer)
      try {
        if (stream) { stream.end(); await finished(stream) }
        if (logFailure) throw logFailure
        if (timedOut || code !== 0) throw new Error(`${command[0]} exited ${code}${timedOut ? " (deadline exceeded)" : ""}: ${diagnostic || output}`)
        resolve(output.trim())
      } catch (error) { reject(error) }
    })
  })
}

export async function availableDevelopmentPort(): Promise<number> {
  for (let attempt = 0; attempt < 32; attempt++) {
    let application: ReturnType<typeof Bun.serve> | undefined, assets: ReturnType<typeof Bun.serve> | undefined
    try {
      // Use the same native listener as the asset service. A node:net close
      // callback is not proof that a different Windows process can bind it.
      const fetch = () => new Response(null, { status: 503 })
      application = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch })
      const port = application.port!
      if (port < 1024 || port >= 65535) continue
      assets = Bun.serve({ hostname: "127.0.0.1", port: port + 1, fetch })
      return port
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== "EADDRINUSE") throw error }
    finally {
      await Promise.all([application?.stop(true), assets?.stop(true)])
    }
  }
  throw new Error("Could not reserve adjacent local development/application asset ports")
}

export async function prepareLocalJob(ref: string, commit: string, root = repositoryRoot, reuse?: string): Promise<{ id: string; directory: string; commit: string }> {
  validateRevision(ref, commit)
  if (reuse !== undefined && !ID.test(reuse)) throw new Error("Invalid reusable job ID")
  const config = await loadLocalConfig(root)
  const env = localJobEnvironment(process.env)
  const origin = await execute(["git", "remote", "get-url", "origin"], root, env)
  const id = reuse ?? randomUUID(), directory = path.join(config.sourceCacheDir, "local-jobs", id), checkout = path.join(directory, "checkout")
  await mkdir(directory, { recursive: true })
  const preparing = await open(path.join(directory, "running"), "wx")
  const pending = path.join(directory, "job.pending.json")
  try {
  if (reuse) {
    const previous = JSON.parse(await readFile(pending, "utf8").catch(error => {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
      return readFile(path.join(directory, "job.json"), "utf8")
    })) as Job
    if (previous.id !== id || previous.origin !== origin) throw new Error("Reusable job origin differs")
    await assertCheckout(checkout, previous)
  }
  const job: Job = { schema: "playsrc-local-job-v1", id, origin, ref, commit, config }
  await writeFile(path.join(directory, `request-${randomUUID()}.json`), JSON.stringify(job, null, 2), { flag: "wx" })
  // No reset/clean/stash in the developer's checkout, no source or generated
  // artifact copying from a controller host, and no alternate toolchain setup.
  if (!reuse) {
    await execute(["git", "init", checkout], root, env)
    await execute(["git", "remote", "add", "origin", origin], checkout, env)
  }
  await execute(["git", "fetch", "--no-tags", "origin", ref], checkout, env, path.join(directory, `fetch-${randomUUID()}.log`))
  await execute(["git", "merge-base", "--is-ancestor", commit, "FETCH_HEAD"], checkout, env)
  await execute(["git", "checkout", "--detach", commit], checkout, env)
  if (!reuse) await writeFile(path.join(checkout, "playsrc.local.json"), JSON.stringify(config, null, 2), { flag: "wx" })
  await writeFile(pending, JSON.stringify(job, null, 2))
  await execute([process.execPath, "install", "--frozen-lockfile"], checkout, env, path.join(directory, `install-${randomUUID()}.log`))
  await assertCheckout(checkout, job)
  await rename(pending, path.join(directory, "job.json"))
  return { id, directory, commit }
  } finally { await preparing.close(); await rm(path.join(directory, "running")) }
}

async function assertCheckout(checkout: string, job: Job, deadline = Date.now() + LIMIT): Promise<void> {
  const env = localJobEnvironment(process.env)
  const query = (args: string[]) => execute(["git", ...args], checkout, env, undefined, Math.max(1, deadline - Date.now()))
  if (await query(["rev-parse", "HEAD"]) !== job.commit
    || await query(["status", "--porcelain", "--untracked-files=normal"]) !== ""
    || JSON.stringify(await loadLocalConfig(checkout)) !== JSON.stringify(job.config)) {
    throw new Error("Prepared checkout/configuration changed; prepare a new job instead of resetting it")
  }
}

export async function runLocalJob(id: string, args: readonly string[], root = repositoryRoot, task: string | null = null) {
  if (!ID.test(id)) throw new Error("Invalid local job ID")
  const plan = localJobCommand(args)
  const config = await loadLocalConfig(root), directory = path.join(config.sourceCacheDir, "local-jobs", id)
  const job = JSON.parse(await readFile(path.join(directory, "job.json"), "utf8")) as Job
  if (job.schema !== "playsrc-local-job-v1" || job.id !== id || !SHA.test(job.commit)
    || JSON.stringify(job.config) !== JSON.stringify(config)) throw new Error("Local job identity/configuration differs")
  const checkout = path.join(directory, "checkout")
  if (process.platform === "win32") {
    if (!task?.startsWith("playsrc-local-job-") || !ID.test(task.slice("playsrc-local-job-".length))) throw new Error("Windows delegated workloads require the scheduled native job bridge")
    const launcher = JSON.parse(await readFile(path.join(directory, `${task.slice("playsrc-local-job-".length)}-launch.owner.json`), "utf8"))
    if (launcher.pid !== process.ppid || !Number.isSafeInteger(launcher.startedEpoch) || launcher.sessionId < 1) throw new Error("Scheduled launcher identity differs")
    // Scheduler retry or duplicate invocation of the same task cannot consume
    // a second decision or launch again. A new attempt needs a new task token.
    await writeFile(path.join(directory, `${task.slice("playsrc-local-job-".length)}-claim.json`), JSON.stringify({ job: id, task, launcher, pid: process.pid, claimedAt: Date.now() }), { flag: "wx" })
  }
  const admittedAt = Date.now()
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  // Acquire once, before claiming the checkout: two tasks for the same prepared
  // job queue normally instead of the second barging into its running marker.
  const lock = process.platform === "win32" ? await acquireHeadedProfileLock(lockPath, `job:${id}:${args.join(" ")}`, LIMIT - 15_000) : undefined
  let running: Awaited<ReturnType<typeof open>> | undefined
  try {
  running = await open(path.join(directory, "running"), "wx")
  const owner = { pid: process.pid, startedAt: admittedAt, job: id, task, command: plan.command }
  const phase = async (name: string) => {
    const text = JSON.stringify({ ...owner, phase: name })
    await running!.write(text, 0, "utf8"); await running!.truncate(Buffer.byteLength(text))
  }
  const run = path.join(directory, randomUUID())
  await mkdir(run)
  await writeFile(path.join(run, "identity.json"), JSON.stringify({ ...owner, run, commit: job.commit }), { flag: "wx" })
  if (task) {
    const link = path.join(directory, `${task.slice("playsrc-local-job-".length)}-run.json`)
    await writeFile(`${link}.tmp`, JSON.stringify({ job: id, task, run, pid: process.pid }), { flag: "wx" })
    await rename(`${link}.tmp`, link)
    if (await Bun.file(path.join(directory, `${task.slice("playsrc-local-job-".length)}-cancel`)).exists()) await writeFile(path.join(run, "cancel"), "Cancellation requested while queued\n", { flag: "wx" })
  }
  const startedAt = owner.startedAt
  let port: number | undefined, preflightFailure: string | null = null
  try {
    await phase("source-validation")
    if (await Bun.file(path.join(directory, "job.pending.json")).exists()) throw new Error("Job preparation is incomplete; retry preparation before running")
    await assertCheckout(checkout, job, admittedAt + LIMIT - 10_000)
    await phase("reserve-ports")
    if (plan.interactive || args[0] === "build") port = await availableDevelopmentPort()
  } catch (error) { preflightFailure = String(error) }
  const command = plan.interactive
    ? [process.execPath, path.join(repositoryRoot, "tools/playsrc/src/profile-runner.ts"), "--application-root", checkout, ...plan.command.slice(1)]
    : [process.execPath, ...plan.command]
  let failure: string | null = null
  let native: NativeJobReceipt | null = null
  let outcome = "failed"
  try {
    if (process.platform === "win32") {
      await phase("native-consent-command-completion")
      const action = args.map(value => value.replace(/[\x00-\x1f]/g, " ")).join(" ").slice(0, 512)
      native = await runWindowsNativeJob({ job: id, task: task!, run, action, invocation: args, command, cwd: checkout,
        lockPath, lockToken: lock!.token, deadline: startedAt + LIMIT, diagnostic: args[0] === "diagnostic", preflightFailure }, localJobEnvironment(process.env, port), async () => {
          await phase("verify-source")
          if (preflightFailure) throw new Error(preflightFailure)
          await assertCheckout(checkout, job, startedAt + LIMIT - 4_000)
        })
      outcome = native.outcome === "completed" ? "passed" : native.outcome
      failure = native.error ?? (outcome === "passed" ? null : `Native job ${outcome} (exit ${native.exitCode})`)
    } else {
      if (preflightFailure) throw new Error(preflightFailure)
      await phase("command")
      await execute(command, checkout, localJobEnvironment(process.env, port), path.join(run, "command.log"), Math.max(1, LIMIT - (Date.now() - startedAt)))
      outcome = "passed"
      await phase("verify-source")
      await assertCheckout(checkout, job)
    }
  } catch (error) { failure = String(error); outcome = "failed" }
  const result = { schema: "playsrc-local-job-result-v1", id, task, commit: job.commit, checkout, command, port, startedAt, finishedAt: Date.now(), outcome, failure, run, native, lockWaitMilliseconds: lock?.milliseconds ?? 0 }
  await writeFile(path.join(run, "result.json"), JSON.stringify(result, null, 2), { flag: "wx" })
  return result
  } finally {
    try { if (running) { await running.close(); await rm(path.join(directory, "running")) } }
    finally { if (lock) await releaseHeadedProfileLock(lockPath, lock.token) }
  }
}

if (import.meta.main) {
  try {
    const [operation, ...args] = process.argv.slice(2)
    if (operation === "prepare" && (args.length === 2 || args.length === 3)) console.log(JSON.stringify(await prepareLocalJob(args[0]!, args[1]!, repositoryRoot, args[2])))
    else if (operation === "result" && args.length === 2 && ID.test(args[0]!)) {
      const config = await loadLocalConfig()
      console.log(JSON.stringify(await readLocalTaskResult(path.join(config.sourceCacheDir, "local-jobs", args[0]!), args[1]!)))
    }
    else if (operation === "run" && args.length >= 2) {
      const task = args[1] === "--task" ? args[2]! : null
      const workload = args.slice(task ? 3 : 1)
      localJobCommand(workload)
      if (process.platform === "win32" && !task) {
        const output = await execute(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", path.join(repositoryRoot, "tools/playsrc/windows-job.ps1"), "-Job", args[0]!, "-JobArguments", JSON.stringify(workload)], repositoryRoot, localJobEnvironment(process.env), undefined, 15_000)
        console.log(output)
      } else {
      const result = await runLocalJob(args[0]!, workload, repositoryRoot, task)
      console.log(JSON.stringify(result))
      if (result.failure) process.exitCode = 1
      }
    } else throw new Error("Usage: bun tools/playsrc/src/local-job.ts prepare <ref> <commit> [existing-job] | run <id> test|build|build-stage|profile|diagnostic ...")
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
