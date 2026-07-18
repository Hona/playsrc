import type { ParticleRenderItem } from "@playsrc/particle"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import type { Camera, Effect } from "@playsrc/rendering"
import type { ModelItem } from "@playsrc/rendering"
import type { PresentationArtifacts } from "./artifacts"
import type { Snapshot } from "./codec"

const UINT32_MAX = 0xffff_ffff
const NORMAL_TOLERANCE = 1e-4
const TF2_DEFAULT_HORIZONTAL_FOV_4_BY_3 = 75
const SOURCE_WORLD_NEAR = 7
const SOURCE_MAP_EXTENT = 16_384
const SOURCE_MAP_EXTENT_DIAGONAL = Math.fround(1.73205080757)

export type PresentationDiagnostic = Readonly<{
  code: "MissingProjectileModel" | "MissingParticleContext" | "MissingAudioContext"
  identity: string
}>

export type Tf2Hud = Readonly<{
  health: number
  maxHealth: number
  className: "Soldier" | "Demoman"
  weaponName: "Rocket Launcher" | "Original" | "Stickybomb Launcher"
  speed: number
  projectileCount: number
}>

export type Tf2AudioRequest = Readonly<{
  voiceIdentity: number
  definition: "Weapon_RPG.Single" | "Weapon_QuakeRPG.Single" | "Weapon_StickyBombLauncher.Single" | "BaseExplosionEffect.Sound" | "Weapon_QuakeRPG.Explode" | "Weapon_Grenade_Pipebomb.Explode"
  source: Readonly<{
    kind: "entity" | "world"
    identity: number
    ownerIdentity: number | null
    origin: readonly [number, number, number]
    radius: number
    sourceClass: string
  }>
  samples: Readonly<{ volume: number; pitch: number; wave: number; soundLevel: number }>
}>

export function tf2Audio(snapshot: Snapshot): readonly Tf2AudioRequest[] {
  const definitions: readonly Tf2AudioRequest["definition"][] = [
    "Weapon_RPG.Single",
    "Weapon_QuakeRPG.Single",
    "Weapon_StickyBombLauncher.Single",
    "BaseExplosionEffect.Sound",
    "Weapon_QuakeRPG.Explode",
    "Weapon_Grenade_Pipebomb.Explode",
  ]
  return Object.freeze(snapshot.audioEvents.map((event) => Object.freeze({
    voiceIdentity: stable32(`${event.tick}:${event.ordinal}:${event.definition}:${event.sourceIdentity}`),
    definition: definitions[event.definition - 1]!,
    source: Object.freeze({
      kind: event.sourceKind === 1 ? "entity" as const : "world" as const,
      identity: event.sourceIdentity,
      ownerIdentity: event.ownerIdentity,
      origin: Object.freeze([...event.position]) as Vector3,
      radius: 0,
      sourceClass: event.sourceKind === 1 ? "tf_weapon" : "tf_projectile",
    }),
    samples: event.samples,
  })))
}

