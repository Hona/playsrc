import { expect, test } from "bun:test"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import {
  createParticleBatchEncoder,
  createViewmodelPresenter,
  decodeModelPoseOutput,
  encodeModelPoseBatch,
  ProjectilePresentationError,
  hitscanMuzzleParticles,
  tf2Camera,
  tf2Hud,
  type ProjectileParticleRequest,
} from "../src/presentation"
import type { PresentationArtifacts } from "../src/artifacts"
import type { Snapshot } from "../src/codec"

test("encodes one bounded complete PCF phase without per-particle calls", () => {
  const request: ProjectileParticleRequest = Object.freeze({
    kind: "start",
    identity: "7:0:fire:9:start:projectile:9:trail",
    effectIdentity: "projectile:9:trail",
    eventIdentity: "7:0:fire:9",
    tick: 7n,
    projectileIdentity: 9,
    ownerIdentity: 1,
    launcherIdentity: 2,
    team: "red",
    system: "rockettrail",
    attachment: Object.freeze({ entityIdentity: 9, name: "trail" }),
    controlPoints: Object.freeze([
      Object.freeze({
        index: 0,
        position: Object.freeze([1, 2, 3]),
        orientation: Object.freeze([0, 0, 0, 1]),
        ownerIdentity: 1,
      }),
    ]),
  })
  const bytes = createParticleBatchEncoder().encode(7n, [4, 5, 6], [request])
  const view = new DataView(bytes.buffer)
  expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("PPTX")
  expect(view.getUint32(4, true)).toBe(2)
  expect(view.getUint32(28, true)).toBe(1)
  expect(bytes[32]).toBe(1)
  expect(new TextDecoder().decode(bytes.subarray(68, 79))).toBe("rockettrail")
})

test("preserves source ticks and graceful stop in one multi-tick Particle phase", () => {
  const requests: readonly ProjectileParticleRequest[] = [
    Object.freeze({
      kind: "start",
      identity: "2:0:fire:9:start:projectile:9:trail",
      effectIdentity: "projectile:9:trail",
      eventIdentity: "2:0:fire:9",
      tick: 2n,
      projectileIdentity: 9,
      ownerIdentity: 1,
      launcherIdentity: 2,
      team: "red",
      system: "rockettrail",
      attachment: Object.freeze({ entityIdentity: 9, name: "trail" }),
      controlPoints: Object.freeze([Object.freeze({
        index: 0,
        position: Object.freeze([1, 2, 3]),
        orientation: Object.freeze([0, 0, 0, 1]),
        ownerIdentity: 1,
      })]),
    }),
    Object.freeze({
      kind: "stop",
      identity: "4:0:explode:9:stop:projectile:9:trail",
      effectIdentity: "projectile:9:trail",
      eventIdentity: "4:0:explode:9",
      tick: 4n,
      projectileIdentity: 9,
      immediate: false,
    }),
  ]
  const bytes = createParticleBatchEncoder().encode(4n, [0, 0, 0], requests)
  const view = new DataView(bytes.buffer)
  expect(view.getFloat32(44, true)).toBeCloseTo(0.03)
  expect(bytes[111]).toBe(3)
  expect(view.getFloat32(123, true)).toBeCloseTo(0.06)

  const reversed = createParticleBatchEncoder()
  reversed.encode(3n, [0, 0, 0], [])
  expect(() => reversed.encode(4n, [0, 0, 0], [requests[0]!])).toThrow(ProjectilePresentationError)
})

test("encodes each Unicode model/activity exactly once into the fire-tick PMRQ v6 contract", () => {
  const request = Object.freeze({
    identity: 7,
    model: "models/é.mdl",
    activity: "ACT_雪",
    previousElapsedSeconds: 0,
    elapsedSeconds: 0.25,
    currentTimeSeconds: 0.25,
    frameTimeSeconds: 0.015,
    planarSpeed: 0,
    screenAspectRatio: 16 / 9,
    worldFarPlane: 32_768,
    skin: 0,
    lod: 0,
    bodygroups: Object.freeze([1, 2]),
  })
  const bytes = encodeModelPoseBatch([request])
  const view = new DataView(bytes.buffer)
  expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("PMRQ")
  expect(view.getUint32(4, true)).toBe(6)
  expect(view.getUint32(8, true)).toBe(1)
  expect(view.getBigUint64(16, true)).toBe(0n)
  expect(view.getUint32(56, true)).toBe(new TextEncoder().encode(request.model).byteLength)
  expect(new TextDecoder().decode(bytes.subarray(60, 60 + view.getUint32(56, true)))).toBe(request.model)
})

