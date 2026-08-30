import { spawn } from "node:child_process"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { repositoryRoot } from "./config"
import { processIsAlive } from "./profile-lock"

export type NativeDialog = {
  decision: string; error: string | null; displayedAt: number; decidedAt: number; dismissedAt: number
  visibleMilliseconds: number; window: number; sessionId: number
}
export type NativeJobReceipt = {
  schema: "playsrc-native-job-v1"; job: string; task: string; run: string; action: string; lockToken: string
  invocation: readonly string[]; interactive: boolean; uiInvocations: number
  ownerPid: number; ownerCreatedAt: number; helperPid: number; helperCreatedAt: number; sessionId: number
  childPid: number; childCreatedAt: number; commandStartedAt: number; teardownAt: number
  startedAt: number; finishedAt: number; outcome: "completed" | "failed" | "cancelled" | "denied"
  exitCode: number | null; treeEmpty: boolean; error: string | null; consent: NativeDialog | null; completion: NativeDialog | null
}

export function approvedNativeDecision(value: NativeDialog | null): boolean {
  if (!value || value.error || ![value.displayedAt, value.decidedAt, value.dismissedAt, value.visibleMilliseconds, value.window, value.sessionId].every(Number.isSafeInteger)
    || value.window <= 0 || value.sessionId <= 0 || value.displayedAt <= 0 || value.decidedAt < value.displayedAt || value.dismissedAt < value.decidedAt || value.visibleMilliseconds < 0) return false
  return value.decision === "approved" || value.decision === "approved-timeout" && value.visibleMilliseconds >= 3_000 && value.decidedAt - value.displayedAt >= 3_000
}

export function validateNativeJobReceipt(value: NativeJobReceipt, expected: { job: string; task: string; run: string; action: string; invocation: readonly string[]; interactive: boolean; lockToken: string; ownerPid: number; helperPid: number; spawnedAt: number }): NativeJobReceipt {
  if (!value || value.schema !== "playsrc-native-job-v1"
    || ["job", "task", "run", "action", "lockToken", "ownerPid", "helperPid", "interactive"].some(key => value[key as keyof NativeJobReceipt] !== expected[key as keyof typeof expected])
    || JSON.stringify(value.invocation) !== JSON.stringify(expected.invocation)
    || ![value.ownerCreatedAt, value.helperCreatedAt, value.startedAt, value.finishedAt, value.teardownAt, value.childPid, value.childCreatedAt, value.commandStartedAt].every(Number.isSafeInteger)
    || value.exitCode !== null && !Number.isSafeInteger(value.exitCode) || value.childPid === 0 && value.exitCode !== null
    || value.helperCreatedAt < expected.spawnedAt - 1_000 || value.ownerCreatedAt > value.helperCreatedAt
    || value.startedAt < value.helperCreatedAt || value.finishedAt < value.startedAt || !value.treeEmpty
    || !["completed", "failed", "cancelled", "denied"].includes(value.outcome)) throw new Error("Malformed or mismatched native job receipt")
  if (!value.interactive && (value.consent !== null || value.completion !== null || value.uiInvocations !== 0)) throw new Error("Background workload invoked UI")
  if (!Number.isSafeInteger(value.uiInvocations) || value.uiInvocations < 0 || value.uiInvocations > 2
    || value.uiInvocations !== Number(value.consent !== null) + Number(value.completion !== null)) throw new Error("Native UI invocation count differs")
  if (value.commandStartedAt && (value.childPid <= 0 || value.childCreatedAt > value.commandStartedAt)) throw new Error("Workload process identity differs")
  if (value.interactive && value.commandStartedAt && (!approvedNativeDecision(value.consent) || value.commandStartedAt < value.consent!.dismissedAt)) throw new Error("Workload has no checked displayed approval")
  if (value.completion && (!value.interactive || value.outcome !== "completed" || !value.commandStartedAt || !approvedNativeDecision(value.consent))) throw new Error("Notification without a completed interactive workload")
  if (value.outcome === "completed" && (!value.commandStartedAt || value.exitCode !== 0 || value.error)) throw new Error("Native completion contradicts workload result")
  if (value.outcome === "denied" && (value.consent?.decision !== "denied" || value.commandStartedAt)) throw new Error("Native denial contradicts workload result")
  if (value.teardownAt < value.commandStartedAt || value.completion?.displayedAt && value.completion.displayedAt < value.teardownAt) throw new Error("Completion precedes owned teardown")
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
  const { localJobCommand } = await import("./local-job")
  const plan = localJobCommand(request.invocation)
  const command = plan.interactive
    ? [process.execPath, path.join(repositoryRoot, "tools/playsrc/src/profile-runner.ts"), "--application-root", request.cwd, ...plan.command.slice(1)]
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
  const { localJobCommand } = await import("./local-job")
  const plan = localJobCommand(consent.invocation)
  if (consent.schema !== "playsrc-native-job-v1" || !invocationMatches || consent.helperPid !== process.ppid
    || path.join(consent.run, "ownership.json") !== file || held.pid !== consent.ownerPid || held.token !== consent.lockToken
    || !processIsAlive(held.pid) || consent.interactive !== plan.interactive
    || (plan.interactive ? !approvedNativeDecision(consent.consent) : consent.consent !== null || consent.uiInvocations !== 0)) throw new Error("No live per-job native classification/ownership")
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
