import { describe, expect, test } from "bun:test"
import {
  createParticleBatchEncoder,
  ProjectilePresentationError,
  type ProjectileParticleRequest,
} from "../../src/presentation"

type StartRequest = Extract<ProjectileParticleRequest, { kind: "start" }>
type StopRequest = Extract<ProjectileParticleRequest, { kind: "stop" }>

function start(tick: bigint, overrides: Partial<StartRequest> = {}): StartRequest {
  return Object.freeze({
    kind: "start",
    identity: `${tick}:fire:9:start:projectile:9:trail`,
    effectIdentity: "projectile:9:trail",
    eventIdentity: `${tick}:fire:9`,
    tick,
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
    ...overrides,
  })
}

function stop(tick: bigint, immediate: boolean): StopRequest {
  return Object.freeze({
    kind: "stop",
    identity: `${tick}:explode:9:stop:projectile:9:trail:${Number(immediate)}`,
    effectIdentity: "projectile:9:trail",
    eventIdentity: `${tick}:explode:9`,
    tick,
    projectileIdentity: 9,
    immediate,
  })
}

describe("generation-owned projectile particle transactions", () => {
  test("uses one version-4 stop opcode with explicit graceful and immediate modes", () => {
    const graceful = createParticleBatchEncoder().encode(1n, [0, 0, 0], [stop(1n, false)])
    const immediate = createParticleBatchEncoder().encode(1n, [0, 0, 0], [stop(1n, true)])

    expect(new DataView(graceful.buffer).getUint32(4, true)).toBe(4)
    expect([...graceful.subarray(32, 36)]).toEqual([3, 0, 0, 0])
    expect([...immediate.subarray(32, 36)]).toEqual([3, 1, 0, 0])
  })

  test("keeps stale captured-generation work isolated from its replacement timeline", () => {
    const retired = createParticleBatchEncoder()
    retired.encode(80n, [1, 2, 3], [])
    const replacement = createParticleBatchEncoder()

    retired.encode(81n, [1, 2, 3], [])
    const bytes = replacement.encode(1n, [4, 5, 6], [])
    const view = new DataView(bytes.buffer)

    expect(view.getFloat32(8, true)).toBe(0)
    expect(view.getFloat32(12, true)).toBe(Math.fround(0.015))
    expect(() => replacement.encode(0n, [4, 5, 6], [])).toThrow(ProjectilePresentationError)
  })

  test("rejects a non-finite camera before changing the source timeline", () => {
    const encoder = createParticleBatchEncoder()
    encoder.encode(2n, [0, 0, 0], [])

    expect(() => encoder.encode(4n, [Number.NaN, 0, 0], [])).toThrow(ProjectilePresentationError)
    expect(() => encoder.encode(4n, [Number.MAX_VALUE, 0, 0], [])).toThrow(ProjectilePresentationError)

    const bytes = encoder.encode(3n, [1, 2, 3], [])
    expect(new DataView(bytes.buffer).getFloat32(8, true)).toBe(Math.fround(0.03))
  })

  test("rejects malformed control transforms without advancing the encoder", () => {
    const encoder = createParticleBatchEncoder()
    encoder.encode(2n, [0, 0, 0], [])
    const malformed = start(4n, {
      controlPoints: Object.freeze([
        Object.freeze({
          index: 0,
          position: Object.freeze([Number.NaN, 2, 3]),
          orientation: Object.freeze([0, 0, 0, 1]),
          ownerIdentity: 1,
        }),
      ]),
    })

    expect(() => encoder.encode(4n, [0, 0, 0], [malformed])).toThrow(ProjectilePresentationError)
    expect(new DataView(encoder.encode(3n, [0, 0, 0], []).buffer).getFloat32(8, true))
      .toBe(Math.fround(0.03))
  })

  test("rejects overflowing binary32 controls, invalid quaternions, and control ownership", () => {
    const mutate = (position: readonly [number, number, number], orientation: readonly [number, number, number, number], ownerIdentity: number) => start(1n, {
      controlPoints: Object.freeze([Object.freeze({ index: 0, position, orientation, ownerIdentity })]),
    })

    for (const malformed of [
      mutate([Number.MAX_VALUE, 0, 0], [0, 0, 0, 1], 1),
      mutate([1, 2, 3], [0, 0, 0, 2], 1),
      mutate([1, 2, 3], [0, 0, 0, 1], 0),
    ]) {
      expect(() => createParticleBatchEncoder().encode(1n, [0, 0, 0], [malformed]))
        .toThrow(ProjectilePresentationError)
    }
  })

  test("rejects invalid request kinds, duplicate identities, and overlong PCF names", () => {
    const invalidKind = { ...stop(1n, false), kind: "destroy" } as unknown as ProjectileParticleRequest
    expect(() => createParticleBatchEncoder().encode(1n, [0, 0, 0], [invalidKind]))
      .toThrow(ProjectilePresentationError)
    expect(() => createParticleBatchEncoder().encode(1n, [0, 0, 0], [stop(1n, false), stop(1n, false)]))
      .toThrow(ProjectilePresentationError)
    expect(() => createParticleBatchEncoder().encode(1n, [0, 0, 0], [start(1n, { system: "x".repeat(1_025) })]))
      .toThrow(ProjectilePresentationError)
  })

  test("preserves ordered fire and same-tick impact timestamps without admitting reversal", () => {
    const requests = [start(2n), stop(4n, false)]
    const bytes = createParticleBatchEncoder().encode(4n, [0, 0, 0], requests)
    const view = new DataView(bytes.buffer)
    const second = 32 + 20 + 16 + new TextEncoder().encode("rockettrail").byteLength + 32

    expect(view.getFloat32(44, true)).toBe(Math.fround(0.03))
    expect(view.getFloat32(second + 12, true)).toBe(Math.fround(0.06))
    expect(() => createParticleBatchEncoder().encode(4n, [0, 0, 0], [stop(4n, false), start(2n)]))
      .toThrow(ProjectilePresentationError)
    expect(() => createParticleBatchEncoder().encode(9_007_199_254_740_993n, [0, 0, 0], []))
      .toThrow(ProjectilePresentationError)
  })
})