test("encodes stock fists as one hands-only viewmodel without an invented item", () => {
  const request = Object.freeze({
    identity: 7,
    model: "models/weapons/c_models/c_heavy_arms.mdl",
    handsOnlyViewmodel: true,
    activity: "ACT_FISTS_VM_DRAW",
    previousElapsedSeconds: 0,
    elapsedSeconds: 0,
    currentTimeSeconds: 0,
    frameTimeSeconds: 0.015,
    planarSpeed: 0,
    screenAspectRatio: 16 / 9,
    worldFarPlane: 32_768,
    phase: 0 as const,
    skin: 2,
    lod: 0,
    bodygroups: Object.freeze([0]),
  })
  const bytes = encodeModelPoseBatch([request])
  expect(bytes[24]).toBe(2)
  const modelBytes = new TextEncoder().encode(request.model).byteLength
  expect(new DataView(bytes.buffer).getUint32(60 + modelBytes, true)).toBe(0)
  expect(() => encodeModelPoseBatch([{ ...request, itemModel: "models/invented.mdl", itemBodygroups: [] }])).toThrow(ProjectilePresentationError)
})

test("encodes historical attachment-only fire samples without extra model transactions", () => {
  const request = Object.freeze({
    identity: 7,
    model: "models/weapons/c_models/c_soldier_arms.mdl",
    itemModel: "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
    activity: "ACT_PRIMARY_VM_PRIMARYATTACK",
    sampleTick: 18n,
    attachmentsOnly: true,
    fireView: Object.freeze({
      eyePosition: Object.freeze([10, 20, 30]) as readonly [number, number, number],
      viewOrientation: Object.freeze([0, 0, 0, 1]) as readonly [number, number, number, number],
    }),
    previousElapsedSeconds: 0,
    elapsedSeconds: 0,
    currentTimeSeconds: 0.27,
    frameTimeSeconds: 0.015,
    planarSpeed: 0,
    screenAspectRatio: 16 / 9,
    worldFarPlane: 32_768,
    phase: 1 as const,
    skin: 0,
    lod: 0,
    bodygroups: Object.freeze([0]),
    itemBodygroups: Object.freeze([0]),
  })
  const bytes = encodeModelPoseBatch([request, { ...request, sampleTick: 19n, attachmentsOnly: false }])
  const view = new DataView(bytes.buffer)
  expect(view.getUint32(8, true)).toBe(2)
  expect(view.getBigUint64(16, true)).toBe(18n)
  expect([...bytes.subarray(24, 28)]).toEqual([1, 1, 1, 0])
  expect([0, 1, 2].map((index) => view.getFloat32(28 + index * 4, true))).toEqual([10, 20, 30])
  expect(() => encodeModelPoseBatch([{ ...request, fireView: undefined }]))
    .toThrow(ProjectilePresentationError)
})

test("decodes exact interleaved PMPO vertex planes and rejects non-finite or truncated geometry", () => {
  const output: number[] = [0x50, 0x4d, 0x50, 0x4f]
  const u32 = (value: number): void => {
    const bytes = new Uint8Array(4)
    new DataView(bytes.buffer).setUint32(0, value, true)
    output.push(...bytes)
  }
  const f32 = (value: number): void => {
    const bytes = new Uint8Array(4)
    new DataView(bytes.buffer).setFloat32(0, value, true)
    output.push(...bytes)
  }
  const text = (value: string): void => {
    const bytes = new TextEncoder().encode(value)
    u32(bytes.byteLength)
    output.push(...bytes)
  }
  u32(5)
  u32(1)
  u32(9)
  u32(7)
  u32(0)
  output.push(0, 0, 0, 0)
  text("models/example.mdl")
  text("ACT_IDLE")
  u32(3)
  ;[24, 1, 1, 1].forEach(f32)
  output.push(0, 0, 0, 0)
  f32(0)
  f32(0.25)
  output.push(...new Array(68).fill(0))
  output.push(...new Array(8).fill(0))
  u32(0)
  u32(0)
  u32(0)
  u32(0)
  u32(1)
  u32(5)
  u32(6)
  u32(2)
  output.push(1, 0, 0, 0)
  ;[1, 2, 3, 0, 0, 1, 4, 5, 6, 1, 7, 8, 9, 0, 1, 0, 10, 11, 12, -1].forEach(f32)
  u32(0)
  const bytes = Uint8Array.from(output)
  const pose = decodeModelPoseOutput(bytes)[0]!
  expect(pose.sampleTick).toBe(7n)
  expect(pose.attachmentsOnly).toBe(false)
  const primitive = pose.primitives[0]!
  expect([...primitive.positions]).toEqual([1, 2, 3, 7, 8, 9])
  expect([...primitive.normals]).toEqual([0, 0, 1, 0, 1, 0])
  expect([...primitive.tangents]).toEqual([4, 5, 6, 1, 10, 11, 12, -1])
  expect(primitive.translucent).toBe(true)
  const invalid = bytes.slice()
  new DataView(invalid.buffer).setFloat32(invalid.byteLength - 84, Number.NaN, true)
  expect(() => decodeModelPoseOutput(invalid)).toThrow(ProjectilePresentationError)
  expect(() => decodeModelPoseOutput(bytes.subarray(0, bytes.byteLength - 1))).toThrow(ProjectilePresentationError)
})

