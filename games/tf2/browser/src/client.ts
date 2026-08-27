import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import type { ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { decodeSnapshot, type Snapshot } from "./codec"
import { SnapshotRanges } from "./snapshot-retention"
import { decodeModelPoseOutput, type PosedModel } from "./presentation"
import { TF2_PRESENTATION_SCHEMA, type InitialView, type VisibilityView, type WorkerFailureCode, type WorkerRequest, type WorkerResponse } from "./protocol"
import type { Tf2TeamChoice, Tf2TeamSelectionServerState } from "./team-selection/model"
import type { Tf2EquipmentState } from "./equipment/types"
import { ReplyReader, type ReplyControl } from "./reply-transport"

const HASH = /^[0-9a-f]{64}$/
const MAX_PENDING = 64
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 1024 * 1024 * 1024
const MAX_CONFIGURATION_SECTION_BYTES = 32 * 1024 * 1024
const MAX_CONFIGURATION_SECTIONS = 1_024
const LITTLE_ENDIAN = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1
const HEX_BYTES = Array.from({ length: 256 }, (_, value) => value.toString(16).padStart(2, "0"))
type RequestWithoutId = WorkerRequest extends infer Request
  ? Request extends { id: number }
    ? Omit<Request, "id">
    : never
  : never

type QueuedModels = {
  id: number
  generation: number
  batch: ArrayBuffer
  visibility?: { id: number; queuedAt: number; views: readonly VisibilityView[] }
}

export type WorkerLike = Readonly<{
  postMessage(message: WorkerRequest, transfer?: Transferable[]): void
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse | ReplyControl>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  removeEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse | ReplyControl>) => void): void
  removeEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  terminate(): void
  __playsrcProfileReply?(response: WorkerResponse): void
}>

export type ResourceConfiguration = Readonly<{
  generation: number
  byteLength: number
  sha256: string
  sections: readonly Uint8Array[]
  sectionIdentities?: readonly string[]
}>

