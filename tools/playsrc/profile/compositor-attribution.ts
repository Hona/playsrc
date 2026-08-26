import type { RawTraceEvent, TraceProbes, analyzeCompositorEvidence } from "./compositor-evidence"
import { summarizeFrameTimes } from "./profile-window"

type Analysis = ReturnType<typeof analyzeCompositorEvidence>
type Span = { event: number; start: number; end: number }
const threadKey = (event: RawTraceEvent) => `${event.pid ?? "unknown"}:${event.tid ?? "unknown"}`
const finite = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value)
const object = (value: unknown): Record<string, any> => value && typeof value === "object" && !Array.isArray(value) ? value : {}

/** Wall-clock coverage, not CPU time: nested native slices must never be added together. */
function coverage(spans: readonly Span[], start: number, end: number) {
  let cursor = start, covered = 0, longestUnobserved = 0
  for (const span of spans.toSorted((a, b) => a.start - b.start || a.end - b.end)) {
    const left = Math.max(start, span.start), right = Math.min(end, span.end)
    if (right <= left) continue
    longestUnobserved = Math.max(longestUnobserved, left - cursor)
    covered += Math.max(0, right - Math.max(cursor, left))
    cursor = Math.max(cursor, right)
  }
  longestUnobserved = Math.max(longestUnobserved, end - cursor)
  return { coveredMicroseconds: covered, unobservedMicroseconds: end - start - covered, longestUnobservedMicroseconds: longestUnobserved }
}

