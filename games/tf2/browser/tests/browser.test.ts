import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { Tf2WorkerClient, type WorkerLike } from "../src/client"
import { decodeSnapshot, encodeCommand, mapDerivedKey } from "../src/codec"
import { tf2Audio, tf2Presentation } from "../src/presentation"
import type { WorkerRequest, WorkerResponse } from "../src/protocol"

function snapshot(): ArrayBuffer {
  const bytes = new ArrayBuffer(56)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x53, 0x53, 0x4e])
  view.setUint32(4, 2, true)
  view.setBigUint64(8, 7n, true)
  data[16] = 1
  data[17] = 1
  data[18] = 1
  view.setFloat32(44, 200, true)
  return bytes
}

function teleportSnapshot(): ArrayBuffer {
  const bytes = new ArrayBuffer(84)
  new Uint8Array(bytes).set(new Uint8Array(snapshot()))
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  view.setUint32(52, 1, true)
  data.set([8, 1, 0, 0], 56)
  view.setUint32(60, 20, true)
  view.setUint32(64, 21, true)
  for (const [index, value] of [10, 11, 12, 90].entries()) {
    view.setFloat32(68 + index * 4, value, true)
  }
  return bytes
}

class MemoryCache implements DerivedObjectCache {
  readonly records = new Map<string, Uint8Array>()
  async read(key: string): Promise<Uint8Array | undefined> {
    return this.records.get(key)?.slice()
  }
  async write(key: string, _sha256: string, bytes: Uint8Array): Promise<void> {
    this.records.set(key, bytes.slice())
  }
  async remove(key: string): Promise<void> {
    this.records.delete(key)
  }
  close(): void {}
}

