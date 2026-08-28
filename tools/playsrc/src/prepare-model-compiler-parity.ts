import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, type LocalConfig } from "./config"
import { acquireMap } from "./targets"
import { buildSourceBundle } from "./source-bundle"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { encodeResourceBatch, parseResourceSet } from "@playsrc/asset-store/graph"
import { parseRuntimeMap } from "@playsrc/rendering/runtime-map"
import { parsePresentationArtifacts } from "../../../games/tf2/browser/src/artifacts"
import { classPipelinePoseRequests } from "../../../games/tf2/browser/src/class-pipeline-preparation"
import { encodeModelPoseBatch, decodeModelPoseOutput } from "../../../games/tf2/browser/src/presentation"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"
import { tf2UiResources } from "../../../games/tf2/browser/src/ui-resources"
import { rustBuildIdentity } from "./build-identity"

const digest = (bytes: Uint8Array) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
const require = (value: unknown, subject: string) => { if (!value) throw new Error(`Model compiler preparation: ${subject}`) }

/** Exact compiler inputs, not authored assets, pixels or a browser benchmark.
 * The normal configured resource/WASM owners generate these local fixtures. */
export async function prepareModelCompilerParity(config: LocalConfig, target: string, profile: 0 | 1) {
  const identity = await rustBuildIdentity()
  const [map, bundle, wasmPath] = await Promise.all([acquireMap(config, target), buildSourceBundle(config, target), buildTf2Wasm(config, false)])
  const [wasm, bsp, chunks] = await Promise.all([readFile(wasmPath), readFile(path.join(config.sourceCacheDir, map.decoded.cachePath)),
    Promise.all(bundle.graph.chunks.filter(descriptor => descriptor.roles.includes("gameplay")).map(async descriptor => ({ descriptor,
      bytes: await readFile(path.join(bundle.graphObjectDirectory, descriptor.encodedSha256)) })))])
  const { instance } = await WebAssembly.instantiate(wasm, { playsrc_metrics: { monotonic_milliseconds: () => performance.now() } })
  const native = instance.exports as any
  const put = (bytes: Uint8Array) => { const pointer = native.playsrc_alloc(bytes.length) >>> 0; require(pointer, "allocation");
    new Uint8Array(native.memory.buffer, pointer, bytes.length).set(bytes); return pointer }
  const copy = (length: number, write: (pointer: number, capacity: number) => number) => {
    require(length > 0 && length <= 512 * 1024 * 1024, "output bound")
    const pointer = native.playsrc_alloc(length) >>> 0
    try { require(write(pointer, length) === length, "output copy"); return new Uint8Array(native.memory.buffer, pointer, length).slice() }
    finally { native.playsrc_free(pointer, length) }
  }
  const batch = encodeResourceBatch(chunks), batchPointer = put(batch)
  require(native.playsrc_resource_decode(batchPointer, batch.length) === 1, "configured resource decode")
  native.playsrc_free(batchPointer, batch.length)
  const resourcesLength = native.playsrc_resource_length(), resourcesPointer = native.playsrc_resource_take() >>> 0
  require(resourcesPointer, "resource ownership")
  const resources = new Uint8Array(native.memory.buffer, resourcesPointer, resourcesLength).slice()
  const table = new Uint8Array(8), view = new DataView(table.buffer)
  view.setUint32(0, resourcesPointer, true); view.setUint32(4, resourcesLength, true)
  const tablePointer = put(table), hash = new Bun.CryptoHasher("sha256").update(resources).digest(), hashPointer = put(hash), bspPointer = put(bsp)
  const handle = native.playsrc_compile_map(bspPointer, bsp.length, profile, tablePointer, 1, hashPointer, 1)
  native.playsrc_free(bspPointer, bsp.length); native.playsrc_free(tablePointer, 8); native.playsrc_free(hashPointer, 32)
  try {
    require(native.playsrc_result_error(handle) === 0, `map compilation:${native.playsrc_result_error(handle)}`)
    const payload = copy(native.playsrc_result_length(handle), (pointer, length) => native.playsrc_result_copy(handle, pointer, length))
    const presentation = copy(native.playsrc_presentation_length(handle), (pointer, length) => native.playsrc_presentation_copy(handle, pointer, length))
    const runtime = parseRuntimeMap(payload), artifacts = await parsePresentationArtifacts(presentation, parseResourceSet(resources))
    const camera = { position: [0, 0, 0] as const, yawDegrees: 0, pitchDegrees: 0, far: 16384 * Math.sqrt(3) }
    const passes = [0, 1].map(skin => classPipelinePoseRequests(artifacts, skin, camera, 16 / 9, nativeEquipment.inventory))
    const roots = new Set(passes.flat().map(entry => entry.request.model))
    const visit = (node: any) => { if (node.value !== null && node.name.toLowerCase() === "modelname" && node.value.endsWith(".mdl")) roots.add(node.value.toLowerCase())
      for (const child of node.children) visit(child) }
    for (const panel of tf2UiResources.panels.filter(panel => ["resource/ui/teammenu.res", "resource/ui/classselection.res"].includes(panel.source.logicalPath))) panel.roots.forEach(visit)
    const poses: Uint8Array[] = []
    for (const requests of passes) {
      const bytes = encodeModelPoseBatch(requests.map(entry => ({ ...entry.request, preparation: true }))), pointer = put(bytes)
      try { require(native.playsrc_model_transact(handle, pointer, bytes.length) === 1, "actual preparation poses") }
      finally { native.playsrc_free(pointer, bytes.length) }
      poses.push(copy(native.playsrc_model_output_length(handle), (pointer, length) => native.playsrc_model_output_copy(handle, pointer, length)))
    }
    const decodedPoses = poses.map(bytes => decodeModelPoseOutput(bytes))
    for (const pose of decodedPoses.flat()) roots.add(pose.model)
    const geometry = runtime.models.filter(model => roots.has(model.logicalPath.split("#skin=")[0]!))
    require(roots.size > 9 && geometry.length > 18, "actual class/team geometry coverage")
    const materialNames = new Set(geometry.flatMap(model => model.materials.map(material => material.logicalPath.toLowerCase())))
    const materials = [...artifacts.modelMaterials].filter(([name]) => materialNames.has(name))
    const textureNames = new Set(materials.flatMap(([, material]) => material.bindings.map(binding => binding.logicalPath.toLowerCase())))
    const fixture = { schema: "playsrc-model-compiler-parity-input-v1", target, profile, identity, contentBuild: bundle.report.contentBuild,
      provenance: { bsp: map.decoded.sha256, resources: bundle.report.graphDescriptor.sha256, wasm: digest(wasm), payload: digest(payload), presentation: digest(presentation) },
      geometry, models: [...artifacts.models].filter(([name]) => roots.has(name)).map(([name, model]) => [name, { ...model, bytes: undefined, attachments: undefined }]),
      materials, materialStates: [...artifacts.materialStates].filter(([name]) => materialNames.has(name)),
      textures: [...artifacts.authoredTextures].filter(([name]) => textureNames.has(name)).map(([name, texture]) => [name, { ...texture,
        planes: texture.planes.map(({ rgba, ...plane }) => ({ ...plane, byteLength: rgba.byteLength, sha256: digest(rgba) })),
      }]),
       requests: passes, poses: decodedPoses, roots: [...roots].sort(),
       particles: artifacts.particleTextures.map(texture => ({ ...texture,
         planes: texture.planes.map(({ rgba, ...plane }) => ({ ...plane, byteLength: rgba.byteLength, sha256: digest(rgba) })),
         state: artifacts.materialStates.get(texture.material.toLowerCase()),
       })) }
    const arrays: Uint8Array[] = []
    let arenaLength = 0
    const metadata = JSON.stringify(fixture, (_, value) => {
      if (typeof value === "bigint") return { bigInt: value.toString() }
      if (!ArrayBuffer.isView(value)) return value
      const padding = (8 - arenaLength % 8) % 8
      if (padding) { arrays.push(new Uint8Array(padding)); arenaLength += padding }
      const reference = { arrayType: value.constructor.name, byteOffset: arenaLength, byteLength: value.byteLength }
      arrays.push(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)); arenaLength += value.byteLength
      return reference
    })
    const arena = Buffer.concat(arrays), arenaHash = digest(arena)
    const bytes = new TextEncoder().encode(JSON.stringify({ arena: { file: `${arenaHash}.bin`, sha256: arenaHash, byteLength: arena.length }, fixture: JSON.parse(metadata) }))
    require(bytes.length + arena.length <= 256 * 1024 * 1024, "fixture bound")
    require(await rustBuildIdentity() === identity, "build inputs changed")
    const directory = path.join(config.sourceCacheDir, "evidence", "model-compiler-parity", target, profile === 0 ? "ldr" : "hdr")
    await mkdir(directory, { recursive: true })
    const file = path.join(directory, `${digest(bytes)}.json`)
    await writeFile(path.join(directory, `${arenaHash}.bin`), arena)
    await writeFile(file, bytes)
    return { file, byteLength: bytes.length, sha256: digest(bytes), arenaBytes: arena.length, arenaSha256: arenaHash, models: geometry.length, roots: roots.size, materials: materials.length, textures: textureNames.size }
  } finally { native.playsrc_dispose(handle); native.playsrc_resource_release(resourcesPointer, resourcesLength) }
}

if (import.meta.main) {
  const [target, profile] = process.argv.slice(2)
  if (!target || !["ldr", "hdr"].includes(profile ?? "") || process.argv.length !== 4) throw new Error("Usage: prepare-model-compiler-parity.ts <configured map> ldr|hdr")
  console.log(JSON.stringify(await prepareModelCompilerParity(await loadLocalConfig(), target, profile === "hdr" ? 1 : 0)))
}
