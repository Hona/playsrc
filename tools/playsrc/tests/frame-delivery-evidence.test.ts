import { expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { gzipSync } from "node:zlib"
import { retainCompositorEvidence } from "../profile/compositor-evidence"
import { TRACE_END, TRACE_START } from "../profile/compositor-truth"
import { analyzeRetainedDelivery, gameplayWorkerCpu } from "../profile/frame-delivery-evidence"

test("Worker CPU attribution excludes nested tasks and does not invent clipped boundary CPU", () => {
  const identity = { pid: 1, tid: 2 }
  const events = [
    { ...identity, name: "FunctionCall", ts: 10, args: { data: { url: "http://127.0.0.1/gameplay-worker.ts" } } },
    { ...identity, name: "HandlePostMessage", ts: -10, dur: 30, tdur: 25 },
    { ...identity, name: "HandlePostMessage", ts: 30, dur: 40, tdur: 35 },
    { ...identity, name: "FunctionCall", ts: 31, dur: 38, tdur: 34 },
    { ...identity, name: "HandlePostMessage", ts: 90, dur: 30, tdur: 25 },
  ]
  const result = gameplayWorkerCpu(events, { startedMicroseconds: 0, endedMicroseconds: 100 })[0]!
  expect(result.interiorCallbacks).toBe(1)
  expect(result.cpuMilliseconds).toBe(0.035)
  expect(result.boundaryCallbacks).toHaveLength(2)
  expect(result.boundaryCallbacks.every(event => event.activeCpuMilliseconds === null)).toBe(true)
})

test("offline delivery analysis binds immutable trace/probes to the original measurement without manufacturing acceptance", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-delivery-evidence-"))
  try {
    const frame = { at: 400, displayFrame: 1, detail: { total: 6, models: 140 } }
    const measurement = { started: 0, ended: 6000, frames: [frame], worker: [], presentationCallbacks: [10, 20] }
    const retained = await retainCompositorEvidence({ directory, complete: true, dataLossOccurred: false,
      identity: { sourceUnchanged: true }, probes: { started: 0, ended: 6000, dropped: 0, joins: [{ kind: "completed-frame", at: frame.at, detail: frame }] },
      raw: gzipSync(JSON.stringify({ traceEvents: [
        { name: TRACE_START, ts: 1_000_000, pid: 1, tid: 1 },
        { name: "FramePresented", ts: 1_400_000, pid: 2, tid: 2 },
        { name: TRACE_END, ts: 7_000_000, pid: 1, tid: 1 },
      ] })) })
    const file = path.join(directory, "measurement.json"), manifest = path.join(directory, retained.artifact.file)
    await writeFile(file, JSON.stringify(measurement))
    const original = await readFile(manifest)
    const result = await analyzeRetainedDelivery(file, manifest)
    expect(result.newCapture).toBe(false)
    expect(result.performanceAccepted).toBe(false)
    expect(result.delivery.completed.count).toBe(1)
    expect(result.delivery.completed.quarterSecondBuckets.filter(bucket => bucket.count === 0)).toHaveLength(23)
    expect((await readFile(manifest)).equals(original)).toBe(true)
    await writeFile(file, JSON.stringify({ ...measurement, frames: [{ ...frame, at: 401 }] }))
    await expect(analyzeRetainedDelivery(file, manifest)).rejects.toThrow("linkage differs")
  } finally { await rm(directory, { recursive: true, force: true }) }
})
