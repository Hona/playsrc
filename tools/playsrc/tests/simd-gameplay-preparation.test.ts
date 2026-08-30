import { expect, test } from "bun:test"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { rustEnvironment } from "../src/setup"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { borrowedWindowsJobLock } from "../src/windows-job-native"
import toolchains from "../toolchains.json"

test.skipIf(!process.env.PLAYSRC_LOCAL_JOB_OWNER && process.env.RUN_CONFIGURED_SIMD_TESTS !== "1")("current configured KOTH and native audio state tests prepare the SIMD comparison", async () => {
  const config = await loadLocalConfig()
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const borrowed = await borrowedWindowsJobLock(lockPath, { testFile: import.meta.filename })
  if (process.platform === "win32" && !borrowed) throw new Error("Windows configured tests require the native local-job supervisor")
  const lock = borrowed ?? await acquireHeadedProfileLock(lockPath, "simd-gameplay-preparation", 175_000)
  const directory = path.join(config.sourceCacheDir, "evidence/tf2-wasm-simd-performance", `native-state-${process.platform}-${crypto.randomUUID()}`)
  await mkdir(directory, { recursive: true })
  try {
    const cargo = path.join(config.sourceCacheDir, "toolchains/rust/cargo/bin", process.platform === "win32" ? "cargo.exe" : "cargo")
    const cases = [
      ["audio", "-p", "playsrc-audio", "--lib"],
      ["koth", "-p", "playsrc-tf2", "--test", "configured_koth", "viaduct_authored_logic_capture_io_and_generated_clocks", "--", "--ignored"],
    ]
    for (const [name, ...args] of cases) {
      const child = Bun.spawn([cargo, `+${toolchains.rust.threadedToolchain}`, "test", "--locked", ...args], { cwd: repositoryRoot,
        env: { ...process.env, ...rustEnvironment(config.sourceCacheDir), CARGO_BUILD_JOBS: "2" }, stdout: "pipe", stderr: "pipe", timeout: 145_000 })
      const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
      await writeFile(path.join(directory, `${name}.log`), stdout + stderr)
      expect(code, stderr).toBe(0)
    }
    console.log(`Native state preparation (not browser or sustained gameplay evidence): ${directory}`)
  } finally { if (!borrowed) await releaseHeadedProfileLock(lockPath, lock.token) }
}, 175_000)
