import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { inflateRawSync } from "node:zlib"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"

type Entry = Readonly<{ logicalPath: string; offset: string; byteLength: string; sha256: string }>
type Chunk = Readonly<{
  codec: "identity" | "deflate"
  encodedByteLength: string
  encodedSha256: string
  decodedByteLength: string
  decodedSha256: string
  roles: readonly string[]
  entries: readonly Entry[]
}>
type Graph = Readonly<{ schema: string; game: string; contentBuild: string; target: string; chunks: readonly Chunk[] }>

const ROOT = path.resolve(import.meta.dir, "../../../..")
const MAX_CHUNK_BYTES = 64 * 1024 * 1024
const SHA256 = /^[0-9a-f]{64}$/
const TARGETS = Object.freeze({
  jump_beef: Object.freeze({
    files: Object.freeze({
      "materials/maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159.vmt": "2824deec4ec65df3ee5d5ef8e4c3419145ff5ff33479fe005518b859376f6335",
      "materials/water/water_2fort_expensive.vmt": "5f61b7786628a7e267419a7b709548102c115f2ef1f468bd3e3dc73aa6349806",
      "materials/water/water_2fort_beneath.vmt": "118cae4c43eda381491f99c0753fbc8963b35c2de787ee58194d3d7feaa028c8",
      "materials/water/tfwater001_normal.vtf": "7b5de49340bfe1ec2f1e37d771289d42773414f130767b5632ca29467494c017",
    }),
    overlay: "materials/effects/water_warp_2fort.vmt",
  }),
  pl_upward: Object.freeze({
    files: Object.freeze({
      "materials/maps/pl_upward/water/water_hydro_cheap_dx80_7168_-2048_128.vmt": "83a4ce4f5abe24b9446a634423b2f0b06e0f05dc9e3faf3e6b022d7c5ab34f57",
      "materials/water/water_hydro_cheap_dx80.vmt": "019a37e6ad1c042baa5bacc4641d88166d034cdbbe7ed0e1e36c87367cd28e65",
      "materials/water/water_well_beneath.vmt": "8b3a2f179dd8544d02dc2364a7cd692d5150e38b2fa2df6ee10935badb6a1aab",
      "materials/water/tfwater001_normal.vtf": "7b5de49340bfe1ec2f1e37d771289d42773414f130767b5632ca29467494c017",
      "materials/water/dx80_tfwater001_normal.vtf": "f763f3afc234f3ad6e9468dc9a98cca0e289f67810d8b6669f4cefd61cc5aea5",
      "materials/water/water_hydro_base.vtf": "f035cc70dfd265564ed6ed33f322eef7a025ab42f616349b92ee85d514281429",
    }),
    overlay: "materials/effects/water_warp_well.vmt",
  }),
})

