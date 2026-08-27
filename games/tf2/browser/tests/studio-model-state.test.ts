import { expect, test } from "bun:test"
import { studioModelFrameState } from "../src/presentation"
import type { EntityPresentation } from "../src/codec"

test("map Skin inputs render without requiring an animation and preserve live skin", () => {
  const presentation = { studioModels: [{ sourceIndex: 97, worldPosition: [1, 2, 3], worldAngles: [0, 90, 0], skin: 2 }], studioAnimations: [] } as unknown as EntityPresentation
  expect(studioModelFrameState(presentation, 97)).toEqual({ position: [1, 2, 3], angles: [0, 90, 0], skin: 2, renderBounds: undefined })
  expect(() => studioModelFrameState(presentation, 98)).toThrow("Studio model state unavailable")
})

test("animated map props use sequence render bounds and current skin", () => {
  const bounds = [[-10, -20, -30], [10, 20, 30]] as const
  const presentation = { studioModels: [{ sourceIndex: 7, worldPosition: [4, 5, 6], worldAngles: [0, 180, 0], skin: 1 }], studioAnimations: [{ sourceIndex: 7, bounds }] } as unknown as EntityPresentation
  const state = studioModelFrameState(presentation, 7)
  expect(state.renderBounds).toBe(bounds)
  expect(state.skin).toBe(1)
})