test("uses the default TF2 horizontal-4:3 world projection and Source clip planes", () => {
  const snapshot = {
    position: Object.freeze([10, 20, 30]),
    movement: { viewOffset: Object.freeze([0, 0, 68]) },
  } as unknown as Snapshot
  expect(tf2Camera(snapshot, 90, -10)).toEqual({
    position: [10, 20, 98],
    yawDegrees: 90,
    pitchDegrees: -10,
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(75),
    near: 7,
    far: 28_377.919921875,
  })
})

test("joins the current team skin to the matching viewmodel template", () => {
  const identity = "models/weapons/c_models/c_soldier_arms.mdl"
  const itemIdentity = "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
  const descriptor = Object.freeze({
    kind: "viewmodel",
    horizontalFov4By3: 54,
    minimumFov: 54,
    maximumFov: 70,
    near: 1,
    depthRange: Object.freeze([0, 0.1]),
    drawsAfterWorld: true,
    opaqueBeforeTranslucent: true,
    optionalViewSpaceYReflection: true,
  })
  const artifacts = {
    models: new Map([[identity, {
      identity, bodygroupCounts: Object.freeze([]), descriptor,
      sequences: Object.freeze([
        { activity: "ACT_PRIMARY_VM_DRAW", durationSeconds: 0.8 },
        { activity: "ACT_PRIMARY_RELOAD_START", durationSeconds: 0.5 },
      ]),
    }], [itemIdentity, { identity: itemIdentity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) }]]),
  } as unknown as PresentationArtifacts
  const snapshot = (team: 2 | 3) => ({
    class: 3,
    team,
    tick: 1n,
    projectileEvents: Object.freeze([]),
    activities: Object.freeze([]),
    loadout: Object.freeze([{ weapon: 1, reload: 0, clip: 4 }]),
    weapon: 1,
    velocity: Object.freeze([0, 0, 0]),
  }) as unknown as Snapshot
  expect(createViewmodelPresenter(artifacts).map(snapshot(2)).item.skin).toBe(0)
  expect(createViewmodelPresenter(artifacts).map(snapshot(3)).item.skin).toBe(1)
  const reloading = {
    ...snapshot(2),
    activities: Object.freeze([{ tick: 1n, weapon: 1, activity: 3 }]),
  } as unknown as Snapshot
  expect(createViewmodelPresenter(artifacts).map(reloading).request.activity).toBe("ACT_PRIMARY_RELOAD_START")
})

