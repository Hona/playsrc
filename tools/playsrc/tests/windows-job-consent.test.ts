import { expect, test } from "bun:test"
import { readFile, mkdtemp, writeFile, rm } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { approvedNativeDecision, validateNativeJobReceipt, validateNativeJobRequest, type NativeDialog, type NativeJobReceipt } from "../src/windows-job-native"
import { localJobCommand } from "../src/local-job-command"
import { repositoryRoot } from "../src/config"

// Synthetic receipt tests, NOT evidence of displayed UI or task authorization.
const consent: NativeDialog = { decision: "approved", error: null, displayedAt: 2000, decidedAt: 2400, dismissedAt: 2401, visibleMilliseconds: 400, window: 1, sessionId: 3 }
const expected = { job: "job", task: "task", run: "run", action: "profile gameplay", invocation: ["profile", "gameplay"], interactive: true, lockToken: "lock", ownerPid: 10, helperPid: 11, spawnedAt: 1000 }
const receipt: NativeJobReceipt = { ...expected, schema: "playsrc-native-job-v2", ownerCreatedAt: 500, helperCreatedAt: 1000, sessionId: 3,
  stage: "stage", preparedIdentity: "a".repeat(64), preparedAt: 1900, desktopStartedAt: 2500, desktopReleasedAt: 2800,
  childPid: 12, childCreatedAt: 1500, commandStartedAt: 1501, teardownAt: 6400, startedAt: 1100, finishedAt: 6500,
  outcome: "completed", exitCode: 0, treeEmpty: true, error: null, uiInvocations: 2, consent, completion: { ...consent, decision: "dismissed-timeout", displayedAt: 3000, decidedAt: 6000, dismissedAt: 6001, visibleMilliseconds: 3000 } }

test("only a displayed, dismissed native approval authorizes dispatch; timeout is not display failure", () => {
  expect(approvedNativeDecision(consent)).toBe(true)
  expect(approvedNativeDecision({ ...consent, decision: "approved-timeout", visibleMilliseconds: 3000, decidedAt: 5000, dismissedAt: 5001 })).toBe(true)
  for (const value of [null, { ...consent, decision: "denied" }, { ...consent, decision: "display-failed" }, { ...consent, error: "locked" },
    { ...consent, window: 0 }, { ...consent, sessionId: 0 }, { ...consent, displayedAt: 0 }, { ...consent, decision: "approved-timeout" },
    { ...consent, dismissedAt: 0 }, { ...consent, visibleMilliseconds: -1 }]) expect(approvedNativeDecision(value)).toBe(false)
})

test("receipt binds job/task/run/creation-time/lock and teardown; malformed or stale output never launches", () => {
  expect(validateNativeJobReceipt(receipt, expected)).toEqual(receipt)
  for (const changed of [{ task: "other" }, { job: "other" }, { run: "other" }, { helperPid: 44 }, { ownerPid: 44 }, { lockToken: "other" },
    { helperCreatedAt: -1 }, { treeEmpty: false }, { outcome: "denied" }, { exitCode: 1 }, { commandStartedAt: 0 },
    { preparedAt: 2300 }, { desktopStartedAt: 2300 }, { desktopReleasedAt: 0 }, { invocation: ["diagnostic 250", "0"] }, { consent: { ...consent, decision: "denied" } }, { teardownAt: 1400 }]) {
    expect(() => validateNativeJobReceipt({ ...receipt, ...changed } as NativeJobReceipt, expected)).toThrow()
  }
  for (const outcome of ["failed", "cancelled", "denied"] as const) {
    const value = { ...receipt, outcome, desktopStartedAt: 0, desktopReleasedAt: 0, exitCode: 1, consent: { ...consent, decision: "denied" }, uiInvocations: 1, completion: null, error: "not launched" }
    expect(validateNativeJobReceipt(value, expected).outcome).toBe(outcome)
  }
})

test("only validated interactive workloads use consent; readback and background never create dialogs", async () => {
  const bridge = await readFile(path.resolve(import.meta.dir, "../windows-job.ps1"), "utf8")
  const native = await readFile(path.resolve(import.meta.dir, "../windows-job-native.cs"), "utf8")
  expect(bridge).not.toMatch(/\[switch\]\$Ready|--ready|-Ready\b/)
  expect(bridge).toContain("'Test','Diagnostic','Cancel'")
  expect(bridge).toContain("--task $(Quote $name)")
  expect(bridge).toContain("plan @workload")
  expect(bridge).toContain("if($plan.interactive){'Interactive'}else{'S4U'}")
  expect(native).toContain('DesktopTransition(request,receipt,owner,job)')
  expect(native).toContain('receipt.preparedAt=Now')
  expect(native).toContain("UpdateProcThreadAttribute(attributes,0,new UIntPtr(0x2000d)")
  expect(native).not.toContain("AssignProcessToJobObject")
  expect(native).toContain("clock.ElapsedMilliseconds>=3000")
  expect(native).toContain("wparam.ToInt32()==100?\"approved\":\"denied\"")
  expect(native).toContain('Check(SetWindowPos(window,new IntPtr(-1),0,0,0,0,0x13),"Present requested message box")')
  expect(native).toContain("GetAncestor(WindowFromPoint")
  expect(native).not.toMatch(/BringWindowToTop|BlockInput|SendInput|SwitchDesktop/)
  expect(localJobCommand(["diagnostic", "250", "0"]).command[0]).toBe("tools/playsrc/src/local-job-diagnostic.ts")
  for (const args of [["--ready", "test"], ["diagnostic", "30001", "0"], ["diagnostic", "1", "2"]]) expect(() => localJobCommand(args)).toThrow()
})