export function projectileFrame(snapshot: Snapshot): ProjectileFrame {
  return Object.freeze({
    tick: snapshot.tick,
    projectiles: Object.freeze(
      snapshot.projectiles.map((p) =>
        Object.freeze({
          identity: p.identity,
          kind: p.kind === 1 ? "rocket" : "sticky",
          team: p.team === 1 ? "red" : "blue",
          ownerIdentity: p.ownerIdentity,
          launcherIdentity: p.launcherIdentity,
          state: p.state === 1 ? "flying" : p.state === 2 ? "stuck-unarmed" : "stuck-armed",
          position: vector(p.position),
          velocity: vector(p.velocity),
          orientation: quat(p.orientation),
          angularVelocity: vector(p.angularVelocity),
          contactNormal: p.contactNormal === null ? null : vector(p.contactNormal),
          ageSeconds: p.ageSeconds,
        }),
      ),
    ),
    events: Object.freeze(
      snapshot.projectileEvents.map((e) =>
        Object.freeze({
          kind: e.type,
          projectileIdentity: e.projectile,
          ownerIdentity: e.ownerIdentity,
          launcherIdentity: e.launcherIdentity,
          team: e.team === 1 ? "red" : "blue",
          tick: e.tick,
          position: vector(e.position),
          orientation: quat(e.orientation),
          contactNormal: e.contactNormal === null ? null : vector(e.contactNormal),
        }),
      ),
    ),
  }) as ProjectileFrame
}
export function projectileModels(models: readonly ProjectileModelRequest[]): readonly ModelItem[] {
  return Object.freeze(
    models.map((m) =>
      Object.freeze({
        identity: m.identity,
        model: m.model,
        position: m.position,
        orientation: m.orientation,
        scale: 1,
        skin: m.skin,
      }),
    ),
  )
}
export function createViewmodelPresenter(artifacts: PresentationArtifacts) {
  let actionTick = 0n
  let priorTick = 0n
  let prior: Snapshot["weapon"] | undefined
  let priorClass: Snapshot["class"] | undefined
  let activity = "ACT_VM_DRAW"
  return Object.freeze({
    map(snapshot: Snapshot,view:Readonly<{aspectRatio:number;farPlane:number}>=Object.freeze({aspectRatio:4/3,farPlane:32768})): Readonly<{ item: ModelItem; request: ModelPoseRequest }> {
      const identity =
        snapshot.class === 1
          ? "models/weapons/c_models/c_soldier_arms.mdl"
          : "models/weapons/c_models/c_demo_arms.mdl"
      const itemIdentity = snapshot.class === 1
        ? "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
        : "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl"
      const artifact = artifacts.models.get(identity)
      const itemArtifact = artifacts.models.get(itemIdentity)
      if (!artifact) throw new ProjectilePresentationError("MissingModel", identity)
      if (!itemArtifact) throw new ProjectilePresentationError("MissingModel", itemIdentity)
      if (artifact.descriptor.kind !== "viewmodel") throw new ProjectilePresentationError("MissingModel", `${identity}:descriptor`)
      if (itemArtifact.descriptor.kind !== "viewmodel") throw new ProjectilePresentationError("MissingModel", `${itemIdentity}:descriptor`)
      const weapon = snapshot.loadout.find((value) => value.weapon === snapshot.weapon)
      if (!weapon) throw new ProjectilePresentationError("MissingModel", `${identity}:weapon-state`)
      const selectionChanged = prior !== snapshot.weapon || priorClass !== snapshot.class
      const exact = snapshot.activities.filter((event) => event.weapon === snapshot.weapon).at(-1)
      const role = snapshot.class === 1 ? "PRIMARY" : "SECONDARY"
      const mapped = exact === undefined ? undefined : [
        "",
        `ACT_${role}_VM_DRAW`,
        `ACT_${role}_VM_PRIMARYATTACK`,
        `ACT_${role}_RELOAD_START`,
        `ACT_${role}_VM_RELOAD`,
        `ACT_${role}_RELOAD_FINISH`,
        `ACT_${role}_VM_IDLE`,
      ][exact.activity]
      let nextActivity = mapped ?? (selectionChanged ? `ACT_${role}_VM_DRAW` : activity)
      let selected = artifact.sequences.find((value) => value.activity === nextActivity)
      if (!selected) throw new ProjectilePresentationError("MissingModel", `${identity}:${activity}`)
      const elapsed = Number(snapshot.tick - actionTick) * 0.015
      if (!selectionChanged && exact === undefined && weapon.reload === 0 && nextActivity === activity && nextActivity !== `ACT_${role}_VM_IDLE` && elapsed >= selected.durationSeconds) {
        nextActivity = `ACT_${role}_VM_IDLE`
        selected = artifact.sequences.find((value) => value.activity === nextActivity)
        if (!selected) throw new ProjectilePresentationError("MissingModel", `${identity}:${nextActivity}`)
      }
      if (exact !== undefined) actionTick = exact.tick
      else if (nextActivity !== activity || selectionChanged) actionTick = snapshot.tick
      activity = nextActivity
      prior = snapshot.weapon
      priorClass = snapshot.class
      const currentElapsed = Number(snapshot.tick - actionTick) * 0.015
      const previousElapsed = Math.max(0, Number(priorTick - actionTick) * 0.015)
      const frameTime=Math.max(0,Number(snapshot.tick-priorTick)*0.015)
      priorTick = snapshot.tick
      const now=Number(snapshot.tick)*0.015
      return Object.freeze({
        item: Object.freeze({
          identity: 0x7fff_ff00 + snapshot.class * 4,
          model: identity,
          position:Object.freeze([0,0,0]) as Vector3,angles:Object.freeze([0,0,0]) as Vector3,
          scale: 1,
          skin: snapshot.team === 1 ? 0 : 1,
          viewModel: true,
          viewModelProjection: artifact.descriptor,
        }),
        request: Object.freeze({
          identity: 0x7fff_ff00 + snapshot.class * 4,
          model: identity,
          itemModel: itemIdentity,
          activity,
          previousElapsedSeconds: Math.min(previousElapsed, currentElapsed),
          elapsedSeconds: currentElapsed,
          currentTimeSeconds:now,frameTimeSeconds:frameTime,planarSpeed:Math.hypot(snapshot.velocity[0],snapshot.velocity[1]),screenAspectRatio:view.aspectRatio,worldFarPlane:view.farPlane,
          skin: snapshot.team === 1 ? 0 : 1,
          lod: 0,
          bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
          itemBodygroups: Object.freeze(itemArtifact.bodygroupCounts.map(() => 0)),
        }),
      })
    },
  })
}
function stable32(value: string) {
  let hash = 0x811c9dc5
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= byte
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0 || 1
}
function stable64(value: string) {
  let hash = 0xcbf29ce484222325n
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= BigInt(byte)
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash || 1n
}

export type ModelPoseRequest = Readonly<{
  identity: number
  model: string
  itemModel?: string
  activity: string
  previousElapsedSeconds: number
  elapsedSeconds: number
  currentTimeSeconds:number;frameTimeSeconds:number;planarSpeed:number;screenAspectRatio:number;worldFarPlane:number
  skin: number
  lod: number
  bodygroups: readonly number[]
  itemBodygroups?: readonly number[]
}>
export type PosedPrimitive = Readonly<{
  primitive: number
  material: number
  positions: Float32Array
  normals: Float32Array
  tangents: Float32Array
  translucent:boolean
}>
export type PosedAttachment = Readonly<{ name: string; worldAligned: boolean; matrix: Float32Array }>
export type PosedModel = Readonly<{
  identity: number
  role: "single" | "hand" | "item"
  model: string
  activity: string
  sequence: number
  framesPerSecond: number
  weightedFrameCount: number
  cyclesPerSecond: number
  durationSeconds: number
  looping: boolean
  previousCycle: number
  cycle: number
  events: readonly Readonly<{ index: number; cycle: number; event: number; eventType: number; options: Uint8Array; name: string }>[]
  primitives: readonly PosedPrimitive[]
  attachments: readonly PosedAttachment[]
  viewmodel:null|Readonly<{transform:Readonly<{origin:Vector3;angles:Vector3}>;projection:Readonly<{unscaledHorizontalFov4By3:number;horizontalFov:number;aspectRatio:number;near:number;far:number}>;depthRange:readonly[number,number];restoredDepthRange:readonly[number,number];passRestored:boolean;depthRestored:boolean;itemTranslucent:boolean}>
}>

