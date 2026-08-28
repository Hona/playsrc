import { readFile, stat } from "node:fs/promises"
import { createHash } from "node:crypto"
import { objectPath } from "@playsrc/asset-store"
import { parseResourceGraphBytes } from "@playsrc/asset-store/graph"
import { loadLocalConfig } from "./config"
const sha = process.argv[2]
if (!sha || !/^[a-f0-9]{64}$/.test(sha)) throw new Error("Expected exact graph SHA")
const config = await loadLocalConfig(), pathname = process.argv[3] ?? objectPath(config.assetDir, sha)
let bytes: Buffer
try { bytes = await readFile(pathname) } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; console.log(JSON.stringify({ graph: sha, present: false })); process.exit(0) }
if (createHash("sha256").update(bytes).digest("hex") !== sha) throw new Error("Graph hash differs")
const graph = parseResourceGraphBytes(bytes), missing = []
for (const chunk of graph.chunks.filter(chunk => chunk.roles.includes("gameplay"))) {
  if (!(await stat(objectPath(config.assetDir, chunk.encodedSha256)).catch(() => null))?.isFile()) missing.push({ sha256: chunk.encodedSha256, byteLength: chunk.encodedByteLength })
}
console.log(JSON.stringify({ graph: sha, present: true, target: graph.target, missing }))
