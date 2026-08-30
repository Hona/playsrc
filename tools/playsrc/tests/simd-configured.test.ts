import { expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { rustEnvironment } from "../src/setup"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { borrowedWindowsJobLock } from "../src/windows-job-native"
import toolchains from "../toolchains.json"

// Configured acceptance is opt-in on the local host, or explicitly scheduled
// under the native local-job supervisor. It is not a browser/profile admission.
test.skipIf(!process.env.PLAYSRC_LOCAL_JOB_OWNER && process.env.RUN_CONFIGURED_SIMD_TESTS !== "1")("configured native and actual scalar/SIMD WASM PCM are byte-exact", async () => {
  const config = await loadLocalConfig()
  const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const borrowed = await borrowedWindowsJobLock(lockPath, { testFile: import.meta.filename })
  if (process.platform === "win32" && !borrowed) throw new Error("Windows configured tests require the native local-job supervisor")
  const lock = borrowed ?? await acquireHeadedProfileLock(lockPath, "simd-configured-parity", 175_000)
  const directory = path.join(config.sourceCacheDir, "evidence/tf2-wasm-simd-performance", `configured-${process.platform}-${crypto.randomUUID()}`)
  await mkdir(directory, { recursive: true })
  const environment = { ...process.env, ...rustEnvironment(config.sourceCacheDir) }
  const cargo = path.join(config.sourceCacheDir, "toolchains/rust/cargo/bin", process.platform === "win32" ? "cargo.exe" : "cargo")
  const run = async (name: string, args: string[], flags?: string) => {
    const child = Bun.spawn([cargo, `+${toolchains.rust.threadedToolchain}`, ...args], { cwd: repositoryRoot,
      env: { ...environment, RUSTFLAGS: flags, CARGO_ENCODED_RUSTFLAGS: undefined, CARGO_BUILD_JOBS: "2" }, stdout: "pipe", stderr: "pipe", timeout: 145_000 })
    const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    await writeFile(path.join(directory, `${name}.log`), stdout + stderr)
    expect(code, stderr).toBe(0)
  }
  const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
  try {
    await run("native", ["test", "--locked", "-p", "playsrc-mp3", "--", "--include-ignored"])
    const input = await readFile(path.join(config.sourceCacheDir, "evidence/tf2-wasm-simd-performance/configured/cow1.mp3"))
    expect(hash(input)).toBe("6d5029641d1a058b5316d4fd49b7ee923ec6490bb5ce93e40fa25ccaa169aad5")
    const records = []
    for (const simd of [false, true]) {
      // MSVC's host build-script linker still has a bounded output path. Keep
      // Cargo scratch short and owned by this checkout; retain module bytes in
      // the unique evidence run rather than relying on mutable Cargo output.
      const variant = simd ? "simd" : "scalar", target = path.join(config.sourceCacheDir, "simd-tests", hash(Buffer.from(repositoryRoot)).slice(0, 8), variant)
      await run(variant, ["rustc", "--locked", "-p", "playsrc-mp3", "--lib", "--crate-type=cdylib", "--target", "wasm32-unknown-unknown", "--target-dir", target, "--release", "-Z", "build-std=panic_abort,std", "--", "--cfg", "test"], `-Ctarget-feature=${simd ? "+" : "-"}simd128`)
      const file = path.join(directory, `${variant}.wasm`)
      await copyFile(path.join(target, "wasm32-unknown-unknown/release/playsrc_mp3.wasm"), file)
      const bytes = await readFile(file)
      const { instance } = await WebAssembly.instantiate(bytes), e = instance.exports as any
      e.check_wasm_synthesis_groups()
      const pointer = e.test_input_alloc(input.length)
      new Uint8Array(e.memory.buffer, pointer, input.length).set(input)
      const count = e.test_decode(pointer, input.length)
      expect(count).toBe(73728)
      const pcmSha256 = hash(new Uint8Array(e.memory.buffer, e.test_pcm_pointer(), count * 2))
      expect(pcmSha256).toBe("b1e43ccf681c3529aad850231599216cfd55778a27bb559b8859917be486ee42")
      records.push({ variant, file, bytes: bytes.length, sha256: hash(bytes), samples: count, pcmSha256 })
    }
    const commit = Bun.spawnSync(["git", "rev-parse", "HEAD"], { cwd: repositoryRoot }).stdout.toString().trim()
    const result = JSON.stringify({ commit, platform: process.platform, arch: process.arch, engine: process.versions, browserEvidence: false, input: { path: path.join(config.sourceCacheDir, "evidence/tf2-wasm-simd-performance/configured/cow1.mp3"), bytes: input.length, sha256: hash(input) }, records }, null, 2)
    const resultPath = path.join(directory, "result.json")
    await writeFile(resultPath, result)
    const index = path.join(config.sourceCacheDir, "simd-tests", hash(Buffer.from(repositoryRoot)).slice(0, 8), "comparison.json")
    await writeFile(index, JSON.stringify({ path: resultPath, sha256: hash(Buffer.from(result)) }))
    console.log(`SIMD parity evidence: ${directory}`)
  } finally { if (!borrowed) await releaseHeadedProfileLock(lockPath, lock.token) }
}, 175_000)
