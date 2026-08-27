import { describe, expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import { mkdtemp, readFile, rm, truncate, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { gzipSync } from "node:zlib"
import { ALLOCATION_LIMITS, replayAllocationMemory, startAllocationCapture, summarizeAllocationProfile } from "../profile/allocation-memory-evidence"
import { retainCapturePlan, retainCompositorEvidence, retainEvidenceBlob, startMainCpuEvidence } from "../profile/compositor-evidence"
import { upwardCapturePlan } from "../profile/upward-capture-plan"
import { summarizeSnapshotTransport } from "../profile/snapshot-transport-memory"
import { replayCpuProfiles } from "../profile/replay-cpu-profile"
import { TRACE_START, TRACE_END } from "../profile/compositor-truth"

const source = { sourceCommit: "1".repeat(40), sourceFingerprint: "2".repeat(64) }
const target = { targetId: "page-1", type: "page", url: "http://localhost:5173/tf2", browserContextId: "context-1" }
const profile = { head: { id: 1, selfSize: 65536, callFrame: { functionName: "copy", url: target.url }, children: [] },
  samples: [{ nodeId: 1, size: 65536, ordinal: 1 }] }
const cpuProfile = { startTime: 905000, endTime: 2105000, samples: [1], timeDeltas: [100000],
  nodes: [{ id: 1, callFrame: { functionName: "copy", url: target.url, lineNumber: 0, columnNumber: 0 } }] }
const events = [{ name: TRACE_START, ts: 1000000, pid: 1, tid: 3 }, { name: TRACE_END, ts: 2000000, pid: 1, tid: 3 },
  { name: "FramePresented", ts: 1000000, pid: 2, tid: 7 }, { name: "FramePresented", ts: 2000000, pid: 2, tid: 7 }]
const boundary = (at: number, retainedBaselineBytes: number, responses: number, ownerToken = 0) => ({ at, ownerToken, values: { retainedBaselineBytes, responses } })
const processes = { platform: "darwin", startedAt: 1000, endedAt: 1010, error: null,
  processes: [{ id: 1, type: "renderer", residentBytes: 1000, privateBytes: null }, { id: 2, type: "GPU", residentBytes: 2000, privateBytes: null }],
  residentBytes: 3000, privateBytes: null }

async function fixture(directory: string, options: { allocation?: any; fail?: string; clearContext?: boolean; hangStop?: boolean; worker?: boolean } = {}) {
  const cdp: any = new EventEmitter(), calls: string[] = []
  const timestamps = [.7, .71, .72, .73, .9, .91, 2.1, 2.11, 2.3, 2.31, 2.32, 2.33]
  cdp.send = async (method: string) => {
    calls.push(method)
    if (method === "Page.getFrameTree") return { frameTree: { frame: { id: "frame-1" } } }
    if (method === "Runtime.enable") cdp.emit("Runtime.executionContextCreated", { context: { id: 7, uniqueId: "unique-main", origin: "http://localhost:5173",
      name: "", auxData: { isDefault: true, frameId: "frame-1" } } })
    if (method === "Target.getTargetInfo") return { targetInfo: { ...target } }
    if (method === "Performance.getMetrics") return { metrics: [{ name: "Timestamp", value: timestamps.shift() }] }
    if (method === "Runtime.getHeapUsage") return { usedSize: 1024, totalSize: 2048, backingStorageSize: 100, embedderHeapUsedSize: 50 }
    if (method === "Profiler.stop") return { profile: cpuProfile }
    if (method === "HeapProfiler.stopSampling") {
      if (options.hangStop) return new Promise(() => {})
      if (options.fail) throw new Error(options.fail)
      return { profile: options.allocation ?? profile }
    }
    return {}
  }
  const allocation = await startAllocationCapture(cdp, directory)
  const cpu = await startMainCpuEvidence(cdp, directory, source)
  const mainCpu = await cpu.stop()
  if (options.clearContext) cdp.emit("Runtime.executionContextsCleared")
  const first = allocation.stop()
  expect(allocation.stop()).toBe(first)
  const main = await first
  expect(calls.filter(call => call === "HeapProfiler.startSampling")).toHaveLength(1)
  expect(calls.filter(call => call === "HeapProfiler.stopSampling")).toHaveLength(1)
  expect(calls).not.toContain("HeapProfiler.collectGarbage")
  expect(cdp.listenerCount("Runtime.executionContextDestroyed")).toBe(0)
  const capturePlan = await retainCapturePlan(directory, upwardCapturePlan(options.worker ? { PROFILE_UPWARD_CLASS_SWITCH: "1" } : {}))
  const captures = options.worker ? [{ target: { ...target, targetId: "worker-1", type: "worker", url: "http://localhost:5173/gameplay-worker.ts" },
    executionContextId: 9, samplingIntervalMicroseconds: 1000, profile: cpuProfile,
    execution: { timeOrigin: 1000, limit: 16384, dropped: 0, tasks: [],
      clocks: [{ name: "worker-start", before: 0, after: 0 }, { name: "worker-stop", before: 1210, after: 1210 }] } }] : []
  const workerCpu = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", captures, error: null })), "workers.json")
  const traceEvents = options.worker ? [...events, { name: "worker-start", ts: 900000, pid: 1, tid: 4 }, { name: "worker-stop", ts: 2110000, pid: 1, tid: 4 }] : events
  const input = { directory, raw: gzipSync(JSON.stringify({ traceEvents })), complete: true, dataLossOccurred: false,
    identity: { ...source, sourceFingerprintAfter: source.sourceFingerprint, sourceUnchanged: true, origin: "http://localhost:5173",
      workerCpu, target: "pl_upward", entry: "training", capturePlan },
    probes: { started: 100, ended: 1100, joins: [], dropped: 0 }, mainCpu,
    memory: { schema: "playsrc-allocation-memory-v1" as const, main,
      snapshotTransport: { before: boundary(99, 144, 10), after: boundary(1101, 72, 12) },
      processes: { before: processes, after: { ...processes, startedAt: 3000, endedAt: 3010 } } } }
  const saved = await retainCompositorEvidence(input)
  return { saved, input, filename: path.join(directory, saved.artifact.file) }
}
async function inDirectory(run: (directory: string) => Promise<void>) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-allocation-evidence-"))
  try { await run(directory) } finally { await rm(directory, { recursive: true, force: true }) }
}
async function changedManifest(directory: string, manifest: any) {
  const artifact = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify(manifest)), "manifest.json")
  return path.join(directory, artifact.file)
}

