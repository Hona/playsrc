import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { buildSourceBundle } from "./source-bundle"

const [target, ...options] = process.argv.slice(2)
const sustainedIndex = options.indexOf("--sustained-bots")
const sustained = sustainedIndex < 0 ? [] : options.slice(sustainedIndex + 1)
const match = options.includes("--control-point-match")
const crossingIndex=options.indexOf("--control-point-crossing")
const crossing=crossingIndex<0?[]:options.slice(crossingIndex+1)
const ordinary=sustainedIndex>=0?options.slice(0,sustainedIndex):crossingIndex<0?options:options.slice(0,crossingIndex)
const retainedGraph = ordinary.find(value => value !== "--control-point-match")
if (!target || ordinary.length > (match ? 2 : 1) || new Set(ordinary).size !== ordinary.length || (retainedGraph && !/^[0-9a-f]{64}$/.test(retainedGraph))
   || crossingIndex>=0&&(match||crossing.length!==2||crossing.some(value=>!/^\d+$/.test(value)||Number(value)>0xffffffff))
   || sustainedIndex>=0&&(match||crossingIndex>=0||!/^koth_(harvest_final|viaduct)$/.test(target)||sustained.length<1||sustained.length>2||!sustained[0]||!/^[A-Za-z0-9-]+$/.test(sustained[0])||sustained[1]!==undefined&&!/^[0-9a-f]{64}$/.test(sustained[1]))) {
  throw new Error("Usage: bun tools/playsrc/src/verify-map-runtime.ts <target> [retained-graph-sha256] [--control-point-match | --control-point-crossing from to | --sustained-bots label [replay-sha256]]")
}
const config = await loadLocalConfig()
await acquireMap(config, target)
const graph = retainedGraph ?? (await buildSourceBundle(config, target)).report.graphDescriptor.sha256
const child = Bun.spawn([
  path.join(config.sourceCacheDir, "toolchains/rust/cargo/bin", process.platform === "win32" ? "cargo.exe" : "cargo"),
  "run", "--profile", "source-bundle", "--features", "verify-hdr", "--bin", "playsrc-verify-map-runtime", "--", target, graph, ...(match ? ["--control-point-match"] : sustainedIndex>=0?["--sustained-bots",...sustained]:crossingIndex>=0?["--control-point-crossing",...crossing]:[]),
], {
  cwd: repositoryRoot,
  env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
  stdout: "inherit",
  stderr: "inherit",
})
process.exitCode = await child.exited