export function encodeModelPoseBatch(requests: readonly ModelPoseRequest[]): Uint8Array {
  if (requests.length > 128) throw new ProjectilePresentationError("BoundExceeded", "model pose request count")
  const encoder = new TextEncoder()
  let length = 12
  for (const request of requests) length += 64 + encoder.encode(request.model).length + encoder.encode(request.itemModel ?? "").length +
    encoder.encode(request.activity).length + (request.bodygroups.length + (request.itemBodygroups?.length ?? 0)) * 4
  if (length > 1024 * 1024) throw new ProjectilePresentationError("BoundExceeded", "model pose request bytes")
  const bytes = new Uint8Array(length), view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x4d, 0x52, 0x51])
  view.setUint32(4, 3, true)
  view.setUint32(8, requests.length, true)
  let at = 12
  const text = (value: string) => {
    const encoded = encoder.encode(value)
    view.setUint32(at, encoded.length, true); at += 4; bytes.set(encoded, at); at += encoded.length
  }
  for (const request of requests) {
    if (!Number.isSafeInteger(request.identity) || request.identity < 1 || !request.model || !request.activity ||
      ![request.previousElapsedSeconds, request.elapsedSeconds].every(Number.isFinite) || request.previousElapsedSeconds < 0 ||
      request.elapsedSeconds < request.previousElapsedSeconds || ![request.skin, request.lod, ...request.bodygroups, ...(request.itemBodygroups ?? [])].every((value) => Number.isSafeInteger(value) && value >= 0) ||
      ((request.itemModel === undefined) !== (request.itemBodygroups === undefined)) || request.itemModel === "") {
      throw new ProjectilePresentationError("MalformedFact", "model pose request")
    }
    view.setUint32(at, request.identity, true); at += 4
    bytes[at] = request.itemModel === undefined ? 0 : 1; at += 4
    text(request.model); text(request.itemModel ?? ""); text(request.activity)
    view.setFloat32(at, request.previousElapsedSeconds, true); at += 4
    view.setFloat32(at, request.elapsedSeconds, true); at += 4
    for(const value of [request.currentTimeSeconds,request.frameTimeSeconds,request.planarSpeed,request.screenAspectRatio,request.worldFarPlane]){view.setFloat32(at,value,true);at+=4}
    view.setUint32(at, request.skin, true); at += 4
    view.setUint32(at, request.lod, true); at += 4
    view.setUint32(at, request.bodygroups.length, true); at += 4
    for (const value of request.bodygroups) { view.setUint32(at, value, true); at += 4 }
    view.setUint32(at, request.itemBodygroups?.length ?? 0, true); at += 4
    for (const value of request.itemBodygroups ?? []) { view.setUint32(at, value, true); at += 4 }
  }
  return bytes
}

export function decodeModelPoseOutput(bytes: Uint8Array): readonly PosedModel[] {
  if (bytes.byteLength < 12 || bytes.byteLength > 64 * 1024 * 1024) throw new ProjectilePresentationError("BoundExceeded", "model pose output bytes")
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), decoder = new TextDecoder("utf-8", { fatal: true })
  if (decoder.decode(bytes.subarray(0, 4)) !== "PMPO" || view.getUint32(4, true) !== 3) throw new ProjectilePresentationError("MalformedFact", "model pose output identity")
  let at = 12
  const ensure = (length: number) => { if (at + length > bytes.length) throw new ProjectilePresentationError("MalformedFact", "model pose output truncation") }
  const u8 = () => { ensure(1); return bytes[at++]! }, u32 = () => { ensure(4); const value = view.getUint32(at, true); at += 4; return value },
    i32 = () => { ensure(4); const value = view.getInt32(at, true); at += 4; return value },
    f32 = () => { ensure(4); const value = view.getFloat32(at, true); at += 4; if (!Number.isFinite(value)) throw new ProjectilePresentationError("MalformedFact", "model pose scalar"); return value },
    text = () => { const length = u32(); ensure(length); const value = decoder.decode(bytes.subarray(at, at + length)); at += length; return value }
  const output: PosedModel[] = []
  for (let count = view.getUint32(8, true); count > 0; count--) {
    const identity = u32(), roleCode = u8()
    if (roleCode > 2 || u8() || u8() || u8()) throw new ProjectilePresentationError("MalformedFact", "model pose role")
    const model = text(), activity = text(), sequence = u32(), framesPerSecond = f32(), weightedFrameCount = f32(),
      cyclesPerSecond = f32(), durationSeconds = f32(), looping = u8()
    if (looping > 1 || u8() || u8() || u8()) throw new ProjectilePresentationError("MalformedFact", "model pose timing")
    const previousCycle = f32(), cycle = f32()
    const present=u8();if(present>1||u8()||u8()||u8())throw new ProjectilePresentationError("MalformedFact","viewmodel state");const values=Array.from({length:15},f32),passRestored=u8(),depthRestored=u8(),itemTranslucent=u8();if(u8())throw new ProjectilePresentationError("MalformedFact","viewmodel flags");const viewmodel=present===0?null:Object.freeze({transform:Object.freeze({origin:vector(values.slice(0,3)),angles:vector(values.slice(3,6))}),projection:Object.freeze({unscaledHorizontalFov4By3:values[6]!,horizontalFov:values[7]!,aspectRatio:values[8]!,near:values[9]!,far:values[10]!}),depthRange:Object.freeze(values.slice(11,13)) as readonly[number,number],restoredDepthRange:Object.freeze(values.slice(13,15)) as readonly[number,number],passRestored:passRestored===1,depthRestored:depthRestored===1,itemTranslucent:itemTranslucent===1})
    const events = Object.freeze(Array.from({ length: u32() }, () => {
      const index = u32(), eventCycle = f32(), event = i32(), eventType = i32(); ensure(64)
      const options = bytes.slice(at, at + 64); at += 64
      return Object.freeze({ index, cycle: eventCycle, event, eventType, options, name: text() })
    }))
    const primitives = Object.freeze(Array.from({ length: u32() }, () => {
      const primitive=u32(),material=u32(),vertices=u32(),translucent=u8();if(translucent>1||u8()||u8()||u8())throw new ProjectilePresentationError("MalformedFact","primitive opacity");const positions = new Float32Array(vertices * 3),
        normals = new Float32Array(vertices * 3), tangents = new Float32Array(vertices * 4)
      for (let vertex = 0; vertex < vertices; vertex++) {
        for (let axis = 0; axis < 3; axis++) positions[vertex * 3 + axis] = f32()
        for (let axis = 0; axis < 3; axis++) normals[vertex * 3 + axis] = f32()
        for (let axis = 0; axis < 4; axis++) tangents[vertex * 4 + axis] = f32()
      }
      return Object.freeze({ primitive, material, positions, normals, tangents,translucent:translucent===1 })
    }))
    const attachments = Object.freeze(Array.from({ length: u32() }, () => {
      const name = text(), worldAligned = u8(); if (worldAligned > 1 || u8() || u8() || u8()) throw new ProjectilePresentationError("MalformedFact", "model attachment")
      const matrix = new Float32Array(12); for (let index = 0; index < 12; index++) matrix[index] = f32()
      return Object.freeze({ name, worldAligned: worldAligned === 1, matrix })
    }))
    output.push(Object.freeze({ identity, role: (["single", "hand", "item"] as const)[roleCode]!, model, activity, sequence, framesPerSecond, weightedFrameCount, cyclesPerSecond, durationSeconds, looping: looping === 1, previousCycle, cycle, events, primitives, attachments,viewmodel }))
  }
  if (at !== bytes.length) throw new ProjectilePresentationError("MalformedFact", "model pose output trailing bytes")
  return Object.freeze(output)
}
export function createParticleBatchEncoder() {
  let from = 0
  return Object.freeze({
    encode(tick: bigint, camera: Vector3, requests: readonly ProjectileParticleRequest[]) {
      const to = Number(tick) * 0.015
      if (to < from) throw new ProjectilePresentationError("TimeReversed", "particle time reversed")
      let length = 32
      for (const r of requests) {
        length += 20
        if (r.kind === "start") length += 8 + 4 + 4 + new TextEncoder().encode(r.system).length + 32
        else if (r.kind === "set-control-point") length += 32
      }
      const bytes = new Uint8Array(length),
        view = new DataView(bytes.buffer),
        encoder = new TextEncoder()
      bytes.set([0x50, 0x50, 0x54, 0x58])
      view.setUint32(4, 1, true)
      view.setFloat32(8, from, true)
      view.setFloat32(12, to, true)
      camera.forEach((v, i) => view.setFloat32(16 + i * 4, v, true))
      view.setUint32(28, requests.length, true)
      let at = 32
      for (const r of requests) {
        bytes[at] = r.kind === "start" ? 1 : r.kind === "set-control-point" ? 2 : 3
        view.setBigUint64(at + 4, stable64(r.identity), true)
        view.setFloat32(at + 12, to, true)
        view.setUint32(at + 16, stable32(r.effectIdentity), true)
        at += 20
        if (r.kind === "start") {
          view.setBigUint64(at, stable64(r.eventIdentity), true)
          view.setUint32(at + 8, r.ownerIdentity, true)
          const text = encoder.encode(r.system)
          view.setUint32(at + 12, text.length, true)
          bytes.set(text, at + 16)
          at += 16 + text.length
          const cp = r.controlPoints[0]!
          cp.position.forEach((v, i) => view.setFloat32(at + i * 4, v, true))
          cp.orientation.forEach((v, i) => view.setFloat32(at + 12 + i * 4, v, true))
          view.setUint32(at + 28, cp.ownerIdentity, true)
          at += 32
        } else if (r.kind === "set-control-point") {
          const cp = r.controlPoint
          cp.position.forEach((v, i) => view.setFloat32(at + i * 4, v, true))
          cp.orientation.forEach((v, i) => view.setFloat32(at + 12 + i * 4, v, true))
          view.setUint32(at + 28, cp.ownerIdentity, true)
          at += 32
        }
      }
      from = to
      return bytes
    },
  })
}

