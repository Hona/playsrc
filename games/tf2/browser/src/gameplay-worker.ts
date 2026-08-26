/// <reference lib="webworker" />

import { TF2_PRESENTATION_SCHEMA, type InitialView, type WorkerFailureCode, type WorkerRequest, type WorkerResponse } from "./protocol"
import { ResourceGenerations } from "./resource-generations"
import { decodeTf2TeamSelectionServerState } from "./team-selection/model"
import initializeWasm, { initThreadPool } from "./wasm-generated/tf2_wasm.js"

const MAX_WASM_BYTES = 64 * 1024 * 1024
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 1024 * 1024 * 1024
const MAX_MESSAGE_BYTES = 512 * 1024 * 1024
const MAX_PRESENTATION_BYTES = 512 * 1024 * 1024
const MAX_RESOURCE_SECTION_BYTES = 32 * 1024 * 1024
declare const __PLAYSRC_APPLICATION_BUILD__: string

type WasmExports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_resource_decode_authenticated(pointer: number, length: number): number
  playsrc_resource_length(): number
  playsrc_resource_take(): number
  playsrc_resource_release(pointer: number, length: number): number
  playsrc_resource_sections_hash(sections: number, count: number, output: number): number
  playsrc_compile_map(bsp: number, length: number, profile: number, sections: number, sectionCount: number, configurationSha256: number): number
  playsrc_compile_map_cached(bsp: number, length: number, profile: number, sections: number, sectionCount: number, configurationSha256: number, presentation: number, presentationLength: number): number
  playsrc_compile_metric_milliseconds(handle: number, index: number): number
  playsrc_memory_bytes(index: number): number
  playsrc_compile_memory_bytes(handle: number, index: number): number
  playsrc_texture_inspection_count(handle: number, index: number): number
  playsrc_model_cache_count(handle: number, index: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_take(handle: number): number
  playsrc_result_release(handle: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_presentation_length(handle: number): number
  playsrc_presentation_copy(handle: number, pointer: number, capacity: number): number
  playsrc_presentation_take(handle: number): number
  playsrc_presentation_release(handle:number):number
  playsrc_coverage_length(handle:number):number
  playsrc_coverage_copy(handle:number,pointer:number,capacity:number):number
  playsrc_particle_transact(handle: number, pointer: number, length: number): number
  playsrc_particle_output_length(handle: number): number
  playsrc_particle_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_model_transact(handle: number, pointer: number, length: number): number
  playsrc_model_output_length(handle: number): number
  playsrc_model_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_model_output_take(handle: number): number
  playsrc_model_output_capacity(handle: number): number
  playsrc_model_output_recycle(handle: number, pointer: number, capacity: number): void
  playsrc_visibility_query(handle: number, pointer: number): number
  playsrc_visibility_output_length(handle: number): number
  playsrc_visibility_output_pointer(handle: number): number
  playsrc_spawn_copy(handle: number, pointer: number, capacity: number): number
  playsrc_team_state_copy(handle: number, pointer: number, capacity: number): number
  playsrc_team_select(handle: number, choice: number): number
  playsrc_jump_configure(handle: number, definition: number, length: number): number
  playsrc_player_set_position(handle: number, x: number, y: number, z: number): number
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
const resourceSets = new ResourceGenerations((section) => {
  if (section.authoredBacking) wasm!.playsrc_resource_release(section.pointer, section.length)
  else wasm!.playsrc_free(section.pointer, section.length)
})
const modelOutputLeases = new Map<number, { generation: number; handle: number; pointer: number; capacity: number }>()
let leasedModelBytes = 0
let closing: Extract<WorkerRequest, { kind: "shutdown" }> | undefined

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
  if (request.applicationBuild !== __PLAYSRC_APPLICATION_BUILD__ || request.presentationSchema !== TF2_PRESENTATION_SCHEMA) {
    fail(request.id, "GenerationMismatch")
    return
  }
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
        candidate.playsrc_resource_decode_authenticated,
        candidate.playsrc_resource_length,
        candidate.playsrc_resource_take,
        candidate.playsrc_resource_release,
        candidate.playsrc_resource_sections_hash,
        candidate.playsrc_model_cache_count,
        candidate.playsrc_compile_map,
        candidate.playsrc_compile_map_cached,
        candidate.playsrc_compile_metric_milliseconds,
        candidate.playsrc_memory_bytes,
        candidate.playsrc_compile_memory_bytes,
        candidate.playsrc_result_length,
        candidate.playsrc_result_error,
        candidate.playsrc_result_copy,
        candidate.playsrc_result_take,
        candidate.playsrc_result_release,
        candidate.playsrc_result_hash,
        candidate.playsrc_presentation_length,
        candidate.playsrc_presentation_copy,
        candidate.playsrc_presentation_take,
        candidate.playsrc_presentation_release,
        candidate.playsrc_coverage_length,
        candidate.playsrc_coverage_copy,
        candidate.playsrc_particle_transact,
        candidate.playsrc_particle_output_length,
        candidate.playsrc_particle_output_copy,
        candidate.playsrc_model_transact,
        candidate.playsrc_model_output_length,
        candidate.playsrc_model_output_copy,
        candidate.playsrc_model_output_take,
        candidate.playsrc_model_output_capacity,
        candidate.playsrc_model_output_recycle,
        candidate.playsrc_visibility_query,
        candidate.playsrc_visibility_output_length,
        candidate.playsrc_visibility_output_pointer,
        candidate.playsrc_spawn_copy,
        candidate.playsrc_team_state_copy,
        candidate.playsrc_team_select,
        candidate.playsrc_jump_configure,
        candidate.playsrc_player_set_position,
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
    Object.defineProperty(scope, "__playsrcWorkerMemory", {
      configurable: true,
      get: () => Object.freeze({
        linearBytes: candidate.memory.buffer.byteLength,
        liveBytes: candidate.playsrc_memory_bytes(0),
        highWaterBytes: candidate.playsrc_memory_bytes(1),
        borrowedModelSourceBytes: candidate.playsrc_memory_bytes(2),
        copiedModelSourceBytes: candidate.playsrc_memory_bytes(3),
        modelSourceSectionBytes: candidate.playsrc_memory_bytes(4),
        resourceBytes: [...new Set([...resourceSets.values()].flatMap((retained) => retained.sections))]
          .reduce((total, section) => total + section.length, 0),
        resourceReferencedBytes: [...resourceSets.values()].reduce((total, retained) => total + retained.sections.reduce((sum, section) => sum + section.length, 0), 0),
        sharedResourceBytes: [...new Set([...resourceSets.values()].flatMap((retained) => retained.sections))]
          .filter((section) => section.references > 1).reduce((total, section) => total + section.length, 0),
        resourceSections: [...resourceSets.entries()].map(([generation, retained]) => Object.freeze({
          generation,
          bytes: retained.sections.map((section) => section.length),
        })),
        shared: candidate.memory.buffer instanceof SharedArrayBuffer,
      }),
    })
    post({ id: request.id, kind: "initialized", applicationBuild: __PLAYSRC_APPLICATION_BUILD__, presentationSchema: TF2_PRESENTATION_SCHEMA, wasmSha256: actual })
  } catch {
    fail(request.id, "WasmUnavailable")
  }
}

