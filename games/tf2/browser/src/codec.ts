const HASH = /^[0-9a-f]{64}$/
const MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024
const MAX_RECORDS = 65_536
const MOVEMENT_BYTES = 96

export type Tf2Class = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
export type Tf2Team = 2 | 3
export type Tf2Weapon = 1 | 2 | 3
export type MovementMode = 0 | 1
export type ProjectileKind = 1 | 2
export type ProjectileState = 1 | 2 | 3
export type ContactKind = 1 | 2 | 3

export type ProjectilePhysicsResult = Readonly<{
  projectile: number
  tick: bigint
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  motionEnabled: boolean
  contact: null | Readonly<{ kind: ContactKind; normal: readonly [number, number, number] }>
}>
export type RandomStreamState = Readonly<{
  current: number
  shuffled: number
  table: readonly number[]
}>
export type Tf2RandomState = Readonly<{
  authority: RandomStreamState
  predictedPresentation: RandomStreamState
  rocketExplosionAvailable: number
  stickyExplosionAvailable: number
}>
export type RandomDraw = Readonly<{
  context: 1 | 2
  decision: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  definition: 0 | 1 | 2 | 3 | 4 | 5 | 6
  phase: 0 | 1 | 2
  raw: number
  result: Readonly<{ kind: "float-bits"; bits: number } | { kind: "integer"; value: number } | { kind: "rejected-integer" }>
}>
export type AudioEvent = Readonly<{
  tick: bigint
  ordinal: number
  identity: 1 | 2
  definition: 1 | 2 | 3 | 4 | 5 | 6
  sourceKind: 1 | 2
  sourceIdentity: number
  ownerIdentity: number | null
  position: readonly [number, number, number]
  samples: Readonly<{ volume: number; pitch: number; wave: number; soundLevel: number }>
}>
export type RocketTraceResult = Readonly<{
  projectile: number
  tick: bigint
  end: readonly [number, number, number]
  solid: boolean
  sky: boolean
  normal: readonly [number, number, number] | null
  directTarget: number | null
}>
export type MoverResult = Readonly<{
  requestId: bigint
  entity: number
  kind: 1 | 2 | 3 | 4 | 5
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
  carry: readonly [number, number, number]
}>
export type CollisionSnapshot = Readonly<{
  worldIdentity: string
  identity: bigint
  objects: number
  bytes: Uint8Array
}>
export type Command = Readonly<{
  forward: number
  side: number
  up?: number
  yawDegrees: number
  pitchDegrees: number
  jump: boolean
  crouch: boolean
  speedButton?: boolean
  fire: boolean
  detonate: boolean
  reload?: boolean
  reset?: boolean
  respawn?: boolean
  selectClass?: Tf2Class | 12
  selectTeam?: Tf2Team
  selectWeapon?: Tf2Weapon
  modeRequest?: MovementMode
  activateEntity?: number
  physicsResults?: readonly ProjectilePhysicsResult[]
}>

export type MovementSnapshot = Readonly<{
  mode: MovementMode
  crouchPhase: 0 | 1 | 2 | 3 | 4
  waterLevel: number
  waterType: number
  jumpLatched: boolean
  previousJump: boolean
  previousCrouch: boolean
  stuckOffset: number
  grounded: boolean
  supportIdentity: bigint | null
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  viewOffset: readonly [number, number, number]
  crouchFraction: number
  crouchStartFraction: number
  crouchElapsed: number
  crouchDuration: number
  fallSpeed: number
  groundNormal: readonly [number, number, number]
  surfaceFriction: number
}>
export type MovementTick = Readonly<{
  mode: number
  crouchPhase: number
  grounded: boolean
  wishDirection: readonly [number, number, number]
  wishSpeed: number
  uncappedWishSpeed: number
  wishVelocity: readonly [number, number, number]
  jumpVelocity: readonly [number, number, number]
  climbedStep: number
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
  sweepQueries: number
  pointQueries: number
  contacts: number
  events: number
  mover: null | Readonly<{
    identity: bigint
    status: 0 | 1 | 2
    displacement: readonly [number, number, number]
    supportVelocity: readonly [number, number, number]
    blocker: bigint | null
  }>
}>

export type WeaponState = Readonly<{
  weapon: Tf2Weapon
  reload: 0 | 1 | 2 | 3
  clip: number
  reserve: number
  maximumClip: number
  maximumReserve: number
  nextPrimaryTick: bigint
  reloadDueTick: bigint | null
  chargeBeginTick: bigint | null
  firstPrimaryTick: bigint
}>

export type WeaponActivity = 1 | 2 | 3 | 4 | 5 | 6
export type ActivityEvent = Readonly<{ tick: bigint; weapon: Tf2Weapon; activity: WeaponActivity }>
export type LifecycleEvent = Readonly<{ tick: bigint; kind: 1 | 2 | 3 | 4; class: Tf2Class; team: Tf2Team }>
export type ProjectilePhysicsRequest = Readonly<{
  operation: 1 | 2 | 3 | 4
  projectile: number
  tick: bigint
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
  gravityScale: number
  friction: number
  elasticity: number
}>
export type RocketTraceRequest = Readonly<{
  projectile: number
  tick: bigint
  start: readonly [number, number, number]
  end: readonly [number, number, number]
  mask: number
}>
export type RadiusDamageRequest = Readonly<{
  projectile: number
  kind: ProjectileKind
  source: readonly [number, number, number]
  baseDamage: number
  radius: number
  selfRadius: number
  directTarget: number | null
}>
export type MoverRequest = Readonly<{
  requestId: bigint
  entity: number
  model: number | null
  start: readonly [number, number, number]
  destination: readonly [number, number, number]
  speed: number
  opening: boolean
}>
export type ContactReconcileRequest = Readonly<{
  tick: bigint
  position: readonly [number, number, number]
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
}>
export type MapEffect = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5
  detail: number
  team: number | null
  contact: ContactKind | null
  subject: number
  auxiliary: number | null
  values: readonly [number, number, number, number, number, number]
}>
export type RegenerateAnimationEvent = Readonly<{
  zone: number
  associatedModel: number
  openTick: bigint
  closeTick: bigint
  body: number
  openAnimation: "open" | "close"
  closeAnimation: "open" | "close"
}>
export type AuthorityBlocker = Readonly<{
  code: 1 | 2
  classification: "Missing"
  detail: string
}>

export type Projectile = Readonly<{
  identity: number
  kind: ProjectileKind
  team: Tf2Team
  ownerIdentity: number
  launcherIdentity: number
  state: ProjectileState
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  contactNormal: readonly [number, number, number] | null
  ageSeconds: number
}>

export type ProjectileEventType = "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode"
export type ProjectileEvent = Readonly<{
  type: ProjectileEventType
  projectile: number
  kind: ProjectileKind
  ownerIdentity: number
  launcherIdentity: number
  team: Tf2Team
  tick: bigint
  position: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  contactNormal: readonly [number, number, number] | null
  launcherPose: Readonly<{
    eyePosition: readonly [number, number, number]
    viewOrientation: readonly [number, number, number, number]
  }> | null
}>

export type ProjectileTimelineTick = Readonly<{
  tick: bigint
  projectiles: readonly Projectile[]
  events: readonly ProjectileEvent[]
}>

export type EntityTransform = Readonly<{
  identity: number
  model: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>
export type BrushModelDrawState = Readonly<{ sourceIndex:number; model:number; worldPosition:readonly[number,number,number]; worldAngles:readonly[number,number,number]; renderMode:number;color:readonly[number,number,number,number];renderFx:number;effects:number;draw:boolean;mover:null|Readonly<{kind:1|2|3;position:1|2|3|4|5;progress:number;requestId:bigint|null;opening:boolean|null}> }>
export type EntityPresentation=Readonly<{sourceIdentity:bigint;registryIdentity:bigint;tick:bigint;entityRevision:bigint;collisionRevision:bigint;models:readonly BrushModelDrawState[]}>

export type EntityEvent = Readonly<{
  sequence: bigint
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  accepted: boolean
  contact: 1 | 2 | 3 | null
  entity: number
  subject: number | null
  name: string
}>

export type GameplayEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11
  detail: number
  subject: number
  auxiliary: number
  values: readonly [number, number, number, number]
}>

