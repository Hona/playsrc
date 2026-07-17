/// <reference lib="webworker" />

import type { InitialView, WorkerFailureCode, WorkerRequest, WorkerResponse } from "./protocol"

const MAX_WASM_BYTES = 64 * 1024 * 1024
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 64 * 1024 * 1024
const MAX_MESSAGE_BYTES = 64 * 1024 * 1024

type WasmExports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_compile_map(bsp: number, length: number, profile: number, config: number, configLength: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_jump_configure(handle: number, definition: number, length: number): number
  playsrc_game_advance(handle: number, command: number, length: number, ticks: number): number
  playsrc_snapshot_length(handle: number): number
  playsrc_snapshot_copy(handle: number, pointer: number, capacity: number): number
  playsrc_dispose(handle: number): number
}>

const scope = self as DedicatedWorkerGlobalScope
let wasm: WasmExports | undefined
let active: { generation: number; handle: number } | undefined
let pending: { generation: number; handle: number } | undefined

function post(message: WorkerResponse, transfer: Transferable[] = []): void {
  scope.postMessage(message, transfer)
}

function fail(id: number, code: WorkerFailureCode, detail = 0): void {
  post({ id, kind: "failure", code, detail })
}

function canonicalId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0 && (value as number) <= 0xffff_ffff
}

