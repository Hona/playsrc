import type { ParticleRenderItem } from "@playsrc/particle"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import type { Camera, Effect } from "@playsrc/rendering"
import type { ModelItem } from "@playsrc/rendering"
import type { PresentationArtifacts } from "./artifacts"
import { tf2ClassPresentation, type Tf2ClassPresentation } from "./class"
import type { Snapshot } from "./codec"

const UINT32_MAX = 0xffff_ffff
const NORMAL_TOLERANCE = 1e-4
const TF2_DEFAULT_HORIZONTAL_FOV_4_BY_3 = 75
const SOURCE_WORLD_NEAR = 7
const SOURCE_MAP_EXTENT = 16_384
const SOURCE_MAP_EXTENT_DIAGONAL = Math.fround(1.73205080757)
const UTF8_ENCODER = new TextEncoder()

export type PresentationDiagnostic = Readonly<{
  code: "MissingProjectileModel" | "MissingParticleContext" | "MissingAudioContext"
  identity: string
}>

export type Tf2Hud = Readonly<{
  health: number
  maxHealth: number
  className: Tf2ClassPresentation["displayName"]
  weaponName: "Rocket Launcher" | "Original" | "Stickybomb Launcher" | "Scattergun" | "Pistol" | "Bat" | "Shotgun" | "Shovel" | null
  speed: number
  projectileCount: number
}>

export type Tf2AudioRequest = Readonly<{
  voiceIdentity: number
  definition: "Weapon_RPG.Single" | "Weapon_QuakeRPG.Single" | "Weapon_StickyBombLauncher.Single" | "BaseExplosionEffect.Sound" | "Weapon_QuakeRPG.Explode" | "Weapon_Grenade_Pipebomb.Explode" | "Weapon_Scatter_Gun.Single" | "Weapon_Pistol.Single" | "Weapon_Bat.Miss" | "Weapon_Bat.HitFlesh" | "Weapon_Bat.HitWorld" | "Weapon_Scatter_Gun.WorldReload" | "Weapon_Pistol.WorldReload" | "Weapon_Shotgun.Single" | "Weapon_Shotgun.WorldReload" | "Weapon_Shovel.Miss" | "Weapon_Shovel.HitFlesh" | "Weapon_Shovel.HitWorld"
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
    "Weapon_Scatter_Gun.Single",
    "Weapon_Pistol.Single",
    "Weapon_Bat.Miss",
    "Weapon_Bat.HitFlesh",
    "Weapon_Bat.HitWorld",
    "Weapon_Scatter_Gun.WorldReload",
    "Weapon_Pistol.WorldReload",
    "Weapon_Shotgun.Single",
    "Weapon_Shotgun.WorldReload",
    "Weapon_Shovel.Miss",
    "Weapon_Shovel.HitFlesh",
    "Weapon_Shovel.HitWorld",
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
    ticks: Object.freeze(snapshot.projectileTimeline.map((entry) => Object.freeze({
      tick: entry.tick,
      projectiles: Object.freeze(entry.projectiles.map((projectile) => Object.freeze({
        identity: projectile.identity,
        kind: projectile.kind === 1 ? "rocket" : "sticky",
        team: projectile.team === 2 ? "red" : "blue",
        ownerIdentity: projectile.ownerIdentity,
        launcherIdentity: projectile.launcherIdentity,
        state: projectile.state === 1 ? "flying" : projectile.state === 2 ? "stuck-unarmed" : "stuck-armed",
        position: vector(projectile.position),
        velocity: vector(projectile.velocity),
        orientation: quat(projectile.orientation),
        angularVelocity: vector(projectile.angularVelocity),
        contactNormal: projectile.contactNormal === null ? null : vector(projectile.contactNormal),
        ageSeconds: projectile.ageSeconds,
      }))),
      events: projectileTimelineEvents(entry.events),
    }))),
  })
}

