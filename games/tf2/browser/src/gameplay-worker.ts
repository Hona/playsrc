/// <reference lib="webworker" />

import type { InitialView, WorkerFailureCode, WorkerRequest, WorkerResponse } from "./protocol"
import { decodeTf2TeamSelectionServerState } from "./team-selection/model"
import initializeWasm, { initThreadPool } from "./wasm-generated/tf2_wasm.js"

const MAX_WASM_BYTES = 64 * 1024 * 1024
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 768 * 1024 * 1024

const MAX_MESSAGE_BYTES = 512 * 1024 * 1024
const MAX_PRESENTATION_BYTES = 512 * 1024 * 1024

type WasmExports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_resource_decode(pointer: number, length: number): number
  playsrc_resource_length(): number
  playsrc_resource_take(): number
  playsrc_compile_map(bsp: number, length: number, profile: number, config: number, configLength: number): number
  playsrc_compile_map_cached(bsp: number, length: number, profile: number, config: number, configLength: number, presentation: number, presentationLength: number): number
  playsrc_compile_metric_milliseconds(handle: number, index: number): number
  playsrc_texture_inspection_count(handle: number, index: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_presentation_length(handle: number): number
  playsrc_presentation_copy(handle: number, pointer: number, capacity: number): number
  playsrc_presentation_release(handle:number):number
  playsrc_coverage_length(handle:number):number
  playsrc_coverage_copy(handle:number,pointer:number,capacity:number):number
  playsrc_particle_transact(handle: number, pointer: number, length: number): number
  playsrc_particle_output_length(handle: number): number
  playsrc_particle_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_model_transact(handle: number, pointer: number, length: number): number
  playsrc_model_output_length(handle: number): number
  playsrc_model_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_visibility_query(handle: number, pointer: number): number
  playsrc_visibility_output_length(handle: number): number
  playsrc_visibility_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_team_state_copy(handle: number, pointer: number, capacity: number): number
  playsrc_team_select(handle: number, choice: number): number
  playsrc_jump_configure(handle: number, definition: number, length: number): number
  playsrc_simulation_observe(handle: number, nowSeconds: number, command: number, length: number, suspended: number): number
  playsrc_simulation_output_length(handle: number): number
  playsrc_simulation_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_simulation_error():number
  playsrc_simulation_error_length():number
  playsrc_simulation_error_copy(pointer:number,capacity:number):number
  playsrc_dispose(handle: number): number
}>

const scope = self as DedicatedWorkerGlobalScope
let wasm: WasmExports | undefined
let active: { generation: number; handle: number } | undefined
let pending: { generation: number; handle: number } | undefined

function post(message: WorkerResponse, transfer: Transferable[] = []): void {
  scope.postMessage(message, transfer)
}

function fail(id: number, code: WorkerFailureCode, detail = 0,reason?:string): void {
  post({ id, kind: "failure", code, detail,...(reason?{reason}:{}) })
}

function queueMilliseconds(request: Pick<WorkerRequest, "queuedAt">, started: number): number {
  return request.queuedAt === undefined || !Number.isFinite(request.queuedAt)
    ? 0
    : Math.max(0, performance.timeOrigin + started - request.queuedAt)
}

function canonicalId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0 && (value as number) <= 0xffff_ffff
}

