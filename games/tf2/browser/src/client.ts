import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { decodeSnapshot, type Snapshot } from "./codec"
import type { InitialView, WorkerFailureCode, WorkerRequest, WorkerResponse } from "./protocol"

const HASH = /^[0-9a-f]{64}$/
const MAX_PENDING = 64
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 256 * 1024 * 1024
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
  presentation: Uint8Array
  presentationSha256: string
  presentationCache: "hit" | "stored" | "unavailable"
  initialView: InitialView
}>
export type StagedGame = LoadedGame

export class Tf2WorkerError extends Error {
  constructor(
    readonly code: WorkerFailureCode | "WorkerFailed" | "Closed" | "BoundExceeded" | "IntegrityFailure",
    readonly detail = 0,
  ) {
    super(detail === 0 ? code : `${code}:${detail}`)
    this.name = "Tf2WorkerError"
  }
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
async function presentationKey(key: string): Promise<string> {
  return sha256(new TextEncoder().encode(`playsrc-tf2-presentation-v6\0${key}`))
}

export class Tf2WorkerClient {
  readonly #worker: WorkerLike
  readonly #cache: DerivedObjectCache
  readonly #pending = new Map<
    number,
    {
      resolve: (response: WorkerResponse) => void
      reject: (error: Error) => void
    }
  >()
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
    if ((await sha256(wasmBytes)) !== wasmSha256) throw new Tf2WorkerError("IntegrityFailure")
    const transferred = wasmBytes.slice().buffer
    const response = await this.#request({ kind: "initialize", wasm: transferred, wasmSha256 }, [transferred])
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
      !Number.isSafeInteger(generation) ||
      generation < 1 ||
      generation > 0xffff_ffff ||
      bsp.byteLength < 1 ||
      bsp.byteLength > MAX_BSP_BYTES ||
      configuration.byteLength > MAX_CONFIGURATION_BYTES ||
      !HASH.test(derivedKey)
    ) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const cached = await this.#cache.read(derivedKey)
    const pkey = await presentationKey(derivedKey)
    const cachedPresentation = await this.#cache.read(pkey)
    const bspBuffer = bsp.slice().buffer
    const configurationBuffer = configuration.slice().buffer
    const loaded = await this.#request(
      { kind: "load", generation, profile, bsp: bspBuffer, configuration: configurationBuffer },
      [bspBuffer, configurationBuffer],
    )
    try {
      if (
        loaded.kind !== "loaded" ||
        loaded.generation !== generation ||
        !Number.isSafeInteger(loaded.payloadBytes) ||
        loaded.payloadBytes < 1 ||
        !HASH.test(loaded.payloadSha256) ||
        !Number.isSafeInteger(loaded.presentationBytes) ||
        loaded.presentationBytes < 1 ||
        loaded.presentationBytes > MAX_BSP_BYTES ||
        !HASH.test(loaded.presentationSha256) ||
        !Number.isSafeInteger(loaded.initialView?.entity) ||
        loaded.initialView.entity < 0 ||
        loaded.initialView.entity > 0xffff_ffff ||
        (loaded.initialView.hammerId !== null &&
          (!Number.isSafeInteger(loaded.initialView.hammerId) ||
            loaded.initialView.hammerId < 0 ||
            loaded.initialView.hammerId >= 0xffff_ffff)) ||
        loaded.initialView.position.length !== 3 ||
        loaded.initialView.angles.length !== 3 ||
        ![...loaded.initialView.position, ...loaded.initialView.angles].every(Number.isFinite)
      ) {
        throw new Tf2WorkerError("WorkerFailed")
      }
      let payload: Uint8Array
      let cache: LoadedGame["cache"]
      if (cached) {
        if (cached.byteLength !== loaded.payloadBytes || (await sha256(cached)) !== loaded.payloadSha256) {
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
        if (payload.byteLength !== loaded.payloadBytes || (await sha256(payload)) !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        await this.#cache.write(derivedKey, loaded.payloadSha256, payload)
        cache = "stored"
      }
      let presentation: Uint8Array
      let presentationCache: LoadedGame["presentationCache"]
      if (cachedPresentation) {
        if (
          cachedPresentation.byteLength !== loaded.presentationBytes ||
          (await sha256(cachedPresentation)) !== loaded.presentationSha256
        )
          throw new Tf2WorkerError("IntegrityFailure")
        presentation = cachedPresentation
        presentationCache = "hit"
      } else {
        const response = await this.#request({ kind: "read-presentation", generation })
        if (
          response.kind !== "presentation" ||
          response.generation !== generation ||
          !(response.payload instanceof ArrayBuffer)
        )
          throw new Tf2WorkerError("WorkerFailed")
        presentation = new Uint8Array(response.payload)
        if (
          presentation.byteLength !== loaded.presentationBytes ||
          (await sha256(presentation)) !== loaded.presentationSha256
        )
          throw new Tf2WorkerError("IntegrityFailure")
        try {
          await this.#cache.write(pkey, loaded.presentationSha256, presentation)
          presentationCache = "stored"
        } catch {
          presentationCache = "unavailable"
        }
      }
      return Object.freeze({
        generation,
        payload,
        payloadSha256: loaded.payloadSha256,
        cache,
        presentation,
        presentationSha256: loaded.presentationSha256,
        presentationCache,
        initialView: Object.freeze({
          entity: loaded.initialView.entity,
          hammerId: loaded.initialView.hammerId,
          position: Object.freeze([...loaded.initialView.position]) as readonly [number, number, number],
          angles: Object.freeze([...loaded.initialView.angles]) as readonly [number, number, number],
        }),
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

  async configureCourse(generation: number, definition: Uint8Array): Promise<void> {
    if (definition.byteLength < 52 || definition.byteLength > 64 * 1024) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const transferred = definition.slice().buffer
    const response = await this.#request({ kind: "configure-course", generation, definition: transferred }, [
      transferred,
    ])
    if (response.kind !== "course-configured" || response.generation !== generation) {
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
    if (command.byteLength < 48 || command.byteLength > 64 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = command.slice(0)
    const response = await this.#request({ kind: "advance", generation, ticks, command: transferred }, [transferred])
    if (response.kind !== "snapshot" || response.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return decodeSnapshot(response.snapshot)
  }
  async particles(generation: number, batch: Uint8Array): Promise<Uint8Array> {
    if (batch.byteLength < 32 || batch.byteLength > 4 * 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = batch.slice().buffer
    const response = await this.#request({ kind: "particles", generation, batch: transferred }, [transferred])
    if (
      response.kind !== "particles" ||
      response.generation !== generation ||
      !(response.output instanceof ArrayBuffer)
    )
      throw new Tf2WorkerError("WorkerFailed")
    return new Uint8Array(response.output)
  }
  async models(generation: number, batch: Uint8Array): Promise<Uint8Array> {
    if (batch.byteLength < 12 || batch.byteLength > 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = batch.slice().buffer
    const response = await this.#request({ kind: "models", generation, batch: transferred }, [transferred])
    if (response.kind !== "models" || response.generation !== generation || !(response.output instanceof ArrayBuffer)) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return new Uint8Array(response.output)
  }
  async visibility(
    generation: number,
    position: readonly [number, number, number],
  ): Promise<Readonly<{ worldIdentity: string; cacheIdentity: string; surfaces: Uint32Array }>> {
    const response = await this.#request({ kind: "visibility", generation, position })
    if (
      response.kind !== "visibility" ||
      response.generation !== generation ||
      !(response.output instanceof ArrayBuffer)
    )
      throw new Tf2WorkerError("WorkerFailed")
    const bytes = new Uint8Array(response.output),
      view = new DataView(response.output)
    if (new TextDecoder().decode(bytes.subarray(0, 4)) !== "PVIS" || view.getUint32(4, true) !== 1)
      throw new Tf2WorkerError("WorkerFailed")
    const count = view.getUint32(76, true)
    if (80 + count * 4 !== bytes.length) throw new Tf2WorkerError("WorkerFailed")
    const hex = (values: Uint8Array) => Array.from(values, (value) => value.toString(16).padStart(2, "0")).join("")
    return Object.freeze({
      cacheIdentity: hex(bytes.subarray(8, 40)),
      worldIdentity: hex(bytes.subarray(40, 72)),
      surfaces: new Uint32Array(Array.from({ length: count }, (_, i) => view.getUint32(80 + i * 4, true))),
    })
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
