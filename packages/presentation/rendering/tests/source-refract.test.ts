import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { sourceShaderGammaToLinear } from "../src/color-output"
import { createSourceRefractMaterial, evaluateSourceRefractPixel } from "../src/source-refract"

const state = Object.freeze({
  refractAmount: 0.05,
  refractTint: [185 / 255, 215 / 255, 245 / 255] as const,
  blurAmount: 1 as const,
  ignoreDepth: true,
})

describe("authored underwater Refract overlay", () => {
  test("preserves normal-alpha displacement, four Source polyphase taps, and gamma-qualified tint", () => {
    const taps: Array<readonly [number, number]> = []
    const pixel = evaluateSourceRefractPixel({
      state,
      coordinate: [0.5, 0.25],
      normal: [0.75, 0.25, 1, 0.8],
      sample: (coordinate) => {
        taps.push(coordinate)
        return [0.6, 0.4, 0.2, 1]
      },
    })
    expect(pixel.warpedCoordinate[0]).toBeCloseTo(0.52, 10)
    expect(pixel.warpedCoordinate[1]).toBeCloseTo(0.23, 10)
    expect(pixel.framebufferSamples).toBe(4)
    expect(taps[0]![0]).toBeCloseTo(0.52 - 0.5 / 512, 12)
    expect(taps[0]![1]).toBeCloseTo(0.23 - 0.5 / 512, 12)
    expect(taps[3]![0]).toBeCloseTo(0.52 + 1 / 512, 12)
    expect(taps[3]![1]).toBeCloseTo(0.23 + 1 / 512, 12)
    for (const index of [0, 1, 2]) {
      expect(pixel.rgba[index]).toBeCloseTo(
        [0.6, 0.4, 0.2][index]! * 0.9999999 * sourceShaderGammaToLinear(state.refractTint[index]!),
        8,
      )
    }
    expect(pixel.rgba[3]).toBe(0.8)
  })

  test("rejects invalid shader inputs and preserves authored ignore-depth blending", () => {
    expect(() => evaluateSourceRefractPixel({
      state: { ...state, refractAmount: Number.NaN },
      coordinate: [0, 0],
      normal: [0, 0, 1, 1],
      sample: () => [0, 0, 0, 1],
    })).toThrow(/invalid/i)
    const texture = new THREE.DataTexture(new Uint8Array([128, 128, 255, 255]), 1, 1)
    const output = createSourceRefractMaterial({ state, normal: texture })
    expect(output.material.transparent).toBe(true)
    expect(output.material.depthTest).toBe(false)
    expect(output.material.depthWrite).toBe(false)
    output.material.dispose()
    texture.dispose()
  })
})
