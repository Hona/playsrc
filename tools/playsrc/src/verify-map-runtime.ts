import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { buildSourceBundle } from "./source-bundle"

const [target, retainedGraph] = process.argv.slice(2)
if (!target || process.argv.length > 4 || (retainedGraph && !/^[0-9a-f]{64}$/.test(retainedGraph))) {
  throw new Error("Usage: bun tools/playsrc/src/verify-map-runtime.ts <target> [retained-graph-sha256]")
}
const config = await loadLocalConfig()
await acquireMap(config, target)
const graph = retainedGraph ?? (await buildSourceBundle(config, target)).report.graphDescriptor.sha256
const child = Bun.spawn([
  path.join(config.sourceCacheDir, "toolchains/rust/cargo/bin", process.platform === "win32" ? "cargo.exe" : "cargo"),
  "run", "--profile", "source-bundle", "--features", "verify-hdr", "--bin", "playsrc-verify-map-runtime", "--", target, graph,
], {
  cwd: repositoryRoot,
  env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
  stdout: "inherit",
  stderr: "inherit",
})
process.exitCode = await child.exited
