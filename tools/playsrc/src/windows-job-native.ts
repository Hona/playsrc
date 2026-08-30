import { spawn } from "node:child_process"
import { readFile, writeFile, rename, mkdir } from "node:fs/promises"
import { createHash, randomUUID } from "node:crypto"
import path from "node:path"
import { repositoryRoot } from "./config"
import { processIsAlive } from "./profile-lock"
import { localJobCommand } from "./local-job-command"
import type { WindowsDesktopState } from "../profile/windows-desktop"

export type NativeDialog = {
  decision: string; error: string | null; displayedAt: number; decidedAt: number; dismissedAt: number
  visibleMilliseconds: number; window: number; sessionId: number
}
export type NativeJobReceipt = {
  schema: "playsrc-native-job-v2"; job: string; task: string; run: string; action: string; lockToken: string
  desktop: NativeDesktopReceipt[]
  invocation: readonly string[]; interactive: boolean; uiInvocations: number
  ownerPid: number; ownerCreatedAt: number; helperPid: number; helperCreatedAt: number; sessionId: number
  childPid: number; childCreatedAt: number; commandStartedAt: number; teardownAt: number
  startedAt: number; finishedAt: number; outcome: "completed" | "failed" | "cancelled" | "denied"
  exitCode: number | null; treeEmpty: boolean; error: string | null
}
export type NativeDesktopReceipt = {
  schema: "playsrc-native-desktop-v1"; job: string; task: string; run: string; lockToken: string
  childPid: number; childCreatedAt: number; helperPid: number; helperCreatedAt: number
  stage: string; preparedIdentity: string; preparedAt: number; desktopStartedAt: number; desktopReleasedAt: number
  succeeded: boolean; consent: NativeDialog | null; completion: NativeDialog | null
  console: WindowsDesktopState | null
}

export function approvedNativeDecision(value: NativeDialog | null): boolean {
  if (!value || value.error || ![value.displayedAt, value.decidedAt, value.dismissedAt, value.visibleMilliseconds, value.window, value.sessionId].every(Number.isSafeInteger)
    || value.window <= 0 || value.sessionId <= 0 || value.displayedAt <= 0 || value.decidedAt < value.displayedAt || value.dismissedAt < value.decidedAt || value.visibleMilliseconds < 0) return false
  return value.decision === "approved" || value.decision === "approved-timeout" && value.visibleMilliseconds >= 3_000 && value.decidedAt - value.displayedAt >= 3_000
}

function admittedNativeConsole(value: WindowsDesktopState | null, sessionId: number): boolean {
  return !!value && [value.consoleSessionId, value.processSessionId, value.level, value.sessionId, value.state, value.flags, value.protocol, value.idleMilliseconds].every(Number.isSafeInteger)
    && sessionId > 0 && value.consoleSessionId === sessionId && value.processSessionId === sessionId && value.sessionId === sessionId
    && value.level === 1 && value.state === 0 && value.flags === 1 && value.protocol === 0 && value.idleMilliseconds >= 2000 && value.idleMilliseconds <= 0xffff_ffff
}

