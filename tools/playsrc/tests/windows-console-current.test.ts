import { expect, test } from "bun:test"
import path from "node:path"
import os from "node:os"
import { loadLocalConfig } from "../src/config"
import { borrowedWindowsJobLock } from "../src/windows-job-native"
import { queryWindowsDesktop, assertWindowsConsole } from "../profile/windows-desktop"

// Read-only diagnosis under the real per-job decision. This never admits a
// browser, creates input, changes focus, or treats an idle result as reusable.
test.skipIf(process.platform !== "win32" || !process.env.PLAYSRC_LOCAL_JOB_CONSENT)("current native console query returns bounded fresh session and idle facts", async () => {
  const { sourceCacheDir } = await loadLocalConfig()
  const lock = await borrowedWindowsJobLock(path.join(sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock"), { testFile: import.meta.path })
  expect(lock).not.toBeNull()
  for (let index = 0; index < 2; index++) {
    const startedAt = Date.now()
    try {
      const state = await queryWindowsDesktop(10_000)
      console.log(JSON.stringify({ purpose: "console-read-only-not-profile-admission", index, startedAt, finishedAt: Date.now(), state }))
      assertWindowsConsole(state, os.release())
    } catch (error) {
      console.log(JSON.stringify({ purpose: "console-read-only-not-profile-admission", index, startedAt, finishedAt: Date.now(), error: String(error) }))
      throw error
    }
    if (index === 0) await new Promise(resolve => setTimeout(resolve, 2000))
  }
}, 25_000)
