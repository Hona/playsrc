import { createHash } from "node:crypto"
import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { gunzipSync, gzipSync } from "node:zlib"
import type { CDPSession } from "@playwright/test"
import { TRACE_START, TRACE_END, activeGameplayTraceWindow, chromiumPresentationEventName, type ChromiumTraceEvent } from "./compositor-truth"

export const COMPOSITOR_TRACE_CATEGORIES = Object.freeze([
  "benchmark", "viz", "gpu", "cc", "renderer.scheduler", "toplevel", "blink.user_timing",
  "disabled-by-default-gpu.service", "disabled-by-default-gpu.dawn", "disabled-by-default-viz.debug",
  "devtools.timeline", "v8", "disabled-by-default-v8.gc",
])
export const TRACE_LIMITS = Object.freeze({ browserKilobytes: 128 * 1024, compressedBytes: 32 * 1024 * 1024, decodedBytes: 256 * 1024 * 1024, events: 1_000_000, probeBytes: 32 * 1024 * 1024 })
export type RawTraceEvent = ChromiumTraceEvent & { pid?: number; tid?: number; ph?: string; cat?: string }
export type TraceJoin = Readonly<{ kind: string; at: number; end?: number; detail?: unknown }>
export type TraceProbes = Readonly<{ started: number; ended: number; joins: readonly TraceJoin[]; dropped: number }>

const families = {
  display: /Display::|viz::Display/u,
  beginFrame: /BeginFrame|BeginMainFrame/u,
  submitCompositorFrame: /SubmitCompositorFrame|SubmitToReceiveCompositorFrame/u,
  presentation: /FramePresented|FrameDisplayed|PresentationFeedback/u,
  frameSink: /FrameSink|SurfaceFrame/u,
  gpuScheduler: /Scheduler::RunTask|GpuChannel|CommandBuffer/u,
  dawn: /Dawn|Queue::|MapAsync|APICreateRenderPipeline/u,
  swap: /SwapBuffers|WaitForPresentation|OnVSyncPresentation/u,
  raster: /Raster|Paint|Layout/u,
  backend: /Metal|ANGLE|D3D|IOSurface/u,
  scheduler: /Idle|Deadline|ShouldThrottle/u,
  displayVsync: /CVDisplayLink|DisplayLinkCallback|VSync/u,
  garbageCollection: /GC|GarbageCollect/u,
  devtools: /DevTools|Inspector|Screenshot/u,
  storageNavigation: /IndexedDB|IDB|Navigation/u,
} as const

export function compositorTraceWindow(events: readonly RawTraceEvent[], probes: TraceProbes) {
  const start = events.filter(event => event.name === TRACE_START)
  const end = events.filter(event => event.name === TRACE_END)
  if (start.length !== 1 || end.length !== 1 || !Number.isFinite(start[0]!.ts) || !Number.isFinite(end[0]!.ts)) throw new Error("Trace requires one exact start/end sample mark")
  const { startedMicroseconds, endedMicroseconds } = activeGameplayTraceWindow(events)
  if (start[0]!.pid !== end[0]!.pid || start[0]!.tid !== end[0]!.tid || endedMicroseconds <= startedMicroseconds
    || !Number.isFinite(probes.started) || !Number.isFinite(probes.ended) || probes.ended <= probes.started) throw new Error("Trace sample clock ownership is invalid")
  const offsetMicroseconds = startedMicroseconds - probes.started * 1_000
  const endErrorMicroseconds = endedMicroseconds - probes.ended * 1_000 - offsetMicroseconds
  if (Math.abs(endErrorMicroseconds) > 100) throw new Error("Trace sample clock anchors disagree")
  return { startedMicroseconds, endedMicroseconds, offsetMicroseconds, endErrorMicroseconds, pid: start[0]!.pid, tid: start[0]!.tid }
}

