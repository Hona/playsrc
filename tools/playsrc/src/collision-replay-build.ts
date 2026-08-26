import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot, type LocalConfig } from "./config"
import { buildCacheDirectory, rustBuildIdentity } from "./build-identity"
import { rustEnvironment } from "./setup"
import toolchains from "../toolchains.json"

export async function buildCollisionReplay(config: LocalConfig): Promise<string> {
  const identity = await rustBuildIdentity()
  const directory = buildCacheDirectory(config.sourceCacheDir, identity)
  const destination = path.join(directory, "collision-replay.wasm")
  const manifest = `${destination}.json`
  try {
    const recorded = JSON.parse(await readFile(manifest, "utf8"))
    const bytes = await readFile(destination)
    if (recorded.identity !== identity || recorded.bytes !== bytes.length || recorded.sha256 !== createHash("sha256").update(bytes).digest("hex")) throw new Error("Collision replay build cache integrity failure")
    return destination
  } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error }
  const env = rustEnvironment(config.sourceCacheDir)
  const cargo = path.join(env.CARGO_HOME!, "bin", process.platform === "win32" ? "cargo.exe" : "cargo")
  const target = path.join(repositoryRoot, "target", "collision-replay-pinned")
  const child = Bun.spawn([cargo, `+${toolchains.rust.threadedToolchain}`, "build", "-p", "playsrc-tf2-wasm", "--target", "wasm32-unknown-unknown", "--release", "--features", "collision-replay", "-Z", "build-std=panic_abort,std", "--target-dir", target], {
    cwd: repositoryRoot, env: { ...process.env, ...env }, stdout: "inherit", stderr: "inherit",
  })
  const timeout = setTimeout(() => child.kill("SIGTERM"), 170_000)
  try { if (await child.exited !== 0) throw new Error("Bounded collision replay build failed; run the configured setup toolchain checks") }
  finally { clearTimeout(timeout) }
  const bytes = await readFile(path.join(target, "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm"))
  if (bytes.length > 64 * 1024 * 1024) throw new Error("Collision replay WASM byte bound exceeded")
  await mkdir(directory, { recursive: true })
  await writeFile(destination, bytes)
  await writeFile(manifest, JSON.stringify({ identity, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") }))
  return destination
}

if (import.meta.main) console.log(await buildCollisionReplay(await loadLocalConfig()))
