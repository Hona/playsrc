import { describe, expect, test } from "bun:test"
import {
  projectedDecalDepthBias,
  projectedDecalReceiverIsValid,
  type ProjectedDecalInput,
} from "../src/decal-occlusion"
import { RetainedWorldVisibility } from "../src/retained-visibility"

const NEAR = 7
const FAR = 28_377.919921875
const DEPTH24_MAX = 2 ** 24 - 1

function projectedDepth(distance: number): number {
  return FAR / (FAR - NEAR) - FAR * NEAR / ((FAR - NEAR) * distance)
}

function fragmentPassesOpaqueDepth(distance: number, opaqueDistance: number): boolean {
  const bias = projectedDecalDepthBias("decal")
  return projectedDepth(distance) + bias.units / DEPTH24_MAX <= projectedDepth(opaqueDistance)
}

function mark(overrides: Partial<ProjectedDecalInput> = {}): ProjectedDecalInput {
  return {
    kind: 0,
    receiver: { entity: null, model: 0 },
    targetFaces: [229, 233],
    renderOrder: 0,
    fragments: [
      { model: 0, face: 229, visibility: { kind: "world" } },
      { model: 0, face: 233, visibility: { kind: "world" } },
    ],
    ...overrides,
  }
}

describe("Source projected decal receivers and opaque-wall depth", () => {
  test("converts Source configuration denominators to effective depth24plus state", () => {
    expect(projectedDecalDepthBias("none")).toEqual({ enabled: false, slopeScale: 0, units: 0 })
    expect(projectedDecalDepthBias("decal")).toEqual({ enabled: true, slopeScale: -2, units: -64 })
  })

  test("the configured 14 receiver cannot pass the nearer solid face 288", () => {
    const camera: readonly [number, number, number] = [11_600, 0, -2_392]
    const receiver: readonly [number, number, number] = [12_695.9, 610, -2_392]
    const receiverDistance = Math.hypot(
      receiver[0] - camera[0], receiver[1] - camera[1], receiver[2] - camera[2],
    )
    const occluderDistance = receiverDistance * (12_064 - camera[0]) / (receiver[0] - camera[0])

    expect(receiverDistance).toBeCloseTo(1254.232, 2)
    expect(occluderDistance).toBeCloseTo(531.037, 2)
    expect(projectedDepth(receiverDistance)).toBeGreaterThan(projectedDepth(occluderDistance))
    expect(fragmentPassesOpaqueDepth(receiverDistance, occluderDistance)).toBe(false)
    expect(fragmentPassesOpaqueDepth(receiverDistance - 0.1, receiverDistance)).toBe(true)
  })

  test("requires every projected fragment to retain its exact selected receiver", () => {
    expect(projectedDecalReceiverIsValid(mark())).toBe(true)
    expect(projectedDecalReceiverIsValid(mark({ receiver: null }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ receiver: { entity: 216n, model: 93 } }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ targetFaces: [229] }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ targetFaces: [229, 229] }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ targetFaces: [229, 234] }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ targetFaces: [233, 229] }))).toBe(false)
    expect(projectedDecalReceiverIsValid(mark({ renderOrder: 4 }))).toBe(false)
  })

  test("admits compiled overlays without a collision receiver and preserves their two-bit order", () => {
    expect(projectedDecalReceiverIsValid(mark({ kind: 1, receiver: null, renderOrder: 3 }))).toBe(true)
    expect(projectedDecalReceiverIsValid(mark({ kind: 2, receiver: null, renderOrder: 2 }))).toBe(true)
  })

  test("keeps moving brush marks on their current entity/model receiver", () => {
    const receiver = { entity: 216n, model: 93 }
    const fragment = { model: 93, face: 891, visibility: { kind: "brush-model" as const, entity: 216n, model: 93 } }
    const moving = mark({ receiver, targetFaces: [891], fragments: [fragment] })
    expect(projectedDecalReceiverIsValid(moving)).toBe(true)
    expect(projectedDecalReceiverIsValid({ ...moving, receiver: { entity: 217n, model: 93 } })).toBe(false)
    expect(projectedDecalReceiverIsValid({ ...moving, fragments: [{ ...fragment, model: 94 }] })).toBe(false)
    expect(projectedDecalReceiverIsValid({ ...moving, fragments: [{ ...fragment, visibility: { ...fragment.visibility, model: 94 } }] })).toBe(false)
  })

  test("world receiver visibility changes never admit an excluded face or stale mark", () => {
    const indices = Uint32Array.from([0, 1, 2, 3, 4, 5, 6, 7, 8])
    const world = new RetainedWorldVisibility([{
      faces: Uint32Array.from([229, 233, 288]),
      sourceIndices: indices,
      targetIndices: indices.slice(),
      transparent: false,
    }])

    world.apply(Uint32Array.from([229, 288]))
    expect([world.has(229), world.has(233), world.has(288)]).toEqual([true, false, true])
    world.apply(Uint32Array.from([233, 288]))
    expect([world.has(229), world.has(233), world.has(288)]).toEqual([false, true, true])
  })
})
