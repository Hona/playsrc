import { readFile } from "node:fs/promises"
import path from "node:path"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { parseRuntimeMap } from "@playsrc/rendering/runtime-map"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import { buildSourceBundle } from "./source-bundle"
import { decodeSnapshot, encodeCommand } from "../../../games/tf2/browser/src/codec"
import { decodeModelPoseOutput, encodeModelPoseBatch } from "../../../games/tf2/browser/src/presentation"
import { parsePresentationArtifacts } from "../../../games/tf2/browser/src/artifacts"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { encodeResourceBatch, parseResourceSet } from "@playsrc/asset-store/graph"

const EXPECTED_MAP_BYTES = 27_137_800
const EXPECTED_MAP_SHA256 = "15cdbb753aedac70a3eee1a2f0dfe627455e25619650a534eba1a9280e47aa17"
const EXPECTED_BSP_SHA256 = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959"
const EXPECTED_HDR_BYTES=63_346_564
const EXPECTED_HDR_SHA256="fa66808948ae3c0f8ebc94fcab5d203bd5032d59dc30712614da80dd619ee986"
const EXPECTED_LDR_DERIVED_SHA256="aad5272deacd8cbfd3883e722c87a549794ddc0a86ddadaa33852955cb5db3ef"
const EXPECTED_HDR_DERIVED_SHA256="e5e30e00773fb030e34b6722b4593290322749ab07f2bad96d1132bb64aaf5fa"
const EXPECTED_PARTICLE_MATERIAL_STATE_SHA256 = "65510289b8254192ecf843283ee18b106a0decef9f0f718b1e54c043cfa9fbdb"
function resourcePathOffset(bytes: Uint8Array, target: string): number {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  let offset = 12
  for (let index = 0; index < view.getUint32(8, true); index++) {
    const length = view.getUint32(offset, true)
    offset += 4
    const path = new TextDecoder().decode(bytes.subarray(offset, offset + length))
    if (path === target) return offset
    offset += length
    const valueLength = view.getUint32(offset, true)
    offset += 4 + valueLength
  }
  return -1
}
function collisionBrushRecords(bytes:Uint8Array):ReadonlyMap<bigint,Readonly<{enabled:boolean;contents:number;model:number|null}>>{
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength)
  require(new TextDecoder().decode(bytes.subarray(0,4))==="CSNP"&&view.getUint32(4,true)===3,"Collision snapshot schema differs")
  let at=52;const output=new Map<bigint,Readonly<{enabled:boolean;contents:number;model:number|null}>>()
  for(let count=view.getUint32(48,true);count>0;count--){const identity=view.getBigUint64(at,true),enabled=(bytes[at+9]!&1)===1,contents=view.getUint32(at+16,true),shape=bytes[at+68]!,model=shape===0?Number(view.getBigUint64(at+69,true)):null;output.set(identity,Object.freeze({enabled,contents,model}));at+=shape===0?77:shape===1||shape===2?93:81}
  require(at===bytes.length,"Collision snapshot records are truncated")
  return output
}

type Exports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_resource_decode(pointer: number, length: number): number
  playsrc_resource_length(): number
  playsrc_resource_take(): number
  playsrc_resource_release(pointer: number, length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_compile_map(bsp: number, length: number, profile: number, sections: number, sectionCount: number, configurationSha256: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_result_derived_hash(handle: number, pointer: number): number
  playsrc_presentation_length(handle: number): number
  playsrc_presentation_copy(handle: number, pointer: number, capacity: number): number
  playsrc_coverage_length(handle:number):number
  playsrc_coverage_copy(handle:number,pointer:number,capacity:number):number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_game_advance(handle: number, command: number, length: number, ticks: number): number
  playsrc_simulation_observe(handle:number,now:number,command:number,length:number,suspended:number,snapshotTick:bigint):number
  playsrc_simulation_output_length(handle:number):number
  playsrc_jump_configure(handle: number, definition: number, length: number): number
  playsrc_particle_transact(handle: number, pointer: number, length: number): number
  playsrc_particle_output_length(handle: number): number
  playsrc_particle_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_model_transact(handle: number, pointer: number, length: number): number
  playsrc_model_output_length(handle: number): number
  playsrc_model_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_visibility_query(handle: number, pointer: number): number
  playsrc_visibility_output_length(handle: number): number
  playsrc_visibility_output_copy(handle:number,pointer:number,capacity:number):number
  playsrc_runtime_count(handle: number, kind: number): number
  playsrc_snapshot_length(handle: number): number
  playsrc_snapshot_copy(handle: number, pointer: number, capacity: number): number
  playsrc_teleport_count(handle: number): number
  playsrc_teleport_destination_count(handle: number): number
  playsrc_dispose(handle: number): number
}>

export class WasmVerificationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "WasmVerificationError"
  }
}

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new WasmVerificationError(message)
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("")
}

class ProfileReader {
  readonly bytes: Uint8Array
  readonly view: DataView
  offset = 0
  constructor(bytes: Uint8Array) {
    this.bytes = bytes
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  }
  take(length: number): Uint8Array {
    require(Number.isSafeInteger(length) &&
      length >= 0 &&
      this.offset + length <= this.bytes.byteLength, "HDR payload range is invalid")
    const value = this.bytes.subarray(this.offset, this.offset + length)
    this.offset += length
    return value
  }
  u8(): number {
    return this.take(1)[0]!
  }
  u32(): number {
    const offset = this.offset
    this.take(4)
    return this.view.getUint32(offset, true)
  }
  i32(): number {
    const offset = this.offset
    this.take(4)
    return this.view.getInt32(offset, true)
  }
  f32(): number {
    const offset = this.offset
    this.take(4)
    const value = this.view.getFloat32(offset, true)
    require(Number.isFinite(value), "HDR payload contains a non-finite scalar")
    return value
  }
  sized(): Uint8Array {
    return this.take(this.u32())
  }
  text(): string {
    return new TextDecoder("utf-8", { fatal: true }).decode(this.sized())
  }
}

function skipRuntimeMaterial(reader: ProfileReader): { shader: number; role: number } {
  const shader = reader.u8()
  reader.u8()
  const hasTexture = reader.u8()
  const role = reader.u8()
  require(hasTexture <= 1, "runtime material texture marker is invalid")
  if (hasTexture === 1) {
    reader.sized()
    reader.u32()
    reader.u32()
  }
  return { shader, role }
}

