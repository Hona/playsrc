import { expect, test } from "bun:test"
import { sustainedFreezes } from "../profile/sustained-freezes"
test("freeze attribution keeps straddling observe calls, native spans, zeros and censored ends", () => {
  const sample = { timeOrigin: 10000, rpc: { records: [{ id: 1, kind: "observe", sent: 50, received: 1600, timings: { transactMilliseconds: 1200 } }],
    pending: [{ id: 2, kind: "models", sent: 1900, censoredEnd: true }] }, inputs: [{ at: 150, completedAt: 1600 }] }
  const workers = [{ target: { targetId: "game" }, execution: { timeOrigin: 10100, tasks: [{ requestId: 1, kind: "observe", started: 0, finished: 1450,
    observes: [{ started: 20, finished: 1220 }], responses: [{ started: 1220, finished: 1450, transport: "atomic" }] }] } }]
  const report = sustainedFreezes(sample, 100, 2200, [200, 1400], workers)
  expect(report.incidents.map(gap => gap.milliseconds)).toEqual([1200, 800])
  expect(report.incidents[0]!.requests[0].straddlesWindow).toBe(true)
  expect(report.incidents[0]!.workerTasks[0].observes[0]).toMatchObject({ start: 120, end: 1320 })
  expect(report.incidents[1]!.censoredEnd).toBe(true)
  expect(report.incidents[1]!.unfinishedRequests).toHaveLength(1)
  expect(report.thresholds.find(value => value.thresholdMilliseconds === 500)).toMatchObject({ count: 2, qualifyingGapMilliseconds: 2000, excessOverThresholdMilliseconds: 1000 })
  expect(report.delivery.quarterSecondBuckets.some(bucket => bucket.count === 0)).toBe(true)
})
test("an entirely silent window is retained, not an empty successful distribution", () => {
  const report = sustainedFreezes({ rpc: { records: [], pending: [] }, inputs: [] }, 0, 1000, [])
  expect(report.incidents).toMatchObject([{ milliseconds: 1000, censoredStart: true, censoredEnd: true }])
  expect(report.delivery.buckets[0]!.count).toBe(0)
})
