import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache, VerifiedDerivedObject } from "@playsrc/asset-store/browser"
import type { ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { Tf2WorkerClient, Tf2WorkerError, type ResourceConfiguration, type WorkerLike } from "../src/client"
import type { VisibilityView, WorkerRequest, WorkerResponse, WorkerTransactionTimings } from "../src/protocol"

const MAP = Uint8Array.from([0x50, 0x53, 0x4d, 0x50, 9, 8, 7, 6])
const PRESENTATION = Uint8Array.from([0x50, 0x54, 0x46, 0x32, 1, 2, 3, 4])
const KEY = "ab".repeat(32)
const BUILD = "cd".repeat(32)
const RESOURCE_CHUNK: ResourceChunkDescriptor = Object.freeze({
  codec: "identity",
  encodedByteLength: "12",
  encodedSha256: "12".repeat(32),
  decodedByteLength: "12",
  decodedSha256: "34".repeat(32),
  roles: Object.freeze(["gameplay"]),
  entries: Object.freeze([{ logicalPath: "materials/test.vmt", offset: "0", byteLength: "12", sha256: "56".repeat(32) }]),
})
const TIMINGS: WorkerTransactionTimings = Object.freeze({
  queueMilliseconds: 0,
  inputCopyMilliseconds: 0,
  transactMilliseconds: 0,
  outputCopyMilliseconds: 0,
  totalMilliseconds: 0,
})

const LOAD_TIMINGS = Object.freeze({
  queueMilliseconds: 0,
  inputCopyMilliseconds: 0,
  compileMilliseconds: 0,
  resultMilliseconds: 0,
  mapCopyMilliseconds: 0,
  presentationCopyMilliseconds: 0,
  presentationReleaseMilliseconds: 0,
  bspParseMilliseconds: 0,
  canonicalMapMilliseconds: 0,
  materialResolutionMilliseconds: 0,
  entityParseMilliseconds: 0,
  presentationCompileMilliseconds: 0,
  modelResolutionMilliseconds: 0,
  particleAndInputMilliseconds: 0,
  runtimeMapMilliseconds: 0,
  collisionSetupMilliseconds: 0,
  gameSetupMilliseconds: 0,
  presentationBundleMilliseconds: 0,
  presentationModelsMilliseconds: 0,
  presentationTexturesMilliseconds: 0,
  presentationParticlesMilliseconds: 0,
  presentationEnvironmentMilliseconds: 0,
  presentationSerializationMilliseconds: 0,
  textureDecoderRequests: 0,
  textureMetadataInspections: 0,
  modelCacheHits: 0,
  modelCacheMisses: 0,
  wasmLinearMemoryBytes: 0,
  wasmAllocatorLiveBytes: 0,
  wasmAllocatorHighWaterBytes: 0,
  wasmCompileOwnerBytes: [],
  resourceSections: 1,
  resourceBytes: 12,
  totalMilliseconds: 0,
})

async function digest(bytes: Uint8Array): Promise<string> {
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes.buffer instanceof SharedArrayBuffer ? bytes.slice() : bytes)), (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("")
}

async function presentationIdentity(key: string, build = BUILD): Promise<string> {
  return digest(new TextEncoder().encode(`playsrc-tf2-presentation-v16\0${build}\0${key}`))
}

async function configuration(generation: number, values: readonly number[] = []): Promise<ResourceConfiguration> {
  const section = new Uint8Array(Math.max(12, values.length))
  section.set(values)
  return Object.freeze({ generation, byteLength: section.byteLength, sha256: await digest(section), sections: Object.freeze([section]) })
}

function visibilityOutput(animated = false): ArrayBuffer {
  const words = [2, 4, 9, 1, 9, 3, 1, 3, 1, 7]
  const identity = new TextEncoder().encode("materials/water/test.vmt")
  const bytes = new Uint8Array(76 + words.length * 4 + 8 + 8 + (animated ? 12 + identity.length + 72 : 0))
  const view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x56, 0x49, 0x53])
  view.setUint32(4, 6, true)
  bytes.fill(0x11, 8, 40)
  bytes.fill(0x22, 40, 72)
  let offset = 76
  for (const value of words) {
    view.setUint32(offset, value, true)
    offset += 4
  }
  offset += 8
  view.setUint32(offset, 0, true)
  offset += 4
  view.setUint32(offset, animated ? 1 : 0, true)
  offset += 4
  if (animated) {
    view.setUint32(offset, identity.length, true)
    offset += 4
    bytes.set(identity, offset)
    offset += identity.length
    view.setUint32(offset, 84, true)
    view.setUint32(offset + 4, 1, true)
    offset += 8
    bytes.set([7, 1, 1, 0], offset)
    view.setInt32(offset + 4, 15, true)
    offset += 8
    for (let index = 0; index < 16; index++) view.setFloat32(offset + index * 4, index % 5 === 0 ? 1 : 0, true)
  }
  return bytes.buffer
}

