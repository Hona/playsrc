import { describe, expect, test } from "bun:test"
import {
  createProjectilePresentationMapper,
  ProjectilePresentationError,
  type ProjectileEvent,
  type ProjectileFact,
  type ProjectileFrame,
  type ProjectileResourceCatalog,
  type Quaternion,
  type Vector3,
} from "../src/presentation"

const systems = new Set([
  "rockettrail",
  "rocketbackblast",
  "stickybombtrail_red",
  "stickybombtrail_blue",
  "stickybomb_pulse_red",
  "stickybomb_pulse_blue",
  "muzzle_pipelauncher",
  "ExplosionCore_Wall",
  "ExplosionCore_MidAir",
])

function catalog(suppliedSystems: ReadonlySet<string> = systems): ProjectileResourceCatalog {
  return Object.freeze({
    models: new Set([
      "models/weapons/w_models/w_rocket.mdl",
      "models/weapons/w_models/w_stickybomb.mdl",
    ]),
    systems: suppliedSystems,
    attachments: new Map([
      [7, new Set(["trail"])],
      [20, new Set(["backblast"])],
      [21, new Set(["muzzle"])],
    ]),
  })
}

function rocket(overrides: Partial<ProjectileFact> = {}): ProjectileFact {
  return Object.freeze({
    identity: 7,
    kind: "rocket",
    team: "blue",
    ownerIdentity: 10,
    launcherIdentity: 20,
    state: "flying",
    position: Object.freeze([1, 2, 3]) as Vector3,
    velocity: Object.freeze([-100, 500, 9]) as Vector3,
    orientation: Object.freeze([0, 0, Math.SQRT1_2, Math.SQRT1_2]) as Quaternion,
    angularVelocity: Object.freeze([0, 0, 0]) as Vector3,
    contactNormal: null,
    ageSeconds: 0.25,
    ...overrides,
  })
}

function sticky(state: "flying" | "stuck-unarmed" | "stuck-armed", team: "red" | "blue" = "red"): ProjectileFact {
  return Object.freeze({
    identity: 9,
    kind: "sticky",
    team,
    ownerIdentity: 11,
    launcherIdentity: 21,
    state,
    position: Object.freeze([4, 5, 6]) as Vector3,
    velocity: Object.freeze([7, 8, 9]) as Vector3,
    orientation: Object.freeze([0, 0, 0, 1]) as Quaternion,
    angularVelocity: Object.freeze([100, -200, 300]) as Vector3,
    contactNormal: state === "flying" ? null : Object.freeze([0, 1, 0]) as Vector3,
    ageSeconds: 0.5,
  })
}

function event(fact: ProjectileFact, kind: ProjectileEvent["kind"], tick: bigint): ProjectileEvent {
  return Object.freeze({
    kind,
    projectileIdentity: fact.identity,
    ownerIdentity: fact.ownerIdentity,
    launcherIdentity: fact.launcherIdentity,
    team: fact.team,
    tick,
    position: fact.position,
    orientation: fact.orientation,
    contactNormal: fact.contactNormal,
  })
}

function frame(tick: bigint, projectiles: readonly ProjectileFact[], events: readonly ProjectileEvent[]): ProjectileFrame {
  return Object.freeze({ tick, projectiles: Object.freeze(projectiles), events: Object.freeze(events) })
}

