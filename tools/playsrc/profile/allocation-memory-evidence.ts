import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import path from "node:path"
import type { CDPSession } from "@playwright/test"
import { retainEvidenceBlob, loadCapturePlan, loadCompositorEvidence, loadMainCpuEvidence, type BlobIdentity } from "./compositor-evidence"
import { loadWorkerIncidents } from "./worker-incident-attribution"
import { summarizeSnapshotTransport, type SnapshotTransportBoundary } from "./snapshot-transport-memory"
import { completeMemoryTotal, type MemorySnapshot } from "./process-memory"

export const ALLOCATION_LIMITS = Object.freeze({ bytes: 16 * 1024 * 1024, nodes: 100_000, samples: 500_000 })
export const ALLOCATION_PLAN = Object.freeze({ owner: "main-page-isolate", worker: "not-requested", samplingInterval: 65_536,
  includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true, forcedGC: false,
  startPhase: "before-main-cpu-setup", stopPhase: "after-native-trace-drain", limits: ALLOCATION_LIMITS })
type HeapUsage = { usedSize: number; totalSize: number; embedderHeapUsedSize?: number; backingStorageSize?: number }
type Context = { id: number; uniqueId: string; origin: string; name: string; auxData?: { isDefault?: boolean; frameId?: string } }
type Clocked<T> = { before: number; after: number; value: T }
export type AllocationCapture = {
  profile: BlobIdentity | null; capturedBytes: number; errors: string[]; plan: typeof ALLOCATION_PLAN;
  clock: { domain: "Chromium monotonic microseconds"; start: number[]; stop: number[] };
  context: Context | null; frameId: string | null; heapBefore: Clocked<HeapUsage> | null; heapAfter: Clocked<HeapUsage> | null;
}
export type AllocationMemoryEvidence = {
  schema: "playsrc-allocation-memory-v1"; main: AllocationCapture;
  snapshotTransport: { before: SnapshotTransportBoundary; after: SnapshotTransportBoundary } | null;
  processes: { before: MemorySnapshot; after: MemorySnapshot | null };
}

/** CDP selfSize and sample.size are statistical estimates, not exact allocated
 * bytes. Samples have ordinal, not time: never pretend to slice active gameplay. */
export function summarizeAllocationProfile(profile: any) {
  if (!profile?.head || !Array.isArray(profile.samples) || profile.samples.length > ALLOCATION_LIMITS.samples) throw new Error("Allocation profile sample bound/format invalid")
  const stack = [profile.head], ids = new Set<number>(), sites = []
  let estimatedBytes = 0
  while (stack.length) {
    const node = stack.pop()
    if (!node || !Number.isSafeInteger(node.id) || ids.has(node.id) || !Number.isSafeInteger(node.selfSize) || node.selfSize < 0
      || !Array.isArray(node.children) || typeof node.callFrame?.functionName !== "string" || typeof node.callFrame?.url !== "string") throw new Error("Allocation node invalid")
    ids.add(node.id)
    if (ids.size + stack.length + node.children.length > ALLOCATION_LIMITS.nodes) throw new Error("Allocation node bound exceeded")
    estimatedBytes += node.selfSize
    if (!Number.isSafeInteger(estimatedBytes)) throw new Error("Allocation estimated byte bound exceeded")
    sites.push({ nodeId: node.id, estimatedBytes: node.selfSize, callFrame: node.callFrame })
    stack.push(...node.children)
  }
  const ordinals = new Set<number>()
  for (const sample of profile.samples) {
    if (!ids.has(sample.nodeId) || !Number.isSafeInteger(sample.size) || sample.size < 0
      || !Number.isSafeInteger(sample.ordinal) || sample.ordinal < 0 || ordinals.has(sample.ordinal)) throw new Error("Allocation sample invalid")
    ordinals.add(sample.ordinal)
  }
  return { estimatedBytesIncludingCollected: estimatedBytes, samples: profile.samples.length, nodes: ids.size,
    largestSelfSites: sites.sort((a, b) => b.estimatedBytes - a.estimatedBytes).slice(0, 32),
    actualAllocatedBytes: null, activeOnlyEstimatedBytes: null, retainedBytes: null,
    scope: "entire sampler interval including setup and collection; no per-sample clock; sampled estimates including collected objects" }
}

