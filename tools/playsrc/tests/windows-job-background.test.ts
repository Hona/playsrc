import { expect, test } from "bun:test"
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { localJobEnvironment } from "../src/local-job"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { runWindowsNativeJob } from "../src/windows-job-native"

// Run directly from the noninteractive transport, not as an already locked job.
// No desktop/session/foreground APIs and no browser. The production supervisor,
// diagnostic, process creation, cancellation and kill-on-close tree are real.
test.skipIf(process.platform !== "win32" || !process.env.SSH_CONNECTION || !!process.env.PLAYSRC_LOCAL_JOB_OWNER)("ordinary native background success/failure/cancellation/preflight/helper crash are silent and preserve outcomes", async () => {
  const config = await loadLocalConfig()
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const lock = await acquireHeadedProfileLock(lockPath, "background-native-regression", 15_000)
  try {
    for (const kind of ["success", "failure", "cancel", "preflight", "crash"] as const) {
      const run = path.join(config.sourceCacheDir, "evidence/windows-job-background-tests", randomUUID())
      await mkdir(run, { recursive: true })
      const invocation = ["diagnostic", kind === "cancel" || kind === "crash" ? "30000" : "0", kind === "failure" ? "1" : "0"]
      const result = runWindowsNativeJob({ job: randomUUID(), task: `playsrc-local-job-${randomUUID()}`, run, action: invocation.join(" "), invocation,
        command: [process.execPath, "tools/playsrc/src/local-job-diagnostic.ts", ...invocation.slice(1)], cwd: repositoryRoot,
        lockPath, lockToken: lock.token, deadline: Date.now() + 20_000, preflightFailure: kind === "preflight" ? "missing content" : null }, localJobEnvironment(process.env), async () => {})
      // Observe rejection immediately; avoid an unhandled rejection while waiting
      // for a creation-bound dispatch record in the termination cases.
      const observed = result.then(value => ({ value, error: null }), error => ({ value: null, error }))
      if (kind === "cancel" || kind === "crash") {
        const deadline = Date.now() + 10_000
        let log = ""
        while (Date.now() < deadline && !log.includes("native diagnostic workload")) {
          log = await readFile(path.join(run, "command.log"), "utf8").catch(() => "")
          await Bun.sleep(25)
        }
        expect(log).toContain("native diagnostic workload")
        if (kind === "cancel") await writeFile(path.join(run, "cancel"), "test cancellation")
        else {
          const dispatch = JSON.parse(await readFile(path.join(run, "dispatch.json"), "utf8"))
          const script = `$p=[Diagnostics.Process]::GetProcessById(${dispatch.helperPid});if(([DateTimeOffset]$p.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds() -ne ${dispatch.helperCreatedAt}){throw 'Helper identity changed'};$p.Kill();$p.WaitForExit(2000)`
          const kill = Bun.spawn(["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")], { windowsHide: true, stdout: "ignore", stderr: "pipe" })
          expect(await kill.exited).toBe(0)
        }
      }
      const { value, error } = await observed
      if (kind === "crash") expect(String(error)).toContain("Native job helper failed")
      else {
        expect(error).toBeNull()
        expect(value!.outcome).toBe(kind === "success" ? "completed" : kind === "cancel" ? "cancelled" : "failed")
        // Job Object termination can finish before the root handle is signaled;
        // retain an unobserved exit as null, never invent a cancellation code.
        if (kind === "cancel") expect([null, 1]).toContain(value!.exitCode)
        else expect(value!.exitCode).toBe(kind === "success" ? 0 : kind === "preflight" ? null : 1)
        expect(value!.interactive).toBe(false)
        expect(value!.uiInvocations).toBe(0)
        expect(value!.desktop).toEqual([]); expect(value!.treeEmpty).toBe(true)
      }
      expect((await readdir(run)).filter(file => /consent|completion|failure-native/.test(file))).toEqual([])
      const dispatch = await readFile(path.join(run, "dispatch.json"), "utf8").then(JSON.parse).catch(() => null)
      if (dispatch) {
        const script = `$ErrorActionPreference='Stop';try{$p=[Diagnostics.Process]::GetProcessById(${dispatch.pid});if(([DateTimeOffset]$p.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds() -eq ${dispatch.createdAt} -and !$p.WaitForExit(2000)){throw 'Owned child survived'}}catch [ArgumentException]{};exit 0`
        const check = Bun.spawn(["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")], { windowsHide: true, stdout: "ignore", stderr: "pipe" })
        const [errors, code] = await Promise.all([new Response(check.stderr).text(), check.exited])
        expect({ code, errors }).toEqual({ code: 0, errors: "" })
      }
      console.log(`Background ${kind}: ${run}`)
    }
  } finally { await releaseHeadedProfileLock(lockPath, lock.token) }
}, 120_000)