export function tf2Hud(snapshot: Snapshot): Tf2Hud {
  return Object.freeze({
    health: snapshot.health,
    maxHealth: snapshot.class === 1 ? 200 : 175,
    className: snapshot.class === 1 ? "Soldier" : "Demoman",
    weaponName: snapshot.weapon === 1 ? "Rocket Launcher" : snapshot.weapon === 2 ? "Original" : "Stickybomb Launcher",
    speed: Math.hypot(...snapshot.velocity),
    projectileCount: snapshot.projectiles.length,
  })
}

export function tf2Camera(snapshot: Snapshot, yawDegrees: number, pitchDegrees: number): Camera {
  return Object.freeze({
    position: Object.freeze([
      snapshot.position[0] + snapshot.movement.viewOffset[0],
      snapshot.position[1] + snapshot.movement.viewOffset[1],
      snapshot.position[2] + snapshot.movement.viewOffset[2],
    ]) as readonly [number, number, number],
    yawDegrees,
    pitchDegrees,
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(TF2_DEFAULT_HORIZONTAL_FOV_4_BY_3),
    near: SOURCE_WORLD_NEAR,
    far: Math.fround(SOURCE_MAP_EXTENT * SOURCE_MAP_EXTENT_DIAGONAL),
  })
}

export function particleEffects(items: readonly ParticleRenderItem[]): readonly Effect[] {
  return Object.freeze(
    items.map((item) =>
      Object.freeze({
        identity: item.identity,
        position: item.position,
        radius: item.radius,
        color: item.color,
        opacity: item.opacity,
      }),
    ),
  )
}

export type ProjectileKind = "rocket" | "sticky"
export type ProjectileTeam = "red" | "blue"
export type ProjectileState = "flying" | "stuck-unarmed" | "stuck-armed"
export type ProjectileEventKind = "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode"
export type Vector3 = readonly [number, number, number]
export type Quaternion = readonly [number, number, number, number]

export type ProjectileFact = Readonly<{
  identity: number
  kind: ProjectileKind
  team: ProjectileTeam
  ownerIdentity: number
  launcherIdentity: number
  state: ProjectileState
  position: Vector3
  velocity: Vector3
  orientation: Quaternion
  angularVelocity: Vector3
  contactNormal: Vector3 | null
  ageSeconds: number
}>

export type ProjectileEvent = Readonly<{
  kind: ProjectileEventKind
  projectileIdentity: number
  ownerIdentity: number
  launcherIdentity: number
  team: ProjectileTeam
  tick: bigint
  position: Vector3
  orientation: Quaternion
  contactNormal: Vector3 | null
}>

export type ProjectileFrame = Readonly<{
  tick: bigint
  projectiles: readonly ProjectileFact[]
  events: readonly ProjectileEvent[]
}>

export type ProjectileModelRequest = Readonly<{
  identity: number
  projectileIdentity: number
  model: "models/weapons/w_models/w_rocket.mdl" | "models/weapons/w_models/w_stickybomb.mdl"
  skin: 0 | 1
  materialVariant: ProjectileTeam
  position: Vector3
  orientation: Quaternion
  angularVelocity: Vector3
  state: ProjectileState
}>

export type ParticleAttachment = Readonly<{
  entityIdentity: number
  name: "backblast" | "muzzle" | "trail"
}>
export type AttachmentTransform = Readonly<{ position: Vector3; orientation: Quaternion }>

export type ParticleControlPoint = Readonly<{
  index: 0
  position: Vector3
  orientation: Quaternion
  ownerIdentity: number
}>