class FakeWorker implements WorkerLike {
  readonly payload = new TextEncoder().encode("map payload")
  readonly payloadSha256: string
  mapReads = 0
  activations = 0
  discards = 0
  terminated = false
  #message?: (event: MessageEvent<WorkerResponse>) => void
  #error?: (event: ErrorEvent) => void
  constructor(payloadSha256: string) {
    this.payloadSha256 = payloadSha256
  }
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  addEventListener(
    type: "message" | "error",
    listener: ((event: MessageEvent<WorkerResponse>) => void) | ((event: ErrorEvent) => void),
  ): void {
    if (type === "message") this.#message = listener as (event: MessageEvent<WorkerResponse>) => void
    else this.#error = listener as (event: ErrorEvent) => void
  }
  removeEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  removeEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  removeEventListener(type: "message" | "error"): void {
    if (type === "message") this.#message = undefined
    else this.#error = undefined
  }
  postMessage(request: WorkerRequest): void {
    let response: WorkerResponse
    switch (request.kind) {
      case "initialize": response = { id: request.id, kind: "initialized" }; break
      case "load": response = {
        id: request.id,
        kind: "loaded",
        generation: request.generation,
        payloadBytes: this.payload.byteLength,
        payloadSha256: this.payloadSha256,
      }; break
      case "read-map":
        this.mapReads += 1
        response = {
          id: request.id,
          kind: "map",
          generation: request.generation,
          payload: this.payload.slice().buffer,
        }
        break
      case "activate":
        this.activations += 1
        response = { id: request.id, kind: "activated", generation: request.generation }
        break
      case "discard":
        this.discards += 1
        response = { id: request.id, kind: "discarded", generation: request.generation }
        break
      case "advance": response = {
        id: request.id,
        kind: "snapshot",
        generation: request.generation,
        snapshot: snapshot(),
      }; break
      case "shutdown": response = { id: request.id, kind: "shutdown" }; break
    }
    queueMicrotask(() => this.#message?.({ data: response } as MessageEvent<WorkerResponse>))
  }
  terminate(): void {
    this.terminated = true
  }
}

describe("TF2 browser adapter", () => {
  test("encodes commands and rejects malformed snapshots", async () => {
    const command = encodeCommand({
      forward: 240,
      side: 0,
      yawDegrees: 90,
      pitchDegrees: -30,
      jump: true,
      crouch: false,
      fire: true,
      detonate: false,
      selectClass: 1,
      selectWeapon: 2,
    })
    const view = new DataView(command)
    expect(view.getUint32(16, true)).toBe(5)
    expect(view.getUint32(20, true)).toBe(0x0201)
    expect(decodeSnapshot(snapshot()).tick).toBe(7n)
    expect(decodeSnapshot(teleportSnapshot()).events[0]).toEqual({
      kind: 8,
      detail: 1,
      subject: 20,
      auxiliary: 21,
      values: [10, 11, 12, 90],
    })
    expect(() => decodeSnapshot(snapshot().slice(0, 55))).toThrow()
    expect(await mapDerivedKey("0".repeat(64), 0, new Uint8Array())).toMatch(/^[0-9a-f]{64}$/)
  })

  test("transfers a compiled map once and reuses its verified derived cache entry", async () => {
    const payload = new TextEncoder().encode("map payload")
    const digest = await crypto.subtle.digest("SHA-256", payload)
    const hash = Array.from(new Uint8Array(digest), (value) => value.toString(16).padStart(2, "0")).join("")
    const worker = new FakeWorker(hash)
    const cache = new MemoryCache()
    const client = new Tf2WorkerClient(worker, cache)
    const wasm = new Uint8Array([0, 97, 115, 109])
    const wasmDigest = await crypto.subtle.digest("SHA-256", wasm)
    const wasmHash = Array.from(
      new Uint8Array(wasmDigest),
      (value) => value.toString(16).padStart(2, "0"),
    ).join("")
    await client.initialize(wasm, wasmHash)
    const key = "1".repeat(64)
    expect((await client.load(1, new Uint8Array([1]), 0, new Uint8Array(), key)).cache).toBe("stored")
    expect((await client.load(2, new Uint8Array([1]), 0, new Uint8Array(), key)).cache).toBe("hit")
    expect(worker.mapReads).toBe(1)
    cache.records.set(key, new TextEncoder().encode("corrupt"))
    await expect(client.load(3, new Uint8Array([1]), 0, new Uint8Array(), key)).rejects.toMatchObject({
      code: "IntegrityFailure",
    })
    expect(worker.activations).toBe(2)
    expect(worker.discards).toBe(1)
    expect((await client.advance(2, encodeCommand({
      forward: 0,
      side: 0,
      yawDegrees: 0,
      pitchDegrees: 0,
      jump: false,
      crouch: false,
      fire: false,
      detonate: false,
    }), 1)).tick).toBe(7n)
    await client.shutdown()
    expect(worker.terminated).toBe(true)
  })

  test("reports unavailable presentation dependencies before diagnostic substitutes", () => {
    const base = decodeSnapshot(snapshot())
    const projectile = {
      id: 9,
      kind: 1 as const,
      armed: false,
      stuck: false,
      position: [1, 2, 3] as const,
      velocity: [4, 5, 6] as const,
      age: 0.25,
    }
    const value = tf2Presentation({
      ...base,
      projectiles: [projectile],
      events: [{ kind: 4, detail: 1, subject: 8, auxiliary: 0, values: [7, 8, 9, 0] }],
    }, [], false)
    expect(value.effects).toEqual([])
    expect(value.models).toHaveLength(2)
    expect(value.diagnostics.map((item) => item.code)).toEqual([
      "MissingAudioContext",
      "MissingParticleContext",
    ])
    expect(tf2Audio(value.diagnostics.length ? [{
      kind: 4,
      detail: 1,
      subject: 8,
      auxiliary: 0,
      values: [7, 8, 9, 0],
    }] : [])).toEqual([{
      voice: 20,
      resource: "sound/weapons/explode3.wav",
      gain: 1,
      pan: 0,
      loop: false,
    }])
    expect(tf2Presentation({ ...base, projectiles: [projectile], events: [] }, [], true).effects)
      .toHaveLength(1)
  })
})
