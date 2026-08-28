import path from "node:path"
import { readFile, writeFile } from "node:fs/promises"
import { loadLocalConfig } from "./config"
import { verifyParticleCompilerParity } from "../../../packages/presentation/rendering/tests/fixtures/particle-compiler-parity"

const [file] = process.argv.slice(2), { sourceCacheDir } = await loadLocalConfig()
if (process.argv.length !== 3 || !file || !path.resolve(file).startsWith(path.resolve(sourceCacheDir) + path.sep)) throw new Error("Usage: verify-particle-compiler-parity.ts <configured-cache compiler input.json>")
const digest = (bytes: string | Uint8Array) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
const bytes = await readFile(file), { fixture } = JSON.parse(bytes.toString())
if (path.basename(file) !== `${digest(bytes)}.json` || fixture.contentBuild !== "24245096" || !fixture.particles?.length) throw new Error("Exact configured particle fixture is required")
const started = performance.now(), summaries = []
for (const hdr of [false, true]) for (const generation of [0, 1]) {
  const result = verifyParticleCompilerParity(fixture.particles, hdr)
  summaries.push({ hdr, generation, ...result, records: result.records.map(record => ({ ...record, vertex: digest(record.vertex), fragment: digest(record.fragment) })) })
}
const report = { input: path.basename(file), provenance: fixture.provenance, contentBuild: fixture.contentBuild, summaries, milliseconds: performance.now() - started, pixelsVerified: false }
const output = path.join(path.dirname(file), `${digest(JSON.stringify(report))}.particle-parity.json`)
await writeFile(output, JSON.stringify(report, null, 2))
console.log(JSON.stringify({ output, summaries: summaries.map(({ records, ...summary }) => summary), milliseconds: report.milliseconds }))
