export type WorkerRequest =
  | Readonly<{ id: number; kind: "initialize"; wasm: ArrayBuffer; wasmSha256: string }>
  | Readonly<{
      id: number
      kind: "load"
      generation: number
      profile: 0 | 1
      bsp: ArrayBuffer
      configuration: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "read-map"; generation: number }>
  | Readonly<{ id: number; kind: "activate"; generation: number }>
  | Readonly<{
      id: number
      kind: "configure-course"
      generation: number
      definition: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "discard"; generation: number }>
  | Readonly<{
      id: number
      kind: "advance"
      generation: number
      ticks: number
      command: ArrayBuffer
    }>
  | Readonly<{ id: number; kind: "shutdown" }>

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

export type WorkerResponse =
  | Readonly<{ id: number; kind: "initialized" }>
  | Readonly<{
      id: number
      kind: "loaded"
      generation: number
      payloadBytes: number
      payloadSha256: string
      initialView: InitialView
    }>
  | Readonly<{ id: number; kind: "map"; generation: number; payload: ArrayBuffer }>
  | Readonly<{ id: number; kind: "activated"; generation: number }>
  | Readonly<{ id: number; kind: "course-configured"; generation: number }>
  | Readonly<{ id: number; kind: "discarded"; generation: number }>
  | Readonly<{ id: number; kind: "snapshot"; generation: number; snapshot: ArrayBuffer }>
  | Readonly<{ id: number; kind: "shutdown" }>
  | Readonly<{ id: number; kind: "failure"; code: WorkerFailureCode; detail: number }>