export function validateNativeJobReceipt(value: NativeJobReceipt, expected: { job: string; task: string; run: string; action: string; invocation: readonly string[]; interactive: boolean; lockToken: string; ownerPid: number; helperPid: number; spawnedAt: number }): NativeJobReceipt {
  if (!value || value.schema !== "playsrc-native-job-v2"
    || ["job", "task", "run", "action", "lockToken", "ownerPid", "helperPid", "interactive"].some(key => value[key as keyof NativeJobReceipt] !== expected[key as keyof typeof expected])
    || JSON.stringify(value.invocation) !== JSON.stringify(expected.invocation)
    || ![value.ownerCreatedAt, value.helperCreatedAt, value.startedAt, value.finishedAt, value.teardownAt, value.childPid, value.childCreatedAt, value.commandStartedAt].every(Number.isSafeInteger)
    || value.exitCode !== null && !Number.isSafeInteger(value.exitCode) || value.childPid === 0 && value.exitCode !== null
    || value.helperCreatedAt < expected.spawnedAt - 1_000 || value.ownerCreatedAt > value.helperCreatedAt
    || value.startedAt < value.helperCreatedAt || value.finishedAt < value.startedAt || !value.treeEmpty
    || !["completed", "failed", "cancelled", "denied"].includes(value.outcome)) throw new Error("Malformed or mismatched native job receipt")
  if (!Array.isArray(value.desktop) || value.desktop.length > 64 || !value.interactive && value.desktop.length) throw new Error("Background workload acquired desktop ownership")
  if (!Number.isSafeInteger(value.uiInvocations) || value.uiInvocations !== value.desktop.reduce((n, stage) => n + Number(stage.consent !== null) + Number(stage.completion !== null), 0)) throw new Error("Native UI invocation count differs")
  if (value.commandStartedAt && (value.childPid <= 0 || value.childCreatedAt > value.commandStartedAt)) throw new Error("Workload process identity differs")
  let previousEnd = value.commandStartedAt
  const stages = new Set<string>()
  for (const [index, stage] of value.desktop.entries()) {
    if (stage.schema !== "playsrc-native-desktop-v1" || ["job", "task", "run", "lockToken", "childPid", "childCreatedAt", "helperPid", "helperCreatedAt"].some(key => stage[key as keyof NativeDesktopReceipt] !== value[key as keyof NativeJobReceipt])) throw new Error("Desktop stage process identity differs")
    if (!stage.stage || stages.has(stage.stage) || !/^[a-f0-9]{64}$/.test(stage.preparedIdentity) || ![stage.preparedAt, stage.desktopStartedAt, stage.desktopReleasedAt].every(Number.isSafeInteger) || stage.preparedAt < previousEnd) throw new Error("Desktop stages overlap or reuse authorization")
    if (stage.preparedAt > value.teardownAt || stage.desktopStartedAt > value.teardownAt || stage.desktopReleasedAt > value.teardownAt
      || (stage.consent?.dismissedAt ?? 0) > value.finishedAt || (stage.completion?.dismissedAt ?? 0) > value.finishedAt) throw new Error("Desktop stage is outside its owned job lifetime")
    if ((stage.console !== null || stage.succeeded) && !admittedNativeConsole(stage.console, value.sessionId)) throw new Error("Desktop stage lacks genuine native console admission")
    stages.add(stage.stage)
    if (stage.succeeded && (!stage.desktopStartedAt || !stage.desktopReleasedAt)) throw new Error("Successful desktop stage did not finish")
    if (index < value.desktop.length - 1 && !stage.desktopReleasedAt) throw new Error("Prior desktop stage has not ended")
    if (stage.consent?.displayedAt && stage.consent.displayedAt < stage.preparedAt) throw new Error("Consent precedes authenticated preparation")
    if (stage.desktopStartedAt && (!approvedNativeDecision(stage.consent) || stage.desktopStartedAt < stage.consent!.dismissedAt || stage.desktopReleasedAt < stage.desktopStartedAt)) throw new Error("Desktop has no checked scoped approval/release")
    if (stage.completion && (!stage.succeeded || !stage.desktopReleasedAt || !approvedNativeDecision(stage.consent) || stage.completion.displayedAt < stage.desktopReleasedAt)) throw new Error("Notification precedes successful desktop teardown")
    previousEnd = stage.completion?.dismissedAt ?? stage.desktopReleasedAt
  }
  if (value.outcome === "completed" && (!value.commandStartedAt || value.exitCode !== 0 || value.error)) throw new Error("Native completion contradicts workload result")
  if (value.interactive && value.outcome === "completed" && !value.desktop.at(-1)?.desktopReleasedAt) throw new Error("Completed profile has no scoped desktop lifecycle")
  if (value.outcome === "denied" && (value.desktop.at(-1)?.consent?.decision !== "denied" || value.desktop.at(-1)?.desktopStartedAt)) throw new Error("Native denial contradicts workload result")
  if (value.teardownAt < value.commandStartedAt) throw new Error("Completion precedes desktop teardown")
  return value
}

