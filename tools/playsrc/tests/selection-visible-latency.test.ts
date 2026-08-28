import { expect, test } from "bun:test"
import { selectionVisibleLatency } from "../profile/selection-visible-latency"

test("selection latency counts changed native pixels, not publication or repeated surfaces", () => {
  const result = selectionVisibleLatency(100, 1000, [
    { startedEpoch: 0, endedEpoch: 90, matches: false },
    { startedEpoch: 110, endedEpoch: 120, matches: false },
    { startedEpoch: 700, endedEpoch: 710, matches: false },
    { startedEpoch: 800, endedEpoch: 810, matches: true },
  ])
  expect(result.lowerMilliseconds).toBe(600)
  expect(result.upperMilliseconds).toBe(710)
  expect(result.captureGapBounds.maximumMilliseconds).toBe(600)
  expect(result.endCensored).toBe(false)
  expect(result.captureGapBounds.over1000Milliseconds).toBe(0)
})

test("frozen, empty and zero-latency native boundaries cannot be filtered out", () => {
  expect(selectionVisibleLatency(100, 5100, []).censoredMilliseconds).toBe(5000)
  const frozen = selectionVisibleLatency(100, 5100, [{ startedEpoch: 200, endedEpoch: 300, matches: false }])
  expect(frozen.upperMilliseconds).toBeNull()
  expect(frozen.endCensored).toBe(true)
  expect(frozen.startCensored).toBe(true)
  expect(selectionVisibleLatency(100, 100, [{ startedEpoch: 100, endedEpoch: 100, matches: true }]).upperMilliseconds).toBe(0)
  expect(() => selectionVisibleLatency(100, 5100, [{ startedEpoch: 200, endedEpoch: 199, matches: false }])).toThrow("unordered")
})