export type ProjectileParticleRequest =
  | Readonly<{
      kind: "start"
      identity: string
      effectIdentity: string
      eventIdentity: string
      projectileIdentity: number
      ownerIdentity: number
      launcherIdentity: number
      team: ProjectileTeam
      system: string
      attachment: ParticleAttachment | null
      controlPoints: readonly ParticleControlPoint[]
    }>
  | Readonly<{
      kind: "set-control-point"
      identity: string
      effectIdentity: string
      eventIdentity: string
      projectileIdentity: number
      controlPoint: ParticleControlPoint
    }>
  | Readonly<{
      kind: "stop"
      identity: string
      effectIdentity: string
      eventIdentity: string
      projectileIdentity: number
      immediate: boolean
    }>

export type ProjectileResourceCatalog = Readonly<{
  models: ReadonlySet<string>
  systems: ReadonlySet<string>
  attachments: ReadonlyMap<number, ReadonlySet<string>>
  attachmentTransforms?: ReadonlyMap<number, ReadonlyMap<string, AttachmentTransform>>
  localOwnerIdentity?: number
}>

export type ProjectilePresentationLimits = Readonly<{
  maxProjectiles: number
  maxEvents: number
  maxRequests: number
}>

export class ProjectilePresentationError extends Error {
  constructor(
    readonly code:
      | "MalformedFact"
      | "MalformedEvent"
      | "IllegalTransition"
      | "MissingModel"
      | "MissingSystem"
      | "MissingAttachment"
      | "BoundExceeded"
      | "TimeReversed",
    message: string,
  ) {
    super(message)
    this.name = "ProjectilePresentationError"
  }
}

type TrackedProjectile = Readonly<{
  kind: ProjectileKind
  team: ProjectileTeam
  ownerIdentity: number
  launcherIdentity: number
  state: ProjectileState
  trailActive: boolean
  pulseActive: boolean
}>

const DEFAULT_LIMITS: ProjectilePresentationLimits = Object.freeze({
  maxProjectiles: 4_096,
  maxEvents: 4_096,
  maxRequests: 16_384,
})

