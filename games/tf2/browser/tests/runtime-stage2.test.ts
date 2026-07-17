import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { Tf2WorkerClient, type WorkerLike } from "../src/client"
import {
  decodeSnapshot,
  encodeCommand,
  encodeJumpCourse,
  Tf2CodecError,
} from "../src/codec"
import type { WorkerRequest, WorkerResponse } from "../src/protocol"

function snapshot(): ArrayBuffer {
  const bytes = new ArrayBuffer(433)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x53, 0x53, 0x4e])
  view.setUint32(4, 3, true)
  view.setBigUint64(8, 7n, true)
  data.set([1, 1, 1, 0], 16)
  view.setFloat32(20, 200, true)
  view.setFloat32(24, 200, true)
  view.setUint32(32, 1, true)
  view.setUint32(36, 1, true)
  view.setUint32(40, 1, true)
  view.setUint32(44, 1, true)
  view.setUint32(48, 1, true)
  view.setUint32(52, 1, true)
  view.setUint32(60, 96, true)

  data.set([0x50, 0x4d, 0x4f, 0x56], 64)
  view.setUint32(68, 1, true)
  data[79] = 1
  view.setBigUint64(80, 0xffff_ffff_ffff_ffffn, true)
  ;[1, 2, 3, 4, 5, 6, 0, 0, 68].forEach((value, index) => {
    view.setFloat32(88 + index * 4, value, true)
  })
  view.setFloat32(144, 1, true)
  view.setFloat32(156, 1, true)

  let at = 160
  data.set([1, 0, 0, 0], at)
  view.setUint16(at + 4, 3, true)
  view.setUint16(at + 6, 20, true)
  view.setUint16(at + 8, 4, true)
  view.setUint16(at + 10, 20, true)
  view.setBigUint64(at + 12, 20n, true)
  at += 32

  view.setUint32(at, 9, true)
  data.set([1, 1, 1, 0], at + 4)
  view.setUint32(at + 8, 1, true)
  view.setUint32(at + 12, 1, true)
  ;[10, 11, 12, 100, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0.1].forEach((value, index) => {
    view.setFloat32(at + 16 + index * 4, value, true)
  })
  at += 84

  data.set([1, 1, 1, 0], at)
  view.setUint32(at + 4, 9, true)
  view.setUint32(at + 8, 1, true)
  view.setUint32(at + 12, 1, true)
  view.setBigUint64(at + 16, 7n, true)
  ;[10, 11, 12, 0, 0, 0, 1, 0, 0, 0].forEach((value, index) => {
    view.setFloat32(at + 24 + index * 4, value, true)
  })
  at += 64

  view.setUint32(at, 67, true)
  view.setUint32(at + 4, 26, true)
  ;[1, 2, 3, 0, 90, 0].forEach((value, index) => view.setFloat32(at + 8 + index * 4, value, true))
  at += 32

  view.setBigUint64(at, 99n, true)
  data.set([2, 1, 0, 0], at + 8)
  view.setUint32(at + 12, 67, true)
  view.setUint32(at + 16, 0xffff_ffff, true)
  view.setUint32(at + 20, 9, true)
  data.set(new TextEncoder().encode("OnTrigger"), at + 24)
  at += 33

  data.set([5, 1, 0, 0], at)
  view.setUint32(at + 4, 85, true)
  ;[200, 4, 20, 0].forEach((value, index) => view.setFloat32(at + 12 + index * 4, value, true))
  return bytes
}

class MemoryCache implements DerivedObjectCache {
  async read(): Promise<Uint8Array | undefined> { return undefined }
  async write(): Promise<void> {}
  async remove(): Promise<void> {}
  close(): void {}
}

