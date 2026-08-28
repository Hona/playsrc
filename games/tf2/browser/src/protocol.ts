import type { Tf2TeamChoice, Tf2TeamSelectionServerState } from "./team-selection/model"
import type { Tf2EquipmentState } from "./equipment/types"

export const TF2_PRESENTATION_SCHEMA = 21

export type VisibilityView = Readonly<{
  position: readonly [number, number, number]
  visibilityPosition?: readonly [number, number, number]
  areaFilter?: number
  yawDegrees: number
  pitchDegrees: number
  verticalFovDegrees: number
  aspectRatio: number
  near: number
  far: number
  presentationTimeSeconds: number
}>

type WorkerEnvelope = Readonly<{ queuedAt?: number }>

export type WorkerRequest = WorkerEnvelope & (
  | Readonly<{ id: number; kind: "initialize"; applicationBuild: string; presentationSchema: number; wasm: ArrayBuffer; wasmSha256: string; threads: number }>
  | Readonly<{
      id: number
      kind: "decode-resources"
      chunks: readonly Readonly<{ descriptor: ArrayBuffer; bytes: ArrayBuffer }>[]
      shared: boolean
      generation?: number
    }>
  | Readonly<{ id: number; kind: "finalize-resources"; generation: number; authenticatedIdentity?: Readonly<{ byteLength: number; sha256: string }> }>
  | Readonly<{ id: number; kind: "retain-resources"; generation: number; section: ArrayBuffer }>
  | Readonly<{ id: number; kind: "retain-resources"; generation: number; sourceGeneration: number; sectionIndex: number }>
  | Readonly<{ id: number; kind: "release-resources"; generation: number }>
  | Readonly<{
      id: number
      kind: "load"
      generation: number
      profile: 0 | 1
      bsp: ArrayBuffer
      configurationSha256: string
      configurationBytes: number
      includeMap: boolean
      presentation?: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "read-coverage"; generation: number }>
  | Readonly<{ id: number; kind: "activate"; generation: number }>
  | Readonly<{ id: number; kind: "team-selection"; generation: number; choice: Tf2TeamChoice | null }>
  | Readonly<{ id: number; kind: "equipment"; generation: number; mutation?: ArrayBuffer }>
  | Readonly<{ id: number; kind: "equipment-models"; generation: number; definitions: readonly number[]; resourceGeneration: number; profile: 0 | 1 }>
  | Readonly<{
      id: number
      kind: "configure-course"
      generation: number
      definition: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "discard"; generation: number }>
  | Readonly<{ id: number; kind: "set-position"; generation: number; position: readonly [number, number, number] }>
  | Readonly<{ id: number; kind: "entity-input"; generation: number; target: string; input: string; value: string; delay: number }>
  | Readonly<{ id: number; kind: "particles"; generation: number; batch: ArrayBuffer }>
  | Readonly<{
      id: number
      kind: "models"
      generation: number
      batch: ArrayBuffer
      visibility?: Readonly<{ id: number; queuedAt: number; views: readonly VisibilityView[]; acoustic?: ArrayBuffer }>
    }>
  | Readonly<{ id: number; kind: "visibility"; generation: number; views: readonly VisibilityView[]; acoustic?: ArrayBuffer }>
  | Readonly<{ id: number; kind: "acoustics"; generation: number; batch: ArrayBuffer }>
  | Readonly<{
      id: number
      kind: "observe"
      generation: number
      nowSeconds: number
      suspended: boolean
      snapshotTick: bigint
      command: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "shutdown" }>
)

export type WorkerFailureCode =
  | "MalformedRequest"
  | "WasmUnavailable"
  | "CompileFailed"
  | "TransitionFailed"
  | "StaleGeneration"
  | "GenerationMismatch"
  | "InternalFailure"

