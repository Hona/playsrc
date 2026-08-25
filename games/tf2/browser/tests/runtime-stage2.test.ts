import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { mergePublicationSnapshots, Tf2WorkerClient, type WorkerLike } from "../src/client"
import {
  decodeSnapshot,
  encodeCommand,
  encodeJumpCourse,
  Tf2CodecError,
} from "../src/codec"
import type { WorkerRequest, WorkerResponse } from "../src/protocol"

function snapshot(): ArrayBuffer {
  const bytes = new ArrayBuffer(969)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x53, 0x53, 0x4e])
  view.setUint32(4, 8, true)
  view.setBigUint64(8, 7n, true)
  data.set([1, 1, 1, 0], 16)
  view.setFloat32(20, 200, true)
  view.setFloat32(24, 200, true)
  data[28] = 1
  view.setUint32(52, 1, true)
  view.setUint32(56, 1, true)
  view.setUint32(60, 1, true)
  view.setUint32(64, 1, true)
  view.setUint32(68, 1, true)
  view.setUint32(72, 1, true)
  view.setUint32(76, 1, true)
  view.setUint32(84, 96, true)
  view.setUint32(88, 1, true)
  view.setUint32(124, 2, true)
  view.setUint32(144, 52, true)
  view.setUint32(148, 284, true)
  view.setUint32(152,52,true);view.setUint32(156,12,true)

  data.set([0x50, 0x4d, 0x4f, 0x56], 160)
  view.setUint32(164, 1, true)
  data[175] = 1
  view.setBigUint64(176, 0xffff_ffff_ffff_ffffn, true)
  ;[1, 2, 3, 4, 5, 6, 0, 0, 68].forEach((value, index) => {
    view.setFloat32(184 + index * 4, value, true)
  })
  view.setFloat32(240, 1, true)
  view.setFloat32(252, 1, true)

  let at = 256
  data.set([1, 0, 0, 0], at)
  view.setUint16(at + 4, 3, true)
  view.setUint16(at + 6, 20, true)
  view.setUint16(at + 8, 4, true)
  view.setUint16(at + 10, 20, true)
  view.setBigUint64(at + 12, 20n, true)
  view.setBigUint64(at + 20, 0xffff_ffff_ffff_ffffn, true)
  view.setBigUint64(at + 28, 0xffff_ffff_ffff_ffffn, true)
  view.setBigUint64(at + 36, 2n, true)
  at += 48

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
  at += 28
  view.setBigUint64(at, 7n, true)
  data.set([1, 2], at + 8)
  at += 16
  data.set([1, 1, 0, 0, 2, 1, 0, 0], at)
  at += 8
  data.set([0x50, 0x52, 0x4e, 0x47], at)
  view.setUint32(at + 4, 1, true)
  data.set([7, 7, 0, 0], at + 280)
  at += 284
  data.set([0x43, 0x53, 0x4e, 0x50], at)
  view.setUint32(at + 4, 2, true)
  data.fill(1,at+8,at+40)
  view.setBigUint64(at + 40, 7n, true)
  view.setUint32(at + 48, 0, true)
  at += 52
  data.set([0x50, 0x4d, 0x54, 0x4b], at)
  view.setUint32(at + 4, 1, true)
  at+=12;data.set([0x50,0x45,0x42,0x50],at);view.setUint32(at+4,1,true);view.setBigUint64(at+8,1n,true);view.setBigUint64(at+16,2n,true);view.setBigUint64(at+24,7n,true);view.setBigUint64(at+32,1n,true);view.setBigUint64(at+40,7n,true)
  return bytes
}
function simulationOutput(){const state=new Uint8Array(snapshot()),output=new ArrayBuffer(68+state.length*2),data=new Uint8Array(output),view=new DataView(output);data.set(new TextEncoder().encode("PSIM"));view.setUint32(4,1,true);view.setUint32(8,1,true);view.setBigUint64(16,1n,true);view.setBigUint64(24,1n,true);view.setBigUint64(32,1n,true);view.setUint32(40,1,true);view.setUint32(48,state.length,true);view.setUint32(52,1,true);data.set(state,56);const at=56+state.length;view.setBigUint64(at,1n,true);view.setUint32(at+8,state.length,true);data.set(state,at+12);return output}

class MemoryCache implements DerivedObjectCache {
  async read(): Promise<Uint8Array | undefined> { return undefined }
  async write(): Promise<string> { return "0".repeat(64) }
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
  postMessage(message: WorkerRequest, transfer: Transferable[] = []): void {
    const request = structuredClone(message, { transfer })
    let response: WorkerResponse
    if (request.kind === "configure-course") {
      this.configuredBytes = request.definition.byteLength
      response = { id: request.id, kind: "course-configured", generation: request.generation }
    } else if(request.kind==="observe"){
      response={id:request.id,kind:"simulation",generation:request.generation,output:simulationOutput()}
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
      physicsResults: [{
        projectile: 8,
        tick: 7n,
        position: [1, 2, 3],
        velocity: [4, 5, 6],
        orientation: [0, 0, 0, 1],
        angularVelocity: [7, 8, 9],
        motionEnabled: false,
        contact: { kind: 1, normal: [0, 0, 1] },
      }],
    })
    const commandView = new DataView(command)
    expect(new TextDecoder().decode(command.slice(0, 4))).toBe("PCMD")
    expect(commandView.getUint32(4, true)).toBe(4)
    expect(command.byteLength).toBe(128)
    expect(commandView.getUint32(28, true)).toBe(0xff)
    expect(commandView.getUint32(32, true)).toBe(0x0202_0302)
    expect(commandView.getUint32(36, true)).toBe(213)
    expect(commandView.getUint16(40, true)).toBe(1)
    expect(commandView.getUint16(42, true)).toBe(0)

