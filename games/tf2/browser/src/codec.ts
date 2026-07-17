const HASH = /^[0-9a-f]{64}$/
const MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024
const MAX_RECORDS = 65_536
const MOVEMENT_BYTES = 96

export type Tf2Class = 1 | 2
export type Tf2Team = 1 | 2
export type Tf2Weapon = 1 | 2 | 3
export type MovementMode = 0 | 1
export type ProjectileKind = 1 | 2
export type ProjectileState = 1 | 2 | 3

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
  selectClass?: Tf2Class
  selectTeam?: Tf2Team
  selectWeapon?: Tf2Weapon
  modeRequest?: MovementMode
  activateEntity?: number
}>

export type MovementSnapshot = Readonly<{
  mode: MovementMode
  crouchPhase: 0 | 1 | 2 | 3 | 4
  waterLevel: number
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
  reload: 0 | 1 | 2
  clip: number
  reserve: number
  maximumClip: number
  maximumReserve: number
  nextPrimaryTick: bigint
  nextReloadTick: bigint
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
}>

export type EntityTransform = Readonly<{
  identity: number
  model: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>

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
  weapon: Tf2Weapon
  health: number
  maximumHealth: number
  conditions: number
  movement: MovementSnapshot
  movementTick: MovementTick | null
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  grounded: boolean
  crouched: boolean
  loadout: readonly WeaponState[]
  projectiles: readonly Projectile[]
  projectileEvents: readonly ProjectileEvent[]
  entityTransforms: readonly EntityTransform[]
  entityEvents: readonly EntityEvent[]
  jump: JumpSnapshot | null
  events: readonly GameplayEvent[]
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
  if (command.selectClass !== undefined && command.selectClass !== 1 && command.selectClass !== 2) {
    throw new Tf2CodecError("command class selector is invalid")
  }
  if (command.selectTeam !== undefined && command.selectTeam !== 1 && command.selectTeam !== 2) {
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
  const bytes = new ArrayBuffer(40)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x43, 0x4d, 0x44])
  view.setUint32(4, 2, true)
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

function movementSnapshot(bytes: ArrayBuffer, offset: number, length: number): MovementSnapshot {
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

function decodeMovementTick(bytes: ArrayBuffer, offset: number): MovementTick | null {
  const length = bytes.byteLength - offset
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
    if (event.type === "impact" && current.last === "impact") {
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

export function decodeSnapshot(bytes: ArrayBuffer): Snapshot {
  if (bytes.byteLength < 64 || bytes.byteLength > MAX_SNAPSHOT_BYTES) {
    throw new Tf2CodecError("snapshot byte length is invalid")
  }
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  if (data[0] !== 0x50 || data[1] !== 0x53 || data[2] !== 0x53 || data[3] !== 0x4e || view.getUint32(4, true) !== 4)
    throw new Tf2CodecError("snapshot identity is invalid")
  const tf2Class = data[16]
  const team = data[17]
  const weapon = data[18]
  if (
    (tf2Class !== 1 && tf2Class !== 2) ||
    (team !== 1 && team !== 2) ||
    (weapon !== 1 && weapon !== 2 && weapon !== 3) ||
    data[19]! > 1
  )
    throw new Tf2CodecError("snapshot selection is invalid")
  const health = view.getFloat32(20, true)
  const maximumHealth = view.getFloat32(24, true)
  if (!finite([health, maximumHealth]) || health < 0 || maximumHealth <= 0) {
    throw new Tf2CodecError("snapshot health is invalid")
  }
  const loadoutCount = count(view.getUint32(32, true), "loadout")
  const projectileCount = count(view.getUint32(36, true), "projectile")
  const projectileEventCount = count(view.getUint32(40, true), "projectile event")
  const entityTransformCount = count(view.getUint32(44, true), "entity transform")
  const entityEventCount = count(view.getUint32(48, true), "entity event")
  const gameplayEventCount = count(view.getUint32(52, true), "gameplay event")
  const jumpLength = view.getUint32(56, true)
  const movementLength = view.getUint32(60, true)
  if (64 + movementLength > bytes.byteLength) throw new Tf2CodecError("Movement section exceeds snapshot bytes")
  const movement = movementSnapshot(bytes, 64, movementLength)
  if (movement.mode !== data[19]) throw new Tf2CodecError("Movement mode projection differs")
  let at = 64 + movementLength
  const requireBytes = (length: number, label: string): void => {
    if (!Number.isSafeInteger(at + length) || at + length > bytes.byteLength) {
      throw new Tf2CodecError(`${label} records exceed snapshot bytes`)
    }
  }

  requireBytes(loadoutCount * 32, "loadout")
  const loadout: WeaponState[] = []
  for (let index = 0; index < loadoutCount; index += 1) {
    const item = at + index * 32
    const itemWeapon = data[item]
    const reload = data[item + 1]
    if (
      itemWeapon === undefined ||
      itemWeapon < 1 ||
      itemWeapon > 3 ||
      reload === undefined ||
      reload > 2 ||
      data[item + 2] !== 0 ||
      data[item + 3] !== 0 ||
      view.getUint32(item + 28, true) !== 0
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
        nextReloadTick: view.getBigUint64(item + 20, true),
      }),
    )
  }
  at += loadoutCount * 32

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
      (projectileTeam !== 1 && projectileTeam !== 2) ||
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
      (state !== 1 && (kind !== 2 || hasNormal !== 1 || velocity.some((value) => value !== 0)))
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

  requireBytes(projectileEventCount * 64, "projectile event")
  const projectileEvents: ProjectileEvent[] = []
  const eventNames: readonly ProjectileEventType[] = ["fire", "impact", "stick", "arm", "fizzle", "explode"]
  for (let index = 0; index < projectileEventCount; index += 1) {
    const item = at + index * 64
    const eventCode = data[item]
    const kind = data[item + 1]
    const projectileTeam = data[item + 2]
    const hasNormal = data[item + 3]
    const position = vector(view, item + 24)
    const orientation = quaternion(view, item + 36)
    const rawNormal = vector(view, item + 52)
    if (
      eventCode === undefined ||
      eventCode < 1 ||
      eventCode > 6 ||
      (kind !== 1 && kind !== 2) ||
      (projectileTeam !== 1 && projectileTeam !== 2) ||
      hasNormal === undefined ||
      hasNormal > 1 ||
      !finite(position) ||
      !normalized(orientation) ||
      (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) ||
      ((eventCode === 2 || eventCode === 3) && hasNormal !== 1)
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
      }),
    )
  }
  validateProjectileTransitions(projectileEvents)
  at += projectileEventCount * 64

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
  requireBytes(jumpLength, "Jump")
  const jump = decodeJump(bytes, at, jumpLength)
  at += jumpLength
  const movementTick = decodeMovementTick(bytes, at)
  at = bytes.byteLength

  return Object.freeze({
    tick: view.getBigUint64(8, true),
    class: tf2Class,
    team,
    weapon,
    health,
    maximumHealth,
    conditions: view.getUint32(28, true),
    movement,
    movementTick,
    position: movement.position,
    velocity: movement.velocity,
    grounded: movement.grounded,
    crouched: movement.crouchPhase >= 2,
    loadout: Object.freeze(loadout),
    projectiles: Object.freeze(projectiles),
    projectileEvents: Object.freeze(projectileEvents),
    entityTransforms: Object.freeze(entityTransforms),
    entityEvents: Object.freeze(entityEvents),
    jump,
    events: Object.freeze(events),
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
