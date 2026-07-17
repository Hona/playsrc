const HASH = /^[0-9a-f]{64}$/
const MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024

export type Tf2Class = 1 | 2
export type Tf2Weapon = 1 | 2 | 3
export type ProjectileKind = 1 | 2

export type Command = Readonly<{
  forward: number
  side: number
  yawDegrees: number
  pitchDegrees: number
  jump: boolean
  crouch: boolean
  fire: boolean
  detonate: boolean
  selectClass?: Tf2Class
  selectWeapon?: Tf2Weapon
}>

export type Projectile = Readonly<{
  id: number
  kind: ProjectileKind
  armed: boolean
  stuck: boolean
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  age: number
}>

export type PresentationEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7
  detail: number
  subject: number
  values: readonly [number, number, number, number]
}>

export type Snapshot = Readonly<{
  tick: bigint
  class: Tf2Class
  weapon: Tf2Weapon
  grounded: boolean
  crouched: boolean
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  health: number
  projectiles: readonly Projectile[]
  events: readonly PresentationEvent[]
}>

export class Tf2CodecError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2CodecError"
  }
}

export function encodeCommand(command: Command): ArrayBuffer {
  const scalars = [command.forward, command.side, command.yawDegrees, command.pitchDegrees]
  if (!scalars.every(Number.isFinite)) throw new Tf2CodecError("command contains a non-finite scalar")
  if (command.selectClass !== undefined && command.selectClass !== 1 && command.selectClass !== 2) {
    throw new Tf2CodecError("command class selector is invalid")
  }
  if (
    command.selectWeapon !== undefined
    && command.selectWeapon !== 1
    && command.selectWeapon !== 2
    && command.selectWeapon !== 3
  ) {
    throw new Tf2CodecError("command weapon selector is invalid")
  }
  const bytes = new ArrayBuffer(24)
  const view = new DataView(bytes)
  scalars.forEach((value, index) => view.setFloat32(index * 4, value, true))
  const flags = Number(command.jump)
    | Number(command.crouch) << 1
    | Number(command.fire) << 2
    | Number(command.detonate) << 3
  view.setUint32(16, flags, true)
  view.setUint32(20, (command.selectClass ?? 0) | ((command.selectWeapon ?? 0) << 8), true)
  return bytes
}

function vector(view: DataView, offset: number): readonly [number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
  ]) as readonly [number, number, number]
}

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

export function decodeSnapshot(bytes: ArrayBuffer): Snapshot {
  if (bytes.byteLength < 56 || bytes.byteLength > MAX_SNAPSHOT_BYTES) {
    throw new Tf2CodecError("snapshot byte length is invalid")
  }
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  if (
    data[0] !== 0x50
    || data[1] !== 0x53
    || data[2] !== 0x53
    || data[3] !== 0x4e
    || view.getUint32(4, true) !== 1
  ) {
    throw new Tf2CodecError("snapshot identity is invalid")
  }
  const tf2Class = data[16]
  const weapon = data[17]
  if ((tf2Class !== 1 && tf2Class !== 2) || (weapon !== 1 && weapon !== 2 && weapon !== 3)) {
    throw new Tf2CodecError("snapshot selection is invalid")
  }
  if (data[18]! > 1 || data[19]! > 1) throw new Tf2CodecError("snapshot player flags are invalid")
  const position = vector(view, 20)
  const velocity = vector(view, 32)
  const health = view.getFloat32(44, true)
  if (!finite([...position, ...velocity, health])) throw new Tf2CodecError("snapshot player scalar is invalid")
  const projectileCount = view.getUint32(48, true)
  const eventCountOffset = 52 + projectileCount * 36
  if (!Number.isSafeInteger(eventCountOffset) || eventCountOffset + 4 > bytes.byteLength) {
    throw new Tf2CodecError("snapshot projectile records exceed its bytes")
  }
  const projectiles: Projectile[] = []
  for (let index = 0; index < projectileCount; index += 1) {
    const offset = 52 + index * 36
    const kind = data[offset + 4]
    if ((kind !== 1 && kind !== 2) || data[offset + 5]! > 1 || data[offset + 6]! > 1 || data[offset + 7] !== 0) {
      throw new Tf2CodecError("snapshot projectile record is invalid")
    }
    const projectilePosition = vector(view, offset + 8)
    const projectileVelocity = vector(view, offset + 20)
    const age = view.getFloat32(offset + 32, true)
    if (!finite([...projectilePosition, ...projectileVelocity, age])) {
      throw new Tf2CodecError("snapshot projectile scalar is invalid")
    }
    projectiles.push(Object.freeze({
      id: view.getUint32(offset, true),
      kind,
      armed: data[offset + 5] === 1,
      stuck: data[offset + 6] === 1,
      position: projectilePosition,
      velocity: projectileVelocity,
      age,
    }))
  }
  const eventCount = view.getUint32(eventCountOffset, true)
  const expectedLength = eventCountOffset + 4 + eventCount * 24
  if (!Number.isSafeInteger(expectedLength) || expectedLength !== bytes.byteLength) {
    throw new Tf2CodecError("snapshot event records do not frame its bytes")
  }
  const events: PresentationEvent[] = []
  for (let index = 0; index < eventCount; index += 1) {
    const offset = eventCountOffset + 4 + index * 24
    const kind = data[offset]
    if (kind === undefined || kind < 1 || kind > 7 || data[offset + 2] !== 0 || data[offset + 3] !== 0) {
      throw new Tf2CodecError("snapshot event record is invalid")
    }
    const values = Object.freeze([
      view.getFloat32(offset + 8, true),
      view.getFloat32(offset + 12, true),
      view.getFloat32(offset + 16, true),
      view.getFloat32(offset + 20, true),
    ]) as readonly [number, number, number, number]
    if (!finite(values)) throw new Tf2CodecError("snapshot event scalar is invalid")
    events.push(Object.freeze({
      kind: kind as PresentationEvent["kind"],
      detail: data[offset + 1]!,
      subject: view.getUint32(offset + 4, true),
      values,
    }))
  }
  return Object.freeze({
    tick: view.getBigUint64(8, true),
    class: tf2Class,
    weapon,
    grounded: data[18] === 1,
    crouched: data[19] === 1,
    position,
    velocity,
    health,
    projectiles: Object.freeze(projectiles),
    events: Object.freeze(events),
  })
}

export async function mapDerivedKey(
  bspSha256: string,
  profile: 0 | 1,
  configuration: Uint8Array,
): Promise<string> {
  if (!HASH.test(bspSha256)) throw new Tf2CodecError("BSP identity is invalid")
  const configurationHash = new Uint8Array(await crypto.subtle.digest("SHA-256", configuration))
  const identity = new TextEncoder().encode(
    `playsrc-map-runtime-1\n${bspSha256}\n${profile}\n${Array.from(
      configurationHash,
      (value) => value.toString(16).padStart(2, "0"),
    ).join("")}\n`,
  )
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", identity))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