class CourseWorker implements WorkerLike {
  configuredBytes = 0
  #message?: (event: MessageEvent<WorkerResponse>) => void
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  addEventListener(type: "message" | "error", listener: unknown): void {
    if (type === "message") this.#message = listener as (event: MessageEvent<WorkerResponse>) => void
  }
  removeEventListener(): void {}
  terminate(): void {}
  postMessage(request: WorkerRequest): void {
    let response: WorkerResponse
    if (request.kind === "configure-course") {
      this.configuredBytes = request.definition.byteLength
      response = { id: request.id, kind: "course-configured", generation: request.generation }
    } else if (request.kind === "advance") {
      response = { id: request.id, kind: "snapshot", generation: request.generation, snapshot: snapshot() }
    } else if (request.kind === "shutdown") response = { id: request.id, kind: "shutdown" }
    else response = { id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 }
    queueMicrotask(() => this.#message?.({ data: response } as MessageEvent<WorkerResponse>))
  }
}

describe("TF2 playable runtime Stage 2 contract", () => {
  test("encodes complete commands and atomically decodes runtime facts", () => {
    const command = encodeCommand({
      forward: 240,
      side: -20,
      up: 100,
      yawDegrees: 90,
      pitchDegrees: -30,
      jump: true,
      crouch: true,
      speedButton: true,
      fire: true,
      detonate: true,
      reload: true,
      reset: true,
      respawn: true,
      selectClass: 2,
      selectWeapon: 3,
      selectTeam: 2,
      modeRequest: 1,
      activateEntity: 213,
    })
    const commandView = new DataView(command)
    expect(new TextDecoder().decode(command.slice(0, 4))).toBe("PCMD")
    expect(command.byteLength).toBe(40)
    expect(commandView.getUint32(28, true)).toBe(0xff)
    expect(commandView.getUint32(32, true)).toBe(0x0202_0302)
    expect(commandView.getUint32(36, true)).toBe(213)

    const value = decodeSnapshot(snapshot())
    expect(value.movement).toMatchObject({
      grounded: true,
      position: [1, 2, 3],
      velocity: [4, 5, 6],
      viewOffset: [0, 0, 68],
    })
    expect(value.loadout[0]).toMatchObject({ clip: 3, reserve: 20 })
    expect(value.projectiles[0]).toEqual({
      identity: 9,
      kind: 1,
      team: 1,
      ownerIdentity: 1,
      launcherIdentity: 1,
      state: 1,
      position: [10, 11, 12],
      velocity: [100, 0, 0],
      orientation: [0, 0, 0, 1],
      angularVelocity: [0, 0, 0],
      contactNormal: null,
      ageSeconds: expect.closeTo(0.1),
    })
    expect(value.projectileEvents[0]?.type).toBe("fire")
    expect(value.entityTransforms[0]).toEqual({
      identity: 67,
      model: 26,
      position: [1, 2, 3],
      angles: [0, 90, 0],
    })
    expect(value.entityEvents[0]).toMatchObject({ sequence: 99n, name: "OnTrigger" })
    expect(value.events[0]).toMatchObject({ kind: 5, subject: 85, values: [200, 4, 20, 0] })

    const malformed = snapshot()
    new DataView(malformed).setFloat32(192 + 52, 0, true)
    expect(() => decodeSnapshot(malformed)).toThrow(Tf2CodecError)
  })

  test("configures one explicit map-bound linear course before advancement", async () => {
    const definition = encodeJumpCourse(7n, "ab".repeat(32), [
      { identity: 1, triggerEntity: 10, kind: "start", index: 1 },
      { identity: 2, triggerEntity: 11, kind: "checkpoint", index: 1 },
      { identity: 3, triggerEntity: 12, kind: "end", index: 1 },
    ])
    expect(new TextDecoder().decode(definition.slice(0, 4))).toBe("PJMP")
    expect(definition.byteLength).toBe(100)
    const worker = new CourseWorker()
    const client = new Tf2WorkerClient(worker, new MemoryCache())
    await client.configureCourse(4, definition)
    expect(worker.configuredBytes).toBe(100)
    const value = await client.advance(4, encodeCommand({
      forward: 0,
      side: 0,
      yawDegrees: 0,
      pitchDegrees: 0,
      jump: false,
      crouch: false,
      fire: false,
      detonate: false,
    }), 1)
    expect(value.tick).toBe(7n)
    await client.shutdown()
  })
})
