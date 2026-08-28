import { expect, test } from "bun:test"
import { selectionVisibleLatency } from "../profile/selection-visible-latency"
import { nativeSelectionRect } from "../profile/selection-transition-analysis"
import { compareSelectionIntervals, selectionLoadingInputs, selectionLoadingPressure } from "../profile/selection-comparison"

test("loading controls require exact input and completed transfer evidence, not equal byte counts alone", () => {
  const measurement = { evidence: { loadingIdentity: [{ kind: "request", objects: { bsp: { sha256: "a" } }, wasm: { sha256: "b" }, renderLevel: 0 },
    { kind: "resource-input", sha256: "c", byteLength: 100 }] } }
  const control = { requests: [{ path: `/objects/sha256/${"a".repeat(64)}`, finished: 10, encodedBytes: 100, diskCache: false }],
    pressure: [{ epoch: 0, cpus: [{ user: 0, sys: 0, idle: 0 }] }, { epoch: 1000, freeBytes: 100, totalBytes: 1000, cpus: [{ user: 10, sys: 10, idle: 80 }] }] }
  expect(selectionLoadingInputs(measurement, control).transfers[0].encodedBytes).toBe(100)
  expect(selectionLoadingPressure(control)[0]!.busyFraction).toBe(.2)
  const invalid = selectionLoadingPressure({ pressure: [control.pressure[1], { ...control.pressure[1], epoch: 1200,
    cpus: [{ user: 10, sys: 10, idle: 79 }] }] })
  expect(invalid[0]!.busyFraction).toBeNull()
  expect(invalid[0]!.counterFault).toBe(true)
  expect(() => selectionLoadingInputs({ evidence: {} }, control)).toThrow("not retained")
  expect(() => selectionLoadingInputs(measurement, { requests: [{ ...control.requests[0], finished: undefined }] })).toThrow("incomplete")
})

test("matched selection acceptance rejects censored and measurably slower transitions without hiding overlap", () => {
  const before = [{ scene: "class", input: "team", lowerMilliseconds: 2000, upperMilliseconds: 2100, endCensored: false },
    { scene: "world", input: "class", lowerMilliseconds: 100, upperMilliseconds: 160, endCensored: false }]
  const after = [{ ...before[0]!, lowerMilliseconds: 900, upperMilliseconds: 1000 }, { ...before[1]!, lowerMilliseconds: 110, upperMilliseconds: 170 }]
  expect(compareSelectionIntervals(before, after).map(value => value.disposition)).toEqual(["proven-reduction", "overlapping-measurement-intervals"])
  expect(compareSelectionIntervals(before, after)[0]!.remainingOver250Milliseconds).toBe(true)
  expect(() => compareSelectionIntervals(before, [{ ...after[0]!, endCensored: true }, after[1]!])).toThrow("complete")
  expect(() => compareSelectionIntervals(before, [after[0]!, { ...after[1]!, lowerMilliseconds: 161, upperMilliseconds: 180 }])).toThrow("regressed")
})

test("full desktop and full window pixel receipts address the same authored glyph without cropping admission", () => {
  const window = { left: 10, top: 10, width: 1296, height: 808 }
  const facts = { screenX: 10, screenY: 10, outerWidth: 1296, outerHeight: 808, innerWidth: 1280, innerHeight: 720,
    bounds: { x: 114, y: 618, width: 75, height: 27 } }
  const desktop = nativeSelectionRect({ width: 6000, height: 1440 }, { X: -2560, Y: 0, Width: 6000, Height: 1440 }, window, facts)
  const scoped = nativeSelectionRect({ width: 1296, height: 808 }, { X: 10, Y: 10, Width: 1296, Height: 808 }, window, facts)
  expect(desktop).toEqual({ x: 2692, y: 708, width: 75, height: 27, scale: 1 })
  expect(scoped).toEqual({ x: 122, y: 698, width: 75, height: 27, scale: 1 })
  expect(() => nativeSelectionRect({ width: 1200, height: 720 }, { X: 10, Y: 10, Width: 1200, Height: 720 }, window, facts)).toThrow("entire measured window")
})

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
