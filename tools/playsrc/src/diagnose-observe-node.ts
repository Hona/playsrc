import path from "node:path"
import { mkdir, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import { loadLocalConfig } from "./config"
import { exactWasmReplayRuntime } from "../profile/exact-wasm-replay"
import { replayGameplay } from "../profile/replay-gameplay"
import { installNodeWorkerHost } from "../profile/node-worker-host.mjs"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
const [source, manifest] = process.argv.slice(2)
if (!source || !manifest) throw new Error("Expected exact generated closure and complete command capture manifest")
const config = await loadLocalConfig(), directory = path.join(config.sourceCacheDir, "observe-diagnostics", "node-" + randomUUID())
await mkdir(directory, { recursive: true })
const restore = installNodeWorkerHost()
const runtime = await exactWasmReplayRuntime(source, directory, 2)
const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
const lock = await acquireHeadedProfileLock(lockPath, "node-observe-offline", 5000)
try {
  const wasm = path.join(source, "tf2_wasm_bg.wasm")
  const result = await replayGameplay(manifest, wasm, false, false, wasm, undefined, runtime)
  await writeFile(path.join(directory, "result.json"), JSON.stringify({ ...result, engine: process.versions, generatedClosure: runtime.manifest, threads: 2, visibleFpsEvidence: false }, null, 2))
  console.log(JSON.stringify({ directory, milliseconds: result.totalMilliseconds, passes: result.passes.map(pass => ({ ticks: pass.tickMilliseconds, observes: pass.observeMilliseconds })) }, null, 2))
} finally { await runtime.close(); restore(); await releaseHeadedProfileLock(lockPath, lock.token) }
process.exit(0)
