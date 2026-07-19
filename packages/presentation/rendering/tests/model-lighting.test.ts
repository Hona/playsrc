import { describe, expect, test } from "bun:test"
import {
  ambientCubeLuminance,
  dilateIrisUv,
  evaluateAmbientCube,
  evaluateLocalLight,
  evaluateModelLighting,
  intersectEyeSphere,
  projectEyeCoordinate,
  prepareModelDrawInputs,
  validateModelEyeStates,
  validateModelLightingInput,
  type ModelLightingInput,
  type ModelLocalLight,
} from "../src/model-lighting"

const cube = [
  [1, 0, 0], [2, 0, 0], [0, 3, 0], [0, 4, 0], [0, 0, 5], [0, 0, 6],
] as const

const point: ModelLocalLight = Object.freeze({
  kind: "point",
  color: [3, 6, 9],
  position: [0, 0, 10],
  direction: [0, 0, -1],
  range: 0,
  falloff: 0,
  attenuation: [1, 0.1, 0.01],
  theta: 0,
  phi: 0,
})

const input = (lights: readonly ModelLocalLight[] = []): ModelLightingInput => Object.freeze({
  lightingOrigin: [0, 0, 0],
  ambientCube: cube,
  localLights: lights,
  cameraPosition: [0, 0, 20],
  localEnvironment: "materials/maps/test/c0_0_0.vtf",
  ambientLight: true,
  staticLightVertex: false,
  staticLightTexel: false,
})

describe("explicit Source model lighting", () => {
  test("selects the ordered ambient-cube side with squared normal components", () => {
    expect(evaluateAmbientCube([1, 0, 0], cube)).toEqual([1, 0, 0])
    expect(evaluateAmbientCube([-1, 0, 0], cube)).toEqual([2, 0, 0])
    expect(evaluateAmbientCube([0, -1, 0], cube)).toEqual([0, 4, 0])
    const diagonal = evaluateAmbientCube([1, 1, 1], cube)
    expect(diagonal[0]).toBeCloseTo(1 / 3, 12)
    expect(diagonal[1]).toBeCloseTo(1, 12)
    expect(diagonal[2]).toBeCloseTo(5 / 3, 12)
  })

  test("evaluates point, directional, spot, and half-Lambert local lights", () => {
    expect(evaluateLocalLight(point, [0, 0, 0], [0, 0, 1], false)).toEqual([1, 2, 3])
    expect(evaluateLocalLight({ ...point, kind: "directional", color: [2, 4, 6] }, [100, 50, 25], [0, 0, 1], false)).toEqual([2, 4, 6])
    expect(evaluateLocalLight({ ...point, kind: "spot", attenuation: [1, 0, 0], theta: 0.3, phi: 0.6, falloff: 5 }, [0, 0, 0], [0, 0, 1], false)).toEqual([3, 6, 9])
    expect(evaluateLocalLight({ ...point, kind: "directional", direction: [1, 0, 0], color: [4, 4, 4] }, [0, 0, 0], [0, 0, 1], true)).toEqual([1, 1, 1])
    expect(evaluateModelLighting(input([point]), [0, 0, 0], [0, 0, 1], false)).toEqual([1, 2, 8])
  })

  test("validates the complete bounded input without selecting missing state", () => {
    expect(validateModelLightingInput(input([point, point, point, point]))).toBeTruthy()
    expect(() => validateModelLightingInput(input([point, point, point, point, point]))).toThrow(/invalid/i)
    expect(() => validateModelLightingInput({ ...input(), localEnvironment: "Materials/Test.vtf" })).toThrow(/invalid/i)
    expect(() => evaluateLocalLight(point, [0, 0, 10], [0, 0, 1], false)).toThrow(/coincides/i)
  })
})

describe("explicit Studio eye state", () => {
  test("projects iris rows, intersects the eyeball, and applies pupil dilation", () => {
    expect(projectEyeCoordinate([2, 3, 4, 5], [7, 11, 13])).toBe(104)
    expect(intersectEyeSphere([0, 0, 5], [0, 0, -1], [0, 0, 0], 1)).toBe(4)
    expect(intersectEyeSphere([0, 0, 5], [1, 0, 0], [0, 0, 0], 1)).toBe(0)
    expect(dilateIrisUv([0.6, 0.5], 0.5)[0]).toBeCloseTo(0.6, 12)
    expect(dilateIrisUv([0.7, 0.5], 1)[0]).toBeCloseTo(0.7, 12)
  })

  test("keeps one eye state per primitive and derives bounded ambient luminance", () => {
    const eye = Object.freeze({
      primitive: 2, mesh: 4, eyeball: 0, texture: 1,
      worldOrigin: [1, 2, 3] as const, authoredUp: [0, 0, 1] as const,
      irisU: [1, 0, 0, 0] as const, irisV: [0, 1, 0, 0] as const,
      glintU: [1, 0, 0, 0] as const, glintV: [0, 1, 0, 0] as const,
    })
    expect(validateModelEyeStates([eye])).toEqual([eye])
    expect(() => validateModelEyeStates([eye, eye])).toThrow(/invalid/i)
    expect(prepareModelDrawInputs({
      primitive: 2,
      required: ["ambient-cube", "local-lights", "camera-position", "studio-eye-parameters", "local-environment", "authored-texture-planes", "game-proxy-values"],
      lighting: input(), eyes: [eye], currentFramebuffer: false, authoredTexturePlanes: true, gameProxyValues: true,
    })).toMatchObject({ primitive: 2, eye })
    expect(() => prepareModelDrawInputs({
      primitive: 2, required: ["current-framebuffer"], currentFramebuffer: false,
      authoredTexturePlanes: true, gameProxyValues: true,
    })).toThrow(/current-framebuffer/i)
    expect(ambientCubeLuminance(cube)).toBeGreaterThanOrEqual(0)
    expect(ambientCubeLuminance(cube)).toBeLessThanOrEqual(1)
  })
})
