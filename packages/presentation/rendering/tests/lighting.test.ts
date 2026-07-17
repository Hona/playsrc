import { describe, expect, test } from "bun:test"
import {
  SOURCE_BUMP_BASIS,
  directionalWeights,
  evaluateDirectionalLightmap,
  prepareWorldLights,
  sampleAmbientCube,
} from "../src/lighting"
import { parseRuntimeMap } from "../src/runtime-map"
import { hdrFixture } from "./hdr-fixture"

describe("HDR directional, world, and ambient lighting inputs", () => {
  test("uses the official normal basis and direct SSBump coefficients", () => {
    const basis = directionalWeights({ kind: "normal", value: SOURCE_BUMP_BASIS[0] })
    expect(basis[0]).toBeCloseTo(1, 12)
    expect(basis[1]).toBeCloseTo(0, 12)
    expect(basis[2]).toBeCloseTo(0, 12)
    expect(directionalWeights({ kind: "normal", value: [0, 0, 1] })).toEqual([
      1 / 3, 1 / 3, 1 / 3,
    ])
    expect(directionalWeights({ kind: "ssbump", value: [0.2, 0.3, 0.5] })).toEqual([0.2, 0.3, 0.5])
    expect(evaluateDirectionalLightmap(
      [9, 9, 9],
      [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
      { kind: "ssbump", value: [0.2, 0.3, 0.5] },
    )).toEqual([4.9, 5.9, 6.9])
    expect(() => evaluateDirectionalLightmap([1, 1, 1], undefined, undefined)).toThrow(/required/i)
  })

  test("consumes caller-selected world lights and explicit style scalars", () => {
    const map = parseRuntimeMap(hdrFixture().bytes)
    if (map.lighting.profile !== "hdr") throw new Error("expected HDR")
    const selected = prepareWorldLights(map.lighting.descriptor, {
      candidates: [0],
      styleScalars: [{ style: 0, scalar: 0.5 }],
    })
    expect(selected).toHaveLength(1)
    expect(selected[0]).toMatchObject({ sourceIndex: 0, intensity: [2, 2.5, 3] })
    expect(selected[0]!.source.radius).toBe(100)
    expect(() => prepareWorldLights(map.lighting.descriptor, { candidates: [0], styleScalars: [] })).toThrow(/missing/i)
  })

  test("combines only caller-selected ambient samples inside the supplied leaf", () => {
    const map = parseRuntimeMap(hdrFixture().bytes)
    if (map.lighting.profile !== "hdr") throw new Error("expected HDR")
    const cube = sampleAmbientCube(map.lighting.descriptor, {
      leaf: 0,
      samples: [{ sample: 0, weight: 1 }],
    })
    expect(cube[0]).toEqual([1, 2, 3])
    expect(cube[5]).toEqual([6, 7, 8])
    expect(() => sampleAmbientCube(map.lighting.descriptor, { leaf: 1, samples: [{ sample: 0, weight: 1 }] })).toThrow(/leaf/i)
    expect(() => sampleAmbientCube(map.lighting.descriptor, { leaf: 0, samples: [{ sample: 0, weight: 0.5 }] })).toThrow(/sum to one/i)
  })
})
