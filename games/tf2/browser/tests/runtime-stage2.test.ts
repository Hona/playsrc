import { describe, expect, test } from "bun:test"
import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { mergePublicationSnapshots, SimulationSnapshotStream, Tf2WorkerClient, type WorkerLike } from "../src/client"
import {
  decodeSnapshot,
  encodeCommand,
  encodeJumpCourse,
  mapDerivedKey,
  Tf2CodecError,
} from "../src/codec"
import type { WorkerRequest, WorkerResponse } from "../src/protocol"
import { tf2Audio } from "../src/presentation"
import { configuredEquipmentSounds } from "../src/equipment/audio.generated"

function snapshot(): ArrayBuffer {
  const bytes = new ArrayBuffer(1293)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x53, 0x53, 0x4e])
  view.setUint32(4, 29, true)
  view.setFloat32(bytes.byteLength - 4, 1, true)
  view.setBigUint64(8, 7n, true)
  data.set([3, 2, 1, 0], 16)
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
  view.setUint32(148, 364, true)
  view.setUint32(152,60,true);view.setUint32(156,12,true)

  view.setUint32(160, 0x101, true)
  data.set([0x50, 0x4d, 0x4f, 0x56], 180)
  view.setUint32(184, 1, true)
  data[195] = 1
  view.setBigUint64(196, 0xffff_ffff_ffff_ffffn, true)
  ;[1, 2, 3, 4, 5, 6, 0, 0, 68].forEach((value, index) => {
    view.setFloat32(204 + index * 4, value, true)
  })
  view.setFloat32(260, 1, true)
  view.setFloat32(272, 1, true)

  let at = 276
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
  data.set([1, 2, 1 | (1 << 3), 0], at + 4)
  view.setUint32(at + 8, 1, true)
  view.setUint32(at + 12, 1, true)
  ;[10, 11, 12, 100, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0.1].forEach((value, index) => {
    view.setFloat32(at + 16 + index * 4, value, true)
  })
  at += 84

  data.set([1 | (1 << 3), 1, 2, 2], at)
  view.setUint32(at + 4, 9, true)
  view.setUint32(at + 8, 1, true)
  view.setUint32(at + 12, 1, true)
  view.setBigUint64(at + 16, 7n, true)
  ;[10, 11, 12, 0, 0, 0, 1, 0, 0, 0, 1, 2, 71, 0, 0, 0, 1].forEach((value, index) => {
    view.setFloat32(at + 24 + index * 4, value, true)
  })
  at += 92

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
  view.setUint32(at + 4, 3, true)
  data.set([7, 7, 3, 31, 3, 3, 7, 31, 15, 3, 7, 7, 3, 0, 0, 0], at + 280)
  at += 364
  data.set([0x43, 0x53, 0x4e, 0x50], at)
  view.setUint32(at + 4, 3, true)
  data.fill(1,at+8,at+40)
  view.setBigUint64(at + 40, 7n, true)
  view.setUint32(at + 48, 0, true)
  at += 52
  data.set([0x50, 0x4d, 0x54, 0x4b], at)
  view.setUint32(at + 4, 1, true)
  at+=12;data.set([0x50,0x45,0x42,0x50],at);view.setUint32(at+4,3,true);view.setBigUint64(at+8,1n,true);view.setBigUint64(at+16,2n,true);view.setBigUint64(at+24,7n,true);view.setBigUint64(at+32,1n,true);view.setBigUint64(at+40,7n,true)
  at += 60
  view.setUint32(at, 0, true)
  at += 4
  data.set(new TextEncoder().encode("PCPN"), at)
  view.setUint32(at + 4, 12, true)
  at += 12
  data.set(new TextEncoder().encode("PCTF"), at)
  view.setUint32(at + 4, 1, true)
  at += 12
  at += 8
  const scoreboard = at
  data.set([1, 0, 1, 0], scoreboard + 8)
  view.setUint32(scoreboard + 12, 1, true)
  view.setUint32(scoreboard + 40, 3, true)
  data.set([3, 2, 1, 0], scoreboard + 16)
  data[scoreboard + 44] = 7
  data.set(new TextEncoder().encode("unnamed"), scoreboard + 45)
  at += 52
  at += 4
  data.set(new TextEncoder().encode("PGRL"), at)
  view.setUint32(at + 4, 4, true)
  data[at + 8] = 4
  view.setFloat32(at + 20, -1, true)
  view.setUint32(at + 24, 0xffff_ffff, true)
  view.setFloat32(at + 28, -1, true)
  at += 48
  view.setUint32(at + 4, 0xffff_ffff, true)
  return bytes
}

test("mini-round completion is authoritative and does not change the round state or framing", () => {
  const bytes = snapshot(), data = new Uint8Array(bytes), at = Buffer.from(bytes).indexOf("PGRL")
  data[at + 8] = 0x85; data[at + 10] = 3; data[at + 11] = 1
  expect(decodeSnapshot(bytes).round).toMatchObject({ state: 5, winningTeam: 3, fullRound: false })
  data[at + 8] = 5
  expect(decodeSnapshot(bytes).round?.fullRound).toBe(true)
  data[at + 8] = 0x45
  expect(() => decodeSnapshot(bytes)).toThrow("Round rules section is invalid")
})

test("persistent authored view corrections survive snapshot decode and reject nonfinite angles", () => {
  const bytes = snapshot(), view = new DataView(bytes)
  view.setFloat32(bytes.byteLength - 24, -5, true)
  view.setFloat32(bytes.byteLength - 20, -150, true)
  expect(decodeSnapshot(bytes).viewAngleOffset).toEqual([-5, -150, 0])
  view.setFloat32(bytes.byteLength - 16, Number.NaN, true)
  expect(() => decodeSnapshot(bytes)).toThrow("view angle correction is invalid")
})

test("control point and overtime events admit every declared wave variant", () => {
  const source = new Uint8Array(snapshot())
  const at = Buffer.from(source).indexOf("CSNP")
  expect(at).toBeGreaterThan(0)
  for (const [definition, waves] of [[85, 4], [86, 2], [89, 3], [90, 2], [91, 4], [98, 2]]) {
    for (let wave = 0; wave <= waves!; wave++) {
      const bytes = new Uint8Array(source.length + 52)
      bytes.set(source.subarray(0, at)); bytes.set(source.subarray(at), at + 52)
      const view = new DataView(bytes.buffer)
      view.setUint32(132, 1, true)
      view.setBigUint64(at, 7n, true)
      bytes[at + 10] = 1; bytes[at + 11] = definition!; bytes[at + 12] = 1; bytes[at + 14] = wave
      view.setUint32(at + 16, 1, true); view.setUint32(at + 20, 0xffff_ffff, true)
      for (const offset of [36, 40, 44]) view.setFloat32(at + offset, 0.5, true)
      if (wave < waves!) expect(decodeSnapshot(bytes.buffer).audioEvents[0]?.samples.wave).toBe(wave)
      else expect(() => decodeSnapshot(bytes.buffer)).toThrow("audio event record is invalid")
    }
  }
})

test("death event wire retains exact uint32 damage bits, killing names and repeated occurrences before later sections", () => {
  const base = new Uint8Array(snapshot()), name = new TextEncoder().encode("tf_projectile_rocket")
  const record = new Uint8Array(28 + name.length), event = new DataView(record.buffer)
  record.set([18, 1]); event.setUint16(2, name.length, true)
  event.setUint32(4, 2, true); event.setUint32(8, 1, true)
  event.setUint32(12, 3, true); event.setUint32(16, 0x8010_0040, true)
  event.setUint32(20, 1, true); record.set(name, 28)
  const bytes = new Uint8Array(base.length - 28 + record.length * 2)
  bytes.set(base.subarray(0, 565)); bytes.set(record, 565); bytes.set(record, 565 + record.length)
  bytes.set(base.subarray(593), 565 + record.length * 2)
  new DataView(bytes.buffer).setUint32(76, 2, true)
  const decoded = decodeSnapshot(bytes.buffer)
  expect(decoded.events).toHaveLength(2)
  expect(decoded.events[0]).toEqual({ kind: 18, detail: 1, subject: 2, auxiliary: 1,
    values: [3, 0x8010_0040, 1, 0], killingWeapon: "tf_projectile_rocket" })
  expect(decoded.events[1]).toEqual(decoded.events[0])
  expect(decoded.scoreboard.players[0]!.name).toBe("unnamed")
  expect(decoded.scoreboard.players[0]!.assists).toBe(3)
  const invalid = bytes.slice(); new DataView(invalid.buffer).setUint16(567, 0, true)
  expect(() => decodeSnapshot(invalid.buffer)).toThrow("gameplay event record is invalid")
})
function simulationOutput(){const state=new Uint8Array(snapshot()),output=new ArrayBuffer(80+state.length),data=new Uint8Array(output),view=new DataView(output);data.set(new TextEncoder().encode("PSIM"));view.setUint32(4,3,true);view.setUint32(8,1,true);view.setBigUint64(16,1n,true);view.setBigUint64(24,1n,true);view.setBigUint64(32,1n,true);view.setUint32(40,1,true);view.setUint32(48,state.length,true);view.setUint32(52,1,true);const at=56;view.setBigUint64(at,1n,true);view.setUint32(at+8,state.length,true);view.setUint32(at+12,state.length,true);data.set(state,at+24);return output}

