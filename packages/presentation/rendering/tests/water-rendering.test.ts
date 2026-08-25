import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { sourceShaderGammaToLinear } from "../src/color-output"
import { evaluateCheapWater, evaluateExpensiveWater, waterBlurOffsets } from "../src/source-environment"
import {
  createSourceWaterMaterial,
  evaluateSourceWaterPixel,
  sourceWaterFogAlpha,
  sourceWaterProjectiveCoordinates,
  type SourceWaterShaderState,
} from "../src/source-water"
import { sourceWaterTangentAttributes } from "../src/source-water-geometry"

function shaderState(overrides: Partial<SourceWaterShaderState> = {}): SourceWaterShaderState {
  return Object.freeze({
    profile: "ldr",
    mode: "expensive",
    aboveWater: true,
    reflectAmount: 0.25,
    refractAmount: 0.32,
    reflectTint: [1, 1, 1],
    refractTint: [0.25, 0.5, 0.75],
    fogColor: [0, 0, 0],
    fogStart: 0,
    fogEnd: 100,
    blurRefraction: false,
    hasBaseTexture: false,
    cheapBlend: false,
    cheapStart: 500,
    cheapEnd: 1000,
    reflectionBlendFactor: 1,
    fresnelEnabled: true,
    linearLightScale: 1,
    environmentScale: 1,
    ...overrides,
  })
}

function geometry(authoredTangents = true): THREE.BufferGeometry {
  const result = new THREE.BufferGeometry()
  result.setAttribute("position", new THREE.Float32BufferAttribute([
    -1, -1, 0,
    1, -1, 0,
    0, 1, 0,
  ], 3))
  result.setAttribute("normal", new THREE.Float32BufferAttribute([
    0, 0, 1,
    0, 0, 1,
    0, 0, 1,
  ], 3))
  result.setAttribute("uv", new THREE.Float32BufferAttribute([0, 0, 1, 0, 0.5, 1], 2))
  if (authoredTangents) {
    result.setAttribute("sourceTangentS", new THREE.Float32BufferAttribute([
      1, 0, 0,
      1, 0, 0,
      1, 0, 0,
    ], 3))
    result.setAttribute("sourceTangentT", new THREE.Float32BufferAttribute([
      0, 1, 0,
      0, 1, 0,
      0, 1, 0,
    ], 3))
  }
  return result
}

function texture(rgba: readonly [number, number, number, number]): THREE.DataTexture {
  const result = new THREE.DataTexture(new Uint8Array(rgba), 1, 1, THREE.RGBAFormat)
  result.colorSpace = THREE.NoColorSpace
  result.generateMipmaps = false
  result.flipY = false
  result.needsUpdate = true
  return result
}

