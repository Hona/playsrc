import { expect, test } from "bun:test"
import { controlPointSwipeUv } from "../../src/hud-integration/control-points"

test("capture swipe consumes client elapsed progress, not server remaining fraction", () => {
  const begin = controlPointSwipeUv(0, true, false)
  const end = controlPointSwipeUv(1, true, false)
  expect(begin[0]).toBeCloseTo(0.9 + 48 / 69)
  expect(begin[2]).toBeCloseTo(0.9)
  expect(end[0]).toBeCloseTo(1.1 - 15 / 69)
  expect(end[2]).toBeCloseTo(0)
  expect(controlPointSwipeUv(0.25, false, false)).toEqual([controlPointSwipeUv(0.25, true, false)[2], 0, controlPointSwipeUv(0.25, true, false)[0], 1])
  const up = controlPointSwipeUv(0, true, true)
  expect(up[1]).toBeCloseTo(-48 / 69 - 0.07)
  expect(up[3]).toBeCloseTo(0.07)
})
