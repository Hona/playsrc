import { describe, expect, test } from "bun:test"
import {
  ExposureController,
  SOURCE_LDR,
  SOURCE_PC_INTEGER_HDR,
  applyColorOutput,
  buildLuminanceHistogram,
  exposureTarget,
  validateRenderConfiguration,
} from "../src/color-output"

describe("immutable LDR and Source PC integer-HDR output", () => {
  test("fixes categorical profile, tone, color, format, and alpha combinations", () => {
    expect(Object.isFrozen(validateRenderConfiguration(SOURCE_LDR))).toBe(true)
    expect(validateRenderConfiguration(SOURCE_PC_INTEGER_HDR)).toMatchObject({
      lightingProfile: "hdr",
      toneOperator: "source-pc-integer",
      outputColorSpace: "srgb",
      canvasFormat: "bgra8unorm",
      alphaMode: "opaque",
    })
    expect(() => validateRenderConfiguration({ ...SOURCE_PC_INTEGER_HDR, toneOperator: "identity" })).toThrow()
    expect(() => validateRenderConfiguration({
      ...SOURCE_LDR,
      exposure: { ...SOURCE_LDR.exposure, minimum: 0.5 },
    })).toThrow(/exactly one/i)
  })

  test("applies scalar exposure, saturation, and the sRGB transfer exactly once", () => {
    const output = applyColorOutput([2, 0.5, 0.0031308, 0.25], SOURCE_PC_INTEGER_HDR, 1)
    expect(output[0]).toBe(0.9999999999999999)
    expect(output[1]).toBeCloseTo(0.7353569831, 10)
    expect(output[2]).toBeCloseTo(0.040449936, 8)
    expect(output[3]).toBe(1)
    expect(applyColorOutput([0.5, 0.5, 0.5, 0.5], {
      ...SOURCE_LDR,
      alphaMode: "premultiplied",
    }, 2)).toEqual([
      0.7353569830524495 * 0.5,
      0.7353569830524495 * 0.5,
      0.7353569830524495 * 0.5,
      0.5,
    ])
  })

  test("matches the fixed 16-bin target and bounded exposure timeline", () => {
    const histogram = buildLuminanceHistogram(Array(256).fill(0.6))
    expect(histogram.reduce((sum, count) => sum + count, 0)).toBe(256)
    expect(histogram.filter(Boolean)).toHaveLength(1)
    expect(exposureTarget(histogram, 1)).toBe(1)

    const controller = new ExposureController()
    const bright = buildLuminanceHistogram(Array(256).fill(1))
    for (let index = 0; index < 10; index += 1) controller.submit(bright)
    expect(controller.advance(0.015).current).toBeCloseTo(0.9937673325, 9)
    const bounded = controller.advance(10)
    expect(bounded.fixedSteps).toBeLessThanOrEqual(33)
    expect(bounded.droppedSeconds).toBeGreaterThan(9)
  })
})