export function createProjectilePresentationMapper(
  catalog: ProjectileResourceCatalog,
  limits: ProjectilePresentationLimits = DEFAULT_LIMITS,
): Readonly<{
  map(frame: ProjectileFrame): Readonly<{
    models: readonly ProjectileModelRequest[]
    particles: readonly ProjectileParticleRequest[]
    events: readonly ProjectileEvent[]
  }>
  reset(tick: bigint): void
  dispose(): void
}> {
  validateLimits(limits)
  let tick = 0n
  let tracked = new Map<number, TrackedProjectile>()
  let disposed = false

  return Object.freeze({
    map(frame: ProjectileFrame) {
      if (disposed) throw new ProjectilePresentationError("IllegalTransition", "mapper is disposed")
      if (typeof frame.tick !== "bigint" || frame.tick < tick) {
        throw new ProjectilePresentationError("TimeReversed", "projectile frame tick moved backward")
      }
      if (frame.projectiles.length > limits.maxProjectiles || frame.events.length > limits.maxEvents) {
        throw new ProjectilePresentationError("BoundExceeded", "projectile frame exceeds its count limit")
      }
      const next = new Map(tracked)
      const facts = new Map<number, ProjectileFact>()
      for (const fact of frame.projectiles) {
        validateFact(fact)
        if (facts.has(fact.identity)) {
          throw new ProjectilePresentationError("MalformedFact", "projectile identity is duplicated")
        }
        facts.set(fact.identity, fact)
      }

      const particles: ProjectileParticleRequest[] = []
      for (let eventIndex = 0; eventIndex < frame.events.length; eventIndex += 1) {
        const event = frame.events[eventIndex]!
        validateEvent(event, frame.tick)
        const fact = facts.get(event.projectileIdentity)
        const prior = next.get(event.projectileIdentity)
        const eventIdentity = `${event.tick}:${eventIndex}:${event.kind}:${event.projectileIdentity}`
        if (event.kind === "fire") {
          if (prior || !fact) {
            throw transition("fire requires one new projectile fact")
          }
          matchEvent(event, fact)
          const trail = trailSystem(fact)
          requireSystem(catalog, trail)
          const trailAttachment = fact.kind === "rocket" ? requireAttachment(catalog, fact.identity, "trail") : null
          const trailTransform = trailAttachment ? requireAttachmentTransform(catalog, trailAttachment) : null
          push(
            particles,
            limits,
            startRequest(
              eventIdentity,
              trailEffect(fact.identity),
              fact,
              trail,
              trailAttachment,
              trailTransform?.position ?? event.position,
              trailTransform?.orientation ?? event.orientation,
            ),
          )
          const muzzle = fact.kind === "rocket" ? "rocketbackblast" : "muzzle_pipelauncher"
          if (!(fact.kind === "rocket" && fact.ownerIdentity === catalog.localOwnerIdentity)) {
            const attachment = requireAttachment(
              catalog,
              fact.launcherIdentity,
              fact.kind === "rocket" ? "backblast" : "muzzle",
            )
            const muzzleTransform = catalog.attachmentTransforms?.get(attachment.entityIdentity)?.get(attachment.name)
            if (muzzleTransform) {
              requireSystem(catalog, muzzle)
              push(
                particles,
                limits,
                startRequest(
                  eventIdentity,
                  oneShotEffect(eventIdentity),
                  fact,
                  muzzle,
                  attachment,
                  muzzleTransform.position,
                  muzzleTransform.orientation,
                ),
              )
            }
          }
          next.set(
            fact.identity,
            Object.freeze({
              kind: fact.kind,
              team: fact.team,
              ownerIdentity: fact.ownerIdentity,
              launcherIdentity: fact.launcherIdentity,
              state: "flying",
              trailActive: true,
              pulseActive: false,
            }),
          )
          continue
        }
        if (!prior) throw transition(`${event.kind} targets an unknown projectile`)
        if (fact) matchEvent(event, fact)
        matchTracked(event, prior)
        if (event.kind === "impact") {
          if (prior.state !== "flying") throw transition("impact requires a flying projectile")
        } else if (event.kind === "stick") {
          if (prior.kind !== "sticky" || prior.state !== "flying" || !fact) {
            throw transition("stick requires a flying sticky fact")
          }
          push(
            particles,
            limits,
            Object.freeze({
              kind: "set-control-point",
              identity: `${eventIdentity}:trail-contact`,
              effectIdentity: trailEffect(event.projectileIdentity),
              eventIdentity,
              projectileIdentity: event.projectileIdentity,
              controlPoint: controlPoint(
                event.position,
                settledOrientation(event.orientation, event.contactNormal),
                event.ownerIdentity,
              ),
            }),
          )
          next.set(event.projectileIdentity, Object.freeze({ ...prior, state: fact.state }))
        } else if (event.kind === "arm") {
          if (prior.kind !== "sticky" || !["flying","stuck-unarmed","stuck-armed"].includes(prior.state) || !fact) {
            throw transition("arm requires a stuck-unarmed sticky fact")
          }
          const system = fact.team === "red" ? "stickybomb_pulse_red" : "stickybomb_pulse_blue"
          requireSystem(catalog, system)
          push(
            particles,
            limits,
            startRequest(
              eventIdentity,
              pulseEffect(fact.identity),
              fact,
              system,
              null,
              event.position,
              event.contactNormal===null?event.orientation:settledOrientation(event.orientation,event.contactNormal),
            ),
          )
          next.set(event.projectileIdentity, Object.freeze({ ...prior, state: fact.state, pulseActive: true }))
        } else if (event.kind === "fizzle" || event.kind === "explode") {
          if (prior.trailActive)
            push(
              particles,
              limits,
              stopRequest(eventIdentity, trailEffect(event.projectileIdentity), event.projectileIdentity),
            )
          if (prior.pulseActive)
            push(
              particles,
              limits,
              stopRequest(eventIdentity, pulseEffect(event.projectileIdentity), event.projectileIdentity),
            )
          if (event.kind === "explode") {
            const system = event.contactNormal === null ? "ExplosionCore_MidAir" : "ExplosionCore_Wall"
            requireSystem(catalog, system)
            const eventFact: ProjectileFact =
              fact ??
              Object.freeze({
                identity: event.projectileIdentity,
                kind: prior.kind,
                team: prior.team,
                ownerIdentity: prior.ownerIdentity,
                launcherIdentity: prior.launcherIdentity,
                state: prior.state,
                position: event.position,
                velocity: Object.freeze([0, 0, 0]) as Vector3,
                orientation: event.orientation,
                angularVelocity: Object.freeze([0, 0, 0]) as Vector3,
                contactNormal: event.contactNormal,
                ageSeconds: 0,
              })
            push(
              particles,
              limits,
              startRequest(
                eventIdentity,
                oneShotEffect(eventIdentity),
                eventFact,
                system,
                null,
                event.position,
                event.contactNormal === null
                  ? event.orientation
                  : settledOrientation(event.orientation, event.contactNormal),
              ),
            )
          }
          next.delete(event.projectileIdentity)
        }
      }

      for (const [identity, state] of next) {
        const fact = facts.get(identity)
        if (!fact || fact.state !== state.state) {
          throw transition("projectile disappearance or state change has no ordered event")
        }
        if (
          fact.kind !== state.kind ||
          fact.team !== state.team ||
          fact.ownerIdentity !== state.ownerIdentity ||
          fact.launcherIdentity !== state.launcherIdentity
        ) {
          throw transition("projectile immutable identity fields changed")
        }
        if (state.trailActive) {
          const attachment = fact.kind === "rocket" ? requireAttachment(catalog, fact.identity, "trail") : null
          const transform = attachment ? requireAttachmentTransform(catalog, attachment) : Object.freeze({ position: fact.position, orientation: fact.orientation })
          push(
            particles,
            limits,
            Object.freeze({
              kind: "set-control-point",
              identity: `${frame.tick}:follow:${identity}`,
              effectIdentity: trailEffect(identity),
              eventIdentity: `${frame.tick}:follow`,
              projectileIdentity: identity,
              controlPoint: controlPoint(transform.position, transform.orientation, fact.ownerIdentity),
            }),
          )
        }
      }
      for (const identity of facts.keys()) {
        if (!next.has(identity)) throw transition("projectile fact has no fire event")
      }

      const models = frame.projectiles.map((fact) => {
        const model =
          fact.kind === "rocket"
            ? ("models/weapons/w_models/w_rocket.mdl" as const)
            : ("models/weapons/w_models/w_stickybomb.mdl" as const)
        if (!catalog.models.has(model)) {
          throw new ProjectilePresentationError("MissingModel", `projectile model ${model} is missing`)
        }
        const orientation =
          fact.kind === "sticky" && fact.state !== "flying"
            ? settledOrientation(fact.orientation, fact.contactNormal)
            : authoredRocketOrientation(fact.orientation)
        return Object.freeze({
          identity: fact.identity,
          projectileIdentity: fact.identity,
          model,
          skin: fact.team === "red" ? (0 as const) : (1 as const),
          materialVariant: fact.team,
          position: vector(fact.position),
          orientation,
          angularVelocity: vector(fact.angularVelocity),
          state: fact.state,
        })
      })
      tracked = next
      tick = frame.tick
      return Object.freeze({
        models: Object.freeze(models),
        particles: Object.freeze(particles),
        events: Object.freeze(
          frame.events.map((event) =>
            Object.freeze({
              ...event,
              position: vector(event.position),
              orientation: quat(event.orientation),
              contactNormal: event.contactNormal === null ? null : vector(event.contactNormal),
            }),
          ),
        ),
      })
    },
    reset(nextTick: bigint): void {
      if (disposed || typeof nextTick !== "bigint" || nextTick < 0n) {
        throw new ProjectilePresentationError("TimeReversed", "projectile mapper reset tick is invalid")
      }
      tick = nextTick
      tracked = new Map()
    },
    dispose(): void {
      disposed = true
      tracked.clear()
    },
  })
}

