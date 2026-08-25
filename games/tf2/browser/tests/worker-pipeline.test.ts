import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { Tf2WorkerClient, Tf2WorkerError, type WorkerLike } from "../src/client"
import type { VisibilityView, WorkerRequest, WorkerResponse, WorkerTransactionTimings } from "../src/protocol"

const MAP = Uint8Array.from([0x50, 0x53, 0x4d, 0x50, 9, 8, 7, 6])
const PRESENTATION = Uint8Array.from([0x50, 0x54, 0x46, 0x32, 1, 2, 3, 4])
const KEY = "ab".repeat(32)
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
  totalMilliseconds: 0,
})

async function digest(bytes: Uint8Array): Promise<string> {
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("")
}

async function presentationIdentity(key: string): Promise<string> {
  return digest(new TextEncoder().encode(`playsrc-tf2-presentation-v13\0${key}`))
}

function visibilityOutput(animated = false): ArrayBuffer {
  const words = [2, 4, 9, 1, 9, 3, 1, 3, 1, 7]
  const identity = new TextEncoder().encode("materials/water/test.vmt")
  const bytes = new Uint8Array(76 + words.length * 4 + 8 + 8 + (animated ? 12 + identity.length + 72 : 0))
  const view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x56, 0x49, 0x53])
  view.setUint32(4, 5, true)
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

  async read(key: string): Promise<Uint8Array | undefined> {
    this.reads += 1
    this.#activeReads += 1
    this.maximumConcurrentReads = Math.max(this.maximumConcurrentReads, this.#activeReads)
    await new Promise((resolve) => setTimeout(resolve, 5))
    this.#activeReads -= 1
    return this.entries.get(key)?.slice()
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

class PipelineWorker implements WorkerLike {
  readonly requests: WorkerRequest[] = []
  readonly mapHash: string
  failure?: WorkerResponse
  animatedWorldMaterial = false
  terminated = false
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
        const output = new Uint8Array([0x50, 0x4d, 0x50, 0x4f]).buffer
        this.#respond({ id: request.id, kind: "models", generation: request.generation, output, timings: TIMINGS }, [output])
        if (request.visibility) {
          const visibility = visibilityOutput(this.animatedWorldMaterial)
          this.#respond({
            id: request.visibility.id,
            kind: "visibility",
            generation: request.generation,
            output: visibility,
            timings: TIMINGS,
          }, [visibility])
        }
        return
      }
      case "visibility": {
        const output = visibilityOutput(this.animatedWorldMaterial)
        this.#respond({ id: request.id, kind: "visibility", generation: request.generation, output, timings: TIMINGS }, [output])
        return
      }
      case "particles": {
        const output = request.batch
        this.#respond({ id: request.id, kind: "particles", generation: request.generation, output, timings: TIMINGS }, [output])
        return
      }
      case "decode-resources": {
        const bytes = request.batch
        this.#respond({ id: request.id, kind: "resources", bytes }, [bytes])
        return
      }
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
        this.#respond({ id: request.id, kind: "shutdown" })
        return
      default:
        this.#respond({ id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 })
    }
  }

  #respond(response: WorkerResponse, transfer: Transferable[] = []): void {
    const received = structuredClone(response, { transfer })
    queueMicrotask(() => this.#message?.({ data: received } as MessageEvent<WorkerResponse>))
  }
}

