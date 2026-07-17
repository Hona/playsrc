import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { decodeSnapshot, type Snapshot } from "./codec"
import type { WorkerFailureCode, WorkerRequest, WorkerResponse } from "./protocol"

const HASH = /^[0-9a-f]{64}$/
const MAX_PENDING = 64
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 1024 * 1024
type RequestWithoutId = WorkerRequest extends infer Request
  ? Request extends { id: number }
    ? Omit<Request, "id">
    : never
  : never

export type WorkerLike = Readonly<{
  postMessage(message: WorkerRequest, transfer?: Transferable[]): void
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  removeEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  removeEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  terminate(): void
}>

export type LoadedGame = Readonly<{
  generation: number
  payload: Uint8Array
  payloadSha256: string
  cache: "hit" | "stored"
}>
export type StagedGame = LoadedGame

export class Tf2WorkerError extends Error {
  constructor(
    readonly code: WorkerFailureCode | "WorkerFailed" | "Closed" | "BoundExceeded" | "IntegrityFailure",
    readonly detail = 0,
  ) {
    super(code)
    this.name = "Tf2WorkerError"
  }
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}

export class Tf2WorkerClient {
  readonly #worker: WorkerLike
  readonly #cache: DerivedObjectCache
  readonly #pending = new Map<number, {
    resolve: (response: WorkerResponse) => void
    reject: (error: Error) => void
  }>()
  #nextId = 1
  #closed = false

  constructor(worker: WorkerLike, cache: DerivedObjectCache) {
    this.#worker = worker
    this.#cache = cache
    worker.addEventListener("message", this.#message)
    worker.addEventListener("error", this.#error)
  }

  readonly #message = (event: MessageEvent<WorkerResponse>): void => {
    const response = event.data
    const pending = response && this.#pending.get(response.id)
    if (!pending) return
    this.#pending.delete(response.id)
    if (response.kind === "failure") pending.reject(new Tf2WorkerError(response.code, response.detail))
    else pending.resolve(response)
  }

  readonly #error = (): void => {
    this.#closed = true
    this.#failAll(new Tf2WorkerError("WorkerFailed"))
    this.#worker.removeEventListener("message", this.#message)
    this.#worker.removeEventListener("error", this.#error)
    this.#worker.terminate()
  }

  #failAll(error: Error): void {
    for (const pending of this.#pending.values()) pending.reject(error)
    this.#pending.clear()
  }

  #request(request: RequestWithoutId, transfer: Transferable[] = []): Promise<WorkerResponse> {
    if (this.#closed) return Promise.reject(new Tf2WorkerError("Closed"))
    if (this.#pending.size >= MAX_PENDING) return Promise.reject(new Tf2WorkerError("BoundExceeded"))
    while (this.#pending.has(this.#nextId)) {
      this.#nextId = this.#nextId === 0xffff_ffff ? 1 : this.#nextId + 1
    }
    const id = this.#nextId
    this.#nextId = id === 0xffff_ffff ? 1 : id + 1
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
      try {
        this.#worker.postMessage({ ...request, id } as WorkerRequest, transfer)
      } catch {
        this.#pending.delete(id)
        reject(new Tf2WorkerError("WorkerFailed"))
      }
    })
  }

  async initialize(wasmBytes: Uint8Array, wasmSha256: string): Promise<void> {
    if (wasmBytes.byteLength < 1 || wasmBytes.byteLength > 64 * 1024 * 1024 || !HASH.test(wasmSha256)) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    if (await sha256(wasmBytes) !== wasmSha256) throw new Tf2WorkerError("IntegrityFailure")
    const transferred = wasmBytes.slice().buffer
    const response = await this.#request(
      { kind: "initialize", wasm: transferred, wasmSha256 },
      [transferred],
    )
    if (response.kind !== "initialized") throw new Tf2WorkerError("WorkerFailed")
  }

  async stage(
    generation: number,
    bsp: Uint8Array,
    profile: 0 | 1,
    configuration: Uint8Array,
    derivedKey: string,
  ): Promise<StagedGame> {
    if (
      !Number.isSafeInteger(generation)
      || generation < 1
      || generation > 0xffff_ffff
      || bsp.byteLength < 1
      || bsp.byteLength > MAX_BSP_BYTES
      || configuration.byteLength > MAX_CONFIGURATION_BYTES
      || !HASH.test(derivedKey)
    ) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const cached = await this.#cache.read(derivedKey)
    const bspBuffer = bsp.slice().buffer
    const configurationBuffer = configuration.slice().buffer
    const loaded = await this.#request(
      { kind: "load", generation, profile, bsp: bspBuffer, configuration: configurationBuffer },
      [bspBuffer, configurationBuffer],
    )
    try {
      if (
        loaded.kind !== "loaded"
        || loaded.generation !== generation
        || !Number.isSafeInteger(loaded.payloadBytes)
        || loaded.payloadBytes < 1
        || !HASH.test(loaded.payloadSha256)
      ) {
        throw new Tf2WorkerError("WorkerFailed")
      }
      let payload: Uint8Array
      let cache: LoadedGame["cache"]
      if (cached) {
        if (cached.byteLength !== loaded.payloadBytes || await sha256(cached) !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        payload = cached
        cache = "hit"
      } else {
        const map = await this.#request({ kind: "read-map", generation })
        if (map.kind !== "map" || map.generation !== generation || !(map.payload instanceof ArrayBuffer)) {
          throw new Tf2WorkerError("WorkerFailed")
        }
        payload = new Uint8Array(map.payload)
        if (payload.byteLength !== loaded.payloadBytes || await sha256(payload) !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        await this.#cache.write(derivedKey, loaded.payloadSha256, payload)
        cache = "stored"
      }
      return Object.freeze({
        generation,
        payload,
        payloadSha256: loaded.payloadSha256,
        cache,
      })
    } catch (error) {
      try {
        await this.#request({ kind: "discard", generation })
      } catch {
        // The worker can already have failed; the original classified failure remains authoritative.
      }
      throw error
    }
  }

  async activate(generation: number): Promise<void> {
    const activated = await this.#request({ kind: "activate", generation })
    if (activated.kind !== "activated" || activated.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async discard(generation: number): Promise<void> {
    const discarded = await this.#request({ kind: "discard", generation })
    if (discarded.kind !== "discarded" || discarded.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async load(
    generation: number,
    bsp: Uint8Array,
    profile: 0 | 1,
    configuration: Uint8Array,
    derivedKey: string,
  ): Promise<LoadedGame> {
    const staged = await this.stage(generation, bsp, profile, configuration, derivedKey)
    try {
      await this.activate(generation)
      return staged
    } catch (error) {
      try {
        await this.discard(generation)
      } catch {
        // Activation failure remains the authoritative result when the worker cannot discard.
      }
      throw error
    }
  }

  async advance(generation: number, command: ArrayBuffer, ticks: number): Promise<Snapshot> {
    if (command.byteLength !== 24) throw new Tf2WorkerError("BoundExceeded")
    const transferred = command.slice(0)
    const response = await this.#request(
      { kind: "advance", generation, ticks, command: transferred },
      [transferred],
    )
    if (response.kind !== "snapshot" || response.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return decodeSnapshot(response.snapshot)
  }

  async shutdown(): Promise<void> {
    if (this.#closed) return
    try {
      const response = await this.#request({ kind: "shutdown" })
      if (response.kind !== "shutdown") throw new Tf2WorkerError("WorkerFailed")
    } finally {
      this.#closed = true
      this.#worker.removeEventListener("message", this.#message)
      this.#worker.removeEventListener("error", this.#error)
      this.#worker.terminate()
      this.#failAll(new Tf2WorkerError("Closed"))
    }
  }
}
