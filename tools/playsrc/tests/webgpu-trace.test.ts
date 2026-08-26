import { expect, test } from "bun:test"
import { nativeTraceSlices, summarizeWebGpuTrace } from "../profile/webgpu-trace"

test("GPU command work retains start-straddling wall and full CPU evidence", () => {
  const report = summarizeWebGpuTrace([
    { name: "thread_name", pid: 1, tid: 2, args: { name: "CrGpuMain" } },
    { name: "WebGPU", ph: "X", pid: 1, tid: 2, ts: 50_000, dur: 836_489, tdur: 810_837 },
    { name: "WebGPU", ph: "X", pid: 1, tid: 2, ts: 1_000_000, dur: 660_744, tdur: 640_281 },
    { name: "WebGPU", ph: "X", pid: 1, tid: 3, ts: 1_000_000, dur: 999_999 },
  ], { startedMicroseconds: 103_957, endedMicroseconds: 2_000_000 })
  expect(report.commands).toBe(2)
  expect(report.maximumStartedWithinActiveMilliseconds).toBe(660.744)
  expect(report.maximumOverlappingMilliseconds).toBe(836.489)
  expect(report.maximumActiveOverlapMilliseconds).toBe(782.532)
  expect(report.longest[0]).toMatchObject({ activeOverlapMilliseconds: 782.532, threadCpuMilliseconds: 810.837 })
})

test("Dawn B/E nesting is process/thread-local and is not summed", () => {
  const report = summarizeWebGpuTrace([
    { name: "thread_name", pid: 1, tid: 2, args: { name: "CrGpuMain" } },
    { name: "WebGPU", ph: "X", pid: 1, tid: 2, ts: 0, dur: 100_000 },
    { name: "compile", ph: "B", cat: "disabled-by-default-gpu.dawn", pid: 1, tid: 2, ts: 1_000, tts: 0 },
    { name: "unrelated", ph: "B", cat: "gpu.dawn", pid: 2, tid: 2, ts: 2_000 },
    { name: "DXC", ph: "B", cat: "disabled-by-default-gpu.dawn", pid: 1, tid: 2, ts: 3_000, tts: 1_000 },
    { ph: "E", pid: 2, tid: 2, ts: 4_000 },
    { ph: "E", pid: 1, tid: 2, ts: 80_000, tts: 70_000 },
    { ph: "E", pid: 1, tid: 2, ts: 90_000, tts: 79_000 },
  ], { startedMicroseconds: 0, endedMicroseconds: 100_000 })
  expect(report.longest[0]!.threadCpuMilliseconds).toBeNull()
  expect(report.longest[0]!.dawn.map(e => [e.name, e.wallMilliseconds, e.threadCpuMilliseconds])).toEqual([
    ["compile", 89, 79], ["DXC", 77, 69],
  ])
  expect(report.longest[0]!.namedWork).toEqual([
    { name: "compile", count: 1, unionMilliseconds: 89 },
    { name: "DXC", count: 1, unionMilliseconds: 77 },
  ])
})

test("same-name recursive spans count time once while keeping both events", () => {
  const report = summarizeWebGpuTrace([
    { name: "thread_name", pid: 1, tid: 2, args: { name: "CrGpuMain" } },
    { name: "WebGPU", ph: "X", pid: 1, tid: 2, ts: 0, dur: 100_000 },
    { name: "compile", cat: "gpu.dawn", ph: "X", pid: 1, tid: 2, ts: 1_000, dur: 90_000 },
    { name: "compile", cat: "gpu.dawn", ph: "X", pid: 1, tid: 2, ts: 2_000, dur: 80_000 },
  ], { startedMicroseconds: 0, endedMicroseconds: 100_000 })
  expect(report.longest[0]!.namedWork).toEqual([{ name: "compile", count: 2, unionMilliseconds: 90 }])
})

test("unmatched endpoints, async fences and invalid durations are not CPU slices", () => {
  expect(nativeTraceSlices([
    { ph: "E", pid: 1, tid: 2, ts: 10 },
    { ph: "B", name: "open", pid: 1, tid: 2, ts: 20 },
    { ph: "b", name: "fence", pid: 1, tid: 2, ts: 25 },
    { ph: "e", name: "fence", pid: 1, tid: 2, ts: 30 },
    { ph: "X", ts: 30, dur: -1 },
  ])).toEqual([])
  expect(() => summarizeWebGpuTrace([], { startedMicroseconds: 1, endedMicroseconds: 0 })).toThrow()
})