export type InitialView = Readonly<{
  entity: number
  hammerId: number | null
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>

export type WorkerTransactionTimings = Readonly<{
  wasmLinearMemoryBytes?: number
  wasmAllocatorLiveBytes?: number
  wasmAllocatorHighWaterBytes?: number
  mainCopyMilliseconds?: number
  queueMilliseconds?: number
  inputCopyMilliseconds: number
  transactMilliseconds: number
  outputCopyMilliseconds: number
  totalMilliseconds: number
}>

export type WorkerResponse =
  | Readonly<{ id: number; kind: "initialized"; applicationBuild: string; presentationSchema: number; wasmSha256: string; replies: import("./reply-transport").ReplyMemory }>
  | Readonly<{ id: number; kind: "resources"; bytes: ArrayBuffer | SharedArrayBuffer; byteOffset: number; byteLength: number }>
  | Readonly<{ id: number; kind: "resources-finalized"; generation: number; byteLength: number; sha256: string; sections: number }>
  | Readonly<{ id: number; kind: "resources-retained"; generation: number }>
  | Readonly<{ id: number; kind: "resources-released"; generation: number }>
  | Readonly<{
      id: number
      kind: "loaded"
      legacyParticleFrames: boolean
      generation: number
      payloadBytes: number
      payloadSha256: string
      payload?: ArrayBuffer
      presentationBytes: number
      presentation: ArrayBuffer
      initialView: InitialView
      timings: Readonly<{
        queueMilliseconds?: number
        inputCopyMilliseconds: number
        compileMilliseconds: number
        resultMilliseconds: number
        mapCopyMilliseconds: number
        presentationCopyMilliseconds: number
        presentationReleaseMilliseconds: number
        bspParseMilliseconds: number
        canonicalMapMilliseconds: number
        materialResolutionMilliseconds: number
        entityParseMilliseconds: number
        presentationCompileMilliseconds: number
        modelResolutionMilliseconds: number
        particleAndInputMilliseconds: number
        runtimeMapMilliseconds: number
        collisionSetupMilliseconds: number
        gameSetupMilliseconds: number
        presentationBundleMilliseconds: number
        presentationModelsMilliseconds: number
        presentationTexturesMilliseconds: number
        presentationParticlesMilliseconds: number
        presentationEnvironmentMilliseconds: number
        presentationSerializationMilliseconds: number
        textureDecoderRequests: number
        textureMetadataInspections: number
        modelCacheHits: number
        modelCacheMisses: number
        wasmLinearMemoryBytes: number
        wasmAllocatorLiveBytes: number
        wasmAllocatorHighWaterBytes: number
        wasmCompileOwnerBytes: readonly number[]
        resourceSections: number
        resourceBytes: number
        totalMilliseconds: number
      }>
    }>
  | Readonly<{ id: number; kind: "coverage"; generation: number; payload: ArrayBuffer }>
  | Readonly<{ id: number; kind: "activated"; generation: number }>
  | Readonly<{ id: number; kind: "team-selection"; generation: number; state: Tf2TeamSelectionServerState }>
  | Readonly<{ id: number; kind: "equipment"; generation: number; state: Tf2EquipmentState }>
  | Readonly<{ id: number; kind: "equipment-models"; generation: number; payload: ArrayBuffer }>
  | Readonly<{ id: number; kind: "course-configured"; generation: number }>
  | Readonly<{ id: number; kind: "discarded"; generation: number }>
  | Readonly<{ id: number; kind: "position-set"; generation: number }>
  | Readonly<{ id: number; kind: "entity-input-queued"; generation: number }>
  | Readonly<{ id: number; kind: "particles"; generation: number; output: ArrayBuffer; visualOutput?: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "models"; generation: number; output: SharedArrayBuffer; byteOffset: number; byteLength: number; lease: number; ownership: SharedArrayBuffer; slot: number; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "visibility"; generation: number; outputs: readonly ArrayBuffer[]; acoustic?: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "acoustics"; generation: number; output: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "simulation"; generation: number; output: ArrayBuffer; replayAttack?: Readonly<{ hostTick: bigint; playerClass: number; weapon: number; lifecycle: number }>; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "shutdown" }>
  | Readonly<{ id: number; kind: "failure"; code: WorkerFailureCode; detail: number; reason?: string }>
