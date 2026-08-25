import type { Tf2TeamChoice, Tf2TeamSelectionServerState } from "./team-selection/model"

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
  | Readonly<{ id: number; kind: "initialize"; wasm: ArrayBuffer; wasmSha256: string; threads: number }>
  | Readonly<{ id: number; kind: "decode-resources"; batch: ArrayBuffer }>
  | Readonly<{
      id: number
      kind: "load"
      generation: number
      profile: 0 | 1
      bsp: ArrayBuffer
      configuration: ArrayBuffer
      includeMap: boolean
      presentation?: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "read-coverage"; generation: number }>
  | Readonly<{ id: number; kind: "activate"; generation: number }>
  | Readonly<{ id: number; kind: "team-selection"; generation: number; choice: Tf2TeamChoice | null }>
  | Readonly<{
      id: number
      kind: "configure-course"
      generation: number
      definition: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "discard"; generation: number }>
  | Readonly<{ id: number; kind: "particles"; generation: number; batch: ArrayBuffer }>
  | Readonly<{
      id: number
      kind: "models"
      generation: number
      batch: ArrayBuffer
      visibility?: Readonly<{ id: number; queuedAt: number; view: VisibilityView }>
    }>
  | Readonly<{ id: number; kind: "visibility"; generation: number; view: VisibilityView }>
  | Readonly<{
      id: number
      kind: "observe"
      generation: number
      nowSeconds: number
      suspended: boolean
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
  | "InternalFailure"

export type InitialView = Readonly<{
  entity: number
  hammerId: number | null
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>

export type WorkerTransactionTimings = Readonly<{
  queueMilliseconds?: number
  inputCopyMilliseconds: number
  transactMilliseconds: number
  outputCopyMilliseconds: number
  totalMilliseconds: number
}>

export type WorkerResponse =
  | Readonly<{ id: number; kind: "initialized" }>
  | Readonly<{ id: number; kind: "resources"; bytes: ArrayBuffer }>
  | Readonly<{
      id: number
      kind: "loaded"
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
        totalMilliseconds: number
      }>
    }>
  | Readonly<{ id: number; kind: "coverage"; generation: number; payload: ArrayBuffer }>
  | Readonly<{ id: number; kind: "activated"; generation: number }>
  | Readonly<{ id: number; kind: "team-selection"; generation: number; state: Tf2TeamSelectionServerState }>
  | Readonly<{ id: number; kind: "course-configured"; generation: number }>
  | Readonly<{ id: number; kind: "discarded"; generation: number }>
  | Readonly<{ id: number; kind: "particles"; generation: number; output: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "models"; generation: number; output: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "visibility"; generation: number; output: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "simulation"; generation: number; output: ArrayBuffer; timings: WorkerTransactionTimings }>
  | Readonly<{ id: number; kind: "shutdown" }>
  | Readonly<{ id: number; kind: "failure"; code: WorkerFailureCode; detail: number; reason?: string }>
