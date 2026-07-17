import { expect, test } from "bun:test"
import { applyPointerDelta } from "../src/input"

test("positive horizontal and vertical pointer deltas turn right and down", () => {
  const ordinary = applyPointerDelta(180, -1, 64, 32)
  expect(ordinary.yaw).toBeCloseTo(174.88)
  expect(ordinary.pitch).toBeCloseTo(1.56)
  expect(applyPointerDelta(0, 88, -64, 32)).toEqual({ yaw: 5.12, pitch: 89 })
})