describe("TF2 Worker transport ownership", () => {
  test("reads and changes the authoritative team roster without a simulation-frame crossing", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    expect(await client.teamSelection(7)).toMatchObject({ localTeam: 0, redCount: 0, blueCount: 0, cancelVisible: false })
    expect(await client.teamSelection(7, "blue")).toMatchObject({ localTeam: 3, redCount: 0, blueCount: 1, cancelVisible: true })
    expect(worker.requests.map((request) => request.kind)).toEqual(["team-selection", "team-selection"])
    await client.shutdown()
  })

  test("publishes authenticated cold map and presentation bytes in one staged Worker request", async () => {
    const cache = new MemoryCache()
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache)
    const bsp = Uint8Array.from([1, 2, 3, 4])
    const configuration = Uint8Array.from([5, 6, 7])
    const staged = await client.stage(3, bsp, 0, configuration, KEY)
    await staged.persistence
    expect(worker.requests.map((request) => request.kind)).toEqual(["load"])
    expect(worker.requests[0]).toMatchObject({ includeMap: true, generation: 3 })
    expect(staged.payload).toEqual(MAP)
    expect(staged.presentation).toEqual(PRESENTATION)
    expect(staged.cache).toBe("stored")
    expect(staged.presentationCache).toBe("stored")
    expect(bsp.byteLength).toBe(0)
    expect(configuration).toEqual(Uint8Array.from([5, 6, 7]))
    expect(cache.maximumConcurrentReads).toBe(2)
    expect(cache.writes.toSorted()).toEqual([KEY, await presentationIdentity(KEY)].toSorted())
    await client.shutdown()
    expect(worker.terminated).toBe(true)
  })

  test("round-trips authenticated warm presentation ownership without map copy or follow-up RPC", async () => {
    const cache = new MemoryCache()
    cache.entries.set(KEY, MAP.slice())
    cache.entries.set(await presentationIdentity(KEY), PRESENTATION.slice())
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache)
    const staged = await client.stage(8, Uint8Array.from([8]), 1, Uint8Array.from([7]), KEY)
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

  test("rejects a verified-but-wrong cached map and discards its staged generation atomically", async () => {
    const cache = new MemoryCache()
    cache.entries.set(KEY, Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8]))
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, cache)
    await expect(client.stage(4, Uint8Array.from([1]), 0, Uint8Array.from([2]), KEY))
      .rejects.toMatchObject({ code: "IntegrityFailure" })
    expect(worker.requests.map((request) => request.kind)).toEqual(["load", "discard"])
    await client.shutdown()
  })

  test("coalesces same-turn model and exact visibility phases without delaying the model response", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    const batch = new Uint8Array(12)
    const order: string[] = []
    const models = client.models(2, batch).then((value) => { order.push("models"); return value })
    const visibility = client.visibility(2, VIEW).then((value) => { order.push("visibility"); return value })
    const [posed, visible] = await Promise.all([models, visibility])
    expect(worker.requests.map((request) => request.kind)).toEqual(["models"])
    expect(worker.requests[0]).toMatchObject({ visibility: { view: VIEW } })
    expect(batch.byteLength).toBe(0)
    expect(posed).toEqual(Uint8Array.from([0x50, 0x4d, 0x50, 0x4f]))
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
    const client = new Tf2WorkerClient(worker, new MemoryCache())
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

  test("flushes deferred models before a later ordered particle transaction", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    const models = client.models(2, new Uint8Array(12))
    const particles = client.particles(2, new Uint8Array(32))
    await Promise.all([models, particles])
    expect(worker.requests.map((request) => request.kind)).toEqual(["models", "particles"])
    await client.shutdown()
  })

  test("moves owned resource batches while preserving unrelated bytes around sliced views", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    const owned = new Uint8Array(12)
    owned[0] = 7
    expect(await client.decodeResources(owned)).toEqual(Uint8Array.from([7, ...new Array(11).fill(0)]))
    expect(owned.byteLength).toBe(0)
    const retained = new Uint8Array(20)
    retained.fill(3)
    const slice = retained.subarray(4, 16)
    expect(await client.decodeResources(slice)).toEqual(new Uint8Array(12).fill(3))
    expect(retained.byteLength).toBe(20)
    expect(retained[0]).toBe(3)
    await client.shutdown()
  })

  test("rejects the sixty-fifth pending request without dropping its first sixty-four", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    const pending = Array.from({ length: 64 }, () => client.visibility(2, VIEW))
    await expect(client.visibility(2, VIEW)).rejects.toMatchObject({ code: "BoundExceeded" })
    expect(await Promise.all(pending)).toHaveLength(64)
    expect(worker.requests).toHaveLength(64)
    await client.shutdown()
  })

  test("never joins visibility from a different staged generation", async () => {
    const worker = new PipelineWorker(await digest(MAP))
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    const models = client.models(2, new Uint8Array(12))
    const visibility = client.visibility(3, VIEW)
    await Promise.all([models, visibility])
    expect(worker.requests.map((request) => [request.kind, "generation" in request ? request.generation : null]))
      .toEqual([["models", 2], ["visibility", 3]])
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
    const client = new Tf2WorkerClient(worker, cache)
    const staged = await client.stage(3, Uint8Array.from([1]), 0, Uint8Array.from([2]), KEY)
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
    const client = new Tf2WorkerClient(worker, new MemoryCache())
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
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    await expect(client.visibility(99, VIEW)).rejects.toMatchObject({ code: "StaleGeneration" })
    worker.failure = undefined
    await client.shutdown()
    await expect(client.visibility(99, VIEW)).rejects.toBeInstanceOf(Tf2WorkerError)
    expect(worker.terminated).toBe(true)
  })
})
