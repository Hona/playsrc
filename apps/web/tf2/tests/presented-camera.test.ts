import { describe, expect, test } from "bun:test"
import { currentPresentedCamera, presentCamera, type PresentedCameraRevisions } from "../src/presented-camera"

const camera = Object.freeze({
  position: Object.freeze([160, -320, 48]) as readonly [number, number, number],
  yawDegrees: 87,
  pitchDegrees: -13,
  verticalFovDegrees: 74,
  near: 7,
  far: 20_000,
})

const revisions = (): PresentedCameraRevisions => ({
  generation: 2,
  viewportRevision: 3,
  preparedRevision: 11,
  viewRevision: 19,
  mouseRevision: 17,
  snapRevision: 2,
  tick: 41n,
})

describe("completed Source main and authored 3D-sky camera coherence", () => {
  test("derives both immutable views from the same interpolated presented eye", () => {
    const authored = { origin: [1024, 2048, -512] as const, scale: 16, area: 4 }
    const presented = presentCamera(camera, revisions(), authored)
    expect(presented.main.position).toEqual([160, -320, 48])
    expect(presented.sky?.position).toEqual([1034, 2028, -509])
    expect(presented.sky).toMatchObject({ yawDegrees: 87, pitchDegrees: -13, verticalFovDegrees: 74, near: 2 })
    expect(Object.isFrozen(presented)).toBe(true)
    expect(Object.isFrozen(presented.main.position)).toBe(true)
    expect(Object.isFrozen(presented.sky?.position)).toBe(true)
    expect(Object.isFrozen(presented.revisions)).toBe(true)
  })

  test("rejects stale async results after pointer motion, authoritative snaps, map replacement, or viewport replacement", () => {
    const presented = presentCamera(camera, revisions(), { origin: [1, 2, 3], scale: 16, area: 4 })
    const current = revisions()
    expect(currentPresentedCamera(presented, current)).toBe(true)
    for (const key of ["generation", "viewportRevision", "viewRevision", "mouseRevision", "snapRevision"] as const) {
      expect(currentPresentedCamera(presented, { ...current, [key]: current[key] + 1 })).toBe(false)
    }
  })

  test("preserves controller-free skies and Source's unscaled zero-scale controller", () => {
    expect(presentCamera(camera, revisions(), null).sky).toBeNull()
    expect(presentCamera(camera, revisions(), { origin: [1, 2, 3], scale: 0, area: 4 }).sky?.position)
      .toEqual([161, -318, 51])
  })
})