describe("allocation capture -> immutable manifest -> offline memory attribution", () => {
  test("retains untouched profile with main target/context/process, all-phase clocks, plan and separate gauges", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory)
    expect(saved.manifest.schema).toBe("playsrc-compositor-evidence-v3")
    expect(saved.manifest.complete).toBe(true)
    expect(await readFile(path.join(directory, saved.manifest.memory!.main.profile!.file), "utf8")).toBe(JSON.stringify(profile))
    const replay = await replayAllocationMemory(filename)
    expect(replay).toMatchObject({ status: "complete", main: { owner: { target, targetAfter: target }, executionContext: { id: 7, uniqueId: "unique-main" },
      process: { pid: 1, before: { residentBytes: 1000 } }, tid: 3,
      estimatedAllocation: { estimatedBytesIncludingCollected: 65536, actualAllocatedBytes: null, retainedBytes: null, activeOnlyEstimatedBytes: null } },
      snapshotTransport: { retainedGauges: { beforeBytes: 144, afterBytes: 72 }, counterDeltas: { responses: 2 } },
      attribution: { nativeExclusiveBytes: null, gpuResourceBytes: null, sharedMappingBytes: null } })
    expect((await replayCpuProfiles(filename)).memory).toEqual(replay)
    expect(await replayAllocationMemory(filename)).toEqual(replay)
  }))

  test("rejects tampered, swapped, oversized and missing raw files before interpretation", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory)
    const file = path.join(directory, saved.manifest.memory!.main.profile!.file), original = await readFile(file)
    await writeFile(file, original.toString().replace("copy", "fake"))
    await expect(replayAllocationMemory(filename)).rejects.toThrow("hash identity")
    const other = await fixture(directory, { allocation: { ...profile, samples: [{ nodeId: 1, size: 65537, ordinal: 1 }] } })
    await writeFile(file, await readFile(path.join(directory, other.saved.manifest.memory!.main.profile!.file)))
    await expect(replayAllocationMemory(filename)).rejects.toThrow("identity")
    await truncate(file, ALLOCATION_LIMITS.bytes + 1)
    await expect(replayAllocationMemory(filename)).rejects.toThrow("byte identity")
    await rm(file)
    await expect(replayAllocationMemory(filename)).rejects.toThrow()
  }))

  test("reuses Worker CPU context and native clock joins without double-counting the shared renderer PID", () => inDirectory(async directory => {
    const { filename } = await fixture(directory, { worker: true })
    expect(await replayAllocationMemory(filename)).toMatchObject({ status: "complete", main: { process: { pid: 1 }, tid: 3 },
      workers: [{ target: { targetId: "worker-1" }, executionContextId: 9, process: { pid: 1 }, tid: 4, estimatedAllocation: null }],
      processes: { after: { residentBytes: 3000 } } })
  }))

  test("replay rejects missing profiles, wrong provenance, phases, plans, negative gauges and false totals", () => inDirectory(async directory => {
    const { saved } = await fixture(directory)
    for (const alter of [
      (m: any) => { delete m.memory },
      (m: any) => { m.memory.main.profile = null },
      (m: any) => { m.memory.main.profile.bytes = ALLOCATION_LIMITS.bytes + 1 },
      (m: any) => { m.memory.main.profile.file = "../escape.main.heapprofile" },
      (m: any) => { m.memory.main.context.origin = "http://other" },
      (m: any) => { m.memory.main.context.auxData.frameId = "other" },
      (m: any) => { m.mainCpu.source.targetAfter.targetId = "other" },
      (m: any) => { m.memory.main.clock.start = [1500000, 1600000] },
      (m: any) => { m.memory.main.clock.stop = [1900000, 1950000] },
      (m: any) => { m.memory.main.heapBefore.value.usedSize = -1 },
      (m: any) => { delete m.memory.main.heapBefore.value.usedSize },
      (m: any) => { m.memory.main.context.uniqueId = 1 },
      (m: any) => { m.memory.main.heapAfter.before = 0 },
      (m: any) => { m.memory.main.plan.worker = "required" },
      (m: any) => { delete m.identity.capturePlan },
      (m: any) => { m.memory.snapshotTransport.after.values.retainedBaselineBytes = -72 },
      (m: any) => { m.memory.snapshotTransport.after.values.retainedBaselineBytes = 1.5 },
      (m: any) => { m.memory.snapshotTransport.after.at = 1099 },
      (m: any) => { m.memory.processes.after.processes[0].residentBytes = -1 },
      (m: any) => { m.memory.processes.after.residentBytes++ },
    ]) {
      const manifest = structuredClone(saved.manifest)
      alter(manifest)
      await expect(replayAllocationMemory(await changedManifest(directory, manifest)), alter.toString()).rejects.toThrow()
    }
  }))

  test("failed, detached and malformed captures retain diagnostics, never passing sampled bytes", () => inDirectory(async directory => {
    for (const options of [{ fail: "detached" }, { clearContext: true }, { allocation: { head: {}, samples: [] } }]) {
      const { saved, filename } = await fixture(directory, options)
      expect(saved.manifest.complete).toBe(false)
      expect(await replayAllocationMemory(filename)).toMatchObject({ status: "incomplete", estimatedAllocation: null })
      if (options.allocation) {
        const file = path.join(directory, saved.manifest.memory!.main.profile!.file)
        expect(await readFile(file, "utf8")).toBe(JSON.stringify(options.allocation))
        await writeFile(file, "tampered")
        await expect(replayAllocationMemory(filename)).rejects.toThrow("identity")
      }
    }
  }))

  test("bounded overflow retains prefix and exact captured size; timeout retains missing-profile failure", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory, { allocation: { ...profile, overflow: "x".repeat(ALLOCATION_LIMITS.bytes) } })
    expect(saved.manifest.memory!.main.profile!.bytes).toBe(ALLOCATION_LIMITS.bytes)
    expect(saved.manifest.memory!.main.capturedBytes).toBeGreaterThan(ALLOCATION_LIMITS.bytes)
    expect(await replayAllocationMemory(filename)).toMatchObject({ status: "incomplete", estimatedAllocation: null })
    const timed = await fixture(directory, { hangStop: true })
    expect(await replayAllocationMemory(timed.filename)).toMatchObject({ status: "incomplete", estimatedAllocation: null })
    expect(timed.saved.manifest.memory!.main.errors.join(" ")).toContain("exceeded 5 seconds")
  }), 8000)

  test("historical missing raw profiles stay unknown; no fake before endpoints or silent migration", () => inDirectory(async directory => {
    const { input } = await fixture(directory)
    const saved = await retainCompositorEvidence({ ...input, memory: undefined })
    const filename = path.join(directory, saved.artifact.file), bytes = await readFile(filename)
    expect(await replayAllocationMemory(filename)).toMatchObject({ status: "unknown" })
    expect(await readFile(filename)).toEqual(bytes)
  }))

  test("failed native trace still authenticates retained heap bytes without inventing a PID/clock join", () => inDirectory(async directory => {
    const { input } = await fixture(directory)
    const saved = await retainCompositorEvidence({ ...input, raw: Buffer.alloc(0), complete: false, dataLossOccurred: true,
      collectionErrors: ["Native trace completion exceeded 5 seconds"] })
    const filename = path.join(directory, saved.artifact.file)
    expect(await replayAllocationMemory(filename)).toMatchObject({ status: "incomplete", estimatedAllocation: null,
      evidence: { main: { profile: input.memory.main.profile, capturedBytes: input.memory.main.capturedBytes } } })
    await writeFile(path.join(directory, input.memory.main.profile!.file), "tampered")
    await expect(replayAllocationMemory(filename)).rejects.toThrow("identity")
  }))

  test("bounded structural validation rejects sample/node inconsistencies without recursive traversal", () => {
    for (const bad of [{ ...profile, samples: [{ nodeId: 99, size: 1, ordinal: 1 }] },
      { ...profile, samples: [profile.samples[0], profile.samples[0]] },
      { ...profile, head: { ...profile.head, selfSize: -1 } },
      { ...profile, samples: Array(ALLOCATION_LIMITS.samples + 1).fill(profile.samples[0]) }]) expect(() => summarizeAllocationProfile(bad)).toThrow()
  })
})