function decodeResources(request: Extract<WorkerRequest, { kind: "decode-resources" }>): void {
  const exports = wasm
  if (!exports || !Array.isArray(request.chunks) || request.chunks.length < 1 || request.chunks.length > 1_024
    || typeof request.shared !== "boolean" || request.shared !== (request.generation !== undefined)
    || (request.generation !== undefined && (!resourceSets.writable(request.generation) || request.generation <= (active?.generation ?? 0)))) {
    fail(request.id, "MalformedRequest")
    return
  }
  let batchLength = 12
  for (const chunk of request.chunks) {
    if (!(chunk?.descriptor instanceof ArrayBuffer) || chunk.descriptor.byteLength < 1 || chunk.descriptor.byteLength > 8 * 1024 * 1024
      || !(chunk.bytes instanceof ArrayBuffer) || chunk.bytes.byteLength < 1 || chunk.bytes.byteLength > 32 * 1024 * 1024) {
      fail(request.id, "MalformedRequest")
      return
    }
    batchLength += 8 + chunk.descriptor.byteLength + chunk.bytes.byteLength
    if (batchLength > MAX_MESSAGE_BYTES) {
      fail(request.id, "MalformedRequest")
      return
    }
  }
  const input = exports.playsrc_alloc(batchLength) >>> 0
  let decoded: number
  try {
    const bytes = new Uint8Array(exports.memory.buffer, input, batchLength)
    const view = new DataView(exports.memory.buffer, input, batchLength)
    bytes.set([0x50, 0x53, 0x47, 0x42])
    view.setUint32(4, 1, true)
    view.setUint32(8, request.chunks.length, true)
    let offset = 12
    for (const chunk of request.chunks) {
      view.setUint32(offset, chunk.descriptor.byteLength, true)
      offset += 4
      bytes.set(new Uint8Array(chunk.descriptor), offset)
      offset += chunk.descriptor.byteLength
      view.setUint32(offset, chunk.bytes.byteLength, true)
      offset += 4
      bytes.set(new Uint8Array(chunk.bytes), offset)
      offset += chunk.bytes.byteLength
    }
    // The application transfers immutable objects acquired by their authenticated
    // graph descriptors; Rust still checks every decoded entry's exact digest.
    decoded = exports.playsrc_resource_decode_authenticated(input, batchLength)
  } finally {
    exports.playsrc_free(input, batchLength)
  }
  if (decoded !== 1) {
    fail(request.id, "CompileFailed")
    return
  }
  const length = exports.playsrc_resource_length()
  if (!Number.isSafeInteger(length) || length < 12 || length > MAX_RESOURCE_SECTION_BYTES) {
    fail(request.id, "InternalFailure")
    return
  }
  const pointer = exports.playsrc_resource_take() >>> 0
  if (pointer === 0) {
    fail(request.id, "InternalFailure")
    return
  }
  if (request.generation !== undefined) {
    const memory = exports.memory.buffer
    if (!(memory instanceof SharedArrayBuffer)) {
      exports.playsrc_resource_release(pointer, length)
      fail(request.id, "InternalFailure")
      return
    }
    resourceSets.adopt(request.generation, { pointer, length, authoredBacking: true })
    post({ id: request.id, kind: "resources", bytes: memory, byteOffset: pointer, byteLength: length })
    return
  }
  const bytes = new Uint8Array(exports.memory.buffer, pointer, length).slice()
  exports.playsrc_resource_release(pointer, length)
  post({ id: request.id, kind: "resources", bytes: bytes.buffer, byteOffset: 0, byteLength: length }, [bytes.buffer])
}