function validateFact(fact: ProjectileFact): void {
  if (
    !uint32(fact.identity) ||
    !uint32(fact.ownerIdentity) ||
    !uint32(fact.launcherIdentity) ||
    (fact.kind !== "rocket" && fact.kind !== "sticky") ||
    (fact.team !== "red" && fact.team !== "blue") ||
    (fact.state !== "flying" && fact.state !== "stuck-unarmed" && fact.state !== "stuck-armed") ||
    !finite(fact.position) ||
    !finite(fact.velocity) ||
    !quaternion(fact.orientation) ||
    !finite(fact.angularVelocity) ||
    !Number.isFinite(fact.ageSeconds) ||
    fact.ageSeconds < 0 ||
    (fact.contactNormal !== null && !normal(fact.contactNormal)) ||
    (fact.state === "flying" && fact.contactNormal !== null) ||
    (fact.state !== "flying" && fact.contactNormal === null) ||
    (fact.kind === "rocket" && fact.state !== "flying")
  ) {
    throw new ProjectilePresentationError("MalformedFact", "projectile fact violates the frozen contract")
  }
}

function validateEvent(event: ProjectileEvent, frameTick: bigint): void {
  if (
    !["fire", "impact", "stick", "arm", "fizzle", "explode"].includes(event.kind) ||
    !uint32(event.projectileIdentity) ||
    !uint32(event.ownerIdentity) ||
    !uint32(event.launcherIdentity) ||
    (event.team !== "red" && event.team !== "blue") ||
    typeof event.tick !== "bigint" ||
    event.tick < 0n ||
    event.tick > frameTick ||
    !finite(event.position) ||
    !quaternion(event.orientation) ||
    (event.contactNormal !== null && !normal(event.contactNormal)) ||
    (event.kind === "fire" && event.contactNormal !== null) ||
    (event.kind === "stick" && event.contactNormal === null)
  ) {
    throw new ProjectilePresentationError("MalformedEvent", "projectile event violates the frozen contract")
  }
}

function matchEvent(event: ProjectileEvent, fact: ProjectileFact): void {
  if (
    event.ownerIdentity !== fact.ownerIdentity ||
    event.launcherIdentity !== fact.launcherIdentity ||
    event.team !== fact.team
  )
    throw transition("event identity fields do not match the projectile fact")
}

function matchTracked(event: ProjectileEvent, tracked: TrackedProjectile): void {
  if (
    event.ownerIdentity !== tracked.ownerIdentity ||
    event.launcherIdentity !== tracked.launcherIdentity ||
    event.team !== tracked.team
  )
    throw transition("event identity fields do not match retained projectile state")
}

function startRequest(
  eventIdentity: string,
  effectIdentity: string,
  fact: ProjectileFact,
  system: string,
  attachment: ParticleAttachment | null,
  position: Vector3,
  orientation: Quaternion,
): ProjectileParticleRequest {
  return Object.freeze({
    kind: "start",
    identity: `${eventIdentity}:start:${effectIdentity}`,
    effectIdentity,
    eventIdentity,
    projectileIdentity: fact.identity,
    ownerIdentity: fact.ownerIdentity,
    launcherIdentity: fact.launcherIdentity,
    team: fact.team,
    system,
    attachment,
    controlPoints: Object.freeze([controlPoint(position, orientation, fact.ownerIdentity)]),
  })
}

function stopRequest(
  eventIdentity: string,
  effectIdentity: string,
  projectileIdentity: number,
): ProjectileParticleRequest {
  return Object.freeze({
    kind: "stop",
    identity: `${eventIdentity}:stop:${effectIdentity}`,
    effectIdentity,
    eventIdentity,
    projectileIdentity,
    immediate: true,
  })
}

function controlPoint(position: Vector3, orientation: Quaternion, ownerIdentity: number): ParticleControlPoint {
  return Object.freeze({
    index: 0,
    position: vector(position),
    orientation: quat(orientation),
    ownerIdentity,
  })
}

function trailSystem(fact: ProjectileFact): string {
  if (fact.kind === "rocket") return "rockettrail"
  return fact.team === "red" ? "stickybombtrail_red" : "stickybombtrail_blue"
}

function requireSystem(catalog: ProjectileResourceCatalog, system: string): void {
  if (!catalog.systems.has(system)) {
    throw new ProjectilePresentationError("MissingSystem", `particle system ${system} is missing`)
  }
}

function requireAttachment(
  catalog: ProjectileResourceCatalog,
  entityIdentity: number,
  name: ParticleAttachment["name"],
): ParticleAttachment {
  if (!catalog.attachments.get(entityIdentity)?.has(name)) {
    throw new ProjectilePresentationError("MissingAttachment", `attachment ${entityIdentity}:${name} is missing`)
  }
  return Object.freeze({ entityIdentity, name })
}

function requireAttachmentTransform(
  catalog: ProjectileResourceCatalog,
  attachment: ParticleAttachment,
): AttachmentTransform {
  const transform = catalog.attachmentTransforms?.get(attachment.entityIdentity)?.get(attachment.name)
  if (!transform) throw new ProjectilePresentationError("MissingAttachment", `attachment transform ${attachment.entityIdentity}:${attachment.name} is missing`)
  return transform
}

function push(
  requests: ProjectileParticleRequest[],
  limits: ProjectilePresentationLimits,
  request: ProjectileParticleRequest,
): void {
  if (requests.length >= limits.maxRequests) {
    throw new ProjectilePresentationError("BoundExceeded", "particle request count exceeds its limit")
  }
  requests.push(request)
}

function authoredRocketOrientation(orientation: Quaternion): Quaternion {
  // The stock model's authored longitudinal axis is Source local +X, matching the transported +X basis.
  return quat(orientation)
}

function settledOrientation(orientation: Quaternion, contactNormal: Vector3 | null): Quaternion {
  if (contactNormal === null) {
    throw new ProjectilePresentationError("MalformedFact", "settled sticky requires a contact normal")
  }
  const up = normalized(contactNormal)
  const transportedX = rotate(orientation, [1, 0, 0])
  let forward = subtract(transportedX, scale(up, dot(transportedX, up)))
  if (lengthSquared(forward) < 1e-8) {
    const transportedY = rotate(orientation, [0, 1, 0])
    forward = subtract(transportedY, scale(up, dot(transportedY, up)))
  }
  forward = normalized(forward)
  const right = normalized(cross(up, forward))
  return quaternionFromBasis(forward, right, up)
}