test("background receipts require ZERO UI invocations for every outcome", () => {
  const background = { ...expected, interactive: false, invocation: ["diagnostic", "0", "0"] }
  for (const outcome of ["completed", "failed", "cancelled"] as const) {
    const value = { ...receipt, ...background, outcome, stage: null, preparedIdentity: null, preparedAt: 0, desktopStartedAt: 0, desktopReleasedAt: 0, consent: null, completion: null, uiInvocations: 0, exitCode: outcome === "completed" ? 0 : 1 }
    expect(validateNativeJobReceipt(value, background)).toEqual(value)
    for (const patch of [{ consent }, { completion: receipt.completion }, { uiInvocations: 1 }, { interactive: true }]) {
      expect(() => validateNativeJobReceipt({ ...value, ...patch }, background)).toThrow()
    }
    if (outcome !== "completed") expect(validateNativeJobReceipt({ ...value, commandStartedAt: 0, childPid: 0, childCreatedAt: 0, exitCode: null }, background).uiInvocations).toBe(0)
  }
})

test("direct native entry cannot relabel profiles, substitute commands or accept unknown work", async () => {
  for (const invocation of [["test"], ["build", "jump_beef"], ["build-stage", "wasm"], ["build-stage", "producer"], ["build-stage", "resources", "jump_beef"], ["diagnostic", "0", "0"], ["profile", "gameplay"]]) {
    const plan = localJobCommand(invocation)
    const command = plan.interactive ? [process.execPath, path.join(repositoryRoot, "tools/playsrc/src/profile-runner.ts"), "--application-root", repositoryRoot, ...plan.command.slice(1)] : [process.execPath, ...plan.command]
    const request = { job: "job", task: "task", run: "run", action: "not authority", invocation, command, cwd: repositoryRoot, lockPath: "lock", lockToken: "token", deadline: 100, preflightFailure: null, interactive: !plan.interactive }
    expect((await validateNativeJobRequest(request)).interactive).toBe(invocation[0] === "profile")
    await expect(validateNativeJobRequest({ ...request, command: [process.execPath, "-e", "throw 1"] })).rejects.toThrow("differs")
    await expect(validateNativeJobRequest({ ...request, invocation: ["unknown"] })).rejects.toThrow()
  }
})

test("classification and native validation CLI entrypoints finish without importing their own pending main", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-job-plan-"))
  try {
    const file = path.join(directory, "request.json")
    await writeFile(file, JSON.stringify({ invocation: ["diagnostic", "0", "0"], command: [process.execPath, "tools/playsrc/src/local-job-diagnostic.ts", "0", "0"], cwd: repositoryRoot }))
    for (const args of [["plan", "diagnostic", "0", "0"], ["validate-native", file]]) {
      const child = Bun.spawn([process.execPath, path.join(repositoryRoot, "tools/playsrc/src/local-job.ts"), ...args], { stdout: "pipe", stderr: "pipe", windowsHide: true })
      const timer = setTimeout(() => child.kill(), 3000)
      try {
        const [output, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
        expect({ code, error }).toEqual({ code: 0, error: "" })
        expect(JSON.parse(output).interactive).toBe(false)
      } finally { clearTimeout(timer) }
    }
  } finally { await rm(directory, { recursive: true, force: true }) }
}, 10_000)

test.skipIf(process.platform !== "win32")("the actual Windows PowerShell bridge keeps zero/one/many arguments flat and rejects malformed arrays", async () => {
  const child = Bun.spawn(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", path.join(import.meta.dir, "fixtures/windows-job-arguments.ps1"), "-Bridge", path.resolve(import.meta.dir, "../windows-job.ps1")], { windowsHide: true, stdout: "pipe", stderr: "pipe" })
  const [output, errors, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
  expect({ code, errors }).toEqual({ code: 0, errors: "" })
  expect(JSON.parse(output)).toEqual({ many: ["test", "a.test.ts", "b.test.ts"], emptyCount: 0, one: ["a.test.ts"], rejected: 4 })
})