export type NativeJobRequest = {
  job: string; task: string; run: string; action: string; command: string[]; cwd: string
  invocation: readonly string[]
  lockPath: string; lockToken: string; deadline: number
  preflightFailure: string | null
}

/** Rechecked by the native -File entry too: a request cannot relabel a profile
 * or replace a background command with an arbitrary executable/script. */
export async function validateNativeJobRequest(request: NativeJobRequest) {
  const plan = localJobCommand(request.invocation)
  const command = plan.controller
    ? [process.execPath, path.join(repositoryRoot, plan.command[0]!), "--application-root", request.cwd, ...plan.command.slice(1)]
    : [process.execPath, ...plan.command]
  if (JSON.stringify(request.command) !== JSON.stringify(command)) throw new Error("Native command differs from validated workload")
  return plan
}

export async function runWindowsNativeJob(request: NativeJobRequest, env: NodeJS.ProcessEnv, verifySource: () => Promise<void>): Promise<NativeJobReceipt> {
  const plan = await validateNativeJobRequest(request)
  const file = path.join(request.run, "native-request.json")
  await writeFile(file, JSON.stringify({ ...request, ownerPid: process.pid,
    manifest: path.join(repositoryRoot, "tools/playsrc/windows-job-native.manifest") }), { flag: "wx" })
  const spawnedAt = Date.now()
  const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-File", path.join(repositoryRoot, "tools/playsrc/windows-job-native.ps1"), "-Request", file], {
    env, windowsHide: true, stdio: ["pipe", "pipe", "pipe"],
  })
  let output = "", diagnostic = "", failure: Error | undefined
  let verification: Promise<void> | undefined
  const timer = setTimeout(() => { failure = new Error("Native job helper exceeded its total deadline; never default approved"); child.kill() }, Math.max(1, request.deadline - Date.now()))
  const cancel = () => { void writeFile(path.join(request.run, "cancel"), "controller cancellation\n").catch(error => { failure = error; child.kill() }) }
  process.once("SIGINT", cancel); process.once("SIGTERM", cancel)
  child.stdin.on("error", error => { failure = error })
  child.stdout.on("data", bytes => {
    output += bytes
    if (output.length > 128 * 1024) { failure = new Error("Native receipt exceeds bound"); child.kill(); return }
    if (!verification && output.includes("\n")) {
      const end = output.indexOf("\n"), line = output.slice(0, end); output = output.slice(end + 1)
      verification = (async () => {
        let error: string | null = null
        try {
          const ready = JSON.parse(line)
          if (ready.phase !== "teardown" || ready.job !== request.job || ready.run !== request.run || ready.helperPid !== child.pid || !ready.treeEmpty) throw new Error("Native teardown handoff differs")
          await verifySource()
        } catch (cause) { error = String(cause) }
        child.stdin.end(JSON.stringify({ error }) + "\n")
      })()
    }
  })
  child.stderr.on("data", bytes => { diagnostic = (diagnostic + bytes).slice(-16_384) })
  child.once("error", error => { failure = error })
  try {
    const code = await new Promise<number | null>(resolve => child.once("close", resolve))
    await verification
    await writeFile(path.join(request.run, "native-helper.json"), JSON.stringify({ pid: child.pid ?? null, spawnedAt, code, failure: failure?.message ?? null, diagnostic, output }), { flag: "wx" })
    if (failure || code !== 0) throw failure ?? new Error(`Native job helper failed (${code}): ${diagnostic}`)
    const retained = JSON.parse(await readFile(path.join(request.run, "native-result.json"), "utf8"))
    if (JSON.stringify(retained) !== JSON.stringify(JSON.parse(output.trim()))) throw new Error("Native helper output differs from retained receipt")
    return validateNativeJobReceipt(retained, { ...request, interactive: plan.interactive, ownerPid: process.pid, helperPid: child.pid!, spawnedAt })
  } finally { clearTimeout(timer); process.off("SIGINT", cancel); process.off("SIGTERM", cancel) }
}