const VIEW: VisibilityView = Object.freeze({
  position: Object.freeze([1, 2, 3]),
  yawDegrees: 90,
  pitchDegrees: 0,
  verticalFovDegrees: 60,
  aspectRatio: 16 / 9,
  near: 7,
  far: 32_768,
  presentationTimeSeconds: 1,
})

class MemoryCache implements DerivedObjectCache {
  readonly entries = new Map<string, Uint8Array>()
  readonly writes: string[] = []
  reads = 0
  maximumConcurrentReads = 0
  #activeReads = 0
  #pendingRead?: Readonly<{ ready: Promise<void>; release(): void }>

  async read(key: string): Promise<VerifiedDerivedObject | undefined> {
    this.reads += 1
    this.#activeReads += 1
    this.maximumConcurrentReads = Math.max(this.maximumConcurrentReads, this.#activeReads)
    if (this.#pendingRead) {
      const pending = this.#pendingRead
      this.#pendingRead = undefined
      pending.release()
    } else {
      let release!: () => void
      const ready = new Promise<void>((resolve) => { release = resolve })
      this.#pendingRead = { ready, release }
      await ready
    }
    this.#activeReads -= 1
    const value = this.entries.get(key)?.slice()
    return value ? Object.freeze({ bytes: value, sha256: await digest(value) }) : undefined
  }

  async write(key: string, expected: string | null, bytes: Uint8Array): Promise<string> {
    const hash = await digest(bytes)
    if (expected !== null && hash !== expected) throw new Error("invalid digest")
    this.entries.set(key, bytes.slice())
    this.writes.push(key)
    return hash
  }

  async remove(key: string): Promise<void> {
    this.entries.delete(key)
  }

  close(): void {}
}

function modelPoseOutput(bones = [0x3f800000, 0x80000000, 0, 0, 0, 0x3f800000, 0, 0, 0, 0, 0x3f800000, 0]): Uint8Array {
  const values: number[] = [0x50, 0x4d, 0x50, 0x4f]
  const u32 = (value: number) => {
    const bytes = new Uint8Array(4)
    new DataView(bytes.buffer).setUint32(0, value, true)
    values.push(...bytes)
  }
  const text = (value: string) => {
    const bytes = new TextEncoder().encode(value)
    u32(bytes.length); values.push(...bytes)
  }
  u32(8); u32(1); u32(9); u32(7); u32(0)
  values.push(0, 0, 0, 0)
  text("model"); text("ACT_IDLE")
  u32(0)
  for (let index = 0; index < 4; index++) u32(0x3f800000)
  values.push(...new Array(4 + 8 + 68 + 8).fill(0))
  for (let index = 0; index < 4; index++) u32(0)
  u32(1)
  for (const bits of bones) u32(bits)
  u32(0); u32(0); u32(0); u32(0)
  return Uint8Array.from(values)
}

class PipelineWorker implements WorkerLike {
  readonly requests: WorkerRequest[] = []
  readonly mapHash: string
  failure?: WorkerResponse
  animatedWorldMaterial = false
  workerBuild = BUILD
  presentationSchema = 16
  workerWasmSha256?: string
  malformedModelOutput = false
  modelBits?: number[]
  readonly modelLeases = new Map<number, SharedArrayBuffer>()
  closing?: number
  terminated = false
  readonly resources = new Map<number, Uint8Array[]>()
  #message?: (event: MessageEvent<WorkerResponse>) => void
  #error?: (event: ErrorEvent) => void

  constructor(mapHash: string) {
    this.mapHash = mapHash
  }

  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  addEventListener(type: "message" | "error", listener: unknown): void {
    if (type === "message") this.#message = listener as (event: MessageEvent<WorkerResponse>) => void
    else this.#error = listener as (event: ErrorEvent) => void
  }

  removeEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  removeEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  removeEventListener(type: "message" | "error"): void {
    if (type === "message") this.#message = undefined
    else this.#error = undefined
  }

  terminate(): void {
    this.terminated = true
  }

