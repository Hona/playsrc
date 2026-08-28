import { spawn } from "node:child_process"
import { randomUUID } from "node:crypto"
import { createWriteStream } from "node:fs"
import { mkdir, open, readFile, rename, rm, writeFile } from "node:fs/promises"
import { finished } from "node:stream/promises"
import { createServer } from "node:net"
import path from "node:path"
import { loadLocalConfig, repositoryRoot, type LocalConfig } from "./config"
import { parseHeadedProfile } from "./profile-runner"
import { requireWindowsProfileConsole } from "../profile/windows-desktop"
import { TF2_TARGET_NAMES } from "@playsrc/game-tf2-browser/maps"

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
      || !SHA.test(result.commit) || !["passed", "failed"].includes(result.outcome) || !Array.isArray(result.command)
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
  throw new Error("Expected test [files...], build <map>, or profile <normal profile name> [normal profiler options]")
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

async function availablePort(): Promise<number> {
  for (let attempt = 0; attempt < 32; attempt++) {
    const application = createServer(), assets = createServer()
    try {
      await new Promise<void>((resolve, reject) => { application.once("error", reject); application.listen(0, "127.0.0.1", resolve) })
      const address = application.address()
      if (!address || typeof address === "string" || address.port < 1024 || address.port >= 65535) continue
      await new Promise<void>((resolve, reject) => { assets.once("error", reject); assets.listen(address.port + 1, "127.0.0.1", resolve) })
      return address.port
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== "EADDRINUSE") throw error }
    finally {
      await Promise.all([application, assets].filter(server => server.listening).map(server => new Promise<void>(resolve => server.close(() => resolve()))))
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

async function assertCheckout(checkout: string, job: Job): Promise<void> {
  const env = localJobEnvironment(process.env)
  if (await execute(["git", "rev-parse", "HEAD"], checkout, env) !== job.commit
    || await execute(["git", "status", "--porcelain", "--untracked-files=normal"], checkout, env) !== ""
    || JSON.stringify(await loadLocalConfig(checkout)) !== JSON.stringify(job.config)) {
    throw new Error("Prepared checkout/configuration changed; prepare a new job instead of resetting it")
  }
}

export async function runLocalJob(id: string, args: readonly string[], ready: boolean, root = repositoryRoot) {
  if (!ID.test(id)) throw new Error("Invalid local job ID")
  const plan = localJobCommand(args)
  if (plan.interactive && !ready) throw new Error("A profile requires --ready for this user-approved hands-off window")
  const config = await loadLocalConfig(root), directory = path.join(config.sourceCacheDir, "local-jobs", id)
  const job = JSON.parse(await readFile(path.join(directory, "job.json"), "utf8")) as Job
  if (job.schema !== "playsrc-local-job-v1" || job.id !== id || !SHA.test(job.commit)
    || JSON.stringify(job.config) !== JSON.stringify(config)) throw new Error("Local job identity/configuration differs")
  const checkout = path.join(directory, "checkout")
  const running = await open(path.join(directory, "running"), "wx")
  try {
  if (await Bun.file(path.join(directory, "job.pending.json")).exists()) throw new Error("Job preparation is incomplete; retry preparation before running")
  await assertCheckout(checkout, job)
  const run = path.join(directory, randomUUID())
  await mkdir(run)
  const startedAt = Date.now(), port = plan.interactive || args[0] === "build" ? await availablePort() : undefined
  const command = [process.execPath, ...plan.command]
  let failure: string | null = null
  try {
    if (plan.interactive) requireWindowsProfileConsole()
    await execute(command, checkout, localJobEnvironment(process.env, port), path.join(run, "command.log"), Math.max(1, LIMIT - (Date.now() - startedAt)))
    await assertCheckout(checkout, job)
  } catch (error) { failure = String(error) }
  const result = { schema: "playsrc-local-job-result-v1", id, commit: job.commit, checkout, command, port, startedAt, finishedAt: Date.now(), outcome: failure ? "failed" : "passed", failure, run }
  await writeFile(path.join(run, "result.json"), JSON.stringify(result, null, 2), { flag: "wx" })
  return result
  } finally {
    await running.close()
    await rm(path.join(directory, "running"))
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
      const ready = args[1] === "--ready"
      const result = await runLocalJob(args[0]!, args.slice(ready ? 2 : 1), ready)
      console.log(JSON.stringify(result))
      if (result.failure) process.exitCode = 1
    } else throw new Error("Usage: bun tools/playsrc/src/local-job.ts prepare <ref> <commit> [existing-job] | run <id> [--ready] test|build|profile ...")
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
