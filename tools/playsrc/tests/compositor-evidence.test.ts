import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { gzipSync } from "node:zlib"
import { TRACE_START, TRACE_END } from "../profile/compositor-truth"
import { analyzeCompositorEvidence, compositorTraceWindow, decodeRawTrace, drainTraceStream, replayCompositorEvidence, retainCompositorEvidence, redecodeCompositorEvidence, type RawTraceEvent, type TraceProbes } from "../profile/compositor-evidence"

const probes: TraceProbes = { started: 100, ended: 1100, dropped: 0, joins: [
  { kind: "gpu", at: 180, end: 700, detail: { method: "mapAsync", resource: 12 } },
  { kind: "class", at: 600, detail: { class: 6 } },
] }
const events: RawTraceEvent[] = [
  { name: "thread_name", ph: "M", pid: 2, tid: 7, args: { name: "CrGpuMain" } },
  { name: TRACE_START, ts: 1_000_000, pid: 1, tid: 3 },
  { name: "Display::FrameDisplayed", ts: 1_050_000, pid: 2, tid: 7, args: { frame: 10 } },
  { name: "CommandBuffer::Flush", ts: 1_070_000, dur: 505_425, pid: 2, tid: 7, cat: "gpu" },
  { name: "Display::FrameDisplayed", ts: 1_600_000, pid: 2, tid: 7, args: { frame: 11 } },
  { name: TRACE_END, ts: 2_000_000, pid: 1, tid: 3 },
  { name: "Display::FrameDisplayed", ts: 2_020_000, pid: 2, tid: 7 },
  { name: "Display::FrameDisplayed", ts: 2_120_001, pid: 2, tid: 7 },
]