test("round codec keeps two independent KOTH timer records and rejects malformed clocks", () => {
  const base = new Uint8Array(snapshot())
  const at = base.findIndex((_, index) => base[index] === 80 && base[index + 1] === 71 && base[index + 2] === 82 && base[index + 3] === 76)
  expect(at).toBeGreaterThan(0)
  const bytes = new Uint8Array(base.length + 60)
  bytes.set(base.subarray(0, at + 48))
  bytes.set(base.subarray(at + 48), at + 108)
  bytes[at + 9] = 128
  const view = new DataView(bytes.buffer)
  view.setUint32(at + 44, 1, true)
  bytes[at + 96] = 16
  view.setUint32(at + 100, 100, true)
  view.setInt32(at + 104, -100_000, true)
  for (let team = 0; team < 2; team++) {
    const timer = at + 48 + team * 24
    view.setUint32(timer, 100 + team, true)
    view.setFloat32(timer + 4, team === 0 ? 12.5 : 0, true)
    view.setInt32(timer + 8, 180, true)
    view.setUint32(timer + 20, team === 0 ? 2 : 3, true)
  }
  expect(decodeSnapshot(bytes.buffer).round.kothTimers?.map(timer => [timer.identity, timer.remaining, timer.paused])).toEqual([[100, 12.5, false], [101, 0, true]])
  expect(decodeSnapshot(bytes.buffer).round.events[0]).toMatchObject({ kind: 16, identity: 100, value: -100_000 })
  view.setFloat32(at + 52, Number.NaN, true)
  expect(() => decodeSnapshot(bytes.buffer)).toThrow("KOTH timer is invalid")
})
test("critical draws preserve separate authority and predicted-presentation records", () => {
  const source = new Uint8Array(snapshot()), at = 981
  const data = new Uint8Array(source.length + 32), view = new DataView(data.buffer)
  data.set(source.subarray(0, at)); data.set(source.subarray(at), at + 32)
  view.setUint32(128, 2, true)
  for (let index = 0; index < 2; index++) {
    const record = at + index * 16
    data.set([index + 1, 14, 0, 0], record)
    view.setInt32(record + 4, 12345, true)
    data[record + 8] = 2
    view.setInt32(record + 12, 2345, true)
  }
  expect(decodeSnapshot(data).randomDraws.map(draw => [draw.context, draw.decision, draw.result]))
    .toEqual([[1, 14, {kind: "integer", value: 2345}], [2, 14, {kind: "integer", value: 2345}]])
  data[at + 16 + 1] = 5
  expect(() => decodeSnapshot(data)).toThrow("random draw record is invalid")
})

test("player weapon counters retain full native heads and reject invalid revenge counts", () => {
  const bytes = snapshot(), view = new DataView(bytes)
  view.setInt32(bytes.byteLength - 12, 800, true)
  view.setInt32(bytes.byteLength - 8, 35, true)
  expect(decodeSnapshot(bytes)).toMatchObject({ decapitations: 800, revengeCrits: 35 })
  view.setInt32(bytes.byteLength - 8, 36, true)
  expect(() => decodeSnapshot(bytes)).toThrow("player weapon counters are invalid")
})

test("native crosshair scale follows counters and rejects non-finite values", () => {
  const bytes = snapshot(), view = new DataView(bytes)
  view.setFloat32(bytes.byteLength - 4, 1.625, true)
  expect(decodeSnapshot(bytes).weaponCrosshairScale).toBe(1.625)
  view.setFloat32(bytes.byteLength - 4, NaN, true)
  expect(() => decodeSnapshot(bytes)).toThrow("weapon crosshair scale is invalid")
})

test("projectile wave cycles do not overwrite main objective or configured sound masks", () => {
  const bytes = new Uint8Array(snapshot()), view = new DataView(bytes.buffer), random = 617
  bytes[random + 293] = 15
  view.setUint16(random + 294, 0x1fff, true)
  bytes.set([255, 255, 7, 0], random + 360)
  expect(decodeSnapshot(bytes).randomState).toMatchObject({ overtimeAvailable: 15, controlPointAvailable: 0x1fff,
    projectileUnlockAvailable: [7, 7, 7, 7, 15, 7] })
  bytes[random + 363] = 1
  expect(() => decodeSnapshot(bytes)).toThrow("TF2 sound selection state is invalid")
})

test("flare radius damage preserves its native kind and rejects nonexplosive syringe requests", () => {
  const source = new Uint8Array(snapshot()), at = 609
  const bytes = new Uint8Array(source.length + 36), view = new DataView(bytes.buffer)
  bytes.set(source.subarray(0, at)); bytes.set(source.subarray(at), at + 36)
  view.setUint32(104, 1, true); view.setUint32(at, 7, true); bytes[at + 4] = 4
  ;[1, 2, 3, 30, 110, 100].forEach((value, index) => view.setFloat32(at + 8 + index * 4, value, true))
  view.setUint32(at + 32, 0xffff_ffff, true)
  expect(decodeSnapshot(bytes).radiusDamageRequests).toEqual([{ projectile: 7, kind: 4, source: [1, 2, 3], baseDamage: 30, radius: 110, selfRadius: 100, directTarget: null }])
  for (const kind of [0, 3, 5]) { bytes[at + 4] = kind; expect(() => decodeSnapshot(bytes)).toThrow("radius damage request record is invalid") }
})

test("studio occurrence revision bytes retain closed, moving, blocked, reversed and restored transforms", () => {
  const source = new Uint8Array(snapshot())
  const insert = 1101
  const states = [320, 360, 444, 400, 400, 420, 320].map(z => {
    const bytes = new Uint8Array(source.length + 32)
    bytes.set(source.subarray(0, insert))
    bytes.set(source.subarray(insert), insert + 32)
    const view = new DataView(bytes.buffer)
    view.setUint32(152, 92, true)
    view.setUint32(insert - 4, 1, true)
    view.setUint32(insert, 784, true)
    ;[886, 1440, z, 0, 90, 0].forEach((value, i) => view.setFloat32(insert + 4 + i * 4, value, true))
    view.setUint32(insert + 28, 1, true)
    return bytes
  })
  for (const [index, bytes] of states.entries()) {
    const state = decodeSnapshot(bytes).entityPresentation
    expect(state.studioModels).toEqual([{ sourceIndex: 784, worldPosition: [886, 1440, [320, 360, 444, 400, 400, 420, 320][index]], worldAngles: [0, 90, 0], draw: true, skin: 0 }])
    expect(state.collisionRevision).toBe(7n)
    expect(state.studioAnimations).toEqual([])
  }
  const malformed = states[0]!.slice()
  const skin = states[0]!.slice()
  new DataView(skin.buffer).setUint32(insert + 28, 5, true)
  expect(decodeSnapshot(skin).entityPresentation.studioModels[0]).toMatchObject({ draw: true, skin: 2 })
  new DataView(malformed.buffer).setFloat32(insert + 4, NaN, true)
  expect(() => decodeSnapshot(malformed)).toThrow("Studio presentation record is invalid")
  const stream = new SimulationSnapshotStream()
  const first = states[0]!, next = states[1]!.slice(), unchanged = next.slice()
  new DataView(next.buffer).setBigUint64(8, 8n, true)
  new DataView(unchanged.buffer).setBigUint64(8, 9n, true)
  const a = stream.decode(snapshotPacket(1n, [first]))[0]!.snapshot.entityPresentation
  const b = stream.decode(snapshotPacket(2n, [next], first))[0]!.snapshot.entityPresentation
  const c = stream.decode(snapshotPacket(3n, [unchanged], next))[0]!.snapshot.entityPresentation
  expect(a.studioModels[0]!.worldPosition[2]).toBe(320)
  expect(b.studioModels[0]!.worldPosition[2]).toBe(360)
  expect(b.studioModels[0]).not.toBe(a.studioModels[0])
  expect(c.studioModels).toBe(b.studioModels)
  const animated = new Uint8Array(first.length + 40), animationOffset = insert + 36
  animated.set(first.subarray(0, animationOffset))
  animated.set(first.subarray(animationOffset), animationOffset + 40)
  const view = new DataView(animated.buffer)
  view.setUint32(152, 132, true)
  view.setUint32(animationOffset - 4, 1, true)
  view.setUint32(animationOffset, 784, true)
  view.setFloat32(animationOffset + 4, 0.15, true)
  view.setUint32(animationOffset + 8, 4, true)
  ;[-128, -4, -64, 128, 0, 192].forEach((value, axis) => view.setFloat32(animationOffset + 12 + axis * 4, value, true))
  animated.set(new TextEncoder().encode("open"), animationOffset + 36)
  expect(decodeSnapshot(animated).entityPresentation.studioAnimations[0]).toMatchObject({ sourceIndex: 784, sequence: "open", bounds: [[-128, -4, -64], [128, 0, 192]] })
  view.setFloat32(animationOffset + 24, -129, true)
  expect(() => decodeSnapshot(animated)).toThrow("Studio animation is invalid")
})