function digest(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

function bounded(value: string, maximum: number, identity: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${identity} is not a canonical byte count`)
  const result = Number(value)
  if (!Number.isSafeInteger(result) || result > maximum) throw new Error(`${identity} exceeds ${maximum} bytes`)
  return result
}

async function run(args: string[]): Promise<void> {
  const child = Bun.spawn(args, { cwd: ROOT, stdout: "inherit", stderr: "inherit", stdin: "ignore" })
  const status = await child.exited
  if (status !== 0) throw new Error(`${args.join(" ")} failed with exit code ${status}`)
}

const configuration = await loadLocalConfig(ROOT)
const build = JSON.parse(await readFile(path.join(ROOT, "games/tf2/content-build.json"), "utf8")) as { contentBuild: string; patchVersion: string }
const evidence = path.join(configuration.sourceCacheDir, "evidence", "tf2-water-rendering")
const objectDirectory = path.join(evidence, "content")
await mkdir(objectDirectory, { recursive: true })
const reports: unknown[] = []
const retained = new Set<string>()

for (const [target, requirements] of Object.entries(TARGETS)) {
  const graphPath = path.join(configuration.sourceCacheDir, "browser-bundles", `${target}.graph.json`)
  const graphBytes = await readFile(graphPath)
  const graph = JSON.parse(graphBytes.toString("utf8")) as Graph
  if (graph.schema !== "playsrc-resource-graph-v1" || graph.game !== "tf2" || graph.target !== target || graph.contentBuild !== build.contentBuild) {
    throw new Error(`${target} resource graph does not match configured TF2 build ${build.contentBuild}`)
  }

  const entries = new Map<string, Readonly<{ entry: Entry; chunk: Chunk }>>()
  for (const chunk of graph.chunks) {
    if (!SHA256.test(chunk.encodedSha256) || !SHA256.test(chunk.decodedSha256)) {
      throw new Error(`${target} resource graph contains an invalid chunk identity`)
    }
    for (const entry of chunk.entries) {
      if (entries.has(entry.logicalPath)) throw new Error(`${target} resource graph repeats ${entry.logicalPath}`)
      entries.set(entry.logicalPath, Object.freeze({ entry, chunk }))
    }
  }

  const selected = new Map<Chunk, Entry[]>()
  for (const [logicalPath, expected] of Object.entries(requirements.files)) {
    const item = entries.get(logicalPath)
    if (!item) throw new Error(`${target} resource graph omits required ${logicalPath}`)
    if (!item.chunk.roles.includes("gameplay")) throw new Error(`${logicalPath} is not in the gameplay role`)
    if (item.entry.sha256 !== expected) throw new Error(`${logicalPath} differs from its fixed configured content identity`)
    const values = selected.get(item.chunk) ?? []
    values.push(item.entry)
    selected.set(item.chunk, values)
  }

  const objects: unknown[] = []
  for (const [chunk, items] of selected) {
    const encoded = await readFile(path.join(configuration.sourceCacheDir, "browser-bundles", `${target}.graph`, "objects", chunk.encodedSha256))
    if (encoded.byteLength !== bounded(chunk.encodedByteLength, MAX_CHUNK_BYTES, "encoded chunk") || digest(encoded) !== chunk.encodedSha256) {
      throw new Error(`${target} encoded chunk ${chunk.encodedSha256} failed integrity verification`)
    }
    const expectedDecoded = bounded(chunk.decodedByteLength, MAX_CHUNK_BYTES, "decoded chunk")
    const decoded = chunk.codec === "identity"
      ? encoded
      : chunk.codec === "deflate"
        ? inflateRawSync(encoded, { maxOutputLength: MAX_CHUNK_BYTES })
        : (() => { throw new Error(`${target} chunk codec is unsupported`) })()
    if (decoded.byteLength !== expectedDecoded || digest(decoded) !== chunk.decodedSha256) {
      throw new Error(`${target} decoded chunk ${chunk.decodedSha256} failed integrity verification`)
    }

    for (const entry of items) {
      const start = bounded(entry.offset, decoded.byteLength, "entry offset")
      const length = bounded(entry.byteLength, decoded.byteLength, "entry length")
      if (start + length > decoded.byteLength) throw new Error(`${entry.logicalPath} exceeds its indexed chunk`)
      const bytes = decoded.subarray(start, start + length)
      if (digest(bytes) !== entry.sha256) throw new Error(`${entry.logicalPath} failed exact source-byte verification`)
      if (!retained.has(entry.sha256)) {
        await writeFile(path.join(objectDirectory, entry.sha256), bytes)
        retained.add(entry.sha256)
      }
      objects.push(Object.freeze({ logicalPath: entry.logicalPath, byteLength: bytes.byteLength, sha256: entry.sha256 }))
    }
  }

  reports.push(Object.freeze({
    target,
    graphSha256: digest(graphBytes),
    entries: entries.size,
    gameplayEntries: graph.chunks.filter((chunk) => chunk.roles.includes("gameplay")).reduce((sum, chunk) => sum + chunk.entries.length, 0),
    objects: Object.freeze(objects.toSorted((left: any, right: any) =>
      left.logicalPath < right.logicalPath ? -1 : left.logicalPath > right.logicalPath ? 1 : 0,
    )),
    underwaterOverlay: Object.freeze({ logicalPath: requirements.overlay, present: entries.has(requirements.overlay) }),
  }))
}

await run(["cargo", "test", "--locked", "-p", "playsrc-material", "--test", "configured_water_materials", "--", "--ignored"])
await run(["cargo", "test", "--locked", "-p", "playsrc-vtf", "--test", "water_authored_planes", "--", "--ignored"])

const authoredNormalFrames = []
for (const frame of [0, 30]) {
  const samples = await readFile(path.join(evidence, `normal-frame-${frame}.rgb`))
  if (samples.byteLength !== 256 * 256 * 3) throw new Error(`decoded authored normal frame ${frame} has an invalid byte length`)
  authoredNormalFrames.push(Object.freeze({ frame, byteLength: samples.byteLength, sha256: digest(samples) }))
}

const report = Object.freeze({
  schema: "playsrc-water-content-evidence-v1",
  contentBuild: build.contentBuild,
  patchVersion: build.patchVersion,
  retainedObjects: retained.size,
  authoredNormalFrames: Object.freeze(authoredNormalFrames),
  targets: Object.freeze(reports),
})
await writeFile(path.join(evidence, "water-content.json"), `${JSON.stringify(report, null, 2)}\n`)
console.log(`SOURCE_WATER_CONTENT ${JSON.stringify(report)}`)