async function initialize(request: Extract<WorkerRequest, { kind: "initialize" }>): Promise<void> {
  if (
    wasm ||
    !(request.wasm instanceof ArrayBuffer) ||
    request.wasm.byteLength < 1 ||
    request.wasm.byteLength > MAX_WASM_BYTES ||
    !/^[0-9a-f]{64}$/.test(request.wasmSha256) ||
    !Number.isSafeInteger(request.threads) ||
    request.threads < 1 ||
    request.threads > 64
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
    const candidate = await initializeWasm({ module_or_path: request.wasm }) as unknown as WasmExports
    if (
      !(candidate.memory instanceof WebAssembly.Memory) ||
      ![
        candidate.playsrc_alloc,
        candidate.playsrc_free,
        candidate.playsrc_resource_decode,
        candidate.playsrc_resource_length,
        candidate.playsrc_resource_take,
        candidate.playsrc_compile_map,
        candidate.playsrc_compile_map_cached,
        candidate.playsrc_compile_metric_milliseconds,
        candidate.playsrc_result_length,
        candidate.playsrc_result_error,
        candidate.playsrc_result_copy,
        candidate.playsrc_result_hash,
        candidate.playsrc_presentation_length,
        candidate.playsrc_presentation_copy,
        candidate.playsrc_presentation_release,
        candidate.playsrc_coverage_length,
        candidate.playsrc_coverage_copy,
        candidate.playsrc_particle_transact,
        candidate.playsrc_particle_output_length,
        candidate.playsrc_particle_output_copy,
        candidate.playsrc_model_transact,
        candidate.playsrc_model_output_length,
        candidate.playsrc_model_output_copy,
        candidate.playsrc_visibility_query,
        candidate.playsrc_visibility_output_length,
        candidate.playsrc_visibility_output_copy,
        candidate.playsrc_spawn_copy,
        candidate.playsrc_team_state_copy,
        candidate.playsrc_team_select,
        candidate.playsrc_jump_configure,
        candidate.playsrc_simulation_observe,
        candidate.playsrc_simulation_output_length,
        candidate.playsrc_simulation_output_copy,
        candidate.playsrc_simulation_error,
        candidate.playsrc_simulation_error_length,
        candidate.playsrc_simulation_error_copy,
        candidate.playsrc_dispose,
      ].every((value) => typeof value === "function")
    ) {
      fail(request.id, "WasmUnavailable")
      return
    }
    await initThreadPool(request.threads)
    wasm = candidate
    post({ id: request.id, kind: "initialized" })
  } catch {
    fail(request.id, "WasmUnavailable")
  }
}

function decodeResources(request: Extract<WorkerRequest, { kind: "decode-resources" }>): void {
  const exports = wasm
  if (!exports || !(request.batch instanceof ArrayBuffer) || request.batch.byteLength < 12 || request.batch.byteLength > MAX_MESSAGE_BYTES) {
    fail(request.id, "MalformedRequest")
    return
  }
  const input = allocateCopy(exports, request.batch)
  const decoded = exports.playsrc_resource_decode(input, request.batch.byteLength)
  exports.playsrc_free(input, request.batch.byteLength)
  if (decoded !== 1) {
    fail(request.id, "CompileFailed")
    return
  }
  const length = exports.playsrc_resource_length()
  if (!Number.isSafeInteger(length) || length < 12 || length > MAX_CONFIGURATION_BYTES) {
    fail(request.id, "InternalFailure")
    return
  }
  const pointer = exports.playsrc_resource_take() >>> 0
  if (pointer === 0) {
    fail(request.id, "InternalFailure")
    return
  }
  let bytes: Uint8Array
  try {
    bytes = new Uint8Array(exports.memory.buffer, pointer, length).slice()
  } finally {
    exports.playsrc_free(pointer, length)
  }
  post({ id: request.id, kind: "resources", bytes: bytes.buffer }, [bytes.buffer])
}

function allocateCopy(exports: WasmExports, bytes: ArrayBuffer): number {
  const pointer = exports.playsrc_alloc(bytes.byteLength) >>> 0
  new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(new Uint8Array(bytes))
  return pointer
}