describe("TF2 projectile presentation contract", () => {
  test("preserves axis-aligned and arbitrary transported rocket quaternions", () => {
    const orientations: readonly Quaternion[] = [
      [0, 0, 0, 1],
      [0, 0, Math.SQRT1_2, Math.SQRT1_2],
      [0, -Math.SQRT1_2, 0, Math.SQRT1_2],
      [0.1825741858, 0.3651483717, 0.5477225575, 0.7302967433],
    ]
    for (const [index, orientation] of orientations.entries()) {
      const mapper = createProjectilePresentationMapper(catalog())
      const fact = rocket({ orientation, velocity: [999 - index, -40 + index, 0.25] })
      expect(mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)])).models[0]!.orientation)
        .toEqual(orientation)
    }
  })

  test("uses transported rocket orientation, BLU skin, and exact trail/backblast resources", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const fact = rocket()
    const result = mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)]))
    expect(result.models).toEqual([{
      identity: 7,
      projectileIdentity: 7,
      model: "models/weapons/w_models/w_rocket.mdl",
      skin: 1,
      materialVariant: "blue",
      position: [1, 2, 3],
      orientation: fact.orientation,
      angularVelocity: [0, 0, 0],
      state: "flying",
    }])
    expect(result.particles.map((request) => request.kind === "start" && [request.system, request.attachment])).toEqual([
      ["rockettrail", { entityIdentity: 7, name: "trail" }],
      ["rocketbackblast", { entityIdentity: 20, name: "backblast" }],
    ])
    expect(Object.isFrozen(result.models[0]!.orientation)).toBe(true)
    expect(fact.velocity).toEqual([-100, 500, 9])
  })

  test("preserves sticky spin, settles to contact, arms by team, then cleans up on explosion", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const flying = sticky("flying")
    const fired = mapper.map(frame(1n, [flying], [event(flying, "fire", 1n)]))
    expect(fired.models[0]).toMatchObject({ skin: 0, angularVelocity: [100, -200, 300] })
    expect(fired.particles.filter((request) => request.kind === "start").map((request) => request.system))
      .toEqual(["stickybombtrail_red", "muzzle_pipelauncher"])

    const stuck = sticky("stuck-unarmed")
    const stuckResult = mapper.map(frame(2n, [stuck], [event(stuck, "stick", 2n)]))
    expect(stuckResult.particles).toHaveLength(1)
    const settled = stuckResult.models[0]!.orientation
    rotate(settled, [0, 0, 1]).forEach((component, index) => {
      expect(component).toBeCloseTo([0, 1, 0][index]!, 6)
    })

    const armed = sticky("stuck-armed")
    const armedResult = mapper.map(frame(3n, [armed], [event(armed, "arm", 3n)]))
    expect(armedResult.particles).toMatchObject([{ kind: "start", system: "stickybomb_pulse_red" }])

    const explosion = event(armed, "explode", 4n)
    const exploded = mapper.map(frame(4n, [], [explosion]))
    expect(exploded.particles.map((request) => request.kind === "start" ? request.system : request.kind))
      .toEqual(["stop", "stop", "ExplosionCore_Wall"])
    expect(exploded.models).toEqual([])
  })

  test("selects BLU sticky resources and mid-air explosion without a contact patch", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const fact = sticky("flying", "blue")
    expect(mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)])).particles[0])
      .toMatchObject({ kind: "start", system: "stickybombtrail_blue", team: "blue" })
    const explosion = Object.freeze({ ...event(fact, "explode", 2n), contactNormal: null })
    const result = mapper.map(frame(2n, [], [explosion]))
    expect(result.particles.at(-1)).toMatchObject({ kind: "start", system: "ExplosionCore_MidAir" })
  })

  test("cancels a fizzled projectile without emitting an explosion substitute", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const fact = sticky("flying")
    mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)]))
    const fizzled = mapper.map(frame(2n, [], [event(fact, "fizzle", 2n)]))
    expect(fizzled.particles).toEqual([{
      kind: "stop",
      identity: "2:0:fizzle:9:stop:projectile:9:trail",
      effectIdentity: "projectile:9:trail",
      eventIdentity: "2:0:fizzle:9",
      projectileIdentity: 9,
      immediate: true,
    }])
  })

  test("applies same-tick stick and arm events in source order against the final fact", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const flying = sticky("flying")
    mapper.map(frame(1n, [flying], [event(flying, "fire", 1n)]))
    const armed = sticky("stuck-armed")
    const result = mapper.map(frame(2n, [armed], [
      event(armed, "stick", 2n),
      event(armed, "arm", 2n),
    ]))
    expect(result.particles.map((request) => request.kind)).toEqual(["set-control-point", "start"])
    expect(result.particles[1]).toMatchObject({ system: "stickybomb_pulse_red" })
  })

  test("fails missing resources and illegal transitions without retaining partial state", () => {
    const mutableSystems = new Set(systems)
    mutableSystems.delete("rocketbackblast")
    const mapper = createProjectilePresentationMapper(catalog(mutableSystems))
    const fact = rocket()
    const first = frame(1n, [fact], [event(fact, "fire", 1n)])
    expect(() => mapper.map(first)).toThrow(ProjectilePresentationError)
    mutableSystems.add("rocketbackblast")
    expect(mapper.map(first).models).toHaveLength(1)
    expect(() => mapper.map(frame(2n, [fact], [event(fact, "arm", 2n)])))
      .toThrow(ProjectilePresentationError)
    expect(mapper.map(frame(2n, [fact], [])).models).toHaveLength(1)

    const missingAttachment: ProjectileResourceCatalog = Object.freeze({
      ...catalog(),
      attachments: new Map([[7, new Set(["trail"])]]),
    })
    const attachmentMapper = createProjectilePresentationMapper(missingAttachment)
    expect(() => attachmentMapper.map(first)).toThrow(ProjectilePresentationError)
  })

  test("rejects non-normalized facts, disappearance inference, and request bounds", () => {
    const mapper = createProjectilePresentationMapper(catalog(), {
      maxProjectiles: 2,
      maxEvents: 2,
      maxRequests: 1,
    })
    const fact = rocket()
    expect(() => mapper.map(frame(1n, [rocket({ orientation: [0, 0, 0, 2] })], [])))
      .toThrow(ProjectilePresentationError)
    expect(() => mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)])))
      .toThrow(ProjectilePresentationError)

    const normalMapper = createProjectilePresentationMapper(catalog())
    normalMapper.map(frame(1n, [fact], [event(fact, "fire", 1n)]))
    expect(() => normalMapper.map(frame(2n, [], []))).toThrow(ProjectilePresentationError)
  })
})

function rotate(quaternion: Quaternion, value: Vector3): Vector3 {
  const [x, y, z, w] = quaternion
  const uv: Vector3 = [y * value[2] - z * value[1], z * value[0] - x * value[2], x * value[1] - y * value[0]]
  const uuv: Vector3 = [y * uv[2] - z * uv[1], z * uv[0] - x * uv[2], x * uv[1] - y * uv[0]]
  return [
    value[0] + 2 * (uv[0] * w + uuv[0]),
    value[1] + 2 * (uv[1] * w + uuv[1]),
    value[2] + 2 * (uv[2] * w + uuv[2]),
  ]
}
