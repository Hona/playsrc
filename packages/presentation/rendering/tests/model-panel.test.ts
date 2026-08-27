import { describe, expect, test } from "bun:test"
import { sourceModelPanelPresentation } from "../src/model-panel"

const request = Object.freeze({
  model: "models/player/soldier.mdl",
  kind: "entity" as const,
  fov: 25,
  origin: Object.freeze([320, 10, -49]) as readonly [number, number, number],
  bounds: Object.freeze({ x: 0, y: 0, width: 960, height: 720 }),
  displayWidth: 1280,
  displayHeight: 720,
  devicePixelRatio: 1,
})

describe("authored Source VGUI model-panel presentation", () => {
  test("keeps studio-panel horizontal FOV and forced origin independent of entity-panel framing", () => {
    const result = sourceModelPanelPresentation({ ...request, kind: "studio", bounds: { x: 0, y: 0, width: 720, height: 720 } })
    expect(result.origin).toEqual([320, 10, -49])
    expect(result.verticalFovDegrees).toBeCloseTo(25)
    expect(result.near).toBe(3)
    expect(result.far).toBeCloseTo(16384 * Math.sqrt(3))
  })
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
    expect(() => sourceModelPanelPresentation({ ...request, fov: Number.NaN })).toThrow("invalid")
  })

  test.each([1, 1.1, 1.25, 1.3, 1.5, 1.75, 2])("keeps right/bottom anchored panels inside the physical framebuffer at DPR %s", (devicePixelRatio) => {
    const cssWidth = 1999.5
    const cssHeight = 1099.5
    const displayWidth = Math.floor(cssWidth * devicePixelRatio)
    const displayHeight = Math.floor(cssHeight * devicePixelRatio)
    const presentation = sourceModelPanelPresentation({
      ...request,
      bounds: { x: cssWidth - 320, y: cssHeight - 180, width: 320, height: 180 },
      displayWidth,
      displayHeight,
      devicePixelRatio,
    })
    expect(presentation.viewport.x + presentation.viewport.width).toBe(displayWidth)
    expect(presentation.viewport.y + presentation.viewport.height).toBe(displayHeight)
    expect(Math.floor(presentation.rendererViewport.x * devicePixelRatio)).toBe(presentation.viewport.x)
    expect(Math.floor(presentation.rendererViewport.y * devicePixelRatio)).toBe(presentation.viewport.y)
    expect(Math.floor(presentation.rendererViewport.width * devicePixelRatio)).toBe(presentation.viewport.width)
    expect(Math.floor(presentation.rendererViewport.height * devicePixelRatio)).toBe(presentation.viewport.height)
  })

  test("clips all physical framebuffer edges without translating or enlarging authored panels", () => {
    expect(sourceModelPanelPresentation({
      ...request,
      bounds: { x: -20, y: -10, width: 100, height: 80 },
      devicePixelRatio: 1.5,
      displayWidth: 1920,
      displayHeight: 1080,
    }).viewport).toEqual({ x: 0, y: 0, width: 120, height: 105 })
    expect(() => sourceModelPanelPresentation({
      ...request,
      bounds: { x: -100, y: 0, width: 100, height: 50 },
    })).toThrow("outside the display")
  })
})