export type JumpCheckpoint = Readonly<{ zoneIdentity: number; index: number; tick: bigint }>
export type JumpRun = Readonly<{
  instance: number
  disposition: 0 | 1 | 2 | 3
  playerIdentity: number
  class: 3 | 4
  team: 2 | 3
  invalidation: 1 | 2 | 3 | 4 | null
  startTick: bigint
  endTick: bigint | null
  checkpoints: readonly JumpCheckpoint[]
}>
export type JumpEvent = Readonly<{
  sequence: bigint
  tick: bigint
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  detail: number
  runInstance: number
  zoneIdentity: number | null
  zoneIndex: number | null
}>
export type JumpResult = Readonly<{
  courseIdentity: bigint
  mapIdentity: string
  runInstance: number
  playerIdentity: number
  class: 3 | 4
  team: 2 | 3
  runKind: "map"
  zoneIndex: number
  disposition: "completed"
  startTick: bigint
  endTick: bigint
  elapsedTicks: bigint
  tickInterval: number
  checkpoints: readonly JumpCheckpoint[]
}>
export type JumpSnapshot = Readonly<{
  run: JumpRun | null
  events: readonly JumpEvent[]
  result: JumpResult | null
}>

export type Snapshot = Readonly<{
  tick: bigint
  class: Tf2Class
  team: Tf2Team
  weapon: Tf2Weapon | null
  playerFlags: number
  inWater: boolean
  health: number
  maximumHealth: number
  lifecycle: 1 | 2
  conditions: readonly [number, number, number, number, number]
  respawnTouchCount: number
  movement: MovementSnapshot
  movementTick: MovementTick | null
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  grounded: boolean
  crouched: boolean
  loadout: readonly WeaponState[]
  projectiles: readonly Projectile[]
  projectileEvents: readonly ProjectileEvent[]
  projectileTimeline: readonly ProjectileTimelineTick[]
  entityTransforms: readonly EntityTransform[]
  entityEvents: readonly EntityEvent[]
  jump: JumpSnapshot | null
  events: readonly GameplayEvent[]
  activities: readonly ActivityEvent[]
  lifecycleEvents: readonly LifecycleEvent[]
  physicsRequests: readonly ProjectilePhysicsRequest[]
  rocketTraceRequests: readonly RocketTraceRequest[]
  radiusDamageRequests: readonly RadiusDamageRequest[]
  moverRequests: readonly MoverRequest[]
  contactReconcileRequests: readonly ContactReconcileRequest[]
  mapEffects: readonly MapEffect[]
  regenerateAnimationEvents: readonly RegenerateAnimationEvent[]
  randomState: Tf2RandomState
  randomDraws: readonly RandomDraw[]
  audioEvents: readonly AudioEvent[]
  rocketTraceResults: readonly RocketTraceResult[]
  moverResults: readonly MoverResult[]
  collisionSnapshot: CollisionSnapshot
  entityPresentation: EntityPresentation
  authorityBlockers: readonly AuthorityBlocker[]
}>

export class Tf2CodecError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2CodecError"
  }
}

export type JumpCourseZone = Readonly<{
  identity: number
  triggerEntity: number
  kind: "start" | "checkpoint" | "end"
  index: number
}>

export function encodeJumpCourse(identity: bigint, mapSha256: string, zones: readonly JumpCourseZone[]): Uint8Array {
  if (identity <= 0n || identity > 0xffff_ffff_ffff_ffffn || !HASH.test(mapSha256) || zones.length > 4_096) {
    throw new Tf2CodecError("Jump course identity or bound is invalid")
  }
  const identities = new Set<number>()
  const triggers = new Set<number>()
  const checkpoints: number[] = []
  let starts = 0
  let ends = 0
  for (const zone of zones) {
    if (
      !canonicalIdentity(zone.identity) ||
      zone.identity === 0 ||
      identities.has(zone.identity) ||
      !canonicalIdentity(zone.triggerEntity) ||
      triggers.has(zone.triggerEntity) ||
      !Number.isSafeInteger(zone.index) ||
      zone.index < 1 ||
      zone.index > 0xffff_ffff
    )
      throw new Tf2CodecError("Jump zone identity is invalid")
    identities.add(zone.identity)
    triggers.add(zone.triggerEntity)
    if (zone.kind === "start" && zone.index === 1) starts += 1
    else if (zone.kind === "end" && zone.index === 1) ends += 1
    else if (zone.kind === "checkpoint") checkpoints.push(zone.index)
    else throw new Tf2CodecError("Jump zone kind or index is invalid")
  }
  checkpoints.sort((left, right) => left - right)
  if (starts !== 1 || ends !== 1 || checkpoints.some((value, index) => value !== index + 1)) {
    throw new Tf2CodecError("Jump linear topology is invalid")
  }
  const bytes = new Uint8Array(52 + zones.length * 16)
  const view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x4a, 0x4d, 0x50])
  view.setUint32(4, 1, true)
  view.setBigUint64(8, identity, true)
  for (let index = 0; index < 32; index += 1) {
    bytes[16 + index] = Number.parseInt(mapSha256.slice(index * 2, index * 2 + 2), 16)
  }
  view.setUint32(48, zones.length, true)
  zones.forEach((zone, index) => {
    const at = 52 + index * 16
    view.setUint32(at, zone.identity, true)
    view.setUint32(at + 4, zone.triggerEntity, true)
    bytes[at + 8] = zone.kind === "start" ? 1 : zone.kind === "checkpoint" ? 2 : 3
    view.setUint32(at + 12, zone.index, true)
  })
  return bytes
}

function canonicalIdentity(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0 && value <= 0xffff_ffff
}

export function encodeCommand(command: Command): ArrayBuffer {
  const scalars = [command.forward, command.side, command.up ?? 0, command.yawDegrees, command.pitchDegrees]
  if (!scalars.every(Number.isFinite)) throw new Tf2CodecError("command contains a non-finite scalar")
  if (command.selectClass !== undefined && (!Number.isInteger(command.selectClass) || command.selectClass < 1 || (command.selectClass > 9 && command.selectClass !== 12))) {
    throw new Tf2CodecError("command class selector is invalid")
  }
  if (command.selectTeam !== undefined && command.selectTeam !== 2 && command.selectTeam !== 3) {
    throw new Tf2CodecError("command team selector is invalid")
  }
  if (
    command.selectWeapon !== undefined &&
    command.selectWeapon !== 1 &&
    command.selectWeapon !== 2 &&
    command.selectWeapon !== 3
  )
    throw new Tf2CodecError("command weapon selector is invalid")
  if (command.modeRequest !== undefined && command.modeRequest !== 0 && command.modeRequest !== 1) {
    throw new Tf2CodecError("command mode request is invalid")
  }
  if (command.activateEntity !== undefined && !canonicalIdentity(command.activateEntity)) {
    throw new Tf2CodecError("command entity identity is invalid")
  }
  const physics = command.physicsResults ?? []
  if (physics.length > 64) {
    throw new Tf2CodecError("command external-result count exceeds its bound")
  }
  const uint64 = (value: bigint): boolean => value >= 0n && value <= 0xffff_ffff_ffff_ffffn
  for (const result of physics) {
    if (!canonicalIdentity(result.projectile) || !uint64(result.tick) ||
      !finite([...result.position, ...result.velocity, ...result.angularVelocity]) || !normalized(result.orientation) ||
      (result.contact !== null && (!([1, 2, 3] as number[]).includes(result.contact.kind) || !normalized(result.contact.normal)))) {
      throw new Tf2CodecError("command projectile Physics result is invalid")
    }
  }
  const length = 48 + physics.length * 80
  const bytes = new ArrayBuffer(length)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x43, 0x4d, 0x44])
  view.setUint32(4, 5, true)
  scalars.forEach((value, index) => view.setFloat32(8 + index * 4, value, true))
  const flags =
    Number(command.jump) |
    (Number(command.crouch) << 1) |
    (Number(command.speedButton ?? false) << 2) |
    (Number(command.fire) << 3) |
    (Number(command.detonate) << 4) |
    (Number(command.reload ?? false) << 5) |
    (Number(command.reset ?? false) << 6) |
    (Number(command.respawn ?? false) << 7)
  view.setUint32(28, flags, true)
  const mode = command.modeRequest === undefined ? 0 : command.modeRequest + 1
  view.setUint32(
    32,
    (command.selectClass ?? 0) | ((command.selectWeapon ?? 0) << 8) | ((command.selectTeam ?? 0) << 16) | (mode << 24),
    true,
  )
  view.setUint32(36, command.activateEntity ?? 0xffff_ffff, true)
  view.setUint16(40, physics.length, true)
  view.setUint16(42, 0, true)
  view.setUint32(44, length, true)
  let at = 48
  const writeVector = (value: readonly number[]): void => {
    value.forEach((scalar, index) => view.setFloat32(at + index * 4, scalar, true))
    at += value.length * 4
  }
  for (const result of physics) {
    view.setUint32(at, result.projectile, true)
    view.setBigUint64(at + 4, result.tick, true)
    data[at + 12] = Number(result.motionEnabled)
    data[at + 13] = result.contact?.kind ?? 0
    at += 16
    writeVector(result.position)
    writeVector(result.velocity)
    writeVector(result.orientation)
    writeVector(result.angularVelocity)
    writeVector(result.contact?.normal ?? [0, 0, 0])
  }
  if (at !== length) throw new Tf2CodecError("command encoding length differs")
  return bytes
}