function projectileTimelineEvents(
  events: Snapshot["projectileEvents"],
): readonly ProjectileEvent[] {
  const output: ProjectileEvent[] = []
  const nextKind = new Array<Snapshot["projectileEvents"][number]["type"] | undefined>(events.length)
  const upcoming = new Map<number, Snapshot["projectileEvents"][number]["type"]>()
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]!
    nextKind[index] = upcoming.get(event.projectile)
    upcoming.set(event.projectile, event.type)
  }
  const append = (
    event: Snapshot["projectileEvents"][number],
    kind: ProjectileEventKind,
    sourceEventOrdinal: number,
  ) => {
    output.push(Object.freeze({
      kind,
      projectileKind: event.kind === 1 ? "rocket" : "sticky",
      projectileIdentity: event.projectile,
      ownerIdentity: event.ownerIdentity,
      launcherIdentity: event.launcherIdentity,
      team: event.team === 2 ? "red" : "blue",
      tick: event.tick,
      sourceOrdinal: output.length,
      sourceEventOrdinal,
      position: vector(event.position),
      orientation: quat(event.orientation),
      contactNormal: event.contactNormal === null ? null : vector(event.contactNormal),
    }))
  }
  for (let sourceEventOrdinal = 0; sourceEventOrdinal < events.length; sourceEventOrdinal += 1) {
    const event = events[sourceEventOrdinal]!
    append(event, event.type, sourceEventOrdinal)
    if (event.type === "impact") {
      const outcome = nextKind[sourceEventOrdinal]
      if (outcome !== "stick" && outcome !== "explode") {
        append(event, "bounce", sourceEventOrdinal)
      }
    } else if (event.type === "fizzle" || event.type === "explode") {
      append(event, "destroy", sourceEventOrdinal)
    }
  }
  return Object.freeze(output)
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
      if (snapshot.weapon === null || (snapshot.class !== 1 && snapshot.class !== 3 && snapshot.class !== 4)) {
        throw new ProjectilePresentationError("MalformedFact", "class has no implemented viewmodel weapon")
      }
      const identity = tf2ClassPresentation(snapshot.class).hands
      const itemIdentity = snapshot.weapon === 4
        ? "models/weapons/c_models/c_scattergun.mdl"
        : snapshot.weapon === 5
          ? "models/weapons/c_models/c_pistol/c_pistol.mdl"
          : snapshot.weapon === 6
            ? "models/weapons/c_models/c_bat.mdl"
            : snapshot.weapon === 7
              ? "models/weapons/c_models/c_shotgun/c_shotgun.mdl"
              : snapshot.weapon === 8
                ? "models/weapons/c_models/c_shovel/c_shovel.mdl"
                : snapshot.class === 3
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
      const role = snapshot.weapon === 6 || snapshot.weapon === 8 ? "MELEE" : snapshot.weapon === 5 || snapshot.weapon === 7 || snapshot.class === 4 ? "SECONDARY" : "PRIMARY"
      const mapped = exact === undefined ? undefined : [
        "",
        `ACT_${role}_VM_DRAW`,
        role === "MELEE" ? "ACT_MELEE_VM_HITCENTER" : `ACT_${role}_VM_PRIMARYATTACK`,
        snapshot.weapon === 5 ? "ACT_SECONDARY_VM_RELOAD" : `ACT_${role}_RELOAD_START`,
        `ACT_${role}_VM_RELOAD`,
        `ACT_${role}_RELOAD_FINISH`,
        `ACT_${role}_VM_IDLE`,
      ][exact.activity]
      let nextActivity = mapped ?? (selectionChanged ? `ACT_${role}_VM_DRAW` : activity)
      let selected = artifact.sequences.find((value) => value.activity === nextActivity)
      if (!selected) throw new ProjectilePresentationError("MissingModel", `${identity}:${nextActivity}`)
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
      const phase = exact ? exact.activity - 1 : nextActivity.endsWith("_VM_DRAW") ? 0
        : nextActivity.endsWith("_VM_PRIMARYATTACK") ? 1
          : nextActivity.endsWith("_RELOAD_START") ? 2
            : nextActivity.endsWith("_VM_RELOAD") ? 3
              : nextActivity.endsWith("_RELOAD_FINISH") ? 4 : 5
      if (phase < 0 || phase > 5) throw new ProjectilePresentationError("MalformedFact", "viewmodel phase")
      return Object.freeze({
        item: Object.freeze({
          identity: 0x7fff_ff00 + snapshot.class * 4,
          model: identity,
          position:Object.freeze([0,0,0]) as Vector3,angles:Object.freeze([0,0,0]) as Vector3,
          scale: 1,
          skin: snapshot.team === 2 ? 0 : 1,
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
          phase: phase as 0 | 1 | 2 | 3 | 4 | 5,
          reflectedViewmodel: false,
          ownerAlive: snapshot.lifecycle === 1,
          skin: snapshot.team === 2 ? 0 : 1,
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
  for (const byte of UTF8_ENCODER.encode(value)) {
    hash ^= byte
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0 || 1
}
function stable64(value: string) {
  let hash = 0xcbf29ce484222325n
  for (const byte of UTF8_ENCODER.encode(value)) {
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
  sampleTick?: bigint
  attachmentsOnly?: boolean
  fireView?: Readonly<{ eyePosition: Vector3; viewOrientation: Quaternion }>
  previousElapsedSeconds: number
  elapsedSeconds: number
  currentTimeSeconds:number;frameTimeSeconds:number;planarSpeed:number;screenAspectRatio:number;worldFarPlane:number
  phase?: 0 | 1 | 2 | 3 | 4 | 5
  reflectedViewmodel?: boolean
  ownerAlive?: boolean
  packedBody?: number
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
  sampleTick: bigint
  attachmentsOnly: boolean
  attachmentsWorld: boolean
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
  viewmodel:null|Readonly<{transform:Readonly<{origin:Vector3;angles:Vector3}>;projection:Readonly<{unscaledHorizontalFov4By3:number;horizontalFov:number;aspectRatio:number;near:number;far:number}>;depthRange:readonly[number,number];restoredDepthRange:readonly[number,number];passRestored:boolean;depthRestored:boolean;itemTranslucent:boolean;phase:"draw"|"primary-fire"|"reload-start"|"reload-insert-or-loop"|"reload-finish"|"idle";drawDisposition:"draw"|"suppressed-success"|"suppressed";suppression:number|null;reflected:boolean;frontFace:"clockwise"|"counter-clockwise";cullFace:"back";restoredCullMode:"counter-clockwise"|"clockwise";handBodygroups:readonly number[];itemBodygroups:readonly number[];itemBodygroupMutations:readonly Readonly<{event:number;bodygroup:number;value:number;name:string}>[]}>
}>

export function encodeModelPoseBatch(requests: readonly ModelPoseRequest[]): Uint8Array {
  if (requests.length > 128) throw new ProjectilePresentationError("BoundExceeded", "model pose request count")
  let length = 12
  const encodedRequests = requests.map((request) => {
    const model = UTF8_ENCODER.encode(request.model)
    const item = UTF8_ENCODER.encode(request.itemModel ?? "")
    const activity = UTF8_ENCODER.encode(request.activity)
    length += 108 + model.length + item.length + activity.length +
      (request.bodygroups.length + (request.itemBodygroups?.length ?? 0)) * 4
    return { request, model, item, activity }
  })
  if (length > 1024 * 1024) throw new ProjectilePresentationError("BoundExceeded", "model pose request bytes")
  const bytes = new Uint8Array(length), view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x4d, 0x52, 0x51])
  view.setUint32(4, 6, true)
  view.setUint32(8, requests.length, true)
  let at = 12
  const text = (encoded: Uint8Array) => {
    view.setUint32(at, encoded.length, true); at += 4; bytes.set(encoded, at); at += encoded.length
  }
  for (const { request, model, item, activity } of encodedRequests) {
    if (!Number.isSafeInteger(request.identity) || request.identity < 1 || !request.model || !request.activity ||
      ![request.previousElapsedSeconds, request.elapsedSeconds].every(Number.isFinite) || request.previousElapsedSeconds < 0 ||
      request.elapsedSeconds < request.previousElapsedSeconds || ![request.skin, request.lod, ...request.bodygroups, ...(request.itemBodygroups ?? [])].every((value) => Number.isSafeInteger(value) && value >= 0) ||
      ((request.itemModel === undefined) !== (request.itemBodygroups === undefined)) || request.itemModel === "") {
      throw new ProjectilePresentationError("MalformedFact", "model pose request")
    }
    view.setUint32(at, request.identity, true); at += 4
    const sampleTick = request.sampleTick ?? 0n
    if (typeof sampleTick !== "bigint" || sampleTick < 0n || sampleTick > 0xffff_ffff_ffff_ffffn ||
      typeof (request.attachmentsOnly ?? false) !== "boolean" ||
      (request.attachmentsOnly && request.itemModel === undefined) ||
      (request.attachmentsOnly && request.fireView === undefined) ||
      (request.fireView !== undefined && (request.itemModel === undefined ||
        !finite(request.fireView.eyePosition) || !quaternion(request.fireView.viewOrientation)))) {
      throw new ProjectilePresentationError("MalformedFact", "model pose sample")
    }
    view.setBigUint64(at, sampleTick, true); at += 8
    bytes[at] = request.itemModel === undefined ? 0 : 1
    bytes[at + 1] = Number(request.attachmentsOnly ?? false)
    bytes[at + 2] = Number(request.fireView !== undefined)
    at += 4
    for (const value of [...(request.fireView?.eyePosition ?? [0, 0, 0]),
      ...(request.fireView?.viewOrientation ?? [0, 0, 0, 0])]) {
      view.setFloat32(at, value, true); at += 4
    }
    text(model); text(item); text(activity)
    view.setFloat32(at, request.previousElapsedSeconds, true); at += 4
    view.setFloat32(at, request.elapsedSeconds, true); at += 4
    for(const value of [request.currentTimeSeconds,request.frameTimeSeconds,request.planarSpeed,request.screenAspectRatio,request.worldFarPlane]){view.setFloat32(at,value,true);at+=4}
    view.setUint32(at, request.skin, true); at += 4
    view.setUint32(at, request.lod, true); at += 4
    bytes[at] = request.itemModel === undefined ? 0xff : (request.phase ?? 0xff)
    bytes[at + 1] = Number(request.reflectedViewmodel ?? false)
    bytes[at + 2] = Number(request.ownerAlive ?? true)
    bytes[at + 3] = 0
    if ((request.itemModel !== undefined && (request.phase === undefined || request.phase < 0 || request.phase > 5)) ||
      (request.itemModel === undefined && request.phase !== undefined) || typeof (request.reflectedViewmodel ?? false) !== "boolean" ||
      typeof (request.ownerAlive ?? true) !== "boolean") throw new ProjectilePresentationError("MalformedFact", "viewmodel frame request")
    at += 4
    view.setInt32(at, request.packedBody ?? -0x8000_0000, true);at+=4
    if(request.packedBody!==undefined&&(!Number.isSafeInteger(request.packedBody)||request.packedBody<0||request.itemModel!==undefined))throw new ProjectilePresentationError("MalformedFact","packed model body")
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
  if (decoder.decode(bytes.subarray(0, 4)) !== "PMPO" || view.getUint32(4, true) !== 5) throw new ProjectilePresentationError("MalformedFact", "model pose output identity")
  let at = 12
  const ensure = (length: number) => { if (at + length > bytes.length) throw new ProjectilePresentationError("MalformedFact", "model pose output truncation") }
  const u8 = () => { ensure(1); return bytes[at++]! }, u32 = () => { ensure(4); const value = view.getUint32(at, true); at += 4; return value },
    i32 = () => { ensure(4); const value = view.getInt32(at, true); at += 4; return value },
    f32 = () => { ensure(4); const value = view.getFloat32(at, true); at += 4; if (!Number.isFinite(value)) throw new ProjectilePresentationError("MalformedFact", "model pose scalar"); return value },
    text = () => { const length = u32(); ensure(length); const value = decoder.decode(bytes.subarray(at, at + length)); at += length; return value }
  const output: PosedModel[] = []
  for (let count = view.getUint32(8, true); count > 0; count--) {
    const identity = u32()
    ensure(8)
    const sampleTick = view.getBigUint64(at, true); at += 8
    const roleCode = u8(), attachmentMode = u8(), attachmentsWorld = u8()
    if (roleCode > 2 || attachmentMode > 1 || attachmentsWorld > 1 ||
      (attachmentMode === 1 && (roleCode !== 2 || attachmentsWorld !== 1)) || u8()) {
      throw new ProjectilePresentationError("MalformedFact", "model pose role")
    }
    const model = text(), activity = text(), sequence = u32(), framesPerSecond = f32(), weightedFrameCount = f32(),
      cyclesPerSecond = f32(), durationSeconds = f32(), looping = u8()
    if (looping > 1 || u8() || u8() || u8()) throw new ProjectilePresentationError("MalformedFact", "model pose timing")
    const previousCycle = f32(), cycle = f32()
    const present=u8();if(present>1||u8()||u8()||u8())throw new ProjectilePresentationError("MalformedFact","viewmodel state");const values=Array.from({length:15},f32),passRestored=u8(),depthRestored=u8(),itemTranslucent=u8();if(u8())throw new ProjectilePresentationError("MalformedFact","viewmodel flags")
    const phase=u8(),drawDisposition=u8(),suppression=u8(),reflected=u8(),frontFace=u8(),cullFace=u8(),restoredCull=u8(),reserved=u8()
    if(phase>5||drawDisposition>2||suppression>12||reflected>1||frontFace>1||cullFace!==0||restoredCull>1||reserved||
      (drawDisposition===0)!==(suppression===0))throw new ProjectilePresentationError("MalformedFact","viewmodel frame state")
    const valuesArray=(limit:number)=>{const count=u32();if(count>limit)throw new ProjectilePresentationError("BoundExceeded","viewmodel frame values");return Object.freeze(Array.from({length:count},u32))}
    const handBodygroups=valuesArray(64),itemBodygroups=valuesArray(64),mutationCount=u32();if(mutationCount>64)throw new ProjectilePresentationError("BoundExceeded","viewmodel bodygroup mutations")
    const itemBodygroupMutations=Object.freeze(Array.from({length:mutationCount},()=>Object.freeze({event:u32(),bodygroup:u32(),value:i32(),name:text()})))
    if(present===0&&(phase||drawDisposition||suppression||reflected||frontFace||cullFace||restoredCull||handBodygroups.length||itemBodygroups.length||itemBodygroupMutations.length))throw new ProjectilePresentationError("MalformedFact","absent viewmodel frame state")
    const viewmodel=present===0?null:Object.freeze({transform:Object.freeze({origin:vector(values.slice(0,3)),angles:vector(values.slice(3,6))}),projection:Object.freeze({unscaledHorizontalFov4By3:values[6]!,horizontalFov:values[7]!,aspectRatio:values[8]!,near:values[9]!,far:values[10]!}),depthRange:Object.freeze(values.slice(11,13)) as readonly[number,number],restoredDepthRange:Object.freeze(values.slice(13,15)) as readonly[number,number],passRestored:passRestored===1,depthRestored:depthRestored===1,itemTranslucent:itemTranslucent===1,phase:(["draw","primary-fire","reload-start","reload-insert-or-loop","reload-finish","idle"] as const)[phase]!,drawDisposition:(["draw","suppressed-success","suppressed"] as const)[drawDisposition]!,suppression:suppression===0?null:suppression,reflected:reflected===1,frontFace:frontFace===0?"clockwise" as const:"counter-clockwise" as const,cullFace:"back" as const,restoredCullMode:restoredCull===0?"counter-clockwise" as const:"clockwise" as const,handBodygroups,itemBodygroups,itemBodygroupMutations})
    const events = Object.freeze(Array.from({ length: u32() }, () => {
      const index = u32(), eventCycle = f32(), event = i32(), eventType = i32(); ensure(64)
      const options = bytes.slice(at, at + 64); at += 64
      return Object.freeze({ index, cycle: eventCycle, event, eventType, options, name: text() })
    }))
    const primitives = Object.freeze(Array.from({ length: u32() }, () => {
      const primitive = u32(), material = u32(), vertices = u32(), translucent = u8()
      if (translucent > 1 || u8() || u8() || u8()) {
        throw new ProjectilePresentationError("MalformedFact", "primitive opacity")
      }
      ensure(vertices * 40)
      const positions = new Float32Array(vertices * 3)
      const normals = new Float32Array(vertices * 3)
      const tangents = new Float32Array(vertices * 4)
      for (let vertex = 0; vertex < vertices; vertex += 1) {
        const position = vertex * 3
        const tangent = vertex * 4
        const x = view.getFloat32(at, true)
        const y = view.getFloat32(at + 4, true)
        const z = view.getFloat32(at + 8, true)
        const nx = view.getFloat32(at + 12, true)
        const ny = view.getFloat32(at + 16, true)
        const nz = view.getFloat32(at + 20, true)
        const tx = view.getFloat32(at + 24, true)
        const ty = view.getFloat32(at + 28, true)
        const tz = view.getFloat32(at + 32, true)
        const tw = view.getFloat32(at + 36, true)
        if (
          !Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z) ||
          !Number.isFinite(nx) || !Number.isFinite(ny) || !Number.isFinite(nz) ||
          !Number.isFinite(tx) || !Number.isFinite(ty) || !Number.isFinite(tz) || !Number.isFinite(tw)
        ) throw new ProjectilePresentationError("MalformedFact", "model pose scalar")
        positions[position] = x
        positions[position + 1] = y
        positions[position + 2] = z
        normals[position] = nx
        normals[position + 1] = ny
        normals[position + 2] = nz
        tangents[tangent] = tx
        tangents[tangent + 1] = ty
        tangents[tangent + 2] = tz
        tangents[tangent + 3] = tw
        at += 40
      }
      return Object.freeze({ primitive, material, positions, normals, tangents, translucent: translucent === 1 })
    }))
    const attachments = Object.freeze(Array.from({ length: u32() }, () => {
      const name = text(), worldAligned = u8(); if (worldAligned > 1 || u8() || u8() || u8()) throw new ProjectilePresentationError("MalformedFact", "model attachment")
      const matrix = new Float32Array(12); for (let index = 0; index < 12; index++) matrix[index] = f32()
      return Object.freeze({ name, worldAligned: worldAligned === 1, matrix })
    }))
    if (attachmentMode === 1 && primitives.length !== 0) throw new ProjectilePresentationError("MalformedFact", "attachment-only pose contains geometry")
    output.push(Object.freeze({ identity, sampleTick, attachmentsOnly: attachmentMode === 1, attachmentsWorld: attachmentsWorld === 1, role: (["single", "hand", "item"] as const)[roleCode]!, model, activity, sequence, framesPerSecond, weightedFrameCount, cyclesPerSecond, durationSeconds, looping: looping === 1, previousCycle, cycle, events, primitives, attachments,viewmodel }))
  }
  if (at !== bytes.length) throw new ProjectilePresentationError("MalformedFact", "model pose output trailing bytes")
  return Object.freeze(output)
}
export function createParticleBatchEncoder() {
  let previousTick = 0n
  let previousTime = 0
  return Object.freeze({
    encode(tick: bigint, camera: Vector3, requests: readonly ProjectileParticleRequest[]) {
      if (
        typeof tick !== "bigint"
        || tick < previousTick
        || tick > BigInt(Number.MAX_SAFE_INTEGER)
        || !Array.isArray(requests)
        || requests.length > 4_096
        || !finite(camera)
        || camera.some(value => !Number.isFinite(Math.fround(value)))
      ) {
        throw new ProjectilePresentationError("TimeReversed", "particle transaction range or input is invalid")
      }
      const to = Math.fround(Number(tick) * 0.015)
      if (!Number.isFinite(to) || to < previousTime) {
        throw new ProjectilePresentationError("TimeReversed", "particle transaction range or input is invalid")
      }

      let length = 32
      let previousRequestTick = previousTick
      const systems = new Map<string, Uint8Array>()
      const identities = new Set<bigint>()
      const requestIdentities: bigint[] = []
      for (const request of requests) {
        if (
          typeof request.tick !== "bigint"
          || request.tick < previousRequestTick
          || request.tick > tick
        ) {
          throw new ProjectilePresentationError("TimeReversed", "particle request time is outside source order")
        }
        if (
          !particleIdentity(request.identity)
          || !particleIdentity(request.effectIdentity)
          || !particleIdentity(request.eventIdentity)
          || !uint32(request.projectileIdentity)
        ) {
          throw new ProjectilePresentationError("MalformedFact", "particle request identity is invalid")
        }
        const identity = stable64(request.identity)
        if (identities.has(identity)) {
          throw new ProjectilePresentationError("MalformedFact", "particle request identity is duplicated")
        }
        identities.add(identity)
        requestIdentities.push(identity)
        previousRequestTick = request.tick
        length += 20

        if (request.kind === "start") {
          if (
            !uint32(request.ownerIdentity)
            || !uint32(request.launcherIdentity)
            || (request.team !== "red" && request.team !== "blue")
            || typeof request.system !== "string"
            || request.system.length === 0
            || !Array.isArray(request.controlPoints)
            || request.controlPoints.length !== 1
            || !particleControlPoint(request.controlPoints[0])
            || request.controlPoints[0].ownerIdentity !== request.ownerIdentity
            || (request.attachment !== null && (
              !uint32(request.attachment.entityIdentity)
              || !["backblast", "muzzle", "trail"].includes(request.attachment.name)
            ))
          ) {
            throw new ProjectilePresentationError("MalformedFact", "particle start request is invalid")
          }
          if (request.system.length > 1_024) {
            throw new ProjectilePresentationError("BoundExceeded", "particle definition identity exceeds its limit")
          }
          let encoded = systems.get(request.system)
          if (!encoded) {
            encoded = UTF8_ENCODER.encode(request.system)
            if (encoded.byteLength > 1_024) {
              throw new ProjectilePresentationError("BoundExceeded", "particle definition identity exceeds its limit")
            }
            systems.set(request.system, encoded)
          }
          length += 16 + encoded.byteLength + 32
        } else if (request.kind === "set-control-point") {
          if (!particleControlPoint(request.controlPoint)) {
            throw new ProjectilePresentationError("MalformedFact", "particle control-point request is invalid")
          }
          length += 32
        } else if (request.kind === "stop") {
          if (typeof request.immediate !== "boolean") {
            throw new ProjectilePresentationError("MalformedFact", "particle stop mode is invalid")
          }
        } else {
          throw new ProjectilePresentationError("MalformedFact", "particle request command is invalid")
        }
        if (length > 4 * 1024 * 1024) {
          throw new ProjectilePresentationError("BoundExceeded", "particle transaction bytes")
        }
      }

      const bytes = new Uint8Array(length)
      const view = new DataView(bytes.buffer)
      bytes.set([0x50, 0x50, 0x54, 0x58])
      view.setUint32(4, 2, true)
      view.setFloat32(8, previousTime, true)
      view.setFloat32(12, to, true)
      camera.forEach((value, index) => view.setFloat32(16 + index * 4, value, true))
      view.setUint32(28, requests.length, true)
      let at = 32
      for (let index = 0; index < requests.length; index += 1) {
        const request = requests[index]!
        bytes[at] = request.kind === "start" ? 1 : request.kind === "set-control-point" ? 2 : 3
        bytes[at + 1] = request.kind === "stop" && request.immediate ? 1 : 0
        view.setBigUint64(at + 4, requestIdentities[index]!, true)
        view.setFloat32(at + 12, Math.fround(Number(request.tick) * 0.015), true)
        view.setUint32(at + 16, stable32(request.effectIdentity), true)
        at += 20
        if (request.kind === "start") {
          view.setBigUint64(at, stable64(request.eventIdentity), true)
          view.setUint32(at + 8, request.ownerIdentity, true)
          const system = systems.get(request.system)!
          view.setUint32(at + 12, system.byteLength, true)
          bytes.set(system, at + 16)
          at += 16 + system.byteLength
          const control = request.controlPoints[0]!
          control.position.forEach((value, index) => view.setFloat32(at + index * 4, value, true))
          control.orientation.forEach((value, index) => view.setFloat32(at + 12 + index * 4, value, true))
          view.setUint32(at + 28, control.ownerIdentity, true)
          at += 32
        } else if (request.kind === "set-control-point") {
          const control = request.controlPoint
          control.position.forEach((value, index) => view.setFloat32(at + index * 4, value, true))
          control.orientation.forEach((value, index) => view.setFloat32(at + 12 + index * 4, value, true))
          view.setUint32(at + 28, control.ownerIdentity, true)
          at += 32
        }
      }
      previousTick = tick
      previousTime = to
      return bytes
    },
  })
}

function particleIdentity(value: string): boolean {
  return typeof value === "string" && value.length > 0
}

function particleControlPoint(value: ParticleControlPoint | undefined): value is ParticleControlPoint {
  return value !== undefined
    && value.index === 0
    && finite(value.position)
    && value.position.every(component => Number.isFinite(Math.fround(component)))
    && quaternion(value.orientation)
    && value.orientation.every(component => Number.isFinite(Math.fround(component)))
    && uint32(value.ownerIdentity)
}

export function tf2Hud(snapshot: Snapshot): Tf2Hud {
  return Object.freeze({
    health: snapshot.health,
    maxHealth: snapshot.maximumHealth,
    className: tf2ClassPresentation(snapshot.class).displayName,
    weaponName: snapshot.weapon === null ? null
      : (["", "Rocket Launcher", "Original", "Stickybomb Launcher", "Scattergun", "Pistol", "Bat", "Shotgun", "Shovel"] as const)[snapshot.weapon],
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
export type ProjectileEventKind =
  | "fire"
  | "impact"
  | "bounce"
  | "stick"
  | "arm"
  | "fizzle"
  | "explode"
  | "destroy"
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
  projectileKind: ProjectileKind
  projectileIdentity: number
  ownerIdentity: number
  launcherIdentity: number
  team: ProjectileTeam
  tick: bigint
  sourceOrdinal: number
  sourceEventOrdinal: number
  position: Vector3
  orientation: Quaternion
  contactNormal: Vector3 | null
}>

export type ProjectileTick = Readonly<{
  tick: bigint
  projectiles: readonly ProjectileFact[]
  events: readonly ProjectileEvent[]
}>

export type ProjectileFrame = Readonly<{
  ticks: readonly ProjectileTick[]
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
      tick: bigint
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
      tick: bigint
      projectileIdentity: number
      controlPoint: ParticleControlPoint
    }>
  | Readonly<{
      kind: "stop"
      identity: string
      effectIdentity: string
      eventIdentity: string
      tick: bigint
      projectileIdentity: number
      immediate: boolean
    }>

export function hitscanMuzzleParticles(
  snapshot: Snapshot,
  catalog: Pick<ProjectileResourceCatalog, "systems" | "attachmentTransforms">,
): readonly ProjectileParticleRequest[] {
  const requests: ProjectileParticleRequest[] = []
  for (const event of snapshot.events) {
    if (event.kind !== 12 || (event.detail !== 4 && event.detail !== 5 && event.detail !== 7)) continue
    const system = event.detail === 4 ? "muzzle_scattergun" : event.detail === 5 ? "muzzle_pistol" : "muzzle_shotgun"
    if (!catalog.systems.has(system)) throw new ProjectilePresentationError("MissingSystem", system)
    const transform = catalog.attachmentTransforms?.get(event.detail)?.get("muzzle")
    if (!transform) throw new ProjectilePresentationError("MissingAttachment", `${event.detail}:muzzle`)
    const eventIdentity = `hitscan:${snapshot.tick}:${event.detail}:${requests.length}`
    requests.push(Object.freeze({
      kind: "start",
      identity: eventIdentity,
      effectIdentity: eventIdentity,
      eventIdentity,
      tick: snapshot.tick,
      projectileIdentity: 0x7fff_0000 + event.detail,
      ownerIdentity: 1,
      launcherIdentity: event.detail,
      team: snapshot.team === 2 ? "red" : "blue",
      system,
      attachment: Object.freeze({ entityIdentity: event.detail, name: "muzzle" }),
      controlPoints: Object.freeze([Object.freeze({
        index: 0,
        position: transform.position,
        orientation: transform.orientation,
        ownerIdentity: 1,
      })]),
    }))
  }
  return Object.freeze(requests)
}

export type ProjectileResourceCatalog = Readonly<{
  models: ReadonlySet<string>
  systems: ReadonlySet<string>
  attachments: ReadonlyMap<number, ReadonlySet<string>>
  attachmentTransforms: ReadonlyMap<number, ReadonlyMap<string, AttachmentTransform>>
  fireAttachmentTransforms: ReadonlyMap<number, ReadonlyMap<string, AttachmentTransform>>
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
  armed: boolean
  trailActive: boolean
  trailLocalTransform: AttachmentTransform | null
}>

type ProjectileSource = Readonly<{
  identity: number
  kind: ProjectileKind
  team: ProjectileTeam
  ownerIdentity: number
  launcherIdentity: number
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
  let hasMapped = false
  let tracked = new Map<number, TrackedProjectile>()
  let disposed = false

  return Object.freeze({
    map(frame: ProjectileFrame) {
      if (disposed) throw new ProjectilePresentationError("IllegalTransition", "mapper is disposed")
      const next = new Map(tracked)
      if (frame.ticks.length === 0) {
        throw new ProjectilePresentationError("MalformedFact", "projectile timeline is empty")
      }
      const particles: ProjectileParticleRequest[] = []
      const mappedEvents: ProjectileEvent[] = []
      let finalFacts = new Map<number, ProjectileFact>()
      let mappedTick = tick
      let totalEvents = 0
      for (let timelineIndex = 0; timelineIndex < frame.ticks.length; timelineIndex += 1) {
        const timeline = frame.ticks[timelineIndex]!
        if (
          typeof timeline.tick !== "bigint"
          || timeline.tick < mappedTick
          || ((hasMapped || timelineIndex > 0) && timeline.tick === mappedTick)
        ) {
          throw new ProjectilePresentationError("TimeReversed", "projectile timeline tick moved backward or repeated")
        }
        totalEvents += timeline.events.length
        if (timeline.projectiles.length > limits.maxProjectiles || totalEvents > limits.maxEvents) {
          throw new ProjectilePresentationError("BoundExceeded", "projectile timeline exceeds its count limit")
        }
        const facts = new Map<number, ProjectileFact>()
        for (const fact of timeline.projectiles) {
          validateFact(fact)
          if (facts.has(fact.identity)) {
            throw new ProjectilePresentationError("MalformedFact", "projectile identity is duplicated")
          }
          facts.set(fact.identity, fact)
        }
        const startedControls = new Map<number, ParticleControlPoint>()
        for (let eventIndex = 0; eventIndex < timeline.events.length; eventIndex += 1) {
          const event = timeline.events[eventIndex]!
          const previousEvent = timeline.events[eventIndex - 1]
          validateEvent(event, timeline.tick, mappedTick, eventIndex)
          const fact = facts.get(event.projectileIdentity)
          const prior = next.get(event.projectileIdentity)
          const eventIdentity = `${event.tick}:${event.sourceEventOrdinal}:${event.kind}:${event.projectileIdentity}`
          if (event.kind === "fire") {
            if (prior) throw transition("fire targets an existing projectile")
            if (fact && event.tick === timeline.tick) matchEvent(event, fact)
            else if (fact && (event.projectileKind !== fact.kind || event.team !== fact.team || event.ownerIdentity !== fact.ownerIdentity || event.launcherIdentity !== fact.launcherIdentity)) throw transition("delayed fire immutable identity fields changed")
            const source = eventFact(event)
            const trail = trailSystem(source)
            requireSystem(catalog, trail)
            const trailAttachment = source.kind === "rocket"
              ? requireAttachment(catalog, source.identity, "trail")
              : null
            const trailWorldTransform = trailAttachment ? requireAttachmentTransform(catalog, trailAttachment) : null
            const retainedReference = frame.ticks.at(-1)?.projectiles.find((candidate) => candidate.identity === source.identity)
            const trailLocalTransform = trailWorldTransform
              ? localAttachmentTransform(
                  trailWorldTransform,
                  retainedReference?.position ?? event.position,
                  retainedReference?.orientation ?? event.orientation,
                )
              : null
            const initialTransform = trailLocalTransform
              ? applyAttachmentTransform(trailLocalTransform, event.position, event.orientation)
              : Object.freeze({ position: event.position, orientation: event.orientation })
            const trailStart = startRequest(
              eventIdentity,
              trailEffect(source.identity),
              source,
              trail,
              trailAttachment,
              initialTransform.position,
              initialTransform.orientation,
              event.tick,
            )
            push(particles, limits, trailStart)
            startedControls.set(source.identity, trailStart.controlPoints[0]!)
            const muzzle = source.kind === "rocket" ? "rocketbackblast" : "muzzle_pipelauncher"
            if (!(source.kind === "rocket" && source.ownerIdentity === catalog.localOwnerIdentity)) {
              const attachment = requireAttachment(
                catalog,
                source.launcherIdentity,
                source.kind === "rocket" ? "backblast" : "muzzle",
              )
              const muzzleTransform = requireFireAttachmentTransform(catalog, event, attachment)
              requireSystem(catalog, muzzle)
              push(
                particles,
                limits,
                startRequest(
                  eventIdentity,
                  oneShotEffect(eventIdentity),
                  source,
                  muzzle,
                  attachment,
                  muzzleTransform.position,
                  muzzleTransform.orientation,
                  event.tick,
                ),
              )
            }
            next.set(
              source.identity,
              Object.freeze({
                kind: source.kind,
                team: source.team,
                ownerIdentity: source.ownerIdentity,
                launcherIdentity: source.launcherIdentity,
                state: "flying",
                armed: false,
                trailActive: true,
                trailLocalTransform,
              }),
            )
          } else {
            if (!prior) throw transition(`${event.kind} targets an unknown projectile`)
            if (fact) matchEvent(event, fact)
            matchTracked(event, prior)
            if (event.kind === "impact") {
              if (prior.state !== "flying") throw transition("impact requires a flying projectile")
            } else if (event.kind === "bounce") {
              if (
                prior.state !== "flying"
                || previousEvent?.kind !== "impact"
                || previousEvent.projectileIdentity !== event.projectileIdentity
              ) {
                throw transition("bounce requires its immediately preceding flying impact")
              }
            } else if (event.kind === "stick") {
              if (
                prior.kind !== "sticky"
                || prior.state !== "flying"
                || previousEvent?.kind !== "impact"
                || previousEvent.projectileIdentity !== event.projectileIdentity
                || !fact
                || fact.state === "flying"
              ) {
                throw transition("stick requires a flying sticky and one settled fact")
              }
              next.set(event.projectileIdentity, Object.freeze({ ...prior, state: fact.state }))
            } else if (event.kind === "arm") {
              if (prior.kind !== "sticky" || prior.armed) throw transition("arm requires one unarmed sticky projectile")
              const system = prior.team === "red" ? "stickybomb_pulse_red" : "stickybomb_pulse_blue"
              requireSystem(catalog, system)
              push(
                particles,
                limits,
                startRequest(
                  eventIdentity,
                  pulseEffect(event.projectileIdentity),
                  eventFact(event),
                  system,
                  null,
                  event.position,
                  event.orientation,
                  event.tick,
                ),
              )
              next.set(event.projectileIdentity, Object.freeze({
                ...prior,
                state: fact?.state ?? prior.state,
                armed: true,
              }))
            } else if (event.kind === "fizzle" || event.kind === "explode") {
              if (prior.trailActive) {
                push(
                  particles,
                  limits,
                  stopRequest(
                    eventIdentity,
                    trailEffect(event.projectileIdentity),
                    event.projectileIdentity,
                    event.tick,
                    false,
                  ),
                )
              }
              if (event.kind === "explode") {
                const system = event.contactNormal === null ? "ExplosionCore_MidAir" : "ExplosionCore_Wall"
                requireSystem(catalog, system)
                push(
                  particles,
                  limits,
                  startRequest(
                    eventIdentity,
                    oneShotEffect(eventIdentity),
                    eventFact(event),
                    system,
                    null,
                    event.position,
                    explosionOrientation(event.contactNormal),
                    event.tick,
                  ),
                )
              }
            } else if (event.kind === "destroy") {
              if (
                (previousEvent?.kind !== "fizzle" && previousEvent?.kind !== "explode")
                || previousEvent.projectileIdentity !== event.projectileIdentity
              ) {
                throw transition("destroy requires its immediately preceding terminal event")
              }
              next.delete(event.projectileIdentity)
            }
          }
          mappedEvents.push(Object.freeze({
            ...event,
            position: vector(event.position),
            orientation: quat(event.orientation),
            contactNormal: event.contactNormal === null ? null : vector(event.contactNormal),
          }))
        }

        for (const [identity, state] of next) {
          const fact = facts.get(identity)
          if (!fact || fact.state !== state.state) {
            throw transition("projectile disappearance or state change has no ordered event")
          }
          if (
            fact.kind !== state.kind
            || fact.team !== state.team
            || fact.ownerIdentity !== state.ownerIdentity
            || fact.launcherIdentity !== state.launcherIdentity
          ) {
            throw transition("projectile immutable identity fields changed")
          }
          if (state.trailActive) {
            const transform = state.trailLocalTransform
              ? applyAttachmentTransform(state.trailLocalTransform, fact.position, fact.orientation)
              : Object.freeze({ position: fact.position, orientation: fact.orientation })
            const control = controlPoint(transform.position, transform.orientation, fact.ownerIdentity)
            const started = startedControls.get(identity)
            if (!started || !sameControlPoint(started, control)) {
              push(
                particles,
                limits,
                Object.freeze({
                  kind: "set-control-point",
                  identity: `${timeline.tick}:follow:${identity}`,
                  effectIdentity: trailEffect(identity),
                  eventIdentity: `${timeline.tick}:follow`,
                  tick: timeline.tick,
                  projectileIdentity: identity,
                  controlPoint: control,
                }),
              )
            }
          }
        }
        for (const identity of facts.keys()) {
          if (!next.has(identity)) throw transition(`projectile fact ${identity} has no fire event after tick ${mappedTick}; tracked ${[...next.keys()].join(",")}`)
        }
        finalFacts = facts
        mappedTick = timeline.tick
      }

      const models = [...finalFacts.values()].filter((fact) => fact.kind !== "sticky" || fact.ageSeconds >= 0.1).map((fact) => {
        const model = fact.kind === "rocket"
          ? ("models/weapons/w_models/w_rocket.mdl" as const)
          : ("models/weapons/w_models/w_stickybomb.mdl" as const)
        if (!catalog.models.has(model)) {
          throw new ProjectilePresentationError("MissingModel", `projectile model ${model} is missing`)
        }
        return Object.freeze({
          identity: fact.identity,
          projectileIdentity: fact.identity,
          model,
          skin: fact.team === "red" ? (0 as const) : (1 as const),
          materialVariant: fact.team,
          position: vector(fact.position),
          orientation: authoredProjectileOrientation(fact.orientation),
          angularVelocity: vector(fact.angularVelocity),
          state: fact.state,
        })
      })
      tracked = next
      tick = mappedTick
      hasMapped = true
      return Object.freeze({
        models: Object.freeze(models),
        particles: Object.freeze(particles),
        events: Object.freeze(mappedEvents),
      })
    },
    reset(nextTick: bigint): void {
      if (disposed || typeof nextTick !== "bigint" || nextTick < 0n) {
        throw new ProjectilePresentationError("TimeReversed", "projectile mapper reset tick is invalid")
      }
      tick = nextTick
      hasMapped = false
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

function validateEvent(event: ProjectileEvent, frameTick: bigint, earliestTick: bigint, sourceOrdinal: number): void {
  if (
    !["fire", "impact", "bounce", "stick", "arm", "fizzle", "explode", "destroy"].includes(event.kind) ||
    (event.projectileKind !== "rocket" && event.projectileKind !== "sticky") ||
    !uint32(event.projectileIdentity) ||
    !uint32(event.ownerIdentity) ||
    !uint32(event.launcherIdentity) ||
    (event.team !== "red" && event.team !== "blue") ||
    typeof event.tick !== "bigint" ||
    event.tick < earliestTick || event.tick > frameTick ||
    !Number.isSafeInteger(event.sourceOrdinal) ||
    event.sourceOrdinal !== sourceOrdinal ||
    !Number.isSafeInteger(event.sourceEventOrdinal) ||
    event.sourceEventOrdinal < 0 ||
    event.sourceEventOrdinal > event.sourceOrdinal ||
    !finite(event.position) ||
    !quaternion(event.orientation) ||
    (event.contactNormal !== null && !normal(event.contactNormal)) ||
    (event.kind === "fire" && event.contactNormal !== null) ||
    ((event.kind === "impact" || event.kind === "bounce" || event.kind === "stick") && event.contactNormal === null)
  ) {
    throw new ProjectilePresentationError("MalformedEvent", "projectile event violates the frozen contract")
  }
}

function matchEvent(event: ProjectileEvent, fact: ProjectileFact): void {
  if (
    event.ownerIdentity !== fact.ownerIdentity ||
    event.launcherIdentity !== fact.launcherIdentity ||
    event.team !== fact.team ||
    event.projectileKind !== fact.kind
  )
    throw transition("event identity fields do not match the projectile fact")
}

function matchTracked(event: ProjectileEvent, tracked: TrackedProjectile): void {
  if (
    event.ownerIdentity !== tracked.ownerIdentity ||
    event.launcherIdentity !== tracked.launcherIdentity ||
    event.team !== tracked.team ||
    event.projectileKind !== tracked.kind
  )
    throw transition("event identity fields do not match retained projectile state")
}

function startRequest(
  eventIdentity: string,
  effectIdentity: string,
  fact: ProjectileSource,
  system: string,
  attachment: ParticleAttachment | null,
  position: Vector3,
  orientation: Quaternion,
  tick: bigint,
): Extract<ProjectileParticleRequest, { kind: "start" }> {
  return Object.freeze({
    kind: "start",
    identity: `${eventIdentity}:start:${effectIdentity}`,
    effectIdentity,
    eventIdentity,
    tick,
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
  tick: bigint,
  immediate: boolean,
): Extract<ProjectileParticleRequest, { kind: "stop" }> {
  return Object.freeze({
    kind: "stop",
    identity: `${eventIdentity}:stop:${effectIdentity}`,
    effectIdentity,
    eventIdentity,
    tick,
    projectileIdentity,
    immediate,
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

function trailSystem(fact: ProjectileSource): string {
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
  const transform = catalog.attachmentTransforms.get(attachment.entityIdentity)?.get(attachment.name)
  return validateAttachmentTransform(transform, `${attachment.entityIdentity}:${attachment.name}`)
}

function requireFireAttachmentTransform(
  catalog: ProjectileResourceCatalog,
  event: ProjectileEvent,
  attachment: ParticleAttachment,
): AttachmentTransform {
  const transform = catalog.fireAttachmentTransforms.get(event.projectileIdentity)?.get(attachment.name)
  return validateAttachmentTransform(transform, `${event.tick}:${attachment.entityIdentity}:${attachment.name}`)
}

function validateAttachmentTransform(
  transform: AttachmentTransform | undefined,
  identity: string,
): AttachmentTransform {
  if (!transform) throw new ProjectilePresentationError("MissingAttachment", `attachment transform ${identity} is missing`)
  if (
    !finite(transform.position)
    || transform.position.some(value => !Number.isFinite(Math.fround(value)))
    || !quaternion(transform.orientation)
    || transform.orientation.some(value => !Number.isFinite(Math.fround(value)))
  ) {
    throw new ProjectilePresentationError("MalformedFact", `attachment transform ${identity} is invalid`)
  }
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

function authoredProjectileOrientation(orientation: Quaternion): Quaternion {
  // Source renders both projectile models at their transported entity quaternion.
  return quat(orientation)
}

function eventFact(event: ProjectileEvent): ProjectileSource {
  return Object.freeze({
    identity: event.projectileIdentity,
    kind: event.projectileKind,
    team: event.team,
    ownerIdentity: event.ownerIdentity,
    launcherIdentity: event.launcherIdentity,
  })
}

function explosionOrientation(contactNormal: Vector3 | null): Quaternion {
  if (contactNormal === null) return Object.freeze([0, 0, 0, 1])
  const [x, y, z] = contactNormal
  let pitch: number
  let yaw: number
  if (x === 0 && y === 0) {
    yaw = 0
    pitch = z > 0 ? 270 : 90
  } else {
    yaw = Math.atan2(y, x) * 180 / Math.PI
    if (yaw < 0) yaw += 360
    pitch = Math.atan2(-z, Math.sqrt(x * x + y * y)) * 180 / Math.PI
    if (pitch < 0) pitch += 360
  }
  return sourceViewOrientation(pitch, yaw)
}

function localAttachmentTransform(
  world: AttachmentTransform,
  position: Vector3,
  orientation: Quaternion,
): AttachmentTransform {
  const inverse = inverseQuaternion(orientation)
  return Object.freeze({
    position: rotate(inverse, subtract(world.position, position)),
    orientation: multiplyQuaternion(inverse, world.orientation),
  })
}

function applyAttachmentTransform(
  local: AttachmentTransform,
  position: Vector3,
  orientation: Quaternion,
): AttachmentTransform {
  return Object.freeze({
    position: add3(position, rotate(orientation, local.position)),
    orientation: multiplyQuaternion(orientation, local.orientation),
  })
}

function inverseQuaternion(value: Quaternion): Quaternion {
  return Object.freeze([-value[0], -value[1], -value[2], value[3]])
}

function sameControlPoint(left: ParticleControlPoint, right: ParticleControlPoint): boolean {
  return left.ownerIdentity === right.ownerIdentity
    && left.position.every((value, index) => value === right.position[index])
    && left.orientation.every((value, index) => value === right.orientation[index])
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

function subtract(left: Vector3, right: Vector3): Vector3 {
  return vector([left[0] - right[0], left[1] - right[1], left[2] - right[2]])
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