function sectionTable(exports: WasmExports, sections: readonly { pointer: number; length: number }[]): number {
  const pointer = exports.playsrc_alloc(sections.length * 8) >>> 0
  const table = new DataView(exports.memory.buffer, pointer, sections.length * 8)
  for (const [index, section] of sections.entries()) {
    table.setUint32(index * 8, section.pointer, true)
    table.setUint32(index * 8 + 4, section.length, true)
  }
  return pointer
}

function releaseResourceSet(_exports: WasmExports, generation: number): boolean {
  return resourceSets.release(generation)
}

function finalizeResources(request: Extract<WorkerRequest, { kind: "finalize-resources" }>): void {
  const exports = wasm
  const retained = resourceSets.get(request.generation)
  if (!exports || !canonicalId(request.generation) || !retained || retained.sections.length < 1 || retained.sections.length > 1024) {
    fail(request.id, "MalformedRequest")
    return
  }
  if (request.authenticatedIdentity) {
    const identity = request.authenticatedIdentity
    const byteLength = 12 + retained.sections.reduce((total, section) => total + section.length - 12, 0)
    if (!Number.isSafeInteger(identity.byteLength) || identity.byteLength !== byteLength
      || byteLength < 12 || byteLength > MAX_CONFIGURATION_BYTES || !/^[0-9a-f]{64}$/.test(identity.sha256)) {
      releaseResourceSet(exports, request.generation)
      fail(request.id, "MalformedRequest")
      return
    }
    retained.byteLength = byteLength
    retained.sha256 = identity.sha256
    post({ id: request.id, kind: "resources-finalized", generation: request.generation, byteLength, sha256: identity.sha256, sections: retained.sections.length })
    return
  }
  const table = sectionTable(exports, retained.sections)
  const digest = exports.playsrc_alloc(32) >>> 0
  try {
    const byteLength = exports.playsrc_resource_sections_hash(table, retained.sections.length, digest)
    if (!Number.isSafeInteger(byteLength) || byteLength < 12 || byteLength > MAX_CONFIGURATION_BYTES) {
      releaseResourceSet(exports, request.generation)
      fail(request.id, "CompileFailed")
      return
    }
    const sha256 = Array.from(new Uint8Array(exports.memory.buffer, digest, 32), (value) => value.toString(16).padStart(2, "0")).join("")
    retained.byteLength = byteLength
    retained.sha256 = sha256
    post({ id: request.id, kind: "resources-finalized", generation: request.generation, byteLength, sha256, sections: retained.sections.length })
  } finally {
    exports.playsrc_free(table, retained.sections.length * 8)
    exports.playsrc_free(digest, 32)
  }
}

