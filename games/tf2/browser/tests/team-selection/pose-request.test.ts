import { expect, test } from "bun:test"
import { encodeModelPoseBatch, type ModelPoseRequest } from "../../src/presentation"

const request: ModelPoseRequest = {
  identity: 4098, model: "models/vgui/ui_team01_red.mdl", activity: "hoveropen",
  entityModelPanel: true, modelPanelReset: true,
  previousElapsedSeconds: 0, elapsedSeconds: 0, currentTimeSeconds: 1, frameTimeSeconds: 0.015,
  planarSpeed: 0, screenAspectRatio: 16 / 9, worldFarPlane: 1000, skin: 0, lod: 0, bodygroups: [0],
}
test("entity panel animation requests have a distinct owner without changing other model panels", () => {
  const bytes = encodeModelPoseBatch([request])
  expect(bytes[52]).toBe(8)
  expect(bytes[55]).toBe(1)
  expect(encodeModelPoseBatch([{ ...request, entityModelPanel: false, modelPanel: true }])[52]).toBe(4)
  expect(encodeModelPoseBatch([{ ...request, entityModelPanel: false, modelPanelReset: false }])[52]).toBe(0)
  expect(() => encodeModelPoseBatch([{ ...request, modelPanel: true }])).toThrow()
  expect(() => encodeModelPoseBatch([{ ...request, actorIdentity: 1 }])).toThrow()
})