test("composes every Scout stock item with its exact primary, secondary, and melee activities", () => {
  const hands = "models/weapons/c_models/c_scout_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze([
    "ACT_PRIMARY_VM_DRAW", "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_PRIMARY_RELOAD_START",
    "ACT_SECONDARY_VM_DRAW", "ACT_SECONDARY_VM_PRIMARYATTACK", "ACT_SECONDARY_VM_RELOAD",
    "ACT_MELEE_VM_DRAW", "ACT_MELEE_VM_HITCENTER",
  ].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  const items = [
    [4, "models/weapons/c_models/c_scattergun.mdl", "ACT_PRIMARY_VM_PRIMARYATTACK"],
    [5, "models/weapons/c_models/c_pistol/c_pistol.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK"],
    [6, "models/weapons/c_models/c_bat.mdl", "ACT_MELEE_VM_HITCENTER"],
  ] as const
  for (const [, identity] of items) models.set(identity, { identity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
  const artifacts = { models } as unknown as PresentationArtifacts
  for (const [weapon, item, activity] of items) {
    const snapshot = { class: 1, team: 2, tick: 1n, weapon, velocity: Object.freeze([0, 0, 0]), loadout: Object.freeze([{ weapon, reload: 0, clip: weapon === 6 ? 0 : 1 }]), activities: Object.freeze([{ tick: 1n, weapon, activity: 2 }]) } as unknown as Snapshot
    const request = createViewmodelPresenter(artifacts).map(snapshot).request
    expect(request.model).toBe(hands)
    expect(request.itemModel).toBe(item)
    expect(request.activity).toBe(activity)
  }
})

test("composes Soldier shotgun and shovel with exact secondary and melee activities", () => {
  const hands = "models/weapons/c_models/c_soldier_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze(["ACT_SECONDARY_VM_PRIMARYATTACK", "ACT_MELEE_VM_HITCENTER"].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  for (const [weapon, item, activity] of [
    [7, "models/weapons/c_models/c_shotgun/c_shotgun.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK"],
    [8, "models/weapons/c_models/c_shovel/c_shovel.mdl", "ACT_MELEE_VM_HITCENTER"],
  ] as const) {
    models.set(item, { identity: item, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
    const snapshot = { class: 3, team: 2, tick: 1n, weapon, velocity: Object.freeze([0, 0, 0]), loadout: Object.freeze([{ weapon, reload: 0, clip: weapon === 8 ? 0 : 5 }]), activities: Object.freeze([{ tick: 1n, weapon, activity: 2 }]) } as unknown as Snapshot
    expect(createViewmodelPresenter({ models } as unknown as PresentationArtifacts).map(snapshot).request)
      .toMatchObject({ model: hands, itemModel: item, activity })
  }
})

test("composes every Heavy stock weapon with distinct identities and hands-only Fists", () => {
  const hands = "models/weapons/c_models/c_heavy_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze([
    "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_SECONDARY_VM_PRIMARYATTACK", "ACT_FISTS_VM_HITLEFT",
  ].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  const items = [
    [9, "models/weapons/c_models/c_minigun/c_minigun.mdl", "ACT_PRIMARY_VM_PRIMARYATTACK", "Minigun"],
    [10, "models/weapons/c_models/c_shotgun/c_shotgun.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK", "Shotgun"],
    [11, undefined, "ACT_FISTS_VM_HITLEFT", "Fists"],
  ] as const
  for (const [, identity] of items) {
    if (identity !== undefined) models.set(identity, { identity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
  }
  const artifacts = { models } as unknown as PresentationArtifacts
  for (const [weapon, item, activity, name] of items) {
    const snapshot = {
      class: 6, team: 2, tick: 1n, weapon, health: 300, maximumHealth: 300,
      velocity: Object.freeze([0, 0, 0]), projectiles: Object.freeze([]),
      loadout: Object.freeze([{ weapon, reload: 0, clip: weapon === 10 ? 6 : 0 }]),
      activities: Object.freeze([{ tick: 1n, weapon, activity: 2 }]),
    } as unknown as Snapshot
    const request = createViewmodelPresenter(artifacts).map(snapshot).request
    expect(request.model).toBe(hands)
    expect(request.itemModel).toBe(item)
    expect(request.handsOnlyViewmodel).toBe(weapon === 11 ? true : undefined)
    expect(request.activity).toBe(activity)
    expect(tf2Hud(snapshot).weaponName).toBe(name)
  }
})

test("starts authored hitscan muzzle systems from exact fire-tick attachment transforms", () => {
  const position = Object.freeze([1, 2, 3]) as readonly [number, number, number]
  const orientation = Object.freeze([0, 0, 0, 1]) as readonly [number, number, number, number]
  const snapshot = { tick: 17n, team: 2, events: Object.freeze([{ kind: 12, detail: 4, subject: 10 }]) } as unknown as Snapshot
  const catalog = { systems: new Set(["muzzle_scattergun"]), attachmentTransforms: new Map([[4, new Map([["muzzle", { position, orientation }]])]]) }
  const requests = hitscanMuzzleParticles(snapshot, catalog)
  expect(requests).toHaveLength(1)
  expect(requests[0]).toMatchObject({ kind: "start", tick: 17n, system: "muzzle_scattergun", launcherIdentity: 4, team: "red", controlPoints: [{ position, orientation, ownerIdentity: 1 }] })
  expect(() => hitscanMuzzleParticles(snapshot, { ...catalog, systems: new Set() })).toThrow(ProjectilePresentationError)
  expect(() => hitscanMuzzleParticles(snapshot, { ...catalog, attachmentTransforms: new Map() })).toThrow(ProjectilePresentationError)
  for (const identity of [7, 10] as const) {
    const shotgun = { ...snapshot, events: Object.freeze([{ kind: 12, detail: identity, subject: 10 }]) } as unknown as Snapshot
    expect(hitscanMuzzleParticles(shotgun, {
      systems: new Set(["muzzle_shotgun"]),
      attachmentTransforms: new Map([[identity, new Map([["muzzle", { position, orientation }]])]]),
    })[0]).toMatchObject({ system: "muzzle_shotgun", launcherIdentity: identity })
  }
})