function readHash(exports: WasmExports, handle: number): string | undefined {
  const pointer = exports.playsrc_alloc(32) >>> 0
  const copied = exports.playsrc_result_hash(handle, pointer)
  const hash =
    copied === 1
      ? Array.from(new Uint8Array(exports.memory.buffer, pointer, 32), (value) =>
          value.toString(16).padStart(2, "0"),
        ).join("")
      : undefined
  exports.playsrc_free(pointer, 32)
  return hash
}
function readInitialView(exports: WasmExports, handle: number): InitialView | undefined {
  const length = 40
  const pointer = exports.playsrc_alloc(length) >>> 0
  try {
    if (exports.playsrc_spawn_copy(handle, pointer, length) !== length) return undefined
    const bytes = new Uint8Array(exports.memory.buffer, pointer, length)
    const view = new DataView(exports.memory.buffer, pointer, length)
    if (
      bytes[0] !== 0x50 || bytes[1] !== 0x53 || bytes[2] !== 0x49 || bytes[3] !== 0x56 ||
      view.getUint32(4, true) !== 1
    ) return undefined
    const position = Object.freeze([
      view.getFloat32(16, true), view.getFloat32(20, true), view.getFloat32(24, true),
    ]) as readonly [number, number, number]
    const angles = Object.freeze([
      view.getFloat32(28, true), view.getFloat32(32, true), view.getFloat32(36, true),
    ]) as readonly [number, number, number]
    if (!position.every(Number.isFinite) || !angles.every(Number.isFinite)) return undefined
    const hammerId = view.getUint32(12, true)
    return Object.freeze({
      entity: view.getUint32(8, true),
      hammerId: hammerId === 0xffff_ffff ? null : hammerId,
      position,
      angles,
    })
  } finally {
    exports.playsrc_free(pointer, length)
  }
}

function copyOutput(
  exports: WasmExports,
  length: number,
  copy: (pointer: number, capacity: number) => number,
): ArrayBuffer | undefined {
  const pointer = exports.playsrc_alloc(length) >>> 0
  try {
    if (copy(pointer, length) !== length) return undefined
    return new Uint8Array(exports.memory.buffer, pointer, length).slice().buffer
  } finally {
    exports.playsrc_free(pointer, length)
  }
}