/** Replaces the existing page HeapProfiler sampler; no second collector or GC.
 * All raw serialization/retention occurs after the active gameplay interval. */
export async function startAllocationCapture(cdp: Pick<CDPSession, "send" | "on" | "off">, directory: string) {
  const evidence: AllocationCapture = { profile: null, capturedBytes: 0, errors: [], plan: ALLOCATION_PLAN,
    clock: { domain: "Chromium monotonic microseconds", start: [], stop: [] }, context: null, frameId: null, heapBefore: null, heapAfter: null }
  let deadline = Date.now() + 5000
  const attempt = async <T>(operation: () => Promise<T>): Promise<T | undefined> => {
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      if (Date.now() >= deadline) throw new Error("Allocation capture exceeded 5 seconds")
      return await Promise.race([operation(), new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Allocation capture exceeded 5 seconds")), deadline - Date.now()) })])
    } catch (error) { evidence.errors.push(String(error)); return undefined } finally { clearTimeout(timer) }
  }
  const timestamp = async () => {
    const result = await attempt(() => cdp.send("Performance.getMetrics"))
    const seconds = result?.metrics.find(metric => metric.name === "Timestamp")?.value
    if (seconds === undefined || !Number.isFinite(seconds)) { evidence.errors.push("Allocation monotonic timestamp missing"); return NaN }
    return seconds * 1_000_000
  }
  const contexts: Context[] = []
  evidence.frameId = (await attempt(() => cdp.send("Page.getFrameTree")))?.frameTree.frame.id ?? null
  const created = ({ context }: { context: Context }) => { if (context.auxData?.isDefault && context.auxData.frameId === evidence.frameId) contexts.push(context) }
  cdp.on("Runtime.executionContextCreated", created)
  await attempt(() => cdp.send("Runtime.enable"))
  cdp.off("Runtime.executionContextCreated", created)
  if (contexts.length === 1) evidence.context = contexts[0]!
  else evidence.errors.push("Main allocation default execution context missing or ambiguous")
  const destroyed = ({ executionContextId }: { executionContextId: number }) => {
    if (executionContextId === evidence.context?.id) evidence.errors.push("Allocation execution context destroyed")
  }
  const cleared = () => { evidence.errors.push("Allocation execution contexts cleared") }
  cdp.on("Runtime.executionContextDestroyed", destroyed)
  cdp.on("Runtime.executionContextsCleared", cleared)
  await attempt(() => cdp.send("Performance.enable"))
  await attempt(() => cdp.send("HeapProfiler.enable"))
  evidence.clock.start.push(await timestamp())
  await attempt(() => cdp.send("HeapProfiler.startSampling", { samplingInterval: ALLOCATION_PLAN.samplingInterval,
    includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true }))
  evidence.clock.start.push(await timestamp())
  const heap = async (): Promise<Clocked<HeapUsage> | null> => {
    const before = await timestamp(), value = await attempt(() => cdp.send("Runtime.getHeapUsage")), after = await timestamp()
    return value ? { before, after, value } : null
  }
  evidence.heapBefore = await heap()
  let stopped: Promise<AllocationCapture> | undefined
  return { stop: () => stopped ??= (async () => {
    deadline = Date.now() + 5000
    evidence.clock.stop.push(await timestamp())
    const result = await attempt(() => cdp.send("HeapProfiler.stopSampling"))
    evidence.clock.stop.push(await timestamp())
    if (result?.profile) {
      const bytes = Buffer.from(JSON.stringify(result.profile))
      evidence.capturedBytes = bytes.length
      evidence.profile = await retainEvidenceBlob(directory, bytes.subarray(0, ALLOCATION_LIMITS.bytes), "main.heapprofile")
      if (bytes.length > ALLOCATION_LIMITS.bytes) evidence.errors.push("Allocation profile byte bound exceeded; prefix retained")
      else try { summarizeAllocationProfile(result.profile) } catch (error) { evidence.errors.push(String(error)) }
    } else evidence.errors.push("Allocation profile missing")
    evidence.heapAfter = await heap()
    cdp.off("Runtime.executionContextDestroyed", destroyed)
    cdp.off("Runtime.executionContextsCleared", cleared)
    return evidence
  })() }
}

export async function loadAllocationMemoryEvidence(filename: string, loaded: Omit<Awaited<ReturnType<typeof loadCompositorEvidence>>, "raw">) {
  const memory = loaded.manifest.memory as AllocationMemoryEvidence | undefined
  if (loaded.manifest.schema !== "playsrc-compositor-evidence-v3") {
    if (memory !== undefined) throw new Error("Historical manifest cannot authenticate memory evidence")
    return { status: "unknown", reason: "Historical raw allocation profiles and retained endpoints absent; never reconstructed" } as const
  }
  if (memory?.schema !== "playsrc-allocation-memory-v1" || !memory.main || !Array.isArray(memory.main.errors)) throw new Error("Memory evidence missing")
  const main = memory.main, artifact = main.profile
  let bytes: Buffer | undefined
  if (artifact) {
    if (!/^[0-9a-f]{64}$/u.test(artifact.sha256) || artifact.file !== `${artifact.sha256}.main.heapprofile`
      || !Number.isSafeInteger(artifact.bytes) || artifact.bytes < 1 || artifact.bytes > ALLOCATION_LIMITS.bytes) throw new Error("Allocation blob identity or byte bound invalid")
    const file = path.join(path.dirname(filename), artifact.file)
    if ((await stat(file)).size !== artifact.bytes) throw new Error("Allocation blob byte identity mismatch")
    bytes = await readFile(file)
    if (bytes.length !== artifact.bytes || createHash("sha256").update(bytes).digest("hex") !== artifact.sha256) throw new Error("Allocation blob hash identity mismatch")
  }
  const plan = await loadCapturePlan(filename, loaded.manifest)
  if (!plan || JSON.stringify(main.plan) !== JSON.stringify(ALLOCATION_PLAN)) throw new Error("Allocation effective capture plan missing or invalid")
  const cpu = await loadMainCpuEvidence(filename, loaded)
  const worker = await loadWorkerIncidents(filename, loaded)
  if (main.errors.length || !cpu?.profile || !loaded.manifest.complete) return { status: "incomplete", evidence: memory, estimatedAllocation: null } as const
  if (!bytes || bytes.length !== main.capturedBytes) throw new Error("Allocation profile missing or truncated")
  const { start, stop } = main.clock
  const window = loaded.analysis.window!
  if (main.clock.domain !== "Chromium monotonic microseconds" || start.length !== 2 || stop.length !== 2
    || [...start, ...stop].some(value => !Number.isFinite(value) || value < 0)
    || start[0]! > start[1]! || start[1]! > stop[0]! || stop[0]! > stop[1]!
    || start[1]! > window.startedMicroseconds || stop[0]! < window.endedMicroseconds) throw new Error("Allocation clock boundaries invalid")
  if (!main.context?.uniqueId || !Number.isSafeInteger(main.context.id) || !main.context.auxData?.isDefault
    || !main.frameId || main.context.auxData.frameId !== main.frameId
    || main.context.origin !== loaded.manifest.identity.origin) throw new Error("Allocation execution context invalid")
  for (const heap of [main.heapBefore, main.heapAfter]) {
    if (!heap || !Number.isFinite(heap.before) || !Number.isFinite(heap.after) || heap.before > heap.after
      || Object.values(heap.value).some(value => !Number.isFinite(value) || value < 0)) throw new Error("Main heap gauge invalid")
  }
  if (main.heapBefore!.before < start[1]! || main.heapBefore!.after > window.startedMicroseconds
    || main.heapAfter!.before < stop[1]!) throw new Error("Main heap gauge phase invalid")
  const snapshotTransport = memory.snapshotTransport ? summarizeSnapshotTransport(memory.snapshotTransport.before, memory.snapshotTransport.after) : null
  if (!snapshotTransport) throw new Error("Snapshot memory endpoints missing")
  if (snapshotTransport.before.at > loaded.probes.started || snapshotTransport.after.at < loaded.probes.ended) throw new Error("Snapshot memory endpoint phase invalid")
  for (const snapshot of [memory.processes?.before, memory.processes?.after]) {
    if (!snapshot || !Number.isSafeInteger(snapshot.startedAt) || !Number.isSafeInteger(snapshot.endedAt) || snapshot.endedAt < snapshot.startedAt
      || !Array.isArray(snapshot.processes) || snapshot.processes.length > 256
      || new Set(snapshot.processes.map(process => process.id)).size !== snapshot.processes.length) throw new Error("Process memory boundary invalid")
    for (const process of snapshot.processes) {
      if (!Number.isSafeInteger(process.id) || process.id < 1 || typeof process.type !== "string"
        || [process.residentBytes, process.privateBytes].some(value => value !== null && (!Number.isSafeInteger(value) || value < 0))) throw new Error("Process memory gauge invalid")
    }
    if (snapshot.residentBytes !== completeMemoryTotal(snapshot.processes, "residentBytes")
      || snapshot.privateBytes !== completeMemoryTotal(snapshot.processes, "privateBytes")) throw new Error("Process memory aggregate mismatch")
  }
  if (memory.processes.before.endedAt > memory.processes.after!.startedAt) throw new Error("Process memory phases overlap")
  const processMapping = (pid: number | undefined) => ({ pid: pid ?? null,
    before: memory.processes.before.processes.find(process => process.id === pid) ?? null,
    after: memory.processes.after!.processes.find(process => process.id === pid) ?? null })
  return { status: "complete", capturePlan: plan, allocationPlan: main.plan,
    main: { owner: cpu.evidence.source, executionContext: main.context, process: processMapping(window.pid), tid: window.tid,
      profile: artifact, clock: main.clock, estimatedAllocation: summarizeAllocationProfile(JSON.parse(bytes.toString("utf8"))),
      heapGauges: { before: main.heapBefore, after: main.heapAfter, scope: "CDP isolate heap usage, not process RSS or exclusive physical memory" } },
    workers: worker.analyses.map(analysis => ({ target: analysis.target, executionContextId: analysis.executionContextId,
      process: processMapping(analysis.clock.clocks[0]?.pid), tid: analysis.clock.clocks[0]?.tid ?? null,
      clock: analysis.clock, estimatedAllocation: null, reason: "Worker allocation sampler not requested" })),
    workerCpuInstrumentation: worker.workerInstrumentation,
    unsampledWorkerTargets: worker.unsampledTargets, snapshotTransport, processes: memory.processes,
    attribution: { processClock: "controller Unix epoch milliseconds; no assumed join to Chromium monotonic clock",
      processPhases: { before: "before-sampler-setup", after: "after-native-trace-drain-and-heap-readback" },
      processScope: "per-PID RSS gauges include native, JS, WASM and shared mappings; aggregate can double count shared pages; not exclusive owner bytes",
      nativeExclusiveBytes: null, gpuResourceBytes: null, sharedMappingBytes: null, wasmExclusiveBytes: null,
      conclusion: "Owner migration is not an allocation increase; main-only sampled estimates and post-sample aggregate RSS cannot establish a leak or regression" } } as const
}

export async function replayAllocationMemory(filename: string) {
  return loadAllocationMemoryEvidence(filename, await loadCompositorEvidence(filename))
}
if (import.meta.main) {
  if (process.argv.length !== 3) throw new Error("Usage: bun tools/playsrc/profile/allocation-memory-evidence.ts <sha256.manifest.json>")
  console.log(JSON.stringify(await replayAllocationMemory(process.argv[2]!), null, 2))
}