/** Event indices address the untouched raw array, not an independently sorted or filtered copy. */
export function analyzeCompositorEvidence(events: readonly RawTraceEvent[], probes: TraceProbes) {
  const issues: string[] = []
  let window: ReturnType<typeof compositorTraceWindow> | null = null
  try { window = compositorTraceWindow(events, probes) } catch (error) { issues.push(String(error)) }
  if (probes.dropped) issues.push(`${probes.dropped} browser probe records were dropped`)
  const eventName = chromiumPresentationEventName(events)
  if (!eventName) issues.push("No Chromium presentation event family was captured")
  const streams = new Map<string, Array<{ index: number; ts: number }>>()
  const coverage = Object.fromEntries(Object.keys(families).map(name => [name, { count: 0, names: new Set<string>() }]))
  const threads = new Map<string, string>()
  events.forEach((event, index) => {
    const key = `${event.pid ?? "unknown"}:${event.tid ?? "unknown"}`
    if (event.name === "thread_name") threads.set(key, String(event.args?.name ?? "unknown"))
    for (const [family, pattern] of Object.entries(families)) if (pattern.test(event.name ?? "")) {
      coverage[family]!.count += 1
      coverage[family]!.names.add(event.name!)
    }
    if (event.name !== eventName || !Number.isFinite(event.ts)) return
    const stream = streams.get(key) ?? []
    stream.push({ index, ts: event.ts! })
    streams.set(key, stream)
  })
  const incidents: Array<{
    stream: string; firstEvent: number; lastEvent: number; startedMicroseconds: number; endedMicroseconds: number;
    milliseconds: number; thresholds: number[]; scope: string; work: Array<{ event: number; overlapMicroseconds: number; thread: string }>;
    joins: Array<{ probe: number; startedMicroseconds: number; endedMicroseconds: number }>;
  }> = []
  let incidentCount = 0
  const thresholdCounts = { over50: 0, over100: 0, over250: 0 }
  for (const [stream, samples] of streams) {
    const sorted = samples.toSorted((left, right) => left.ts - right.ts)
    const unique = sorted.filter((event, index) => index === 0 || event.ts !== sorted[index - 1]!.ts)
    for (let index = 1; index < unique.length; index += 1) {
      const first = unique[index - 1]!, last = unique[index]!
      const milliseconds = (last.ts - first.ts) / 1_000
      if (milliseconds <= 50) continue
      incidentCount += 1
      thresholdCounts.over50 += 1
      if (milliseconds > 100) thresholdCounts.over100 += 1
      if (milliseconds > 250) thresholdCounts.over250 += 1
      if (incidents.length === 64 && milliseconds <= incidents.at(-1)!.milliseconds) continue
      incidents.push({ stream, firstEvent: first.index, lastEvent: last.index, startedMicroseconds: first.ts, endedMicroseconds: last.ts, milliseconds,
        thresholds: [50, 100, 250].filter(threshold => milliseconds > threshold),
        scope: !window ? "unknown" : first.ts >= window.startedMicroseconds && last.ts <= window.endedMicroseconds ? "sample"
          : last.ts < window.startedMicroseconds ? "before-sample" : first.ts > window.endedMicroseconds ? "after-sample" : "sample-boundary",
        work: [], joins: [] })
      incidents.sort((left, right) => right.milliseconds - left.milliseconds)
      if (incidents.length > 64) incidents.pop()
    }
  }
  incidents.sort((left, right) => right.milliseconds - left.milliseconds)
  // Bound derived detail, not raw evidence. Counts cover every incident; raw endpoints are never removed.
  for (const incident of incidents.slice(0, 64)) {
    events.forEach((event, index) => {
      if (!Number.isFinite(event.ts) || !Number.isFinite(event.dur) || event.dur! < 1_000) return
      const overlapMicroseconds = Math.min(incident.endedMicroseconds, event.ts! + event.dur!) - Math.max(incident.startedMicroseconds, event.ts!)
      if (overlapMicroseconds > 0) incident.work.push({ event: index, overlapMicroseconds, thread: threads.get(`${event.pid}:${event.tid}`) ?? `${event.pid}:${event.tid}` })
    })
    incident.work.sort((left, right) => right.overlapMicroseconds - left.overlapMicroseconds)
    incident.work.length = Math.min(32, incident.work.length)
    if (window) probes.joins.forEach((probe, index) => {
      const startedMicroseconds = probe.at * 1_000 + window!.offsetMicroseconds
      const endedMicroseconds = (probe.end ?? probe.at) * 1_000 + window!.offsetMicroseconds
      if (startedMicroseconds <= incident.endedMicroseconds && endedMicroseconds >= incident.startedMicroseconds) incident.joins.push({ probe: index, startedMicroseconds, endedMicroseconds })
    })
  }
  return { schema: "playsrc-compositor-analysis-v1", issues, window, eventName: eventName ?? null, eventCount: events.length,
    incidentCount, thresholdCounts, detailedIncidentLimit: 64, incidentDetailsTruncated: incidentCount > 64,
    coverage: Object.fromEntries(Object.entries(coverage).map(([name, value]) => [name, { count: value.count, names: [...value.names].sort() }])),
    incidents }
}