  postMessage(message: WorkerRequest, transfer: Transferable[] = []): void {
    const request = structuredClone(message, { transfer })
    this.requests.push(request)
    if (this.failure) {
      this.#respond({ ...this.failure, id: request.id } as WorkerResponse)
      return
    }
    switch (request.kind) {
      case "initialize":
        this.#respond({ id: request.id, kind: "initialized", applicationBuild: this.workerBuild, presentationSchema: this.presentationSchema, wasmSha256: this.workerWasmSha256 ?? request.wasmSha256 })
        return
      case "load": {
        const payload = request.includeMap ? MAP.slice().buffer : undefined
        const presentation = request.presentation ?? PRESENTATION.slice().buffer
        this.#respond({
          id: request.id,
          kind: "loaded",
          generation: request.generation,
          payloadBytes: MAP.byteLength,
          payloadSha256: this.mapHash,
          ...(payload ? { payload } : {}),
          presentationBytes: presentation.byteLength,
          presentation,
          initialView: Object.freeze({ entity: 1, hammerId: null, position: [1, 2, 3], angles: [4, 5, 6] }),
          timings: LOAD_TIMINGS,
        }, [...(payload ? [payload] : []), presentation])
        return
      }
      case "models": {
        const bytes = modelPoseOutput(this.modelBits)
        const output = new SharedArrayBuffer(bytes.length + 24)
        new Uint8Array(output, 12, bytes.length).set(bytes)
        if (this.malformedModelOutput) new Uint8Array(output)[12] = 0
        this.modelLeases.set(request.id, output)
        this.#respond({ id: request.id, kind: "models", generation: request.generation, output, byteOffset: 12, byteLength: bytes.length, lease: request.id, timings: TIMINGS })
        if (request.visibility) {
          const outputs = request.visibility.views.map(() => visibilityOutput(this.animatedWorldMaterial))
          this.#respond({
            id: request.visibility.id,
            kind: "visibility",
            generation: request.generation,
            outputs,
            timings: TIMINGS,
          }, outputs)
        }
        return
      }
      case "visibility": {
        const outputs = request.views.map(() => visibilityOutput(this.animatedWorldMaterial))
        this.#respond({ id: request.id, kind: "visibility", generation: request.generation, outputs, timings: TIMINGS }, outputs)
        return
      }
      case "particles": {
        const output = request.batch
        this.#respond({ id: request.id, kind: "particles", generation: request.generation, output, timings: TIMINGS }, [output])
        return
      }
      case "decode-resources": {
        const input = request.chunks[0]!.bytes
        if (request.generation !== undefined) {
          const bytes = new SharedArrayBuffer(input.byteLength + 16)
          const section = new Uint8Array(bytes, 8, input.byteLength)
          section.set(new Uint8Array(input))
          const retained = this.resources.get(request.generation) ?? []
          retained.push(section)
          this.resources.set(request.generation, retained)
          this.#respond({ id: request.id, kind: "resources", bytes, byteOffset: 8, byteLength: input.byteLength })
        } else {
          this.#respond({ id: request.id, kind: "resources", bytes: input, byteOffset: 0, byteLength: input.byteLength }, [input])
        }
        return
      }
      case "finalize-resources": {
        const sections = this.resources.get(request.generation) ?? []
        const first = sections[0]
        if (!first) {
          this.#respond({ id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 })
          return
        }
        void (request.authenticatedIdentity ? Promise.resolve(request.authenticatedIdentity.sha256) : digest(first)).then((sha256) => this.#respond({
          id: request.id,
          kind: "resources-finalized",
          generation: request.generation,
          byteLength: sections.reduce((total, section) => total + section.byteLength, 0),
          sha256,
          sections: sections.length,
        }))
        return
      }
      case "retain-resources": {
        const retained = this.resources.get(request.generation) ?? []
        if ("section" in request) retained.push(new Uint8Array(request.section).slice())
        else {
          const source = this.resources.get(request.sourceGeneration)?.[request.sectionIndex]
          if (!source) {
            this.#respond({ id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 })
            return
          }
          retained.push(source)
        }
        this.resources.set(request.generation, retained)
        this.#respond({ id: request.id, kind: "resources-retained", generation: request.generation })
        return
      }
      case "release-resources":
        this.resources.delete(request.generation)
        this.#respond({ id: request.id, kind: "resources-released", generation: request.generation })
        return
      case "discard":
        this.#respond({ id: request.id, kind: "discarded", generation: request.generation })
        return
      case "activate":
        this.#respond({ id: request.id, kind: "activated", generation: request.generation })
        return
      case "team-selection": {
        const team = request.choice === "red" ? 2 : request.choice === "blue" ? 3 : request.choice === "spectate" ? 1 : 0
        this.#respond({
          id: request.id,
          kind: "team-selection",
          generation: request.generation,
          state: Object.freeze({
            localTeam: team,
            redCount: Number(team === 2),
            blueCount: Number(team === 3),
            redDisabled: false,
            blueDisabled: false,
            spectatorsVisible: true,
            autoAssignVisible: true,
            cancelVisible: team !== 0,
            highlander: false,
            teamsFull: false,
            teamsFullArrow: false,
          }),
        })
        return
      }
      case "shutdown":
        if (this.modelLeases.size > 0) { this.closing = request.id; return }
        this.#respond({ id: request.id, kind: "shutdown" })
        return
      case "release-model-output":
        const output = this.modelLeases.get(request.lease)
        if (output) new Uint8Array(output).fill(0xff)
        this.modelLeases.delete(request.lease)
        if (this.closing !== undefined && this.modelLeases.size === 0) {
          this.#respond({ id: this.closing, kind: "shutdown" })
          this.closing = undefined
        }
        return
      default:
        this.#respond({ id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 })
    }
  }

  #respond(response: WorkerResponse, transfer: Transferable[] = []): void {
    const received = structuredClone(response, { transfer })
    if (response.kind === "resources" && received.kind === "resources" && response.bytes instanceof SharedArrayBuffer) {
      Object.assign(received, { bytes: response.bytes })
    }
    if (response.kind === "models" && received.kind === "models" && response.output instanceof SharedArrayBuffer) {
      Object.assign(received, { output: response.output })
    }
    queueMicrotask(() => this.#message?.({ data: received } as MessageEvent<WorkerResponse>))
  }
}