// Large canonical records, not a model of game rules. Every optimized result is
// compared with the unchanged full snapshot decoder, including ordered events.
function rosterSnapshot(tick: bigint, roster = 31, brushes = 512): Uint8Array {
  const original = new Uint8Array(snapshot())
  const objective = original.length - 184, brushHeader = objective - 64
  const insert = brushes * 128, botBytes = roster * 128, names = Array.from({ length: roster }, (_, i) => new TextEncoder().encode(`bot-${i}`))
  const scoreboardBytes = names.reduce((sum, name) => sum + 33 + name.length, 0)
  const bytes = new Uint8Array(original.length + insert + botBytes + scoreboardBytes + roster * 28)
  bytes.set(original.subarray(0, brushHeader + 52))
  bytes.set(original.subarray(brushHeader + 52, objective), brushHeader + 52 + insert)
  bytes.set(original.subarray(objective, objective + 84), objective + insert + botBytes)
  bytes.set(original.subarray(objective + 84, original.length - 24), objective + insert + botBytes + 84 + scoreboardBytes)
  bytes.set(original.subarray(original.length - 24), bytes.length - 24)
  const view = new DataView(bytes.buffer)
  view.setBigUint64(8, tick, true)
  view.setBigUint64(424, tick, true) // projectile event
  view.setBigUint64(532, tick, true) // entity event sequence
  view.setBigUint64(593, tick, true) // weapon activity
  view.setUint32(152, 60 + insert, true)
  view.setBigUint64(brushHeader + 24, tick, true)
  view.setUint32(brushHeader + 48, brushes, true)
  for (let i = 0; i < brushes; i++) {
    const at = brushHeader + 52 + i * 128
    view.setUint32(at + 8, i + 1, true); view.setUint32(at + 12, i + 1, true)
    view.setFloat32(at + 40, i * 8, true); bytes[at + 67] = 1
  }
  view.setUint32(objective + insert - 4, roster, true)
  let score = objective + insert + botBytes + 32
  bytes[score + 9] = roster; bytes[score + 10] = roster + 1
  score += 52
  for (let i = 0; i < roster; i++) {
    const at = objective + insert + i * 128
    view.setUint32(at, i + 2, true); bytes.set([3, 3, 1, 1, 1], at + 4)
    view.setInt32(at + 12, 200, true); view.setInt32(at + 16, 200, true)
    bytes[at + 64] = 1; bytes[at + 67] = 1
    view.setUint16(at + 68, 4, true); view.setUint16(at + 70, 20, true)
    view.setUint16(at + 72, 4, true); view.setUint16(at + 74, 20, true)
    view.setBigUint64(at + 96, 0xffff_ffff_ffff_ffffn, true)
    view.setBigUint64(at + 104, 0xffff_ffff_ffff_ffffn, true)
    view.setUint32(score, i + 2, true); bytes.set([3, 3, 1, 1], score + 4)
    bytes[score + 32] = names[i]!.length; bytes.set(names[i]!, score + 33)
    score += 33 + names[i]!.length
    view.setFloat32(bytes.length - 24 - roster * 28 + i * 28 + 24, 88, true)
  }
  return bytes
}

function snapshotPacket(firstTick: bigint, states: readonly Uint8Array[], previous?: Uint8Array): ArrayBuffer {
  const records: Uint8Array[] = []
  for (let i = 0; i < states.length; i++) {
    const state = states[i]!, changes: number[] = []
    if (previous?.length === state.length) {
      // Independent, deliberately simple test encoder: one replacement per byte.
      for (let at = 0; at < state.length; at++) if (previous[at] !== state[at]) {
        changes.push(at & 255, at >>> 8 & 255, at >>> 16 & 255, at >>> 24, 1, 0, 0, 0, state[at]!)
      }
    }
    const delta = previous?.length === state.length && changes.length < state.length
    const wire = delta ? new Uint8Array(changes) : state
    const record = new Uint8Array(24 + wire.length), view = new DataView(record.buffer)
    view.setBigUint64(0, firstTick + BigInt(i), true)
    view.setUint32(8, state.length, true); view.setUint32(12, wire.length, true)
    view.setBigUint64(16, delta ? firstTick + BigInt(i) - 1n : 0n, true)
    record.set(wire, 24); records.push(record); previous = state
  }
  const bytes = new Uint8Array(56 + records.reduce((sum, record) => sum + record.length, 0)), view = new DataView(bytes.buffer)
  bytes.set(new TextEncoder().encode("PSIM")); view.setUint32(4, 3, true); view.setUint32(8, 1, true)
  view.setBigUint64(16, firstTick, true); view.setBigUint64(24, firstTick, true)
  view.setBigUint64(32, firstTick + BigInt(states.length) - 1n, true)
  view.setUint32(40, states.length, true); view.setFloat32(44, 0.25, true)
  view.setUint32(48, states.at(-1)!.length, true); view.setUint32(52, states.length, true)
  let at = 56
  for (const record of records) { bytes.set(record, at); at += record.length }
  return bytes.buffer
}

class MemoryCache implements DerivedObjectCache {
  async read(): Promise<undefined> { return undefined }
  async write(): Promise<string> { return "0".repeat(64) }
  async remove(): Promise<void> {}
  close(): void {}
}

class CourseWorker implements WorkerLike {
  configuredBytes = 0
  replayAttack?: Extract<WorkerResponse, { kind: "simulation" }>["replayAttack"]
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
      response={id:request.id,kind:"simulation",generation:request.generation,output:simulationOutput(),replayAttack:this.replayAttack}
    } else if (request.kind === "shutdown") response = { id: request.id, kind: "shutdown" }
    else response = { id: request.id, kind: "failure", code: "MalformedRequest", detail: 0 }
    queueMicrotask(() => this.#message?.({ data: response } as MessageEvent<WorkerResponse>))
  }
}