type BlobIdentity = Readonly<{ file: string; sha256: string; bytes: number }>
const digest = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")

async function immutableBlob(directory: string, bytes: Uint8Array, suffix: string): Promise<BlobIdentity> {
  const sha256 = digest(bytes)
  const file = `${sha256}.${suffix}`
  await mkdir(directory, { recursive: true })
  try { await writeFile(path.join(directory, file), bytes, { flag: "wx" }) }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error
    if (digest(await readFile(path.join(directory, file))) !== sha256) throw new Error("Existing immutable trace evidence is corrupt")
  }
  return { file, sha256, bytes: bytes.byteLength }
}

export function decodeRawTrace(bytes: Uint8Array, limit = TRACE_LIMITS.decodedBytes): RawTraceEvent[] {
  const raw = gunzipSync(bytes, { maxOutputLength: limit })
  const value = JSON.parse(raw.toString("utf8")) as { traceEvents?: RawTraceEvent[] }
  if (!Array.isArray(value.traceEvents) || value.traceEvents.length > TRACE_LIMITS.events
    || value.traceEvents.some(event => !event || typeof event !== "object" || Array.isArray(event))) throw new Error("Raw trace event bound or format is invalid")
  return value.traceEvents
}

/** Drain after Tracing.end, never send CDP payloads or write files during active gameplay. */
export async function drainTraceStream(cdp: Pick<CDPSession, "send">, stream: string, maximumBytes = TRACE_LIMITS.compressedBytes) {
  if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > TRACE_LIMITS.compressedBytes) throw new Error("Trace stream byte bound is invalid")
  const chunks: Buffer[] = []
  let bytes = 0
  let complete = false
  const deadline = Date.now() + 15_000
  const bounded = async <T>(promise: Promise<T>, milliseconds: number): Promise<T> => {
    let timer: ReturnType<typeof setTimeout> | undefined
    try { return await Promise.race([promise, new Promise<never>((_resolve, reject) => { timer = setTimeout(() => reject(new Error("Trace stream timed out")), milliseconds) })]) }
    finally { if (timer) clearTimeout(timer) }
  }
  try {
    while (Date.now() < deadline) {
      const result = await bounded(cdp.send("IO.read", { handle: stream, size: Math.min(256 * 1024, maximumBytes - bytes + 1) }), Math.max(1, deadline - Date.now()))
      const chunk = Buffer.from(result.data, result.base64Encoded ? "base64" : "utf8")
      if (bytes + chunk.length > maximumBytes) { chunks.push(chunk.subarray(0, maximumBytes - bytes)); bytes = maximumBytes; break }
      chunks.push(chunk); bytes += chunk.length
      if (result.eof) { complete = true; break }
    }
  } catch { complete = false }
  finally { await bounded(cdp.send("IO.close", { handle: stream }), 1_000).catch(() => { complete = false }) }
  return { bytes: Buffer.concat(chunks, bytes), complete }
}