function vector(view: DataView, offset: number): readonly [number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
  ]) as readonly [number, number, number]
}

function quaternion(view: DataView, offset: number): readonly [number, number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
    view.getFloat32(offset + 12, true),
  ]) as readonly [number, number, number, number]
}

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

function normalized(values: readonly number[]): boolean {
  if (!finite(values)) return false
  const length = Math.hypot(...values)
  return Math.abs(length - 1) <= 0.001
}

function count(value: number, label: string): number {
  if (value > MAX_RECORDS) throw new Tf2CodecError(`${label} count exceeds its bound`)
  return value
}

function movementSnapshot(bytes: ArrayBuffer, offset: number, length: number, waterType: number): MovementSnapshot {
  if (length !== MOVEMENT_BYTES) throw new Tf2CodecError("Movement snapshot length is invalid")
  const data = new Uint8Array(bytes, offset, length)
  const view = new DataView(bytes, offset, length)
  if (
    data[0] !== 0x50 ||
    data[1] !== 0x4d ||
    data[2] !== 0x4f ||
    data[3] !== 0x56 ||
    view.getUint32(4, true) !== 1 ||
    data[8]! > 1 ||
    data[9]! > 4 ||
    data[11]! > 1 ||
    data[12]! > 1 ||
    data[13]! > 1 ||
    data[15]! > 1
  )
    throw new Tf2CodecError("Movement snapshot identity or flags are invalid")
  const position = vector(view, 24)
  const velocity = vector(view, 36)
  const viewOffset = vector(view, 48)
  const groundNormal = vector(view, 80)
  const scalars = [
    ...position,
    ...velocity,
    ...viewOffset,
    view.getFloat32(60, true),
    view.getFloat32(64, true),
    view.getFloat32(68, true),
    view.getFloat32(72, true),
    view.getFloat32(76, true),
    ...groundNormal,
    view.getFloat32(92, true),
  ]
  if (!finite(scalars)) throw new Tf2CodecError("Movement snapshot contains a non-finite scalar")
  const support = view.getBigUint64(16, true)
  return Object.freeze({
    mode: data[8] as MovementMode,
    crouchPhase: data[9] as MovementSnapshot["crouchPhase"],
    waterLevel: data[10]!,
    waterType,
    jumpLatched: data[11] === 1,
    previousJump: data[12] === 1,
    previousCrouch: data[13] === 1,
    stuckOffset: data[14]!,
    grounded: data[15] === 1,
    supportIdentity: support === 0xffff_ffff_ffff_ffffn ? null : support,
    position,
    velocity,
    viewOffset,
    crouchFraction: scalars[9]!,
    crouchStartFraction: scalars[10]!,
    crouchElapsed: scalars[11]!,
    crouchDuration: scalars[12]!,
    fallSpeed: scalars[13]!,
    groundNormal,
    surfaceFriction: scalars[17]!,
  })
}

function decodeJump(bytes: ArrayBuffer, offset: number, length: number): JumpSnapshot | null {
  if (length === 0) return null
  if (length < 16 || offset + length > bytes.byteLength) throw new Tf2CodecError("Jump section is truncated")
  const data = new Uint8Array(bytes, offset, length)
  const view = new DataView(bytes, offset, length)
  if (data[0] !== 0x50 || data[1] !== 0x4a || data[2] !== 0x4f || data[3] !== 0x46 || view.getUint32(4, true) !== 1)
    throw new Tf2CodecError("Jump section identity is invalid")
  let at = 8
  const ensure = (size: number): void => {
    if (at + size > length) throw new Tf2CodecError("Jump section record exceeds its bytes")
  }
  ensure(4)
  const hasRun = data[at]
  if (hasRun === undefined || hasRun > 1) throw new Tf2CodecError("Jump run flag is invalid")
  let run: JumpRun | null = null
  if (hasRun === 0) at += 4
  else {
    ensure(36)
    const disposition = data[at + 1]
    const tf2Class = data[at + 2]
    const team = data[at + 3]
    const invalidation = data[at + 12]
    if (
      disposition === undefined ||
      disposition > 3 ||
      (tf2Class !== 3 && tf2Class !== 4) ||
      (team !== 2 && team !== 3) ||
      invalidation === undefined ||
      invalidation > 4 ||
      data[at + 13] !== 0 ||
      data[at + 14] !== 0 ||
      data[at + 15] !== 0
    )
      throw new Tf2CodecError("Jump run record is invalid")
    const checkpointCount = count(view.getUint32(at + 32, true), "Jump checkpoint")
    ensure(36 + checkpointCount * 16)
    const checkpoints: JumpCheckpoint[] = []
    for (let index = 0; index < checkpointCount; index += 1) {
      const item = at + 36 + index * 16
      checkpoints.push(
        Object.freeze({
          zoneIdentity: view.getUint32(item, true),
          index: view.getUint32(item + 4, true),
          tick: view.getBigUint64(item + 8, true),
        }),
      )
    }
    const end = view.getBigUint64(at + 24, true)
    run = Object.freeze({
      instance: view.getUint32(at + 4, true),
      disposition: disposition as JumpRun["disposition"],
      playerIdentity: view.getUint32(at + 8, true),
      class: tf2Class,
      team,
      invalidation: invalidation === 0 ? null : (invalidation as 1 | 2 | 3 | 4),
      startTick: view.getBigUint64(at + 16, true),
      endTick: end === 0xffff_ffff_ffff_ffffn ? null : end,
      checkpoints: Object.freeze(checkpoints),
    })
    at += 36 + checkpointCount * 16
  }
  ensure(4)
  const eventCount = count(view.getUint32(at, true), "Jump event")
  at += 4
  ensure(eventCount * 32)
  const events: JumpEvent[] = []
  for (let index = 0; index < eventCount; index += 1) {
    const item = at + index * 32
    const kind = data[item + 16]
    const detail = data[item + 17]
    if (kind === undefined || kind < 1 || kind > 8 || data[item + 18] !== 0 || data[item + 19] !== 0) {
      throw new Tf2CodecError("Jump event record is invalid")
    }
    const zoneIdentity = view.getUint32(item + 24, true)
    const zoneIndex = view.getUint32(item + 28, true)
    events.push(
      Object.freeze({
        sequence: view.getBigUint64(item, true),
        tick: view.getBigUint64(item + 8, true),
        kind: kind as JumpEvent["kind"],
        detail: detail!,
        runInstance: view.getUint32(item + 20, true),
        zoneIdentity: zoneIdentity === 0xffff_ffff ? null : zoneIdentity,
        zoneIndex: zoneIndex === 0xffff_ffff ? null : zoneIndex,
      }),
    )
  }
  at += eventCount * 32
  ensure(4)
  const hasResult = data[at]
  if (hasResult === undefined || hasResult > 1) throw new Tf2CodecError("Jump result flag is invalid")
  let result: JumpResult | null = null
  if (hasResult === 0) at += 4
  else {
    ensure(92)
    const tf2Class = data[at + 1]
    const team = data[at + 2]
    if (
      (tf2Class !== 3 && tf2Class !== 4) ||
      (team !== 2 && team !== 3) ||
      data[at + 3] !== 1 ||
      data[at + 56] !== 2 ||
      data[at + 57] !== 0 ||
      data[at + 58] !== 0 ||
      data[at + 59] !== 0
    ) {
      throw new Tf2CodecError("Jump result selection is invalid")
    }
    const checkpointCount = count(view.getUint32(at + 88, true), "Jump result checkpoint")
    ensure(92 + checkpointCount * 16)
    const checkpoints: JumpCheckpoint[] = []
    for (let index = 0; index < checkpointCount; index += 1) {
      const item = at + 92 + index * 16
      checkpoints.push(
        Object.freeze({
          zoneIdentity: view.getUint32(item, true),
          index: view.getUint32(item + 4, true),
          tick: view.getBigUint64(item + 8, true),
        }),
      )
    }
    const map = data.slice(at + 12, at + 44)
    result = Object.freeze({
      courseIdentity: view.getBigUint64(at + 4, true),
      mapIdentity: Array.from(map, (value) => value.toString(16).padStart(2, "0")).join(""),
      runInstance: view.getUint32(at + 44, true),
      playerIdentity: view.getUint32(at + 48, true),
      runKind: "map",
      zoneIndex: view.getUint32(at + 52, true),
      disposition: "completed",
      class: tf2Class,
      team,
      startTick: view.getBigUint64(at + 60, true),
      endTick: view.getBigUint64(at + 68, true),
      elapsedTicks: view.getBigUint64(at + 76, true),
      tickInterval: view.getFloat32(at + 84, true),
      checkpoints: Object.freeze(checkpoints),
    })
    if (!Number.isFinite(result.tickInterval) || result.tickInterval <= 0) {
      throw new Tf2CodecError("Jump result tick interval is invalid")
    }
    at += 92 + checkpointCount * 16
  }
  if (at !== length) throw new Tf2CodecError("Jump section has trailing bytes")
  return Object.freeze({ run, events: Object.freeze(events), result })
}