function inspectHdrPayload(payload: Uint8Array, expectedConfigurationSha256: string) {
  const reader = new ProfileReader(payload)
  require(new TextDecoder().decode(reader.take(4)) === "PSMP", "HDR map magic is invalid")
  require(reader.u32() === 7, "HDR map schema is invalid")
  require(reader.u32() === 20 && reader.u32() === 731 && reader.u8() === 1, "HDR map identity is invalid")
  const materialCount = reader.u32()
  const surfaceCount = reader.u32()
  const lightingSamples = reader.u32()
  const entityCount = reader.u32()
  require(materialCount === 14 &&
    surfaceCount === 3_793 &&
    lightingSamples === 3_896_843 &&
    entityCount === 361, "HDR map root counts are invalid")
  for (let index = 0; index < materialCount; index += 1) {
    reader.sized()
    reader.i32()
    reader.i32()
  }
  for (let index = 0; index < surfaceCount; index += 1) {
    reader.take(16)
    reader.u8()
    const vertices = reader.u32()
    const triangles = reader.u32()
    require(vertices >= 3 && triangles >= 1, "HDR surface dimensions are invalid")
    reader.take(vertices * 40 + triangles * 12 + 16)
  }
  let belowOne = false
  let equalOne = false
  let aboveOne = false
  let maximumLinearChannel = 0
  for (let sample = 0; sample < lightingSamples; sample += 1) {
    for (let channel = 0; channel < 3; channel += 1) {
      const value = reader.f32()
      if (value > 0 && value < 1) belowOne = true
      else if (value === 1) equalOne = true
      else if (value > 1) aboveOne = true
      maximumLinearChannel = Math.max(maximumLinearChannel, value)
    }
  }
  require(belowOne && equalOne && aboveOne, "HDR linear samples do not cover below, equal, and above one")
  require(maximumLinearChannel === 50.94902420043945, `HDR maximum linear radiance changed: ${maximumLinearChannel}`)
  reader.sized()
  require(reader.u32() === materialCount, "HDR resolved-material count is invalid")
  for (let index = 0; index < materialCount; index += 1) skipRuntimeMaterial(reader)
  const modelCount = reader.u32()
  require(modelCount === 56, "HDR model count is invalid")
  for (let model = 0; model < modelCount; model += 1) {
    reader.sized()
    const modelMaterials = reader.u32()
    for (let material = 0; material < modelMaterials; material += 1) {
      reader.sized()
      skipRuntimeMaterial(reader)
    }
    const primitives = reader.u32()
    for (let primitive = 0; primitive < primitives; primitive += 1) {
      reader.u32()
      const vertices = reader.u32()
      const triangles = reader.u32()
      reader.take(vertices * 32 + triangles * 12)
    }
  }
  const occurrences = reader.u32()
  require(occurrences === 33, "HDR model occurrence count is invalid")
  reader.take(occurrences * 32)
  require(new TextDecoder().decode(reader.take(4)) === "PSHD", "HDR descriptor magic is invalid")
  require(reader.u32() === 1 && reader.u8() === 1, "HDR descriptor version or encoding is invalid")
  require(reader.take(3).every((value) => value === 0), "HDR descriptor reserved bytes are nonzero")
  const outputRole = reader.text()
  const compilerIdentity = reader.text()
  const bspSha256 = hex(reader.take(32))
  const configurationSha256 = hex(reader.take(32))
  const profileSha256 = hex(reader.take(32))
  require(outputRole === "map-runtime-hdr" &&
    compilerIdentity === "playsrc-map-runtime-hdr-1", "HDR compiler identity is invalid")
  require(bspSha256 === EXPECTED_BSP_SHA256 &&
    configurationSha256 === expectedConfigurationSha256, "HDR source/configuration identity is invalid")
  const memberCount = reader.u32()
  require(memberCount === 10, "HDR profile-member count is invalid")
  const memberSlots = new Map<number, number | string>()
  for (let index = 0; index < memberCount; index += 1) {
    const role = reader.u8()
    const source = reader.u8()
    const slot = reader.u8()
    reader.take(2)
    if (source === 1) {
      memberSlots.set(role, slot)
      reader.i32()
    } else if (source === 2) {
      const id = new TextDecoder().decode(reader.take(4))
      memberSlots.set(role, id)
      reader.u32()
    } else {
      require(source === 0, "HDR profile member source is invalid")
      reader.take(4)
    }
    reader.u32()
    reader.u32()
    reader.take(64)
    reader.u32()
  }
  require(memberSlots.get(1) === 58 &&
    memberSlots.get(2) === 53 &&
    memberSlots.get(3) === 54 &&
    memberSlots.get(4) === 51 &&
    memberSlots.get(5) === 55 &&
    memberSlots.get(6) === 59, "HDR profile selected an incorrect standard lump")
  const lightmappedFaces = reader.u32()
  const directionalFaces = reader.u32()
  const profileSurfaces = reader.u32()
  const surfaceKinds = [0, 0, 0, 0]
  for (let index = 0; index < profileSurfaces; index += 1) {
    reader.u32()
    const kind = reader.u8()
    require(kind < surfaceKinds.length, "HDR surface lighting kind is invalid")
    surfaceKinds[kind]! += 1
    const styles = reader.u8()
    const layers = reader.u8()
    require(reader.u8() === 0 &&
      styles <= 1 &&
      (layers === 0 || layers === 1 || layers === 4), "HDR surface lighting framing is invalid")
    reader.take(12)
  }
  require(lightmappedFaces === 2_984 &&
    directionalFaces === 1_511 &&
    profileSurfaces === 3_793 &&
    surfaceKinds.join(",") === "809,1473,0,1511", `HDR surface classifications are invalid: ${surfaceKinds.join(",")}`)
  const worldLights = reader.u32()
  require(worldLights === 73, "HDR world-light count is invalid")
  reader.take(worldLights * 88)
  const ambientIndexes = reader.u32()
  require(ambientIndexes === 1_899, "HDR ambient-index count is invalid")
  reader.take(ambientIndexes * 4)
  const ambientSamples = reader.u32()
  require(ambientSamples === 9_014, "HDR ambient-sample count is invalid")
  reader.take(ambientSamples * 76)
  const detailProps = reader.u32()
  const detailStyles = reader.u32()
  const staticProps = reader.u32()
  const mapFlags = reader.u32()
  require(detailProps === 0 &&
    detailStyles === 0 &&
    staticProps === 0 &&
    mapFlags === 0, "HDR prop-lighting disposition is invalid")
  const profileMaterials = reader.u32()
  require(profileMaterials === 6, "HDR profile-material count is invalid")
  const skyDimensions: string[] = []
  for (let index = 0; index < profileMaterials; index += 1) {
    const material = reader.text()
    const shader = reader.u8()
    reader.u8()
    const role = reader.u8()
    require(reader.u8() === 0, "HDR profile material reserved byte is nonzero")
    const texture = reader.text()
    const width = reader.u32()
    const height = reader.u32()
    const format = reader.i32()
    const sourceSha256 = hex(reader.take(32))
    const source = reader.sized()
    require(shader === 8 && role === 2 && format === 12, "HDR sky material selection is invalid")
    require(material.endsWith(".vmt") && texture.endsWith(".vtf"), "HDR sky dependency identity is invalid")
    require(new Bun.CryptoHasher("sha256").update(source).digest("hex") === sourceSha256, "HDR sky VTF hash is invalid")
    skyDimensions.push(`${width}x${height}`)
  }
  require(skyDimensions.join(",") === "512x256,512x256,512x256,512x256,512x512,4x4", "HDR sky dimensions are invalid")
  const inputCount = reader.u32()
  require(inputCount===725,`HDR input-hash count is invalid: ${inputCount}`)
  for (let index = 0; index < inputCount; index += 1) {
    require(reader.u8() === 1 && reader.take(3).every((value) => value === 0), "HDR input record is invalid")
    require(reader.text().length > 0, "HDR input path is empty")
    reader.take(32)
  }
  require(reader.offset === payload.byteLength, "HDR payload contains trailing bytes")
  return {
    lightingSamples,
    lightmappedFaces,
    directionalFaces,
    worldLights,
    ambientIndexes,
    ambientSamples,
    surfaceKinds: surfaceKinds.join(","),
    maximumLinearChannel,
    profileSha256,
    profileMaterials,
    inputCount,
  }
}

async function buildNativeHdr(
  config: LocalConfig,
  target: string,
): Promise<{
  path: string
  bytes: number
  sha256: string
  derivedSha256: string
}> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const child = Bun.spawn(
    [
      cargo,
      `+${toolchains.rust.toolchain}`,
      "run",
      "--release",
      "--features",
      "verify-hdr",
      "-p",
      "playsrc-source-bundle",
      "--",
      target,
      "--verify-hdr",
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
      stdout: "pipe",
      stderr: "inherit",
    },
  )
  const output = await new Response(child.stdout).text()
  require((await child.exited) === 0, "native HDR generation failed")
  const report = JSON.parse(output) as Record<string, unknown>
  require(report.target === target &&
    Number.isSafeInteger(report.nativeHdrBytes) &&
    typeof report.nativeHdrSha256 === "string" &&
    /^[0-9a-f]{64}$/.test(report.nativeHdrSha256) &&
    typeof report.nativeHdrDerivedSha256 === "string" &&
    /^[0-9a-f]{64}$/.test(report.nativeHdrDerivedSha256), "native HDR report is malformed")
  return {
    path: path.join(config.sourceCacheDir, "browser-bundles", `${target}.native-hdr.psmp`),
    bytes: report.nativeHdrBytes as number,
    sha256: report.nativeHdrSha256,
    derivedSha256: report.nativeHdrDerivedSha256,
  }
}

