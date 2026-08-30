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
test.skipIf(process.platform !== "win32" || !process.env.SSH_CONNECTION)("session-zero helper may prepare but refuses desktop admission under the real FIFO lock", async () => {
  const desktop = await queryWindowsDesktop()
  expect(desktop.processSessionId).toBe(0)
  const config = await loadLocalConfig()
  const run = path.join(config.sourceCacheDir, "evidence/windows-job-ui-tests", randomUUID())
  await mkdir(run, { recursive: true })
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const lock = await acquireHeadedProfileLock(lockPath, "session-zero-rejection-test", 10_000)
  try {
    const result = await runWindowsNativeJob({ job: randomUUID(), task: `playsrc-local-job-${randomUUID()}`, run,
      action: "profile runner-handoff session-zero rejection", invocation: ["profile", "runner-handoff"],
      command: [process.execPath, path.join(repositoryRoot, "tools/playsrc/src/profile-runner.ts"), "--application-root", repositoryRoot, "runner-handoff"], cwd: repositoryRoot,
      lockPath, lockToken: lock.token, deadline: Date.now() + 60_000, preflightFailure: null }, process.env, async () => {})
    console.log(`Actual session-zero rejection receipt: ${path.join(run, "native-result.json")}`)
    expect(result.outcome).toBe("failed")
    expect(result.error).toContain("matching physical console")
    expect(result.childPid).toBeGreaterThan(0)
    expect(result.desktop[0]!.preparedAt).toBeGreaterThan(result.commandStartedAt)
    expect(result.desktop[0]!.desktopStartedAt).toBe(0)
    expect(result.desktop[0]!.consent).toBeNull()
    expect(result.desktop[0]!.completion).toBeNull()
    expect(result.treeEmpty).toBe(true)
    expect(result.uiInvocations).toBe(0)
  } finally { await releaseHeadedProfileLock(lockPath, lock.token) }
}, 70_000)
