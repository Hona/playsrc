import type { ParticleRenderItem } from "@playsrc/particle"
import type { Camera, Effect } from "@playsrc/rendering"
import type { Snapshot } from "./codec"

const UINT32_MAX = 0xffff_ffff
const NORMAL_TOLERANCE = 1e-4

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
  voice: number
  resource: string
  gain: number
  pan: number
  loop: false
}>

export function tf2Audio(events: Snapshot["events"]): readonly Tf2AudioRequest[] {
  const requests: Tf2AudioRequest[] = []
  for (const event of events) {
    let resource: string | undefined
    if (event.kind === 3) {
      resource = event.detail === 1
        ? "sound/weapons/rocket_shoot.wav"
        : event.detail === 2
          ? "sound/weapons/stickybomblauncher_shoot.wav"
          : undefined
    } else if (event.kind === 4 && (event.detail === 1 || event.detail === 2)) {
      const ordinal = event.subject % 3 + 1
      resource = event.detail === 1
        ? `sound/weapons/explode${ordinal}.wav`
        : `sound/weapons/pipe_bomb${ordinal}.wav`
    }
    if (resource) {
      requests.push(Object.freeze({
        voice: event.subject * 2 + event.kind,
        resource,
        gain: 1,
        pan: 0,
        loop: false,
      }))
    }
  }
  return Object.freeze(requests)
}

export function tf2Hud(snapshot: Snapshot): Tf2Hud {
  return Object.freeze({
    health: snapshot.health,
    maxHealth: snapshot.class === 1 ? 200 : 175,
    className: snapshot.class === 1 ? "Soldier" : "Demoman",
    weaponName: snapshot.weapon === 1
      ? "Rocket Launcher"
      : snapshot.weapon === 2
        ? "Original"
        : "Stickybomb Launcher",
    speed: Math.hypot(...snapshot.velocity),
    projectileCount: snapshot.projectiles.length,
  })
}

export function tf2Camera(snapshot: Snapshot, yawDegrees: number, pitchDegrees: number): Camera {
  return Object.freeze({
    position: Object.freeze([
      snapshot.position[0],
      snapshot.position[1],
      snapshot.position[2] + (snapshot.crouched ? 45 : 68),
    ]) as readonly [number, number, number],
    yawDegrees,
    pitchDegrees,
    verticalFovDegrees: 74,
    near: 1,
    far: 32_768,
  })
}

