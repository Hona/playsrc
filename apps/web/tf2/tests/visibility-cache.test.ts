import { describe, expect, test } from "bun:test"
import { ExactSkyVisibilityCache, sameSkyVisibilityIdentity, type SkyVisibilityIdentity } from "../src/visibility-cache"

const identity = (): SkyVisibilityIdentity => ({
  generation: 2,
  viewportRevision: 7,
  tick: 41n,
  position: [10, 20, 30],
  origin: [100, 200, 300],
  area: 4,
  yawDegrees: 90,
  pitchDegrees: -12,
  verticalFovDegrees: 60,
  near: 2,
  far: 56755.840576,
})

describe("exact application-owned 3D-sky visibility reuse", () => {
  test("reuses only one complete generation, viewport, tick, area, and camera identity", () => {
    const cache = new ExactSkyVisibilityCache<object>()
    const expected = Object.freeze({ surfaces: Object.freeze([2, 5, 8]) })
    cache.write(identity(), expected)
    expect(cache.read(identity())).toBe(expected)
    expect(sameSkyVisibilityIdentity(identity(), identity())).toBe(true)

    const changes: Partial<SkyVisibilityIdentity>[] = [
      { generation: 3 },
      { viewportRevision: 8 },
      { tick: 42n },
      { position: [10, 20, 31] },
      { origin: [100, 201, 300] },
      { area: 5 },
      { yawDegrees: 91 },
      { pitchDegrees: -11 },
      { verticalFovDegrees: 61 },
      { near: 3 },
      { far: 56756 },
    ]
    for (const change of changes) expect(cache.read({ ...identity(), ...change })).toBeUndefined()
    cache.clear()
    expect(cache.read(identity())).toBeUndefined()
  })

  test("distinguishes signed-zero camera inputs without retaining replacement generations", () => {
    const cache = new ExactSkyVisibilityCache<string>()
    const first = { ...identity(), yawDegrees: -0 }
    cache.write(first, "first")
    expect(cache.read({ ...first, yawDegrees: 0 })).toBeUndefined()
    const replacement = { ...identity(), generation: 3 }
    cache.write(replacement, "replacement")
    expect(cache.read(first)).toBeUndefined()
    expect(cache.read(replacement)).toBe("replacement")
  })
})