    const source = snapshot()
    const value = decodeSnapshot(source)
    expect(value.collisionSnapshot.bytes.buffer).toBe(source)
    const enclosed = new Uint8Array(source.byteLength + 7)
    enclosed.set(new Uint8Array(source), 3)
    const offset = decodeSnapshot(enclosed.subarray(3, source.byteLength + 3))
    expect(offset.tick).toBe(value.tick)
    expect(offset.collisionSnapshot.bytes.buffer).toBe(enclosed.buffer)
    expect(value.movement).toMatchObject({
      grounded: true,
      position: [1, 2, 3],
      velocity: [4, 5, 6],
      viewOffset: [0, 0, 68],
    })
    expect(value.loadout[0]).toMatchObject({ clip: 3, reserve: 20 })
    expect(value.activities).toEqual([{ tick: 7n, weapon: 1, activity: 2 }])
    expect(value.lifecycle).toBe(1)
    expect(value.respawnTouchCount).toBe(1)
    expect(value.authorityBlockers.map((value) => value.code)).toEqual([1, 2])
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
    expect(value.projectileTimeline).toEqual([{
      tick: 7n,
      projectiles: value.projectiles,
      events: value.projectileEvents,
    }])
    expect(value.entityTransforms[0]).toEqual({
      identity: 67,
      model: 26,
      position: [1, 2, 3],
      angles: [0, 90, 0],
    })
    expect(value.entityEvents[0]).toMatchObject({ sequence: 99n, name: "OnTrigger" })
    expect(value.events[0]).toMatchObject({ kind: 5, subject: 85, values: [200, 4, 20, 0] })

    const malformed = snapshot()
    new DataView(malformed).setFloat32(344 + 12, 0, true)
    expect(() => decodeSnapshot(malformed)).toThrow(Tf2CodecError)
    const priorVersion = snapshot()
    new DataView(priorVersion).setUint32(4, 5, true)
    expect(() => decodeSnapshot(priorVersion)).toThrow(Tf2CodecError)
    const stuck = snapshot()
    const stuckData = new Uint8Array(stuck), stuckView = new DataView(stuck)
    stuckData.set([2, 1, 3, 1], 308)
    stuckView.setFloat32(380, 1, true)
    expect(decodeSnapshot(stuck).projectiles[0]).toMatchObject({ kind: 2, state: 3, velocity: [100, 0, 0], contactNormal: [0, 0, 1] })
  })

  test("retains transient projectile ticks across repeated publication merges", () => {
    const fired = decodeSnapshot(snapshot())
    const fire = fired.projectileEvents[0]!
    const impact = Object.freeze({
      ...fire,
      type: "impact" as const,
      tick: 8n,
      contactNormal: Object.freeze([0, 0, 1]) as readonly [number, number, number],
    })
    const explode = Object.freeze({ ...impact, type: "explode" as const })
    const terminalEvents = Object.freeze([impact, explode])
    const terminal = Object.freeze({
      ...fired,
      tick: 8n,
      projectiles: Object.freeze([]),
      projectileEvents: terminalEvents,
      projectileTimeline: Object.freeze([
        Object.freeze({ tick: 8n, projectiles: Object.freeze([]), events: terminalEvents }),
      ]),
    })
    const merged = mergePublicationSnapshots([fired, terminal])
    expect(merged.projectiles).toEqual([])
    expect(merged.projectileEvents.map((event) => event.type)).toEqual(["fire", "impact", "explode"])
    expect(merged.projectileTimeline.map((entry) => entry.tick)).toEqual([7n, 8n])

    const idle = Object.freeze({
      ...terminal,
      tick: 9n,
      projectileEvents: Object.freeze([]),
      projectileTimeline: Object.freeze([
        Object.freeze({ tick: 9n, projectiles: Object.freeze([]), events: Object.freeze([]) }),
      ]),
    })
    expect(mergePublicationSnapshots([merged, idle]).projectileTimeline.map((entry) => entry.tick))
      .toEqual([7n, 8n, 9n])
    expect(() => mergePublicationSnapshots([fired, fired])).toThrow()
  })

  test("retains one explicit externally supplied course seam without application inference", async () => {
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
    const command = encodeCommand({
      forward: 0,
      side: 0,
      yawDegrees: 0,
      pitchDegrees: 0,
      jump: false,
      crouch: false,
      fire: false,
      detonate: false,
    })
    const publication = (await client.observe(4, 1, command))[0]!
    expect(command.byteLength).toBe(0)
    expect(publication.snapshot.tick).toBe(7n)
    expect(publication.snapshotBytes.buffer).toBe(publication.eventBatches[0]!.bytes.buffer)
    expect(publication.eventBatches[0]!.snapshot.collisionSnapshot.bytes.buffer).toBe(publication.snapshotBytes.buffer)
    expect(publication.snapshot).toBe(publication.eventBatches[0]!.snapshot)
    await client.shutdown()
  })
})
