import { expect, test } from "bun:test"
import { summarizeObserveStages } from "../profile/observe-stages"

test("observe stages preserve real order, allocations and missing phases", () => {
  const stages = [9, 10, 11, 12, 13, 8, 9, 10]
  const events = stages.map((stage, index) => ({ stage, actor: 0, tick: index < 6 ? 0 : 1, at: index,
    heapBytes: 100 - index, allocations: index * 2, allocatedBytes: index * 10, value: 0 }))
  const result = summarizeObserveStages(events)
  expect(result.transactions).toBe(2)
  expect(result.incomplete).toEqual([1])
  expect(result.phases.map(phase => phase.wall.total)).toEqual([1, 1, 1, 1, 1])
  expect(result.phases[0]!.allocatedBytes).toBe(10)
  expect(result.phases[0]!.samples[0]!.liveByteDelta).toBe(-1)
  expect(summarizeObserveStages(events, new Set([1])).phases[0]!.samples).toEqual([])
})
