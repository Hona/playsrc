import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile, truncate } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { gzipSync } from "node:zlib"
import { CPU_PROFILE_LIMITS } from "../profile/cpu-profile-time"
import { loadCompositorEvidence, loadMainCpuEvidence, retainCompositorEvidence, retainEvidenceBlob, startMainCpuEvidence } from "../profile/compositor-evidence"
import { replayCpuProfiles } from "../profile/replay-cpu-profile"
import { TRACE_START, TRACE_END } from "../profile/compositor-truth"

const source = { sourceCommit: "1".repeat(40), sourceFingerprint: "2".repeat(64) }
const target = { targetId: "page-1", type: "page", url: "http://localhost:5173/tf2", browserContextId: "context-1" }
const identity = { ...source, sourceFingerprintAfter: source.sourceFingerprint, sourceUnchanged: true, origin: "http://localhost:5173" }
const profile = { startTime: 900100, endTime: 2100100, samples: [1, 1, 1, 1], timeDeltas: [100000, 600000, -400000, 600000],
  nodes: [{ id: 1, callFrame: { functionName: "sample", url: target.url, lineNumber: 0, columnNumber: 0 } }] }
const events = [{ name: TRACE_START, ts: 1000000, pid: 1, tid: 3 }, { name: TRACE_END, ts: 2000000, pid: 1, tid: 3 },
  { name: "FramePresented", ts: 1000000, pid: 2, tid: 7 }, { name: "FramePresented", ts: 2000000, pid: 2, tid: 7 }]
const probes = { started: 100, ended: 1100, joins: [], dropped: 0 }

async function fixture(directory: string, options: { profile?: unknown; stopError?: string; hangStop?: boolean; timestamps?: number[]; targetAfter?: typeof target } = {}) {
  const calls: string[] = []
  const timestamps = [...(options.timestamps ?? [0.9, 0.9002, 2.1, 2.1002])]
  let targets = 0
  const cdp = { send: async (method: string) => {
    calls.push(method)
    if (method === "Target.getTargetInfo") return { targetInfo: { ...(targets++ ? options.targetAfter ?? target : target) } }
    if (method === "Performance.getMetrics") return { metrics: [{ name: "Timestamp", value: timestamps.shift() }] }
    if (method === "Profiler.stop") {
      if (options.hangStop) return new Promise<never>(() => {})
      if (options.stopError) throw new Error(options.stopError)
      return { profile: options.profile ?? profile }
    }
    return {}
  } }
  const capture = await startMainCpuEvidence(cdp as any, directory, source)
  const first = capture.stop(), second = capture.stop()
  expect(first).toBe(second)
  const mainCpu = await first
  expect(calls.filter(call => call === "Profiler.start")).toHaveLength(1)
  expect(calls.filter(call => call === "Profiler.stop")).toHaveLength(1)
  const workerCpu = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify({ schema: "playsrc-worker-cpu-v1", captures: [], error: null })), "workers.json")
  const input = { directory, raw: gzipSync(JSON.stringify({ traceEvents: events })), complete: true, dataLossOccurred: false,
    identity: { ...identity, workerCpu }, probes, mainCpu }
  const saved = await retainCompositorEvidence(input)
  return { mainCpu, saved, input, filename: path.join(directory, saved.artifact.file) }
}

async function inDirectory(run: (directory: string) => Promise<void>) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-main-evidence-"))
  try { await run(directory) } finally { await rm(directory, { recursive: true, force: true }) }
}

async function changedManifest(directory: string, manifest: any) {
  const artifact = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify(manifest, null, 2)), "manifest.json")
  return path.join(directory, artifact.file)
}

