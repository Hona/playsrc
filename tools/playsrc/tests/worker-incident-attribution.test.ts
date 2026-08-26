import { describe, expect, test } from "bun:test"
import { attributeWorkerIncidents, replayWorkerIncidents } from "../profile/worker-incident-attribution"
import type { WorkerCpuCapture } from "../profile/worker-cpu-profiler"
import { retainCompositorEvidence, retainEvidenceBlob } from "../profile/compositor-evidence"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { gzipSync } from "node:zlib"
import { summarizeActivePresentationSilence } from "../profile/compositor-truth"

const capture: WorkerCpuCapture = {
  target: { targetId: "worker", type: "worker", url: "gameplay-worker.ts" }, samplingIntervalMicroseconds: 1000,
  profile: {
    startTime: 1_000_000, endTime: 1_150_000,
    nodes: [{ id: 1, callFrame: { functionName: "playsrc_simulation_observe", url: "wasm://module", lineNumber: 0, columnNumber: 42 } }],
    samples: [1, 1, 1], timeDeltas: [25_000, 25_000, 80_000],
  },
  execution: {
    timeOrigin: 8888, limit: 16384, dropped: 0,
    clocks: [{ name: "start", before: 0, after: 0 }, { name: "end", before: 150, after: 150 }],
    tasks: [{ sequence: 1, requestId: 9, kind: "observe", started: 20, finished: 140, startMark: "task:start", endMark: "task:end",
      responses: [{ requestId: 9, started: 139, finished: 139.1, timings: { transactMilliseconds: 118 } }], memory: [{ linearBytes: 65536 }, { linearBytes: 131072 }] }],
  },
}
const events = [
  { name: "start", ts: 1_000_000, pid: 1, tid: 2 }, { name: "end", ts: 1_150_000, pid: 1, tid: 2 },
  { name: "HandlePostMessage", ts: 1_019_999, dur: 120_002, pid: 1, tid: 2, args: { data: { traceId: "native-task-123" } } },
  { name: "MinorGC", ts: 1_050_000, dur: 3000, pid: 1, tid: 2 },
  { name: "MinorGC", ts: 1_060_000, dur: 5000, pid: 1, tid: 99 },
]
const window = { startedMicroseconds: 1_010_000, endedMicroseconds: 1_145_000 }

describe("Worker CPU and monotonic task joins", () => {
  test("cannot hide a long gameplay silence by ending the sample before its next native presentation", () => {
    const result = summarizeActivePresentationSilence([
      { name: "Display::FrameDisplayed", ts: 450_000, pid: 1, tid: 1 },
      { name: "Display::FrameDisplayed", ts: 1000_000, pid: 1, tid: 1 },
      { name: "Display::FrameDisplayed", ts: 1400_000, pid: 1, tid: 1 },
      { name: "Display::FrameDisplayed", ts: 650_000, pid: 2, tid: 2 },
    ], { startedMicroseconds: 0, endedMicroseconds: 800_000 })
    expect(result.maximumActiveSilenceMilliseconds).toBe(350)
    expect(result.maximumObservedOverlappingGapMilliseconds).toBe(550)
    expect(result.maximumActiveSilenceMilliseconds < 250).toBe(false)
    expect(result.longestActiveSilence?.stream).toBe("1:1")
    expect(summarizeActivePresentationSilence([{ ts: 0 }, { ts: 1_000_000 }], { startedMicroseconds: 0, endedMicroseconds: 1_000_000 }).longestActiveSilence).toBeNull()
  })

  test("replays hash-bound raw trace, Worker CPU and authoritative publication joins offline", async () => {
    const directory = await mkdtemp(path.join(tmpdir(), "playsrc-worker-evidence-"))
    try {
      const artifact = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", captures: [capture], error: null })), "workers.json")
      const raw = gzipSync(JSON.stringify({ traceEvents: [...events,
        { name: "playsrc-active-gameplay-start", ts: 1_010_000, pid: 1, tid: 1 },
        { name: "playsrc-active-gameplay-end", ts: 1_145_000, pid: 1, tid: 1 },
        { name: "Display::FrameDisplayed", ts: 1_010_000 }, { name: "Display::FrameDisplayed", ts: 1_145_000 },
      ] }))
      const evidence = await retainCompositorEvidence({ directory, raw, complete: true, dataLossOccurred: false, identity: { workerCpu: artifact },
        probes: { started: 10, ended: 145, dropped: 0, joins: [{ kind: "simulation-publication", at: 140, detail: { requestId: 9, publications: [{ selectedTicks: 6, eventBatches: 6 }] } }] } })
      const file = path.join(directory, evidence.artifact.file)
      const replay = await replayWorkerIncidents(file)
      expect(replay.compositorComplete).toBe(true)
      expect(replay.analyses[0]!.slowTasks[0]!.browserTransactions[0]?.publication).toMatchObject({ requestId: 9, publications: [{ selectedTicks: 6, eventBatches: 6 }] })
      await writeFile(path.join(directory, artifact.file), "corrupted")
      await expect(replayWorkerIncidents(file)).rejects.toThrow("byte bound or identity mismatch")
    } finally { await rm(directory, { recursive: true, force: true }) }
  })

  test("joins the actual target/thread/task/CPU without treating transaction execution as postMessage cost", () => {
    const result = attributeWorkerIncidents(events, [capture], window)[0]!
    expect(result.clock.offsetMicroseconds).toBe(1_000_000)
    expect(result.taskMilliseconds.max).toBe(120)
    expect(result.postMessageMilliseconds.max).toBe(0.1)
    expect(result.slowTasks[0]).toMatchObject({
      requestId: 9, linearMemoryGrowthBytes: 65536,
      nativeTasks: [{ args: { data: { traceId: "native-task-123" } } }],
      garbageCollection: [{ tid: 2 }], stacks: [{ samples: 3, frames: [{ functionName: "playsrc_simulation_observe" }] }],
    })
    expect(result.slowTasks[0]!.garbageCollection).toHaveLength(1)
  })
  test("rejects missing, ambiguous and incompatible clock evidence instead of inventing an offset", () => {
    expect(() => attributeWorkerIncidents(events.slice(1), [capture], window)).toThrow("missing or ambiguous")
    expect(() => attributeWorkerIncidents([...events, events[0]!], [capture], window)).toThrow("missing or ambiguous")
    expect(() => attributeWorkerIncidents(events.map(e => e.name === "end" ? { ...e, ts: e.ts + 5000 } : e), [capture], window)).toThrow("does not intersect")
  })
  test("excludes tasks outside active gameplay and marks bounded capture loss", () => {
    const lost = { ...capture, execution: { ...capture.execution, dropped: 4 } }
    const result = attributeWorkerIncidents(events, [lost], { startedMicroseconds: 1_141_000, endedMicroseconds: 1_145_000 })[0]!
    expect(result.captureComplete).toBe(false)
    expect(result.droppedTasks).toBe(4)
    expect(result.slowTasks).toHaveLength(0)
  })
})
