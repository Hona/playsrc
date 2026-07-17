import { readFile } from "node:fs/promises"
import path from "node:path"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { parseRuntimeMap } from "@playsrc/rendering/runtime-map"
import { buildSourceBundle } from "./source-bundle"
import { decodeSnapshot, encodeCommand, encodeJumpCourse } from "../../../games/tf2/browser/src/codec"

const EXPECTED_MAP_BYTES = 49_414_468
const EXPECTED_MAP_SHA256 = "f44941ce76aa276d7a278cb84c122709f47e477baaec865091c0b0ab5653ab0e"
const EXPECTED_BSP_SHA256 = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959"
const EXPECTED_HDR_BYTES = 85_586_296
const EXPECTED_HDR_SHA256 = "d39f32489a7449075e788f78cde8bb0263b161e917d9a1b10cd0f6a96e865c68"
const EXPECTED_LDR_DERIVED_SHA256 = "76fff83deb09129cef5359bd92f5572da0e9468c72534b15a5521b15a1359bf5"
const EXPECTED_HDR_DERIVED_SHA256 = "9f4b214cd3edc7509f623c5ca9d42b9ad37d4d6a5de12a56c3d84bb1d215667a"
const EXPECTED_DEPENDENCY_BYTES = 112_112_616
const EXPECTED_DEPENDENCY_SHA256 = "34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78"
function bundlePathOffset(bytes: Uint8Array, target: string): number {
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

type Exports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_compile_map(bsp: number, length: number, profile: number, config: number, configLength: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_result_derived_hash(handle: number, pointer: number): number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_game_advance(handle: number, command: number, length: number, ticks: number): number
  playsrc_jump_configure(handle: number, definition: number, length: number): number
  playsrc_particle_transact(handle: number, pointer: number, length: number): number
  playsrc_particle_output_length(handle: number): number
  playsrc_visibility_query(handle: number, pointer: number): number
  playsrc_visibility_output_length(handle: number): number
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
    reader.sized()
  }
  return { shader, role }
}

function inspectHdrPayload(payload: Uint8Array) {
  const reader = new ProfileReader(payload)
  require(new TextDecoder().decode(reader.take(4)) === "PSMP", "HDR map magic is invalid")
  require(reader.u32() === 4, "HDR map schema is invalid")
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
  require(modelCount === 53, "HDR model count is invalid")
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
    configurationSha256 === EXPECTED_DEPENDENCY_SHA256, "HDR source/configuration identity is invalid")
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
  require(inputCount === 294, "HDR input-hash count is invalid")
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