/** Derived offline only. The immutable v1 analysis and raw event indices remain unchanged. */
export function attributeCompositorEvidence(events: readonly RawTraceEvent[], probes: TraceProbes, analysis: Analysis) {
  const names = new Map<string, string>(), processes = new Map<number, string>()
  const spans = new Map<string, Span[]>(), presentations = new Map<string, number[]>()
  const posts = new Map<string, number[]>(), handles: number[] = []
  const deliveries = new Map<string, number>()
  const messageKey = (event: RawTraceEvent) => {
    const id = event.args?.data?.traceId
    // Do not coerce IDs: numeric values outside JSON's exact integer range cannot be joined.
    return event.pid !== undefined && (typeof id === "string" || Number.isSafeInteger(id)) ? JSON.stringify([event.pid, typeof id, id]) : null
  }
  events.forEach((event, index) => {
    const key = threadKey(event)
    if (event.name === "thread_name") names.set(key, String(event.args?.name ?? "unknown"))
    if (event.name === "process_name" && event.pid !== undefined) processes.set(event.pid, String(event.args?.name ?? "unknown"))
    if (event.ph === "X" && finite(event.ts) && finite(event.dur) && event.dur > 0) {
      const list = spans.get(key) ?? []; list.push({ event: index, start: event.ts, end: event.ts + event.dur }); spans.set(key, list)
    }
    if (event.name === analysis.eventName && finite(event.ts)) {
      const list = presentations.get(key) ?? []; list.push(event.ts); presentations.set(key, list)
    }
    if (event.name === "SchedulePostMessage") {
      const id = messageKey(event)
      if (id) { const list = posts.get(id) ?? []; list.push(index); posts.set(id, list) }
    }
    if (event.name === "HandlePostMessage") {
      handles.push(index)
      const id = messageKey(event)
      if (id) deliveries.set(id, (deliveries.get(id) ?? 0) + 1)
    }
  })
  const owner = (event: RawTraceEvent) => ({ pid: event.pid ?? null, tid: event.tid ?? null,
    process: processes.get(event.pid!) ?? null, thread: names.get(threadKey(event)) ?? null })
  const describe = (index: number) => {
    const event = events[index]!, data = object(event.args?.data)
    return { event: index, name: event.name ?? null, category: event.cat ?? null, phase: event.ph ?? null,
      timestampMicroseconds: event.ts ?? null, durationMicroseconds: event.dur ?? null, owner: owner(event),
      source: { file: event.args?.src_file ?? null, function: event.args?.src_func ?? data.functionName ?? null,
        url: data.url ?? null, scriptId: data.scriptId ?? null, line: data.lineNumber ?? null, column: data.columnNumber ?? null },
      args: event.args ?? null }
  }
  const streams = [...presentations].map(([thread, times]) => {
    const sorted = [...new Set(times)].sort((a, b) => a - b)
    const scopes = Object.fromEntries(["sample", "before-sample", "after-sample", "sample-boundary", "unknown"].map(scope => [scope, { intervals: 0, over50: 0, over100: 0, over250: 0 }]))
    const gameplay: number[] = []
    for (let i = 1; i < sorted.length; i++) {
      const start = sorted[i - 1]!, end = sorted[i]!, window = analysis.window
      const scope = !window ? "unknown" : start >= window.startedMicroseconds && end <= window.endedMicroseconds ? "sample"
        : end < window.startedMicroseconds ? "before-sample" : start > window.endedMicroseconds ? "after-sample" : "sample-boundary"
      const milliseconds = (end - start) / 1000, count = scopes[scope]!
      count.intervals++; if (milliseconds > 50) count.over50++; if (milliseconds > 100) count.over100++; if (milliseconds > 250) count.over250++
      if (scope === "sample") gameplay.push(milliseconds)
    }
    return { thread, scopes, gameplayIntervals: gameplay.length ? summarizeFrameTimes(gameplay) : null }
  })
  const incidents = analysis.incidents.map(incident => {
    const start = incident.startedMicroseconds, end = incident.endedMicroseconds
    const threads = [...spans].flatMap(([key, all]) => {
      const overlapping = all.filter(span => span.start < end && span.end > start)
      if (!overlapping.length) return []
      const work = overlapping.toSorted((a, b) => Math.min(end, b.end) - Math.max(start, b.start) - (Math.min(end, a.end) - Math.max(start, a.start)))
      return [{ key, owner: owner(events[overlapping[0]!.event]!), ...coverage(overlapping, start, end),
        sliceCount: overlapping.length, detailTruncated: work.length > 12,
        work: work.slice(0, 12).map(span => ({ ...describe(span.event), overlapMicroseconds: Math.min(end, span.end) - Math.max(start, span.start) })) }]
    })
    const dependencies = handles.flatMap(index => {
      const event = events[index]!
      if (!finite(event.ts) || event.ts > end || event.ts + (event.dur ?? 0) < start) return []
      const key = messageKey(event), candidates = key ? posts.get(key) ?? [] : []
      const eligible = candidates.filter(post => finite(events[post]!.ts) && events[post]!.ts! <= event.ts!)
      // Ambiguous/reused IDs are evidence gaps, not permission to guess the nearest task.
      const post = candidates.length === 1 && eligible.length === 1 && deliveries.get(key!) === 1 ? eligible[0]! : null
      return [{ kind: "postMessage" as const, status: post === null ? "unmatched-or-ambiguous" : "matched-trace-id",
        scheduled: post === null ? null : describe(post), handled: describe(index),
        scheduleToHandleMicroseconds: post === null ? null : event.ts! - events[post]!.ts!,
        interpretation: "Dispatch latency includes transport and scheduling; it is not measured queue wait or proof of presentation blocking." }]
    })
    const joined = incident.joins.map(join => {
      const probe = probes.joins[join.probe]!, detail = object(probe.detail)
      const gpu = probe.kind === "gpu"
      const synchronousEnd = gpu && finite(detail.returned) && detail.returned >= probe.at ? detail.returned * 1000 + analysis.window!.offsetMicroseconds : null
      const observedEnd = gpu ? detail.end : probe.kind === "worker" ? detail.finished : probe.end
      const rightCensored = (gpu || probe.kind === "worker") && !finite(observedEnd)
      return { ...join, kind: probe.kind, phase: detail.phase ?? null,
        timing: gpu ? "native-call-and-optional-promise" : probe.kind === "worker" ? "request-round-trip" : probe.end === undefined ? "point-observation" : "observed-interval",
        rightCensored, observedEndMicroseconds: rightCensored ? null : finite(observedEnd) ? observedEnd * 1000 + analysis.window!.offsetMicroseconds : join.startedMicroseconds,
        synchronousEndMicroseconds: synchronousEnd,
        synchronousMicroseconds: synchronousEnd === null ? null : synchronousEnd - join.startedMicroseconds,
        promiseSettled: gpu && ["mapAsync", "onSubmittedWorkDone", "createRenderPipelineAsync"].includes(detail.kind) ? finite(detail.end) : null,
        detail }
    })
    return { firstEvent: incident.firstEvent, lastEvent: incident.lastEvent, stream: incident.stream, scope: incident.scope,
      startedMicroseconds: start, endedMicroseconds: end, milliseconds: incident.milliseconds,
      verdict: "unexplained-presentation-gap", criticalPath: null,
      threads, dependencyCount: dependencies.length, dependencies: dependencies.slice(0, 128), dependenciesTruncated: dependencies.length > 128,
      probeCount: joined.length, probes: joined.slice(0, 256), probesTruncated: joined.length > 256,
      limitations: [
        "Overlapping slices measure wall coverage, not CPU utilization, causality, or additive thread costs; uncovered time is not proven idle.",
        "Message trace IDs establish dispatch edges only; no verified chain links application frame/resource IDs to this native presentation pair.",
        "GPU call return, asynchronous settlement and native Dawn/ANGLE slices are not GPU execution durations or a proven wait dependency.",
        "Right-censored probe join ends are capture bounds, not observed completion timestamps.",
        "Worker/renderer/PCF/collision/NAV phase totals and completed-frame pass totals have no stage clock anchors; no stage timestamps are reconstructed.",
        "Model palette ownership and VGUI layout ownership require recorded resource/source evidence; generic uploads and Layout slices do not establish them.",
        "Native presentation timestamps do not explain OS display scheduling or physical scanout.",
      ] }
  })
  return { schema: "playsrc-compositor-attribution-v1", streams, incidents,
    detailLimits: { incidents: analysis.detailedIncidentLimit, slicesPerThread: 12, dependenciesPerIncident: 128, probesPerIncident: 256 },
    incidentDetailsTruncated: analysis.incidentDetailsTruncated,
    durationSupport: "Complete (X) slices only; other phases remain in raw evidence and are not silently assigned durations.",
    sourceOwnership: "Only captured source locations are reported, under the retained build identity; no mapping to the current checkout is inferred." }
}