describe("main CPU capture -> immutable bytes -> manifest -> offline replay", () => {
  test("binds exact bytes, source, page and monotonic clocks without changing signed samples or tails", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory)
    expect(saved.manifest.schema).toBe("playsrc-compositor-evidence-v2")
    expect(saved.manifest.complete).toBe(true)
    const main = saved.manifest.mainCpu!
    expect(await readFile(path.join(directory, main.profile!.file), "utf8")).toBe(JSON.stringify(profile))
    expect(main.profile!.bytes).toBe(Buffer.byteLength(JSON.stringify(profile)))
    const result = await replayCpuProfiles(filename)
    expect(result.main).toMatchObject({ authenticated: true, captureComplete: true,
      cpu: { estimatedSampledMilliseconds: 800, unattributedMilliseconds: 400 },
      source: { ...source, target, targetAfter: target } })
    expect(result.main!.cpu!.negativeDeltaCount).toBe(1)
    expect(result.main!.activeCpu!.trailingUnattributedMilliseconds).toBeCloseTo(199.9)
    await expect(replayCpuProfiles(filename, path.join(directory, main.profile!.file))).rejects.toThrow("authenticated manifest")
    expect((await retainCompositorEvidence((await fixture(directory)).input)).artifact).toEqual(saved.artifact)
  }))

  test("partial/failed samplers retain diagnostics and any malformed original bytes, never sampled CPU", () => inDirectory(async directory => {
    for (const options of [{ stopError: "renderer detached" }, { profile: { ...profile, timeDeltas: [1] } },
      { targetAfter: { ...target, targetId: "other-page" } }, { timestamps: [0.9, 0.9002, 1.9, 2.1002] }]) {
      const { saved, filename } = await fixture(directory, options)
      expect(saved.manifest.complete).toBe(false)
      expect(saved.manifest.mainCpu!.errors.length).toBeGreaterThan(0)
      const replay = await replayCpuProfiles(filename)
      expect(replay.main).toMatchObject({ authenticated: true, captureComplete: false, cpu: null, activeCpu: null })
      if (options.profile) expect(await readFile(path.join(directory, saved.manifest.mainCpu!.profile!.file), "utf8")).toBe(JSON.stringify(options.profile))
    }
    const { input } = await fixture(directory)
    const partial = await retainCompositorEvidence({ ...input, raw: Buffer.from("partial trace"), complete: false, dataLossOccurred: true })
    const filename = path.join(directory, partial.artifact.file)
    const loaded = await loadCompositorEvidence(filename)
    expect((await loadMainCpuEvidence(filename, loaded))!.profile).toBeNull()
    expect(loaded.manifest.errors).toContain("Chromium reported trace data loss")
    expect(await readFile(path.join(directory, partial.manifest.mainCpu!.profile!.file), "utf8")).toBe(JSON.stringify(profile))
  }))

  test("bounded stop timeout remains replayable failure evidence", () => inDirectory(async directory => {
    const { filename } = await fixture(directory, { hangStop: true })
    const replay = await replayCpuProfiles(filename)
    expect(replay.main).toMatchObject({ authenticated: true, captureComplete: false, profile: null, cpu: null })
    expect(replay.main!.errors).toContain("Error: Main CPU collection exceeded 5 seconds")
  }), 8000)

  test("capture overflow retains exactly the bounded prefix and byte-count loss", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory, { profile: { ...profile, overflow: "x".repeat(CPU_PROFILE_LIMITS.bytes) } })
    expect(saved.manifest.mainCpu!.profile!.bytes).toBe(CPU_PROFILE_LIMITS.bytes)
    expect(saved.manifest.mainCpu!.capturedBytes).toBeGreaterThan(CPU_PROFILE_LIMITS.bytes)
    expect(saved.manifest.complete).toBe(false)
    expect((await replayCpuProfiles(filename)).main).toMatchObject({ captureComplete: false, cpu: null })
  }))

  test("missing, duplicate, cross-thread and drifting active marks cannot authenticate a CPU clock join", () => inDirectory(async directory => {
    const { input } = await fixture(directory)
    for (const traceEvents of [events.slice(1), [...events, events[0]],
      events.map(event => event.name === TRACE_END ? { ...event, tid: 4 } : event),
      events.map(event => event.name === TRACE_END ? { ...event, ts: event.ts + 101 } : event)]) {
      const saved = await retainCompositorEvidence({ ...input, raw: gzipSync(JSON.stringify({ traceEvents })) })
      const filename = path.join(directory, saved.artifact.file)
      const loaded = await loadCompositorEvidence(filename)
      expect(loaded.manifest.complete).toBe(false)
      expect((await loadMainCpuEvidence(filename, loaded))!.profile).toBeNull()
    }
  }))

  test("rejects tampered, missing, swapped and oversized blob files including failed-capture bytes", () => inDirectory(async directory => {
    const { saved, filename } = await fixture(directory)
    const file = path.join(directory, saved.manifest.mainCpu!.profile!.file)
    const bytes = await readFile(file)
    await writeFile(file, bytes.toString().replace("sample", "tamper"))
    await expect(replayCpuProfiles(filename)).rejects.toThrow("hash identity")
    await writeFile(file, JSON.stringify({ ...profile, endTime: profile.endTime + 1 }))
    await expect(replayCpuProfiles(filename)).rejects.toThrow("identity")
    await truncate(file, CPU_PROFILE_LIMITS.bytes + 1)
    await expect(replayCpuProfiles(filename)).rejects.toThrow("byte identity")
    await rm(file)
    await expect(replayCpuProfiles(filename)).rejects.toThrow()
    await writeFile(file, bytes)
    const failed = await fixture(directory, { profile: { ...profile, timeDeltas: [1] } })
    await writeFile(path.join(directory, failed.saved.manifest.mainCpu!.profile!.file), "corrupt")
    await expect(replayCpuProfiles(failed.filename)).rejects.toThrow("identity")
  }))

  test("replay independently checks source, target, clock joins, sample bounds and exact artifact identity", () => inDirectory(async directory => {
    const { saved } = await fixture(directory)
    for (const alter of [
      (m: any) => { m.mainCpu.source.sourceFingerprint = "3".repeat(64) },
      (m: any) => { m.identity.sourceFingerprintAfter = "3".repeat(64) },
      (m: any) => { m.mainCpu.source.target.type = "worker" },
      (m: any) => { m.mainCpu.source.targetAfter.targetId = "other" },
      (m: any) => { m.mainCpu.clock.domain = "wall clock" },
      (m: any) => { m.mainCpu.clock.start = [0, 1] },
      (m: any) => { m.mainCpu.clock.stop = [1, 2] },
      (m: any) => { m.mainCpu.profile.bytes++ },
      (m: any) => { m.mainCpu.profile.file = "../escape.main.cpuprofile" },
      (m: any) => { m.mainCpu.profile = null },
    ]) {
      const manifest = structuredClone(saved.manifest)
      alter(manifest)
      await expect(replayCpuProfiles(await changedManifest(directory, manifest))).rejects.toThrow()
    }
    for (const bad of [{ ...profile, startTime: 800100 }, { ...profile, timeDeltas: [1] },
      { ...profile, samples: Array(CPU_PROFILE_LIMITS.samples + 1).fill(1), timeDeltas: Array(CPU_PROFILE_LIMITS.samples + 1).fill(0) }]) {
      const blob = await retainEvidenceBlob(directory, Buffer.from(JSON.stringify(bad)), "main.cpuprofile")
      const manifest = structuredClone(saved.manifest)
      manifest.mainCpu!.profile = blob
      manifest.mainCpu!.capturedBytes = blob.bytes
      await expect(replayCpuProfiles(await changedManifest(directory, manifest))).rejects.toThrow()
    }
  }))

  test("historical supplied captures stay unlinked and unchanged, never silently upgraded", () => inDirectory(async directory => {
    const { input } = await fixture(directory)
    const saved = await retainCompositorEvidence({ ...input, mainCpu: undefined })
    const filename = path.join(directory, saved.artifact.file)
    const before = await readFile(filename)
    const file = path.join(directory, "historical.cpuprofile")
    await writeFile(file, JSON.stringify(profile))
    expect((await replayCpuProfiles(filename)).main).toBeNull()
    expect((await replayCpuProfiles(filename, file)).main).toMatchObject({ authenticated: false,
      identity: "separately supplied; not linked by compositor manifest", cpu: { estimatedSampledMilliseconds: 800 } })
    expect(await readFile(filename)).toEqual(before)
    expect(await readFile(file, "utf8")).toBe(JSON.stringify(profile))
    await truncate(file, CPU_PROFILE_LIMITS.bytes + 1)
    await expect(replayCpuProfiles(filename, file)).rejects.toThrow("byte bound")
  }))
})
