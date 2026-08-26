import { describe, expect, test } from "bun:test"
import { sourceModelPanelPresentation } from "../src/model-panel"

const request = Object.freeze({
  model: "models/player/soldier.mdl",
  horizontalFov4By3: 25,
  origin: Object.freeze([320, 10, -49]) as readonly [number, number, number],
  bounds: Object.freeze({ x: 0, y: 0, width: 960, height: 720 }),
  displayWidth: 1280,
  displayHeight: 720,
  devicePixelRatio: 1,
})

describe("authored Source VGUI model-panel presentation", () => {
  test("preserves the exact 4:3 player origin and Source horizontal-field-of-view conversion", () => {
    const presentation = sourceModelPanelPresentation(request)
    expect(presentation.viewport).toEqual({ x: 0, y: 0, width: 960, height: 720 })
    expect(presentation.origin).toEqual([320, 10, -49])
    expect(presentation.verticalFovDegrees).toBeCloseTo(18.878551147)
  })

  test("applies the authored wide and narrow player-only horizontal-origin corrections", () => {
    expect(sourceModelPanelPresentation({ ...request, bounds: { x: 0, y: 0, width: 1280, height: 720 } }).origin).toEqual([260, 10, -49])
    expect(sourceModelPanelPresentation({ ...request, bounds: { x: 0, y: 0, width: 720, height: 720 } }).origin).toEqual([335, 10, -49])
    expect(sourceModelPanelPresentation({ ...request, model: "models/vgui/ui_class01.mdl", bounds: { x: 0, y: 0, width: 720, height: 720 } }).origin).toEqual([320, 10, -49])
  })

  test("converts top-left VGUI bounds to bounded device-pixel WebGPU viewports", () => {
    expect(sourceModelPanelPresentation({
      ...request,
      bounds: { x: 100, y: 40, width: 300, height: 200 },
      displayWidth: 2560,
      displayHeight: 1440,
      devicePixelRatio: 2,
    }).viewport).toEqual({ x: 200, y: 80, width: 600, height: 400 })
    expect(sourceModelPanelPresentation({
      ...request,
      bounds: { x: 0, y: 399, width: 150, height: 300 },
    }).viewport).toEqual({ x: 0, y: 399, width: 150, height: 300 })
    expect(() => sourceModelPanelPresentation({ ...request, bounds: { x: 1280, y: 0, width: 1, height: 1 } })).toThrow("outside the display")
    expect(() => sourceModelPanelPresentation({ ...request, horizontalFov4By3: Number.NaN })).toThrow("invalid")
  })
})
