import { expect, test } from "bun:test"
import { sourceModelPanelPresentation, withSourceModelPanelTargetViewport } from "../src/model-panel"

class Rectangle {
  constructor(public x: number, public y: number, public z: number, public w: number) {}
  set(x: number, y: number, width: number, height: number) { Object.assign(this, { x, y, z: width, w: height }) }
}

for (const dpr of [1, 1.25, 1.5, 2]) test(`HUD frame-target viewport uses physical pixels at DPR ${dpr} and restores ownership`, () => {
  const target = { viewport: new Rectangle(0, 0, 1280 * dpr, 720 * dpr), scissor: new Rectangle(1, 2, 300, 400) }
  const before = structuredClone(target)
  const presentation = sourceModelPanelPresentation({ model: "models/player/soldier.mdl", kind: "studio", fov: 25, origin: [110, 0, -56], bounds: { x: 12, y: 600, width: 100, height: 100 }, displayWidth: 1280 * dpr, displayHeight: 720 * dpr, devicePixelRatio: dpr })
  expect(() => withSourceModelPanelTargetViewport(target, presentation, () => {
    expect(target.viewport).toMatchObject({ x: 12 * dpr, y: 600 * dpr, z: 100 * dpr, w: 100 * dpr })
    expect(target.scissor).toEqual(target.viewport)
    throw new Error("draw failure")
  })).toThrow("draw failure")
  expect(structuredClone(target)).toEqual(before)
  expect(withSourceModelPanelTargetViewport(null, presentation, () => 7)).toBe(7)
})