async function initialize(request: Extract<WorkerRequest, { kind: "initialize" }>): Promise<void> {
  if (
    wasm
    || !(request.wasm instanceof ArrayBuffer)
    || request.wasm.byteLength < 1
    || request.wasm.byteLength > MAX_WASM_BYTES
    || !/^[0-9a-f]{64}$/.test(request.wasmSha256)
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  try {
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", request.wasm))
    const actual = Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
    if (actual !== request.wasmSha256) {
      fail(request.id, "WasmUnavailable")
      return
    }
    const loaded = await WebAssembly.instantiate(request.wasm)
    const candidate = loaded.instance.exports as unknown as WasmExports
    if (
      !(candidate.memory instanceof WebAssembly.Memory)
      || ![
        candidate.playsrc_alloc,
        candidate.playsrc_free,
        candidate.playsrc_compile_map,
        candidate.playsrc_result_length,
        candidate.playsrc_result_error,
        candidate.playsrc_result_copy,
        candidate.playsrc_result_hash,
        candidate.playsrc_spawn_copy,
        candidate.playsrc_jump_configure,
        candidate.playsrc_game_advance,
        candidate.playsrc_snapshot_length,
        candidate.playsrc_snapshot_copy,
        candidate.playsrc_dispose,
      ].every((value) => typeof value === "function")
    ) {
      fail(request.id, "WasmUnavailable")
      return
    }
    wasm = candidate
    post({ id: request.id, kind: "initialized" })
  } catch {
    fail(request.id, "WasmUnavailable")
  }
}

function allocateCopy(exports: WasmExports, bytes: ArrayBuffer): number {
  const pointer = exports.playsrc_alloc(bytes.byteLength)
  new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(new Uint8Array(bytes))
  return pointer
}

function readHash(exports: WasmExports, handle: number): string | undefined {
  const pointer = exports.playsrc_alloc(32)
  const copied = exports.playsrc_result_hash(handle, pointer)
  const hash = copied === 1
    ? Array.from(
        new Uint8Array(exports.memory.buffer, pointer, 32),
        (value) => value.toString(16).padStart(2, "0"),
      ).join("")
    : undefined
  exports.playsrc_free(pointer, 32)
  return hash
}

function readInitialView(exports: WasmExports, handle: number): InitialView | undefined {
  const length = 40
  const pointer = exports.playsrc_alloc(length)
  const copied = exports.playsrc_spawn_copy(handle, pointer, length)
  if (copied !== length) {
    exports.playsrc_free(pointer, length)
    return undefined
  }
  const bytes = new Uint8Array(exports.memory.buffer, pointer, length).slice()
  exports.playsrc_free(pointer, length)
  const view = new DataView(bytes.buffer)
  if (
    new TextDecoder().decode(bytes.subarray(0, 4)) !== "PSIV"
    || view.getUint32(4, true) !== 1
  ) return undefined
  const scalars = Array.from({ length: 6 }, (_, index) => view.getFloat32(16 + index * 4, true))
  if (!scalars.every(Number.isFinite)) return undefined
  return Object.freeze({
    entity: view.getUint32(8, true),
    hammerId: view.getUint32(12, true) === 0xffff_ffff ? null : view.getUint32(12, true),
    position: Object.freeze(scalars.slice(0, 3)) as readonly [number, number, number],
    angles: Object.freeze(scalars.slice(3, 6)) as readonly [number, number, number],
  })
}

function load(request: Extract<WorkerRequest, { kind: "load" }>): void {
  const exports = wasm
  if (
    !exports
    || !canonicalId(request.generation)
    || request.generation <= Math.max(active?.generation ?? 0, pending?.generation ?? 0)
    || (request.profile !== 0 && request.profile !== 1)
    || !(request.bsp instanceof ArrayBuffer)
    || request.bsp.byteLength < 1
    || request.bsp.byteLength > MAX_BSP_BYTES
    || !(request.configuration instanceof ArrayBuffer)
    || request.configuration.byteLength > MAX_CONFIGURATION_BYTES
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const bspPointer = allocateCopy(exports, request.bsp)
  const configurationPointer = allocateCopy(exports, request.configuration)
  const candidate = exports.playsrc_compile_map(
    bspPointer,
    request.bsp.byteLength,
    request.profile,
    configurationPointer,
    request.configuration.byteLength,
  )
  exports.playsrc_free(bspPointer, request.bsp.byteLength)
  exports.playsrc_free(configurationPointer, request.configuration.byteLength)
  const error = exports.playsrc_result_error(candidate)
  if (error !== 0) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "CompileFailed", error)
    return
  }
  const payloadBytes = exports.playsrc_result_length(candidate)
  const payloadSha256 = readHash(exports, candidate)
  const initialView = readInitialView(exports, candidate)
  if (
    !Number.isSafeInteger(payloadBytes)
    || payloadBytes < 1
    || payloadBytes > MAX_MESSAGE_BYTES
    || payloadSha256 === undefined
    || initialView === undefined
  ) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "CompileFailed", 5)
    return
  }
  if (pending) exports.playsrc_dispose(pending.handle)
  pending = { generation: request.generation, handle: candidate }
  post({
    id: request.id,
    kind: "loaded",
    generation: request.generation,
    payloadBytes,
    payloadSha256,
    initialView,
  })
}

function requireActive(id: number, generation: number): { exports: WasmExports; handle: number } | undefined {
  if (!wasm || !active || active.generation !== generation) {
    fail(id, "StaleGeneration")
    return undefined
  }
  return { exports: wasm, handle: active.handle }
}

