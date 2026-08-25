import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import {
  createSourceLightmappedEnvironmentNode,
  evaluateSourceLightmappedPixel,
  type SourceLightmappedPixelRequest,
} from "../src/source-lightmapped"

function request(overrides: Partial<SourceLightmappedPixelRequest> = {}): SourceLightmappedPixelRequest {
  return Object.freeze({
    base: [0.5, 0.25, 0.75, 0.4],
    irradiance: [0.5, 1, 0.25],
    normalSample: [0.5, 0.5, 1, 1],
    tangentS: [1, 0, 0],
    tangentT: [0, 1, 0],
    surfaceNormal: [0, 0, 1],
    eyeVector: [0, 0, 1],
    cubemap: [0.25, 0.5, 0.75],
    environment: Object.freeze({
      tint: [0.2, 0.2, 0.2],
      contrast: 0,
      saturation: 1,
      fresnelReflection: 1,
      environmentScale: 1,
    }),
    exposure: 1,
    ...overrides,
  })
}

describe("Source LightmappedGeneric authored environment", () => {
  test("adds the exact untinted-lightmap and linear authored cubemap terms", () => {
    const result = evaluateSourceLightmappedPixel(request())
    expect(result[0]).toBeCloseTo(0.3, 12)
    expect(result[1]).toBeCloseTo(0.35, 12)
    expect(result[2]).toBeCloseTo(0.3375, 12)
    expect(result[3]).toBe(0.4)
  })

  test("retains authored Fresnel, contrast, saturation, integer-HDR scaling and alpha", () => {
    const result = evaluateSourceLightmappedPixel(request({
      eyeVector: [1, 0, 0],
      exposure: 0.5,
      environment: Object.freeze({
        tint: [0.2, 0.2, 0.2],
        contrast: 1,
        saturation: 0,
        fresnelReflection: 0,
        environmentScale: 16,
      }),
    }))
    const grey = 0.8 ** 2 * 0.299 + 1.6 ** 2 * 0.587 + 2.4 ** 2 * 0.114
    expect(result[0]).toBeCloseTo((0.25 + grey) * 0.5, 12)
    expect(result[1]).toBeCloseTo((0.25 + grey) * 0.5, 12)
    expect(result[2]).toBeCloseTo((0.1875 + grey) * 0.5, 12)
    expect(result[3]).toBe(0.4)
  })

  test("rejects unavailable authored tangent bases rather than inventing reflection normals", () => {
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0], 3))
    geometry.setAttribute("normal", new THREE.Float32BufferAttribute([0, 0, 1], 3))
    expect(() => createSourceLightmappedEnvironmentNode({
      geometry,
      normal: new THREE.Texture(),
      cubemap: new THREE.CubeTexture(),
      state: request().environment,
    })).toThrow(/authored tangent/)
  })
})