describe("TF2 Worker transport ownership", () => {
  test("rejects a stale gameplay Worker generation before WASM or resources are published", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    worker.workerBuild = "ef".repeat(32)
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const wasm = Uint8Array.from([1, 2, 3])
    await expect(client.initialize(wasm, await digest(wasm), 1)).rejects.toThrow("GenerationMismatch")
    expect(worker.requests).toHaveLength(1)
    expect(worker.requests[0]).toMatchObject({ kind: "initialize", applicationBuild: BUILD, presentationSchema: 16 })
    expect(worker.terminated).toBe(true)
  })

  test("authenticates the exact application, presentation schema, and WASM before accepting its Worker", async () => {
    for (const mismatch of ["application", "schema", "wasm"] as const) {
      const worker = new PipelineWorker(await digest(MAP))
      if (mismatch === "application") worker.workerBuild = "ef".repeat(32)
      if (mismatch === "schema") worker.presentationSchema = 15
      const wasm = Uint8Array.from([1, 2, 3])
      const actual = await digest(wasm)
      if (mismatch === "wasm") worker.workerWasmSha256 = "ef".repeat(32)
      const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
      await expect(client.initialize(wasm, actual, 1)).rejects.toThrow("GenerationMismatch")
      expect(worker.terminated).toBe(true)
    }
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const wasm = Uint8Array.from([1, 2, 3])
    await expect(client.initialize(wasm, await digest(wasm), 1)).resolves.toBeUndefined()
    await client.shutdown()
  })

  test("rejects an unauthenticated application generation before attaching a Worker", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    expect(() => new Tf2WorkerClient(worker, new MemoryCache(), "stale")).toThrow("IntegrityFailure")
    expect(worker.requests).toEqual([])
  })

  test("returns owned exact palettes before releasing and reusing the shared model lease", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const result = await client.models(7, new Uint8Array(12))
    expect(result[0]!.model).toBe("model")
    expect(result[0]!.boneMatrices.buffer).toBeInstanceOf(ArrayBuffer)
    expect([...new Uint32Array(result[0]!.boneMatrices.buffer)]).toEqual([0x3f800000, 0x80000000, 0, 0, 0, 0x3f800000, 0, 0, 0, 0, 0x3f800000, 0])
    expect(worker.modelLeases.size).toBe(0)
    expect(worker.requests.map(request => request.kind)).toEqual(["models", "release-model-output"])
    const request = worker.requests[1]
    expect(request).toMatchObject({ id: worker.requests[0]!.id, generation: 7, lease: worker.requests[0]!.id })
    await client.shutdown()
  })

  test("releases a shared model lease after a synchronous decoder rejects its exact bytes", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    worker.malformedModelOutput = true
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    await expect(client.models(3, new Uint8Array(12))).rejects.toThrow("model pose output identity")
    expect(worker.modelLeases.size).toBe(0)
    expect(worker.requests.map(request => request.kind)).toEqual(["models", "release-model-output"])
    await client.shutdown()
  })

  test("does not reclaim an unread model lease when shutdown overtakes the browser response", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const models = client.models(3, new Uint8Array(12))
    const shutdown = client.shutdown()
    expect(client.shutdown()).toBe(shutdown)
    await expect(client.models(3, new Uint8Array(12))).rejects.toThrow("Closed")
    const [poses] = await Promise.all([models, shutdown])
    expect(poses[0]!.model).toBe("model")
    expect(poses[0]!.boneMatrices[0]).toBe(1)
    expect(Object.is(poses[0]!.boneMatrices[1], -0)).toBe(true)
    expect(worker.requests.map(request => request.kind)).toEqual(["models", "shutdown", "release-model-output"])
    expect(worker.modelLeases.size).toBe(0)
    expect(worker.terminated).toBe(true)
  })

  test("preserves every finite binary32 edge and rejects nonfinite palettes before publication", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const bits = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000, 0x80800000, 0x7f7fffff, 0xff7fffff, 0x3f800001, 0xbf800001]
    worker.modelBits = bits
    const poses = await client.models(2, new Uint8Array(12))
    expect([...new Uint32Array(poses[0]!.boneMatrices.buffer)]).toEqual(bits)
    for (const invalid of [0x7fc00001, 0x7f800001, 0xffc00001, 0x7f800000, 0xff800000]) {
      worker.modelBits = [invalid, ...bits.slice(1)]
      await expect(client.models(2, new Uint8Array(12))).rejects.toThrow("model pose scalar")
      expect(worker.modelLeases.size).toBe(0)
    }
    await client.shutdown()
  })

  test("rejects malformed lease identities and ranges without retaining a live Worker", async () => {
    for (const mutation of [{ byteOffset: -1 }, { byteOffset: 1 }, { byteLength: NaN }, { byteLength: 65 * 1024 * 1024 }, { lease: 9 }, { generation: 4 }]) {
      const worker = new PipelineWorker(await digest(MAP))
      worker.failure = { id: 1, kind: "models", generation: 2, lease: 1, output: new SharedArrayBuffer(12), byteOffset: 0, byteLength: 12, timings: TIMINGS, ...mutation }
      const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
      await expect(client.models(2, new Uint8Array(12))).rejects.toThrow("WorkerFailed")
      expect(worker.terminated).toBe(true)
    }
  })

  test("reads and changes the authoritative team roster without a simulation-frame crossing", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    expect(await client.teamSelection(7)).toMatchObject({ localTeam: 0, redCount: 0, blueCount: 0, cancelVisible: false })
    expect(await client.teamSelection(7, "blue")).toMatchObject({ localTeam: 3, redCount: 0, blueCount: 1, cancelVisible: true })
    expect(worker.requests.map((request) => request.kind)).toEqual(["team-selection", "team-selection"])
    await client.shutdown()
  })

  test("publishes authenticated cold map and presentation bytes in one staged Worker request", async () => {
    const cache = new MemoryCache()
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache, BUILD)
    const bsp = Uint8Array.from([1, 2, 3, 4])
    const source = await configuration(3, [5, 6, 7])
    const staged = await client.stage(3, bsp, 0, source, KEY)
    await staged.persistence
    expect(worker.requests.map((request) => request.kind)).toEqual(["load"])
    expect(worker.requests[0]).toMatchObject({ includeMap: true, generation: 3 })
    expect(staged.payload).toEqual(MAP)
    expect(staged.presentation).toEqual(PRESENTATION)
    expect(staged.cache).toBe("stored")
    expect(staged.presentationCache).toBe("stored")
    expect(bsp.byteLength).toBe(0)
    expect(source.sections[0]!.subarray(0, 3)).toEqual(Uint8Array.from([5, 6, 7]))
    expect(worker.requests[0]).toMatchObject({ configurationSha256: source.sha256, configurationBytes: source.byteLength })
    expect(worker.requests[0]).not.toHaveProperty("configuration")
    expect(cache.maximumConcurrentReads).toBe(2)
    expect(cache.writes.toSorted()).toEqual([KEY, await presentationIdentity(KEY)].toSorted())
    await client.shutdown()
    expect(worker.terminated).toBe(true)
  })

  test("retains every bounded source-backed section without copying it into a map request", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const first = new Uint8Array(12), second = new Uint8Array(12)
    first.set([1, 2, 3]); second.set([4, 5, 6, 7])
    const source: ResourceConfiguration = Object.freeze({ generation: 9, byteLength: 24, sha256: await digest(first), sections: Object.freeze([first, second]) })
    await client.stage(9, Uint8Array.from([8]), 0, source, KEY)
    const request = worker.requests[0]
    expect(request?.kind).toBe("load")
    if (request?.kind !== "load") throw new Error("sectioned Worker map request is absent")
    expect(request).not.toHaveProperty("configuration")
    expect([...first.subarray(0, 3)]).toEqual([1, 2, 3])
    expect([...second.subarray(0, 4)]).toEqual([4, 5, 6, 7])
    await expect(client.stage(10, Uint8Array.from([8]), 0,
      { ...source, sections: Object.freeze([]) }, KEY)).rejects.toMatchObject({ code: "BoundExceeded" })
    await client.shutdown()
  })

  test("round-trips authenticated warm presentation ownership without map copy or follow-up RPC", async () => {
    const cache = new MemoryCache()
    cache.entries.set(KEY, MAP.slice())
    cache.entries.set(await presentationIdentity(KEY), PRESENTATION.slice())
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache, BUILD)
    const staged = await client.stage(8, Uint8Array.from([8]), 1, await configuration(8, [7]), KEY)
    expect(worker.requests.map((request) => request.kind)).toEqual(["load"])
    expect(worker.requests[0]).toMatchObject({ includeMap: false, generation: 8 })
    expect(staged.cache).toBe("hit")
    expect(staged.presentationCache).toBe("hit")
    expect(staged.payload).toEqual(MAP)
    expect(staged.presentation).toEqual(PRESENTATION)
    expect((await staged.persistence).presentationCache).toBe("hit")
    expect(cache.writes).toEqual([])
    await client.shutdown()
  })

  test("recompiles stale PMTX presentation after an application upgrade without discarding the warm map", async () => {
    const cache = new MemoryCache()
    cache.entries.set(KEY, MAP.slice())
    const staleBuild = "ef".repeat(32)
    cache.entries.set(await presentationIdentity(KEY, staleBuild), Uint8Array.from([0x50, 0x4d, 0x54, 0x58, 2, 0, 0, 0]))
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache, BUILD)
    const staged = await client.stage(11, Uint8Array.of(1), 0, await configuration(11), KEY)
    await staged.persistence
    expect(staged.cache).toBe("hit")
    expect(staged.presentationCache).toBe("stored")
    expect(worker.requests[0]).toMatchObject({ includeMap: false })
    expect(worker.requests[0]).not.toHaveProperty("presentation")
    expect(cache.entries.has(await presentationIdentity(KEY, staleBuild))).toBe(true)
    expect(cache.entries.get(await presentationIdentity(KEY))).toEqual(PRESENTATION)
    await client.shutdown()
  })

  test("rejects a verified-but-wrong cached map and discards its staged generation atomically", async () => {
    const cache = new MemoryCache()
    cache.entries.set(KEY, Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8]))
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache, BUILD)
    await expect(client.stage(4, Uint8Array.from([1]), 0, await configuration(4, [2]), KEY))
      .rejects.toMatchObject({ code: "IntegrityFailure" })
    expect(worker.requests.map((request) => request.kind)).toEqual(["load", "discard"])
    await client.shutdown()
  })

  test("coalesces same-turn model and exact visibility phases without delaying the model response", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const batch = new Uint8Array(12)
    const order: string[] = []
    const models = client.models(2, batch).then((value) => { order.push("models"); return value })
    const visibility = client.visibility(2, VIEW).then((value) => { order.push("visibility"); return value })
    const [posed, visible] = await Promise.all([models, visibility])
    expect(worker.requests.map((request) => request.kind)).toEqual(["models", "release-model-output"])
    expect(worker.requests[0]).toMatchObject({ visibility: { views: [VIEW] } })
    expect(batch.byteLength).toBe(0)
    expect(posed).toMatchObject([{ identity: 9, model: "model" }])
    expect([...visible.surfaces]).toEqual([4, 9])
    expect([...visible.drawSurfaces]).toEqual([9])
    expect(visible.surfaces.buffer).toBe(visible.drawSurfaces.buffer)
    expect(visible.eyeLeaf).toBe(3)
    expect(visible.leaves).toEqual([3])
    expect(visible.areas).toEqual([7])
    expect(order).toEqual(["models", "visibility"])
    await client.shutdown()
  })

  test("transports Rust-evaluated world bump frames and transforms in the existing visibility response", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    worker.animatedWorldMaterial = true
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const visible = await client.visibility(2, VIEW)
    expect(worker.requests.map((request) => request.kind)).toEqual(["visibility"])
    expect(visible.worldMaterials).toHaveLength(1)
    expect(visible.worldMaterials[0]).toMatchObject({
      identity: "materials/water/test.vmt",
      mapMaterial: 84,
      textures: [{ role: 7, frame: 15 }],
    })
    expect([...visible.worldMaterials[0]!.textures[0]!.transform!]).toEqual([
      1, 0, 0, 0,
      0, 1, 0, 0,
      0, 0, 1, 0,
      0, 0, 0, 1,
    ])
    await client.shutdown()
  })

  test("publishes distinct authored main and sky views atomically in one model companion", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const sky = { ...VIEW, position: [8, 9, 10] as const, visibilityPosition: [1, 2, 3] as const, areaFilter: 7 }
    const models = client.models(2, new Uint8Array(12))
    const views = client.visibilityViews(2, [VIEW, sky])
    const [, results] = await Promise.all([models, views])
    expect(worker.requests).toHaveLength(2)
    expect(worker.requests[1]!.kind).toBe("release-model-output")
    expect(worker.requests[0]).toMatchObject({ kind: "models", visibility: { views: [VIEW, sky] } })
    expect(results).toHaveLength(2)
    expect(results[0]).not.toBe(results[1])
    await client.shutdown()
  })

  test("deduplicates only exactly equivalent multi-view identities", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const equivalent = { ...VIEW, position: [...VIEW.position] as [number, number, number] }
    const [first, second] = await client.visibilityViews(2, [VIEW, equivalent])
    expect(first).toBe(second)
    expect(worker.requests[0]).toMatchObject({ kind: "visibility", views: [VIEW] })
    await client.visibilityViews(2, [VIEW, { ...VIEW, presentationTimeSeconds: VIEW.presentationTimeSeconds + 1 }])
    expect(worker.requests[1]).toMatchObject({ views: [VIEW, { presentationTimeSeconds: VIEW.presentationTimeSeconds + 1 }] })
    await expect(client.visibilityViews(2, [])).rejects.toMatchObject({ code: "BoundExceeded" })
    await expect(client.visibilityViews(2, [VIEW, VIEW, VIEW])).rejects.toMatchObject({ code: "BoundExceeded" })
    await client.shutdown()
  })

  test("rejects malformed multi-view publication without exposing a partial result", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    worker.failure = { kind: "failure", code: "TransitionFailed", detail: 203 }
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    await expect(client.visibilityViews(2, [VIEW, { ...VIEW, areaFilter: 7 }]))
      .rejects.toMatchObject({ code: "TransitionFailed", detail: 203 })
    worker.failure = undefined
    await client.shutdown()
  })

  test("flushes deferred models before a later ordered particle transaction", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const models = client.models(2, new Uint8Array(12))
    const particles = client.particles(2, new Uint8Array(32))
    await Promise.all([models, particles])
    expect(worker.requests.map((request) => request.kind)).toEqual(["models", "particles", "release-model-output"])
    await client.shutdown()
  })

  test("moves owned resource batches while preserving unrelated bytes around sliced views", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const owned = new Uint8Array(12)
    owned[0] = 7
    expect(await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: owned }])).toEqual(Uint8Array.from([7, ...new Array(11).fill(0)]))
    expect(owned.byteLength).toBe(0)
    expect(worker.requests[0]).toMatchObject({ kind: "decode-resources", chunks: [{ descriptor: expect.any(ArrayBuffer) }] })
    const retained = new Uint8Array(20)
    retained.fill(3)
    const slice = retained.subarray(4, 16)
    expect(await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: slice }])).toEqual(new Uint8Array(12).fill(3))
    expect(retained.byteLength).toBe(20)
    expect(retained[0]).toBe(3)
    await client.shutdown()
  })

  test("shares gameplay source sections with the Worker without cloning or detaching retained authored bytes", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const input = new Uint8Array(12)
    input.fill(9)
    const decoded = await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: input }], 3)
    expect(decoded.buffer).toBeInstanceOf(SharedArrayBuffer)
    expect(decoded.byteOffset).toBe(8)
    const resources = await client.finalizeResources(3, [decoded])
    await client.stage(3, Uint8Array.from([1]), 0, resources, KEY)
    const request = worker.requests.find((entry) => entry.kind === "load")
    if (!request || request.kind !== "load") throw new Error("load request is absent")
    expect(request).not.toHaveProperty("configuration")
    worker.resources.get(3)![0]![1] = 19
    expect(decoded[1]).toBe(19)
    await client.shutdown()
  })

  test("retains bounded source sections in the Worker without cloning a monolithic configuration", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const input = new Uint8Array(12)
    input[0] = 9
    const section = await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: input }], 5)
    const resources = await client.finalizeResources(5, [section])
    expect(input.byteLength).toBe(0)
    expect(resources).toMatchObject({ generation: 5, byteLength: 12, sha256: await digest(section) })
    expect(worker.requests.map((request) => request.kind)).toEqual(["decode-resources", "finalize-resources"])
    const staged = await client.stage(5, Uint8Array.of(1), 0, resources, KEY)
    expect(staged.payload).toEqual(MAP)
    expect(worker.requests.at(-1)).not.toHaveProperty("configuration")
    await client.shutdown()
  })

  test("reuses an authenticated generation-bound source identity without accepting mismatched sections or a substituted response", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const input = new Uint8Array(12).fill(7)
    const section = await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: input }], 8)
    const identity = { byteLength: section.byteLength, sha256: await digest(section) }
    expect(await client.finalizeResources(8, [section], identity)).toMatchObject(identity)
    expect(worker.requests.at(-1)).toMatchObject({ kind: "finalize-resources", generation: 8, authenticatedIdentity: identity })
    await expect(client.finalizeResources(8, [section], { ...identity, byteLength: identity.byteLength + 1 }))
      .rejects.toMatchObject({ code: "BoundExceeded" })
    await expect(client.finalizeResources(8, [section], { ...identity, sha256: "invalid" }))
      .rejects.toMatchObject({ code: "BoundExceeded" })
    await client.shutdown()
  })

  test("readmits retained source sections individually when a later generation reuses them", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const resources = await configuration(3, [8, 7, 6])
    const staged = await client.stage(4, Uint8Array.of(1), 0, resources, KEY)
    expect(staged.payload).toEqual(MAP)
    expect(worker.requests.map((request) => request.kind)).toEqual(["retain-resources", "finalize-resources", "load"])
    expect(resources.sections[0]!.byteLength).toBe(12)
    await client.shutdown()
  })

  test("retains authenticated shared source owners across generations without copying their bytes", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const section = await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: new Uint8Array(12).fill(4) }], 3)
    const resources = await client.finalizeResources(3, [section])
    const staged = await client.stage(4, Uint8Array.of(1), 0, resources, KEY)
    expect(staged.payload).toEqual(MAP)
    expect(worker.requests.find((request) => request.kind === "retain-resources"))
      .toMatchObject({ generation: 4, sourceGeneration: 3, sectionIndex: 0 })
    expect(worker.resources.get(4)![0]).toBe(worker.resources.get(3)![0])
    await client.releaseResources(3)
    expect(worker.resources.get(4)![0]![0]).toBe(4)
    await client.shutdown()
  })

  test("rejects stale, out-of-bounds, and reversed shared section owners", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const section = await client.decodeResources([{ descriptor: RESOURCE_CHUNK, bytes: new Uint8Array(12) }], 6)
    const resources = await client.finalizeResources(6, [section])
    await expect(client.retainResourceSection(6, resources, 0)).rejects.toMatchObject({ code: "BoundExceeded" })
    await expect(client.retainResourceSection(7, resources, 1)).rejects.toMatchObject({ code: "BoundExceeded" })
    await client.releaseResources(6)
    await expect(client.retainResourceSection(7, resources, 0)).rejects.toMatchObject({ code: "MalformedRequest" })
    await client.shutdown()
  })

  test("rejects the sixty-fifth pending request without dropping its first sixty-four", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const pending = Array.from({ length: 64 }, () => client.visibility(2, VIEW))
    await expect(client.visibility(2, VIEW)).rejects.toMatchObject({ code: "BoundExceeded" })
    expect(await Promise.all(pending)).toHaveLength(64)
    expect(worker.requests).toHaveLength(64)
    await client.shutdown()
  })

  test("never joins visibility from a different staged generation", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const models = client.models(2, new Uint8Array(12))
    const visibility = client.visibility(3, VIEW)
    await Promise.all([models, visibility])
    expect(worker.requests.map((request) => [request.kind, "generation" in request ? request.generation : null]))
      .toEqual([["models", 2], ["visibility", 3], ["release-model-output", 2]])
    await client.shutdown()
  })

  test("retains presentation persistence failure without discarding a valid staged generation", async () => {
    const cache = new class extends MemoryCache {
      override async write(key: string, expected: string | null, bytes: Uint8Array): Promise<string> {
        if (expected === null) throw new Error("presentation storage unavailable")
        return super.write(key, expected, bytes)
      }
    }()
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache, BUILD)
    const staged = await client.stage(3, Uint8Array.from([1]), 0, await configuration(3, [2]), KEY)
    expect(await staged.persistence).toMatchObject({
      presentationCache: "unavailable",
      presentationCacheError: "Error: presentation storage unavailable",
    })
    await client.activate(3)
    expect(worker.requests.map((request) => request.kind)).toEqual(["load", "activate"])
    await client.shutdown()
  })

  test("rejects both coalesced requests when transfer fails and cleans up shutdown", async () => {
    const worker = new class extends PipelineWorker {
      override postMessage(): void { throw new Error("transfer failed") }
    }(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    const models = client.models(2, new Uint8Array(12))
    const visibility = client.visibility(2, VIEW)
    const results = await Promise.allSettled([models, visibility])
    expect(results.every((result) => result.status === "rejected" && result.reason.code === "WorkerFailed")).toBe(true)
    await expect(client.shutdown()).rejects.toMatchObject({ code: "WorkerFailed" })
    expect(worker.terminated).toBe(true)
  })

  test("preserves structured Worker failures and closes every outstanding transport", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    worker.failure = { id: 0, kind: "failure", code: "StaleGeneration", detail: 0 }
    const client = new Tf2WorkerClient(worker, new MemoryCache(), BUILD)
    await expect(client.visibility(99, VIEW)).rejects.toMatchObject({ code: "StaleGeneration" })
    worker.failure = undefined
    await client.shutdown()
    await expect(client.visibility(99, VIEW)).rejects.toBeInstanceOf(Tf2WorkerError)
    expect(worker.terminated).toBe(true)
  })
})