export type LoadedGame = Readonly<{
  legacyParticleFrames: boolean
  generation: number
  payload: Uint8Array
  payloadSha256: string
  cache: "hit" | "stored"
  presentation: Uint8Array
  /** Existing derived presentation identity: map/configuration/compiler + application build. */
  presentationKey: string
  presentationCache: "hit" | "stored" | "unavailable"
  presentationCacheError: string | null
  persistence: Promise<Readonly<{
    mapCacheWriteMilliseconds: number
    presentationCacheWriteMilliseconds: number
    presentationCache: "hit" | "stored" | "unavailable"
    presentationCacheError: string | null
  }>>
  timings: Readonly<{
    mapCacheReadMilliseconds: number
    presentationKeyMilliseconds: number
    presentationCacheReadMilliseconds: number
    inputCloneMilliseconds: number
    workerLoadMilliseconds: number
    workerInputCopyMilliseconds: number
    workerCompileMilliseconds: number
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
    mapIntegrityMilliseconds: number
    mapReadMilliseconds: number
    mapCacheWriteMilliseconds: number
    presentationIntegrityMilliseconds: number
    presentationReadMilliseconds: number
    presentationCacheWriteMilliseconds: number
    presentationReleaseMilliseconds: number
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
  initialView: InitialView
}>
export type StagedGame = LoadedGame
export type SimulationEventBatch = Readonly<{ hostTick: bigint; byteLength: number; snapshot: Snapshot }>
export type SimulationPublication = Readonly<{ hostFrame: bigint; firstHostTick: bigint; lastHostTick: bigint; selectedTicks: number; interpolation: number; snapshotByteLength: number; eventBatches: readonly SimulationEventBatch[]; snapshot: Snapshot }>
export type WaterViewPass = Readonly<{ kind: "reflection" | "refraction" | "main" | "intersection"; origin: readonly [number,number,number]; angles: readonly [number,number,number]; renderAboveWater:boolean;renderUnderWater:boolean;renderWaterSurface:boolean;drawEntities:boolean;drawSky2d:boolean;clip:null|Readonly<{height:number;keep:"above"|"below"}>;forcedVisibilityLeaf:number|null;fog:Readonly<{kind:"world"}|{kind:"water";volume:number;heightFog:boolean}>;surfaces:Uint32Array }>
export type WaterViewPlan = Readonly<{ visibleWater:null|Readonly<{volume:number;visibleLeaf:number;eyeLeaf:number;eyeInVolume:boolean;surfaceZ:number;distanceToWater:number|null;material:string;translucent:boolean;evaluated:null|Readonly<{normalFrame:number;normalTransform:Float32Array;cheapStart:number;cheapEnd:number}>;overlay:null|Readonly<{identity:string;normalFrame:number;normalTransform:Float32Array}>}>;render:Readonly<{cheap:boolean;reflect:boolean;refract:boolean;reflectEntities:boolean;drawSurface:boolean;opaque:boolean}>;nearPlaneIntersects:boolean;passes:readonly WaterViewPass[] }>
export type EvaluatedWorldTexture = Readonly<{ role: number; frame: number | null; transform: Float32Array | null }>
export type EvaluatedWorldMaterial = Readonly<{ identity: string; mapMaterial: number; textures: readonly EvaluatedWorldTexture[] }>
export type VisibilityResult = Readonly<{ worldIdentity:string;cacheIdentity:string;outsideWorld:boolean;sky:0|1|2;eyeLeaf:number|null;leaves:readonly number[];areas:readonly number[];surfaces:Uint32Array;drawSurfaces:Uint32Array;water:WaterViewPlan;worldMaterials:readonly EvaluatedWorldMaterial[];screenOverlay:import("@playsrc/rendering").ScreenOverlayFrame|null }>
export type CoverageSample=Readonly<{leaf:number;cluster:number;area:number;position:readonly[number,number,number]}>

export class Tf2WorkerError extends Error {
  constructor(
    readonly code: WorkerFailureCode | "WorkerFailed" | "Closed" | "BoundExceeded" | "IntegrityFailure",
    readonly detail: number|string = 0,
  ) {
    super(detail === 0 ? code : `${code}:${detail}`)
    this.name = "Tf2WorkerError"
  }
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
async function presentationKey(key: string, applicationBuild: string): Promise<string> {
  return sha256(new TextEncoder().encode(`playsrc-tf2-presentation-v${TF2_PRESENTATION_SCHEMA}\0${applicationBuild}\0${key}`))
}

function transferredBytes(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer instanceof ArrayBuffer && bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength
    ? bytes.buffer
    : bytes.slice().buffer
}

function queuedAt(): number {
  return performance.timeOrigin + performance.now()
}

export class Tf2WorkerClient {
  readonly #worker: WorkerLike
  readonly #cache: DerivedObjectCache
  readonly #applicationBuild: string
  readonly #pending = new Map<
    number,
    {
      resolve: (response: WorkerResponse) => void
      reject: (error: Error) => void
    }
  >()
  #nextId = 1
  #closed = false
  #shutdownRequested?: Promise<void>
  #staleMessages = 0
  #queuedModels?: QueuedModels
  readonly #modelReads = new Set<Promise<readonly PosedModel[]>>()
  readonly #snapshotStreams = new Map<number, SimulationSnapshotStream>()
  #initialization?: Readonly<{ wasmSha256: string; threads: number; ready: Promise<void> }>
  #replies?: ReplyReader

  get staleMessages(): number { return this.#staleMessages }

  abort(): void {
    if (this.#closed) return
    this.#closed = true
    this.#replies?.close()
    this.#replies = undefined
    this.#worker.removeEventListener("message", this.#message)
    this.#worker.removeEventListener("error", this.#error)
    this.#worker.terminate()
    this.#failAll(new DOMException("Worker generation was aborted", "AbortError"))
  }

  constructor(worker: WorkerLike, cache: DerivedObjectCache, applicationBuild: string) {
    if (!HASH.test(applicationBuild)) throw new Tf2WorkerError("IntegrityFailure")
    this.#worker = worker
    this.#cache = cache
    this.#applicationBuild = applicationBuild
    worker.addEventListener("message", this.#message)
    worker.addEventListener("error", this.#error)
  }

  readonly #message = (event: MessageEvent<WorkerResponse | ReplyControl>): void => {
    const response = event.data
    try {
      if (response?.kind === "reply-control") {
        if (!this.#replies) throw new Error("Reply mailbox not initialized")
        this.#replies.accept(response)
      } else {
        if (this.#replies) throw new Error("Unordered reply outside mailbox")
        this.#receive(response)
      }
    } catch { this.#error() }
  }

  readonly #receive = (response: WorkerResponse): void => {
    this.#worker.__playsrcProfileReply?.(response)
    const pending = response && this.#pending.get(response.id)
    if (!pending) {
      this.#staleMessages += 1
      return
    }
    if (response.kind === "initialized" && response.applicationBuild === this.#applicationBuild
      && response.presentationSchema === TF2_PRESENTATION_SCHEMA && response.wasmSha256 === this.#initialization?.wasmSha256) {
      this.#replies = new ReplyReader(response.replies, this.#receive)
      void this.#replies.run().catch(() => this.#error())
    }
    this.#pending.delete(response.id)
    if(response.kind==="failure")pending.reject(new Tf2WorkerError(response.code,response.reason?`${response.detail}:${response.reason}`:response.detail))
    else pending.resolve(response)
  }

  readonly #error = (): void => {
    this.#closed = true
    this.#replies?.close()
    this.#replies = undefined
    this.#failAll(new Tf2WorkerError("WorkerFailed"))
    this.#worker.removeEventListener("message", this.#message)
    this.#worker.removeEventListener("error", this.#error)
    this.#worker.terminate()
  }

  #failAll(error: Error): void {
    this.#queuedModels = undefined
    for (const stream of this.#snapshotStreams.values()) stream.close()
    this.#snapshotStreams.clear()
    for (const pending of this.#pending.values()) pending.reject(error)
    this.#pending.clear()
  }

  #reserve(shutdown = false): { id: number; response: Promise<WorkerResponse> } {
    if (this.#closed || this.#shutdownRequested && !shutdown) throw new Tf2WorkerError("Closed")
    if (this.#pending.size >= MAX_PENDING) throw new Tf2WorkerError("BoundExceeded")
    while (this.#pending.has(this.#nextId)) {
      this.#nextId = this.#nextId === 0xffff_ffff ? 1 : this.#nextId + 1
    }
    const id = this.#nextId
    this.#nextId = id === 0xffff_ffff ? 1 : id + 1
    const response = new Promise<WorkerResponse>((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
    })
    return { id, response }
  }

  #send(request: RequestWithoutId, id: number, transfer: Transferable[] = []): void {
    try {
      this.#worker.postMessage({ ...request, id, queuedAt: queuedAt() } as WorkerRequest, transfer)
    } catch {
      const pending = this.#pending.get(id)
      this.#pending.delete(id)
      pending?.reject(new Tf2WorkerError("WorkerFailed"))
      if (request.kind === "models" && request.visibility) {
        const companion = this.#pending.get(request.visibility.id)
        this.#pending.delete(request.visibility.id)
        companion?.reject(new Tf2WorkerError("WorkerFailed"))
      }
    }
  }

  #flushModels(): void {
    const queued = this.#queuedModels
    if (!queued) return
    this.#queuedModels = undefined
    this.#send({
      kind: "models",
      generation: queued.generation,
      batch: queued.batch,
      ...(queued.visibility ? { visibility: queued.visibility } : {}),
    }, queued.id, [queued.batch])
  }

  #request(request: RequestWithoutId, transfer: Transferable[] = []): Promise<WorkerResponse> {
    if (this.#queuedModels) this.#flushModels()
    try {
      const pending = this.#reserve(request.kind === "shutdown")
      this.#send(request, pending.id, transfer)
      return pending.response
    } catch (error) {
      return Promise.reject(error)
    }
  }

  async initialize(wasmBytes: Uint8Array, wasmSha256: string, threads = navigator.hardwareConcurrency): Promise<void> {
    if (wasmBytes.byteLength < 1 || wasmBytes.byteLength > 64 * 1024 * 1024 || !HASH.test(wasmSha256)
      || !Number.isSafeInteger(threads) || threads < 1 || threads > 64) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    if ((await sha256(wasmBytes)) !== wasmSha256) throw new Tf2WorkerError("IntegrityFailure")
    if (this.#closed || this.#shutdownRequested) throw new Tf2WorkerError("Closed")
    if (this.#initialization) {
      if (this.#initialization.wasmSha256 !== wasmSha256 || this.#initialization.threads !== threads) throw new Tf2WorkerError("TransitionFailed")
      return this.#initialization.ready
    }
    const ready = this.#initialize(wasmBytes, wasmSha256, threads)
    this.#initialization = { wasmSha256, threads, ready }
    return ready
  }

  async #initialize(wasmBytes: Uint8Array, wasmSha256: string, threads: number): Promise<void> {
    const transferred = transferredBytes(wasmBytes)
    let response: WorkerResponse
    try {
      response = await this.#request({
        kind: "initialize", applicationBuild: this.#applicationBuild,
        presentationSchema: TF2_PRESENTATION_SCHEMA, wasm: transferred, wasmSha256, threads,
      }, [transferred])
    } catch (error) {
      this.abort()
      throw error
    }
    if (response.kind !== "initialized") {
      this.abort()
      throw new Tf2WorkerError("WorkerFailed")
    }
    if (response.applicationBuild !== this.#applicationBuild
      || response.presentationSchema !== TF2_PRESENTATION_SCHEMA || response.wasmSha256 !== wasmSha256) {
      this.#error()
      throw new Tf2WorkerError("GenerationMismatch")
    }
  }

  async decodeResources(records: readonly Readonly<{ descriptor: ResourceChunkDescriptor; bytes: Uint8Array }>[], generation?: number): Promise<Uint8Array> {
    if (records.length < 1 || records.length > 1_024
      || (generation !== undefined && (!Number.isSafeInteger(generation) || generation < 1 || generation > 0xffff_ffff))) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const encoder = new TextEncoder()
    let total = 12
    const chunks = records.map(({ descriptor, bytes }) => {
      const encodedDescriptor = encoder.encode(JSON.stringify(descriptor))
      if (encodedDescriptor.byteLength < 1 || encodedDescriptor.byteLength > 8 * 1024 * 1024
        || bytes.byteLength < 1 || bytes.byteLength > 32 * 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
      total += 8 + encodedDescriptor.byteLength + bytes.byteLength
      if (total > 512 * 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
      return Object.freeze({ descriptor: transferredBytes(encodedDescriptor), bytes: transferredBytes(bytes) })
    })
    const shared = generation !== undefined
    const response = await this.#request({ kind: "decode-resources", chunks, shared, ...(shared ? { generation } : {}) },
      chunks.flatMap(({ descriptor, bytes }) => [descriptor, bytes]))
    if (response.kind !== "resources" || !(response.bytes instanceof ArrayBuffer || response.bytes instanceof SharedArrayBuffer)
      || (response.bytes instanceof SharedArrayBuffer) !== shared
      || !Number.isSafeInteger(response.byteOffset) || response.byteOffset < 0
      || !Number.isSafeInteger(response.byteLength) || response.byteLength < 12 || response.byteLength > MAX_CONFIGURATION_SECTION_BYTES
      || response.byteOffset + response.byteLength > response.bytes.byteLength) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return new Uint8Array(response.bytes, response.byteOffset, response.byteLength)
  }

  async finalizeResources(
    generation: number,
    sections: readonly Uint8Array[],
    authenticatedIdentity?: Readonly<{ byteLength: number; sha256: string }>,
  ): Promise<ResourceConfiguration> {
    if (!Number.isSafeInteger(generation) || generation < 1 || generation > 0xffff_ffff
      || sections.length < 1 || sections.length > MAX_CONFIGURATION_SECTIONS
      || sections.some((section) => section.byteLength < 12 || section.byteLength > MAX_CONFIGURATION_SECTION_BYTES)
      || (authenticatedIdentity && (!Number.isSafeInteger(authenticatedIdentity.byteLength)
        || authenticatedIdentity.byteLength !== 12 + sections.reduce((total, section) => total + section.byteLength - 12, 0)
        || !HASH.test(authenticatedIdentity.sha256)))) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const response = await this.#request({ kind: "finalize-resources", generation, ...(authenticatedIdentity ? { authenticatedIdentity } : {}) })
    if (response.kind !== "resources-finalized" || response.generation !== generation
      || !Number.isSafeInteger(response.byteLength) || response.byteLength < 12 || response.byteLength > MAX_CONFIGURATION_BYTES
       || !HASH.test(response.sha256) || response.sections !== sections.length
       || (authenticatedIdentity && (response.byteLength !== authenticatedIdentity.byteLength || response.sha256 !== authenticatedIdentity.sha256))) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return Object.freeze({ generation, byteLength: response.byteLength, sha256: response.sha256, sections: Object.freeze([...sections]) })
  }

  async releaseResources(generation: number): Promise<void> {
    const response = await this.#request({ kind: "release-resources", generation })
    if (response.kind !== "resources-released" || response.generation !== generation) throw new Tf2WorkerError("WorkerFailed")
  }

  async retainResourceSection(generation: number, source: ResourceConfiguration, sectionIndex: number): Promise<Uint8Array> {
    const section = source.sections[sectionIndex]
    if (!Number.isSafeInteger(generation) || generation < 1 || generation > 0xffff_ffff
      || !Number.isSafeInteger(source.generation) || source.generation < 1 || source.generation >= generation
      || !Number.isSafeInteger(sectionIndex) || sectionIndex < 0 || !section
      || section.byteLength < 12 || section.byteLength > MAX_CONFIGURATION_SECTION_BYTES) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const response = section.buffer instanceof SharedArrayBuffer
      ? await this.#request({ kind: "retain-resources", generation, sourceGeneration: source.generation, sectionIndex })
      : await (async () => {
        const transferred = section.slice().buffer
        return this.#request({ kind: "retain-resources", generation, section: transferred }, [transferred])
      })()
    if (response.kind !== "resources-retained" || response.generation !== generation) throw new Tf2WorkerError("WorkerFailed")
    return section
  }

  async #retainResources(generation: number, configuration: ResourceConfiguration): Promise<ResourceConfiguration> {
    try {
      for (let index = 0; index < configuration.sections.length; index += 1) await this.retainResourceSection(generation, configuration, index)
      const retained = await this.finalizeResources(generation, configuration.sections)
      if (retained.byteLength !== configuration.byteLength || retained.sha256 !== configuration.sha256) throw new Tf2WorkerError("IntegrityFailure")
      return retained
    } catch (error) {
      await this.releaseResources(generation).catch(() => {})
      throw error
    }
  }

  async stage(
    generation: number,
    bsp: Uint8Array,
    profile: 0 | 1,
    configuration: ResourceConfiguration,
    derivedKey: string,
  ): Promise<StagedGame> {
    const started = performance.now()
    if (
      !Number.isSafeInteger(generation) ||
      generation < 1 ||
      generation > 0xffff_ffff ||
      bsp.byteLength < 1 ||
      bsp.byteLength > MAX_BSP_BYTES ||
      !Number.isSafeInteger(configuration?.byteLength) || configuration.byteLength < 12 || configuration.byteLength > MAX_CONFIGURATION_BYTES ||
      !HASH.test(configuration.sha256) || !Array.isArray(configuration.sections)
      || configuration.sections.length < 1 || configuration.sections.length > MAX_CONFIGURATION_SECTIONS
      || configuration.sections.some((section) => !(section instanceof Uint8Array) || section.byteLength < 12 || section.byteLength > MAX_CONFIGURATION_SECTION_BYTES) ||
      !HASH.test(derivedKey)
    ) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    let mapCacheReadMilliseconds = 0
    let presentationKeyMilliseconds = 0
    let presentationCacheReadMilliseconds = 0
    let pkey = ""
    const mapRead = (async () => {
      const phase = performance.now()
      try {
        return await this.#cache.read(derivedKey)
      } finally {
        mapCacheReadMilliseconds = performance.now() - phase
      }
    })()
    const presentationRead = (async () => {
      let phase = performance.now()
      pkey = await presentationKey(derivedKey, this.#applicationBuild)
      presentationKeyMilliseconds = performance.now() - phase
      phase = performance.now()
      try {
        return await this.#cache.read(pkey)
      } finally {
        presentationCacheReadMilliseconds = performance.now() - phase
      }
    })()
    const [mapResult, presentationResult] = await Promise.allSettled([mapRead, presentationRead])
    if (mapResult.status === "rejected") throw mapResult.reason
    if (presentationResult.status === "rejected") throw presentationResult.reason
    const cachedRecord = mapResult.value
    const cachedPresentationRecord = presentationResult.value
    const cached = cachedRecord?.bytes
    const cachedPresentation = cachedPresentationRecord?.bytes
    const cachedPresentationBytes = cachedPresentation?.byteLength
    if (configuration.generation !== generation) configuration = await this.#retainResources(generation, configuration)
    let phase = performance.now()
    const bspBuffer = transferredBytes(bsp)
    const presentationBuffer = cachedPresentation ? transferredBytes(cachedPresentation) : undefined
    const inputCloneMilliseconds = performance.now() - phase
    phase = performance.now()
    const loaded = await this.#request({
      kind: "load",
      generation,
      profile,
      bsp: bspBuffer,
      configurationSha256: configuration.sha256,
      configurationBytes: configuration.byteLength,
      includeMap: !cached,
      ...(presentationBuffer ? { presentation: presentationBuffer } : {}),
    }, [bspBuffer, ...(presentationBuffer ? [presentationBuffer] : [])])
    const workerLoadMilliseconds = performance.now() - phase
    try {
      if (
        loaded.kind !== "loaded" ||
        loaded.generation !== generation ||
        !Number.isSafeInteger(loaded.payloadBytes) ||
        loaded.payloadBytes < 1 ||
        loaded.payloadBytes > MAX_BSP_BYTES ||
        !HASH.test(loaded.payloadSha256) ||
        !Number.isSafeInteger(loaded.presentationBytes) ||
        loaded.presentationBytes < 1 ||
        loaded.presentationBytes > MAX_BSP_BYTES ||
        !(loaded.presentation instanceof ArrayBuffer) ||
        loaded.presentation.byteLength !== loaded.presentationBytes ||
        (cached ? loaded.payload !== undefined : !(loaded.payload instanceof ArrayBuffer)) ||
        typeof loaded.legacyParticleFrames !== "boolean" ||
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
      let mapIntegrityMilliseconds = 0
      let mapPersistence = Promise.resolve(0)
      if (cached) {
        phase = performance.now()
        if (cached.byteLength !== loaded.payloadBytes || cachedRecord!.sha256 !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        mapIntegrityMilliseconds = performance.now() - phase
        payload = cached
        cache = "hit"
      } else {
        payload = new Uint8Array(loaded.payload!)
        if (payload.byteLength !== loaded.payloadBytes) throw new Tf2WorkerError("IntegrityFailure")
        const writeStarted = performance.now()
        mapPersistence = this.#cache.write(derivedKey, loaded.payloadSha256, payload)
          .then(() => performance.now() - writeStarted)
        cache = "stored"
      }
      const presentation = new Uint8Array(loaded.presentation)
      let presentationCache: LoadedGame["presentationCache"]
      let presentationPersistence: Promise<{
        milliseconds: number
        cache: "hit" | "stored" | "unavailable"
        error: string | null
      }> = Promise.resolve({
        milliseconds: 0,
        cache: "hit",
        error: null,
      })
      if (cachedPresentationBytes !== undefined) {
        if (cachedPresentationBytes !== loaded.presentationBytes) throw new Tf2WorkerError("IntegrityFailure")
        presentationCache = "hit"
      } else {
        const writeStarted = performance.now()
        presentationPersistence = this.#cache.write(pkey, null, presentation).then(
          () => ({ milliseconds: performance.now() - writeStarted, cache: "stored" as const, error: null }),
          (error) => ({
            milliseconds: performance.now() - writeStarted,
            cache: "unavailable" as const,
            error: error instanceof Error ? `${error.name}: ${error.message}` : String(error),
          }),
        )
        presentationCache = "stored"
      }
      const persistence = Promise.all([mapPersistence, presentationPersistence])
        .then(([mapMilliseconds, retained]) => Object.freeze({
          mapCacheWriteMilliseconds: mapMilliseconds,
          presentationCacheWriteMilliseconds: retained.milliseconds,
          presentationCache: retained.cache,
          presentationCacheError: retained.error,
        }))
      return Object.freeze({
        generation,
        payload,
        payloadSha256: loaded.payloadSha256,
        cache,
        presentation,
        presentationKey: pkey,
        presentationCache,
        presentationCacheError: null,
        persistence,
        timings: Object.freeze({
          mapCacheReadMilliseconds,
          presentationKeyMilliseconds,
          presentationCacheReadMilliseconds,
          inputCloneMilliseconds,
          workerLoadMilliseconds,
          workerInputCopyMilliseconds: loaded.timings.inputCopyMilliseconds,
          workerCompileMilliseconds: loaded.timings.compileMilliseconds,
          bspParseMilliseconds: loaded.timings.bspParseMilliseconds,
          canonicalMapMilliseconds: loaded.timings.canonicalMapMilliseconds,
          materialResolutionMilliseconds: loaded.timings.materialResolutionMilliseconds,
          entityParseMilliseconds: loaded.timings.entityParseMilliseconds,
          presentationCompileMilliseconds: loaded.timings.presentationCompileMilliseconds,
          modelResolutionMilliseconds: loaded.timings.modelResolutionMilliseconds,
          particleAndInputMilliseconds: loaded.timings.particleAndInputMilliseconds,
          runtimeMapMilliseconds: loaded.timings.runtimeMapMilliseconds,
          collisionSetupMilliseconds: loaded.timings.collisionSetupMilliseconds,
          gameSetupMilliseconds: loaded.timings.gameSetupMilliseconds,
          mapIntegrityMilliseconds,
          mapReadMilliseconds: loaded.timings.mapCopyMilliseconds,
          mapCacheWriteMilliseconds: 0,
          presentationIntegrityMilliseconds: 0,
          presentationReadMilliseconds: loaded.timings.presentationCopyMilliseconds,
          presentationCacheWriteMilliseconds: 0,
          presentationReleaseMilliseconds: loaded.timings.presentationReleaseMilliseconds,
          textureDecoderRequests: loaded.timings.textureDecoderRequests,
          textureMetadataInspections: loaded.timings.textureMetadataInspections,
          modelCacheHits: loaded.timings.modelCacheHits,
          modelCacheMisses: loaded.timings.modelCacheMisses,
          wasmLinearMemoryBytes: loaded.timings.wasmLinearMemoryBytes,
          wasmAllocatorLiveBytes: loaded.timings.wasmAllocatorLiveBytes,
          wasmAllocatorHighWaterBytes: loaded.timings.wasmAllocatorHighWaterBytes,
          wasmCompileOwnerBytes: Object.freeze([...loaded.timings.wasmCompileOwnerBytes]),
          resourceSections: loaded.timings.resourceSections,
          resourceBytes: loaded.timings.resourceBytes,
          totalMilliseconds: performance.now() - started,
        }),
        initialView: Object.freeze({
          entity: loaded.initialView.entity,
          hammerId: loaded.initialView.hammerId,
          position: Object.freeze([...loaded.initialView.position]) as readonly [number, number, number],
          angles: Object.freeze([...loaded.initialView.angles]) as readonly [number, number, number],
        }),
        legacyParticleFrames: loaded.legacyParticleFrames,
      })
    } catch (error) {
      try {
        await this.#request({ kind: "discard", generation })
      } catch {
        // The original classified failure remains authoritative if the Worker has already failed.
      }
      throw error
    }
  }

  async teamSelection(generation: number, choice: Tf2TeamChoice | null = null): Promise<Tf2TeamSelectionServerState> {
    if (choice !== null && !["red", "blue", "spectate", "auto"].includes(choice)) throw new Tf2WorkerError("BoundExceeded")
    const response = await this.#request({ kind: "team-selection", generation, choice })
    if (response.kind !== "team-selection" || response.generation !== generation) throw new Tf2WorkerError("WorkerFailed")
    return response.state
  }

  async equipment(generation: number, mutation?: ArrayBuffer): Promise<Tf2EquipmentState> {
    const response = await this.#request({ kind: "equipment", generation, mutation })
    if (response.kind !== "equipment" || response.generation !== generation) throw new Tf2WorkerError("WorkerFailed")
    return response.state
  }

  async admitEquipmentModels(generation: number, definitions: readonly number[], configuration: ResourceConfiguration, profile: 0 | 1 = 1): Promise<Uint8Array> {
    if (!Number.isSafeInteger(generation) || generation < 0 || !Array.isArray(definitions) || definitions.length < 1 || definitions.length > 32
      || definitions.some((value) => !Number.isSafeInteger(value) || value < 0 || value >= 0xffff_ffff) || ![0, 1].includes(profile)) throw new Tf2WorkerError("BoundExceeded")
    const response = await this.#request({ kind: "equipment-models", generation, definitions, resourceGeneration: configuration.generation, profile })
    if (response.kind !== "equipment-models" || response.generation !== generation || !(response.payload instanceof ArrayBuffer)) throw new Tf2WorkerError("WorkerFailed")
    return new Uint8Array(response.payload)
  }

  async activate(generation: number): Promise<void> {
    const activated = await this.#request({ kind: "activate", generation })
    if (activated.kind !== "activated" || activated.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    for (const [prior, stream] of this.#snapshotStreams) if (prior !== generation) {
      stream.close(); this.#snapshotStreams.delete(prior)
    }
  }
  async coverage(generation:number):Promise<readonly CoverageSample[]>{const response=await this.#request({kind:"read-coverage",generation});if(response.kind!=="coverage"||response.generation!==generation||!(response.payload instanceof ArrayBuffer))throw new Tf2WorkerError("WorkerFailed");const bytes=new Uint8Array(response.payload),view=new DataView(response.payload);if(bytes.length<12||new TextDecoder().decode(bytes.subarray(0,4))!=="PCOV"||view.getUint32(4,true)!==1||12+view.getUint32(8,true)*24!==bytes.length)throw new Tf2WorkerError("WorkerFailed");return Object.freeze(Array.from({length:view.getUint32(8,true)},(_,index)=>{const at=12+index*24,position=Object.freeze([view.getFloat32(at+8,true),view.getFloat32(at+12,true),view.getFloat32(at+16,true)]) as readonly[number,number,number];if(!position.every(Number.isFinite)||view.getUint32(at+20,true)!==0)throw new Tf2WorkerError("WorkerFailed");return Object.freeze({leaf:view.getUint32(at,true),cluster:view.getInt16(at+4,true),area:view.getUint16(at+6,true),position})}))}

  async discard(generation: number): Promise<void> {
    const discarded = await this.#request({ kind: "discard", generation })
    if (discarded.kind !== "discarded" || discarded.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    this.#snapshotStreams.get(generation)?.close()
    this.#snapshotStreams.delete(generation)
  }

  async setPosition(generation: number, position: readonly [number, number, number]): Promise<void> {
    if (position.length !== 3 || !position.every(Number.isFinite)) throw new Tf2WorkerError("BoundExceeded")
    const response = await this.#request({ kind: "set-position", generation, position })
    if (response.kind !== "position-set" || response.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async fireEntityInput(generation: number, target: string, input: string, value = "", delay = 0): Promise<void> {
    const response = await this.#request({ kind: "entity-input", generation, target, input, value, delay })
    if (response.kind !== "entity-input-queued" || response.generation !== generation) throw new Tf2WorkerError("WorkerFailed")
  }

  async configureCourse(generation: number, definition: Uint8Array): Promise<void> {
    if (definition.byteLength < 52 || definition.byteLength > 64 * 1024) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const transferred = transferredBytes(definition)
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
    configuration: ResourceConfiguration,
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

  async observe(generation: number, nowSeconds: number, command: ArrayBuffer, suspended = false): Promise<readonly SimulationPublication[]> {
    if (!Number.isSafeInteger(generation) || generation < 1 || generation > 0xffff_ffff) throw new Tf2WorkerError("BoundExceeded")
    if (command.byteLength < 84 || command.byteLength > 64 * 1024) throw new Tf2WorkerError("BoundExceeded")
    if (!Number.isFinite(nowSeconds) || nowSeconds < 0) throw new Tf2WorkerError("BoundExceeded")
    let stream = this.#snapshotStreams.get(generation)
    if (!stream) { stream = new SimulationSnapshotStream(); this.#snapshotStreams.set(generation, stream) }
    try {
      const response = await this.#request({ kind: "observe", generation, nowSeconds, suspended, command, snapshotTick: stream.tick }, [command])
      if (response.kind !== "simulation" || response.generation !== generation || !(response.output instanceof ArrayBuffer)) {
        throw new Tf2WorkerError("WorkerFailed")
      }
      const profile = (globalThis as typeof globalThis & {
        __playsrcFrameProfiler?: { active: boolean; simulation: unknown[]; simulationDropped: number }
      }).__playsrcFrameProfiler
      const started = profile?.active ? performance.now() : 0
      const publications = stream.decode(response.output)
      if (profile?.active) {
        if (profile.simulation.length >= 16_384) profile.simulationDropped += 1
        else profile.simulation.push({
          requestId: response.id, at: started, nowSeconds, suspended, decodeMilliseconds: performance.now() - started, bytes: response.output.byteLength,
          replayAttack: response.replayAttack ? { ...response.replayAttack, hostTick: String(response.replayAttack.hostTick) } : null,
          publications: publications.map(publication => ({
            hostFrame: String(publication.hostFrame), firstHostTick: String(publication.firstHostTick), lastHostTick: String(publication.lastHostTick),
            selectedTicks: publication.selectedTicks, interpolation: publication.interpolation, eventBatches: publication.eventBatches.length,
            player: { tick: String(publication.snapshot.tick), playerClass: publication.snapshot.class, weapon: publication.snapshot.weapon, lifecycle: publication.snapshot.lifecycle },
            weapons: publication.snapshot.loadout.map(weapon => ({ weapon: weapon.weapon, firstPrimaryTick: String(weapon.firstPrimaryTick), nextPrimaryTick: String(weapon.nextPrimaryTick) })),
            activities: publication.snapshot.activities.map(activity => ({ ...activity, tick: String(activity.tick) })),
          })),
        })
      }
      return publications
    } catch (error) {
      if (stream.tick === 0n && this.#snapshotStreams.get(generation) === stream) {
        stream.close(); this.#snapshotStreams.delete(generation)
      }
      throw error
    }
  }

  snapshotMetrics(generation: number): Readonly<SimulationSnapshotStream["metrics"]> | undefined {
    return this.#snapshotStreams.get(generation)?.metrics
  }
  async particles(generation: number, batch: Uint8Array): Promise<Uint8Array> {
    const response = await this.#readParticleReply(generation, batch)
    if (response.visualOutput) throw new Tf2WorkerError("WorkerFailed", "Unexpected client-frame visual output in a PCF transaction")
    return new Uint8Array(response.output)
  }
  async legacyFrame(generation: number, batch: Uint8Array): Promise<Readonly<{ particles: Uint8Array; visuals: Uint8Array }>> {
    const response = await this.#readParticleReply(generation, batch)
    return { particles: new Uint8Array(response.output), visuals: response.visualOutput ? new Uint8Array(response.visualOutput) : new Uint8Array(0) }
  }
  async #readParticleReply(generation: number, batch: Uint8Array): Promise<Extract<WorkerResponse, { kind: "particles" }>> {
    if (batch.byteLength < 32 || batch.byteLength > 4 * 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = transferredBytes(batch)
    const response = await this.#request({ kind: "particles", generation, batch: transferred }, [transferred])
    if (
      response.kind !== "particles" ||
      response.generation !== generation ||
      !(response.output instanceof ArrayBuffer) ||
      (response.visualOutput !== undefined && !(response.visualOutput instanceof ArrayBuffer))
    )
      throw new Tf2WorkerError("WorkerFailed")
    return response
  }
  models(generation: number, batch: Uint8Array): Promise<readonly PosedModel[]> {
    const read = this.#readModels(generation, batch)
    this.#modelReads.add(read)
    void read.then(() => this.#modelReads.delete(read), () => this.#modelReads.delete(read))
    return read
  }

  async #readModels(generation: number, batch: Uint8Array): Promise<readonly PosedModel[]> {
    if (batch.byteLength < 12 || batch.byteLength > 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    if (this.#queuedModels) this.#flushModels()
    const pending = this.#reserve()
    const queued: QueuedModels = { id: pending.id, generation, batch: transferredBytes(batch) }
    this.#queuedModels = queued
    queueMicrotask(() => {
      if (this.#queuedModels === queued) this.#flushModels()
    })
    const response = await pending.response
    if (response.kind !== "models" || response.generation !== generation || !(response.output instanceof SharedArrayBuffer) ||
      !Number.isSafeInteger(response.byteOffset) || !Number.isSafeInteger(response.byteLength) ||
      response.byteOffset < 0 || response.byteOffset % 4 !== 0 || response.byteLength < 12 || response.byteLength > 64 * 1024 * 1024 ||
      response.byteOffset > response.output.byteLength - response.byteLength || response.lease !== response.id ||
      !(response.ownership instanceof SharedArrayBuffer) || response.ownership.byteLength !== 64 * Int32Array.BYTES_PER_ELEMENT ||
      !Number.isSafeInteger(response.slot) || response.slot < 0 || response.slot >= 64) {
      this.#error()
      throw new Tf2WorkerError("WorkerFailed")
    }
    const ownership = new Int32Array(response.ownership)
    if ((Atomics.load(ownership, response.slot) >>> 0) !== response.lease) {
      this.#error()
      throw new Tf2WorkerError("WorkerFailed")
    }
    try {
      // The decoder returns owned compact palettes/attachments, never a lease-backed view.
      // No caller, asynchronous callback, renderer or GPU may outlive this read ownership.
      return decodeModelPoseOutput(new Uint8Array(response.output, response.byteOffset, response.byteLength))
    } finally {
      Atomics.store(ownership, response.slot, 0)
    }
  }

  async visibility(generation: number, input: VisibilityView): Promise<VisibilityResult> {
    return (await this.visibilityViews(generation, [input]))[0]!
  }

  async visibilityViews(generation: number, inputs: readonly VisibilityView[]): Promise<readonly VisibilityResult[]> {
    if (inputs.length < 1 || inputs.length > 2) throw new Tf2WorkerError("BoundExceeded")
    const unique: VisibilityView[] = []
    const indexes = inputs.map(input => {
      const match = unique.findIndex(view => view.position.every((value, index) => Object.is(value, input.position[index]))
        && (view.visibilityPosition ?? view.position).every((value, index) => Object.is(value, (input.visibilityPosition ?? input.position)[index]))
        && Object.is(view.areaFilter, input.areaFilter)
        && Object.is(view.yawDegrees, input.yawDegrees) && Object.is(view.pitchDegrees, input.pitchDegrees)
        && Object.is(view.verticalFovDegrees, input.verticalFovDegrees) && Object.is(view.aspectRatio, input.aspectRatio)
        && Object.is(view.near, input.near) && Object.is(view.far, input.far)
        && Object.is(view.presentationTimeSeconds, input.presentationTimeSeconds))
      if (match !== -1) return match
      unique.push(input)
      return unique.length - 1
    })
    let requested: Promise<WorkerResponse>
    const queued = this.#queuedModels
    if (queued && queued.generation === generation && !queued.visibility) {
      const companion = this.#reserve()
      queued.visibility = { id: companion.id, queuedAt: queuedAt(), views: unique }
      requested = companion.response
      this.#flushModels()
    } else {
      requested = this.#request({ kind: "visibility", generation, views: unique })
    }
    const response = await requested
    if (
      response.kind !== "visibility" ||
      response.generation !== generation ||
      !Array.isArray(response.outputs) || response.outputs.length !== unique.length ||
      response.outputs.some(output => !(output instanceof ArrayBuffer))
    )
      throw new Tf2WorkerError("WorkerFailed")
    const results = response.outputs.map(output => this.#decodeVisibility(output))
    return Object.freeze(indexes.map(index => results[index]!))
  }

  #decodeVisibility(output: ArrayBuffer): VisibilityResult {
    const bytes = new Uint8Array(output), view = new DataView(output), decoder=new TextDecoder("utf-8",{fatal:true})
    if (decoder.decode(bytes.subarray(0, 4)) !== "PVIS" || view.getUint32(4, true) !== 8)
      throw new Tf2WorkerError("WorkerFailed")
    let at=76
    const require=(length:number)=>{if(at+length>bytes.length)throw new Tf2WorkerError("WorkerFailed")},u8=()=>{require(1);return bytes[at++]!},u32=()=>{require(4);const value=view.getUint32(at,true);at+=4;return value},i32=()=>{require(4);const value=view.getInt32(at,true);at+=4;return value},f32=()=>{require(4);const value=view.getFloat32(at,true);at+=4;if(!Number.isFinite(value))throw new Tf2WorkerError("WorkerFailed");return value},text=()=>{const length=u32();require(length);const value=decoder.decode(bytes.subarray(at,at+length));at+=length;return value},vector=()=>Object.freeze([f32(),f32(),f32()]) as readonly[number,number,number]
    const indices = (count: number): Uint32Array => {
      require(count * Uint32Array.BYTES_PER_ELEMENT)
      if (LITTLE_ENDIAN && at % Uint32Array.BYTES_PER_ELEMENT === 0) {
        const values = new Uint32Array(output, at, count)
        at += count * Uint32Array.BYTES_PER_ELEMENT
        return values
      }
      const values = new Uint32Array(count)
      for (let index = 0; index < count; index += 1) values[index] = u32()
      return values
    }
    const surfaces = indices(u32())
    const drawSurfaces = indices(u32())
    const eyeLeafValue=u32(),leaves=Object.freeze(Array.from({length:u32()},u32)),areas=Object.freeze(Array.from({length:u32()},u32))
    const present=u8(),cheap=u8(),reflect=u8(),refract=u8(),reflectEntities=u8(),drawSurface=u8(),opaque=u8(),nearPlaneIntersects=u8()
    if([present,cheap,reflect,refract,reflectEntities,drawSurface,opaque,nearPlaneIntersects].some(value=>value>1))throw new Tf2WorkerError("WorkerFailed")
    let visibleWater:WaterViewPlan["visibleWater"]=null
    if(present===1){const volume=u32(),visibleLeaf=u32(),eyeLeaf=u32(),eyeInVolume=u8(),translucent=u8(),hasOverlay=u8(),waterShader=u8();if(eyeInVolume>1||translucent>1||hasOverlay>1||waterShader>1||(!eyeInVolume&&hasOverlay)||(!waterShader&&(hasOverlay||!cheap||reflect||refract)))throw new Tf2WorkerError("WorkerFailed");const surfaceZ=f32(),distance=u32(),material=text();let evaluated:NonNullable<WaterViewPlan["visibleWater"]>["evaluated"]=null;if(waterShader){const normalFrame=i32(),normalTransform=new Float32Array(16);for(let index=0;index<16;index++)normalTransform[index]=f32();evaluated=Object.freeze({normalFrame,normalTransform,cheapStart:f32(),cheapEnd:f32()})}let overlay:NonNullable<WaterViewPlan["visibleWater"]>["overlay"]=null;if(hasOverlay){const identity=text(),frame=i32(),transform=new Float32Array(16);if(!identity||frame<0)throw new Tf2WorkerError("WorkerFailed");for(let index=0;index<16;index++)transform[index]=f32();overlay=Object.freeze({identity,normalFrame:frame,normalTransform:transform})}visibleWater=Object.freeze({volume,visibleLeaf,eyeLeaf,eyeInVolume:eyeInVolume===1,surfaceZ,distanceToWater:distance===0xffff?null:distance,material,translucent:translucent===1,evaluated,overlay})}
    const passes:WaterViewPass[]=[]
    for(let passCount=u32();passCount>0;passCount--){const kind=u8(),renderAboveWater=u8(),renderUnderWater=u8(),renderWaterSurface=u8(),drawEntities=u8(),drawSky2d=u8(),hasClip=u8(),keep=u8();if(kind>3||[renderAboveWater,renderUnderWater,renderWaterSurface,drawEntities,drawSky2d,hasClip].some(value=>value>1)||keep>2||(hasClip===0)!==(keep===0))throw new Tf2WorkerError("WorkerFailed");const origin=vector(),angles=vector(),clipHeight=f32(),forced=u32(),fogKind=u8(),heightFog=u8();if(fogKind>1||heightFog>1||u8()||u8())throw new Tf2WorkerError("WorkerFailed");const fogVolume=u32();if(fogKind===0&&(heightFog!==0||fogVolume!==0))throw new Tf2WorkerError("WorkerFailed");const passSurfaces=indices(u32());passes.push(Object.freeze({kind:(["reflection","refraction","main","intersection"] as const)[kind]!,origin,angles,renderAboveWater:renderAboveWater===1,renderUnderWater:renderUnderWater===1,renderWaterSurface:renderWaterSurface===1,drawEntities:drawEntities===1,drawSky2d:drawSky2d===1,clip:hasClip===1?Object.freeze({height:clipHeight,keep:keep===1?"above" as const:"below" as const}):null,forcedVisibilityLeaf:forced===0xffff_ffff?null:forced,fog:fogKind===0?Object.freeze({kind:"world" as const}):Object.freeze({kind:"water" as const,volume:fogVolume,heightFog:heightFog===1}),surfaces:passSurfaces}))}
    const worldMaterials: EvaluatedWorldMaterial[] = []
    const worldMaterialCount = u32()
    if (worldMaterialCount > 4096) throw new Tf2WorkerError("WorkerFailed")
    for (let index = 0; index < worldMaterialCount; index++) {
      const identity = text(), mapMaterial = u32(), textureCount = u32()
      if (!identity || textureCount > 18 || worldMaterials.some((material) => material.identity === identity)) {
        throw new Tf2WorkerError("WorkerFailed")
      }
      const textures: EvaluatedWorldTexture[] = []
      for (let texture = 0; texture < textureCount; texture++) {
        const role = u8(), hasFrame = u8(), hasTransform = u8()
        if (role > 17 || hasFrame > 1 || hasTransform > 1 || u8()) throw new Tf2WorkerError("WorkerFailed")
        const selectedFrame = i32(), matrix = new Float32Array(16)
        for (let component = 0; component < matrix.length; component++) matrix[component] = f32()
        if ((!hasFrame && selectedFrame !== 0) || (!hasTransform && matrix.some((value) => value !== 0))) {
          throw new Tf2WorkerError("WorkerFailed")
        }
        textures.push(Object.freeze({ role, frame: hasFrame ? selectedFrame : null, transform: hasTransform ? matrix : null }))
      }
      worldMaterials.push(Object.freeze({ identity, mapMaterial, textures: Object.freeze(textures) }))
    }
    const hasScreenOverlay = u8()
    if (hasScreenOverlay > 1 || u8() || u8() || u8()) throw new Tf2WorkerError("WorkerFailed")
    let screenOverlay: VisibilityResult["screenOverlay"] = null
    if (hasScreenOverlay) {
      const identity = text(), normalFrame = i32(), normalTransform = new Float32Array(16)
      if (!identity || normalFrame < 0) throw new Tf2WorkerError("WorkerFailed")
      for (let index = 0; index < normalTransform.length; index++) normalTransform[index] = f32()
      screenOverlay = Object.freeze({ identity, normalFrame, normalTransform, refractTint: vector() })
    }
    if(at!==bytes.length||(present===0&&visibleWater!==null))throw new Tf2WorkerError("WorkerFailed")
    const hex = (values: Uint8Array): string => {
      let output = ""
      for (let index = 0; index < values.byteLength; index += 1) output += HEX_BYTES[values[index]!]!
      return output
    }
    return Object.freeze({
      cacheIdentity: hex(bytes.subarray(8, 40)),
      worldIdentity: hex(bytes.subarray(40, 72)),
      outsideWorld:bytes[72]===1,
      sky:bytes[73] as 0|1|2,
      eyeLeaf:eyeLeafValue===0xffff_ffff?null:eyeLeafValue,
      leaves,areas,surfaces,drawSurfaces,
      water:Object.freeze({visibleWater,render:Object.freeze({cheap:cheap===1,reflect:reflect===1,refract:refract===1,reflectEntities:reflectEntities===1,drawSurface:drawSurface===1,opaque:opaque===1}),nearPlaneIntersects:nearPlaneIntersects===1,passes:Object.freeze(passes)}),
      worldMaterials: Object.freeze(worldMaterials),
      screenOverlay,
    })
  }

  shutdown(): Promise<void> {
    if (this.#shutdownRequested) return this.#shutdownRequested
    if (this.#closed) return Promise.resolve()
    this.#shutdownRequested = this.#finishShutdown()
    return this.#shutdownRequested
  }

  async #finishShutdown(): Promise<void> {
    try {
      // Submit already reserved reads, then finish their synchronous decoders
      // before shutdown can overtake their shared-memory ownership release.
      if (this.#queuedModels) this.#flushModels()
      if (this.#modelReads.size > 0) await Promise.allSettled(this.#modelReads)
      const response = await this.#request({ kind: "shutdown" })
      if (response.kind !== "shutdown") throw new Tf2WorkerError("WorkerFailed")
    } finally {
      this.#closed = true
      this.#replies?.close()
      this.#replies = undefined
      this.#worker.removeEventListener("message", this.#message)
      this.#worker.removeEventListener("error", this.#error)
      this.#worker.terminate()
      this.#failAll(new Tf2WorkerError("Closed"))
    }
  }
}

export function mergePublicationSnapshots(snapshots: readonly Snapshot[]): Snapshot {
  const final = snapshots.at(-1)
  if (!final) throw new Tf2WorkerError("WorkerFailed")
  let previous: bigint | undefined
  let entries = 0
  for (const snapshot of snapshots) {
    for (const entry of snapshot.projectileTimeline) {
      if (previous !== undefined && entry.tick <= previous) throw new Tf2WorkerError("WorkerFailed")
      previous = entry.tick
      entries += 1
    }
  }
  if (entries === 0 || previous !== final.tick) throw new Tf2WorkerError("WorkerFailed")
  if (snapshots.length === 1) return final
  const all = (key: keyof Snapshot) => Object.freeze(snapshots.flatMap((snapshot) => snapshot[key] as readonly unknown[]))
  return Object.freeze({
    ...final,
    objectives: final.objectives === null ? null : Object.freeze({
      ...final.objectives,
      events: Object.freeze(snapshots.flatMap((snapshot) => snapshot.objectives?.events ?? [])),
    }),
    projectileEvents: all("projectileEvents"),
    projectileTimeline: all("projectileTimeline"),
    entityEvents: all("entityEvents"),
    events: all("events"),
    combatDecals: all("combatDecals"),
    activities: all("activities"),
    lifecycleEvents: all("lifecycleEvents"),
    physicsRequests: all("physicsRequests"),
    rocketTraceRequests: all("rocketTraceRequests"),
    radiusDamageRequests: all("radiusDamageRequests"),
    moverRequests: all("moverRequests"),
    contactReconcileRequests: all("contactReconcileRequests"),
    mapEffects: all("mapEffects"),
    regenerateAnimationEvents: all("regenerateAnimationEvents"),
    randomDraws: all("randomDraws"),
    audioEvents: all("audioEvents"),
    rocketTraceResults: all("rocketTraceResults"),
    moverResults: all("moverResults"),
  }) as Snapshot
}

export class SimulationSnapshotStream {
  #tick = 0n
  #frame = 0n
  #gameTick: bigint | undefined
  #bytes: Uint8Array | undefined
  #ranges: SnapshotRanges | undefined
  #closed = false
  readonly metrics = { responses: 0, wireBytes: 0, canonicalWireBytes: 0, restoredBytes: 0, fullSnapshots: 0, deltaSnapshots: 0,
    decodedRanges: 0, reusedRanges: 0, reusedBytes: 0, decodeMilliseconds: 0, retainedBaselineBytes: 0 }

  get tick(): bigint { return this.#tick }
  close(): void { this.#closed = true; this.#bytes = undefined; this.#ranges = undefined; this.metrics.retainedBaselineBytes = 0 }

  decode(buffer: ArrayBuffer): readonly SimulationPublication[] {
  if (this.#closed) throw new Tf2WorkerError("Closed")
  const started = performance.now()
  // Commit only after the entire response, including every event tick, validates.
  let tick = this.#tick, frame = this.#frame, baseline = this.#bytes, ranges = this.#ranges
  let gameTick = this.#gameTick
  let restoredBytes = 0, fullSnapshots = 0, deltaSnapshots = 0, decodedRanges = 0, reusedRanges = 0, reusedBytes = 0
  const bytes = new Uint8Array(buffer)
  const view = new DataView(buffer)
  if (
    bytes.byteLength < 16 || bytes.byteLength > 512 * 1024 * 1024 ||
    bytes[0] !== 0x50 || bytes[1] !== 0x53 || bytes[2] !== 0x49 || bytes[3] !== 0x4d ||
    view.getUint32(4, true) !== 3 || view.getUint32(12, true) !== 0
  ) throw new Tf2WorkerError("WorkerFailed")
  const count = view.getUint32(8, true)
  if (count > 256) throw new Tf2WorkerError("BoundExceeded")
  let offset = 16
  const publications: SimulationPublication[] = []
  const require = (length: number): void => {
    if (length > bytes.byteLength - offset) throw new Tf2WorkerError("WorkerFailed")
  }
  for (let index = 0; index < count; index += 1) {
    require(40)
    const hostFrame = view.getBigUint64(offset, true)
    const firstHostTick = view.getBigUint64(offset + 8, true)
    const lastHostTick = view.getBigUint64(offset + 16, true)
    const selectedTicks = view.getUint32(offset + 24, true)
    const interpolation = view.getFloat32(offset + 28, true)
    const snapshotLength = view.getUint32(offset + 32, true)
    const eventCount = view.getUint32(offset + 36, true)
    offset += 40
    if (
      selectedTicks < 1 || eventCount !== selectedTicks || eventCount > 1792 ||
      !Number.isFinite(interpolation) || interpolation < 0 || interpolation > 1 || hostFrame <= frame ||
      firstHostTick !== tick + 1n ||
      lastHostTick - firstHostTick + 1n !== BigInt(selectedTicks)
    ) throw new Tf2WorkerError("WorkerFailed")
    const eventBatches: SimulationEventBatch[] = []
    for (let event = 0; event < eventCount; event += 1) {
      require(24)
      const hostTick = view.getBigUint64(offset, true)
      const length = view.getUint32(offset + 8, true)
      const wireLength = view.getUint32(offset + 12, true)
      const baseTick = view.getBigUint64(offset + 16, true)
      offset += 24
      require(wireLength)
      restoredBytes += length
      if (hostTick !== tick + 1n) throw new Tf2WorkerError("WorkerFailed")
      if (length < 184 || length > 64 * 1024 * 1024 || restoredBytes > 448 * 1024 * 1024)
        throw new Tf2WorkerError("BoundExceeded")
      let eventBytes: Uint8Array
      if (baseTick === 0n) {
        if (wireLength !== length) throw new Tf2WorkerError("WorkerFailed")
        // A full restore owns just this snapshot, not an arbitrarily large response.
        eventBytes = bytes.slice(offset, offset + wireLength)
        fullSnapshots++
      } else {
        if (baseTick !== tick || !baseline || baseline.byteLength !== length || wireLength >= length)
          throw new Tf2WorkerError("WorkerFailed")
        eventBytes = baseline.slice()
        let at = offset, end = 0
        while (at < offset + wireLength) {
          if (offset + wireLength - at < 8) throw new Tf2WorkerError("WorkerFailed")
          const start = view.getUint32(at, true), size = view.getUint32(at + 4, true)
          at += 8
          if (start < end || size === 0 || start + size > length || size > offset + wireLength - at)
            throw new Tf2WorkerError("WorkerFailed")
          eventBytes.set(bytes.subarray(at, at + size), start)
          end = start + size
          at += size
        }
        deltaSnapshots++
      }
      offset += wireLength
      const nextRanges = new SnapshotRanges(eventBytes, ranges)
      const snapshot = decodeSnapshot(eventBytes, nextRanges)
      if (gameTick !== undefined && snapshot.tick !== gameTick + 1n) throw new Tf2WorkerError("WorkerFailed")
      gameTick = snapshot.tick
      nextRanges.finish()
      decodedRanges += nextRanges.decoded; reusedRanges += nextRanges.reused; reusedBytes += nextRanges.reusedBytes
      ranges = nextRanges
      baseline = eventBytes
      tick = hostTick
      eventBatches.push(Object.freeze({
        hostTick,
        byteLength: length,
        snapshot,
      }))
    }
    if (baseline?.byteLength !== snapshotLength) throw new Tf2WorkerError("WorkerFailed")
    frame = hostFrame
    publications.push(Object.freeze({
      hostFrame,
      firstHostTick,
      lastHostTick,
      selectedTicks,
      interpolation,
      snapshotByteLength: snapshotLength,
      eventBatches: Object.freeze(eventBatches),
      snapshot: mergePublicationSnapshots(eventBatches.map((event) => event.snapshot)),
    }))
  }
  if (offset !== bytes.byteLength) throw new Tf2WorkerError("WorkerFailed")
  this.#tick = tick; this.#frame = frame; this.#gameTick = gameTick; this.#bytes = baseline; this.#ranges = ranges
  this.metrics.retainedBaselineBytes = baseline?.byteLength ?? 0
  this.metrics.responses++; this.metrics.wireBytes += bytes.byteLength; this.metrics.restoredBytes += restoredBytes
  this.metrics.canonicalWireBytes += 16 + count * 40 + (fullSnapshots + deltaSnapshots) * 12 + restoredBytes
  this.metrics.fullSnapshots += fullSnapshots; this.metrics.deltaSnapshots += deltaSnapshots
  this.metrics.decodedRanges += decodedRanges; this.metrics.reusedRanges += reusedRanges; this.metrics.reusedBytes += reusedBytes
  this.metrics.decodeMilliseconds += performance.now() - started
  return Object.freeze(publications)
  }
}