export async function retainCompositorEvidence(options: Readonly<{
  directory: string; raw: Uint8Array; complete: boolean; dataLossOccurred: boolean;
  identity: Record<string, unknown>; probes: TraceProbes;
}>) {
  // Persist original bytes before parsing/analysis: malformed and overflow traces are evidence too.
  const errors: string[] = []
  const trace = await immutableBlob(options.directory, options.raw.subarray(0, TRACE_LIMITS.compressedBytes), "trace.json.gz")
  if (trace.bytes !== options.raw.byteLength) errors.push("Raw trace exceeded the compressed byte bound")
  let capturedProbes = options.probes
  let probeBytes = Buffer.from(JSON.stringify(capturedProbes))
  if (probeBytes.length > TRACE_LIMITS.probeBytes) {
    errors.push("Browser probes exceeded the byte bound; raw trace retained")
    capturedProbes = { started: options.probes.started, ended: options.probes.ended, joins: [], dropped: options.probes.dropped + options.probes.joins.length }
    probeBytes = Buffer.from(JSON.stringify(capturedProbes))
  }
  const probes = await immutableBlob(options.directory, gzipSync(probeBytes), "probes.json.gz")
  if (!options.complete) errors.push("Trace stream exceeded its byte/time bound")
  if (options.dataLossOccurred) errors.push("Chromium reported trace data loss")
  if (options.identity.sourceUnchanged === false) errors.push("Source changed during capture")
  let events: RawTraceEvent[] = []
  try { events = decodeRawTrace(options.raw.subarray(0, TRACE_LIMITS.compressedBytes)) } catch (error) { errors.push(String(error)) }
  const analysis = analyzeCompositorEvidence(events, capturedProbes)
  const manifest = { schema: "playsrc-compositor-evidence-v1", identity: options.identity, limits: TRACE_LIMITS,
    categories: COMPOSITOR_TRACE_CATEGORIES, trace, probes, complete: options.complete && !options.dataLossOccurred && !errors.length && !analysis.issues.length,
    errors, analysis }
  const artifact = await immutableBlob(options.directory, Buffer.from(JSON.stringify(manifest, null, 2)), "manifest.json")
  return { artifact, manifest, events }
}

export async function replayCompositorEvidence(filename: string) {
  if ((await stat(filename)).size > TRACE_LIMITS.probeBytes) throw new Error("Trace manifest exceeds byte bound")
  const bytes = await readFile(filename)
  if (path.basename(filename) !== `${digest(bytes)}.manifest.json`) throw new Error("Trace manifest identity mismatch")
  const manifest = JSON.parse(bytes.toString("utf8"))
  if (manifest.schema !== "playsrc-compositor-evidence-v1") throw new Error("Unsupported trace evidence schema")
  const blob = async (identity: BlobIdentity) => {
    if (!/^[0-9a-f]{64}\.(trace|probes)\.json\.gz$/u.test(identity.file) || !identity.file.startsWith(identity.sha256)) throw new Error("Trace blob identity is invalid")
    const file = path.join(path.dirname(filename), identity.file)
    const size = (await stat(file)).size
    if (size > TRACE_LIMITS.compressedBytes || size !== identity.bytes) throw new Error("Trace blob byte bound or identity mismatch")
    const value = await readFile(file)
    if (value.length !== identity.bytes || digest(value) !== identity.sha256) throw new Error("Trace blob identity mismatch")
    return value
  }
  const trace = await blob(manifest.trace)
  const probes = JSON.parse(gunzipSync(await blob(manifest.probes), { maxOutputLength: TRACE_LIMITS.probeBytes }).toString("utf8")) as TraceProbes
  let events: RawTraceEvent[] = []
  try { events = decodeRawTrace(trace) } catch (error) { if (manifest.complete) throw error }
  const analysis = analyzeCompositorEvidence(events, probes)
  if (JSON.stringify(analysis) !== JSON.stringify(manifest.analysis)) throw new Error("Trace replay does not match retained analysis")
  return { complete: manifest.complete, identity: manifest.identity, errors: manifest.errors, analysis }
}

if (import.meta.main) {
  const filename = process.argv[2]
  if (!filename) throw new Error("Usage: bun tools/playsrc/profile/compositor-evidence.ts <sha256.manifest.json>")
  console.log(JSON.stringify(await replayCompositorEvidence(filename), null, 2))
}
