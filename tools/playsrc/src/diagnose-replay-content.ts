import { readFile, stat } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { descriptor, objectPath, putObject, verifyObject } from "@playsrc/asset-store"
import { parseResourceGraphBytes, resourceChunkObject } from "@playsrc/asset-store/graph"
import { loadLocalConfig } from "./config"
const sha = process.argv[2]
if (!sha || !/^[a-f0-9]{64}$/.test(sha)) throw new Error("Expected exact graph SHA")
const config = await loadLocalConfig(), pathname = process.argv[3] ?? objectPath(config.assetDir, sha)
let bytes: Buffer
try { bytes = await readFile(pathname) } catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; console.log(JSON.stringify({ graph: sha, present: false })); process.exit(0) }
if (createHash("sha256").update(bytes).digest("hex") !== sha) throw new Error("Graph hash differs")
const graph = parseResourceGraphBytes(bytes), missing = []
const install = process.argv[4] === "--install-diagnostic-inputs"
if (install) await putObject(config.assetDir, descriptor("source-root", "application/vnd.playsrc.resource-graph+json", bytes), bytes)
for (const chunk of graph.chunks.filter(chunk => chunk.roles.includes("gameplay"))) {
  const expected = resourceChunkObject(chunk)
  if (!(await stat(objectPath(config.assetDir, chunk.encodedSha256)).catch(() => null))?.isFile()) {
    if (install) await putObject(config.assetDir, expected, await readFile(path.join(path.dirname(pathname), chunk.encodedSha256)))
    else missing.push({ sha256: chunk.encodedSha256, byteLength: chunk.encodedByteLength })
  }
  if (install) await verifyObject(config.assetDir, expected)
}
console.log(JSON.stringify({ graph: sha, present: true, target: graph.target, missing, localDiagnosticInputsVerified: install, productionChanged: false }))
