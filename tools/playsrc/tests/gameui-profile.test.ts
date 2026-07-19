import { expect, test } from "bun:test"
import { metricDelta, summarizeCpuProfile, summarizeDistribution, summarizeTrace } from "../profile/gameui-profile"

test("summarizes bounded distributions", () => {
  expect(summarizeDistribution([4, 1, 2, 3])).toEqual({ count: 4, total: 10, mean: 2.5, p50: 3, p95: 4, p99: 4, max: 4 })
  expect(summarizeDistribution([])).toEqual({ count: 0, total: 0, mean: 0, p50: 0, p95: 0, p99: 0, max: 0 })
})

test("attributes CPU samples to self and inclusive frames", () => {
  const summary = summarizeCpuProfile({
    startTime: 0,
    endTime: 4_000,
    nodes: [
      { id: 1, callFrame: { functionName: "root", url: "", lineNumber: 0, columnNumber: 0 }, children: [2] },
      { id: 2, callFrame: { functionName: "publishDom", url: "http://127.0.0.1/packages/presentation/vgui/src/runtime.ts", lineNumber: 10, columnNumber: 2 } },
    ],
    samples: [2, 2],
    timeDeltas: [1_000, 2_000],
  })
  expect(summary.sampledMilliseconds).toBe(3)
  expect(summary.topSelf[0]?.function).toBe("publishDom")
  expect(summary.topInclusive.some((row) => row.function === "root" && row.milliseconds === 3)).toBe(true)
})

test("summarizes complete CDP events and metric deltas", () => {
  const trace = summarizeTrace([
    { name: "RunTask", cat: "devtools.timeline", ph: "X", dur: 60_000 },
    { name: "Layout", cat: "devtools.timeline", ph: "X", dur: 5_000 },
  ])
  expect(trace.longTasks.count).toBe(1)
  expect(trace.categories[0]?.name).toBe("RunTask")
  expect(metricDelta([{ name: "TaskDuration", value: 1 }], [{ name: "TaskDuration", value: 3 }])).toEqual({ TaskDuration: 2 })
})
