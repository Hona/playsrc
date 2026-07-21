import { expect, test } from "bun:test"
import { executeViewModelDepthPhase } from "../src/viewmodel-depth-phase"

test("clears world depth before drawing and restores the full depth range", () => {
  const operations: string[] = []
  const result = executeViewModelDepthPhase({
    depthRange: [0, 0.1],
    clearWorldDepth: () => operations.push("clear-depth"),
    setDepthRange: (range) => operations.push(`depth:${range.join(",")}`),
    draw: () => operations.push("draw-viewmodel"),
  })
  expect(operations).toEqual(["clear-depth", "depth:0,0.1", "draw-viewmodel", "depth:0,1"])
  expect(result).toEqual({ depthRange: [0, 0.1], worldDepthCleared: true, depthRangeRestored: true })
})

test("restores the full range when viewmodel drawing fails", () => {
  const operations: string[] = []
  expect(() => executeViewModelDepthPhase({
    depthRange: [0, 0.1],
    clearWorldDepth: () => operations.push("clear-depth"),
    setDepthRange: (range) => operations.push(`depth:${range.join(",")}`),
    draw: () => { operations.push("draw-viewmodel"); throw new Error("draw failed") },
  })).toThrow("draw failed")
  expect(operations).toEqual(["clear-depth", "depth:0,0.1", "draw-viewmodel", "depth:0,1"])
})