export async function buildTf2Wasm(config: LocalConfig): Promise<string> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const child = Bun.spawn(
    [
      cargo,
      `+${toolchains.rust.toolchain}`,
      "build",
      "-p",
      "playsrc-tf2-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--release",
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
      stdout: "inherit",
      stderr: "inherit",
    },
  )
  const exitCode = await child.exited
  if (exitCode !== 0) throw new WasmVerificationError(`cargo build exited with code ${exitCode}`)
  return path.join(repositoryRoot, "target", "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
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
  const wasmPath = await buildTf2Wasm(config)
  const bundlePath = await buildSourceBundle(config, identity ?? "")
  const nativeHdr = await buildNativeHdr(config, identity ?? "")
  const wasmBytes = await readFile(wasmPath)
  require(wasmBytes.byteLength > 0 && wasmBytes.byteLength <= 64 * 1024 * 1024, "WASM byte length is invalid")
  const loaded = await WebAssembly.instantiate(wasmBytes)
  const exports = loaded.instance.exports as unknown as Exports
  const [bspBytes, dependencyBytes, nativeHdrPayload] = await Promise.all([
    readFile(path.join(config.sourceCacheDir, map.decoded.cachePath)),
    readFile(bundlePath),
    readFile(nativeHdr.path),
  ])
  require(bspBytes.byteLength === map.decoded.byteLength, "cached BSP byte length changed")
  require(dependencyBytes.byteLength === EXPECTED_DEPENDENCY_BYTES, "source dependency byte length changed")
  require(new Bun.CryptoHasher("sha256").update(dependencyBytes).digest("hex") ===
    EXPECTED_DEPENDENCY_SHA256, "source dependency SHA-256 changed")

  const compileProfile = (profile: 0 | 1) => {
    const source = exports.playsrc_alloc(bspBytes.byteLength)
    new Uint8Array(exports.memory.buffer, source, bspBytes.byteLength).set(bspBytes)
    const configuration = exports.playsrc_alloc(dependencyBytes.byteLength)
    new Uint8Array(exports.memory.buffer, configuration, dependencyBytes.byteLength).set(dependencyBytes)
    const result = exports.playsrc_compile_map(
      source,
      bspBytes.byteLength,
      profile,
      configuration,
      dependencyBytes.byteLength,
    )
    exports.playsrc_free(source, bspBytes.byteLength)
    exports.playsrc_free(configuration, dependencyBytes.byteLength)
    const error = exports.playsrc_result_error(result)
    require(error === 0, `TF2 WASM profile ${profile} compilation failed with error ${error}`)
    const length = exports.playsrc_result_length(result)
    require(length > 0 && length <= 512 * 1024 * 1024, `profile ${profile} payload length is invalid`)
    const pointer = exports.playsrc_alloc(length)
    require(exports.playsrc_result_copy(result, pointer, length) === length, `profile ${profile} payload copy failed`)
    const payload = new Uint8Array(exports.memory.buffer, pointer, length).slice()
    exports.playsrc_free(pointer, length)
    const hashPointer = exports.playsrc_alloc(32)
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
    const result = exports.playsrc_compile_map(
      source,
      sourceBytes.byteLength,
      profile,
      configuration,
      configBytes.byteLength,
    )
    exports.playsrc_free(source, sourceBytes.byteLength)
    exports.playsrc_free(configuration, configBytes.byteLength)
    const error = exports.playsrc_result_error(result)
    require(exports.playsrc_dispose(result) === 1, "failed compilation handle disposal failed")
    return error
  }

  const bspPointer = exports.playsrc_alloc(bspBytes.byteLength)
  new Uint8Array(exports.memory.buffer, bspPointer, bspBytes.byteLength).set(bspBytes)
  const dependencyPointer = exports.playsrc_alloc(dependencyBytes.byteLength)
  new Uint8Array(exports.memory.buffer, dependencyPointer, dependencyBytes.byteLength).set(dependencyBytes)
  const handle = exports.playsrc_compile_map(
    bspPointer,
    bspBytes.byteLength,
    0,
    dependencyPointer,
    dependencyBytes.byteLength,
  )
  exports.playsrc_free(bspPointer, bspBytes.byteLength)
  exports.playsrc_free(dependencyPointer, dependencyBytes.byteLength)
  const error = exports.playsrc_result_error(handle)
  require(error === 0, `TF2 WASM map compilation failed with error ${error}`)
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
  require(renderMap.models.length === 53, `runtime model count ${renderMap.models.length} is invalid`)
  require(renderMap.modelOccurrences.length === 33, "runtime model occurrence count is invalid")
  require(renderMap.lightmap !== undefined, "runtime lightmap atlas is unavailable")
  const teleports = exports.playsrc_teleport_count(handle)
  const teleportDestinations = exports.playsrc_teleport_destination_count(handle)
  require(teleports === 56, "runtime map teleport count is invalid")
  require(teleportDestinations === 25, "runtime map teleport-destination count is invalid")
  require(exports.playsrc_runtime_count(handle, 9) === 22, "runtime map regenerate-zone count is invalid")

  const course = encodeJumpCourse(7n, EXPECTED_BSP_SHA256, [
    { identity: 1, triggerEntity: 316, kind: "start", index: 1 },
    { identity: 2, triggerEntity: 87, kind: "checkpoint", index: 1 },
    { identity: 3, triggerEntity: 257, kind: "end", index: 1 },
  ])
  const coursePointer = exports.playsrc_alloc(course.byteLength)
  new Uint8Array(exports.memory.buffer, coursePointer, course.byteLength).set(course)
  require(exports.playsrc_jump_configure(handle, coursePointer, course.byteLength) ===
    1, "Jump course configuration failed")
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
  const commandPointer = exports.playsrc_alloc(40)
  new Uint8Array(exports.memory.buffer, commandPointer, 40).set(commandBytes)
  require(exports.playsrc_game_advance(handle, commandPointer, 40, 64) === 1, "64-tick gameplay phase failed")
  const snapshotLength = exports.playsrc_snapshot_length(handle)
  require(snapshotLength >= 56, "snapshot is shorter than its fixed fields")
  const snapshotPointer = exports.playsrc_alloc(snapshotLength)
  require(exports.playsrc_snapshot_copy(handle, snapshotPointer, snapshotLength) ===
    snapshotLength, "snapshot copy failed")
  const snapshot = new Uint8Array(exports.memory.buffer, snapshotPointer, snapshotLength).slice()
  const decoded = decodeSnapshot(snapshot.buffer)
  require(decoded.tick === 64n, "snapshot tick is invalid")
  require(decoded.projectileEvents.some((event) => event.type === "fire"), "fixed phase omitted fire event")

  new DataView(exports.memory.buffer, commandPointer, 40).setFloat32(4, Number.NaN, true)
  require(exports.playsrc_game_advance(handle, commandPointer, 40, 1) === 0, "non-finite command was accepted")
  require(exports.playsrc_snapshot_length(handle) === snapshotLength, "rejected command replaced the snapshot")
  const unchangedPointer = exports.playsrc_alloc(snapshotLength)
  require(exports.playsrc_snapshot_copy(handle, unchangedPointer, snapshotLength) ===
    snapshotLength, "unchanged snapshot copy failed")
  require(Buffer.from(exports.memory.buffer, unchangedPointer, snapshotLength).equals(
    snapshot,
  ), "rejected command mutated the snapshot")
  const definition = new TextEncoder().encode("rockettrail"),
    particleBatch = new Uint8Array(100 + definition.length),
    particleView = new DataView(particleBatch.buffer)
  particleBatch.set([0x50, 0x50, 0x54, 0x58])
  particleView.setUint32(4, 1, true)
  particleView.setFloat32(12, 0.1, true)
  ;[5328, 3376, -3052].forEach((value, index) => particleView.setFloat32(16 + index * 4, value, true))
  particleView.setUint32(28, 1, true)
  particleBatch[32] = 1
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
  const visibilityPointer = exports.playsrc_alloc(12)
  new Float32Array(exports.memory.buffer, visibilityPointer, 3).set([5328, 3376, -3068])
  require(exports.playsrc_visibility_query(handle, visibilityPointer) === 1, "fixed-camera PVS query failed")
  require(exports.playsrc_visibility_output_length(handle) === 80 + 91 * 4, "fixed-camera PVS surface count changed")

  exports.playsrc_free(coursePointer, course.byteLength)
  exports.playsrc_free(particlePointer, particleBatch.length)
  exports.playsrc_free(visibilityPointer, 12)
  exports.playsrc_free(commandPointer, 40)
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
  require(ldrDerivedSha256 === EXPECTED_LDR_DERIVED_SHA256, `LDR derived identity ${ldrDerivedSha256} changed`)
  require(hdrFirst.payload.byteLength === EXPECTED_HDR_BYTES, "HDR payload byte length changed")
  require(hdrFirst.sha256 === EXPECTED_HDR_SHA256, "HDR payload SHA-256 changed")
  require(hdrFirst.derivedSha256 === EXPECTED_HDR_DERIVED_SHA256, "HDR derived identity changed")
  require(nativeHdr.bytes === hdrFirst.payload.byteLength &&
    nativeHdr.sha256 === hdrFirst.sha256 &&
    nativeHdr.derivedSha256 === hdrFirst.derivedSha256 &&
    Buffer.from(nativeHdrPayload).equals(
      hdrFirst.payload,
    ), `native and WASM HDR generations differ: native=${nativeHdr.sha256}/${nativeHdr.derivedSha256} wasm=${hdrFirst.sha256}/${hdrFirst.derivedSha256}`)
  const hdr = inspectHdrPayload(hdrFirst.payload)
  const incompleteHdr = Uint8Array.from(bspBytes)
  new DataView(incompleteHdr.buffer).setInt32(8 + 54 * 16 + 4, 0, true)
  require(compileFailure(incompleteHdr, 1, dependencyBytes) ===
    6, "incomplete HDR profile did not fail before LDR fallback")
  require(compileFailure(bspBytes, 2, dependencyBytes) === 2, "unknown lighting profile was accepted")
  const missingDependency = Uint8Array.from(dependencyBytes)
  const selectedSky = "materials/skybox/sky_day01_01_hdrrt.vmt"
  const selectedSkyOffset = bundlePathOffset(missingDependency, selectedSky)
  require(selectedSkyOffset >= 0, "selected HDR sky dependency is absent from its bundle")
  missingDependency[selectedSkyOffset + new TextEncoder().encode(selectedSky).byteLength - 5] = "x".charCodeAt(0)
  require(compileFailure(bspBytes, 1, missingDependency) !== 0, "missing selected HDR material dependency was accepted")
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
    modelOccurrences: renderMap.modelOccurrences.length,
    lightmapWidth: renderMap.lightmap.width,
    lightmapHeight: renderMap.lightmap.height,
    teleports,
    teleportDestinations,
    tick: 64,
    snapshotBytes: snapshotLength,
    projectiles: decoded.projectiles.length,
    events: decoded.projectileEvents.length,
    spawn,
  }
}
