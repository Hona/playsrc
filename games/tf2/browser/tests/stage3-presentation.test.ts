import { expect, test } from "bun:test"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import {
  createParticleBatchEncoder,
  createViewmodelPresenter,
  decodeModelPoseOutput,
  encodeModelPoseBatch,
  ProjectilePresentationError,
  hitscanMuzzleParticles,
  combatImpactParticles,
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
  expect(view.getUint32(4, true)).toBe(3)
  expect(view.getUint32(28, true)).toBe(1)
  expect(bytes[32]).toBe(1)
  expect(new TextDecoder().decode(bytes.subarray(68, 79))).toBe("rockettrail")
})

test("encodes both authored Medi Gun beam endpoints in one particle transaction", () => {
  const controls = Object.freeze([
    Object.freeze({ index: 0 as const, position: Object.freeze([1, 2, 3] as const), orientation: Object.freeze([0, 0, 0, 1] as const), ownerIdentity: 1 }),
    Object.freeze({ index: 1 as const, position: Object.freeze([100, 20, 68] as const), orientation: Object.freeze([0, 0, 0, 1] as const), ownerIdentity: 2 }),
  ])
  const start: ProjectileParticleRequest = Object.freeze({
    kind: "start", identity: "8:medic:1:2:start", effectIdentity: "medic:1:2", eventIdentity: "8:medic:1:2", tick: 8n,
    projectileIdentity: 2, ownerIdentity: 1, launcherIdentity: 20, team: "red", system: "medicgun_beam_red", attachment: null,
    controlPoints: controls,
  })
  const update: ProjectileParticleRequest = Object.freeze({
    kind: "set-control-point", identity: "9:medic:1:2:patient", effectIdentity: "medic:1:2", eventIdentity: "9:medic:1:2", tick: 9n,
    projectileIdentity: 2, controlPoint: controls[1]!,
  })
  const bytes = createParticleBatchEncoder().encode(9n, [0, 0, 0], [start, update])
  const view = new DataView(bytes.buffer)
  expect(view.getUint32(4, true)).toBe(3)
  expect(bytes[34]).toBe(2)
  const nameBytes = new TextEncoder().encode("medicgun_beam_red").length
  const patient = 68 + nameBytes + 32
  expect(view.getFloat32(patient, true)).toBe(100)
  expect(view.getUint32(patient + 28, true)).toBe(2)
  expect(bytes[patient + 34]).toBe(1)
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

test("encodes each Unicode model/activity exactly once into the snapshot-bound PMRQ v9 contract", () => {
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
  expect(view.getUint32(4, true)).toBe(9)
  expect(view.getUint32(8, true)).toBe(1)
  expect(view.getUint32(16, true)).toBe(0)
  expect(view.getBigUint64(44, true)).toBe(0n)
  expect(view.getUint32(84, true)).toBe(new TextEncoder().encode(request.model).byteLength)
  expect(new TextDecoder().decode(bytes.subarray(88, 88 + view.getUint32(84, true)))).toBe(request.model)
  const cloak = { identity: 42, localFactor: 0.5, worldFactor: 0.95, rawFactor: 1, playerTint: [0.4, 0.5, 1] as const }
  const encoded = new DataView(encodeModelPoseBatch([{ ...request, cloak }]).buffer)
  expect(encoded.getUint32(16, true)).toBe(42)
  expect(encoded.getFloat32(24, true)).toBe(Math.fround(0.95))
  expect(() => encodeModelPoseBatch([{ ...request, cloak: { ...cloak, identity: -1 } }])).toThrow("model actor identity")
})

test("keeps exact UTF-8 pose bytes across bounded cache eviction and repeated full-roster batches", () => {
  const request = Object.freeze({
    identity: 1,
    model: "models/player/雪.mdl",
    activity: "ACT_MP_RUN_PRIMARY",
    previousElapsedSeconds: 0,
    elapsedSeconds: 0.015,
    currentTimeSeconds: 1,
    frameTimeSeconds: 0.015,
    planarSpeed: 320,
    screenAspectRatio: 16 / 9,
    worldFarPlane: 32_768,
    skin: 0,
    lod: 0,
    bodygroups: Object.freeze([0, 1]),
    lighting: Object.freeze({
      origin: Object.freeze([1, 2, 3]) as readonly [number, number, number],
      angles: Object.freeze([4, 5, 6]) as readonly [number, number, number],
      cameraPosition: Object.freeze([7, 8, 9]) as readonly [number, number, number],
      cameraAngles: Object.freeze([10, 11, 12]) as readonly [number, number, number],
    }),
  })
  const initial = encodeModelPoseBatch([request])
  for (let index = 0; index < 300; index += 1) {
    encodeModelPoseBatch([{ ...request, model: `models/player/unique-${index}.mdl` }])
  }
  expect(encodeModelPoseBatch([request])).toEqual(initial)
  const roster = Array.from({ length: 24 }, (_, index) => ({ ...request, identity: index + 1 }))
  expect(encodeModelPoseBatch(roster)).toEqual(encodeModelPoseBatch(roster))
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
  expect(bytes[52]).toBe(2)
  const modelBytes = new TextEncoder().encode(request.model).byteLength
  expect(new DataView(bytes.buffer).getUint32(88 + modelBytes, true)).toBe(0)
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
  expect(view.getBigUint64(44, true)).toBe(18n)
  expect([...bytes.subarray(52, 56)]).toEqual([1, 1, 1, 0])
  expect([0, 1, 2].map((index) => view.getFloat32(56 + index * 4, true))).toEqual([10, 20, 30])
  expect(() => encodeModelPoseBatch([{ ...request, fireView: undefined }]))
    .toThrow(ProjectilePresentationError)
})

test("decodes compact authored PMPO bone matrices and rejects invalid or truncated matrices", () => {
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
  u32(10)
  u32(1)
  u32(9)
  output.push(1, 0, 1, 0)
  ;[0.5, 0.95, 1, 1, 0.5, 0.4].forEach(f32)
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
  ;[1, 0, 0, 4, 0, 1, 0, 5, 0, 0, 1, 6].forEach(f32)
  u32(1)
  u32(5)
  u32(6)
  u32(2)
  output.push(1, 0, 0, 0)
  u32(0)
  u32(0)
  u32(0)
  u32(0)
  const bytes = Uint8Array.from(output)
  const pose = decodeModelPoseOutput(bytes)[0]!
  expect(pose.sampleTick).toBe(7n)
  expect(pose.cloak?.worldFactor).toBe(Math.fround(0.95))
  expect(pose.cloak?.localFactor).toBe(0.5)
  expect(pose.cloak?.player).toBe(true)
  expect(pose.attachmentsOnly).toBe(false)
  const primitive = pose.primitives[0]!
  expect([...pose.boneMatrices]).toEqual([1, 0, 0, 4, 0, 1, 0, 5, 0, 0, 1, 6])
  expect(primitive.vertexCount).toBe(2)
  expect(primitive.translucent).toBe(true)
  const repeated = bytes.slice(12)
  new DataView(repeated.buffer).setUint32(0, 10, true)
  const shared = new Uint8Array(bytes.byteLength + repeated.byteLength)
  shared.set(bytes)
  shared.set(repeated, bytes.byteLength)
  new DataView(shared.buffer).setUint32(8, 2, true)
  const [first, second] = decodeModelPoseOutput(shared)
  expect(first!.events).toBe(second!.events)
  expect(first!.eyes).toBe(second!.eyes)
  expect(second!.identity).toBe(10)
  const memory = new SharedArrayBuffer(bytes.byteLength + 24)
  new Uint8Array(memory, 12, bytes.byteLength).set(bytes)
  const resident = decodeModelPoseOutput(new Uint8Array(memory, 12, bytes.byteLength))[0]!
  expect([...resident.boneMatrices]).toEqual([...pose.boneMatrices])
  expect(resident.primitives).toEqual(pose.primitives)
  expect(resident.model).toBe(pose.model)
  const invalid = bytes.slice()
  new DataView(invalid.buffer).setFloat32(invalid.byteLength - 76, Number.NaN, true)
  expect(() => decodeModelPoseOutput(invalid)).toThrow(ProjectilePresentationError)
  expect(() => decodeModelPoseOutput(bytes.subarray(0, bytes.byteLength - 1))).toThrow(ProjectilePresentationError)
})

test("uses the default TF2 horizontal-4:3 world projection and Source clip planes", () => {
  const snapshot = {
    position: Object.freeze([10, 20, 30]),
    viewAngleOffset: Object.freeze([0, 0, 0]),
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
  const snapped = { ...snapshot, viewAngleOffset: [-5, -150, 0] as const }
  expect(tf2Camera(snapped, 330, 0)).toMatchObject({ yawDegrees: 180, pitchDegrees: -5 })
  expect(tf2Camera(snapped, 335, 2)).toMatchObject({ yawDegrees: 185, pitchDegrees: -3 })
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

test("composes every Demoman stock item with authored primary, secondary, and Bottle activities", () => {
  const hands = "models/weapons/c_models/c_demo_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze([
    "ACT_PRIMARY_VM_DRAW", "ACT_PRIMARY_VM_PRIMARYATTACK",
    "ACT_SECONDARY_VM_DRAW", "ACT_SECONDARY_VM_PRIMARYATTACK",
    "ACT_MELEE_VM_DRAW", "ACT_MELEE_VM_HITCENTER",
  ].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  const items = [
    [18, "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl", "ACT_PRIMARY_VM_PRIMARYATTACK", "Grenade Launcher"],
    [3, "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK", "Stickybomb Launcher"],
    [17, "models/weapons/c_models/c_bottle/c_bottle.mdl", "ACT_MELEE_VM_HITCENTER", "Bottle"],
  ] as const
  for (const [, identity] of items) models.set(identity, { identity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
  const artifacts = { models } as unknown as PresentationArtifacts
  for (const [weapon, item, activity, name] of items) {
    const snapshot = { class: 4, team: 2, tick: 1n, weapon, health: 175, maximumHealth: 175, projectiles: Object.freeze([]), velocity: Object.freeze([0, 0, 0]), loadout: Object.freeze([{ weapon, reload: 0, clip: weapon === 17 ? 0 : 1 }]), activities: Object.freeze([{ tick: 1n, weapon, activity: 2 }]) } as unknown as Snapshot
    const request = createViewmodelPresenter(artifacts).map(snapshot).request
    expect(request).toMatchObject({ model: hands, itemModel: item, activity })
    expect(tf2Hud(snapshot).weaponName).toBe(name)
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

test("composes every Medic stock item without colliding with Heavy or Sniper identities", () => {
  const hands = "models/weapons/c_models/c_medic_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze([
    "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_SECONDARY_VM_PRIMARYATTACK", "ACT_MELEE_VM_HITCENTER",
  ].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  const items = [
    [19, "models/weapons/c_models/c_syringegun/c_syringegun.mdl", "ACT_PRIMARY_VM_PRIMARYATTACK", "Syringe Gun"],
    [20, "models/weapons/c_models/c_medigun/c_medigun.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK", "Medi Gun"],
    [21, "models/weapons/c_models/c_bonesaw/c_bonesaw.mdl", "ACT_MELEE_VM_HITCENTER", "Bonesaw"],
  ] as const
  for (const [, identity] of items) models.set(identity, { identity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
  const artifacts = { models } as unknown as PresentationArtifacts
  for (const [weapon, item, activity, name] of items) {
    const snapshot = {
      class: 5, team: 2, tick: 1n, weapon, health: 150, maximumHealth: 150,
      velocity: Object.freeze([0, 0, 0]), projectiles: Object.freeze([]),
      loadout: Object.freeze([{ weapon, reload: 0, clip: weapon === 19 ? 40 : 0 }]),
      activities: Object.freeze([{ tick: 1n, weapon, activity: 2 }]),
    } as unknown as Snapshot
    expect(createViewmodelPresenter(artifacts).map(snapshot).request)
      .toMatchObject({ model: hands, itemModel: item, activity })
    expect(tf2Hud(snapshot).weaponName).toBe(name)
  }
})

test("composes every Engineer stock item with exact primary, secondary, melee, PDA, and builder activities", () => {
  const hands = "models/weapons/c_models/c_engineer_arms.mdl"
  const descriptor = Object.freeze({ kind: "viewmodel", horizontalFov4By3: 54, minimumFov: 54, maximumFov: 70, near: 1, depthRange: Object.freeze([0, 0.1]), drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true })
  const sequences = Object.freeze([
    "ACT_PRIMARY_VM_DRAW", "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_PRIMARY_RELOAD_START",
    "ACT_SECONDARY_VM_DRAW", "ACT_SECONDARY_VM_PRIMARYATTACK", "ACT_SECONDARY_VM_RELOAD",
    "ACT_MELEE_VM_DRAW", "ACT_MELEE_VM_HITCENTER",
    "ACT_ENGINEER_PDA1_VM_DRAW", "ACT_ENGINEER_PDA2_VM_DRAW", "ACT_ENGINEER_BLD_VM_DRAW",
  ].map((activity) => ({ activity, durationSeconds: 0.8 })))
  const models = new Map([[hands, { identity: hands, bodygroupCounts: Object.freeze([]), descriptor, sequences }]])
  const items = [
    [40, "models/weapons/c_models/c_shotgun/c_shotgun.mdl", "ACT_PRIMARY_VM_PRIMARYATTACK"],
    [41, "models/weapons/c_models/c_pistol/c_pistol.mdl", "ACT_SECONDARY_VM_PRIMARYATTACK"],
    [42, "models/weapons/c_models/c_wrench/c_wrench.mdl", "ACT_MELEE_VM_HITCENTER"],
    [43, "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl", "ACT_ENGINEER_PDA1_VM_DRAW"],
    [44, "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl", "ACT_ENGINEER_PDA2_VM_DRAW"],
    [45, "models/weapons/c_models/c_toolbox/c_toolbox.mdl", "ACT_ENGINEER_BLD_VM_DRAW"],
  ] as const
  for (const [, identity] of items) models.set(identity, { identity, bodygroupCounts: Object.freeze([]), descriptor, sequences: Object.freeze([]) })
  const artifacts = { models } as unknown as PresentationArtifacts
  for (const [weapon, item, activity] of items) {
    const snapshot = { class: 9, team: 2, tick: 1n, weapon, velocity: Object.freeze([0, 0, 0]), loadout: Object.freeze([{ weapon, reload: 0, clip: weapon >= 42 ? 0 : 1 }]), activities: Object.freeze([{ tick: 1n, weapon, activity: weapon>=43?1:2 }]) } as unknown as Snapshot
    const request = createViewmodelPresenter(artifacts).map(snapshot).request
    expect(request.model).toBe(hands)
    expect(request.itemModel).toBe(item)
    expect(request.activity).toBe(activity)
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

test("joins Source tracer cadence, both endpoint control points, blood LOD and attacker-only crit effects", () => {
  const muzzle = Object.freeze([2, 3, 4]) as readonly [number, number, number]
  const orientation = Object.freeze([0, 0, 0, 1]) as readonly [number, number, number, number]
  const snapshot = Object.freeze({
    tick: 22n, team: 2, position: Object.freeze([0, 0, 0]), bots: Object.freeze([{ identity: 7, team: 3, position: Object.freeze([128, 0, 0]) }]),
    events: Object.freeze([
      Object.freeze({ kind: 12, detail: 4, subject: 10, auxiliary: 1, values: Object.freeze([0, 0, 64, 0]) }),
      Object.freeze({ kind: 13, detail: 4, subject: 7, auxiliary: 0x0001_0000, values: Object.freeze([128, 0, 64, 18]) }),
      Object.freeze({ kind: 13, detail: 4, subject: 0, auxiliary: 1, values: Object.freeze([256, 0, 64, 9]) }),
      Object.freeze({ kind: 17, detail: 4, subject: 7, auxiliary: 1, values: Object.freeze([18, 82, 1, 1]) }),
    ]),
  }) as unknown as Snapshot
  const systems = new Set(["bullet_scattergun_tracer01_red_crit", "blood_impact_red_01", "blood_spray_red_01", "crit_text"])
  const result = combatImpactParticles(snapshot, { tracerCount: 0 }, {
    systems,
    attachmentTransforms: new Map([[4, new Map([["muzzle", { position: muzzle, orientation }]])]]),
    playerAttachmentTransforms:new Map([[7,new Map([["head",{position:Object.freeze([128,0,72]) as readonly[number,number,number],orientation}]])]]),
  })
  expect(result.state.tracerCount).toBe(2)
  expect(result.particles.map(request => request.kind === "start" ? request.system : "")).toEqual([
    "bullet_scattergun_tracer01_red_crit", "blood_impact_red_01", "blood_spray_red_01", "crit_text",
  ])
  const tracer = result.particles[0]!
  expect(tracer.kind === "start" && tracer.controlPoints.map(point => point.position)).toEqual([muzzle, [128, 0, 64]])
  const encoded = createParticleBatchEncoder().encode(22n, [0, 0, 0], result.particles)
  expect(encoded[32]).toBe(1)
  expect(encoded[34]).toBe(2)
})
