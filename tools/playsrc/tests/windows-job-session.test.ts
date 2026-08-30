import { expect, test } from "bun:test"
import { mkdir } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { queryWindowsDesktop } from "../profile/windows-desktop"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { runWindowsNativeJob } from "../src/windows-job-native"

// An actual rejection probe, deliberately run from SSH/session 0. Interactive
// scheduled tests skip it. No desktop is locked and no fake receipt is supplied.
test.skipIf(process.platform !== "win32" || !process.env.SSH_CONNECTION)("session-zero helper refuses UI and dispatch under the real FIFO lock", async () => {
  const desktop = await queryWindowsDesktop()
  expect(desktop.processSessionId).toBe(0)
  const config = await loadLocalConfig()
  const run = path.join(config.sourceCacheDir, "evidence/windows-job-ui-tests", randomUUID())
  await mkdir(run, { recursive: true })
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const lock = await acquireHeadedProfileLock(lockPath, "session-zero-rejection-test", 10_000)
  try {
    const result = await runWindowsNativeJob({ job: randomUUID(), task: `playsrc-local-job-${randomUUID()}`, run,
      action: "diagnostic session-zero rejection", invocation: ["diagnostic", "0", "1"],
      command: [process.execPath, "-e", "throw Error('A session-zero diagnostic must never dispatch')"], cwd: repositoryRoot,
      lockPath, lockToken: lock.token, deadline: Date.now() + 15_000, diagnostic: true, preflightFailure: null }, process.env, async () => {})
    expect(result.outcome).toBe("failed")
    expect(result.error).toContain("matching physical console")
    expect(result.childPid).toBe(0)
    expect(result.consent).toBeNull()
    expect(result.completion).toBeNull()
    expect(result.treeEmpty).toBe(true)
    console.log(`Actual session-zero rejection receipt: ${path.join(run, "native-result.json")}`)
  } finally { await releaseHeadedProfileLock(lockPath, lock.token) }
}, 30_000)