export async function verifyTf2Wasm(
  config: LocalConfig,
  identity: string | undefined,
): Promise<Record<string, number | string>> {
  const map = await acquireMap(config, identity)
  const wasmPath = await buildTf2Wasm(config, false)
  const sourceBundle = await buildSourceBundle(config, identity ?? "")
  const nativeHdr = await buildNativeHdr(config, identity ?? "")
  const wasmBytes = await readFile(wasmPath)
  require(wasmBytes.byteLength > 0 && wasmBytes.byteLength <= 64 * 1024 * 1024, "WASM byte length is invalid")
  const loaded=await WebAssembly.instantiate(wasmBytes,{playsrc_metrics:{monotonic_milliseconds:()=>performance.now()}})
  const exports = loaded.instance.exports as unknown as Exports
  const [bspBytes, nativeHdrPayload, chunks] = await Promise.all([
    readFile(path.join(config.sourceCacheDir, map.decoded.cachePath)),
    readFile(nativeHdr.path),
    Promise.all(sourceBundle.graph.chunks.filter((descriptor) => descriptor.roles.includes("gameplay")).map(async (descriptor) => Object.freeze({
      descriptor,
      bytes: await readFile(path.join(sourceBundle.graphObjectDirectory, descriptor.encodedSha256)),
    }))),
  ])
  const batch = encodeResourceBatch(chunks)
  const batchPointer = exports.playsrc_alloc(batch.byteLength) >>> 0
  new Uint8Array(exports.memory.buffer, batchPointer, batch.byteLength).set(batch)
  require(exports.playsrc_resource_decode(batchPointer, batch.byteLength) === 1, "resource graph decoding failed")
  exports.playsrc_free(batchPointer, batch.byteLength)
  const dependencyBytes = new Uint8Array(exports.playsrc_resource_length())
  const resourcePointer = exports.playsrc_resource_take() >>> 0
  require(resourcePointer !== 0, "resource set ownership transfer failed")
  dependencyBytes.set(new Uint8Array(exports.memory.buffer, resourcePointer, dependencyBytes.byteLength))
  require(exports.playsrc_resource_release(resourcePointer, dependencyBytes.byteLength) === 1, "resource source release failed")
  require(bspBytes.byteLength === map.decoded.byteLength, "cached BSP byte length changed")
  require(dependencyBytes.byteLength > 0 && dependencyBytes.byteLength <= 1024 * 1024 * 1024, "resource set byte length changed")

  const resourceTable = (pointer: number, bytes: Uint8Array): Readonly<{ table: number; hash: number }> => {
    const table = exports.playsrc_alloc(8) >>> 0
    const view = new DataView(exports.memory.buffer, table, 8)
    view.setUint32(0, pointer, true)
    view.setUint32(4, bytes.byteLength, true)
    const hash = exports.playsrc_alloc(32) >>> 0
    new Uint8Array(exports.memory.buffer, hash, 32).set(new Bun.CryptoHasher("sha256").update(bytes).digest())
    return Object.freeze({ table, hash })
  }

  const compileProfile = (profile: 0 | 1) => {
    const source = exports.playsrc_alloc(bspBytes.byteLength) >>> 0
    new Uint8Array(exports.memory.buffer, source, bspBytes.byteLength).set(bspBytes)
    const configuration = exports.playsrc_alloc(dependencyBytes.byteLength) >>> 0
    new Uint8Array(exports.memory.buffer, configuration, dependencyBytes.byteLength).set(dependencyBytes)
    const sections = resourceTable(configuration, dependencyBytes)
    const result = exports.playsrc_compile_map(
      source,
      bspBytes.byteLength,
      profile,
      sections.table,
      1,
      sections.hash,
    )
    exports.playsrc_free(source, bspBytes.byteLength)
    exports.playsrc_free(sections.table, 8)
    exports.playsrc_free(sections.hash, 32)
    exports.playsrc_free(configuration, dependencyBytes.byteLength)
    const error = exports.playsrc_result_error(result)
    require(error === 0, `TF2 WASM profile ${profile} compilation failed with error ${error}`)
    const length = exports.playsrc_result_length(result)
    require(length > 0 && length <= 512 * 1024 * 1024, `profile ${profile} payload length is invalid`)
    const pointer = exports.playsrc_alloc(length) >>> 0
    require(exports.playsrc_result_copy(result, pointer, length) === length, `profile ${profile} payload copy failed`)
    const payload = new Uint8Array(exports.memory.buffer, pointer, length).slice()
    exports.playsrc_free(pointer, length)
    const hashPointer = exports.playsrc_alloc(32) >>> 0
    require(exports.playsrc_result_hash(result, hashPointer) === 1, `profile ${profile} payload hash is unavailable`)
    const sha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
    require(new Bun.CryptoHasher("sha256").update(payload).digest("hex") ===
      sha256, `profile ${profile} declared payload hash differs from its bytes`)
    require(exports.playsrc_result_derived_hash(result, hashPointer) ===
      1, `profile ${profile} derived hash is unavailable`)
    const derivedSha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
    exports.playsrc_free(hashPointer, 32)
    return { handle: result, payload, sha256, derivedSha256 }
  }
  const compileFailure = (sourceBytes: Uint8Array, profile: number, configBytes: Uint8Array) => {
    const source = exports.playsrc_alloc(sourceBytes.byteLength)
    new Uint8Array(exports.memory.buffer, source, sourceBytes.byteLength).set(sourceBytes)
    const configuration = exports.playsrc_alloc(configBytes.byteLength)
    new Uint8Array(exports.memory.buffer, configuration, configBytes.byteLength).set(configBytes)
    const sections = resourceTable(configuration, configBytes)
    const result = exports.playsrc_compile_map(
      source,
      sourceBytes.byteLength,
      profile,
      sections.table,
      1,
      sections.hash,
    )
    exports.playsrc_free(source, sourceBytes.byteLength)
    exports.playsrc_free(sections.table, 8)
    exports.playsrc_free(sections.hash, 32)
    exports.playsrc_free(configuration, configBytes.byteLength)
    const error = exports.playsrc_result_error(result)
    require(exports.playsrc_dispose(result) === 1, "failed compilation handle disposal failed")
    return error
  }

  if (identity === "ctf_2fort") {
    const hdr = compileProfile(1)
    require(hdr.payload.byteLength === nativeHdr.bytes, "ctf_2fort native/WASM payload length differs")
    require(hdr.sha256 === nativeHdr.sha256, "ctf_2fort native/WASM payload SHA-256 differs")
    require(hdr.derivedSha256 === nativeHdr.derivedSha256, "ctf_2fort native/WASM derived identity differs")
    const runtime = parseRuntimeMap(hdr.payload)
    require(runtime.schema === 10 && runtime.displacementSurfaces === 232, "ctf_2fort displacement runtime coverage differs")
    require(runtime.materials.every((material) => !material.baseTexture || !Object.hasOwn(material.baseTexture, "rgba")),
      "ctf_2fort world textures duplicate authored source planes")
    const presentationLength = exports.playsrc_presentation_length(hdr.handle)
    require(presentationLength > 0 && presentationLength <= 512 * 1024 * 1024, "ctf_2fort presentation length is invalid")
    const presentationPointer = exports.playsrc_alloc(presentationLength) >>> 0
    require(exports.playsrc_presentation_copy(hdr.handle, presentationPointer, presentationLength) === presentationLength,
      "ctf_2fort presentation copy failed")
    const presentation = new Uint8Array(exports.memory.buffer, presentationPointer, presentationLength).slice()
    exports.playsrc_free(presentationPointer, presentationLength)
    const artifacts = await parsePresentationArtifacts(presentation, parseResourceSet(dependencyBytes))
    require(artifacts.staticProps.count === 2265 && artifacts.staticProps.vhv.length === 4482,
      "ctf_2fort static-prop and accepted VHV coverage differs")
    require(artifacts.staticProps.runtimeLightingCount === 24,
      "ctf_2fort invalid or unavailable baked-lighting disposition differs")
    const sun = artifacts.modelMaterials.get("materials/models/props_skybox/sun_ray1.vmt")
    require(sun?.shader === "unlit-two-texture" && sun.bindings.filter((binding) => binding.kind === "material").length === 2,
      "ctf_2fort authored dual-texture sky model differs")
    require(artifacts.environment.waterMaterials.get("materials/water/water_2fort_beneath.vmt")?.underwaterOverlay === "materials/effects/water_warp_2fort.vmt",
      "ctf_2fort underwater overlay identity differs")
    const overlay = artifacts.environment.refractMaterials.get("materials/effects/water_warp_2fort.vmt")
    require(overlay?.normal.logicalPath === "materials/water/tfwater001_normal.vtf"
      && overlay.blurAmount === 1 && overlay.ignoreDepth
      && overlay.refractAmount === Math.fround(0.05)
      && overlay.refractTint.every((value, index) => value === Math.fround([185, 215, 245][index]! * Math.fround(1 / 255))),
    "ctf_2fort authored underwater Refract material differs")
    require(exports.playsrc_dispose(hdr.handle) === 1, "ctf_2fort HDR handle disposal failed")
    return Object.freeze({
      target: identity,
      bspBytes: bspBytes.byteLength,
      dependencyBytes: dependencyBytes.byteLength,
      payloadBytes: hdr.payload.byteLength,
      payloadSha256: hdr.sha256,
      derivedSha256: hdr.derivedSha256,
      displacementSurfaces: runtime.displacementSurfaces,
      drawableSurfaces: runtime.drawableSurfaces,
      drawBatches: runtime.batches.length,
      staticProps: artifacts.staticProps.count,
      runtimeLitProps: artifacts.staticProps.runtimeLightingCount,
      presentationBytes: presentationLength,
    })
  }

  if (identity === "pl_upward") {
    const hdr = compileProfile(1)
    require(hdr.payload.byteLength === nativeHdr.bytes, "pl_upward native/WASM payload length differs")
    require(hdr.sha256 === nativeHdr.sha256, "pl_upward native/WASM payload SHA-256 differs")
    require(hdr.derivedSha256 === nativeHdr.derivedSha256, "pl_upward native/WASM derived identity differs")
    const runtime = parseRuntimeMap(hdr.payload)
    require(runtime.schema === 10 && runtime.displacementSurfaces === 558, "pl_upward displacement runtime coverage differs")
    const wall = runtime.materials.find((material) => material.logicalPath.toLowerCase() === "materials/brick/wall028.vmt")
    require(wall?.detail?.texture.logicalPath.toLowerCase() === "materials/overlays/detail001.vtf"
      && wall.detail.scale[0] === Math.fround(1.1) && wall.detail.scale[1] === Math.fround(2.3)
      && wall.detail.blendMode === 0 && wall.detail.blendFactor === 1,
    "pl_upward wall detail state differs")
    const transitions = runtime.materials.filter((material) => material.shader === 4)
    require(transitions.length > 0 && transitions.every((material) => material.secondTexture),
      "pl_upward WorldVertexTransition second-texture closure differs")
    const presentationLength = exports.playsrc_presentation_length(hdr.handle)
    require(presentationLength > 0 && presentationLength <= 512 * 1024 * 1024, "pl_upward presentation length is invalid")
    const presentationPointer = exports.playsrc_alloc(presentationLength) >>> 0
    require(exports.playsrc_presentation_copy(hdr.handle, presentationPointer, presentationLength) === presentationLength,
      "pl_upward presentation copy failed")
    const presentation = new Uint8Array(exports.memory.buffer, presentationPointer, presentationLength).slice()
    exports.playsrc_free(presentationPointer, presentationLength)
    const artifacts = await parsePresentationArtifacts(presentation, parseResourceSet(dependencyBytes))
    const upwardWater = artifacts.environment.worldMaterials.get("materials/maps/pl_upward/water/water_hydro_cheap_dx80_7168_-2048_128.vmt")
    const upwardNormal = artifacts.environment.authoredTextures.get("materials/water/dx80_tfwater001_normal.vtf")
    require(upwardWater?.shader === "lightmapped-generic"
      && upwardWater.proxies.length === 1
      && upwardWater.proxies[0]?.name === "AnimatedTexture"
      && upwardWater.environmentMap?.tint.every((value) => value === Math.fround(0.2))
      && upwardWater.textures.find((texture) => texture.role === 7)?.frameProxyMutated === true
      && upwardNormal?.frameCount === 30 && upwardNormal.mipCount === 9
      && upwardNormal.planes.length === 270,
    "pl_upward authored animated LightmappedGeneric state differs")
    const banner = artifacts.modelMaterials.get("materials/models/props_ui/bannerflag_comp.vmt")
    require(banner?.shader === "unlit-generic" && banner.state.kind === "unlit-generic"
      && banner.vertexRequirements === 9
      && !banner.requiredInputs.includes("ambient-cube") && !banner.requiredInputs.includes("local-lights"),
    "pl_upward banner UnlitGeneric model state differs")
    require(exports.playsrc_dispose(hdr.handle) === 1, "pl_upward HDR handle disposal failed")
    return Object.freeze({
      target: identity,
      bspBytes: bspBytes.byteLength,
      dependencyBytes: dependencyBytes.byteLength,
      payloadBytes: hdr.payload.byteLength,
      payloadSha256: hdr.sha256,
      derivedSha256: hdr.derivedSha256,
      displacementSurfaces: runtime.displacementSurfaces,
      drawableSurfaces: runtime.drawableSurfaces,
      drawBatches: runtime.batches.length,
      worldVertexTransitions: transitions.length,
      presentationBytes: presentationLength,
    })
  }

  const bspPointer = exports.playsrc_alloc(bspBytes.byteLength)
  new Uint8Array(exports.memory.buffer, bspPointer, bspBytes.byteLength).set(bspBytes)
  const dependencyPointer = exports.playsrc_alloc(dependencyBytes.byteLength)
  new Uint8Array(exports.memory.buffer, dependencyPointer, dependencyBytes.byteLength).set(dependencyBytes)
  const dependencySections = resourceTable(dependencyPointer, dependencyBytes)
  const handle = exports.playsrc_compile_map(
    bspPointer,
    bspBytes.byteLength,
    0,
    dependencySections.table,
    1,
    dependencySections.hash,
  )
  exports.playsrc_free(bspPointer, bspBytes.byteLength)
  exports.playsrc_free(dependencySections.table, 8)
  exports.playsrc_free(dependencySections.hash, 32)
  exports.playsrc_free(dependencyPointer, dependencyBytes.byteLength)
  const error = exports.playsrc_result_error(handle)
  require(error === 0, `TF2 WASM map compilation failed with error ${error}`)
  const presentationBytes = exports.playsrc_presentation_length(handle)
  require(presentationBytes > 0 && presentationBytes <= 512 * 1024 * 1024, "TF2 presentation byte length is invalid")
  const presentationPointer = exports.playsrc_alloc(presentationBytes)
  require(exports.playsrc_presentation_copy(handle, presentationPointer, presentationBytes) === presentationBytes,
    "TF2 presentation output copy failed")
  const presentation = new Uint8Array(exports.memory.buffer, presentationPointer, presentationBytes).slice()
  exports.playsrc_free(presentationPointer, presentationBytes)
  const presentationArtifacts = await parsePresentationArtifacts(presentation, parseResourceSet(dependencyBytes))
  const particleMaterialIdentities = [
    "effects/brightglow_y_nomodel.vmt",
    "effects/circle2.vmt",
    "effects/circle3.vmt",
    "effects/circle4.vmt",
    "effects/debris/debris_chunk.vmt",
    "effects/rocketrailsmoke.vmt",
    "effects/sc_brightglow_y_nomodel.vmt",
    "effects/sc_softglow.vmt",
    "effects/smokelit2/smoke2lit.vmt",
    "effects/softglow.vmt",
    "effects/softglow_translucent.vmt",
    "effects/starflash01.vmt",
    "effects/wispy_smoke.vmt",
    "particle/smoke1/smoke1.vmt",
  ] as const
  const suppliedParticleMaterials = [...presentationArtifacts.particleMaterials].sort()
  require(particleMaterialIdentities.every((identity) => suppliedParticleMaterials.includes(identity))
    && new Set(suppliedParticleMaterials).size === suppliedParticleMaterials.length
    && presentationArtifacts.particleTextures.length === suppliedParticleMaterials.length
    && presentationArtifacts.particleTextures.every((texture) => suppliedParticleMaterials.includes(texture.material)),
  `TF2 Particle material/texture identities differ: ${JSON.stringify(suppliedParticleMaterials)}`)
  const particleMaterialStates = particleMaterialIdentities.map((identity) => {
    const state = presentationArtifacts.materialStates.get(identity)
    require(state !== undefined, `TF2 Particle material state ${identity} is missing`)
    return Object.freeze({ identity, state })
  })
  const spriteCards = new Set([
    "effects/circle3.vmt", "effects/circle4.vmt", "effects/debris/debris_chunk.vmt",
    "effects/rocketrailsmoke.vmt", "effects/sc_brightglow_y_nomodel.vmt", "effects/sc_softglow.vmt",
    "effects/smokelit2/smoke2lit.vmt", "particle/smoke1/smoke1.vmt",
  ])
  require(particleMaterialStates.filter(({ identity }) => spriteCards.has(identity)).every(({ state }) =>
    state.alphaTest && Math.abs(state.alphaTestReference - 0.01) < 1e-6 && state.cull === 1
    && state.depthTest && !state.depthWrite && state.fragmentDiscard.kind === "alpha"
    && state.fragmentDiscard.source === "shader-output" && state.fragmentDiscard.pass === "greater"),
  "TF2 SpriteCard material state differs")
  const particleMaterialStateSha256 = new Bun.CryptoHasher("sha256")
    .update(new TextEncoder().encode(JSON.stringify(particleMaterialStates))).digest("hex")
  require(particleMaterialStateSha256 === EXPECTED_PARTICLE_MATERIAL_STATE_SHA256,
    "TF2 Particle material-state hash differs")
  require(presentationArtifacts.environment.markRecords.length === 39 &&
    presentationArtifacts.environment.waterVolumeFacts.length === 1 &&
    presentationArtifacts.environment.waterMaterials.size === 2,
  "TF2 complete environment presentation differs")
  const coverageLength=exports.playsrc_coverage_length(handle),coveragePointer=exports.playsrc_alloc(coverageLength)
  require(coverageLength===12+278*24&&exports.playsrc_coverage_copy(handle,coveragePointer,coverageLength)===coverageLength,"TF2 map coverage framing differs")
  const coverage=new Uint8Array(exports.memory.buffer,coveragePointer,coverageLength).slice(),coverageView=new DataView(coverage.buffer);exports.playsrc_free(coveragePointer,coverageLength)
  require(new TextDecoder().decode(coverage.subarray(0,4))==="PCOV"&&coverageView.getUint32(4,true)===1&&coverageView.getUint32(8,true)===278,"TF2 map coverage identity differs")
  let previousLeaf=-1;for(let index=0;index<278;index++){const at=12+index*24,leaf=coverageView.getUint32(at,true),position=[coverageView.getFloat32(at+8,true),coverageView.getFloat32(at+12,true),coverageView.getFloat32(at+16,true)];require(leaf>previousLeaf&&coverageView.getInt16(at+4,true)>=0&&coverageView.getUint32(at+20,true)===0&&position.every(Number.isFinite),"TF2 map coverage record differs");previousLeaf=leaf}
  const spawnPointer = exports.playsrc_alloc(40)
  require(exports.playsrc_spawn_copy(handle, spawnPointer, 40) === 40, "TF2 spawn descriptor is unavailable")
  const spawnBytes = new Uint8Array(exports.memory.buffer, spawnPointer, 40).slice()
  exports.playsrc_free(spawnPointer, 40)
  const spawnView = new DataView(spawnBytes.buffer)
  require(new TextDecoder().decode(spawnBytes.subarray(0, 4)) === "PSIV", "TF2 spawn descriptor magic is invalid")
  require(spawnView.getUint32(4, true) === 1, "TF2 spawn descriptor version is invalid")
  const spawn = {
    entity: spawnView.getUint32(8, true),
    hammerId: spawnView.getUint32(12, true),
    position: Array.from({ length: 3 }, (_, index) => spawnView.getFloat32(16 + index * 4, true)),
    angles: Array.from({ length: 3 }, (_, index) => spawnView.getFloat32(28 + index * 4, true)),
  }
  require(spawn.entity === 1 &&
    spawn.hammerId === 29 &&
    spawn.position.every((value, index) => value === [5328, 3376, -3120][index]) &&
    spawn.angles.every(
      (value, index) => value === [-1, 180, 0][index],
    ), "TF2 spawn descriptor differs from the selected teamspawn")
  const mapBytes = exports.playsrc_result_length(handle)
  require(mapBytes === EXPECTED_MAP_BYTES, `map payload length ${mapBytes} != ${EXPECTED_MAP_BYTES}`)
  const hashPointer = exports.playsrc_alloc(32)
  require(exports.playsrc_result_hash(handle, hashPointer) === 1, "map payload hash is unavailable")
  const declaredMapSha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
  require(exports.playsrc_result_derived_hash(handle, hashPointer) === 1, "map derived hash is unavailable")
  const ldrDerivedSha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
  exports.playsrc_free(hashPointer, 32)
  const mapPointer = exports.playsrc_alloc(mapBytes)
  require(exports.playsrc_result_copy(handle, mapPointer, mapBytes) === mapBytes, "map payload copy failed")
  const mapPayload = new Uint8Array(exports.memory.buffer, mapPointer, mapBytes).slice()
  const hasher = new Bun.CryptoHasher("sha256")
  hasher.update(mapPayload)
  const mapSha256 = hasher.digest("hex")
  exports.playsrc_free(mapPointer, mapBytes)
  require(declaredMapSha256 === mapSha256, "declared map payload hash does not match its bytes")
  require(mapSha256 === EXPECTED_MAP_SHA256, `map payload SHA-256 ${mapSha256} != ${EXPECTED_MAP_SHA256}`)
  const renderMap = parseRuntimeMap(mapPayload)
  require(renderMap.materials.length === 14, "runtime map material count is invalid")
  require(renderMap.drawableSurfaces === 2_761, "runtime map drawable world-surface count is invalid")
  require(renderMap.batches.length === 10, "runtime map draw-batch count is invalid")
  const resolvedTextures = renderMap.materials.filter((material) => material.baseTexture).length
  require(resolvedTextures === 12, "runtime map resolved-texture count is invalid")
  let alignedTriangles = 0
  let opposedTriangles = 0
  let degenerateTriangles = 0
  for (const batch of renderMap.batches) {
    for (let offset = 0; offset < batch.indices.length; offset += 3) {
      const indexes = [batch.indices[offset]!, batch.indices[offset + 1]!, batch.indices[offset + 2]!]
      const position = (index: number) =>
        [batch.positions[index * 3]!, batch.positions[index * 3 + 1]!, batch.positions[index * 3 + 2]!] as const
      const a = position(indexes[0])
      const b = position(indexes[1])
      const c = position(indexes[2])
      const ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]] as const
      const ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]] as const
      const cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
      ] as const
      const normal = indexes.reduce(
        (sum, index) =>
          [
            sum[0] + batch.normals[index * 3]!,
            sum[1] + batch.normals[index * 3 + 1]!,
            sum[2] + batch.normals[index * 3 + 2]!,
          ] as const,
        [0, 0, 0] as readonly [number, number, number],
      )
      const facing = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2]
      if (Math.abs(facing) <= 1e-6) degenerateTriangles += 1
      else if (facing > 0) alignedTriangles += 1
      else opposedTriangles += 1
    }
  }
  require(alignedTriangles === 6_497 &&
    opposedTriangles === 0 &&
    degenerateTriangles ===
      0, `runtime triangle orientation is ${alignedTriangles} aligned, ${opposedTriangles} opposed, ${degenerateTriangles} degenerate`)
  require(renderMap.models.length === 56, `runtime model count ${renderMap.models.length} is invalid`)
  require(renderMap.modelOccurrences.length === 33, "runtime model occurrence count is invalid")
  require(renderMap.lightmap !== undefined, "runtime lightmap atlas is unavailable")
  const teleports = exports.playsrc_teleport_count(handle)
  const teleportDestinations = exports.playsrc_teleport_destination_count(handle)
  require(teleports === 56, "runtime map teleport count is invalid")
  require(teleportDestinations === 25, "runtime map teleport-destination count is invalid")
  require(exports.playsrc_runtime_count(handle, 9) === 22, "runtime map regenerate-zone count is invalid")

  const commandBytes = new Uint8Array(
    encodeCommand({
      forward: 0,
      side: 0,
      yawDegrees: 0,
      pitchDegrees: 0,
      jump: false,
      crouch: false,
      fire: true,
      detonate: false,
    }),
  )
  const commandPointer = exports.playsrc_alloc(commandBytes.byteLength)
  new Uint8Array(exports.memory.buffer, commandPointer, commandBytes.byteLength).set(commandBytes)
  require(exports.playsrc_game_advance(handle, commandPointer, commandBytes.byteLength, 64) === 1, "64-tick gameplay phase failed")
  const snapshotLength = exports.playsrc_snapshot_length(handle)
  require(snapshotLength >= 56, "snapshot is shorter than its fixed fields")
  const snapshotPointer = exports.playsrc_alloc(snapshotLength)
  require(exports.playsrc_snapshot_copy(handle, snapshotPointer, snapshotLength) ===
    snapshotLength, "snapshot copy failed")
  const snapshot = new Uint8Array(exports.memory.buffer, snapshotPointer, snapshotLength).slice()
  const decoded = decodeSnapshot(snapshot.buffer)
  require(decoded.tick === 64n, "snapshot tick is invalid")
  require(decoded.projectileEvents.some((event) => event.type === "fire"), "fixed phase omitted fire event")
  const initialStockFire=decoded.projectileEvents.find(event=>event.type==="fire"&&event.kind===1),initialEye=[decoded.position[0]+decoded.movement.viewOffset[0],decoded.position[1]+decoded.movement.viewOffset[1],decoded.position[2]+decoded.movement.viewOffset[2]]
  require(!!initialStockFire&&Math.abs(initialStockFire.position[1]-initialEye[1]!+12)<0.05,"stock rocket source side differs")
  require(!!initialStockFire?.launcherPose&&initialStockFire.launcherPose.eyePosition.every((value,index)=>Math.abs(value-initialEye[index]!)<0.001)&&initialStockFire.launcherPose.viewOrientation.every((value,index)=>Math.abs(value-[0,0,0,1][index]!)<0.001),"stock rocket authoritative fire-tick launcher pose differs")
  require(decoded.authorityBlockers.map((blocker) => blocker.code).join(",") === "1,2",
    "authority blocker ledger differs")
  require(decoded.jump === null, "unavailable Tempus course was inferred")
  require(decoded.rocketTraceRequests.length > 0, "rocket Collision request seam is empty")
  require(decoded.entityPresentation.collisionRevision===decoded.collisionSnapshot.identity&&decoded.entityPresentation.models.length===122&&decoded.entityPresentation.models.filter(model=>model.draw).length===17,"Entity brush presentation revision join differs")

  new DataView(exports.memory.buffer, commandPointer, commandBytes.byteLength).setFloat32(4, Number.NaN, true)
  require(exports.playsrc_game_advance(handle, commandPointer, commandBytes.byteLength, 1) === 0, "malformed command was accepted")
  require(exports.playsrc_snapshot_length(handle) === snapshotLength, "rejected command replaced the snapshot")
  const unchangedPointer = exports.playsrc_alloc(snapshotLength)
  require(exports.playsrc_snapshot_copy(handle, unchangedPointer, snapshotLength) ===
    snapshotLength, "unchanged snapshot copy failed")
  require(Buffer.from(exports.memory.buffer, unchangedPointer, snapshotLength).equals(
    snapshot,
  ), "rejected command mutated the snapshot")
  const copySnapshot = () => {
    const length = exports.playsrc_snapshot_length(handle)
    const pointer = exports.playsrc_alloc(length)
    require(exports.playsrc_snapshot_copy(handle, pointer, length) === length, "gameplay trace snapshot copy failed")
    const bytes = new Uint8Array(exports.memory.buffer, pointer, length).slice()
    exports.playsrc_free(pointer, length)
    return decodeSnapshot(bytes.buffer)
  }
  let advanceOrdinal = 0
  const advance = (command: ReturnType<typeof encodeCommand>, ticks: number) => {
    advanceOrdinal += 1
    const bytes = new Uint8Array(command)
    const pointer = exports.playsrc_alloc(bytes.byteLength)
    new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(bytes)
    const accepted = exports.playsrc_game_advance(handle, pointer, bytes.byteLength, ticks)
    exports.playsrc_free(pointer, bytes.byteLength)
    require(accepted === 1, `fixed gameplay transaction ${advanceOrdinal} failed with ${exports.playsrc_result_error(handle)}`)
    return copySnapshot()
  }
  const button = advance(encodeCommand({
    forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false,
    fire: false, detonate: false, activateEntity: 213,
  }), 1)
  const moverRequest = button.moverRequests.find((request) => request.entity === 213)
  const moverTransform = button.entityTransforms.find((transform) => transform.identity === 213)
  require(moverRequest && moverTransform, "fixed button did not publish its mover request/transform")
  const moverTicks = Math.ceil(Math.hypot(...moverRequest.destination.map((value, axis) => value - moverRequest.start[axis]!)) /
    moverRequest.speed / 0.015) + 1
  let moved = button
  let moverCompleted = false
  let moverCompletionEvent = false
  for (let remaining = moverTicks; remaining > 0;) {
    const batch = Math.min(64, remaining)
    moved = advance(encodeCommand({
      forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false,
      fire: false, detonate: false,
    }), batch)
    moverCompleted ||= moved.moverResults.some((result) => result.requestId === moverRequest.requestId && result.kind === 2)
    moverCompletionEvent ||= moved.entityEvents.some((event) => event.entity === 213 && event.kind === 7)
    remaining -= batch
  }
  require(moverCompletionEvent && moverCompleted &&
    moved.entityTransforms.some((transform) => transform.identity === 213 &&
      transform.position.every((value, index) => value === moverRequest.destination[index])),
  "fixed mover completion did not publish Entity completion and destination transform")
  const neutralCommand = (extra: Partial<Parameters<typeof encodeCommand>[0]> = {}) => encodeCommand({
    forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 89, jump: false, crouch: false,
    fire: false, detonate: false, ...extra,
  })
  const resolveLatestRocket = (fired: ReturnType<typeof copySnapshot>, crouch: boolean) => {
    const fire = fired.projectileEvents.filter((event) => event.type === "fire" && event.kind === 1).at(-1)
    const request = fire && fired.rocketTraceRequests.filter((value) => value.projectile === fire.projectile).at(-1)
    require(fire && request, "fixed rocket fire did not publish a Collision request")
    let resolved = fired
    for (let tick = 0; tick < 16 && !resolved.projectileEvents.some((event) => event.projectile === request.projectile && event.type === "explode"); tick++) {
      resolved = advance(neutralCommand({ crouch }), 1)
    }
    const impulse = resolved.events.find((event) => event.kind === 8)
    require(impulse && resolved.radiusDamageRequests.some((value) => value.projectile === request.projectile) &&
      resolved.rocketTraceResults.some((value) => value.projectile === request.projectile && value.solid),
      "fixed rocket result did not publish blast force and radius damage")
    return { resolved, impulse }
  }
  advance(neutralCommand(), 42)
  const standingReady = copySnapshot()
  require(standingReady.grounded && !standingReady.crouched, "standing rocket trace did not begin grounded and standing")
  const standingFire = advance(neutralCommand({ fire: true }), 1)
  const standingBlast = resolveLatestRocket(standingFire, false)
  advance(neutralCommand({ respawn: true }), 1)
  advance(neutralCommand(), 64)
  const crouchedReady = advance(neutralCommand({ crouch: true }), 16)
  require(crouchedReady.grounded && crouchedReady.crouched && crouchedReady.movement.crouchFraction === 1,
    "crouched rocket trace did not begin grounded and fully crouched")
  const crouchedFire = advance(neutralCommand({ crouch: true, fire: true }), 1)
  const crouchedBlast = resolveLatestRocket(crouchedFire, true)
  const standingBlastVelocity = Math.hypot(...standingBlast.impulse.values.slice(0, 3))
  const crouchedBlastVelocity = Math.hypot(...crouchedBlast.impulse.values.slice(0, 3))
  require(crouchedBlastVelocity > standingBlastVelocity,
    `crouched rocket blast velocity ${crouchedBlastVelocity} did not exceed standing ${standingBlastVelocity}`)
  let travel = advance(neutralCommand({ modeRequest: 1 }), 1)
  const regenerateCenter = [2324, 3048, -3032] as const
  let resupplied: typeof travel | undefined
  for (let batch = 0; batch < 32 && !resupplied; batch++) {
    const delta = regenerateCenter.map((value, axis) => value - travel.position[axis]!) as [number, number, number]
    const horizontal = Math.hypot(delta[0], delta[1])
    const distance = Math.hypot(...delta)
    if (distance < 1) break
    const yawDegrees = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
    const forward = horizontal > 0.001 ? 450 : 0
    const up = Math.max(-450, Math.min(450, horizontal > 0.001 ? delta[2] / horizontal * 450 : Math.sign(delta[2]) * 450))
    const estimatedTicks = Math.max(1, Math.min(64, Math.ceil(distance / 3.5)))
    travel = advance(encodeCommand({
      forward, side: 0, up, yawDegrees, pitchDegrees: 0, jump: false, crouch: false,
      fire: false, detonate: false,
    }), estimatedTicks)
    if (travel.events.some((event) => event.kind === 5 && event.subject === 151)) resupplied = travel
  }
  require(resupplied?.events.some((event) => event.kind === 5 && event.subject === 151) &&
    resupplied.loadout.find((weapon) => weapon.weapon === 1)?.clip === 4 && resupplied.health === resupplied.maximumHealth,
  "fixed regenerate-volume trace did not restore health and Rocket Launcher resources")
  require(resupplied?.regenerateAnimationEvents.some(event=>event.associatedModel===315&&event.body===0&&event.openAnimation==="open"&&event.closeAnimation==="close"),"configured regenerate locker animation output differs")
  const brushCollision=collisionBrushRecords((resupplied??travel).collisionSnapshot.bytes)
  require(brushCollision.get(294n)?.enabled===true&&brushCollision.get(294n)?.model===109&&brushCollision.get(294n)?.contents===1,
    "configured model-109 divider collision is absent")
  for(const [entity,model] of [[307n,113],[322n,117],[323n,118]] as const){const record=brushCollision.get(entity);require(record?.enabled===false&&record.model===model&&record.contents===0x10000008,`configured never-solid fence ${entity} differs`)}
  const heldCommand=encodeCommand({forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:true,detonate:false}),stockOriginState=resupplied??travel;let heldFire=stockOriginState;const stockFires:typeof heldFire.projectileEvents[number][]=[]
  for(let batch=0;batch<4;batch++){heldFire=advance(heldCommand,55);stockFires.push(...heldFire.projectileEvents.filter(event=>event.type==="fire"&&event.kind===1))}
  const stockIntervals=stockFires.slice(1).map((event,index)=>event.tick-stockFires[index]!.tick)
  require(stockFires.length>=4&&stockIntervals.every(value=>value>=53n&&value<=54n),`held Rocket Launcher cadence differs: ${stockFires.map(event=>event.tick).join(",")}`)
  advance(encodeCommand({forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:false,detonate:false,selectWeapon:2}),1)
  const originalReady=advance(neutralCommand(),35),originalEye=[originalReady.position[0]+originalReady.movement.viewOffset[0],originalReady.position[1]+originalReady.movement.viewOffset[1],originalReady.position[2]+originalReady.movement.viewOffset[2]]
  const originalFire=advance(encodeCommand({forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:true,detonate:false}),1).projectileEvents.find(event=>event.type==="fire"&&event.kind===1)
  require(!!originalFire&&Math.abs(originalFire.position[1]-originalEye[1]!)<0.05,"Original rocket source is not centered")
  const definition = new TextEncoder().encode("rockettrail"),
    particleBatch = new Uint8Array(108 + definition.length),
    particleView = new DataView(particleBatch.buffer)
  particleBatch.set([0x50, 0x50, 0x54, 0x58])
  particleView.setUint32(4, 5, true)
  particleView.setFloat32(12, 0.1, true)
  ;[5328, 3376, -3052].forEach((value, index) => particleView.setFloat32(16 + index * 4, value, true))
  particleView.setUint32(28, 1, true)
  particleBatch[32] = 1
  particleBatch[34] = 1
  particleView.setBigUint64(36, 1n, true)
  particleView.setUint32(48, 1, true)
  particleView.setBigUint64(52, 7n, true)
  particleView.setUint32(60, 1, true)
  particleView.setUint32(64, definition.length, true)
  particleBatch.set(definition, 68)
  let particleAt = 68 + definition.length
  ;[5328, 3376, -3052, 0, 0, 0, 1].forEach((value, index) =>
    particleView.setFloat32(particleAt + index * 4, value, true),
  )
  particleView.setUint32(particleAt + 28, 1, true)
  const particlePointer = exports.playsrc_alloc(particleBatch.length)
  new Uint8Array(exports.memory.buffer, particlePointer, particleBatch.length).set(particleBatch)
  require(exports.playsrc_particle_transact(handle, particlePointer, particleBatch.length) ===
    1, "configured rockettrail particle transaction failed")
  require(exports.playsrc_particle_output_length(handle) > 12, "configured rockettrail produced no render data")
  const particleOutputLength = exports.playsrc_particle_output_length(handle)
  const particleOutputPointer = exports.playsrc_alloc(particleOutputLength)
  require(exports.playsrc_particle_output_copy(handle, particleOutputPointer, particleOutputLength) === particleOutputLength,
    "configured rockettrail render output copy failed")
  const particleOutput = new Uint8Array(exports.memory.buffer, particleOutputPointer, particleOutputLength).slice()
  const particleOutputView = new DataView(particleOutput.buffer)
  require(new TextDecoder().decode(particleOutput.subarray(0, 4)) === "PSPR" &&
    particleOutputView.getUint32(4,true)===5&&particleOutputView.getUint32(8,true)>0,
  "configured rockettrail render output identity differs")
  require((particleOutputView.getUint32(40+124,true)&1)!==0,
    "configured rockettrail render output omitted its primary sheet sample")

  const modelRequest = Object.freeze({
    identity: 1,
    model: "models/weapons/c_models/c_soldier_arms.mdl",
    itemModel: "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
    activity: "ACT_PRIMARY_VM_DRAW",
    previousElapsedSeconds: 0,
    elapsedSeconds: 0.4,
    currentTimeSeconds:0.4,frameTimeSeconds:0.015,planarSpeed:0,screenAspectRatio:16/9,worldFarPlane:32768,
    phase: 0 as const,
    reflectedViewmodel: false,
    ownerAlive: true,
    skin: 0,
    lod: 0,
    bodygroups: [0],
    itemBodygroups: [0],
  })
  const modelBatch = encodeModelPoseBatch([
    {...modelRequest,sampleTick:3n,attachmentsOnly:true,fireView:{eyePosition:[10,20,30],viewOrientation:[0,0,0,1]}},
    {...modelRequest,sampleTick:4n,attachmentsOnly:true,fireView:{eyePosition:[110,20,30],viewOrientation:[0,0,0,1]}},
    {...modelRequest,sampleTick:5n},
  ])
  const modelPointer = exports.playsrc_alloc(modelBatch.byteLength)
  new Uint8Array(exports.memory.buffer, modelPointer, modelBatch.byteLength).set(modelBatch)
  require(exports.playsrc_model_transact(handle, modelPointer, modelBatch.byteLength) === 1,
    "fixed StudioModel viewmodel pose phase failed")
  const modelOutputLength = exports.playsrc_model_output_length(handle)
  const modelOutputPointer = exports.playsrc_alloc(modelOutputLength)
  require(exports.playsrc_model_output_copy(handle, modelOutputPointer, modelOutputLength) === modelOutputLength,
    "fixed StudioModel viewmodel pose output copy failed")
  const modelPoses = decodeModelPoseOutput(
    new Uint8Array(exports.memory.buffer, modelOutputPointer, modelOutputLength).slice(),
  )
  const firePoses=modelPoses.filter(pose=>pose.attachmentsOnly),displayPoses=modelPoses.filter(pose=>!pose.attachmentsOnly)
  require(firePoses.length===2&&firePoses.every((pose,index)=>pose.role==="item"&&pose.attachmentsWorld&&pose.primitives.length===0&&pose.sampleTick===BigInt(index+3)),"fixed StudioModel fire-tick attachment-only timeline differs")
  const firstMuzzle=firePoses[0]!.attachments.find(attachment=>attachment.name.toLowerCase()==="muzzle"),secondMuzzle=firePoses[1]!.attachments.find(attachment=>attachment.name.toLowerCase()==="muzzle")
  require(!!firstMuzzle&&!!secondMuzzle&&Math.abs(secondMuzzle.matrix[3]!-firstMuzzle.matrix[3]!-100)<0.001,"fixed StudioModel fire-tick launcher attachment poses differ")
  require(displayPoses.length===2&&displayPoses[0]?.role==="item"&&displayPoses[1]?.role==="hand"&&
    displayPoses[1].model === "models/weapons/c_models/c_soldier_arms.mdl" &&
    displayPoses[0].model === "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl" &&
    displayPoses.every((pose) => pose.sampleTick===5n&&pose.activity === "ACT_PRIMARY_VM_DRAW" && pose.primitives.length > 0 &&
      pose.boneMatrices.length > 0 && pose.boneMatrices.length % 12 === 0 &&
      pose.primitives.every((primitive) => primitive.vertexCount > 0)),
  "fixed StudioModel viewmodel pose output differs")
  const visibilityProbe=(values:readonly number[])=>{
    require(values.length===10,"fixed-camera visibility input differs")
    const expanded=[...values.slice(0,3),...values,-1]
    const pointer=exports.playsrc_alloc(56)
    new Float32Array(exports.memory.buffer,pointer,14).set(expanded)
    require(exports.playsrc_visibility_query(handle,pointer)===1,"fixed-camera PVS query failed")
    exports.playsrc_free(pointer,56)
    const length=exports.playsrc_visibility_output_length(handle),outputPointer=exports.playsrc_alloc(length)
    require(exports.playsrc_visibility_output_copy(handle,outputPointer,length)===length,"fixed-camera PVS output copy failed")
    const output=new Uint8Array(exports.memory.buffer,outputPointer,length).slice()
    exports.playsrc_free(outputPointer,length)
    const view=new DataView(output.buffer)
    require(new TextDecoder().decode(output.subarray(0,4))==="PVIS"&&view.getUint32(4,true)===5,"PVS output identity differs")
    let at=76,surfaceCount=view.getUint32(at,true);at+=4+surfaceCount*4
    const drawSurfaceCount=view.getUint32(at,true);at+=4+drawSurfaceCount*4
    at+=4;const leafCount=view.getUint32(at,true);at+=4+leafCount*4
    const areaCount=view.getUint32(at,true);at+=4+areaCount*4
    const flags=Array.from(output.subarray(at,at+8));at+=8
    let normalFrame:number|null=null
    if(flags[0]===1){at+=12+4+4+4;const textLength=view.getUint32(at,true);at+=4+textLength;normalFrame=view.getInt32(at,true);at+=4+64+8}
    const passCount=view.getUint32(at,true);at+=4;const passes:number[]=[]
    for(let index=0;index<passCount;index++){passes.push(output[at]!);at+=8+24+4+4+8;const count=view.getUint32(at,true);at+=4+count*4}
    const worldMaterialCount=view.getUint32(at,true);at+=4
    for(let index=0;index<worldMaterialCount;index++){const identityLength=view.getUint32(at,true);at+=4+identityLength+4;const textures=view.getUint32(at,true);at+=4+textures*(4+4+64)}
    require(at===output.length,"Water visibility output is truncated")
    return Object.freeze({surfaceCount,drawSurfaceCount,flags,normalFrame,passes})
  }
  const spawnVisibility=visibilityProbe([5328,3376,-3068,180,0,sourceHorizontal4By3FovToVertical(75),16/9,7,32768,Number(travel.tick)*0.015])
  require(spawnVisibility.surfaceCount===91,"fixed-camera PVS surface count changed")
  require(spawnVisibility.drawSurfaceCount>0&&spawnVisibility.drawSurfaceCount<=spawnVisibility.surfaceCount,"fixed-camera frustum surface count differs")
  const aboveWater=visibilityProbe([-4800,3000,-2100,0,20,sourceHorizontal4By3FovToVertical(75),16/9,7,32768,1]),belowWater=visibilityProbe([-4800,3000,-2300,0,-20,sourceHorizontal4By3FovToVertical(75),16/9,7,32768,1]),crossingWater=visibilityProbe([-4800,3000,-2160,0,0,sourceHorizontal4By3FovToVertical(75),16/9,7,32768,1])
  require(aboveWater.flags[0]===1&&aboveWater.flags[2]===1&&aboveWater.flags[3]===1&&aboveWater.normalFrame===30&&aboveWater.passes.join(",")==="0,1,2","above-Water reflection/refraction plan differs")
  require(belowWater.flags[0]===1&&belowWater.flags[3]===1&&belowWater.passes.includes(1)&&belowWater.passes.includes(2),"below-Water plan differs")
  require(crossingWater.flags[7]===1&&crossingWater.passes.includes(3),"Water near-plane intersection plan differs")

  exports.playsrc_free(particlePointer, particleBatch.length)
  exports.playsrc_free(particleOutputPointer, particleOutputLength)
  exports.playsrc_free(modelPointer, modelBatch.byteLength)
  exports.playsrc_free(modelOutputPointer, modelOutputLength)
  exports.playsrc_free(commandPointer, commandBytes.byteLength)
  exports.playsrc_free(snapshotPointer, snapshotLength)
  exports.playsrc_free(unchangedPointer, snapshotLength)
  require(exports.playsrc_dispose(handle) === 1, "handle disposal failed")
  require(exports.playsrc_snapshot_length(handle) === 0, "disposed handle retained a snapshot")
  const hdrFirst = compileProfile(1)
  const hdrSecond = compileProfile(1)
  require(Buffer.from(hdrFirst.payload).equals(hdrSecond.payload), "repeated HDR payload bytes differ")
  require(hdrFirst.sha256 === hdrSecond.sha256, "repeated HDR payload hashes differ")
  require(hdrFirst.derivedSha256 === hdrSecond.derivedSha256, "repeated HDR derived hashes differ")
  require(hdrFirst.derivedSha256 !== ldrDerivedSha256, "LDR and HDR derived identities are equal")
  require(ldrDerivedSha256 === EXPECTED_LDR_DERIVED_SHA256, `LDR derived identity ${ldrDerivedSha256} changed for payload ${mapSha256}`)
  require(hdrFirst.payload.byteLength === EXPECTED_HDR_BYTES, `HDR payload byte length changed to ${hdrFirst.payload.byteLength}`)
  require(hdrFirst.sha256 === EXPECTED_HDR_SHA256, `HDR payload SHA-256 changed to ${hdrFirst.sha256}`)
  require(hdrFirst.derivedSha256 === EXPECTED_HDR_DERIVED_SHA256, `HDR derived identity changed: ${hdrFirst.derivedSha256}`)
  require(nativeHdr.bytes === hdrFirst.payload.byteLength &&
    nativeHdr.sha256 === hdrFirst.sha256 &&
    nativeHdr.derivedSha256 === hdrFirst.derivedSha256 &&
    Buffer.from(nativeHdrPayload).equals(
      hdrFirst.payload,
    ), `native and WASM HDR generations differ: native=${nativeHdr.sha256}/${nativeHdr.derivedSha256} wasm=${hdrFirst.sha256}/${hdrFirst.derivedSha256}`)
  const hdr = inspectHdrPayload(hdrFirst.payload, new Bun.CryptoHasher("sha256").update(dependencyBytes).digest("hex"))
  const incompleteHdr = Uint8Array.from(bspBytes)
  new DataView(incompleteHdr.buffer).setInt32(8 + 54 * 16 + 4, 0, true)
  require(compileFailure(incompleteHdr, 1, dependencyBytes) ===
    6, "incomplete HDR profile did not fail before LDR fallback")
  require(compileFailure(bspBytes, 2, dependencyBytes) === 2, "unknown lighting profile was accepted")
  const missingDependency = Uint8Array.from(dependencyBytes)
  const selectedSky = "materials/skybox/sky_day01_01_hdrrt.vmt"
  const selectedSkyOffset = resourcePathOffset(missingDependency, selectedSky)
  require(selectedSkyOffset >= 0, "selected HDR sky dependency is absent from its bundle")
  missingDependency[selectedSkyOffset + new TextEncoder().encode(selectedSky).byteLength - 5] = "x".charCodeAt(0)
  require(compileFailure(bspBytes, 1, missingDependency) !== 0, "missing selected HDR material dependency was accepted")
  const simulationCommand=new Uint8Array(encodeCommand({forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:false,detonate:false})),simulationPointer=exports.playsrc_alloc(simulationCommand.length);new Uint8Array(exports.memory.buffer,simulationPointer,simulationCommand.length).set(simulationCommand);require(exports.playsrc_simulation_observe(hdrFirst.handle,0,simulationPointer,simulationCommand.length,0,0n)===1,"Simulation baseline failed");require(exports.playsrc_simulation_observe(hdrFirst.handle,0.016,simulationPointer,simulationCommand.length,0,0n)===1&&exports.playsrc_simulation_output_length(hdrFirst.handle)>16,"Simulation selected tick failed");exports.playsrc_free(simulationPointer,simulationCommand.length)
  require(exports.playsrc_dispose(hdrFirst.handle) === 1, "first HDR handle disposal failed")
  require(exports.playsrc_dispose(hdrSecond.handle) === 1, "second HDR handle disposal failed")
  return {
    target: identity!,
    mapBytes,
    mapSha256,
    ldrDerivedSha256,
    hdrBytes: hdrFirst.payload.byteLength,
    hdrSha256: hdrFirst.sha256,
    hdrDerivedSha256: hdrFirst.derivedSha256,
    hdr,
    dependencyBytes: dependencyBytes.byteLength,
    materials: renderMap.materials.length,
    drawableSurfaces: renderMap.drawableSurfaces,
    drawBatches: renderMap.batches.length,
    resolvedTextures,
    alignedTriangles,
    opposedTriangles,
    degenerateTriangles,
    models: renderMap.models.length,
    presentationBytes,
    particleMaterialStateSha256,
    modelOccurrences: renderMap.modelOccurrences.length,
    lightmapWidth: renderMap.lightmap.width,
    lightmapHeight: renderMap.lightmap.height,
    teleports,
    teleportDestinations,
    tick: 64,
    snapshotBytes: snapshotLength,
    projectiles: decoded.projectiles.length,
    events: decoded.projectileEvents.length,
    moverEntity: moverRequest.entity,
    moverCompletionEvents: Number(moverCompletionEvent),
    standingBlastVelocity,
    crouchedBlastVelocity,
    regenerateEntity: resupplied.events.find((event) => event.kind === 5)?.subject ?? 0,
    particleItems: particleOutputView.getUint32(8, true),
    viewmodelPrimitives: modelPoses.reduce((total, pose) => total + pose.primitives.length, 0),
    viewmodelEvents: modelPoses[0]?.events.length ?? 0,
    spawn,
  }
}