/** Only the actual child of the live native supervisor may borrow the
 * outer job lock. A token/--ready supplied by a caller is not authorization. */
export async function borrowedWindowsJobLock(lockPath: string, invocation: readonly string[] | { testFile: string }) {
  const file = process.env.PLAYSRC_LOCAL_JOB_OWNER
  if (!file) return null
  if (process.platform !== "win32" || process.env.PLAYSRC_LOCAL_JOB_LOCK !== lockPath) throw new Error("Native job lock path differs")
  const consent = JSON.parse(await readFile(file, "utf8")) as NativeJobReceipt
  const held = JSON.parse(await readFile(lockPath, "utf8")) as { pid: number; token: string }
  // Bun exposes the current test file as argv[1], not the original suite argv.
  // A native regression test may borrow only if it belongs to the validated
  // test selection (or the explicitly selected whole suite).
  const invocationMatches = "testFile" in invocation
    ? consent.invocation?.[0] === "test" && !path.relative(process.cwd(), invocation.testFile).startsWith("..")
      && (consent.invocation.length === 1 || consent.invocation.slice(1).some(file => path.resolve(file) === path.resolve(invocation.testFile)))
    : JSON.stringify(consent.invocation) === JSON.stringify(invocation)
  const plan = localJobCommand(consent.invocation)
  if (consent.schema !== "playsrc-native-job-v2" || !invocationMatches || consent.helperPid !== process.ppid
    || path.join(consent.run, "ownership.json") !== file || held.pid !== consent.ownerPid || held.token !== consent.lockToken
    || !processIsAlive(held.pid) || consent.interactive !== plan.interactive
    || !Array.isArray(consent.desktop) || consent.desktop.length || consent.uiInvocations !== 0) throw new Error("No live per-job native classification/ownership")
  // Creation-time readback defeats recycled helper or lock-owner PIDs.
  const script = `$ErrorActionPreference='Stop';@(${consent.ownerPid},${consent.helperPid})|ForEach-Object {$p=[Diagnostics.Process]::GetProcessById($_);@{pid=$p.Id;created=([DateTimeOffset]$p.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();session=$p.SessionId};$p.Dispose()}|ConvertTo-Json -Compress`
  const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")], { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] })
  let output = "", failure: Error | undefined
  child.stdout.on("data", bytes => { output += bytes; if (output.length > 8192) child.kill() })
  child.stderr.resume(); child.once("error", error => { failure = error })
  const timer = setTimeout(() => child.kill(), 5_000)
  try {
    const code = await new Promise(resolve => child.once("close", resolve))
    if (failure || code !== 0) throw new Error("Native job creation-time readback failed")
    const identities = JSON.parse(output)
    if (!Array.isArray(identities) || identities.length !== 2 || identities[0].pid !== consent.ownerPid || identities[0].created !== consent.ownerCreatedAt
      || identities[1].pid !== consent.helperPid || identities[1].created !== consent.helperCreatedAt || identities.some(value => value.session !== consent.sessionId)) throw new Error("Native job process identity changed")
  } finally { clearTimeout(timer) }
  const deadline = Number(process.env.PLAYSRC_LOCAL_JOB_DEADLINE)
  if (!Number.isSafeInteger(deadline) || deadline <= Date.now() || deadline > consent.startedAt + 175_000) throw new Error("Native job deadline differs")
  return { token: held.token, milliseconds: 0, deadline, ownerPid: held.pid }
}

/** The profile owner calls this only after its silent preparation. The native
 * supervisor retains the SAME FIFO resource reservation and process tree across
 * the transition; ownership.json is never a desktop authorization. */
