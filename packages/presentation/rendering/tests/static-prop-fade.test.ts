import { describe, expect, test } from "bun:test"
import { distanceFadeOpacity, quantizeStaticPropOpacity, screenFadeOpacity } from "../src/static-prop-fade"

describe("authored Source static-prop fade boundaries", () => {
  test("preserves squared-distance admission at minimum, between thresholds, and maximum", () => {
    expect(distanceFadeOpacity(0, 10, 20)).toBe(1)
    expect(distanceFadeOpacity(100, 10, 20)).toBe(1)
    expect(distanceFadeOpacity(250, 10, 20)).toBe(0.5)
    expect(distanceFadeOpacity(400, 10, 20)).toBe(0)
    expect(distanceFadeOpacity(401, 10, 20)).toBe(0)
  })

  test("preserves authored screen-width fade and disabled upper thresholds", () => {
    const authoredMinimumDistanceField = 20
    const authoredMaximumDistanceField = 10
    expect(screenFadeOpacity(15, authoredMaximumDistanceField, authoredMinimumDistanceField)).toBe(0.5)
    expect(screenFadeOpacity(9, 10, 20)).toBe(0)
    expect(screenFadeOpacity(10, 10, 20)).toBe(0)
    expect(screenFadeOpacity(15, 10, 20)).toBe(0.5)
    expect(screenFadeOpacity(20, 10, 20)).toBe(1)
    expect(screenFadeOpacity(30, 10, -1)).toBe(1)
  })

  test("retains Source 8-bit truncation and endpoint clamps", () => {
    expect(quantizeStaticPropOpacity(-1)).toBe(0)
    expect(quantizeStaticPropOpacity(0.5)).toBe(127 / 255)
    expect(quantizeStaticPropOpacity(1)).toBe(1)
    expect(quantizeStaticPropOpacity(2)).toBe(1)
  })
})