describe("snapshot counters are not retained byte gauges", () => {
  test("shrinking baseline never reports negative physical bytes; growth is two gauges too", () => {
    for (const bytes of [72, 168]) {
      const result = summarizeSnapshotTransport(boundary(1, 144, 5), boundary(2, bytes, 8))
      expect(result.retainedGauges).toEqual({ beforeBytes: 144, afterBytes: bytes })
      expect(result.counterDeltas.responses).toBe(3)
      expect(result.counterDeltas).not.toHaveProperty("retainedBaselineBytes")
    }
  })
  test("missing endpoints, resets and owner replacement are unknown counter deltas, not guessed zeros", () => {
    expect(summarizeSnapshotTransport(boundary(1, 144, 5), boundary(2, 72, 1)).counterDeltas.responses).toBeNull()
    expect(summarizeSnapshotTransport(boundary(1, 144, 5), boundary(2, 72, 8, 1)).counterDeltas.responses).toBeNull()
    expect(summarizeSnapshotTransport({ at: 1, ownerToken: null, values: {} }, boundary(2, 72, 8)).retainedGauges.beforeBytes).toBeNull()
    expect(() => summarizeSnapshotTransport(boundary(1, 144, 5), { at: 2, ownerToken: 0, values: { futureGauge: 1 } })).toThrow("Unknown snapshot metric")
  })
})