describe("Source Water pixel contracts", () => {
  test("non-blurred refraction preserves sampled RGB instead of applying the blur-only tint", () => {
    const pixel = evaluateExpensiveWater({
      reflection: null,
      refraction: [0.8, 0.6, 0.4],
      normal: [0, 0, 1],
      tangentEyeVector: [0, 0, 1],
      reflectionTint: [1, 1, 1],
      refractionTint: [0.25, 0.5, 0.75],
      reflectionOverbright: 1,
      linearLightScale: 1,
      aboveWater: true,
      waterFogDepth: 0.05,
      projectedDepth: 0,
      fogColor: [0, 0, 0],
      fogStart: 0,
      fogEnd: 1,
      hasBaseTexture: false,
      blurRefraction: false,
    })

    expect(pixel).toEqual([0.8, 0.6, 0.4])
  })

  test("Source shader constants use byte-quantized nearest-even gamma rather than sRGB", () => {
    expect(sourceShaderGammaToLinear(-0.5)).toBe(0)
    expect(sourceShaderGammaToLinear(0)).toBe(0)
    expect(sourceShaderGammaToLinear(0.5)).toBe(Math.fround(Math.pow(Math.fround(128 / 255), Math.fround(2.2))))
    expect(sourceShaderGammaToLinear(126.5 / 255)).toBe(Math.fround(Math.pow(Math.fround(126 / 255), Math.fround(2.2))))
    expect(sourceShaderGammaToLinear(0.95)).toBe(1)
    expect(sourceShaderGammaToLinear(Math.fround(0.95))).toBe(1)
    expect(sourceShaderGammaToLinear(0.97)).toBe(1)
    expect(sourceShaderGammaToLinear(1.5)).toBe(1.5)
    expect(() => sourceShaderGammaToLinear(Number.NaN)).toThrow(/invalid/)
  })

  test("cheap HDR fog uses Source gamma 2.2 rather than sRGB transfer", () => {
    const fog = [0.5, 0.25, 0.75] as const
    const result = evaluateCheapWater({
      cubemap: [1, 1, 1],
      environmentScale: 1,
      fogColor: fog,
      reflectTint: [1, 1, 1],
      reflectionBlendFactor: 0,
      worldNormal: [0, 0, 1],
      worldEyeVector: [0, 0, 1],
      distance: 750,
      cheapStart: 500,
      cheapEnd: 1000,
      blend: false,
      refractionDepth: null,
      fresnel: true,
      hdr: true,
    })
    for (let channel = 0; channel < 3; channel += 1) {
      expect(result[channel]).toBeCloseTo(fog[channel]! ** 2.2, 12)
    }
    expect(result[3]).toBe(1)
  })

  test("reflection and refraction preserve their opposite projected Y coordinates", () => {
    expect(sourceWaterProjectiveCoordinates([0.5, -0.25, 7, 2])).toEqual({
      reflection: [0.625, 0.4375],
      refraction: [0.625, 0.5625],
    })
    expect(() => sourceWaterProjectiveCoordinates([0, 0, 0, 0])).toThrow(/projected/)
  })

  test("height fog writes exact Source depth alpha instead of using device depth", () => {
    expect(sourceWaterFogAlpha({
      waterHeight: 10,
      eyeHeight: 20,
      fragmentHeight: 0,
      projectedDepth: 8,
      inverseFogRange: 0.125,
    })).toBe(0.5)
    expect(sourceWaterFogAlpha({
      waterHeight: 10,
      eyeHeight: 20,
      fragmentHeight: 12,
      projectedDepth: 8,
      inverseFogRange: 0.125,
    })).toBe(0)
    expect(() => sourceWaterFogAlpha({
      waterHeight: 10,
      eyeHeight: 12,
      fragmentHeight: 12,
      projectedDepth: 8,
      inverseFogRange: 0.125,
    })).toThrow(/coincide/)
  })

  test("above-water distortion, depth alpha, Fresnel, fog, and opaque output match fixed samples", () => {
    const result = evaluateSourceWaterPixel({
      state: shaderState(),
      clipPosition: [0, 0, 0, 1],
      normalSample: [0.75, 0.25, 1, 1],
      tangentEyeVector: [0, 0, 1],
      reflection: { sample: () => [0.2, 0.4, 0.6, 1] },
      refraction: { sample: () => [0.8, 0.6, 0.4, 0.5] },
    })

    expect(result.reflectionUv).toEqual([0.5625, 0.4375])
    expect(result.refractionUv[0]).toBeCloseTo(0.58, 12)
    expect(result.refractionUv[1]).toBeCloseTo(0.42, 12)
    expect(result.fresnel).toBe(0)
    expect(result.waterFogDepth).toBe(0.5)
    expect(result.refractionSamples).toBe(2)
    expect(result.rgba[0]).toBeCloseTo(0.44, 12)
    expect(result.rgba[1]).toBeCloseTo(0.33, 12)
    expect(result.rgba[2]).toBeCloseTo(0.22, 12)
    expect(result.rgba[3]).toBe(1)
  })

  test("underwater blur uses exactly 25 X-major taps and applies gamma-qualified refraction tint", () => {
    const state = shaderState({
      aboveWater: false,
      blurRefraction: true,
      refractAmount: 0,
      fogStart: 0,
      fogEnd: 100,
    })
    const samples: (readonly [number, number])[] = []
    const result = evaluateSourceWaterPixel({
      state,
      clipPosition: [0, 0, 0, 1],
      normalSample: [0.5, 0.5, 1, 1],
      tangentEyeVector: [0, 0, 1],
      reflection: null,
      refraction: {
        sample: (coordinate) => {
          samples.push(coordinate)
          return [coordinate[0], coordinate[1], 0.5, 0.4]
        },
      },
    })

    expect(samples).toHaveLength(25)
    expect(samples[0]).toEqual([0.49, 0.49])
    expect(samples[1]).toEqual([0.49, 0.495])
    expect(samples[24]).toEqual([0.51, 0.51])
    expect(waterBlurOffsets()[1]).toEqual([-0.01, -0.005])
    expect(result.refractionSamples).toBe(25)
    expect(result.waterFogDepth).toBe(1)
    expect(result.rgba[0]).toBeCloseTo(0.5 * sourceShaderGammaToLinear(0.25), 12)
    expect(result.rgba[1]).toBeCloseTo(0.5 * sourceShaderGammaToLinear(0.5), 12)
    expect(result.rgba[2]).toBeCloseTo(0.5 * sourceShaderGammaToLinear(0.75), 12)
    expect(result.rgba[3]).toBe(1)
  })

  test("integer-HDR reflection applies its exact four-times shader constant", () => {
    const state = shaderState({ profile: "hdr", aboveWater: false })
    const result = evaluateSourceWaterPixel({
      state,
      clipPosition: [0, 0, 0, 1],
      normalSample: [0.5, 0.5, 1, 1],
      tangentEyeVector: [1, 0, 0],
      reflection: { sample: () => [0.1, 0.2, 0.3, 1] },
      refraction: null,
    })
    expect(result.rgba[0]).toBeCloseTo(0.4, 12)
    expect(result.rgba[1]).toBeCloseTo(0.8, 12)
    expect(result.rgba[2]).toBeCloseTo(1.2, 12)
    expect(result.rgba[3]).toBe(1)
  })

  test("recovers exact configured texture-axis tangents from immutable Source surface geometry", () => {
    const input = {
      positions: new Float32Array([0, 0, -2160, 64, 0, -2160, 0, 64, -2160]),
      normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
      uv: new Float32Array([0, 0, 0.25, 0, 0, -0.25]),
      indices: new Uint32Array([0, 1, 2]),
      faces: new Uint32Array([2033]),
      surfacePlanes: new Map([[2033, [0, 0, 1, -2160] as const]]),
    }
    const above = sourceWaterTangentAttributes(input)
    expect([...above.tangentS]).toEqual([1, 0, -0, 1, 0, -0, 1, 0, -0])
    expect([...above.tangentT]).toEqual([0, -1, 0, 0, -1, 0, 0, -1, 0])

    const below = sourceWaterTangentAttributes({
      ...input,
      normals: new Float32Array([0, 0, -1, 0, 0, -1, 0, 0, -1]),
      surfacePlanes: new Map([[2033, [0, 0, -1, 2160] as const]]),
    })
    expect([...below.tangentS].map((value) => Object.is(value, -0) ? 0 : value)).toEqual([1, 0, 0, 1, 0, 0, 1, 0, 0])
    expect([...below.tangentT].map((value) => Object.is(value, -0) ? 0 : value)).toEqual([0, -1, 0, 0, -1, 0, 0, -1, 0])

    expect(() => sourceWaterTangentAttributes({ ...input, surfacePlanes: new Map() })).toThrow(/oriented plane/)
    expect(() => sourceWaterTangentAttributes({ ...input, uv: new Float32Array(6) })).toThrow(/texture basis/)
  })

  test("retains authored water-face tangents across invisible degenerate triangles", () => {
    const result = sourceWaterTangentAttributes({
      positions: new Float32Array([0, 0, -128, 64, 0, -128, 0, 64, -128, 32, 0, -128]),
      normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
      uv: new Float32Array([0, 0, 0.25, 0, 0, -0.25, 0.125, 0]),
      indices: new Uint32Array([0, 1, 2, 0, 3, 1]),
      faces: new Uint32Array([9270, 9270]),
      surfacePlanes: new Map([[9270, [0, 0, 1, -128] as const]]),
    })
    expect([...result.tangentS.slice(9)].map((value) => Object.is(value, -0) ? 0 : value)).toEqual([1, 0, 0])
    expect([...result.tangentT.slice(9)]).toEqual([0, -1, 0])
  })

  test("GPU Water rejects missing authored tangents and missing depth-alpha provenance", () => {
    const normal = texture([128, 128, 255, 255])
    const refraction = texture([64, 128, 192, 128])
    expect(() => createSourceWaterMaterial({
      geometry: geometry(false),
      state: shaderState(),
      normal,
      reflection: null,
      refraction,
      cubemap: null,
      refractionDepthEncoding: "source-water-fog-alpha",
    })).toThrow(/tangent-S/)
    expect(() => createSourceWaterMaterial({
      geometry: geometry(),
      state: shaderState(),
      normal,
      reflection: null,
      refraction,
      cubemap: null,
      refractionDepthEncoding: null,
    })).toThrow(/depth-alpha/)
  })

  test("GPU expensive Water retains opaque depth writes while cheap blending disables them", () => {
    const normal = texture([128, 128, 255, 255])
    const refraction = texture([64, 128, 192, 128])
    const expensive = createSourceWaterMaterial({
      geometry: geometry(),
      state: shaderState(),
      normal,
      reflection: texture([255, 0, 0, 255]),
      refraction,
      cubemap: null,
      refractionDepthEncoding: "source-water-fog-alpha",
    })
    expect(expensive.material.transparent).toBe(false)
    expect(expensive.material.depthTest).toBe(true)
    expect(expensive.material.depthWrite).toBe(true)
    expect(expensive.normalNode.value).toBe(normal)
    expect(expensive.material.colorNode).not.toBeNull()

    const cubemap = new THREE.CubeTexture()
    const cheap = createSourceWaterMaterial({
      geometry: geometry(),
      state: shaderState({ mode: "cheap", cheapBlend: true }),
      normal,
      reflection: null,
      refraction,
      cubemap,
      refractionDepthEncoding: "source-water-fog-alpha",
    })
    expect(cheap.material.transparent).toBe(true)
    expect(cheap.material.depthTest).toBe(true)
    expect(cheap.material.depthWrite).toBe(false)
    expect(cheap.material.blendSrc).toBe(THREE.SrcAlphaFactor)
    expect(cheap.material.blendDst).toBe(THREE.OneMinusSrcAlphaFactor)
  })
})
