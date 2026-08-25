import { expect, test } from "bun:test"
import {
  createProjectilePresentationMapper,
  ProjectilePresentationError,
  type AttachmentTransform,
  type ProjectileEvent,
  type ProjectileFact,
  type Quaternion,
  type Vector3,
} from "../../src/presentation"

const orientation = Object.freeze([0, 0, 0, 1]) as Quaternion

function rocket(tick: bigint): Readonly<{ fact: ProjectileFact; event: ProjectileEvent }> {
  const position = Object.freeze([1, 2, 3]) as Vector3
  const fact: ProjectileFact = Object.freeze({
    identity: 9,
    kind: "rocket",
    team: "red",
    ownerIdentity: 4,
    launcherIdentity: 2,
    state: "flying",
    position,
    velocity: Object.freeze([100, 0, 0]),
    orientation,
    angularVelocity: Object.freeze([0, 0, 0]),
    contactNormal: null,
    ageSeconds: 0,
  })
  const event: ProjectileEvent = Object.freeze({
    kind: "fire",
    projectileKind: "rocket",
    projectileIdentity: fact.identity,
    ownerIdentity: fact.ownerIdentity,
    launcherIdentity: fact.launcherIdentity,
    team: fact.team,
    tick,
    sourceOrdinal: 0,
    sourceEventOrdinal: 0,
    position,
    orientation,
    contactNormal: null,
  })
  return Object.freeze({ fact, event })
}

test("rejects malformed authored attachment transforms before publishing projectile state", () => {
  const transforms = new Map<number, ReadonlyMap<string, AttachmentTransform>>([
    [9, new Map([["trail", Object.freeze({ position: Object.freeze([2, 3, 4]), orientation })]])],
    [2, new Map([["backblast", Object.freeze({ position: Object.freeze([Number.NaN, 5, 6]), orientation })]])],
  ])
  const mapper = createProjectilePresentationMapper(Object.freeze({
    models: new Set(["models/weapons/w_models/w_rocket.mdl"]),
    systems: new Set(["rockettrail", "rocketbackblast"]),
    attachments: new Map([[9, new Set(["trail"])], [2, new Set(["backblast"])]]),
    attachmentTransforms: transforms,
  }))
  const { fact, event } = rocket(1n)
  const frame = Object.freeze({
    ticks: Object.freeze([
      Object.freeze({ tick: 1n, projectiles: Object.freeze([fact]), events: Object.freeze([event]) }),
    ]),
  })

  expect(() => mapper.map(frame)).toThrow(ProjectilePresentationError)
  transforms.set(2, new Map([
    ["backblast", Object.freeze({ position: Object.freeze([4, 5, 6]), orientation })],
  ]))
  expect(mapper.map(frame).particles.map(request => request.kind === "start" ? request.system : request.kind))
    .toEqual(["rockettrail", "rocketbackblast"])
})
