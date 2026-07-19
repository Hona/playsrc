import { describe, expect, test } from "bun:test"
import {
  buildSourceSkyGeometry,
  decodeRgbsBilinear,
  evaluateCheapWater,
  evaluateExpensiveWater,
  evaluateFogColor,
  linearFogFactor,
  sampleAuthoredCubemap,
  selectNearestCubemap,
  sourceCubemapCoordinate,
  sourceSkyFaceVisible,
  waterBlurOffsets,
  type AuthoredCubemap,
} from "../src/source-environment"

describe("Source 2D sky", () => {
  test("builds six camera-relative faces with exact orientation and seam UVs", () => {
    const geometry = buildSourceSkyGeometry([10, 20, 30], 300)
    expect(geometry.map((face) => face.face)).toEqual(["right", "left", "back", "front", "up", "down"])
    expect(geometry[0]!.vertices[0]!.position).toEqual([183.205, 193.205, -143.205])
    expect(geometry[0]!.vertices.map((vertex) => vertex.uv)).toEqual([
      [1 / 512, 511 / 512], [1 / 512, 1 / 512], [511 / 512, 1 / 512], [511 / 512, 511 / 512],
    ])
    const translated = buildSourceSkyGeometry([110, 220, 330], 300)[0]!.vertices[0]!.position
      .map((value, index) => value - geometry[0]!.vertices[0]!.position[index]!)
    expect(translated[0]).toBeCloseTo(100, 12)
    expect(translated[1]).toBeCloseTo(200, 12)
    expect(translated[2]).toBeCloseTo(300, 12)
    expect(sourceSkyFaceVisible("right", [1, 0, 0])).toBe(true)
    expect(sourceSkyFaceVisible("right", [-1, 0, 0])).toBe(false)
  })

  test("decodes RGBS per tap before bilinear interpolation", () => {
    expect(decodeRgbsBilinear([
      [1, 0, 0, 1], [0, 1, 0, 0.5], [0, 0, 1, 0.25], [1, 1, 1, 0],
    ], [0.5, 0.5])).toEqual([2, 1, 0.5])
  })
})

function cubemap(): AuthoredCubemap {
  return Object.freeze({
    mipCount: 1,
    scalarEncoding: "u8" as const,
    planes: Object.freeze(Array.from({ length: 6 }, (_, face) => Object.freeze({
      mip: 0, face, width: 1, height: 1, rgba: new Uint8Array([face * 20, 0, 255 - face * 20, 255]),
    }))),
  })
}

describe("Source cubemaps", () => {
  test("retains strict source-order ties and Source-space squared distance", () => {
    expect(selectNearestCubemap([
      { index: 4, origin: [-10, 0, 0] }, { index: 9, origin: [10, 0, 0] },
    ], [0, 0, 0])).toBe(4)
    expect(selectNearestCubemap([
      { index: 4, origin: [-10, 0, 0] }, { index: 9, origin: [10, 0, 0] },
    ], [9, 0, 0])).toBe(9)
  })

  test("maps all six axes and samples authored faces without generated mips", () => {
    const directions = [[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[0,0,1],[0,0,-1]] as const
    expect(directions.map((direction) => sourceCubemapCoordinate(direction).face)).toEqual([0, 1, 2, 3, 4, 5])
    for (let face = 0; face < 6; face += 1) {
      expect(sampleAuthoredCubemap(cubemap(), directions[face]!, 0, "nearest")).toEqual([face * 20 / 255, 0, (255 - face * 20) / 255, 1])
    }
    expect(() => sampleAuthoredCubemap({ ...cubemap(), planes: cubemap().planes.slice(0, 5) }, [1, 0, 0], 0, "linear")).toThrow(/incomplete/i)
  })
})

describe("Source Water and fog probes", () => {
  test("evaluates cheap distance/Fresnel/shore alpha and opaque fog blending", () => {
    const request = Object.freeze({
      cubemap: [1, 0.5, 0.25] as const, environmentScale: 1, fogColor: [0, 0, 0] as const, reflectTint: [0.5, 1, 1] as const,
      reflectionBlendFactor: 0, worldNormal: [0, 0, 1] as const, worldEyeVector: [0, 0, 1] as const,
      distance: 750, cheapStart: 500, cheapEnd: 1000, blend: true, refractionDepth: null, fresnel: true, hdr: false,
    })
    expect(evaluateCheapWater(request)).toEqual([0.5, 0.5, 0.25, 0.5])
    expect(evaluateCheapWater({ ...request, refractionDepth: 0.05 })[3]).toBe(0)
    expect(evaluateCheapWater({ ...request, blend: false })).toEqual([0, 0, 0, 1])
  })

  test("evaluates expensive above/below fog, Fresnel branches, and exact blur taps", () => {
    const base = Object.freeze({
      reflection: [1, 0, 0] as const, refraction: [0, 0, 1] as const,
      normal: [0, 0, 1] as const, tangentEyeVector: [0, 0, 1] as const,
      reflectionTint: [1, 1, 1] as const, refractionTint: [1, 1, 1] as const,
      reflectionOverbright: 1, linearLightScale: 1, aboveWater: true, waterFogDepth: 0.05,
      projectedDepth: 0, fogColor: [0, 1, 0] as const, fogStart: 0, fogEnd: 100, hasBaseTexture: false,
    })
    expect(evaluateExpensiveWater(base)).toEqual([0, 0, 1])
    expect(evaluateExpensiveWater({ ...base, tangentEyeVector: [1, 0, 0], waterFogDepth: 1 })).toEqual([1, 0, 0])
    const offsets = waterBlurOffsets()
    expect(offsets).toHaveLength(25)
    expect(offsets[0]).toEqual([-0.01, -0.01])
    expect(offsets[12]).toEqual([0, 0])
    expect(offsets[24]).toEqual([0.01, 0.01])
  })

  test("blends directional fog color and clamps linear density", () => {
    const state = Object.freeze({ enabled: true, blend: true, radial: false, direction: [1,0,0] as const,
      primary: [255,0,0,255] as const, secondary: [0,0,255,255] as const, start: 10, end: 110, maximumDensity: 0.6 })
    expect(evaluateFogColor(state, [1,0,0])).toEqual([1,0,0])
    expect(evaluateFogColor(state, [-1,0,0])).toEqual([0,0,1])
    expect(linearFogFactor(60, 10, 110, 0.6)).toBe(0.5)
    expect(linearFogFactor(110, 10, 110, 0.6)).toBe(0.6)
  })
})