function load(request: Extract<WorkerRequest, { kind: "load" }>): void {
  const started = performance.now()
  const exports = wasm
  if (
    !exports ||
    !canonicalId(request.generation) ||
    request.generation <= Math.max(active?.generation ?? 0, pending?.generation ?? 0) ||
    (request.profile !== 0 && request.profile !== 1) ||
    !(request.bsp instanceof ArrayBuffer) ||
    request.bsp.byteLength < 1 ||
    request.bsp.byteLength > MAX_BSP_BYTES ||
    !(request.configuration instanceof ArrayBuffer) ||
    request.configuration.byteLength > MAX_CONFIGURATION_BYTES ||
    typeof request.includeMap !== "boolean" ||
    (request.presentation !== undefined && (
      !(request.presentation instanceof ArrayBuffer) ||
      request.presentation.byteLength < 1 ||
      request.presentation.byteLength > MAX_PRESENTATION_BYTES
    ))
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const inputCopyStarted = performance.now()
  const bspPointer = allocateCopy(exports, request.bsp)
  const configurationPointer = allocateCopy(exports, request.configuration)
  const presentationPointer = request.presentation ? allocateCopy(exports, request.presentation) : 0
  const inputCopyMilliseconds = performance.now() - inputCopyStarted
  const compileStarted = performance.now()
  const candidate = request.presentation
    ? exports.playsrc_compile_map_cached(
        bspPointer,
        request.bsp.byteLength,
        request.profile,
        configurationPointer,
        request.configuration.byteLength,
        presentationPointer,
        request.presentation.byteLength,
      )
    : exports.playsrc_compile_map(
        bspPointer,
        request.bsp.byteLength,
        request.profile,
        configurationPointer,
        request.configuration.byteLength,
      )
  const compileMilliseconds = performance.now() - compileStarted
  const resultStarted = performance.now()
  exports.playsrc_free(bspPointer, request.bsp.byteLength)
  exports.playsrc_free(configurationPointer, request.configuration.byteLength)
  if (request.presentation) exports.playsrc_free(presentationPointer, request.presentation.byteLength)
  const error = exports.playsrc_result_error(candidate)
  if (error !== 0) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "CompileFailed", error)
    return
  }
  const payloadBytes = exports.playsrc_result_length(candidate)
  const payloadSha256 = readHash(exports, candidate)
  const presentationBytes = exports.playsrc_presentation_length(candidate)
  const initialView = readInitialView(exports, candidate)
  const compileMetrics = Array.from({ length: 17 }, (_, index) => exports.playsrc_compile_metric_milliseconds(candidate, index))
  if (
    !Number.isSafeInteger(payloadBytes) || payloadBytes < 1 || payloadBytes > MAX_MESSAGE_BYTES ||
    payloadSha256 === undefined ||
    !Number.isSafeInteger(presentationBytes) || presentationBytes < 1 || presentationBytes > MAX_PRESENTATION_BYTES ||
    (request.presentation && request.presentation.byteLength !== presentationBytes) ||
    initialView === undefined
  ) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "CompileFailed", 5)
    return
  }
  let phase = performance.now()
  const payload = request.includeMap
    ? copyOutput(exports, payloadBytes, (pointer, capacity) => exports.playsrc_result_copy(candidate, pointer, capacity))
    : undefined
  const mapCopyMilliseconds = performance.now() - phase
  phase = performance.now()
  const presentation = request.presentation ?? copyOutput(
    exports,
    presentationBytes,
    (pointer, capacity) => exports.playsrc_presentation_copy(candidate, pointer, capacity),
  )
  const presentationCopyMilliseconds = performance.now() - phase
  if ((request.includeMap && !payload) || !presentation) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "InternalFailure")
    return
  }
  phase = performance.now()
  if (exports.playsrc_presentation_release(candidate) !== 1) {
    exports.playsrc_dispose(candidate)
    fail(request.id, "StaleGeneration")
    return
  }
  const presentationReleaseMilliseconds = performance.now() - phase
  if (pending) exports.playsrc_dispose(pending.handle)
  pending = { generation: request.generation, handle: candidate }
  post({
    id: request.id,
    kind: "loaded",
    generation: request.generation,
    payloadBytes,
    payloadSha256,
    ...(payload ? { payload } : {}),
    presentationBytes,
    presentation,
    initialView,
    timings: {
      queueMilliseconds: queueMilliseconds(request, started),
      inputCopyMilliseconds,
      compileMilliseconds,
      resultMilliseconds: performance.now() - resultStarted,
      mapCopyMilliseconds,
      presentationCopyMilliseconds,
      presentationReleaseMilliseconds,
      bspParseMilliseconds: compileMetrics[0]!,
      canonicalMapMilliseconds: compileMetrics[1]!,
      materialResolutionMilliseconds: compileMetrics[2]!,
      entityParseMilliseconds: compileMetrics[3]!,
      presentationCompileMilliseconds: compileMetrics[4]!,
      modelResolutionMilliseconds: compileMetrics[5]!,
      particleAndInputMilliseconds: compileMetrics[6]!,
      runtimeMapMilliseconds: compileMetrics[7]!,
      collisionSetupMilliseconds: compileMetrics[8]!,
      gameSetupMilliseconds: compileMetrics[9]!,
      presentationBundleMilliseconds: compileMetrics[11]!,
      presentationModelsMilliseconds: compileMetrics[12]!,
      presentationTexturesMilliseconds: compileMetrics[13]!,
      presentationParticlesMilliseconds: compileMetrics[14]!,
      presentationEnvironmentMilliseconds: compileMetrics[15]!,
      presentationSerializationMilliseconds: compileMetrics[16]!,
      textureDecoderRequests: exports.playsrc_texture_inspection_count(candidate, 0),
      textureMetadataInspections: exports.playsrc_texture_inspection_count(candidate, 1),
      totalMilliseconds: performance.now() - started,
    },
  }, [...(payload ? [payload] : []), presentation])
}