function readMap(request: Extract<WorkerRequest, { kind: "read-map" }>): void {
  if (!wasm || !pending || pending.generation !== request.generation) {
    fail(request.id, "StaleGeneration")
    return
  }
  const value = { exports: wasm, handle: pending.handle }
  const length = value.exports.playsrc_result_length(value.handle)
  if (!Number.isSafeInteger(length) || length < 1 || length > MAX_MESSAGE_BYTES) {
    fail(request.id, "InternalFailure")
    return
  }
  const pointer = value.exports.playsrc_alloc(length)
  const copied = value.exports.playsrc_result_copy(value.handle, pointer, length)
  if (copied !== length) {
    value.exports.playsrc_free(pointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const payload = new Uint8Array(value.exports.memory.buffer, pointer, length).slice().buffer
  value.exports.playsrc_free(pointer, length)
  post({ id: request.id, kind: "map", generation: request.generation, payload }, [payload])
}

function activate(request: Extract<WorkerRequest, { kind: "activate" }>): void {
  if (!wasm || !pending || pending.generation !== request.generation) {
    fail(request.id, "StaleGeneration")
    return
  }
  if (active) wasm.playsrc_dispose(active.handle)
  active = pending
  pending = undefined
  post({ id: request.id, kind: "activated", generation: request.generation })
}

function discard(request: Extract<WorkerRequest, { kind: "discard" }>): void {
  if (!wasm || !pending || pending.generation !== request.generation) {
    fail(request.id, "StaleGeneration")
    return
  }
  wasm.playsrc_dispose(pending.handle)
  pending = undefined
  post({ id: request.id, kind: "discarded", generation: request.generation })
}

function configureCourse(request: Extract<WorkerRequest, { kind: "configure-course" }>): void {
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (
    !(request.definition instanceof ArrayBuffer)
    || request.definition.byteLength < 52
    || request.definition.byteLength > 64 * 1024
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const pointer = allocateCopy(value.exports, request.definition)
  const configured = value.exports.playsrc_jump_configure(
    value.handle,
    pointer,
    request.definition.byteLength,
  )
  value.exports.playsrc_free(pointer, request.definition.byteLength)
  if (configured !== 1) {
    fail(request.id, "TransitionFailed")
    return
  }
  post({ id: request.id, kind: "course-configured", generation: request.generation })
}

function advance(request: Extract<WorkerRequest, { kind: "advance" }>): void {
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (
    !(request.command instanceof ArrayBuffer)
    || request.command.byteLength !== 40
    || !Number.isSafeInteger(request.ticks)
    || request.ticks < 1
    || request.ticks > 64
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const pointer = allocateCopy(value.exports, request.command)
  const result = value.exports.playsrc_game_advance(value.handle, pointer, 40, request.ticks)
  value.exports.playsrc_free(pointer, 40)
  if (result !== 1) {
    fail(request.id, "TransitionFailed")
    return
  }
  const length = value.exports.playsrc_snapshot_length(value.handle)
  if (!Number.isSafeInteger(length) || length < 64 || length > MAX_MESSAGE_BYTES) {
    fail(request.id, "InternalFailure")
    return
  }
  const snapshotPointer = value.exports.playsrc_alloc(length)
  const copied = value.exports.playsrc_snapshot_copy(value.handle, snapshotPointer, length)
  if (copied !== length) {
    value.exports.playsrc_free(snapshotPointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const snapshot = new Uint8Array(value.exports.memory.buffer, snapshotPointer, length).slice().buffer
  value.exports.playsrc_free(snapshotPointer, length)
  post({ id: request.id, kind: "snapshot", generation: request.generation, snapshot }, [snapshot])
}

function shutdown(request: Extract<WorkerRequest, { kind: "shutdown" }>): void {
  if (wasm && active) wasm.playsrc_dispose(active.handle)
  if (wasm && pending) wasm.playsrc_dispose(pending.handle)
  active = undefined
  pending = undefined
  wasm = undefined
  post({ id: request.id, kind: "shutdown" })
}

function dispatch(request: WorkerRequest): void | Promise<void> {
  if (!request || !canonicalId(request.id) || typeof request.kind !== "string") return
  switch (request.kind) {
    case "initialize": return initialize(request)
    case "load": return load(request)
    case "read-map": return readMap(request)
    case "activate": return activate(request)
    case "discard": return discard(request)
    case "configure-course": return configureCourse(request)
    case "advance": return advance(request)
    case "shutdown": return shutdown(request)
    default: return fail((request as { id: number }).id, "MalformedRequest")
  }
}

let queue = Promise.resolve()
scope.onmessage = (event: MessageEvent<WorkerRequest>) => {
  queue = queue.then(() => dispatch(event.data)).catch(() => {
    if (canonicalId(event.data?.id)) fail(event.data.id, "InternalFailure")
  })
}
