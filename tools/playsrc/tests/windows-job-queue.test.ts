import { expect, test } from "bun:test"
import { readFile, readdir, mkdir, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { localJobEnvironment, readLocalTaskResult } from "../src/local-job"

// Explicit fixture identity, not an authorization switch. Run from the silent
// transport with an existing prepared job; never from inside an owned job lock.
test.skipIf(process.platform !== "win32" || !process.env.SSH_CONNECTION || !process.env.PLAYSRC_TEST_JOB_ID)("direct caller queues silently and standard task cancellation removes only its FIFO ticket", async () => {
  const id = process.env.PLAYSRC_TEST_JOB_ID!
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "local-jobs", id)
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const lock = await acquireHeadedProfileLock(lockPath, "queued-task-cancellation-regression", 10_000)
  const execute = async (command: string[]) => {
    const child = Bun.spawn(command, { cwd: repositoryRoot, env: localJobEnvironment(process.env), windowsHide: true, stdout: "pipe", stderr: "pipe" })
    const timer = setTimeout(() => child.kill(), 10_000)
    try {
      const [output, error, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
      expect({ code, error }).toEqual({ code: 0, error: "" })
      return JSON.parse(output)
    } finally { clearTimeout(timer) }
  }
  try {
    const { task } = await execute([process.execPath, "tools/playsrc/src/local-job.ts", "run", id, "diagnostic", "250", "0"])
    const link = path.join(directory, `${task.slice("playsrc-local-job-".length)}-run.json`)
    const deadline = Date.now() + 10_000
    let run: string | undefined
    while (!run && Date.now() < deadline) {
      run = await readFile(link, "utf8").then(text => JSON.parse(text).run).catch(error => { if (error.code !== "ENOENT") throw error; return undefined })
      if (!run) await Bun.sleep(25)
    }
    expect(run).toBeDefined()
    const queuedAt = Date.now()
    await Bun.sleep(300)
    const before = await readdir(run!)
    expect(before).not.toContain("native-request.json")
    expect(before).not.toContain("desktop")
    await execute(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", "tools/playsrc/windows-job.ps1", "-Action", "Cancel", "-Job", id, "-Task", task])
    let result: any
    while (!result && Date.now() < deadline) {
      result = (await readLocalTaskResult(directory, task)).result
      if (!result) await Bun.sleep(25)
    }
    expect(result).toMatchObject({ task, outcome: "cancelled", native: null, run })
    expect(result.lockWaitMilliseconds).toBeGreaterThanOrEqual(300)
    expect(JSON.parse(await readFile(lockPath, "utf8"))).toMatchObject({ pid: process.pid, token: lock.token })
    const evidence = path.join(config.sourceCacheDir, "evidence/windows-job-queue-tests", randomUUID())
    await mkdir(evidence, { recursive: true })
    await writeFile(path.join(evidence, "result.json"), JSON.stringify({ queuedAt, checkedAt: Date.now(), before, result }, null, 2))
    console.log(`Actual queued cancellation: ${evidence}`)
  } finally { await releaseHeadedProfileLock(lockPath, lock.token) }
}, 40_000)
