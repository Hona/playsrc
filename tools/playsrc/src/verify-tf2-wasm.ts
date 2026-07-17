import { readFile } from "node:fs/promises"
import path from "node:path"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { parseRuntimeMap } from "@playsrc/rendering/runtime-map"
import { buildSourceBundle } from "./source-bundle"

const EXPECTED_MAP_BYTES = 39_814_462
const EXPECTED_MAP_SHA256 = "d0576dff06413848d8712ab6218c8c6f34078a1b347795c5a7a694a108c29725"
const EXPECTED_DEPENDENCY_BYTES = 39_936_317
const EXPECTED_DEPENDENCY_SHA256 = "d7582f82f4a39c087d24246550192753ea879cb912a842c1b33d84a9d7b27ee0"

type Exports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_compile_map(bsp: number, length: number, profile: number, config: number, configLength: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_game_advance(handle: number, command: number, length: number, ticks: number): number
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
  return path.join(
    repositoryRoot,
    "target",
    "wasm32-unknown-unknown",
    "release",
    "playsrc_tf2_wasm.wasm",
  )
}

export async function verifyTf2Wasm(
  config: LocalConfig,
  identity: string | undefined,
): Promise<Record<string, number | string>> {
  const map = await acquireMap(config, identity)
  const wasmPath = await buildTf2Wasm(config)
  const bundlePath = await buildSourceBundle(config, identity ?? "")
  const wasmBytes = await readFile(wasmPath)
  require(wasmBytes.byteLength > 0 && wasmBytes.byteLength <= 64 * 1024 * 1024, "WASM byte length is invalid")
  const loaded = await WebAssembly.instantiate(wasmBytes)
  const exports = loaded.instance.exports as unknown as Exports
  const [bspBytes, dependencyBytes] = await Promise.all([
    readFile(path.join(config.sourceCacheDir, map.decoded.cachePath)),
    readFile(bundlePath),
  ])
  require(bspBytes.byteLength === map.decoded.byteLength, "cached BSP byte length changed")
  require(dependencyBytes.byteLength === EXPECTED_DEPENDENCY_BYTES, "source dependency byte length changed")
  require(
    new Bun.CryptoHasher("sha256").update(dependencyBytes).digest("hex") === EXPECTED_DEPENDENCY_SHA256,
    "source dependency SHA-256 changed",
  )

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
  require(
    spawn.entity === 1
    && spawn.hammerId === 29
    && spawn.position.every((value, index) => value === [5328, 3376, -3120][index])
    && spawn.angles.every((value, index) => value === [-1, 180, 0][index]),
    "TF2 spawn descriptor differs from the selected teamspawn",
  )
  const mapBytes = exports.playsrc_result_length(handle)
  require(mapBytes === EXPECTED_MAP_BYTES, `map payload length ${mapBytes} != ${EXPECTED_MAP_BYTES}`)
  const hashPointer = exports.playsrc_alloc(32)
  require(exports.playsrc_result_hash(handle, hashPointer) === 1, "map payload hash is unavailable")
  const declaredMapSha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
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
      const position = (index: number) => [
        batch.positions[index * 3]!,
        batch.positions[index * 3 + 1]!,
        batch.positions[index * 3 + 2]!,
      ] as const
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
      const normal = indexes.reduce((sum, index) => [
        sum[0] + batch.normals[index * 3]!,
        sum[1] + batch.normals[index * 3 + 1]!,
        sum[2] + batch.normals[index * 3 + 2]!,
      ] as const, [0, 0, 0] as readonly [number, number, number])
      const facing = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2]
      if (Math.abs(facing) <= 1e-6) degenerateTriangles += 1
      else if (facing > 0) alignedTriangles += 1
      else opposedTriangles += 1
    }
  }
  require(
    alignedTriangles === 6_497 && opposedTriangles === 0 && degenerateTriangles === 0,
    `runtime triangle orientation is ${alignedTriangles} aligned, ${opposedTriangles} opposed, ${degenerateTriangles} degenerate`,
  )
  require(renderMap.models.length === 9, `runtime model count ${renderMap.models.length} is invalid`)
  require(renderMap.modelOccurrences.length === 33, "runtime model occurrence count is invalid")
  require(renderMap.lightmap !== undefined, "runtime lightmap atlas is unavailable")
  const teleports = exports.playsrc_teleport_count(handle)
  const teleportDestinations = exports.playsrc_teleport_destination_count(handle)
  require(teleports === 56, "runtime map teleport count is invalid")
  require(teleportDestinations === 25, "runtime map teleport-destination count is invalid")

  const commandPointer = exports.playsrc_alloc(24)
  const command = new DataView(exports.memory.buffer, commandPointer, 24)
  command.setUint32(16, 4, true)
  require(exports.playsrc_game_advance(handle, commandPointer, 24, 64) === 1, "64-tick gameplay phase failed")
  const snapshotLength = exports.playsrc_snapshot_length(handle)
  require(snapshotLength >= 56, "snapshot is shorter than its fixed fields")
  const snapshotPointer = exports.playsrc_alloc(snapshotLength)
  require(
    exports.playsrc_snapshot_copy(handle, snapshotPointer, snapshotLength) === snapshotLength,
    "snapshot copy failed",
  )
  const snapshot = new Uint8Array(exports.memory.buffer, snapshotPointer, snapshotLength).slice()
  const view = new DataView(snapshot.buffer)
  require(new TextDecoder().decode(snapshot.subarray(0, 4)) === "PSSN", "snapshot magic is invalid")
  require(view.getUint32(4, true) === 2, "snapshot version is invalid")
  require(view.getBigUint64(8, true) === 64n, "snapshot tick is invalid")
  const projectiles = view.getUint32(48, true)
  const eventCountOffset = 52 + 36 * projectiles
  require(eventCountOffset + 4 <= snapshotLength, "projectile records exceed snapshot length")
  const events = view.getUint32(eventCountOffset, true)
  require(snapshotLength === eventCountOffset + 4 + events * 28, "event records do not frame the snapshot")
  const eventKinds = Array.from(
    { length: events },
    (_, index) => snapshot[eventCountOffset + 4 + index * 28],
  )
  require(eventKinds.includes(3) && eventKinds.includes(4), "fixed phase omitted fire or explosion events")

  new DataView(exports.memory.buffer, commandPointer, 24).setFloat32(0, Number.NaN, true)
  require(exports.playsrc_game_advance(handle, commandPointer, 24, 1) === 0, "non-finite command was accepted")
  require(exports.playsrc_snapshot_length(handle) === snapshotLength, "rejected command replaced the snapshot")
  const unchangedPointer = exports.playsrc_alloc(snapshotLength)
  require(
    exports.playsrc_snapshot_copy(handle, unchangedPointer, snapshotLength) === snapshotLength,
    "unchanged snapshot copy failed",
  )
  require(
    Buffer.from(exports.memory.buffer, unchangedPointer, snapshotLength).equals(snapshot),
    "rejected command mutated the snapshot",
  )

  exports.playsrc_free(commandPointer, 24)
  exports.playsrc_free(snapshotPointer, snapshotLength)
  exports.playsrc_free(unchangedPointer, snapshotLength)
  require(exports.playsrc_dispose(handle) === 1, "handle disposal failed")
  require(exports.playsrc_snapshot_length(handle) === 0, "disposed handle retained a snapshot")
  return {
    target: identity!,
    mapBytes,
    mapSha256,
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
    projectiles,
    events,
    spawn,
  }
}