function retainResources(request: Extract<WorkerRequest, { kind: "retain-resources" }>): void {
  const exports = wasm
  if (!exports || !canonicalId(request.generation) || request.generation <= (active?.generation ?? 0)
    || (!("section" in request) && (!("sourceGeneration" in request) || !canonicalId(request.sourceGeneration)
      || request.sourceGeneration >= request.generation || !Number.isSafeInteger(request.sectionIndex) || request.sectionIndex < 0))
    || ("section" in request && (!(request.section instanceof ArrayBuffer) || request.section.byteLength < 12 || request.section.byteLength > MAX_RESOURCE_SECTION_BYTES))) {
    fail(request.id, "MalformedRequest")
    return
  }
  if (!resourceSets.writable(request.generation)) {
    fail(request.id, "MalformedRequest")
    return
  }
  if ("section" in request) {
    resourceSets.adopt(request.generation, { pointer: allocateCopy(exports, request.section), length: request.section.byteLength, authoredBacking: false })
  } else {
    if (!resourceSets.retain(request.generation, request.sourceGeneration, request.sectionIndex)) {
      fail(request.id, "MalformedRequest")
      return
    }
  }
  post({ id: request.id, kind: "resources-retained", generation: request.generation })
}

function releaseResources(request: Extract<WorkerRequest, { kind: "release-resources" }>): void {
  if (!wasm || !canonicalId(request.generation)) {
    fail(request.id, "MalformedRequest")
    return
  }
  if (request.generation === active?.generation || request.generation === pending?.generation) {
    fail(request.id, "StaleGeneration")
    return
  }
  releaseResourceSet(wasm, request.generation)
  post({ id: request.id, kind: "resources-released", generation: request.generation })
}