export function particleEffects(items: readonly ParticleRenderItem[]): readonly Effect[] {
  return Object.freeze(items.map((item) => Object.freeze({
    identity: item.identity,
    position: item.position,
    radius: item.radius,
    color: item.color,
    opacity: item.opacity,
  })))
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
          const trailAttachment = fact.kind === "rocket"
            ? requireAttachment(catalog, fact.identity, "trail")
            : null
          push(particles, limits, startRequest(
            eventIdentity,
            trailEffect(fact.identity),
            fact,
            trail,
            trailAttachment,
            event.position,
            event.orientation,
          ))
          const muzzle = fact.kind === "rocket" ? "rocketbackblast" : "muzzle_pipelauncher"
          const attachment = requireAttachment(
            catalog,
            fact.launcherIdentity,
            fact.kind === "rocket" ? "backblast" : "muzzle",
          )
          requireSystem(catalog, muzzle)
          push(particles, limits, startRequest(
            eventIdentity,
            oneShotEffect(eventIdentity),
            fact,
            muzzle,
            attachment,
            event.position,
            event.orientation,
          ))
          next.set(fact.identity, Object.freeze({
            kind: fact.kind,
            team: fact.team,
            ownerIdentity: fact.ownerIdentity,
            launcherIdentity: fact.launcherIdentity,
            state: "flying",
            trailActive: true,
            pulseActive: false,
          }))
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
          push(particles, limits, Object.freeze({
            kind: "set-control-point",
            identity: `${eventIdentity}:trail-contact`,
            effectIdentity: trailEffect(event.projectileIdentity),
            eventIdentity,
            projectileIdentity: event.projectileIdentity,
            controlPoint: controlPoint(event.position, settledOrientation(event.orientation, event.contactNormal), event.ownerIdentity),
          }))
          next.set(event.projectileIdentity, Object.freeze({ ...prior, state: "stuck-unarmed" }))
        } else if (event.kind === "arm") {
          if (prior.kind !== "sticky" || prior.state !== "stuck-unarmed" || !fact) {
            throw transition("arm requires a stuck-unarmed sticky fact")
          }
          const system = fact.team === "red" ? "stickybomb_pulse_red" : "stickybomb_pulse_blue"
          requireSystem(catalog, system)
          push(particles, limits, startRequest(
            eventIdentity,
            pulseEffect(fact.identity),
            fact,
            system,
            null,
            event.position,
            settledOrientation(event.orientation, event.contactNormal),
          ))
          next.set(event.projectileIdentity, Object.freeze({ ...prior, state: "stuck-armed", pulseActive: true }))
        } else if (event.kind === "fizzle" || event.kind === "explode") {
          if (prior.trailActive) push(particles, limits, stopRequest(eventIdentity, trailEffect(event.projectileIdentity), event.projectileIdentity))
          if (prior.pulseActive) push(particles, limits, stopRequest(eventIdentity, pulseEffect(event.projectileIdentity), event.projectileIdentity))
          if (event.kind === "explode") {
            const system = event.contactNormal === null ? "ExplosionCore_MidAir" : "ExplosionCore_Wall"
            requireSystem(catalog, system)
            const eventFact: ProjectileFact = fact ?? Object.freeze({
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
            push(particles, limits, startRequest(
              eventIdentity,
              oneShotEffect(eventIdentity),
              eventFact,
              system,
              null,
              event.position,
              event.contactNormal === null
                ? event.orientation
                : settledOrientation(event.orientation, event.contactNormal),
            ))
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
          fact.kind !== state.kind
          || fact.team !== state.team
          || fact.ownerIdentity !== state.ownerIdentity
          || fact.launcherIdentity !== state.launcherIdentity
        ) {
          throw transition("projectile immutable identity fields changed")
        }
      }
      for (const identity of facts.keys()) {
        if (!next.has(identity)) throw transition("projectile fact has no fire event")
      }

      const models = frame.projectiles.map((fact) => {
        const model = fact.kind === "rocket"
          ? "models/weapons/w_models/w_rocket.mdl" as const
          : "models/weapons/w_models/w_stickybomb.mdl" as const
        if (!catalog.models.has(model)) {
          throw new ProjectilePresentationError("MissingModel", `projectile model ${model} is missing`)
        }
        const orientation = fact.kind === "sticky" && fact.state !== "flying"
          ? settledOrientation(fact.orientation, fact.contactNormal)
          : authoredRocketOrientation(fact.orientation)
        return Object.freeze({
          identity: fact.identity,
          projectileIdentity: fact.identity,
          model,
          skin: fact.team === "red" ? 0 as const : 1 as const,
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
        events: Object.freeze(frame.events.map((event) => Object.freeze({
          ...event,
          position: vector(event.position),
          orientation: quat(event.orientation),
          contactNormal: event.contactNormal === null ? null : vector(event.contactNormal),
        }))),
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
    !uint32(fact.identity)
    || !uint32(fact.ownerIdentity)
    || !uint32(fact.launcherIdentity)
    || (fact.kind !== "rocket" && fact.kind !== "sticky")
    || (fact.team !== "red" && fact.team !== "blue")
    || (fact.state !== "flying" && fact.state !== "stuck-unarmed" && fact.state !== "stuck-armed")
    || !finite(fact.position)
    || !finite(fact.velocity)
    || !quaternion(fact.orientation)
    || !finite(fact.angularVelocity)
    || !Number.isFinite(fact.ageSeconds)
    || fact.ageSeconds < 0
    || (fact.contactNormal !== null && !normal(fact.contactNormal))
    || (fact.state === "flying" && fact.contactNormal !== null)
    || (fact.state !== "flying" && fact.contactNormal === null)
    || (fact.kind === "rocket" && fact.state !== "flying")
  ) {
    throw new ProjectilePresentationError("MalformedFact", "projectile fact violates the frozen contract")
  }
}

function validateEvent(event: ProjectileEvent, frameTick: bigint): void {
  if (
    !["fire", "impact", "stick", "arm", "fizzle", "explode"].includes(event.kind)
    || !uint32(event.projectileIdentity)
    || !uint32(event.ownerIdentity)
    || !uint32(event.launcherIdentity)
    || (event.team !== "red" && event.team !== "blue")
    || typeof event.tick !== "bigint"
    || event.tick < 0n
    || event.tick > frameTick
    || !finite(event.position)
    || !quaternion(event.orientation)
    || (event.contactNormal !== null && !normal(event.contactNormal))
    || (event.kind === "fire" && event.contactNormal !== null)
    || ((event.kind === "stick" || event.kind === "arm") && event.contactNormal === null)
  ) {
    throw new ProjectilePresentationError("MalformedEvent", "projectile event violates the frozen contract")
  }
}

function matchEvent(event: ProjectileEvent, fact: ProjectileFact): void {
  if (
    event.ownerIdentity !== fact.ownerIdentity
    || event.launcherIdentity !== fact.launcherIdentity
    || event.team !== fact.team
  ) throw transition("event identity fields do not match the projectile fact")
}

function matchTracked(event: ProjectileEvent, tracked: TrackedProjectile): void {
  if (
    event.ownerIdentity !== tracked.ownerIdentity
    || event.launcherIdentity !== tracked.launcherIdentity
    || event.team !== tracked.team
  ) throw transition("event identity fields do not match retained projectile state")
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

function stopRequest(eventIdentity: string, effectIdentity: string, projectileIdentity: number): ProjectileParticleRequest {
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
  const m00 = x[0], m01 = y[0], m02 = z[0]
  const m10 = x[1], m11 = y[1], m12 = z[1]
  const m20 = x[2], m21 = y[2], m22 = z[2]
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
  if (!positiveInteger(limits.maxProjectiles) || !positiveInteger(limits.maxEvents) || !positiveInteger(limits.maxRequests)) {
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
