import { describe, expect, test } from "bun:test"
import {
  createProjectilePresentationMapper,
  projectileFrame,
  ProjectilePresentationError,
  sourceViewOrientation,
  type ProjectileEvent,
  type ProjectileFact,
  type ProjectileFrame,
  type ProjectileResourceCatalog,
  type ProjectileTick,
  type Quaternion,
  type Vector3,
} from "../src/presentation"
import type { Snapshot } from "../src/codec"

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
    attachmentTransforms: new Map([
      [7, new Map([["trail", { position: [2, 3, 4] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
    ]),
    fireAttachmentTransforms: new Map([
      [7, new Map([["backblast", { position: [5, 6, 7] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
      [9, new Map([["muzzle", { position: [8, 9, 10] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
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
    projectileKind: fact.kind,
    projectileIdentity: fact.identity,
    ownerIdentity: fact.ownerIdentity,
    launcherIdentity: fact.launcherIdentity,
    team: fact.team,
    tick,
    sourceOrdinal: 0,
    sourceEventOrdinal: 0,
    position: fact.position,
    orientation: fact.orientation,
    contactNormal: fact.contactNormal,
  })
}

function frame(tick: bigint, projectiles: readonly ProjectileFact[], events: readonly ProjectileEvent[]): ProjectileFrame {
  return timeline(projectileTick(tick, projectiles, events))
}

function projectileTick(
  tick: bigint,
  projectiles: readonly ProjectileFact[],
  events: readonly ProjectileEvent[],
): ProjectileTick {
  const expanded: ProjectileEvent[] = []
  const append = (value: ProjectileEvent, kind: ProjectileEvent["kind"], sourceEventOrdinal: number) => {
    expanded.push(Object.freeze({ ...value, kind, sourceOrdinal: expanded.length, sourceEventOrdinal }))
  }
  events.forEach((value, sourceEventOrdinal) => {
    append(value, value.kind, sourceEventOrdinal)
    if (value.kind === "impact") {
      const outcome = events.slice(sourceEventOrdinal + 1)
        .find((candidate) => candidate.projectileIdentity === value.projectileIdentity)
      if (outcome?.kind !== "stick" && outcome?.kind !== "explode") append(value, "bounce", sourceEventOrdinal)
    } else if (value.kind === "fizzle" || value.kind === "explode") {
      append(value, "destroy", sourceEventOrdinal)
    }
  })
  return Object.freeze({
    tick,
    projectiles: Object.freeze(projectiles),
    events: Object.freeze(expanded),
  })
}

function timeline(...ticks: readonly ProjectileTick[]): ProjectileFrame {
  return Object.freeze({ ticks: Object.freeze(ticks) })
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

  test("preserves sticky spin and VPhysics orientation through stick, arm, and explosion", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const flying = sticky("flying")
    const fired = mapper.map(frame(1n, [flying], [event(flying, "fire", 1n)]))
    expect(fired.models[0]).toMatchObject({ skin: 0, angularVelocity: [100, -200, 300] })
    expect(fired.particles.filter((request) => request.kind === "start").map((request) => request.system))
      .toEqual(["stickybombtrail_red", "muzzle_pipelauncher"])

    const stuck = sticky("stuck-unarmed")
    const stuckResult = mapper.map(frame(2n, [stuck], [event(stuck, "impact", 2n), event(stuck, "stick", 2n)]))
    expect(stuckResult.particles).toHaveLength(1)
    const settled = stuckResult.models[0]!.orientation
    expect(settled).toEqual(stuck.orientation)

    const armed = sticky("stuck-armed")
    const armedResult = mapper.map(frame(3n, [armed], [event(armed, "arm", 3n)]))
    expect(armedResult.particles[0]).toMatchObject({ kind: "start", system: "stickybomb_pulse_red" })

    const explosion = event(armed, "explode", 4n)
    const exploded = mapper.map(frame(4n, [], [explosion]))
    expect(exploded.particles.map((request) => request.kind === "start" ? request.system : request.kind))
      .toEqual(["stop", "ExplosionCore_Wall"])
    expect(exploded.particles[0]).toMatchObject({ kind: "stop", immediate: false, tick: 4n })
    expect(exploded.models).toEqual([])
  })

  test("records a non-sticking sticky impact as an ordered bounce outcome", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const flying = sticky("flying")
    mapper.map(frame(1n, [flying], [event(flying, "fire", 1n)]))
    const rawFact = Object.freeze({
      identity: flying.identity,
      kind: 2 as const,
      team: 2 as const,
      ownerIdentity: flying.ownerIdentity,
      launcherIdentity: flying.launcherIdentity,
      state: 1 as const,
      position: flying.position,
      velocity: flying.velocity,
      orientation: flying.orientation,
      angularVelocity: flying.angularVelocity,
      contactNormal: null,
      ageSeconds: flying.ageSeconds,
    })
    const impact = Object.freeze({
      type: "impact" as const,
      projectile: flying.identity,
      kind: 2 as const,
      ownerIdentity: flying.ownerIdentity,
      launcherIdentity: flying.launcherIdentity,
      team: 2 as const,
      tick: 2n,
      position: flying.position,
      orientation: flying.orientation,
      contactNormal: Object.freeze([0, 0, 1]) as Vector3,
    })
    const snapshot = {
      projectileTimeline: Object.freeze([
        Object.freeze({ tick: 2n, projectiles: Object.freeze([rawFact]), events: Object.freeze([impact]) }),
      ]),
    } as unknown as Snapshot
    expect(mapper.map(projectileFrame(snapshot)).events.map((value) => value.kind))
      .toEqual(["impact", "bounce"])
  })

  test("selects BLU sticky resources and mid-air explosion without a contact patch", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const fact = sticky("flying", "blue")
    expect(mapper.map(frame(1n, [fact], [event(fact, "fire", 1n)])).particles[0])
      .toMatchObject({ kind: "start", system: "stickybombtrail_blue", team: "blue" })
    const explosion = Object.freeze({ ...event(fact, "explode", 2n), contactNormal: null })
    const result = mapper.map(frame(2n, [], [explosion]))
    expect(result.particles.at(-1)).toMatchObject({
      kind: "start",
      system: "ExplosionCore_MidAir",
      controlPoints: [{ orientation: [0, 0, 0, 1] }],
    })
  })

  test("maps a transient near-wall rocket from every selected tick without a final fact", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const fired = rocket({ orientation: [0, 0, 0, 1] })
    const normal = Object.freeze([-1, 0, 0]) as Vector3
    const rawFact = Object.freeze({
      identity: fired.identity,
      kind: 1 as const,
      team: 2 as const,
      ownerIdentity: fired.ownerIdentity,
      launcherIdentity: fired.launcherIdentity,
      state: 1 as const,
      position: fired.position,
      velocity: fired.velocity,
      orientation: fired.orientation,
      angularVelocity: fired.angularVelocity,
      contactNormal: null,
      ageSeconds: fired.ageSeconds,
    })
    const rawEvent = (type: "fire" | "impact" | "explode", tick: bigint, contactNormal: Vector3 | null) => Object.freeze({
      type,
      projectile: fired.identity,
      kind: 1 as const,
      ownerIdentity: fired.ownerIdentity,
      launcherIdentity: fired.launcherIdentity,
      team: 2 as const,
      tick,
      position: type === "fire" ? fired.position : Object.freeze([12, 2, 3]) as Vector3,
      orientation: fired.orientation,
      contactNormal,
    })
    const snapshot = {
      projectileTimeline: Object.freeze([
        Object.freeze({ tick: 1n, projectiles: Object.freeze([rawFact]), events: Object.freeze([rawEvent("fire", 1n, null)]) }),
        Object.freeze({ tick: 2n, projectiles: Object.freeze([]), events: Object.freeze([
          rawEvent("impact", 2n, normal),
          rawEvent("explode", 2n, normal),
        ]) }),
      ]),
    } as unknown as Snapshot
    const result = mapper.map(projectileFrame(snapshot))
    expect(result.models).toEqual([])
    expect(result.events.map((value) => `${value.tick}:${value.sourceOrdinal}:${value.kind}`)).toEqual([
      "1:0:fire",
      "2:0:impact",
      "2:1:explode",
      "2:2:destroy",
    ])
    expect(result.particles.map((request) => request.kind === "start" ? request.system : request.kind)).toEqual([
      "rockettrail",
      "rocketbackblast",
      "stop",
      "ExplosionCore_Wall",
    ])
    expect(result.particles.map((request) => request.tick)).toEqual([1n, 1n, 2n, 2n])
    const explosion = result.particles.at(-1)
    expect(explosion).toMatchObject({ kind: "start", system: "ExplosionCore_Wall" })
    if (explosion?.kind !== "start") throw new Error("wall explosion request is missing")
    explosion.controlPoints[0]!.orientation.forEach((component, index) => {
      expect(component).toBeCloseTo(sourceViewOrientation(0, 180)[index]!, 12)
    })
  })

  test("binds coalesced launcher effects to each authoritative fire-tick pose", () => {
    const first = rocket({ position: [10, 20, 30], orientation: [0, 0, 0, 1] })
    const second = rocket({ identity: 8, position: [40, 50, 60], orientation: [0, 0, 0, 1] })
    const resources: ProjectileResourceCatalog = Object.freeze({
      ...catalog(),
      attachments: new Map([
        [7, new Set(["trail"])],
        [8, new Set(["trail"])],
        [20, new Set(["backblast"])],
      ]),
      attachmentTransforms: new Map([
        [7, new Map([["trail", { position: [11, 20, 30] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
        [8, new Map([["trail", { position: [41, 50, 60] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
      ]),
      fireAttachmentTransforms: new Map([
        [7, new Map([["backblast", { position: [1, 2, 3] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
        [8, new Map([["backblast", { position: [101, 102, 103] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
      ]),
    })
    const result = createProjectilePresentationMapper(resources).map(timeline(
      projectileTick(4n, [first], [event(first, "fire", 4n)]),
      projectileTick(5n, [first, second], [event(second, "fire", 5n)]),
    ))
    expect(result.particles.filter((request) => request.kind === "start" && request.system === "rocketbackblast")
      .map((request) => request.kind === "start" ? [request.tick, request.controlPoints[0]!.position] : null))
      .toEqual([[4n, [1, 2, 3]], [5n, [101, 102, 103]]])
  })

  test("retains every far-flight control with one attachment-local transform", () => {
    const initial = rocket({ position: [10, 20, 30], orientation: [0, 0, 0, 1] })
    const middle = rocket({ position: [20, 30, 40], orientation: [0, 0, 0, 1], ageSeconds: 0.5 })
    const final = rocket({ position: [30, 40, 50], orientation: [0, 0, 0, 1], ageSeconds: 1 })
    const resources: ProjectileResourceCatalog = Object.freeze({
      ...catalog(),
      attachmentTransforms: new Map([
        [7, new Map([["trail", { position: [32, 43, 54] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
        [20, new Map([["backblast", { position: [5, 6, 7] as Vector3, orientation: [0, 0, 0, 1] as Quaternion }]])],
      ]),
    })
    const result = createProjectilePresentationMapper(resources).map(timeline(
      projectileTick(1n, [initial], [event(initial, "fire", 1n)]),
      projectileTick(2n, [middle], []),
      projectileTick(3n, [final], []),
    ))
    expect(result.particles.filter((request) => request.effectIdentity === "projectile:7:trail").map((request) =>
      request.kind === "start" ? request.controlPoints[0]!.position : request.kind === "set-control-point" ? request.controlPoint.position : []
    )).toEqual([[12, 23, 34], [22, 33, 44], [32, 43, 54]])
  })

  test("suppresses only local rocket backblast and keeps local sticky muzzle", () => {
    const localCatalog = Object.freeze({ ...catalog(), localOwnerIdentity: 10 })
    const rocketResult = createProjectilePresentationMapper(localCatalog).map(
      frame(1n, [rocket()], [event(rocket(), "fire", 1n)]),
    )
    expect(rocketResult.particles.filter((request) => request.kind === "start").map((request) => request.system))
      .toEqual(["rockettrail"])

    const localSticky = sticky("flying")
    const stickyCatalog = Object.freeze({ ...catalog(), localOwnerIdentity: localSticky.ownerIdentity })
    const stickyResult = createProjectilePresentationMapper(stickyCatalog).map(
      frame(1n, [localSticky], [event(localSticky, "fire", 1n)]),
    )
    expect(stickyResult.particles.filter((request) => request.kind === "start").map((request) => request.system))
      .toEqual(["stickybombtrail_red", "muzzle_pipelauncher"])
  })

  test("preserves VPhysics sticky orientation and hides its first 0.1 seconds", () => {
    const orientation = Object.freeze([0.1825741858, 0.3651483717, 0.5477225575, 0.7302967433]) as Quaternion
    const hidden = sticky("flying")
    const mapper = createProjectilePresentationMapper(catalog())
    expect(mapper.map(frame(1n, [{ ...hidden, orientation, ageSeconds: 0.099 }], [
      event({ ...hidden, orientation }, "fire", 1n),
    ])).models).toEqual([])
    const visible = Object.freeze({ ...hidden, orientation, ageSeconds: 0.1 })
    expect(mapper.map(frame(2n, [visible], [])).models[0]?.orientation).toEqual(orientation)
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
      tick: 2n,
      projectileIdentity: 9,
      immediate: false,
    }])
  })

  test("applies same-tick stick and arm events in source order against the final fact", () => {
    const mapper = createProjectilePresentationMapper(catalog())
    const flying = sticky("flying")
    mapper.map(frame(1n, [flying], [event(flying, "fire", 1n)]))
    const armed = sticky("stuck-armed")
    const result = mapper.map(frame(2n, [armed], [
      event(armed, "impact", 2n),
      event(armed, "stick", 2n),
      event(armed, "arm", 2n),
    ]))
    expect(result.particles.map((request) => request.kind)).toEqual(["start", "set-control-point"])
    expect(result.particles[0]).toMatchObject({ system: "stickybomb_pulse_red" })
  })
  test("accepts an airborne arm without a contact normal",()=>{const mapper=createProjectilePresentationMapper(catalog()),flying=sticky("flying");mapper.map(frame(1n,[flying],[event(flying,"fire",1n)]));const armed=mapper.map(frame(54n,[flying],[event(flying,"arm",54n)]));expect(armed.particles.some(request=>request.kind==="start"&&request.system==="stickybomb_pulse_red")).toBeTrue();expect(armed.models[0]?.state).toBe("flying")})

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
    expect(() => normalMapper.map(frame(1n, [fact], []))).toThrow(ProjectilePresentationError)
    expect(normalMapper.map(frame(2n, [fact], [])).models).toHaveLength(1)
    expect(() => normalMapper.map(frame(3n, [], []))).toThrow(ProjectilePresentationError)
  })
})