function allocateCopy(exports: WasmExports, bytes: ArrayBuffer | SharedArrayBuffer): number {
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

function takeOutput(
  exports: WasmExports,
  length: number,
  take: () => number,
): ArrayBuffer | undefined {
  const pointer = take() >>> 0
  if (pointer === 0) return undefined
  try {
    return new Uint8Array(exports.memory.buffer, pointer, length).slice().buffer
  } finally {
    exports.playsrc_free(pointer, length)
  }
}

function load(request: Extract<WorkerRequest, { kind: "load" }>): void {
  const started = performance.now()
  const exports = wasm
  const configuration = resourceSets.get(request.generation)
  if (
    !exports ||
    !canonicalId(request.generation) ||
    request.generation <= Math.max(active?.generation ?? 0, pending?.generation ?? 0) ||
    (request.profile !== 0 && request.profile !== 1) ||
    !(request.bsp instanceof ArrayBuffer) ||
    request.bsp.byteLength < 1 ||
    request.bsp.byteLength > MAX_BSP_BYTES ||
    !configuration ||
    !Number.isSafeInteger(request.configurationBytes) ||
    request.configurationBytes < 12 || request.configurationBytes > MAX_CONFIGURATION_BYTES ||
    !/^[0-9a-f]{64}$/.test(request.configurationSha256) ||
    configuration.byteLength !== request.configurationBytes || configuration.sha256 !== request.configurationSha256 ||
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
  const configurationPointer = sectionTable(exports, configuration.sections)
  const configurationHashPointer = exports.playsrc_alloc(32) >>> 0
  new Uint8Array(exports.memory.buffer, configurationHashPointer, 32).set(
    Array.from({ length: 32 }, (_, index) => Number.parseInt(request.configurationSha256.slice(index * 2, index * 2 + 2), 16)),
  )
  const presentationPointer = request.presentation ? allocateCopy(exports, request.presentation) : 0
  const inputCopyMilliseconds = performance.now() - inputCopyStarted
  const compileStarted = performance.now()
  const candidate = request.presentation
    ? exports.playsrc_compile_map_cached(
        bspPointer,
        request.bsp.byteLength,
        request.profile,
        configurationPointer,
        configuration.sections.length,
        configurationHashPointer,
        presentationPointer,
        request.presentation.byteLength,
      )
    : exports.playsrc_compile_map(
        bspPointer,
        request.bsp.byteLength,
        request.profile,
        configurationPointer,
        configuration.sections.length,
        configurationHashPointer,
      )
  const compileMilliseconds = performance.now() - compileStarted
  const resultStarted = performance.now()
  exports.playsrc_free(bspPointer, request.bsp.byteLength)
  exports.playsrc_free(configurationPointer, configuration.sections.length * 8)
  exports.playsrc_free(configurationHashPointer, 32)
  if (request.presentation) exports.playsrc_free(presentationPointer, request.presentation.byteLength)
  const error = exports.playsrc_result_error(candidate)
  if (error !== 0) {
    exports.playsrc_dispose(candidate)
    releaseResourceSet(exports, request.generation)
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
    releaseResourceSet(exports, request.generation)
    fail(request.id, "CompileFailed", 5)
    return
  }
  let phase = performance.now()
  const payload = request.includeMap
    ? takeOutput(exports, payloadBytes, () => exports.playsrc_result_take(candidate))
    : undefined
  if (!request.includeMap && exports.playsrc_result_release(candidate) !== 1) {
    exports.playsrc_dispose(candidate)
    releaseResourceSet(exports, request.generation)
    fail(request.id, "InternalFailure")
    return
  }
  const mapCopyMilliseconds = performance.now() - phase
  phase = performance.now()
  const presentation = request.presentation ?? takeOutput(
    exports,
    presentationBytes,
    () => exports.playsrc_presentation_take(candidate),
  )
  const presentationCopyMilliseconds = performance.now() - phase
  if ((request.includeMap && !payload) || !presentation) {
    exports.playsrc_dispose(candidate)
    releaseResourceSet(exports, request.generation)
    fail(request.id, "InternalFailure", request.includeMap && !payload ? 801 : 802)
    return
  }
  phase = performance.now()
  if (exports.playsrc_presentation_release(candidate) !== 1) {
    exports.playsrc_dispose(candidate)
    releaseResourceSet(exports, request.generation)
    fail(request.id, "StaleGeneration")
    return
  }
  const presentationReleaseMilliseconds = performance.now() - phase
  if (pending) {
    exports.playsrc_dispose(pending.handle)
    releaseResourceSet(exports, pending.generation)
  }
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
      modelCacheHits: exports.playsrc_model_cache_count(candidate, 0),
      modelCacheMisses: exports.playsrc_model_cache_count(candidate, 1),
      wasmLinearMemoryBytes: exports.memory.buffer.byteLength,
      wasmAllocatorLiveBytes: exports.playsrc_memory_bytes(0),
      wasmAllocatorHighWaterBytes: exports.playsrc_memory_bytes(1),
      wasmCompileOwnerBytes: Array.from({ length: 12 }, (_, index) => exports.playsrc_compile_memory_bytes(candidate, index)),
      resourceSections: configuration.sections.length,
      resourceBytes: request.configurationBytes,
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

function readCoverage(request:Extract<WorkerRequest,{kind:"read-coverage"}>):void{if(!wasm||!pending||pending.generation!==request.generation){fail(request.id,"StaleGeneration");return}const length=wasm.playsrc_coverage_length(pending.handle);if(!Number.isSafeInteger(length)||length<12||length>4*1024*1024){fail(request.id,"InternalFailure",820);return}const pointer=wasm.playsrc_alloc(length)>>>0,copied=wasm.playsrc_coverage_copy(pending.handle,pointer,length);if(copied!==length){wasm.playsrc_free(pointer,length);fail(request.id,"InternalFailure",821);return}const payload=new Uint8Array(wasm.memory.buffer,pointer,length).slice().buffer;wasm.playsrc_free(pointer,length);post({id:request.id,kind:"coverage",generation:request.generation,payload},[payload])}

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
  if (active) {
    wasm.playsrc_dispose(active.handle)
    releaseResourceSet(wasm, active.generation)
  }
  active = pending
  pending = undefined
  post({ id: request.id, kind: "activated", generation: request.generation })
}

function discard(request: Extract<WorkerRequest, { kind: "discard" }>): void {
  if (!wasm || !canonicalId(request.generation) || request.generation === active?.generation) {
    fail(request.id, "StaleGeneration")
    return
  }
  if (pending?.generation === request.generation) {
    wasm.playsrc_dispose(pending.handle)
    pending = undefined
  }
  releaseResourceSet(wasm, request.generation)
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

function setPosition(request: Extract<WorkerRequest, { kind: "set-position" }>): void {
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (!Array.isArray(request.position) || request.position.length !== 3 || !request.position.every(Number.isFinite)) {
    fail(request.id, "MalformedRequest")
    return
  }
  if (value.exports.playsrc_player_set_position(value.handle, request.position[0], request.position[1], request.position[2]) !== 1) {
    fail(request.id, "TransitionFailed", 202)
    return
  }
  post({ id: request.id, kind: "position-set", generation: request.generation })
}

function observe(request: Extract<WorkerRequest, { kind: "observe" }>): void {
  const started = performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (
    !(request.command instanceof ArrayBuffer) ||
    request.command.byteLength < 84 ||
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
    fail(request.id, "InternalFailure", 812)
    return
  }
  const outputCopyStarted = performance.now()
  const snapshotPointer = value.exports.playsrc_alloc(length) >>> 0
  const copied = value.exports.playsrc_simulation_output_copy(value.handle, snapshotPointer, length)
  if (copied !== length) {
    value.exports.playsrc_free(snapshotPointer, length)
    fail(request.id, "InternalFailure", 813)
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
    fail(request.id, "InternalFailure", 814)
    return
  }
  const outputCopyStarted=performance.now(),outputPointer = value.exports.playsrc_alloc(length) >>> 0
  if (value.exports.playsrc_particle_output_copy(value.handle, outputPointer, length) !== length) {
    value.exports.playsrc_free(outputPointer, length)
    fail(request.id, "InternalFailure", 815)
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
  if (modelOutputLeases.size >= 64 || modelOutputLeases.has(request.id)) {
    fail(request.id, "MalformedRequest")
    return
  }
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
    fail(request.id, "InternalFailure", 816)
    return
  }
  const outputCopyStarted = performance.now()
  const capacity = value.exports.playsrc_model_output_capacity(value.handle)
  if (!Number.isSafeInteger(capacity) || capacity < length || capacity > 128 * 1024 * 1024 - leasedModelBytes) {
    fail(request.id, "InternalFailure", 817)
    return
  }
  const outputPointer = value.exports.playsrc_model_output_take(value.handle) >>> 0
  const output = value.exports.memory.buffer
  if (outputPointer === 0 || !(output instanceof SharedArrayBuffer)) {
    if (outputPointer !== 0) value.exports.playsrc_model_output_recycle(value.handle, outputPointer, capacity)
    fail(request.id, "InternalFailure", 817)
    return
  }
  modelOutputLeases.set(request.id, { generation: request.generation, handle: value.handle, pointer: outputPointer, capacity })
  leasedModelBytes += capacity
  const outputCopyMilliseconds = performance.now() - outputCopyStarted
  try {
    post({ id: request.id, kind: "models", generation: request.generation, output, byteOffset: outputPointer, byteLength: length, lease: request.id, timings: { queueMilliseconds: queueMilliseconds(request, started), inputCopyMilliseconds, transactMilliseconds, outputCopyMilliseconds, totalMilliseconds: performance.now() - started } })
  } catch (error) {
    releaseModelOutput({ id: request.id, kind: "release-model-output", generation: request.generation, lease: request.id })
    throw error
  }
}

function releaseModelOutput(request: Extract<WorkerRequest, { kind: "release-model-output" }>): void {
  const retained = modelOutputLeases.get(request.lease)
  if (!retained || retained.generation !== request.generation || request.id !== request.lease || !wasm) return
  modelOutputLeases.delete(request.lease)
  leasedModelBytes -= retained.capacity
  wasm.playsrc_model_output_recycle(retained.handle, retained.pointer, retained.capacity)
  if (closing && modelOutputLeases.size === 0) {
    const request = closing
    closing = undefined
    shutdown(request)
  }
}
function visibility(request: Extract<WorkerRequest, { kind: "visibility" }>): void {
  const started = performance.now()
  const value = requireActive(request.id, request.generation)
  if (!value) return
  if (!Array.isArray(request.views) || request.views.length < 1 || request.views.length > 2 || request.views.some(view => {
    const visibilityPosition = view.visibilityPosition ?? view.position
    return view.position.length !== 3 || !view.position.every(Number.isFinite) || visibilityPosition.length !== 3 || !visibilityPosition.every(Number.isFinite)
      || ![view.yawDegrees, view.pitchDegrees, view.verticalFovDegrees, view.aspectRatio, view.near, view.far, view.presentationTimeSeconds, view.areaFilter ?? -1].every(Number.isFinite)
      || (view.areaFilter !== undefined && (!Number.isSafeInteger(view.areaFilter) || view.areaFilter < 0 || view.areaFilter > 511))
      || view.verticalFovDegrees <= 0 || view.verticalFovDegrees >= 180 || view.aspectRatio <= 0 || view.near <= 0 || view.far <= view.near || view.presentationTimeSeconds < 0
  })) {
    fail(request.id, "MalformedRequest")
    return
  }
  const pointer = value.exports.playsrc_alloc(56) >>> 0
  let inputCopyMilliseconds = 0, transactMilliseconds = 0, outputCopyMilliseconds = 0
  const outputs: ArrayBuffer[] = []
  try {
    for (const view of request.views) {
      const inputCopyStarted = performance.now()
      new Float32Array(value.exports.memory.buffer, pointer, 14).set([
        ...(view.visibilityPosition ?? view.position), ...view.position, view.yawDegrees, view.pitchDegrees,
        view.verticalFovDegrees, view.aspectRatio, view.near, view.far, view.presentationTimeSeconds, view.areaFilter ?? -1,
      ])
      inputCopyMilliseconds += performance.now() - inputCopyStarted
      const transactStarted = performance.now()
      const ok = value.exports.playsrc_visibility_query(value.handle, pointer)
      transactMilliseconds += performance.now() - transactStarted
      if (ok !== 1) { fail(request.id, "TransitionFailed", 203); return }
      const length = value.exports.playsrc_visibility_output_length(value.handle)
      if (length < 80 || length > 4 * 1024 * 1024) { fail(request.id, "InternalFailure", 818); return }
      const outputCopyStarted = performance.now()
      const outputPointer = value.exports.playsrc_visibility_output_pointer(value.handle) >>> 0
      if (outputPointer === 0) { fail(request.id, "InternalFailure", 819); return }
      outputs.push(new Uint8Array(value.exports.memory.buffer, outputPointer, length).slice().buffer)
      outputCopyMilliseconds += performance.now() - outputCopyStarted
    }
  } finally { value.exports.playsrc_free(pointer, 56) }
  post({ id: request.id, kind: "visibility", generation: request.generation, outputs, timings: { queueMilliseconds: queueMilliseconds(request, started), inputCopyMilliseconds, transactMilliseconds, outputCopyMilliseconds, totalMilliseconds: performance.now() - started } }, outputs)
}

function shutdown(request: Extract<WorkerRequest, { kind: "shutdown" }>): void {
  // A queued shutdown can run before the main thread has read an earlier response.
  // Keep its immutable allocation alive until the decoder acknowledges ownership.
  if (modelOutputLeases.size > 0) {
    closing = request
    return
  }
  if (wasm && active) wasm.playsrc_dispose(active.handle)
  if (wasm && pending) wasm.playsrc_dispose(pending.handle)
  if (wasm) for (const generation of resourceSets.keys()) releaseResourceSet(wasm, generation)
  active = undefined
  pending = undefined
  wasm = undefined
  post({ id: request.id, kind: "shutdown" })
}

function dispatch(request: WorkerRequest): void | Promise<void> {
  if (!request || !canonicalId(request.id) || typeof request.kind !== "string") return
  if (closing && request.kind !== "release-model-output") {
    fail(request.id, "TransitionFailed")
    return
  }
  switch (request.kind) {
    case "initialize":
      return initialize(request)
    case "decode-resources":
      return decodeResources(request)
    case "finalize-resources":
      return finalizeResources(request)
    case "retain-resources":
      return retainResources(request)
    case "release-resources":
      return releaseResources(request)
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
    case "set-position":
      return setPosition(request)
    case "observe":
      return observe(request)
    case "particles":
      return particles(request)
    case "release-model-output":
      return releaseModelOutput(request)
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
            views: companion.views,
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
