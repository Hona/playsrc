import { expect, test } from "bun:test"
import { sourceHorizontal4By3FovToVertical, sourceViewportDepthRange } from "../src/source-camera"

test("converts Source horizontal-4:3 fields of view to browser vertical projection", () => {
  expect(sourceHorizontal4By3FovToVertical(75)).toBeCloseTo(59.84044400898543, 12)
  expect(sourceHorizontal4By3FovToVertical(54)).toBeCloseTo(41.82812169855287, 12)
  expect(() => sourceHorizontal4By3FovToVertical(180)).toThrow()
})

test("accepts only a bounded increasing WebGPU viewport depth range", () => {
  expect(sourceViewportDepthRange([0, 0.1])).toEqual([0, 0.1])
  expect(() => sourceViewportDepthRange([0.1, 0.1])).toThrow()
  expect(() => sourceViewportDepthRange([-0.1, 1])).toThrow()
})