function requireActive(id: number, generation: number): { exports: WasmExports; handle: number } | undefined {
  if (!wasm || !active || active.generation !== generation) {
    fail(id, "StaleGeneration")
    return undefined
  }
  return { exports: wasm, handle: active.handle }
}

function readCoverage(request:Extract<WorkerRequest,{kind:"read-coverage"}>):void{if(!wasm||!pending||pending.generation!==request.generation){fail(request.id,"StaleGeneration");return}const length=wasm.playsrc_coverage_length(pending.handle);if(!Number.isSafeInteger(length)||length<12||length>4*1024*1024){fail(request.id,"InternalFailure");return}const pointer=wasm.playsrc_alloc(length)>>>0,copied=wasm.playsrc_coverage_copy(pending.handle,pointer,length);if(copied!==length){wasm.playsrc_free(pointer,length);fail(request.id,"InternalFailure");return}const payload=new Uint8Array(wasm.memory.buffer,pointer,length).slice().buffer;wasm.playsrc_free(pointer,length);post({id:request.id,kind:"coverage",generation:request.generation,payload},[payload])}

function teamSelection(request: Extract<WorkerRequest, { kind: "team-selection" }>): void {
  const selected = pending?.generation === request.generation
    ? pending
    : active?.generation === request.generation ? active : undefined
  if (!wasm || !selected) {
    fail(request.id, "StaleGeneration")
    return
  }
  const choices = { red: 2, blue: 3, spectate: 1, auto: 4 } as const
  if (request.choice !== null) {
    const code = choices[request.choice]
    if (code === undefined) {
      fail(request.id, "MalformedRequest")
      return
    }
    if (wasm.playsrc_team_select(selected.handle, code) !== 1) {
      fail(request.id, "TransitionFailed", 201)
      return
    }
  }
  const length = 12
  const pointer = wasm.playsrc_alloc(length) >>> 0
  try {
    if (wasm.playsrc_team_state_copy(selected.handle, pointer, length) !== length) {
      fail(request.id, "InternalFailure")
      return
    }
    const bytes = new Uint8Array(wasm.memory.buffer, pointer, length)
    if (bytes[0] !== 0x50 || bytes[1] !== 0x54 || bytes[2] !== 0x45 || bytes[3] !== 0x4d
      || new DataView(wasm.memory.buffer, pointer, length).getUint32(4, true) !== 1) {
      fail(request.id, "InternalFailure")
      return
    }
    const state = decodeTf2TeamSelectionServerState(bytes[8]!, bytes[9]!, bytes[10]!, bytes[11]!)
    post({ id: request.id, kind: "team-selection", generation: request.generation, state })
  } finally {
    wasm.playsrc_free(pointer, length)
  }
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
  const selected =
    pending?.generation === request.generation
      ? pending
      : active?.generation === request.generation
        ? active
        : undefined
  if (!wasm || !selected) {
    fail(request.id, "StaleGeneration")
    return
  }
  const value = { exports: wasm, handle: selected.handle }
  if (
    !(request.definition instanceof ArrayBuffer) ||
    request.definition.byteLength < 52 ||
    request.definition.byteLength > 64 * 1024
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const pointer = allocateCopy(value.exports, request.definition)
  const configured = value.exports.playsrc_jump_configure(value.handle, pointer, request.definition.byteLength)
  value.exports.playsrc_free(pointer, request.definition.byteLength)
  if (configured !== 1) {
    fail(request.id, "TransitionFailed",200)
    return
  }
  post({ id: request.id, kind: "course-configured", generation: request.generation })
}

function observe(request: Extract<WorkerRequest, { kind: "observe" }>): void {
  const started = performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (
    !(request.command instanceof ArrayBuffer) ||
    request.command.byteLength < 48 ||
    request.command.byteLength > 64 * 1024 ||
    !Number.isFinite(request.nowSeconds) || request.nowSeconds < 0 || typeof request.suspended !== "boolean"
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const inputCopyStarted = performance.now()
  const pointer = allocateCopy(value.exports, request.command)
  const inputCopyMilliseconds = performance.now() - inputCopyStarted
  const transactStarted = performance.now()
  const result = value.exports.playsrc_simulation_observe(value.handle, request.nowSeconds, pointer, request.command.byteLength, Number(request.suspended))
  const transactMilliseconds = performance.now() - transactStarted
  value.exports.playsrc_free(pointer, request.command.byteLength)
  if (result !== 1) {
    const length=value.exports.playsrc_simulation_error_length(),detailPointer=length?value.exports.playsrc_alloc(length)>>>0:0,copied=length?value.exports.playsrc_simulation_error_copy(detailPointer,length):0,reason=copied===length&&length?new TextDecoder().decode(new Uint8Array(value.exports.memory.buffer,detailPointer,length).slice()):undefined;if(detailPointer)value.exports.playsrc_free(detailPointer,length);fail(request.id,"TransitionFailed",value.exports.playsrc_simulation_error(),reason)
    return
  }
  const length = value.exports.playsrc_simulation_output_length(value.handle)
  if (!Number.isSafeInteger(length) || length < 16 || length > MAX_MESSAGE_BYTES) {
    fail(request.id, "InternalFailure")
    return
  }
  const outputCopyStarted = performance.now()
  const snapshotPointer = value.exports.playsrc_alloc(length) >>> 0
  const copied = value.exports.playsrc_simulation_output_copy(value.handle, snapshotPointer, length)
  if (copied !== length) {
    value.exports.playsrc_free(snapshotPointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const snapshot = new Uint8Array(value.exports.memory.buffer, snapshotPointer, length).slice().buffer
  value.exports.playsrc_free(snapshotPointer, length)
  const outputCopyMilliseconds = performance.now() - outputCopyStarted
  post({ id: request.id, kind: "simulation", generation: request.generation, output: snapshot, timings: { queueMilliseconds: queueMilliseconds(request, started), inputCopyMilliseconds, transactMilliseconds, outputCopyMilliseconds, totalMilliseconds: performance.now() - started } }, [snapshot])
}
function particles(request: Extract<WorkerRequest, { kind: "particles" }>): void {
  const started=performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (
    !(request.batch instanceof ArrayBuffer) ||
    request.batch.byteLength < 32 ||
    request.batch.byteLength > 4 * 1024 * 1024
  ) {
    fail(request.id, "MalformedRequest")
    return
  }
  const inputCopyStarted=performance.now(),pointer = allocateCopy(value.exports, request.batch),inputCopyMilliseconds=performance.now()-inputCopyStarted
  const transactStarted=performance.now()
  const ok = value.exports.playsrc_particle_transact(value.handle, pointer, request.batch.byteLength)
  const transactMilliseconds=performance.now()-transactStarted
  value.exports.playsrc_free(pointer, request.batch.byteLength)
  if (ok !== 1) {
    const length = value.exports.playsrc_simulation_error_length()
    const pointer = length ? value.exports.playsrc_alloc(length) >>> 0 : 0
    const copied = pointer ? value.exports.playsrc_simulation_error_copy(pointer, length) : 0
    const reason = copied === length && length
      ? new TextDecoder().decode(new Uint8Array(value.exports.memory.buffer, pointer, length).slice())
      : undefined
    if (pointer) value.exports.playsrc_free(pointer, length)
    fail(request.id, "TransitionFailed", 201, reason)
    return
  }
  const length = value.exports.playsrc_particle_output_length(value.handle)
  if (length < 12 || length > 64 * 1024 * 1024) {
    fail(request.id, "InternalFailure")
    return
  }
  const outputCopyStarted=performance.now(),outputPointer = value.exports.playsrc_alloc(length) >>> 0
  if (value.exports.playsrc_particle_output_copy(value.handle, outputPointer, length) !== length) {
    value.exports.playsrc_free(outputPointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const output = new Uint8Array(value.exports.memory.buffer, outputPointer, length).slice().buffer
  value.exports.playsrc_free(outputPointer, length)
  const outputCopyMilliseconds=performance.now()-outputCopyStarted
  post({ id: request.id, kind: "particles", generation: request.generation, output, timings:{queueMilliseconds:queueMilliseconds(request,started),inputCopyMilliseconds,transactMilliseconds,outputCopyMilliseconds,totalMilliseconds:performance.now()-started} }, [output])
}
function models(request: Extract<WorkerRequest, { kind: "models" }>): void {
  const started = performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (!(request.batch instanceof ArrayBuffer) || request.batch.byteLength < 12 || request.batch.byteLength > 1024 * 1024) {
    fail(request.id, "MalformedRequest")
    return
  }
  const inputCopyStarted = performance.now()
  const pointer = allocateCopy(value.exports, request.batch)
  const inputCopyMilliseconds = performance.now() - inputCopyStarted
  const transactStarted = performance.now()
  const ok = value.exports.playsrc_model_transact(value.handle, pointer, request.batch.byteLength)
  const transactMilliseconds = performance.now() - transactStarted
  value.exports.playsrc_free(pointer, request.batch.byteLength)
  if (ok !== 1) {
    fail(request.id, "TransitionFailed",202)
    return
  }
  const length = value.exports.playsrc_model_output_length(value.handle)
  if (length < 12 || length > 64 * 1024 * 1024) {
    fail(request.id, "InternalFailure")
    return
  }
  const outputCopyStarted = performance.now()
  const outputPointer = value.exports.playsrc_alloc(length) >>> 0
  if (value.exports.playsrc_model_output_copy(value.handle, outputPointer, length) !== length) {
    value.exports.playsrc_free(outputPointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const output = new Uint8Array(value.exports.memory.buffer, outputPointer, length).slice().buffer
  value.exports.playsrc_free(outputPointer, length)
  const outputCopyMilliseconds = performance.now() - outputCopyStarted
  post({ id: request.id, kind: "models", generation: request.generation, output, timings: { queueMilliseconds: queueMilliseconds(request, started), inputCopyMilliseconds, transactMilliseconds, outputCopyMilliseconds, totalMilliseconds: performance.now() - started } }, [output])
}
function visibility(request: Extract<WorkerRequest, { kind: "visibility" }>): void {
  const started = performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  const view = request.view
  const visibilityPosition=view.visibilityPosition??view.position
  if (view.position.length !== 3 || !view.position.every(Number.isFinite) || visibilityPosition.length!==3||!visibilityPosition.every(Number.isFinite) ||
    ![view.yawDegrees, view.pitchDegrees, view.verticalFovDegrees, view.aspectRatio, view.near, view.far, view.presentationTimeSeconds,view.areaFilter??-1].every(Number.isFinite) ||
    (view.areaFilter!==undefined&&(!Number.isSafeInteger(view.areaFilter)||view.areaFilter<0||view.areaFilter>511))||
    view.verticalFovDegrees <= 0 || view.verticalFovDegrees >= 180 || view.aspectRatio <= 0 || view.near <= 0 || view.far <= view.near || view.presentationTimeSeconds < 0) {
    fail(request.id, "MalformedRequest")
    return
  }
  const inputCopyStarted = performance.now()
  const pointer = value.exports.playsrc_alloc(56) >>> 0
  new Float32Array(value.exports.memory.buffer, pointer, 14).set([
    ...visibilityPosition,...view.position, view.yawDegrees, view.pitchDegrees, view.verticalFovDegrees,
    view.aspectRatio, view.near, view.far, view.presentationTimeSeconds,view.areaFilter??-1,
  ])
  const inputCopyMilliseconds = performance.now() - inputCopyStarted
  const transactStarted = performance.now()
  const ok = value.exports.playsrc_visibility_query(value.handle, pointer)
  const transactMilliseconds = performance.now() - transactStarted
  value.exports.playsrc_free(pointer, 56)
  if (ok !== 1) {
    fail(request.id, "TransitionFailed",203)
    return
  }
  const length = value.exports.playsrc_visibility_output_length(value.handle)
  if (length < 80 || length > 4 * 1024 * 1024) {
    fail(request.id, "InternalFailure")
    return
  }
  const outputCopyStarted = performance.now()
  const outputPointer = value.exports.playsrc_alloc(length) >>> 0
  if (value.exports.playsrc_visibility_output_copy(value.handle, outputPointer, length) !== length) {
    value.exports.playsrc_free(outputPointer, length)
    fail(request.id, "InternalFailure")
    return
  }
  const output = new Uint8Array(value.exports.memory.buffer, outputPointer, length).slice().buffer
  value.exports.playsrc_free(outputPointer, length)
  const outputCopyMilliseconds = performance.now() - outputCopyStarted
  post({ id: request.id, kind: "visibility", generation: request.generation, output, timings: { queueMilliseconds: queueMilliseconds(request, started), inputCopyMilliseconds, transactMilliseconds, outputCopyMilliseconds, totalMilliseconds: performance.now() - started } }, [output])
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
    case "initialize":
      return initialize(request)
    case "decode-resources":
      return decodeResources(request)
    case "load":
      return load(request)
    case "read-coverage":
      return readCoverage(request)
    case "activate":
      return activate(request)
    case "team-selection":
      return teamSelection(request)
    case "discard":
      return discard(request)
    case "configure-course":
      return configureCourse(request)
    case "observe":
      return observe(request)
    case "particles":
      return particles(request)
    case "models": {
      const companion = request.visibility
      if (companion && (!canonicalId(companion.id) || companion.id === request.id || !Number.isFinite(companion.queuedAt))) {
        fail(request.id, "MalformedRequest")
        if (canonicalId(companion.id) && companion.id !== request.id) fail(companion.id, "MalformedRequest")
        return
      }
      try {
        models(request)
      } catch {
        fail(request.id, "InternalFailure", 903)
      }
      if (companion) {
        try {
          visibility({
            id: companion.id,
            kind: "visibility",
            generation: request.generation,
            queuedAt: companion.queuedAt,
            view: companion.view,
          })
        } catch {
          fail(companion.id, "InternalFailure", 904)
        }
      }
      return
    }
    case "visibility":
      return visibility(request)
    case "shutdown":
      return shutdown(request)
    default:
      return fail((request as { id: number }).id, "MalformedRequest")
  }
}

let initializing: Promise<void> | undefined
const deferred: WorkerRequest[] = []

function unexpected(request: WorkerRequest, error: unknown): void {
  if (!canonicalId(request?.id)) return
  fail(
    request.id,
    "InternalFailure",
    ({ observe: 901, particles: 902, models: 903, visibility: 904 } as Record<string, number>)[request.kind] ?? 999,
    error instanceof Error ? `${error.name}:${error.message}` : String(error),
  )
}

function handle(request: WorkerRequest): void {
  try {
    const result = dispatch(request)
    if (result instanceof Promise) {
      initializing = result.catch((error: unknown) => unexpected(request, error)).finally(() => {
        initializing = undefined
        while (deferred.length > 0 && !initializing) handle(deferred.shift()!)
      })
    }
  } catch (error) {
    unexpected(request, error)
  }
}

scope.onmessage = (event: MessageEvent<WorkerRequest>) => {
  if (initializing) deferred.push(event.data)
  else handle(event.data)
}
