import { describe, expect, test } from "bun:test"
import { currentPresentedCamera, equivalentPresentedVisibility, presentCamera, selectPresentedCamera, type PresentedCameraRevisions } from "../src/presented-camera"

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

  test("selects the identical immutable diagnostic camera before model and world visibility", () => {
    const selected = selectPresentedCamera(camera, { position: [523, -439, 318], yawDegrees: 90, pitchDegrees: -8 })
    expect(selected).toMatchObject({ position: [523, -439, 318], yawDegrees: 90, pitchDegrees: -8 })
    expect(Object.isFrozen(selected.position)).toBe(true)
    expect(selectPresentedCamera(camera, { position: [1, 2, Number.NaN], yawDegrees: 90, pitchDegrees: -8 })).toBe(camera)
    expect(selectPresentedCamera(camera, { position: [1, 2, 3], yawDegrees: Number.NaN, pitchDegrees: -8 })).toBe(camera)
  })

  test("reuses visibility only for exact camera, authored sky, generation, viewport, presentation and tick identities", () => {
    let state = 0x5eed1234
    const next = () => ((state = Math.imul(state ^ state >>> 15, 2246822519) + 3266489917 | 0) >>> 0) / 0x1_0000_0000
    for (let index = 0; index < 256; index += 1) {
      const position = Object.freeze([next() * 4000 - 2000, next() * 4000 - 2000, next() * 1000]) as readonly [number, number, number]
      const randomized = { ...camera, position, yawDegrees: next() * 360, pitchDegrees: next() * 160 - 80 }
      const controller = index % 3 === 0 ? null : { origin: [next() * 100, next() * 100, next() * 100] as const, scale: index % 2 ? 16 : 0, area: index % 8 }
      const original = presentCamera(randomized, revisions(), controller)
      expect(equivalentPresentedVisibility(original, presentCamera(randomized, { ...revisions(), viewRevision: index, mouseRevision: index }, controller))).toBe(true)
      for (const key of ["generation", "viewportRevision", "preparedRevision"] as const) {
        expect(equivalentPresentedVisibility(original, presentCamera(randomized, { ...revisions(), [key]: revisions()[key] + 1 }, controller))).toBe(false)
      }
      expect(equivalentPresentedVisibility(original, presentCamera(randomized, { ...revisions(), tick: 42n }, controller))).toBe(false)
      for (const key of ["yawDegrees", "pitchDegrees", "verticalFovDegrees", "near", "far"] as const) {
        expect(equivalentPresentedVisibility(original, presentCamera({ ...randomized, [key]: randomized[key] + 1 }, revisions(), controller))).toBe(false)
      }
      expect(equivalentPresentedVisibility(original, presentCamera({ ...randomized, position: [position[0] + 1, position[1], position[2]] }, revisions(), controller))).toBe(false)
      if (controller) {
        expect(equivalentPresentedVisibility(original, presentCamera(randomized, revisions(), { ...controller, area: controller.area + 1 }))).toBe(false)
        expect(equivalentPresentedVisibility(original, presentCamera(randomized, revisions(), { ...controller, scale: controller.scale + 1 }))).toBe(false)
        expect(equivalentPresentedVisibility(original, presentCamera(randomized, revisions(), null))).toBe(false)
      }
    }
    const positive = presentCamera({ ...camera, position: [0, 1, 2] }, revisions(), null)
    const negative = presentCamera({ ...camera, position: [-0, 1, 2] }, revisions(), null)
    expect(equivalentPresentedVisibility(positive, negative)).toBe(false)
  })
})