describe("bounded durable cross-process compositor evidence", () => {
  test("count-only reanalysis preserves the failed manifest and every bad frame, never stream loss", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-trace-count-"))
    try {
      const options = { directory, raw: gzipSync(JSON.stringify({ traceEvents: events })), complete: true, dataLossOccurred: false,
        identity: { sourceUnchanged: true, interrupted: false }, probes, maximumEvents: 2 }
      const original = await retainCompositorEvidence(options)
      expect(original.manifest.complete).toBe(false)
      const filename = path.join(directory, original.artifact.file)
      const before = await readFile(filename)
      expect((await replayCompositorEvidence(filename)).complete).toBe(false)
      const recovered = await redecodeCompositorEvidence(filename)
      expect(recovered.manifest.complete).toBe(true)
      expect(recovered.manifest.analysis.eventCount).toBe(events.length)
      expect(recovered.manifest.analysis.incidents[0]!.milliseconds).toBe(550)
      expect((await readFile(filename)).equals(before)).toBe(true)
      expect((await replayCompositorEvidence(filename)).complete).toBe(false)
      const loss = await retainCompositorEvidence({ ...options, dataLossOccurred: true })
      await expect(redecodeCompositorEvidence(path.join(directory, loss.artifact.file))).rejects.toThrow("Only a count-limited")
    } finally { await rm(directory, { recursive: true, force: true }) }
  })
  test("joins exact page clocks to unchanged native event indices across Renderer and GPU threads", () => {
    const result = analyzeCompositorEvidence(events, probes)
    expect(result.window).toMatchObject({ offsetMicroseconds: 900_000, endErrorMicroseconds: 0, pid: 1, tid: 3 })
    expect(result.incidents[0]).toMatchObject({ firstEvent: 2, lastEvent: 4, milliseconds: 550, thresholds: [50, 100, 250], scope: "sample",
      work: [{ event: 3, overlapMicroseconds: 505_425, thread: "CrGpuMain" }],
      joins: [{ probe: 0, startedMicroseconds: 1_080_000, endedMicroseconds: 1_600_000 }, { probe: 1, startedMicroseconds: 1_500_000 }] })
    expect(result.incidents.find(incident => incident.firstEvent === 6)).toMatchObject({ milliseconds: 100.001, scope: "after-sample", thresholds: [50, 100] })
    expect(result.coverage.displayVsync).toEqual({ count: 0, names: [] })
    expect(result.issues).toEqual([])
  })

  test("retains boundary/collection stalls rather than hiding them or counting them as active gameplay", () => {
    const result = analyzeCompositorEvidence(events, probes)
    expect(result.incidents.map(incident => incident.scope)).toEqual(["sample", "sample-boundary", "after-sample"])
    expect(result.incidents).toHaveLength(3)
  })

  test("never joins separate display threads or uses callbacks as displayed frames", () => {
    const result = analyzeCompositorEvidence([...events.slice(0, 2), events[5]!,
      { name: "Display::FrameDisplayed", ts: 1_100_000, pid: 2, tid: 7 },
      { name: "Display::FrameDisplayed", ts: 1_900_000, pid: 9, tid: 7 },
      { name: "RequestAnimationFrame", ts: 1_500_000, pid: 2, tid: 7 }], probes)
    expect(result.incidents).toEqual([])
  })

  test("uses strict 50/100/250 ms thresholds and bounds detail without losing total incidence", () => {
    const presentations: RawTraceEvent[] = [50, 100, 250, 251].flatMap((milliseconds, index) => [
      { name: "FramePresented", ts: 1_000_000, pid: index, tid: 7 },
      { name: "FramePresented", ts: 1_000_000 + milliseconds * 1000, pid: index, tid: 7 },
    ])
    const result = analyzeCompositorEvidence([events[1]!, events[5]!, ...presentations], probes)
    expect(result.incidents.map(incident => incident.thresholds)).toEqual([[50, 100, 250], [50, 100], [50]])
    const many = Array.from({ length: 102 }, (_, index) => ({ name: "FramePresented", ts: 1_000_000 + index * 60_000, pid: 2, tid: 7 }))
    const bounded = analyzeCompositorEvidence([events[1]!, events[5]!, ...many], probes)
    expect(bounded.incidentCount).toBe(101)
    expect(bounded.thresholdCounts.over50).toBe(101)
    expect(bounded.incidents).toHaveLength(64)
    expect(bounded.incidentDetailsTruncated).toBe(true)
  })

  test("rejects missing, duplicate, cross-thread and drifting clock anchors without metric fallback", () => {
    for (const bad of [events.filter(event => event.name !== TRACE_END), [...events, events[1]!],
      events.map(event => event.name === TRACE_END ? { ...event, tid: 99 } : event),
      events.map(event => event.name === TRACE_END ? { ...event, ts: event.ts! + 1000 } : event)]) {
      expect(() => compositorTraceWindow(bad, probes)).toThrow()
      expect(analyzeCompositorEvidence(bad, probes).window).toBeNull()
      expect(analyzeCompositorEvidence(bad, probes).issues.length).toBeGreaterThan(0)
    }
  })

  test("retains and replays raw incident events with immutable source/build/adapter/viewport identity", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-compositor-evidence-"))
    try {
      const raw = gzipSync(JSON.stringify({ traceEvents: events, metadata: { original: "preserved" } }))
      const identity = { sourceCommit: "1".repeat(40), sourceFingerprint: "2".repeat(64), applicationGeneration: { bundle: "3".repeat(64) }, gpu: { device: "test" }, viewport: { width: 1280, height: 720, dpr: 2 } }
      const saved = await retainCompositorEvidence({ directory, raw, identity, probes, complete: true, dataLossOccurred: false })
      expect(saved.manifest.complete).toBe(true)
      expect(await readFile(path.join(directory, saved.manifest.trace.file))).toEqual(raw)
      expect(saved.events).toEqual(events)
      const replay = await replayCompositorEvidence(path.join(directory, saved.artifact.file))
      expect(replay.identity).toEqual(identity)
      expect(replay.analysis).toEqual(saved.manifest.analysis)
      expect((await retainCompositorEvidence({ directory, raw, identity, probes, complete: true, dataLossOccurred: false })).artifact).toEqual(saved.artifact)
      await writeFile(path.join(directory, saved.manifest.trace.file), "corrupt")
      await expect(replayCompositorEvidence(path.join(directory, saved.artifact.file))).rejects.toThrow("identity mismatch")
    } finally { await rm(directory, { recursive: true, force: true }) }
  })

  test("persists malformed/partial traces and missing-mark evidence before reporting failure", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-compositor-loss-"))
    try {
      for (const raw of [Buffer.from("partial gzip"), gzipSync(JSON.stringify({ traceEvents: events.slice(2, 5) }))]) {
        const saved = await retainCompositorEvidence({ directory, raw, identity: {}, probes, complete: false, dataLossOccurred: true })
        expect(saved.manifest.complete).toBe(false)
        expect(saved.manifest.errors).toContain("Chromium reported trace data loss")
        expect(await readFile(path.join(directory, saved.manifest.trace.file))).toEqual(raw)
        expect((await replayCompositorEvidence(path.join(directory, saved.artifact.file))).complete).toBe(false)
      }
    } finally { await rm(directory, { recursive: true, force: true }) }
  })

  test("bounds decompression and rejects non-event data", () => {
    expect(() => decodeRawTrace(gzipSync(" ".repeat(1024)), 128)).toThrow()
    expect(() => decodeRawTrace(gzipSync(JSON.stringify({ traceEvents: [null] })))).toThrow("format")
  })

  test("retains a complete ten-second 24-player trace beyond one million events without raising byte caps", () => {
    const events = Array.from({ length: 1_045_840 }, () => ({ name: "RunTask" }))
    expect(decodeRawTrace(gzipSync(JSON.stringify({ traceEvents: events })))).toHaveLength(events.length)
  })

  test("drains bounded raw CDP bytes after sampling and always closes the stream", async () => {
    const calls: string[] = []
    const send = async (method: string) => { calls.push(method); return method === "IO.read" ? { data: "YWJjZA==", base64Encoded: true, eof: true } : {} }
    expect(await drainTraceStream({ send } as any, "trace", 3)).toEqual({ bytes: Buffer.from("abc"), complete: false })
    expect(calls).toEqual(["IO.read", "IO.close"])
    expect(await drainTraceStream({ send } as any, "trace", 4)).toEqual({ bytes: Buffer.from("abcd"), complete: true })
  })

  test("retains partial stream bytes on a transport failure rather than throwing away the incident", async () => {
    let reads = 0, closed = false
    const send = async (method: string) => {
      if (method === "IO.close") { closed = true; return {} }
      if (++reads > 1) throw new Error("transport closed")
      return { data: "partial", eof: false }
    }
    expect(await drainTraceStream({ send } as any, "trace")).toEqual({ bytes: Buffer.from("partial"), complete: false })
    expect(closed).toBe(true)
  })
})