function decodeMovementTick(bytes: ArrayBuffer, offset: number, length: number): MovementTick | null {
  if (length < 12 || length > 144) throw new Tf2CodecError("Movement tick section length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (data[0] !== 0x50 || data[1] !== 0x4d || data[2] !== 0x54 || data[3] !== 0x4b || view.getUint32(4, true) !== 1)
    throw new Tf2CodecError("Movement tick identity is invalid")
  const present = data[8]
  if (present === 0) {
    if (length !== 12 || data[9] !== 0 || data[10] !== 0 || data[11] !== 0) throw new Tf2CodecError("Movement tick absent record is invalid")
    return null
  }
  if (present !== 1 || data[9]! > 7 || data[10]! > 4 || data[11]! > 1 || length < 104) throw new Tf2CodecError("Movement tick header is invalid")
  let at = 12
  const vec = () => { const result = vector(view, at); at += 12; return result }
  const scalar = () => { const value = view.getFloat32(at, true); at += 4; if (!Number.isFinite(value)) throw new Tf2CodecError("Movement tick scalar is invalid"); return value }
  const wishDirection = vec(), wishSpeed = scalar(), uncappedWishSpeed = scalar(), wishVelocity = vec(), jumpVelocity = vec(),
    climbedStep = scalar(), hullMins = vec(), hullMaxs = vec()
  const sweepQueries = view.getUint32(at, true), pointQueries = view.getUint32(at + 4, true), contacts = view.getUint32(at + 8, true), events = view.getUint32(at + 12, true)
  at += 16
  const hasMover = data[at]
  if (hasMover === 0) {
    if (data[at + 1] !== 0 || data[at + 2] !== 0 || data[at + 3] !== 0 || at + 4 !== length) throw new Tf2CodecError("Movement tick mover absence is invalid")
    return Object.freeze({ mode: data[9]!, crouchPhase: data[10]!, grounded: data[11] === 1, wishDirection, wishSpeed, uncappedWishSpeed, wishVelocity, jumpVelocity, climbedStep, hullMins, hullMaxs, sweepQueries, pointQueries, contacts, events, mover: null })
  }
  const status = data[at + 1]
  if (hasMover !== 1 || status === undefined || status > 2 || data[at + 2] !== 0 || data[at + 3] !== 0 || at + 44 !== length) throw new Tf2CodecError("Movement tick mover record is invalid")
  at += 4
  const identity = view.getBigUint64(at, true); at += 8
  const displacement = vec(), supportVelocity = vec(), blockerValue = view.getBigUint64(at, true)
  return Object.freeze({
    mode: data[9]!, crouchPhase: data[10]!, grounded: data[11] === 1, wishDirection, wishSpeed, uncappedWishSpeed,
    wishVelocity, jumpVelocity, climbedStep, hullMins, hullMaxs, sweepQueries, pointQueries, contacts, events,
    mover: Object.freeze({ identity, status: status as 0 | 1 | 2, displacement, supportVelocity, blocker: blockerValue === 0xffff_ffff_ffff_ffffn ? null : blockerValue }),
  })
}
function decodeEntityPresentation(bytes:ArrayBuffer,offset:number,length:number):EntityPresentation{
  const data=new Uint8Array(bytes,offset,length),view=new DataView(bytes,offset,length);if(length<52||(length-52)%128!==0||new TextDecoder().decode(data.subarray(0,4))!=="PEBP"||view.getUint32(4,true)!==1)throw new Tf2CodecError("Entity presentation identity is invalid");const count=view.getUint32(48,true);if(52+count*128!==length)throw new Tf2CodecError("Entity presentation records do not frame bytes");const models:BrushModelDrawState[]=[];let prior=-1;for(let i=0;i<count;i++){const at=52+i*128,source=view.getUint32(at+8,true),model=view.getUint32(at+12,true),worldPosition=vector(view,at+40),worldAngles=vector(view,at+52),renderMode=data[at+65]!,renderFx=data[at+66]!,draw=data[at+67]!,kind=data[at+82]!,position=data[at+83]!,progress=view.getFloat32(at+84,true),request=view.getBigUint64(at+88,true),opening=data[at+120]!;if(source<=prior||model===0||draw>1||kind>3||position>5||opening>2||!finite([...worldPosition,...worldAngles,progress]))throw new Tf2CodecError("Entity presentation record is invalid");prior=source;models.push(Object.freeze({sourceIndex:source,model,worldPosition,worldAngles,renderMode,color:Object.freeze([data[at+76]!,data[at+77]!,data[at+78]!,data[at+79]!]),renderFx,effects:view.getUint16(at+80,true),draw:draw===1,mover:kind===0?null:Object.freeze({kind:kind as 1|2|3,position:position as 1|2|3|4|5,progress,requestId:request===0xffff_ffff_ffff_ffffn?null:request,opening:opening===0?null:opening===1})}))}return Object.freeze({sourceIdentity:view.getBigUint64(8,true),registryIdentity:view.getBigUint64(16,true),tick:view.getBigUint64(24,true),entityRevision:view.getBigUint64(32,true),collisionRevision:view.getBigUint64(40,true),models:Object.freeze(models)})}

function validateProjectileTransitions(events: readonly ProjectileEvent[]): void {
  const state = new Map<number, { kind: ProjectileKind; last?: ProjectileEventType; armed: boolean }>()
  for (const event of events) {
    const current = state.get(event.projectile) ?? { kind: event.kind, armed: false }
    if (current.kind !== event.kind || current.last === "fizzle" || current.last === "explode") {
      throw new Tf2CodecError("projectile event transition is illegal")
    }
    if (event.type === "fire" && current.last !== undefined) {
      throw new Tf2CodecError("projectile fire event is duplicated")
    }
    if (event.type === "impact" && current.last === "impact" && event.kind !== 2) {
      throw new Tf2CodecError("projectile impact event is duplicated")
    }
    if (event.type === "stick" && (event.kind !== 2 || current.last !== "impact")) {
      throw new Tf2CodecError("projectile stick event is illegal")
    }
    if (event.type === "arm") {
      if (event.kind !== 2 || current.armed) throw new Tf2CodecError("projectile arm event is illegal")
      current.armed = true
    }
    current.last = event.type
    state.set(event.projectile, current)
  }
}

function decodeRandomState(bytes: ArrayBuffer, offset: number, length: number): Tf2RandomState {
  if (length !== 284) throw new Tf2CodecError("TF2 random state length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "PRNG" || view.getUint32(4, true) !== 1) {
    throw new Tf2CodecError("TF2 random state identity is invalid")
  }
  let at = 8
  const stream = (): RandomStreamState => {
    const current = view.getInt32(at, true), shuffled = view.getInt32(at + 4, true)
    at += 8
    const table = Object.freeze(Array.from({ length: 32 }, () => {
      const value = view.getInt32(at, true)
      at += 4
      return value
    }))
    const initialized = current > 0 && current < 2_147_483_647 && shuffled > 0 && shuffled < 2_147_483_647 &&
      table.every((value) => value > 0 && value < 2_147_483_647)
    const uninitialized = shuffled === 0 && current <= 0 && current !== -2_147_483_648
    if (!initialized && !uninitialized) throw new Tf2CodecError("TF2 random stream state is invalid")
    return Object.freeze({ current, shuffled, table })
  }
  const authority = stream(), predictedPresentation = stream(), rocketExplosionAvailable = data[at]!, stickyExplosionAvailable = data[at + 1]!
  if ((rocketExplosionAvailable & ~7) !== 0 || (stickyExplosionAvailable & ~7) !== 0 || data[at + 2] !== 0 || data[at + 3] !== 0) {
    throw new Tf2CodecError("TF2 sound selection state is invalid")
  }
  return Object.freeze({ authority, predictedPresentation, rocketExplosionAvailable, stickyExplosionAvailable })
}

function decodeCollisionSnapshot(bytes: ArrayBuffer, offset: number, length: number): CollisionSnapshot {
  if (length < 52 || length > 16 * 1024 * 1024) throw new Tf2CodecError("Collision snapshot length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "CSNP" || view.getUint32(4, true) !== 3) {
    throw new Tf2CodecError("Collision snapshot identity is invalid")
  }
  const worldIdentity = Array.from(data.subarray(8, 40), (value) => value.toString(16).padStart(2, "0")).join("")
  if (!HASH.test(worldIdentity)) throw new Tf2CodecError("Collision world identity is invalid")
  return Object.freeze({ worldIdentity, identity: view.getBigUint64(40, true), objects: count(view.getUint32(48, true), "Collision object"), bytes: data })
}

export function decodeSnapshot(bytes: ArrayBuffer | Uint8Array): Snapshot {
  if (bytes.byteLength < 168 || bytes.byteLength > MAX_SNAPSHOT_BYTES) {
    throw new Tf2CodecError("snapshot byte length is invalid")
  }
  const data = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)
  const buffer = data.buffer as ArrayBuffer
  const base = data.byteOffset
  const view = new DataView(buffer, base, data.byteLength)
  if (data[0] !== 0x50 || data[1] !== 0x53 || data[2] !== 0x53 || data[3] !== 0x4e || view.getUint32(4, true) !== 11)
    throw new Tf2CodecError("snapshot identity is invalid")
  const tf2Class = data[16]
  const team = data[17]
  const weapon = data[18]
  if (
    (tf2Class === undefined || tf2Class < 1 || tf2Class > 9) ||
    (team !== 2 && team !== 3) ||
    (weapon !== 0 && weapon !== 1 && weapon !== 2 && weapon !== 3) ||
    data[19]! > 1 || (data[28] !== 1 && data[28] !== 2) || data[29] !== 0 || data[30] !== 0 || data[31] !== 0
  )
    throw new Tf2CodecError("snapshot selection is invalid")
  const health = view.getFloat32(20, true)
  const maximumHealth = view.getFloat32(24, true)
  if (!finite([health, maximumHealth]) || health < 0 || maximumHealth <= 0) {
    throw new Tf2CodecError("snapshot health is invalid")
  }
  const conditions = Object.freeze(Array.from({ length: 5 }, (_, index) => view.getUint32(32 + index * 4, true))) as
    readonly [number, number, number, number, number]
  const loadoutCount = count(view.getUint32(56, true), "loadout")
  const projectileCount = count(view.getUint32(60, true), "projectile")
  const projectileEventCount = count(view.getUint32(64, true), "projectile event")
  const entityTransformCount = count(view.getUint32(68, true), "entity transform")
  const entityEventCount = count(view.getUint32(72, true), "entity event")
  const gameplayEventCount = count(view.getUint32(76, true), "gameplay event")
  const jumpLength = view.getUint32(80, true)
  const movementLength = view.getUint32(84, true)
  const activityCount = count(view.getUint32(88, true), "activity")
  const lifecycleEventCount = count(view.getUint32(92, true), "lifecycle event")
  const physicsRequestCount = count(view.getUint32(96, true), "Physics request")
  const rocketRequestCount = count(view.getUint32(100, true), "rocket trace request")
  const radiusRequestCount = count(view.getUint32(104, true), "radius damage request")
  const moverRequestCount = count(view.getUint32(108, true), "mover request")
  const contactRequestCount = count(view.getUint32(112, true), "contact reconcile request")
  const mapEffectCount = count(view.getUint32(116, true), "map effect")
  const regenerateEventCount = count(view.getUint32(120, true), "regenerate animation event")
  const blockerCount = count(view.getUint32(124, true), "authority blocker")
  const randomDrawCount = count(view.getUint32(128, true), "random draw")
  const audioEventCount = count(view.getUint32(132, true), "audio event")
  const rocketResultCount = count(view.getUint32(136, true), "rocket result")
  const moverResultCount = count(view.getUint32(140, true), "mover result")
  const collisionSnapshotLength = view.getUint32(144, true)
  const randomStateLength = view.getUint32(148, true)
  const entityPresentationLength=view.getUint32(152,true),movementTickLength=view.getUint32(156,true)
  const playerFlags=view.getUint32(160,true),waterType=view.getUint32(164,true)
  if (entityPresentationLength<52||movementTickLength<12||168 + movementLength > bytes.byteLength||waterType&~0x30) {
    throw new Tf2CodecError("snapshot extension header is invalid")
  }
  const movement = movementSnapshot(buffer, base + 168, movementLength, waterType)
  if (movement.mode !== data[19]||movement.waterLevel>3||(movement.waterLevel===0)!==(waterType===0)) {
    throw new Tf2CodecError("Movement mode or water projection differs")
  }
  let at = 168 + movementLength
  const requireBytes = (length: number, label: string): void => {
    if (!Number.isSafeInteger(at + length) || at + length > bytes.byteLength) {
      throw new Tf2CodecError(`${label} records exceed snapshot bytes`)
    }
  }

  requireBytes(loadoutCount * 48, "loadout")
  const loadout: WeaponState[] = []
  for (let index = 0; index < loadoutCount; index += 1) {
    const item = at + index * 48
    const itemWeapon = data[item]
    const reload = data[item + 1]
    if (
      itemWeapon === undefined ||
      itemWeapon < 1 ||
      itemWeapon > 3 ||
      reload === undefined ||
      reload > 3 ||
      data[item + 2] !== 0 ||
      data[item + 3] !== 0 ||
      view.getUint32(item + 44, true) !== 0
    )
      throw new Tf2CodecError("loadout record is invalid")
    loadout.push(
      Object.freeze({
        weapon: itemWeapon as Tf2Weapon,
        reload: reload as WeaponState["reload"],
        clip: view.getUint16(item + 4, true),
        reserve: view.getUint16(item + 6, true),
        maximumClip: view.getUint16(item + 8, true),
        maximumReserve: view.getUint16(item + 10, true),
        nextPrimaryTick: view.getBigUint64(item + 12, true),
        reloadDueTick: view.getBigUint64(item + 20, true) === 0xffff_ffff_ffff_ffffn ? null : view.getBigUint64(item + 20, true),
        chargeBeginTick: view.getBigUint64(item + 28, true) === 0xffff_ffff_ffff_ffffn ? null : view.getBigUint64(item + 28, true),
        firstPrimaryTick: view.getBigUint64(item + 36, true),
      }),
    )
  }
  if ((weapon === 0 && loadout.length !== 0)
    || (weapon !== 0 && !loadout.some((entry) => entry.weapon === weapon))) {
    throw new Tf2CodecError("snapshot active weapon does not match its loadout")
  }
  at += loadoutCount * 48

  requireBytes(projectileCount * 84, "projectile")
  const projectiles: Projectile[] = []
  for (let index = 0; index < projectileCount; index += 1) {
    const item = at + index * 84
    const kind = data[item + 4]
    const projectileTeam = data[item + 5]
    const state = data[item + 6]
    const hasNormal = data[item + 7]
    const position = vector(view, item + 16)
    const velocity = vector(view, item + 28)
    const orientation = quaternion(view, item + 40)
    const angularVelocity = vector(view, item + 56)
    const rawNormal = vector(view, item + 68)
    const ageSeconds = view.getFloat32(item + 80, true)
    if (
      (kind !== 1 && kind !== 2) ||
      (projectileTeam !== 2 && projectileTeam !== 3) ||
      state === undefined ||
      state < 1 ||
      state > 3 ||
      hasNormal === undefined ||
      hasNormal > 1 ||
      !finite([...position, ...velocity, ...angularVelocity, ageSeconds]) ||
      !normalized(orientation) ||
      ageSeconds < 0 ||
      (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) ||
      (state !== 1 && (kind !== 2 || hasNormal !== 1))
    )
      throw new Tf2CodecError("projectile record is invalid")
    projectiles.push(
      Object.freeze({
        identity: view.getUint32(item, true),
        kind,
        team: projectileTeam,
        ownerIdentity: view.getUint32(item + 8, true),
        launcherIdentity: view.getUint32(item + 12, true),
        state: state as ProjectileState,
        position,
        velocity,
        orientation,
        angularVelocity,
        contactNormal: hasNormal === 1 ? rawNormal : null,
        ageSeconds,
      }),
    )
  }
  at += projectileCount * 84

  requireBytes(projectileEventCount * 92, "projectile event")
  const projectileEvents: ProjectileEvent[] = []
  const eventNames: readonly ProjectileEventType[] = ["fire", "impact", "stick", "arm", "fizzle", "explode"]
  for (let index = 0; index < projectileEventCount; index += 1) {
    const item = at + index * 92
    const eventCode = data[item]
    const kind = data[item + 1]
    const projectileTeam = data[item + 2]
    const flags = data[item + 3]
    const hasNormal = flags === undefined ? undefined : flags & 1
    const hasLauncherPose = flags === undefined ? undefined : (flags >> 1) & 1
    const position = vector(view, item + 24)
    const orientation = quaternion(view, item + 36)
    const rawNormal = vector(view, item + 52)
    const eyePosition = vector(view, item + 64)
    const viewOrientation = quaternion(view, item + 76)
    if (
      eventCode === undefined ||
      eventCode < 1 ||
      eventCode > 6 ||
      (kind !== 1 && kind !== 2) ||
      (projectileTeam !== 2 && projectileTeam !== 3) ||
      flags === undefined ||
      flags > 3 ||
      hasNormal === undefined ||
      hasLauncherPose === undefined ||
      !finite(position) ||
      !normalized(orientation) ||
      (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) ||
      ((eventCode === 2 || eventCode === 3) && hasNormal !== 1) ||
      (eventCode === 1) !== (hasLauncherPose === 1) ||
      (hasLauncherPose === 1 && (!finite(eyePosition) || !normalized(viewOrientation))) ||
      (hasLauncherPose === 0 && [...eyePosition, ...viewOrientation].some((value) => value !== 0))
    )
      throw new Tf2CodecError("projectile event record is invalid")
    projectileEvents.push(
      Object.freeze({
        type: eventNames[eventCode - 1]!,
        projectile: view.getUint32(item + 4, true),
        kind,
        ownerIdentity: view.getUint32(item + 8, true),
        launcherIdentity: view.getUint32(item + 12, true),
        team: projectileTeam,
        tick: view.getBigUint64(item + 16, true),
        position,
        orientation,
        contactNormal: hasNormal === 1 ? rawNormal : null,
        launcherPose: hasLauncherPose === 1
          ? Object.freeze({ eyePosition, viewOrientation })
          : null,
      }),
    )
  }
  validateProjectileTransitions(projectileEvents)
  at += projectileEventCount * 92

  requireBytes(entityTransformCount * 32, "entity transform")
  const entityTransforms: EntityTransform[] = []
  for (let index = 0; index < entityTransformCount; index += 1) {
    const item = at + index * 32
    const position = vector(view, item + 8)
    const angles = vector(view, item + 20)
    if (!finite([...position, ...angles])) throw new Tf2CodecError("entity transform scalar is invalid")
    entityTransforms.push(
      Object.freeze({
        identity: view.getUint32(item, true),
        model: view.getUint32(item + 4, true),
        position,
        angles,
      }),
    )
  }
  at += entityTransformCount * 32

  const entityEvents: EntityEvent[] = []
  const decoder = new TextDecoder("utf-8", { fatal: true })
  for (let index = 0; index < entityEventCount; index += 1) {
    requireBytes(24, "entity event")
    const kind = data[at + 8]
    const accepted = data[at + 9]
    const contact = data[at + 10]
    const nameLength = view.getUint32(at + 20, true)
    if (
      kind === undefined ||
      kind < 1 ||
      kind > 8 ||
      accepted === undefined ||
      accepted > 1 ||
      contact === undefined ||
      contact > 3 ||
      data[at + 11] !== 0 ||
      nameLength > 2_047
    )
      throw new Tf2CodecError("entity event record is invalid")
    requireBytes(24 + nameLength, "entity event")
    let name: string
    try {
      name = decoder.decode(data.subarray(at + 24, at + 24 + nameLength))
    } catch {
      throw new Tf2CodecError("entity event name is invalid UTF-8")
    }
    const subject = view.getUint32(at + 16, true)
    entityEvents.push(
      Object.freeze({
        sequence: view.getBigUint64(at, true),
        kind: kind as EntityEvent["kind"],
        accepted: accepted === 1,
        contact: contact === 0 ? null : (contact as 1 | 2 | 3),
        entity: view.getUint32(at + 12, true),
        subject: subject === 0xffff_ffff ? null : subject,
        name,
      }),
    )
    at += 24 + nameLength
  }

  requireBytes(gameplayEventCount * 28, "gameplay event")
  const events: GameplayEvent[] = []
  for (let index = 0; index < gameplayEventCount; index += 1) {
    const item = at + index * 28
    const kind = data[item]
    const values = Object.freeze([
      view.getFloat32(item + 12, true),
      view.getFloat32(item + 16, true),
      view.getFloat32(item + 20, true),
      view.getFloat32(item + 24, true),
    ]) as readonly [number, number, number, number]
    if (kind === undefined || kind < 1 || kind > 11 || data[item + 2] !== 0 || data[item + 3] !== 0 || !finite(values))
      throw new Tf2CodecError("gameplay event record is invalid")
    events.push(
      Object.freeze({
        kind: kind as GameplayEvent["kind"],
        detail: data[item + 1]!,
        subject: view.getUint32(item + 4, true),
        auxiliary: view.getUint32(item + 8, true),
        values,
      }),
    )
  }
  at += gameplayEventCount * 28

  requireBytes(activityCount * 16, "activity")
  const activities: ActivityEvent[] = []
  for (let index = 0; index < activityCount; index += 1) {
    const item = at + index * 16
    const itemWeapon = data[item + 8]
    const activity = data[item + 9]
    if (itemWeapon === undefined || itemWeapon < 1 || itemWeapon > 3 || activity === undefined || activity < 1 || activity > 6 ||
      !data.subarray(item + 10, item + 16).every((value) => value === 0)) {
      throw new Tf2CodecError("activity record is invalid")
    }
    activities.push(Object.freeze({
      tick: view.getBigUint64(item, true),
      weapon: itemWeapon as Tf2Weapon,
      activity: activity as WeaponActivity,
    }))
  }
  at += activityCount * 16

  requireBytes(lifecycleEventCount * 16, "lifecycle event")
  const lifecycleEvents: LifecycleEvent[] = []
  for (let index = 0; index < lifecycleEventCount; index += 1) {
    const item = at + index * 16
    const kind = data[item + 8], itemClass = data[item + 9], itemTeam = data[item + 10]
    if (kind === undefined || kind < 1 || kind > 4 || itemClass === undefined || itemClass < 1 || itemClass > 9 ||
      (itemTeam !== 2 && itemTeam !== 3) || !data.subarray(item + 11, item + 16).every((value) => value === 0)) {
      throw new Tf2CodecError("lifecycle event record is invalid")
    }
    lifecycleEvents.push(Object.freeze({
      tick: view.getBigUint64(item, true),
      kind: kind as LifecycleEvent["kind"],
      class: itemClass as Tf2Class,
      team: itemTeam,
    }))
  }
  at += lifecycleEventCount * 16

  requireBytes(physicsRequestCount * 104, "Physics request")
  const physicsRequests: ProjectilePhysicsRequest[] = []
  for (let index = 0; index < physicsRequestCount; index += 1) {
    const item = at + index * 104
    const operation = data[item]
    const position = vector(view, item + 16), velocity = vector(view, item + 28), orientation = quaternion(view, item + 40),
      angularVelocity = vector(view, item + 56), hullMins = vector(view, item + 68), hullMaxs = vector(view, item + 80)
    const gravityScale = view.getFloat32(item + 92, true), friction = view.getFloat32(item + 96, true), elasticity = view.getFloat32(item + 100, true)
    if (operation === undefined || operation < 1 || operation > 4 || !data.subarray(item + 1, item + 4).every((value) => value === 0) ||
      !finite([...position, ...velocity, ...angularVelocity, ...hullMins, ...hullMaxs, gravityScale, friction, elasticity]) ||
      !normalized(orientation) || hullMins.some((value, axis) => value > hullMaxs[axis]!)) {
      throw new Tf2CodecError("Physics request record is invalid")
    }
    physicsRequests.push(Object.freeze({ operation: operation as ProjectilePhysicsRequest["operation"], projectile: view.getUint32(item + 4, true),
      tick: view.getBigUint64(item + 8, true), position, velocity, orientation, angularVelocity, hullMins, hullMaxs, gravityScale, friction, elasticity }))
  }
  at += physicsRequestCount * 104

  requireBytes(rocketRequestCount * 44, "rocket trace request")
  const rocketTraceRequests: RocketTraceRequest[] = []
  for (let index = 0; index < rocketRequestCount; index += 1) {
    const item = at + index * 44, start = vector(view, item + 16), end = vector(view, item + 28)
    if (view.getUint32(item + 4, true) !== 0 || !finite([...start, ...end])) throw new Tf2CodecError("rocket trace request record is invalid")
    rocketTraceRequests.push(Object.freeze({ projectile: view.getUint32(item, true), tick: view.getBigUint64(item + 8, true), start, end, mask: view.getUint32(item + 40, true) }))
  }
  at += rocketRequestCount * 44

  requireBytes(radiusRequestCount * 36, "radius damage request")
  const radiusDamageRequests: RadiusDamageRequest[] = []
  for (let index = 0; index < radiusRequestCount; index += 1) {
    const item = at + index * 36, kind = data[item + 4], source = vector(view, item + 8)
    const values = [view.getFloat32(item + 20, true), view.getFloat32(item + 24, true), view.getFloat32(item + 28, true)]
    if ((kind !== 1 && kind !== 2) || !data.subarray(item + 5, item + 8).every((value) => value === 0) || !finite([...source, ...values])) {
      throw new Tf2CodecError("radius damage request record is invalid")
    }
    const directTarget = view.getUint32(item + 32, true)
    radiusDamageRequests.push(Object.freeze({ projectile: view.getUint32(item, true), kind, source, baseDamage: values[0]!, radius: values[1]!,
      selfRadius: values[2]!, directTarget: directTarget === 0xffff_ffff ? null : directTarget }))
  }
  at += radiusRequestCount * 36

  requireBytes(moverRequestCount * 48, "mover request")
  const moverRequests: MoverRequest[] = []
  for (let index = 0; index < moverRequestCount; index += 1) {
    const item = at + index * 48, start = vector(view, item + 16), destination = vector(view, item + 28), speed = view.getFloat32(item + 40, true)
    const opening = data[item + 44]
    if (opening === undefined || opening > 1 || !data.subarray(item + 45, item + 48).every((value) => value === 0) || !finite([...start, ...destination, speed])) {
      throw new Tf2CodecError("mover request record is invalid")
    }
    const model = view.getUint32(item + 12, true)
    moverRequests.push(Object.freeze({ requestId: view.getBigUint64(item, true), entity: view.getUint32(item + 8, true),
      model: model === 0xffff_ffff ? null : model, start, destination, speed, opening: opening === 1 }))
  }
  at += moverRequestCount * 48

  requireBytes(contactRequestCount * 44, "contact reconcile request")
  const contactReconcileRequests: ContactReconcileRequest[] = []
  for (let index = 0; index < contactRequestCount; index += 1) {
    const item = at + index * 44, position = vector(view, item + 8), hullMins = vector(view, item + 20), hullMaxs = vector(view, item + 32)
    if (!finite([...position, ...hullMins, ...hullMaxs]) || hullMins.some((value, axis) => value > hullMaxs[axis]!)) {
      throw new Tf2CodecError("contact reconcile request record is invalid")
    }
    contactReconcileRequests.push(Object.freeze({ tick: view.getBigUint64(item, true), position, hullMins, hullMaxs }))
  }
  at += contactRequestCount * 44

  requireBytes(mapEffectCount * 40, "map effect")
  const mapEffects: MapEffect[] = []
  for (let index = 0; index < mapEffectCount; index += 1) {
    const item = at + index * 40, kind = data[item], detail = data[item + 1], rawTeam = data[item + 2], rawContact = data[item + 3]
    const values = Object.freeze(Array.from({ length: 6 }, (_, value) => view.getFloat32(item + 16 + value * 4, true))) as
      readonly [number, number, number, number, number, number]
    if (kind === undefined || kind < 1 || kind > 5 || detail === undefined || detail > 1 || rawTeam === undefined ||
      (rawTeam !== 0 && rawTeam !== 2 && rawTeam !== 3) || rawContact === undefined || rawContact > 3 ||
      view.getUint32(item + 12, true) !== 0 || !finite(values)) throw new Tf2CodecError("map effect record is invalid")
    const auxiliary = view.getUint32(item + 8, true)
    const semantic = kind === 1
      ? rawTeam === 0 && rawContact === 0 && auxiliary !== 0xffff_ffff &&
        (detail === 1 || values.slice(3).every((value) => value === 0))
      : kind === 2
        ? detail === 0 && rawTeam === 0 && rawContact >= 1 && auxiliary === 0xffff_ffff &&
          values.slice(1).every((value) => value === 0)
        : kind === 3
          ? rawTeam === 0 && rawContact === 0 && auxiliary === 0xffff_ffff && values.slice(3).every((value) => value === 0)
          : kind === 4
            ? detail === 0 && rawContact === 0 && values.every((value) => value === 0)
            : detail === 0 && rawContact >= 1 && auxiliary === 0xffff_ffff && values.every((value) => value === 0)
    if (!semantic) throw new Tf2CodecError("map effect semantics are invalid")
    mapEffects.push(Object.freeze({ kind: kind as MapEffect["kind"], detail, team: rawTeam === 0 ? null : rawTeam,
      contact: rawContact === 0 ? null : rawContact as ContactKind, subject: view.getUint32(item + 4, true),
      auxiliary: auxiliary === 0xffff_ffff ? null : auxiliary, values }))
  }
  at += mapEffectCount * 40

  requireBytes(regenerateEventCount * 32, "regenerate animation event")
  const regenerateAnimationEvents: RegenerateAnimationEvent[] = []
  for (let index = 0; index < regenerateEventCount; index += 1) {
    const item = at + index * 32,openAnimation=data[item+28],closeAnimation=data[item+29]
    if((openAnimation!==1&&openAnimation!==2)||(closeAnimation!==1&&closeAnimation!==2)||data[item+30]!==0||data[item+31]!==0)throw new Tf2CodecError("regenerate animation event is invalid")
    regenerateAnimationEvents.push(Object.freeze({ zone: view.getUint32(item, true), associatedModel: view.getUint32(item + 4, true),
      openTick: view.getBigUint64(item + 8, true), closeTick: view.getBigUint64(item + 16, true),body:view.getInt32(item+24,true),openAnimation:openAnimation===1?"open":"close",closeAnimation:closeAnimation===1?"open":"close" }))
  }
  at += regenerateEventCount * 32

  requireBytes(blockerCount * 4, "authority blocker")
  const authorityBlockers: AuthorityBlocker[] = []
  const blockerDetails = new Map<number, string>([
    [1, "TF2 sticky IVP solver unavailable: current body/contact transition"],
    [2, "Tempus core and configured Jump course contract unavailable"],
  ])
  const blockerCodes = new Set<number>()
  for (let index = 0; index < blockerCount; index += 1) {
    const item = at + index * 4, code = data[item], classification = data[item + 1]
    const detail = code === undefined ? undefined : blockerDetails.get(code)
    if (!detail || classification !== 1 || data[item + 2] !== 0 || data[item + 3] !== 0 || blockerCodes.has(code)) {
      throw new Tf2CodecError("authority blocker record is invalid")
    }
    blockerCodes.add(code)
    authorityBlockers.push(Object.freeze({ code: code as 1 | 2, classification: "Missing", detail }))
  }
  if (blockerCodes.size !== 2 || !blockerCodes.has(1) || !blockerCodes.has(2)) {
    throw new Tf2CodecError("authority blocker set is incomplete")
  }
  at += blockerCount * 4

  requireBytes(randomStateLength, "TF2 random state")
  const randomState = decodeRandomState(buffer, base + at, randomStateLength)
  at += randomStateLength

  requireBytes(randomDrawCount * 16, "random draw")
  const randomDraws: RandomDraw[] = []
  for (let index = 0; index < randomDrawCount; index += 1) {
    const item = at + index * 16, context = data[item], decision = data[item + 1], definition = data[item + 2], phase = data[item + 3]
    const raw = view.getInt32(item + 4, true), resultKind = data[item + 8], resultValue = view.getUint32(item + 12, true)
    const soundDecision = decision !== undefined && decision >= 1 && decision <= 4
    if (
      (context !== 1 && context !== 2) || decision === undefined || decision < 1 || decision > 8 ||
      (soundDecision ? definition === undefined || definition < 1 || definition > 6 || (phase !== 1 && phase !== 2) : definition !== 0 || phase !== 0 || context !== 1) ||
      raw <= 0 || raw >= 2_147_483_647 || resultKind === undefined || resultKind < 1 || resultKind > 3 ||
      data[item + 9] !== 0 || data[item + 10] !== 0 || data[item + 11] !== 0 ||
      ((decision === 3 || decision === 7 || decision === 8) ? resultKind === 1 : resultKind !== 1) ||
      (resultKind === 3 && resultValue !== 0)
    ) throw new Tf2CodecError("random draw record is invalid")
    const result: RandomDraw["result"] = resultKind === 1
      ? Object.freeze({ kind: "float-bits", bits: resultValue })
      : resultKind === 2
        ? Object.freeze({ kind: "integer", value: view.getInt32(item + 12, true) })
        : Object.freeze({ kind: "rejected-integer" })
    randomDraws.push(Object.freeze({ context, decision, definition: definition as RandomDraw["definition"], phase: phase as RandomDraw["phase"], raw, result }))
  }
  at += randomDrawCount * 16

  requireBytes(audioEventCount * 52, "audio event")
  const audioEvents: AudioEvent[] = []
  const nextOrdinal = new Map<bigint, number>()
  for (let index = 0; index < audioEventCount; index += 1) {
    const item = at + index * 52, tick = view.getBigUint64(item, true), ordinal = view.getUint16(item + 8, true), identity = data[item + 10],
      definition = data[item + 11], sourceKind = data[item + 12], hasOwner = data[item + 13], wave = data[item + 14]
    const sourceIdentity = view.getUint32(item + 16, true), rawOwner = view.getUint32(item + 20, true), position = vector(view, item + 24)
    const volume = view.getFloat32(item + 36, true), pitch = view.getFloat32(item + 40, true), soundLevel = view.getFloat32(item + 44, true)
    const expectedOrdinal = nextOrdinal.get(tick) ?? 0, waveCount = definition === 4 || definition === 6 ? 3 : 1
    if (
      (identity !== 1 && identity !== 2) || definition === undefined || definition < 1 || definition > 6 ||
      (sourceKind !== 1 && sourceKind !== 2) || (hasOwner !== 0 && hasOwner !== 1) || data[item + 15] !== 0 ||
      ordinal !== expectedOrdinal || !canonicalIdentity(sourceIdentity) ||
      (hasOwner === 0 ? rawOwner !== 0xffff_ffff : !canonicalIdentity(rawOwner)) ||
      !finite([...position, volume, pitch, soundLevel]) || [volume, pitch, soundLevel].some((value) => value < 0 || value >= 1) ||
      wave === undefined || wave >= waveCount || view.getUint32(item + 48, true) !== 0
    ) throw new Tf2CodecError("audio event record is invalid")
    nextOrdinal.set(tick, expectedOrdinal + 1)
    audioEvents.push(Object.freeze({
      tick, ordinal, identity, definition, sourceKind, sourceIdentity,
      ownerIdentity: hasOwner === 1 ? rawOwner : null,
      position,
      samples: Object.freeze({ volume, pitch, wave, soundLevel }),
    }))
  }
  at += audioEventCount * 52

  requireBytes(rocketResultCount * 44, "rocket result")
  const rocketTraceResults: RocketTraceResult[] = []
  for (let index = 0; index < rocketResultCount; index += 1) {
    const item = at + index * 44, solid = data[item + 12], sky = data[item + 13], hasNormal = data[item + 14], hasTarget = data[item + 15]
    const end = vector(view, item + 16), rawNormal = vector(view, item + 28), rawTarget = view.getUint32(item + 40, true)
    if (
      solid === undefined || solid > 1 || sky === undefined || sky > 1 || hasNormal === undefined || hasNormal > 1 || hasTarget === undefined || hasTarget > 1 ||
      !canonicalIdentity(view.getUint32(item, true)) || !finite([...end, ...rawNormal]) ||
      (sky === 1 && solid !== 1) || (solid === 1 && sky === 0 && hasNormal !== 1) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) || (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasTarget === 0 ? rawTarget !== 0xffff_ffff : !canonicalIdentity(rawTarget)) || (solid === 0 && hasTarget === 1)
    ) throw new Tf2CodecError("rocket result record is invalid")
    rocketTraceResults.push(Object.freeze({
      projectile: view.getUint32(item, true), tick: view.getBigUint64(item + 4, true), end,
      solid: solid === 1, sky: sky === 1, normal: hasNormal === 1 ? rawNormal : null,
      directTarget: hasTarget === 1 ? rawTarget : null,
    }))
  }
  at += rocketResultCount * 44

  requireBytes(moverResultCount * 52, "mover result")
  const moverResults: MoverResult[] = []
  for (let index = 0; index < moverResultCount; index += 1) {
    const item = at + index * 52, kind = data[item + 12], position = vector(view, item + 16), angles = vector(view, item + 28), carry = vector(view, item + 40)
    if (!canonicalIdentity(view.getUint32(item + 8, true)) || kind === undefined || kind < 1 || kind > 5 ||
      data[item + 13] !== 0 || data[item + 14] !== 0 || data[item + 15] !== 0 || !finite([...position, ...angles, ...carry])) {
      throw new Tf2CodecError("mover result record is invalid")
    }
    moverResults.push(Object.freeze({ requestId: view.getBigUint64(item, true), entity: view.getUint32(item + 8, true), kind, position, angles, carry }))
  }
  at += moverResultCount * 52

  requireBytes(collisionSnapshotLength, "Collision snapshot")
  const collisionSnapshot = decodeCollisionSnapshot(buffer, base + at, collisionSnapshotLength)
  at += collisionSnapshotLength
  requireBytes(jumpLength, "Jump")
  const jump = decodeJump(buffer, base + at, jumpLength)
  at += jumpLength
  requireBytes(movementTickLength,"Movement tick");const movementTick=decodeMovementTick(buffer,base+at,movementTickLength);at+=movementTickLength
  requireBytes(entityPresentationLength,"Entity presentation");const entityPresentation=decodeEntityPresentation(buffer,base+at,entityPresentationLength);at+=entityPresentationLength
  if(at!==bytes.byteLength||entityPresentation.collisionRevision!==collisionSnapshot.identity)throw new Tf2CodecError("Entity presentation revision join is invalid")

  const tick = view.getBigUint64(8, true)
  const frozenProjectiles = Object.freeze(projectiles)
  const frozenProjectileEvents = Object.freeze(projectileEvents)
  return Object.freeze({
    tick,
    class: tf2Class as Tf2Class,
    team,
    weapon: weapon === 0 ? null : weapon as Tf2Weapon,
    playerFlags,
    inWater: (playerFlags & 0x400) !== 0,
    health,
    maximumHealth,
    lifecycle: data[28] as 1 | 2,
    conditions,
    respawnTouchCount: view.getUint32(52, true),
    movement,
    movementTick,
    position: movement.position,
    velocity: movement.velocity,
    grounded: movement.grounded,
    crouched: movement.crouchPhase >= 2,
    loadout: Object.freeze(loadout),
    projectiles: frozenProjectiles,
    projectileEvents: frozenProjectileEvents,
    projectileTimeline: Object.freeze([
      Object.freeze({ tick, projectiles: frozenProjectiles, events: frozenProjectileEvents }),
    ]),
    entityTransforms: Object.freeze(entityTransforms),
    entityEvents: Object.freeze(entityEvents),
    jump,
    events: Object.freeze(events),
    activities: Object.freeze(activities),
    lifecycleEvents: Object.freeze(lifecycleEvents),
    physicsRequests: Object.freeze(physicsRequests),
    rocketTraceRequests: Object.freeze(rocketTraceRequests),
    radiusDamageRequests: Object.freeze(radiusDamageRequests),
    moverRequests: Object.freeze(moverRequests),
    contactReconcileRequests: Object.freeze(contactReconcileRequests),
    mapEffects: Object.freeze(mapEffects),
    regenerateAnimationEvents: Object.freeze(regenerateAnimationEvents),
    randomState,
    randomDraws: Object.freeze(randomDraws),
    audioEvents: Object.freeze(audioEvents),
    rocketTraceResults: Object.freeze(rocketTraceResults),
    moverResults: Object.freeze(moverResults),
    collisionSnapshot,
    entityPresentation,
    authorityBlockers: Object.freeze(authorityBlockers),
  })
}

export async function mapDerivedKey(
  bspSha256: string,
  profile: 0 | 1,
  renderLevel: 0 | 1 | 2,
  compilerSha256: string,
  configuration: Uint8Array,
): Promise<string> {
  if (!HASH.test(bspSha256) || !HASH.test(compilerSha256) || (renderLevel === 2) !== (profile === 1))
    throw new Tf2CodecError("BSP, compiler, or render profile identity is invalid")
  const configurationHash = new Uint8Array(await crypto.subtle.digest("SHA-256", configuration))
  const identity = new TextEncoder().encode(
    `playsrc-map-runtime-9\n${bspSha256}\n${compilerSha256}\n${profile}\n${renderLevel}\n${Array.from(
      configurationHash,
      (value) => value.toString(16).padStart(2, "0"),
    ).join("")}\n`,
  )
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", identity))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
