import { describe, expect, test } from "bun:test"
import { attributeCompositorEvidence } from "../profile/compositor-attribution"
import { analyzeCompositorEvidence, type RawTraceEvent, type TraceProbes } from "../profile/compositor-evidence"
import { TRACE_START, TRACE_END } from "../profile/compositor-truth"

const probes: TraceProbes = { started: 100, ended: 300, dropped: 0, joins: [] }
const base: RawTraceEvent[] = [
  { name: TRACE_START, ts: 1_000_000, pid: 1, tid: 1 },
  { name: TRACE_END, ts: 1_200_000, pid: 1, tid: 1 },
  { name: "Display::FrameDisplayed", ts: 1_010_000, pid: 2, tid: 2 },
  { name: "Display::FrameDisplayed", ts: 1_076_666, pid: 2, tid: 2 },
  { name: "Display::FrameDisplayed", ts: 1_210_000, pid: 2, tid: 2 },
  { name: "Display::FrameDisplayed", ts: 1_359_998, pid: 2, tid: 2 },
]
const attribute = (extra: RawTraceEvent[] = [], captured = probes) => {
  const events = [...base, ...extra]
  return attributeCompositorEvidence(events, captured, analyzeCompositorEvidence(events, captured))
}
const gameplay = (result: ReturnType<typeof attribute>) => result.incidents.find(i => i.scope === "sample")!

