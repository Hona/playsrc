import path from "node:path"
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises"
import { createHash, randomUUID } from "node:crypto"
import { loadLocalConfig } from "./config"
import { readWasmManifest } from "./tf2-wasm-build"
import { exactWasmReplayRuntime } from "../profile/exact-wasm-replay"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
const [job, wanted, wantedClosure] = process.argv.slice(2)
if (!job || !/^[a-f0-9-]{36}$/.test(job) || [wanted, wantedClosure].some(value => value && !/^[a-f0-9]{64}$/.test(value))) throw new Error("Expected native job and optional exact retained WASM/closure SHA")
const config = await loadLocalConfig()
let source = path.join(config.sourceCacheDir, "local-jobs", job, "checkout/games/tf2/browser/src/wasm-generated")
if (wanted) {
  const cache = path.join(config.sourceCacheDir, "prepared-builds", `${process.platform}-${process.arch}`)
  const matches: string[] = []
  for (const entry of await readdir(cache, { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^[a-f0-9]{64}$/.test(entry.name)) continue
    const directory = path.join(cache, entry.name, "threaded-wasm")
    try {
      const manifest = JSON.parse(await readFile(path.join(directory, ".playsrc-build.json"), "utf8"))
      if (manifest.files.some((file: any) => file.name === "tf2_wasm_bg.wasm" && file.sha256 === wanted)) {
        const identity = createHash("sha256").update(JSON.stringify(manifest))
        for (const file of manifest.files) identity.update(file.name).update(file.sha256)
        if (!wantedClosure || identity.digest("hex") === wantedClosure) matches.push(directory)
      }
    } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error }
  }
  if (matches.length !== 1) throw new Error("Requested retained artifact is absent or ambiguous; pin its closure identity")
  source = matches[0]!
}
const manifest = JSON.parse(await readFile(path.join(source, ".playsrc-build.json"), "utf8"))
if (!await readWasmManifest(source, manifest.identity)) throw new Error("Retained generated closure changed")
const directory = path.join(config.sourceCacheDir, "observe-diagnostics", randomUUID())
await mkdir(directory, { recursive: true })
const runtime = await exactWasmReplayRuntime(source, directory, 2)
const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
const lock = await acquireHeadedProfileLock(lockPath, "allocator-offline-diagnostic", 5000)
try {
  const e = await runtime.instantiate(await readFile(path.join(source, "tf2_wasm_bg.wasm")), 0)
  const results = []
  for (const bytes of [64, 256, 4096]) {
    const before = e.playsrc_memory_bytes(0), cpu = process.cpuUsage(), start = performance.now()
    for (let index = 0; index < 50_000; index++) {
      const pointer = e.playsrc_alloc(bytes) >>> 0
      if (!pointer) throw new Error("Diagnostic allocation failed")
      e.playsrc_free(pointer, bytes)
    }
    results.push({ bytes, operations: 50_000, milliseconds: performance.now() - start, cpuMicroseconds: process.cpuUsage(cpu), before, after: e.playsrc_memory_bytes(0) })
  }
  const result = { source, manifest, engine: process.versions, threads: 2, visibleFpsEvidence: false, results }
  await writeFile(path.join(directory, "allocator.json"), JSON.stringify(result, null, 2))
  console.log(JSON.stringify({ directory, results }))
} finally { await runtime.close(); await releaseHeadedProfileLock(lockPath, lock.token) }
process.exit(0)