describe("TF2 canonical gameplay command and snapshot contract", () => {
  test("remember-weapon preferences share the command carrying respawn and lastinv selections", () => {
    for (const rememberActive of [false, true]) for (const rememberLast of [false, true]) {
      const bytes = encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: false, detonate: false,
        respawn: true, selectLastWeapon: true, weaponPreferences: { rememberActive, rememberLast } })
      const view = new DataView(bytes)
      expect(view.getUint8(57)).toBe(128 | Number(rememberActive) | Number(rememberLast) << 1)
      expect(view.getUint8(33)).toBe(255)
    }
  })
  test("accepts every authored KOTH and capture announcer wave ordinal, rejecting the first out-of-range wave", () => {
    for (const [definition, waveCount] of [[85, 4], [86, 2], [89, 3], [90, 2], [91, 4], [98, 2], [103, 1], [104, 1], [105, 1], [106, 1], [107, 1], [108, 1]] as const) {
      const source = rosterSnapshot(1n, 0, 0), at = 981
      const bytes = new Uint8Array(source.length + 52), view = new DataView(bytes.buffer)
      bytes.set(source.subarray(0, at)); bytes.set(source.subarray(at), at + 52)
      view.setUint32(132, 1, true)
      view.setBigUint64(at, 1n, true)
      bytes[at + 10] = 4; bytes[at + 11] = definition; bytes[at + 12] = 1
      view.setUint32(at + 16, 1, true); view.setUint32(at + 20, 0xffff_ffff, true)
      for (let wave = 0; wave < waveCount; wave++) {
        bytes[at + 14] = wave
        expect(decodeSnapshot(bytes).audioEvents[0]).toMatchObject({ definition, samples: { wave } })
      }
      bytes[at + 14] = waveCount
      expect(() => decodeSnapshot(bytes)).toThrow("audio event record is invalid")
    }
  })
  test("damage events preserve full versus mini critical kinds and reject untyped values", () => {
    for (const crit of [0, 1, 2, 3, 1.5]) {
      const bytes = new Uint8Array(snapshot()), view = new DataView(bytes.buffer)
      const random = bytes.findIndex((_, index) => bytes[index] === 80 && bytes[index + 1] === 82 && bytes[index + 2] === 78 && bytes[index + 3] === 71)
      const event = random - 24 - 28
      bytes[event] = 17
      view.setFloat32(event + 20, crit, true)
      if (crit <= 2 && Number.isInteger(crit)) expect(decodeSnapshot(bytes).events[0]!.values[2]).toBe(crit)
      else expect(() => decodeSnapshot(bytes)).toThrow("damage critical kind")
    }
  })

  test("configured sound identities and bounded cycle masks survive the native wire", () => {
    const source = new Uint8Array(snapshot()), at = 981
    const bytes = new Uint8Array(source.length + 52), view = new DataView(bytes.buffer)
    bytes.set(source.subarray(0, at)); bytes.set(source.subarray(at), at + 52)
    view.setUint32(132, 1, true)
    view.setBigUint64(at, 7n, true)
    bytes.set([1, 160, 1, 1, 0, 0], at + 10)
    view.setUint32(at + 16, 1, true); view.setUint32(at + 20, 1, true)
    expect(tf2Audio(decodeSnapshot(bytes))[0]!.definition).toBe(configuredEquipmentSounds[0]!)
    const badIdentity = bytes.slice(); badIdentity[at + 11] = 160 + configuredEquipmentSounds.length
    expect(() => decodeSnapshot(badIdentity)).toThrow(Tf2CodecError)
    const random = bytes.findIndex((_, index) => bytes[index] === 80 && bytes[index + 1] === 82 && bytes[index + 2] === 78 && bytes[index + 3] === 71)
    const badMask = bytes.slice(); badMask[random + 359] = 255
    expect(() => decodeSnapshot(badMask)).toThrow("configured sound selection mask")
  })

  test("Worker snapshot deltas and coalescing retain ordered sound patch starts, destruction and re-press", () => {
    const frame = (tick: bigint, events: readonly (readonly [number, number, number])[]) => {
      const base = rosterSnapshot(tick, 0, 0), at = 981
      const bytes = new Uint8Array(base.length + events.length * 52), view = new DataView(bytes.buffer)
      bytes.set(base.subarray(0, at)); bytes.set(base.subarray(at), at + events.length * 52)
      view.setUint32(132, events.length, true)
      events.forEach(([definition, action, source], ordinal) => {
        const item = at + ordinal * 52
        view.setBigUint64(item, tick, true); view.setUint16(item + 8, ordinal, true)
        bytes.set([1, definition, 1, 1, 0, action], item + 10)
        view.setUint32(item + 16, source, true); view.setUint32(item + 20, source, true)
        view.setFloat32(item + 48, action > 1 ? 3.5 : 0, true)
      })
      return bytes
    }
    const first = frame(7n, [[37, 3, 1], [38, 2, 1], [37, 3, 2], [38, 2, 2]])
    const second = frame(8n, [[39, 0, 1], [38, 1, 1], [37, 1, 1], [37, 3, 1], [38, 2, 1]])
    const stream = new SimulationSnapshotStream()
    const publications = stream.decode(snapshotPacket(1n, [first, second]))
    const merged = mergePublicationSnapshots(publications.flatMap(publication => publication.eventBatches.map(batch => batch.snapshot)))
    const requests = tf2Audio(merged)
    expect(requests.map(request => request.action)).toEqual(["fade-out", "fade-in", "fade-out", "fade-in", "play", "stop", "stop", "fade-out", "fade-in"])
    expect(requests.map(request => request.source.identity)).toEqual([1, 1, 2, 2, 1, 1, 1, 1, 1])
    expect(requests[0]!.fadeSeconds).toBe(3.5)
    expect(new Set(requests.map(request => request.voiceIdentity)).size).toBe(9)
    const pitched = first.slice()
    pitched[981 + 11] = 115; pitched[981 + 15] = 4
    new DataView(pitched.buffer).setFloat32(981 + 48, 92, true)
    expect(tf2Audio(decodeSnapshot(pitched))[0]).toMatchObject({ definition: "Weapon_Minigun.FireCrit", action: "play", overrides: { pitch: 92 } })
    for (const invalid of [5, 255]) {
      const bad = first.slice(); bad[981 + 15] = invalid
      expect(() => decodeSnapshot(bad.buffer)).toThrow(Tf2CodecError)
    }
    const invalidDuration = first.slice()
    new DataView(invalidDuration.buffer).setFloat32(981 + 48, NaN, true)
    expect(() => decodeSnapshot(invalidDuration.buffer)).toThrow(Tf2CodecError)
  })

  test("keys derived maps to one authenticated resource root without rehashing shared gameplay sections", async () => {
    const bsp = "1".repeat(64)
    const compiler = "2".repeat(64)
    const root = "3".repeat(64)
    const first = await mapDerivedKey(bsp, 1, 2, compiler, root)
    expect(first).toMatch(/^[0-9a-f]{64}$/)
    expect(await mapDerivedKey(bsp, 1, 2, compiler, root)).toBe(first)
    expect(await mapDerivedKey(bsp, 1, 2, compiler, "4".repeat(64))).not.toBe(first)
    await expect(mapDerivedKey(bsp, 1, 2, compiler, "invalid")).rejects.toBeInstanceOf(Tf2CodecError)
    await expect(mapDerivedKey(bsp, 0, 2, compiler, root)).rejects.toBeInstanceOf(Tf2CodecError)
  })

  test("preserves all nine Source class selectors and rejects invalid class/team identities", () => {
    const base = {
      forward: 0,
      side: 0,
      yawDegrees: 0,
      pitchDegrees: 0,
      jump: false,
      crouch: false,
      fire: false,
      detonate: false,
    }
    for (let identity = 1; identity <= 9; identity += 1) {
      for (const team of [2, 3] as const) {
        const bytes = encodeCommand({ ...base, selectClass: identity as 1, selectTeam: team })
        expect(new DataView(bytes).getUint32(32, true)).toBe(identity | team << 16)
      }
    }
    expect(new DataView(encodeCommand({ ...base, selectClass: 12 })).getUint32(32, true)).toBe(12)
    expect(new DataView(encodeCommand({ ...base, selectLastWeapon: true })).getUint32(32, true)).toBe(255 << 8)
    expect(() => encodeCommand({ ...base, selectLastWeapon: true, selectWeapon: 43 })).toThrow("weapon selectors conflict")
    for (const identity of [0, 10, 11, 13, 1.5, Number.NaN]) {
      expect(() => encodeCommand({ ...base, selectClass: identity as 1 })).toThrow("class selector is invalid")
    }
    for (const team of [0, 1, 4]) {
      expect(() => encodeCommand({ ...base, selectTeam: team as 2 })).toThrow("team selector is invalid")
    }
    for (const weapon of [...Array.from({length:21},(_,index)=>index+1),40,41,42,43,44,45]) {
      expect(new DataView(encodeCommand({ ...base, selectWeapon: weapon as 1 })).getUint32(32, true)).toBe(weapon << 8)
    }
    for (const weapon of [40, 41, 42, 43, 44, 45] as const) {
      expect(new DataView(encodeCommand({ ...base, selectWeapon: weapon })).getUint32(32, true)).toBe(weapon << 8)
    }
    for (const weapon of [0, 22, 39, 46, 1.5, Number.NaN]) {
      expect(() => encodeCommand({ ...base, selectWeapon: weapon as 1 })).toThrow("weapon selector is invalid")
    }
  })

  test("keeps an authored Payload rigid-body constraint as an explicit map-specific blocker", () => {
    const ordinary = new Uint8Array(snapshot())
    const constrained = new Uint8Array(ordinary.byteLength + 4)
    const blockerEnd = 617
    constrained.set(ordinary.subarray(0, blockerEnd))
    constrained.set([3, 1, 0, 0], blockerEnd)
    constrained.set(ordinary.subarray(blockerEnd), blockerEnd + 4)
    new DataView(constrained.buffer).setUint32(124, 3, true)
    expect(decodeSnapshot(constrained).authorityBlockers).toEqual([
      { code: 1, classification: "Missing", detail: "TF2 sticky IVP solver unavailable: current body/contact transition" },
      { code: 2, classification: "Missing", detail: "Tempus core and configured Jump course contract unavailable" },
      { code: 3, classification: "Missing", detail: "Payload cart visible model requires its authored breakable constraint and VPhysics rigid-body authority" },
    ])
    expect(decodeSnapshot(ordinary).authorityBlockers.map((blocker) => blocker.code)).toEqual([1, 2])
  })

  test("decodes every canonical class identity and a genuinely unarmed class snapshot", () => {
    for (let identity = 1; identity <= 9; identity += 1) {
      const base = new Uint8Array(snapshot())
      const bytes = new Uint8Array(base.length + (identity === 8 ? 28 : 0))
      bytes.set(base)
      bytes[16] = identity
      if (identity === 8) {
        bytes[18] = 54
        bytes[276] = 54
        new DataView(bytes.buffer).setFloat32(320, 100, true)
        const view = new DataView(bytes.buffer), cloak = base.length - 28
        bytes.set(base.subarray(cloak), cloak + 28)
        view.setUint32(cloak - 4, 1, true)
        view.setUint32(cloak, 1, true)
        ;[0, 0, 0, 1, 0.5, 0.4].forEach((value, index) => view.setFloat32(cloak + 4 + index * 4, value, true))
        expect(decodeSnapshot(bytes).actorCloaks[0]!.identity).toBe(1)
      }
      expect(decodeSnapshot(bytes).class).toBe(identity)
    }
    for (const weapon of [15, 16, 17, 18, 19, 20, 21, 40, 41, 42] as const) {
      const bytes = new Uint8Array(snapshot())
      bytes[18] = weapon
      bytes[276] = weapon
      expect(decodeSnapshot(bytes).loadout[0]?.weapon).toBe(weapon)
    }
    const armed = new Uint8Array(snapshot())
    const unarmed = new Uint8Array(armed.length - 48)
    unarmed.set(armed.subarray(0, 276))
    unarmed.set(armed.subarray(324), 276)
    unarmed[16] = 1
    unarmed[18] = 0
    new DataView(unarmed.buffer).setUint32(56, 0, true)
    const decoded = decodeSnapshot(unarmed)
    expect(decoded.class).toBe(1)
    expect(decoded.weapon).toBeNull()
    expect(decoded.loadout).toEqual([])
    unarmed[18] = 1
    expect(() => decodeSnapshot(unarmed)).toThrow("active weapon does not match its loadout")
  })

  test("decodes unassigned and spectator lifecycles without inventing a combat team or weapon", () => {
    const armed = new Uint8Array(snapshot())
    const inactive = new Uint8Array(armed.length - 48)
    inactive.set(armed.subarray(0, 264))
    inactive.set(armed.subarray(312), 264)
    inactive[18] = 0
    new DataView(inactive.buffer).setUint32(56, 0, true)
    for (const [team, lifecycle] of [[0, 3], [1, 4]] as const) {
      inactive[17] = team
      inactive[28] = lifecycle
      const decoded = decodeSnapshot(inactive)
      expect(decoded.team).toBe(team)
      expect(decoded.lifecycle).toBe(lifecycle)
      expect(decoded.weapon).toBeNull()
    }
    inactive[17] = 1
    inactive[28] = 1
    expect(() => decodeSnapshot(inactive)).toThrow("snapshot selection is invalid")
  })

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
      selectClass: 4,
      selectWeapon: 3,
      selectTeam: 3,
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
    expect(commandView.getUint32(4, true)).toBe(9)
    expect(command.byteLength).toBe(164)
    expect(commandView.getUint32(28, true)).toBe(0xff)
    expect(commandView.getUint32(32, true)).toBe(0x0203_0304)
    expect(commandView.getUint32(36, true)).toBe(213)
    expect(commandView.getUint16(40, true)).toBe(1)
    expect(commandView.getUint16(42, true)).toBe(0)
    expect(commandView.getUint32(44, true)).toBe(0)
    expect(commandView.getUint32(48, true)).toBe(164)
    expect(commandView.getUint32(52, true)).toBe(0)
    const stopped = encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: false, detonate: false, nextbotStop: true })
    expect(new DataView(stopped).getUint32(28, true)).toBe(0)
    expect(new DataView(stopped).getUint16(42, true)).toBe(0x8000)
    const disguised = encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: false, detonate: false, nextbotStop: true, disguise: { class: 5, team: 3 } })
    expect(new DataView(disguised).getUint32(28, true)).toBe(5 << 9 | 3 << 13)
    expect(new DataView(disguised).getUint16(42, true)).toBe(0x8000)

    const source = snapshot()
    const value = decodeSnapshot(source)
    expect(value.collisionSnapshot.bytes.buffer).not.toBe(source)
    const enclosed = new Uint8Array(source.byteLength + 7)
    enclosed.set(new Uint8Array(source), 3)
    const offset = decodeSnapshot(enclosed.subarray(3, source.byteLength + 3))
    expect(offset.tick).toBe(value.tick)
    expect(offset.collisionSnapshot.bytes.buffer).not.toBe(enclosed.buffer)
    expect(value.movement).toMatchObject({
      grounded: true,
      position: [1, 2, 3],
      velocity: [4, 5, 6],
      viewOffset: [0, 0, 68],
    })
    expect(value.loadout[0]).toMatchObject({ clip: 3, reserve: 20 })
    expect(value.activities).toEqual([{ tick: 7n, weapon: 1, activity: 2 }])
    expect(value.lifecycle).toBe(1)
    expect(value.playerFlags).toBe(0x101)
    expect(value.inWater).toBe(false)
    expect(value.movement.waterLevel).toBe(0)
    expect(value.movement.waterType).toBe(0)
    expect(value.respawnTouchCount).toBe(1)
    expect(value.authorityBlockers.map((value) => value.code)).toEqual([1, 2])
    expect(value.projectiles[0]).toEqual({
      weapon: 1, critical: false, trail: 0, miniRocket: false, practiceExplosion: false,
      selfBlastOnly: false, modelVisible: false, airBurst: false, underwaterExplosion: false,
      identity: 9,
      kind: 1,
      team: 2,
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
    expect(value.projectileEvents[0]).toMatchObject({
      type: "fire",
      launcherPose: { eyePosition: [1, 2, 71], viewOrientation: [0, 0, 0, 1] },
    })
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
    new DataView(malformed).setFloat32(364 + 12, 0, true)
    expect(() => decodeSnapshot(malformed)).toThrow(Tf2CodecError)
    const priorVersion = snapshot()
    new DataView(priorVersion).setUint32(4, 5, true)
    expect(() => decodeSnapshot(priorVersion)).toThrow(Tf2CodecError)
    const stuck = snapshot()
    const stuckData = new Uint8Array(stuck), stuckView = new DataView(stuck)
    stuckData.set([2, 2, 3 | (3 << 3), 1], 328)
    stuckView.setFloat32(400, 1, true)
    expect(decodeSnapshot(stuck).projectiles[0]).toMatchObject({ kind: 2, state: 3, velocity: [100, 0, 0], contactNormal: [0, 0, 1] })
  })

  test("encodes canonical Engineer object commands without inventing object modes",()=>{
    const base={forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:false,detonate:false}
    const build=new DataView(encodeCommand({...base,building:{action:"build",object:{kind:2,mode:0}}}))
    expect(build.getUint32(28,true)).toBe((1<<16)|(2<<19))
    const exit=new DataView(encodeCommand({...base,building:{action:"destroy",object:{kind:1,mode:1}}}))
    expect(exit.getUint32(28,true)).toBe((2<<16)|(1<<19)|(1<<21))
    expect(new DataView(encodeCommand({...base,building:{action:"rotate"}})).getUint32(28,true)).toBe(3<<16)
    expect(new DataView(encodeCommand({...base,building:{action:"hurt",amount:100}})).getUint32(28,true)).toBe(0x8000|(100<<16))
    const hurtStopped = new DataView(encodeCommand({...base,nextbotStop:true,building:{action:"hurt",amount:65535}}))
    expect(hurtStopped.getUint32(28,true)).toBe((0x8000 | (65535 << 16)) >>> 0)
    expect(hurtStopped.getUint16(42,true)).toBe(0x8000)
    expect(()=>encodeCommand({...base,building:{action:"build",object:{kind:2,mode:1}}})).toThrow("building object is invalid")
    expect(()=>encodeCommand({...base,building:{action:"hurt",amount:65536}})).toThrow("building damage is invalid")
  })

  test("retains a primary-fire click beside every stock Engineer blueprint command", () => {
    const base = { forward: 0, side: 0, yawDegrees: 315, pitchDegrees: 0, jump: false, crouch: false, fire: true, detonate: false }
    for (const object of [
      { kind: 2, mode: 0 },
      { kind: 0, mode: 0 },
      { kind: 1, mode: 0 },
      { kind: 1, mode: 1 },
    ] as const) {
      const flags = new DataView(encodeCommand({ ...base, building: { action: "build", object } })).getUint32(28, true)
      expect(flags & (1 << 3)).toBe(1 << 3)
      expect((flags >> 16) & 7).toBe(1)
      expect((flags >> 19) & 3).toBe(object.kind)
      expect((flags >> 21) & 1).toBe(object.mode)
    }
  })

  test("encodes bounded bot commands and decodes ordered player lifecycle snapshots", () => {
    const base = { forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: false, detonate: false }
    const add = new DataView(encodeCommand({ ...base, bot: { action: "add", count: 3, class: 4, team: 3, difficulty: 3 } }))
    expect(add.getUint16(42, true)).toBe(1 | (3 << 2) | (4 << 7) | (3 << 11) | (3 << 13))
    expect(new DataView(encodeCommand({ ...base, bot: { action: "kick-all" } })).getUint16(42, true)).toBe(2)
    expect(new DataView(encodeCommand({ ...base, bot: { action: "kick-team", team: 2 } })).getUint16(42, true)).toBe(3 | (2 << 11))
    expect(() => encodeCommand({ ...base, bot: { action: "add", count: 32, class: 3, difficulty: 1 } })).toThrow(Tf2CodecError)
    const objectives = new DataView(encodeCommand({
      ...base,
      objectiveConfiguration: { capturesPerRound: 1, returnOnTouch: true },
    }))
    expect(objectives.getUint32(52, true)).toBe((0x8000_0000 | 1 | (1 << 16)) >>> 0)
    expect(() => encodeCommand({
      ...base,
      objectiveConfiguration: { capturesPerRound: 65_536, returnOnTouch: false },
    })).toThrow("command capture objective configuration is invalid")
    const teleport = new DataView(encodeCommand({
      ...base,
      botControl: { action: "teleport", identity: 4, position: [10, 20, 30], pitchDegrees: -5, yawDegrees: 90 },
    }))
    expect(teleport.getUint8(56)).toBe(1)
    expect(teleport.getUint32(60, true)).toBe(4)
    expect([64, 68, 72, 76, 80].map(offset => teleport.getFloat32(offset, true))).toEqual([10, 20, 30, -5, 90])
    expect(new DataView(encodeCommand({ ...base, botControl: { action: "whack", identity: 2 } })).getUint8(56)).toBe(2)
    expect(() => encodeCommand({ ...base, botControl: { action: "whack", identity: 1 } })).toThrow("command bot control identity is invalid")
    const configured = new DataView(encodeCommand({
      ...base,
      botConfiguration: {
        quota: 7, maximumPlayers: 24, mode: "fill", difficulty: 2,
        joinAfterPlayer: true, autoVacate: false, offlinePractice: true,
      },
    }))
    expect(configured.getUint32(44, true)).toBe((0x8000_0000 | 7 | (24 << 6) | (2 << 12) | (1 << 14) | (1 << 16) | (1 << 18)) >>> 0)
    expect(() => encodeCommand({
      ...base,
      botConfiguration: {
        quota: 32, maximumPlayers: 24, mode: "fill", difficulty: 2,
        joinAfterPlayer: true, autoVacate: false, offlinePractice: true,
      },
    })).toThrow("command bot configuration is invalid")

    const prior = new Uint8Array(snapshot())
    const objectiveOffset = prior.byteLength - 184
    const botName = new TextEncoder().encode("Chucklenuts")
    const bytes = new Uint8Array(prior.byteLength + 128 + 33 + botName.length + 28)
    const roundOffset = prior.byteLength - 96
    bytes.set(prior.subarray(0, objectiveOffset))
    bytes.set(prior.subarray(objectiveOffset, roundOffset), objectiveOffset + 128)
    bytes.set(prior.subarray(roundOffset, prior.length - 24), roundOffset + 128 + 33 + botName.length)
    bytes.set(prior.subarray(prior.length - 24), bytes.length - 24)
    const view = new DataView(bytes.buffer)
    view.setUint32(objectiveOffset - 4, 1, true)
    const at = objectiveOffset
    const scoreboardOffset = objectiveOffset + 32
    view.setUint32(at, 2, true)
    bytes.set([3, 3, 1, 1, 1], at + 4)
    view.setInt32(at + 12, 200, true)
    view.setInt32(at + 16, 200, true)
    view.setUint32(at + 20, 1, true)
    view.setUint32(at + 24, 10785, true)
    view.setUint32(at + 28, 16, true)
    view.setFloat32(at + 32, 90, true)
    ;[-2528, -1744, 17, 240, 0, 0].forEach((value, index) => view.setFloat32(at + 36 + index * 4, value, true))
    view.setFloat32(at + 60, -8, true)
    bytes.set([1, 0, 0, 1], at + 64)
    view.setUint16(at + 68, 3, true)
    view.setUint16(at + 70, 20, true)
    view.setUint16(at + 72, 4, true)
    view.setUint16(at + 74, 20, true)
    view.setUint32(at + 76, 4, true)
    view.setUint32(at + 80, 2, true)
    view.setBigUint64(at + 96, 6n, true)
    view.setBigUint64(at + 104, 0xffff_ffff_ffff_ffffn, true)
    view.setBigUint64(at + 112, 20n, true)
    const scoreboard = scoreboardOffset + 128
    bytes[scoreboard + 9] = 1
    bytes[scoreboard + 10] = 2
    const scoreboardBot = scoreboard + 52
    view.setUint32(scoreboardBot, 2, true)
    bytes.set([3, 3, 1, 1], scoreboardBot + 4)
    bytes[scoreboardBot + 32] = botName.length
    bytes.set(botName, scoreboardBot + 33)
    view.setFloat32(bytes.byteLength - 28, 88, true)
    view.setFloat32(bytes.byteLength - 4, 1, true)
    const decoded = decodeSnapshot(bytes)
    expect(decoded.scoreboard).toMatchObject({ redCount: 1, blueCount: 1, players: [{ name: "unnamed" }, { name: "Chucklenuts", fake: true }] })
    expect(decoded.bots).toEqual([{
      conditions: [0, 0, 0, 0, 0],
      overheadHeight: 88,
      equippedItems: [],
      identity: 2,
      class: 3,
      team: 3,
      lifecycle: 1,
      difficulty: 1,
      objective: 1,
      health: 200,
      maximumHealth: 200,
      target: 1,
      area: 10785,
      remainingPathAreas: 16,
      yawDegrees: 90,
      pitchDegrees: -8,
      position: [-2528, -1744, 17],
      velocity: [240, 0, 0],
      weapon: { identity: 1, reload: 0, clip: 3, reserve: 20, maximumClip: 4, maximumReserve: 20, nextPrimaryTick: 20n, nextReloadTick: 0n },
      shots: 4, hits: 2, kills: 0, deaths: 0, captures: 0, carryingFlag: false, animationRole: "PRIMARY", lastFireTick: 6n, respawnTick: null,
    }])
    view.setInt32(at + 12, 250, true)
    expect(decodeSnapshot(bytes).bots[0]?.health).toBe(250)
    view.setInt32(at + 12, 301, true)
    expect(() => decodeSnapshot(bytes)).toThrow(Tf2CodecError)
    view.setInt32(at + 12, 200, true)
    for (const [playerClass, weapon] of [[2, 12], [2, 13], [5, 19], [5, 20], [5, 21], [7, 15], [7, 16], [4, 17], [4, 18], [9, 40], [9, 41], [8, 50], [8, 51], [8, 52], [8, 53], [8, 54]] as const) {
      bytes[at + 4] = playerClass
      bytes[scoreboardBot + 4] = playerClass
      bytes[at + 64] = weapon
      if (playerClass === 8) {
        const count = bytes.length - 60
        const bound = new Uint8Array(bytes.length + 28)
        bound.set(bytes.subarray(0, count + 4)); bound.set(bytes.subarray(count + 4), count + 32)
        const fields = new DataView(bound.buffer)
        fields.setUint32(count, 1, true)
        fields.setUint32(count + 4, view.getUint32(at, true), true)
        ;[0, 0, 0, 0.4, 0.5, 1].forEach((value, index) => fields.setFloat32(count + 8 + index * 4, value, true))
        expect(decodeSnapshot(bound).bots[0]?.weapon?.identity).toBe(weapon)
        expect(decodeSnapshot(bound).actorCloaks[0]!.identity).toBe(view.getUint32(at, true))
      } else expect(decodeSnapshot(bytes).bots[0]?.weapon?.identity).toBe(weapon)
    }
    bytes[at + 64] = 22
    expect(() => decodeSnapshot(bytes)).toThrow(Tf2CodecError)
    bytes[at + 64] = 40
    view.setUint32(at, 1, true)
    expect(() => decodeSnapshot(bytes)).toThrow(Tf2CodecError)
  })

  test("retains independent Source in-water flags and canonical fluid contents", () => {
    for (const [level, flags, fluid, inWater] of [
      [0, 0x101, 0, false],
      [1, 0x101, 0x20, false],
      [2, 0x501, 0x20, true],
      [3, 0x101, 0x20, false],
      [3, 0x501, 0x10, true],
      [0, 0x501, 0, true],
    ] as const) {
      const bytes = snapshot(), view = new DataView(bytes), data = new Uint8Array(bytes)
      view.setUint32(160, flags, true)
      view.setUint32(164, fluid, true)
      data[190] = level
      const decoded = decodeSnapshot(bytes)
      expect(decoded.playerFlags).toBe(flags)
      expect(decoded.inWater).toBe(inWater)
      expect(decoded.movement.waterLevel).toBe(level)
      expect(decoded.movement.waterType).toBe(fluid)
    }
    const malformed = snapshot()
    new DataView(malformed).setUint32(164, 0x1000_0020, true)
    expect(() => decodeSnapshot(malformed)).toThrow(Tf2CodecError)
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
    const client = new Tf2WorkerClient(worker, new MemoryCache(), "cd".repeat(32))
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
    expect(publication.snapshotByteLength).toBe(publication.eventBatches[0]!.byteLength)
    expect(publication.eventBatches[0]!.snapshot.collisionSnapshot.bytes.byteLength).toBe(52)
    expect(publication.snapshot).toBe(publication.eventBatches[0]!.snapshot)
    await client.shutdown()
  })

  test("opt-in simulation evidence reads authoritative publications without retaining their buffers", async () => {
    const host = globalThis as any
    const previous = host.__playsrcFrameProfiler
    const profile = { active: true, simulation: [] as any[], simulationDropped: 0 }
    host.__playsrcFrameProfiler = profile
    const client = new Tf2WorkerClient(new CourseWorker(), new MemoryCache(), "cd".repeat(32))
    try {
      const command = encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: false, detonate: false })
      const publications = await client.observe(4, 1, command)
      expect(profile.simulation[0]).toMatchObject({ publications: [{ hostFrame: "1", firstHostTick: "1", lastHostTick: "1", selectedTicks: 1, eventBatches: 1 }] })
      expect(profile.simulation[0].requestId).toBeNumber()
      expect(profile.simulation[0].replayAttack).toBeNull()
      expect(profile.simulation[0].publications[0].player).toEqual({
        tick: String(publications[0]!.snapshot.tick), playerClass: publications[0]!.snapshot.class,
        weapon: publications[0]!.snapshot.weapon, lifecycle: publications[0]!.snapshot.lifecycle,
      })
      expect(profile.simulation[0].publications[0].weapons).toEqual(publications[0]!.snapshot.loadout.map(weapon => ({
        weapon: weapon.weapon, firstPrimaryTick: String(weapon.firstPrimaryTick), nextPrimaryTick: String(weapon.nextPrimaryTick),
      })))
      expect(profile.simulation[0].decodeMilliseconds).toBeGreaterThanOrEqual(0)
      expect(profile.simulation[0]).not.toHaveProperty("snapshotBytes")
      expect(publications[0]!.snapshot).toBe(publications[0]!.eventBatches[0]!.snapshot)
    } finally {
      await client.shutdown()
      if (previous === undefined) delete host.__playsrcFrameProfiler
      else host.__playsrcFrameProfiler = previous
    }
  })

  test("opt-in attack telemetry preserves the Rust owner's tick and player instead of inferring a shot", async () => {
    const host = globalThis as any, previous = host.__playsrcFrameProfiler
    const profile = { active: true, simulation: [] as any[], simulationDropped: 0 }
    host.__playsrcFrameProfiler = profile
    const worker = new CourseWorker()
    worker.replayAttack = { hostTick: 1n, playerClass: 3, weapon: 1, lifecycle: 1 }
    const client = new Tf2WorkerClient(worker, new MemoryCache(), "cd".repeat(32))
    try {
      const command = encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: true, detonate: false })
      const publications = await client.observe(4, 1, command)
      expect(command.byteLength).toBe(0)
      expect(profile.simulation[0].replayAttack).toEqual({ hostTick: "1", playerClass: 3, weapon: 1, lifecycle: 1 })
      expect(profile.simulation[0].publications[0].activities).toEqual(publications[0]!.snapshot.activities.map(activity => ({ ...activity, tick: String(activity.tick) })))
      expect(profile.simulation[0].replayAttack).not.toHaveProperty("fired")
    } finally {
      await client.shutdown()
      if (previous === undefined) delete host.__playsrcFrameProfiler
      else host.__playsrcFrameProfiler = previous
    }
  })

  test("randomized lossless full/delta sequence parity retains all event ticks and large rosters", () => {
    const stream = new SimulationSnapshotStream(), retained: Array<{ value: ReturnType<typeof decodeSnapshot>; bytes: Uint8Array }> = []
    let seed = 0x74c12fe3, tick = 1n, previous: Uint8Array | undefined, fullBytes = 0, wireBytes = 0
    let fullDecodeMilliseconds = 0, optimizedDecodeMilliseconds = 0, fullObjects = 0, optimizedObjects = 0
    const fullSeen = new WeakSet<object>(), optimizedSeen = new WeakSet<object>()
    const objects = (value: unknown, seen: WeakSet<object>): number => {
      if (value === null || typeof value !== "object" || seen.has(value)) return 0
      seen.add(value)
      return 1 + (ArrayBuffer.isView(value) ? 0 : Object.values(value).reduce((sum, child) => sum + objects(child, seen), 0))
    }
    const random = () => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return seed >>> 0 }
    for (let frame = 0; frame < 100; frame++) {
      const count = 1 + random() % 4, states: Uint8Array[] = []
      for (let i = 0; i < count; i++) {
        const state = rosterSnapshot(tick + BigInt(i) + 6n, frame < 30 ? 31 : frame < 60 ? 23 : 15)
        const view = new DataView(state.buffer), brushHeader = 1045
        view.setFloat32(204, random() % 1000 - 500, true)
        view.setFloat32(20, random() % 201, true)
        const brush = random() % 512
        view.setFloat32(brushHeader + 52 + brush * 128 + 40, random() % 100, true)
        states.push(state)
      }
      const packet = snapshotPacket(tick, states, frame % 17 === 0 ? undefined : previous)
      wireBytes += packet.byteLength; fullBytes += states.reduce((sum, bytes) => sum + bytes.length + 24, 56)
      const optimizedStarted = performance.now()
      const publication = stream.decode(packet)[0]!
      optimizedDecodeMilliseconds += performance.now() - optimizedStarted
      const fullStarted = performance.now()
      const expected = states.map(bytes => decodeSnapshot(bytes))
      const fullSnapshot = mergePublicationSnapshots(expected)
      fullDecodeMilliseconds += performance.now() - fullStarted
      fullObjects += expected.reduce((sum, state) => sum + objects(state, fullSeen), 0)
      optimizedObjects += publication.eventBatches.reduce((sum, event) => sum + objects(event.snapshot, optimizedSeen), 0)
      expect(publication.snapshot).toEqual(fullSnapshot)
      expect(publication.eventBatches.map(event => event.snapshot)).toEqual(expected)
      expect(publication.eventBatches.map(event => event.hostTick)).toEqual(states.map((_, i) => tick + BigInt(i)))
      expect(publication.snapshot.projectileTimeline.map(event => event.tick)).toEqual(expected.map(value => value.tick))
      retained.push({ value: publication.eventBatches[0]!.snapshot, bytes: states[0]!.slice() })
      tick += BigInt(count); previous = states.at(-1)
    }
    for (const entry of retained) expect(entry.value).toEqual(decodeSnapshot(entry.bytes))
    expect(wireBytes).toBeLessThan(fullBytes / 8)
    expect(stream.metrics.reusedRanges).toBeGreaterThan(stream.metrics.decodedRanges * 8)
    expect(stream.metrics.deltaSnapshots).toBeGreaterThan(200)
    expect(optimizedObjects).toBeLessThan(fullObjects / 4)
    if (process.env.PLAYSRC_SNAPSHOT_METRICS === "1") console.log("SNAPSHOT_PARITY", JSON.stringify({ ticks: Number(tick - 1n), fullBytes, wireBytes,
      fullDecodeMilliseconds, optimizedDecodeMilliseconds, fullObjects, optimizedObjects, ...stream.metrics }))
  })

  test("immutable sections, signed zero, caller-owned collision bytes and shutdown do not alias baselines", () => {
    const stream = new SimulationSnapshotStream(), first = rosterSnapshot(7n)
    const packet = snapshotPacket(1n, [first])
    const a = stream.decode(packet)[0]!.snapshot
    expect(stream.metrics.retainedBaselineBytes).toBe(first.byteLength)
    new Uint8Array(packet).fill(0) // stream owns its authoritative full restore
    const second = first.slice(), view = new DataView(second.buffer)
    view.setBigUint64(8, 8n, true)
    const b = stream.decode(snapshotPacket(2n, [second], first))[0]!.snapshot
    expect(stream.metrics.retainedBaselineBytes).toBe(second.byteLength)
    expect(b.entityPresentation.models).toBe(a.entityPresentation.models)
    expect(b.bots).toBe(a.bots); expect(b.loadout).toBe(a.loadout); expect(b.scoreboard).toBe(a.scoreboard)
    expect(b.entityTransforms).toBe(a.entityTransforms); expect(b.randomState).toBe(a.randomState)
    expect(Object.isFrozen(b.bots[0]!.position)).toBe(true)
    a.collisionSnapshot.bytes.fill(0)
    expect(b.collisionSnapshot.bytes[0]).toBe(67)
    const third = second.slice(), thirdView = new DataView(third.buffer)
    thirdView.setBigUint64(8, 9n, true); thirdView.setFloat32(1045 + 52 + 40, -0, true)
    const c = stream.decode(snapshotPacket(3n, [third], second))[0]!.snapshot
    expect(Object.is(c.entityPresentation.models[0]!.worldPosition[0], -0)).toBe(true)
    expect(Object.is(a.entityPresentation.models[0]!.worldPosition[0], 0)).toBe(true)
    expect(c.entityPresentation.models[0]).not.toBe(b.entityPresentation.models[0])
    expect(c.entityPresentation.models[1]).toBe(b.entityPresentation.models[1])
    stream.close()
    expect(stream.metrics.retainedBaselineBytes).toBe(0)
    expect(() => stream.decode(snapshotPacket(4n, [third]))).toThrow("Closed")
    expect(c.bots[0]!.identity).toBe(2)
  })

  test("malformed, stale, NaN, cross-section and truncated responses roll back the entire decode", () => {
    const first = rosterSnapshot(7n, 31, 1), second = rosterSnapshot(8n, 31, 1)
    const stream = new SimulationSnapshotStream()
    stream.decode(snapshotPacket(1n, [first]))
    const valid = snapshotPacket(2n, [second], first)
    for (const offset of [4, 24, 32, 40, 48, 52, 56, 64, 68, 72, 80, 84]) {
      const malformed = valid.slice(0); new DataView(malformed).setUint32(offset, 0xffff_ffff, true)
      expect(() => stream.decode(malformed)).toThrow()
      expect(stream.tick).toBe(1n)
      expect(stream.metrics.retainedBaselineBytes).toBe(first.byteLength)
    }
    for (let size = 0; size < valid.byteLength; size++) {
      expect(() => stream.decode(valid.slice(0, size))).toThrow()
      expect(stream.tick).toBe(1n)
    }
    for (const offset of [20, 204, 1045 + 52 + 40]) {
      const malformed = second.slice(); new DataView(malformed.buffer).setFloat32(offset, NaN, true)
      expect(() => stream.decode(snapshotPacket(2n, [malformed], first))).toThrow()
    }
    const reordered = second.slice(), reorderedView = new DataView(reordered.buffer)
    reorderedView.setUint32(1109 + 128, 1, true)
    expect(() => stream.decode(snapshotPacket(2n, [reordered], first))).toThrow()
    const mismatchedClass = second.slice()
    mismatchedClass[1109 + 128 + 4] = 6
    expect(() => stream.decode(snapshotPacket(2n, [mismatchedClass], first))).toThrow("scoreboard roster")
    const third = rosterSnapshot(9n, 31, 1)
    new DataView(third.buffer).setFloat32(20, NaN, true)
    expect(() => stream.decode(snapshotPacket(2n, [second, third], first))).toThrow()
    expect(stream.tick).toBe(1n)
    expect(stream.decode(valid)[0]!.snapshot).toEqual(decodeSnapshot(second))
    expect(() => stream.decode(valid)).toThrow()
    const independentGeneration = new SimulationSnapshotStream()
    expect(() => independentGeneration.decode(valid)).toThrow()
    expect(independentGeneration.decode(snapshotPacket(1n, [first]))[0]!.snapshot).toEqual(decodeSnapshot(first))
  })

  test("identical objective events are retained once for each exact tick, not deduplicated with state", () => {
    const withObjective = (tick: bigint) => {
      const base = rosterSnapshot(tick, 31, 1), at = 1121 + 128 + 31 * 128
      const bytes = new Uint8Array(base.length + 48), view = new DataView(bytes.buffer)
      bytes.set(base.subarray(0, at + 12)); bytes.set(base.subarray(at + 12), at + 60)
      bytes[at + 8] = 1; view.setUint32(at + 32, 1, true)
      bytes.set([1, 0, 2, 0], at + 36); view.setUint32(at + 40, 123, true)
      view.setUint32(at + 44, 1, true); view.setFloat32(at + 52, 3, true)
      return bytes
    }
    const states = [withObjective(7n), withObjective(8n), withObjective(9n)]
    const publication = new SimulationSnapshotStream().decode(snapshotPacket(1n, states))[0]!
    expect(publication.snapshot).toEqual(mergePublicationSnapshots(states.map(state => decodeSnapshot(state))))
    expect(publication.snapshot.objectives!.events).toHaveLength(3)
    expect(publication.eventBatches[1]!.snapshot.objectives).toBe(publication.eventBatches[0]!.snapshot.objectives)
    expect(publication.snapshot.activities.map(event => event.tick)).toEqual([7n, 8n, 9n])
    expect(publication.snapshot.entityEvents.map(event => event.sequence)).toEqual([7n, 8n, 9n])
  })

  test("multiple publications commit as one response and reject overlapping replacement runs", () => {
    const first = rosterSnapshot(7n, 15, 1), second = rosterSnapshot(8n, 15, 1)
    const a = new Uint8Array(snapshotPacket(1n, [first])), b = new Uint8Array(snapshotPacket(2n, [second], first))
    const response = new Uint8Array(a.length + b.length - 16)
    response.set(a); response.set(b.subarray(16), a.length)
    new DataView(response.buffer).setUint32(8, 2, true)
    const stream = new SimulationSnapshotStream()
    const malformed = response.slice()
    // The first publication is valid; corrupt the base tick of the second.
    new DataView(malformed.buffer).setBigUint64(a.length + 56, 99n, true)
    expect(() => stream.decode(malformed.buffer)).toThrow("WorkerFailed")
    expect(stream.tick).toBe(0n); expect(stream.metrics.responses).toBe(0)
    expect(stream.metrics.retainedBaselineBytes).toBe(0)
    expect(stream.decode(response.buffer).map(publication => publication.snapshot.tick)).toEqual([7n, 8n])
    expect(stream.metrics.retainedBaselineBytes).toBe(second.byteLength)
    const third = rosterSnapshot(9n, 15, 1), overlapping = snapshotPacket(3n, [third], second)
    const view = new DataView(overlapping)
    const firstRunStart = view.getUint32(80, true)
    view.setUint32(89, firstRunStart, true)
    expect(() => stream.decode(overlapping)).toThrow("WorkerFailed")
    expect(stream.tick).toBe(2n)
  })

  test("client generation replacement cancels stale decodes and shutdown drains pending snapshot ownership", async () => {
    let listener: ((event: MessageEvent<WorkerResponse>) => void) | undefined
    const requests: WorkerRequest[] = []
    let terminated = false
    const worker = {
      postMessage(request: WorkerRequest, transfer: Transferable[] = []) { requests.push(structuredClone(request, { transfer })) },
      addEventListener(kind: string, callback: (event: MessageEvent<WorkerResponse>) => void) { if (kind === "message") listener = callback },
      removeEventListener() {}, terminate() { terminated = true },
    } as WorkerLike
    const send = (response: WorkerResponse) => listener!({ data: response } as MessageEvent<WorkerResponse>)
    const command = () => encodeCommand({ forward: 0, side: 0, yawDegrees: 0, pitchDegrees: 0, jump: false, crouch: false, fire: true, detonate: false })
    const client = new Tf2WorkerClient(worker, new MemoryCache(), "cd".repeat(32))
    const first = client.observe(1, 1, command())
    send({ id: requests.at(-1)!.id, kind: "simulation", generation: 1, output: snapshotPacket(1n, [rosterSnapshot(7n)]) })
    const retained = (await first)[0]!.snapshot
    const stale = client.observe(1, 2, command()).catch(error => error)
    const staleId = requests.at(-1)!.id
    expect((requests.at(-1) as Extract<WorkerRequest, { kind: "observe" }>).snapshotTick).toBe(1n)
    const activation = client.activate(2)
    send({ id: requests.at(-1)!.id, kind: "activated", generation: 2 }); await activation
    send({ id: staleId, kind: "simulation", generation: 1, output: snapshotPacket(2n, [rosterSnapshot(8n)]) })
    expect((await stale).code).toBe("Closed")
    const pending = client.observe(2, 3, command())
    const pendingId = requests.at(-1)!.id
    expect((requests.at(-1) as Extract<WorkerRequest, { kind: "observe" }>).snapshotTick).toBe(0n)
    const shutdown = client.shutdown(), shutdownId = requests.at(-1)!.id
    send({ id: pendingId, kind: "simulation", generation: 2, output: snapshotPacket(1n, [rosterSnapshot(7n, 23)]) })
    expect((await pending)[0]!.snapshot.bots).toHaveLength(23)
    send({ id: shutdownId, kind: "shutdown" }); await shutdown
    expect(terminated).toBe(true); expect(client.snapshotMetrics(1)).toBeUndefined(); expect(client.snapshotMetrics(2)).toBeUndefined()
    expect(retained.bots).toHaveLength(31)
    expect(retained.entityPresentation.models[0]!.model).toBe(1)
  })
})