let desktopSequence = 0
export async function withWindowsDesktop<T>(prepared: Readonly<Record<string, unknown>>, signal: AbortSignal,
  verify: () => Promise<void>, work: (release: (succeeded: boolean) => Promise<NativeDesktopReceipt | undefined>, grant: NativeDesktopReceipt | undefined) => Promise<T>, teardown: () => Promise<void>, succeeded: (result: T) => boolean): Promise<T> {
  if (process.platform !== "win32") return work(async () => undefined, undefined)
  const file = process.env.PLAYSRC_LOCAL_JOB_OWNER
  if (!file) throw new Error("Missing prepared native desktop identity")
  const owner = JSON.parse(await readFile(file, "utf8")) as NativeJobReceipt
  await borrowedWindowsJobLock(process.env.PLAYSRC_LOCAL_JOB_LOCK!, owner.invocation)
  if (!owner.interactive) throw new Error("Background ownership cannot authorize a desktop")
  const dispatch = JSON.parse(await readFile(path.join(owner.run, "dispatch.json"), "utf8"))
  if (dispatch.pid !== process.pid || dispatch.helperPid !== process.ppid || dispatch.helperCreatedAt !== owner.helperCreatedAt
    || dispatch.job !== owner.job || dispatch.task !== owner.task || dispatch.run !== owner.run) throw new Error("Desktop requester is not the owned profile process")
  const preparedBytes = JSON.stringify(prepared)
  const preparedIdentity = createHash("sha256").update(preparedBytes).digest("hex")
  const directory = path.join(owner.run, "desktop", String(desktopSequence++).padStart(4, "0"))
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "prepared.json"), preparedBytes, { flag: "wx" })
  const request = { job: owner.job, task: owner.task, run: owner.run, lockToken: owner.lockToken,
    stage: randomUUID(), preparedIdentity, childPid: process.pid, childCreatedAt: dispatch.createdAt,
    helperPid: owner.helperPid, helperCreatedAt: owner.helperCreatedAt }
  const publish = async (name: string, value: unknown) => {
    const destination = path.join(directory, name)
    await writeFile(`${destination}.tmp`, JSON.stringify(value), { flag: "wx" })
    await rename(`${destination}.tmp`, destination)
  }
  const wait = async (name: string, cancelling: boolean) => {
    while (Date.now() < Number(process.env.PLAYSRC_LOCAL_JOB_DEADLINE)) {
      if (cancelling) signal.throwIfAborted()
      if (!processIsAlive(owner.helperPid)) throw new Error("Desktop supervisor exited")
      const value = await readFile(path.join(directory, name), "utf8").catch(error => {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
        return null
      })
      if (value) {
        const receipt = JSON.parse(value) as NativeDesktopReceipt
        for (const key of ["job", "task", "run", "lockToken", "stage", "preparedIdentity", "childPid", "childCreatedAt", "helperPid", "helperCreatedAt"] as const) {
          if (receipt[key] !== request[key]) throw new Error("Desktop stage receipt differs")
        }
        if (receipt.schema !== "playsrc-native-desktop-v1" || !approvedNativeDecision(receipt.consent) || !receipt.desktopStartedAt
          || !admittedNativeConsole(receipt.console, owner.sessionId)) throw new Error("Desktop stage was not authorized")
        return receipt
      }
      await Bun.sleep(25)
    }
    throw new Error("Desktop transition exceeded the unchanged job deadline")
  }
  signal.throwIfAborted()
  await verify()
  signal.throwIfAborted()
  await publish("request.json", request)
  const grant = await wait("grant.json", true)
  let releasing: Promise<NativeDesktopReceipt> | undefined
  const release = (success: boolean) => releasing ??= (async () => {
    // No release receipt on unconfirmed teardown. The native Job Object's
    // kill-on-close fallback owns that failure, not an optimistic lease expiry.
    await teardown()
    await publish("release.json", { ...request, succeeded: success && !signal.aborted })
    const released = await wait("released.json", false)
    if (released.desktopReleasedAt < released.desktopStartedAt) throw new Error("Desktop release was not observed")
    return released
  })()
  let success = false
  try { signal.throwIfAborted(); await verify(); signal.throwIfAborted(); const result = await work(release, grant); success = succeeded(result); return result }
  finally { await release(success) }
}
