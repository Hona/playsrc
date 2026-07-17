import { readFile } from "node:fs/promises"
import path from "node:path"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { acquireMap } from "./targets"
import { parseRuntimeMap } from "@playsrc/rendering/runtime-map"

const EXPECTED_MAP_BYTES = 16_581_206
const EXPECTED_MAP_SHA256 = "2019f979e72a98f4a9548a69c92e138991df0964d155576acc958a49c35db2e2"

type Exports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_compile_map(bsp: number, length: number, profile: number, config: number, configLength: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_game_advance(handle: number, command: number, length: number, ticks: number): number
  playsrc_snapshot_length(handle: number): number
  playsrc_snapshot_copy(handle: number, pointer: number, capacity: number): number
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
  const wasmBytes = await readFile(wasmPath)
  require(wasmBytes.byteLength > 0 && wasmBytes.byteLength <= 64 * 1024 * 1024, "WASM byte length is invalid")
  const loaded = await WebAssembly.instantiate(wasmBytes)
  const exports = loaded.instance.exports as unknown as Exports
  const bspBytes = await readFile(path.join(config.sourceCacheDir, map.decoded.cachePath))
  require(bspBytes.byteLength === map.decoded.byteLength, "cached BSP byte length changed")

  const bspPointer = exports.playsrc_alloc(bspBytes.byteLength)
  new Uint8Array(exports.memory.buffer, bspPointer, bspBytes.byteLength).set(bspBytes)
  const handle = exports.playsrc_compile_map(bspPointer, bspBytes.byteLength, 0, 0, 0)
  exports.playsrc_free(bspPointer, bspBytes.byteLength)
  const error = exports.playsrc_result_error(handle)
  require(error === 0, `TF2 WASM map compilation failed with error ${error}`)
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
  require(view.getUint32(4, true) === 1, "snapshot version is invalid")
  require(view.getBigUint64(8, true) === 64n, "snapshot tick is invalid")
  const projectiles = view.getUint32(48, true)
  const eventCountOffset = 52 + 36 * projectiles
  require(eventCountOffset + 4 <= snapshotLength, "projectile records exceed snapshot length")
  const events = view.getUint32(eventCountOffset, true)
  require(snapshotLength === eventCountOffset + 4 + events * 24, "event records do not frame the snapshot")
  const eventKinds = Array.from(
    { length: events },
    (_, index) => snapshot[eventCountOffset + 4 + index * 24],
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
    materials: renderMap.materials.length,
    drawableSurfaces: renderMap.drawableSurfaces,
    drawBatches: renderMap.batches.length,
    tick: 64,
    snapshotBytes: snapshotLength,
    projectiles,
    events,
  }
}