describe("offline compositor ownership without guessed critical paths", () => {
  test("separates gameplay, boundary and collection counts for each native presentation stream", () => {
    const result = attribute([{ name: "Display::FrameDisplayed", ts: 1_050_000, pid: 3, tid: 2 }])
    expect(result.streams[0]!.gameplayIntervals?.maximumMilliseconds).toBe(66.666)
    expect(result.streams[0]!.scopes.sample).toMatchObject({ intervals: 1, over50: 1, over100: 0 })
    expect(result.streams[0]!.scopes["after-sample"]).toMatchObject({ intervals: 1, over50: 1, over100: 1 })
    expect(result.streams[0]!.scopes["sample-boundary"]!.intervals).toBe(1)
    expect(result.streams[1]!.gameplayIntervals).toBeNull()
    expect(gameplay(result)).toMatchObject({ verdict: "unexplained-presentation-gap", criticalPath: null, firstEvent: 2, lastEvent: 3 })
  })

  test("unions nested and crossing slices per pid/tid including sub-millisecond work, never adding thread costs", () => {
    const extra: RawTraceEvent[] = [
      { name: "thread_name", pid: 1, tid: 1, args: { name: "CrRendererMain" } },
      { name: "process_name", pid: 1, args: { name: "Renderer" } },
      { name: "Task", ts: 1_005_000, dur: 20_000, ph: "X", pid: 1, tid: 1, args: { src_file: "task.cc", src_func: "Run" } },
      { name: "FunctionCall", ts: 1_015_000, dur: 5_000, ph: "X", pid: 1, tid: 1, args: { data: { url: "bundle.js", functionName: "render", lineNumber: 7 } } },
      { name: "Task", ts: 1_020_000, dur: 15_000, ph: "X", pid: 1, tid: 1 },
      { name: "small", ts: 1_040_000, dur: 500, ph: "X", pid: 1, tid: 1 },
      { name: "Task", ts: 1_010_000, dur: 50_000, ph: "X", pid: 2, tid: 1 },
      { name: "unpaired", ts: 1_010_000, dur: 66_666, ph: "B", pid: 1, tid: 1 },
    ]
    const before = JSON.stringify(extra), result = gameplay(attribute(extra)), thread = result.threads.find(t => t.key === "1:1")!
    expect(thread).toMatchObject({ coveredMicroseconds: 25_500, unobservedMicroseconds: 41_166, longestUnobservedMicroseconds: 36_166, sliceCount: 4,
      owner: { process: "Renderer", thread: "CrRendererMain" } })
    expect(thread.work.find(w => w.event === 8)!.source).toMatchObject({ file: "task.cc", function: "Run" })
    expect(thread.work.find(w => w.event === 9)!.source).toMatchObject({ url: "bundle.js", function: "render", line: 7 })
    expect(result.threads.find(t => t.key === "2:1")!.coveredMicroseconds).toBe(50_000)
    expect(JSON.stringify(extra)).toBe(before)
  })

  test("joins exact native dispatch IDs across threads, including scheduling outside the incident", () => {
    const result = gameplay(attribute([
      { name: "SchedulePostMessage", ts: 1_009_000, pid: 1, tid: 9, args: { data: { traceId: "9007199254740993" } } },
      { name: "HandlePostMessage", ts: 1_014_000, dur: 2000, ph: "X", pid: 1, tid: 1, args: { data: { traceId: "9007199254740993" } } },
    ]))
    expect(result.dependencies[0]).toMatchObject({ status: "matched-trace-id", scheduled: { event: 6 }, handled: { event: 7 }, scheduleToHandleMicroseconds: 5000 })
    expect(result.criticalPath).toBeNull()
  })

  test("does not join ambiguous, reversed, unsafe, cross-process or type-coerced message IDs", () => {
    const post: RawTraceEvent = { name: "SchedulePostMessage", ts: 1_009_000, pid: 1, tid: 9, args: { data: { traceId: "9" } } }
    const handle: RawTraceEvent = { name: "HandlePostMessage", ts: 1_014_000, pid: 1, tid: 1, args: { data: { traceId: "9" } } }
    for (const extra of [[post, post, handle], [post, handle, handle], [{ ...post, ts: 1_015_000 }, handle], [{ ...post, pid: 2 }, handle],
      [{ ...post, args: { data: { traceId: 9 } } }, handle], [post, { ...handle, args: { data: { traceId: 2 ** 54 } } }]]) {
      expect(gameplay(attribute(extra)).dependencies.every(d => d.status === "unmatched-or-ambiguous" && d.scheduleToHandleMicroseconds === null)).toBe(true)
    }
  })

  test("keeps call return separate from promise settlement and worker round trip separate from phase totals", () => {
    const result = gameplay(attribute([], { ...probes, joins: [
      { kind: "gpu", at: 115, end: 155, detail: { kind: "mapAsync", returned: 116, end: 155, phase: "main", resource: 12 } },
      { kind: "gpu", at: 117, end: 300, detail: { kind: "mapAsync", returned: 118 } },
      { kind: "gpu", at: 119, end: 120, detail: { kind: "submit", returned: 120, end: 120 } },
      { kind: "worker", at: 120, end: 140, detail: { kind: "observe", finished: 140, timings: { transactMilliseconds: 12 } } },
      { kind: "completed-frame", at: 141, detail: { renderer: { passes: [{ identity: "main", milliseconds: 15 }] } } },
    ] }))
    expect(result.probes[0]).toMatchObject({ synchronousMicroseconds: 1000, promiseSettled: true, phase: "main", endedMicroseconds: 1_055_000 })
    expect(result.probes[1]).toMatchObject({ promiseSettled: false, synchronousMicroseconds: 1000, rightCensored: true, observedEndMicroseconds: null })
    expect(result.probes[2]!.promiseSettled).toBeNull()
    expect(result.probes[3]).toMatchObject({ timing: "request-round-trip", rightCensored: false, observedEndMicroseconds: 1_040_000, detail: { timings: { transactMilliseconds: 12 } } })
    expect(result.probes[4]).toMatchObject({ timing: "point-observation", startedMicroseconds: 1_041_000, endedMicroseconds: 1_041_000 })
  })

  test("absent clock anchors never fabricate gameplay attribution", () => {
    const events = base.slice(2)
    const result = attributeCompositorEvidence(events, probes, analyzeCompositorEvidence(events, probes))
    expect(result.streams[0]!.gameplayIntervals).toBeNull()
    expect(result.incidents.every(i => i.scope === "unknown" && i.probes.length === 0)).toBe(true)
  })

  test("bounds detail without dropping full-stream counts and is deterministic without mutating evidence", () => {
    const extra: RawTraceEvent[] = Array.from({ length: 70 }, (_, i) => ({ name: "FramePresented", ts: 1_000_000 + i * 60_000, pid: 9, tid: 9 }))
    const events = base.filter(e => e.name !== "Display::FrameDisplayed").concat(extra)
    const analysis = analyzeCompositorEvidence(events, probes), before = JSON.stringify(analysis)
    const result = attributeCompositorEvidence(events, probes, analysis)
    expect(result.incidentDetailsTruncated).toBe(true)
    expect(result.incidents.length).toBe(64)
    expect(Object.values(result.streams[0]!.scopes).reduce((sum, count) => sum + count.over50, 0)).toBe(69)
    expect(attributeCompositorEvidence(events, probes, analysis)).toEqual(result)
    expect(JSON.stringify(analysis)).toBe(before)
    const many = gameplay(attribute(Array.from({ length: 130 }, (_, i) => ({ name: "HandlePostMessage", ph: "X", pid: 1, tid: 1, ts: 1_014_000 + i, dur: 1 })),
      { ...probes, joins: Array.from({ length: 260 }, () => ({ kind: "completed-frame", at: 120 })) }))
    expect(many).toMatchObject({ dependencyCount: 130, dependenciesTruncated: true, probeCount: 260, probesTruncated: true })
    expect(many.dependencies.length).toBe(128)
    expect(many.probes.length).toBe(256)
    expect(many.threads[0]!.work.length).toBe(12)
    expect(many.threads[0]).toMatchObject({ sliceCount: 130, coveredMicroseconds: 130, detailTruncated: true })
  })
})