function quaternionFromBasis(x: Vector3, y: Vector3, z: Vector3): Quaternion {
  const m00 = x[0],
    m01 = y[0],
    m02 = z[0]
  const m10 = x[1],
    m11 = y[1],
    m12 = z[1]
  const m20 = x[2],
    m21 = y[2],
    m22 = z[2]
  const trace = m00 + m11 + m22
  let qx: number, qy: number, qz: number, qw: number
  if (trace > 0) {
    const s = Math.sqrt(trace + 1) * 2
    qw = 0.25 * s
    qx = (m21 - m12) / s
    qy = (m02 - m20) / s
    qz = (m10 - m01) / s
  } else if (m00 > m11 && m00 > m22) {
    const s = Math.sqrt(1 + m00 - m11 - m22) * 2
    qw = (m21 - m12) / s
    qx = 0.25 * s
    qy = (m01 + m10) / s
    qz = (m02 + m20) / s
  } else if (m11 > m22) {
    const s = Math.sqrt(1 + m11 - m00 - m22) * 2
    qw = (m02 - m20) / s
    qx = (m01 + m10) / s
    qy = 0.25 * s
    qz = (m12 + m21) / s
  } else {
    const s = Math.sqrt(1 + m22 - m00 - m11) * 2
    qw = (m10 - m01) / s
    qx = (m02 + m20) / s
    qy = (m12 + m21) / s
    qz = 0.25 * s
  }
  const length = Math.hypot(qx, qy, qz, qw)
  return Object.freeze([qx / length, qy / length, qz / length, qw / length]) as Quaternion
}

export function transformAttachment(
  matrix: Float32Array,
  position: Vector3,
  orientation: Quaternion,
): AttachmentTransform {
  if (matrix.length !== 12 || ![...matrix].every(Number.isFinite)) throw new ProjectilePresentationError("MalformedFact", "attachment matrix")
  const localPosition = vector([matrix[3]!, matrix[7]!, matrix[11]!])
  const localOrientation = quaternionFromBasis(
    vector([matrix[0]!, matrix[4]!, matrix[8]!]),
    vector([matrix[1]!, matrix[5]!, matrix[9]!]),
    vector([matrix[2]!, matrix[6]!, matrix[10]!]),
  )
  return Object.freeze({
    position: vector(add3(position, rotate(orientation, localPosition))),
    orientation: multiplyQuaternion(orientation, localOrientation),
  })
}

export function sourceViewOrientation(pitchDegrees: number, yawDegrees: number): Quaternion {
  const pitch = pitchDegrees * Math.PI / 180, yaw = yawDegrees * Math.PI / 180
  const cp = Math.cos(pitch), sp = Math.sin(pitch), cy = Math.cos(yaw), sy = Math.sin(yaw)
  return quaternionFromBasis(
    vector([cp * cy, cp * sy, -sp]),
    vector([-sy, cy, 0]),
    vector([sp * cy, sp * sy, cp]),
  )
}

function add3(left: Vector3, right: Vector3): Vector3 {
  return vector([left[0] + right[0], left[1] + right[1], left[2] + right[2]])
}

function multiplyQuaternion(left: Quaternion, right: Quaternion): Quaternion {
  return quat([
    left[3] * right[0] + left[0] * right[3] + left[1] * right[2] - left[2] * right[1],
    left[3] * right[1] - left[0] * right[2] + left[1] * right[3] + left[2] * right[0],
    left[3] * right[2] + left[0] * right[1] - left[1] * right[0] + left[2] * right[3],
    left[3] * right[3] - left[0] * right[0] - left[1] * right[1] - left[2] * right[2],
  ])
}

function rotate(quaternion: Quaternion, value: Vector3): Vector3 {
  const q = [quaternion[0], quaternion[1], quaternion[2]] as const
  const uv = cross(q, value)
  const uuv = cross(q, uv)
  return vector([
    value[0] + 2 * (uv[0] * quaternion[3] + uuv[0]),
    value[1] + 2 * (uv[1] * quaternion[3] + uuv[1]),
    value[2] + 2 * (uv[2] * quaternion[3] + uuv[2]),
  ])
}

function transition(message: string): ProjectilePresentationError {
  return new ProjectilePresentationError("IllegalTransition", message)
}

function validateLimits(limits: ProjectilePresentationLimits): void {
  if (
    !positiveInteger(limits.maxProjectiles) ||
    !positiveInteger(limits.maxEvents) ||
    !positiveInteger(limits.maxRequests)
  ) {
    throw new ProjectilePresentationError("BoundExceeded", "projectile presentation limits must be positive integers")
  }
}

function uint32(value: number): boolean {
  return Number.isInteger(value) && value > 0 && value <= UINT32_MAX
}

function positiveInteger(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0
}

function finite(value: readonly number[]): boolean {
  return value.length === 3 && value.every(Number.isFinite)
}

function quaternion(value: readonly number[]): boolean {
  return value.length === 4 && value.every(Number.isFinite) && Math.abs(lengthSquared4(value) - 1) <= NORMAL_TOLERANCE
}

function normal(value: readonly number[]): boolean {
  return finite(value) && Math.abs(lengthSquared(value as Vector3) - 1) <= NORMAL_TOLERANCE
}

function vector(value: readonly number[]): Vector3 {
  return Object.freeze([value[0]!, value[1]!, value[2]!])
}

function quat(value: readonly number[]): Quaternion {
  return Object.freeze([value[0]!, value[1]!, value[2]!, value[3]!])
}

function normalized(value: Vector3): Vector3 {
  const length = Math.sqrt(lengthSquared(value))
  return vector([value[0] / length, value[1] / length, value[2] / length])
}

function subtract(left: Vector3, right: Vector3): Vector3 {
  return vector([left[0] - right[0], left[1] - right[1], left[2] - right[2]])
}

function scale(value: Vector3, amount: number): Vector3 {
  return vector([value[0] * amount, value[1] * amount, value[2] * amount])
}

function cross(left: Vector3, right: Vector3): Vector3 {
  return vector([
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ])
}

function dot(left: Vector3, right: Vector3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

function lengthSquared(value: Vector3): number {
  return dot(value, value)
}

function lengthSquared4(value: readonly number[]): number {
  return value.reduce((sum, component) => sum + component * component, 0)
}

function trailEffect(identity: number): string {
  return `projectile:${identity}:trail`
}

function pulseEffect(identity: number): string {
  return `projectile:${identity}:pulse`
}

function oneShotEffect(eventIdentity: string): string {
  return `event:${eventIdentity}`
}
