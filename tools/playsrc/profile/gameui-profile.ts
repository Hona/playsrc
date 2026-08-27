import { reconstructCpuProfile, selectCpuWindow, type CpuWindow } from "./cpu-profile-time"

export type Distribution = Readonly<{
  count: number
  total: number
  mean: number
  p50: number
  p95: number
  p99: number
  max: number
}>

type CpuNode = Readonly<{
  id: number
  callFrame: Readonly<{ functionName: string; url: string; lineNumber: number; columnNumber: number }>
  children?: readonly number[]
}>

export type CpuProfile = Readonly<{
  nodes: readonly CpuNode[]
  samples?: readonly number[]
  timeDeltas?: readonly number[]
  startTime: number
  endTime: number
}>

export type TraceEvent = Readonly<{
  name?: string
  cat?: string
  ph?: string
  ts?: number
  dur?: number
  pid?: number
  tid?: number
  args?: Readonly<Record<string, unknown> & { data?: Readonly<Record<string, unknown>> }>
}>

const rounded = (value: number): number => Number(value.toFixed(3))

export function summarizeDistribution(values: readonly number[]): Distribution {
  const sorted = [...values].filter(Number.isFinite).sort((left, right) => left - right)
  const percentile = (fraction: number): number => sorted.length === 0 ? 0 : sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!
  const total = sorted.reduce((sum, value) => sum + value, 0)
  return Object.freeze({
    count: sorted.length,
    total: rounded(total),
    mean: rounded(sorted.length === 0 ? 0 : total / sorted.length),
    p50: rounded(percentile(0.5)),
    p95: rounded(percentile(0.95)),
    p99: rounded(percentile(0.99)),
    max: rounded(sorted.at(-1) ?? 0),
  })
}

function location(node: CpuNode): string {
  const raw = node.callFrame.url
  if (!raw) return "(runtime)"
  try {
    const pathname = decodeURIComponent(new URL(raw).pathname).replaceAll("\\", "/")
    const marker = pathname.lastIndexOf("/playsrc/")
    return marker >= 0 ? pathname.slice(marker + 9) : pathname
  } catch {
    return raw.replaceAll("\\", "/")
  }
}

const nodeKey = (node: CpuNode): string => `${node.callFrame.functionName}\0${location(node)}\0${node.callFrame.lineNumber}\0${node.callFrame.columnNumber}`

function moduleIdentity(value: string): string {
  const parts = value.split("/").filter(Boolean)
  if (parts[0] === "packages" && parts.length >= 3) return parts.slice(0, 3).join("/")
  if ((parts[0] === "games" || parts[0] === "apps") && parts.length >= 3) return parts.slice(0, 3).join("/")
  if (parts[0] === "tools" && parts.length >= 2) return parts.slice(0, 2).join("/")
  return value
}

function cpuRows(values: ReadonlyMap<string, { microseconds: number; samples: number; node: CpuNode }>, totalMicroseconds: number) {
  return Object.freeze([...values].map(([key, value]) => Object.freeze({
    function: value.node.callFrame.functionName || "(anonymous)",
    location: location(value.node),
    line: value.node.callFrame.lineNumber + 1,
    column: value.node.callFrame.columnNumber + 1,
    samples: value.samples,
    estimatedMilliseconds: rounded(value.microseconds / 1_000),
    percent: rounded(totalMicroseconds === 0 ? 0 : value.microseconds * 100 / totalMicroseconds),
    key,
  })).sort((left, right) => right.estimatedMilliseconds - left.estimatedMilliseconds).slice(0, 50))
}

export function summarizeCpuProfile(profile: CpuProfile, window?: CpuWindow) {
  const timeline = reconstructCpuProfile(profile)
  const { nodes, parents } = timeline
  const selected = selectCpuWindow(timeline, window)
  const self = new Map<string, { microseconds: number; samples: number; node: CpuNode }>()
  const inclusive = new Map<string, { microseconds: number; samples: number; node: CpuNode }>()
  const modules = new Map<string, { microseconds: number; samples: number }>()
  const edges = new Map<string, { caller: CpuNode; callee: CpuNode; microseconds: number; samples: number }>()
  const stacks = new Map<string, { frames: readonly string[]; microseconds: number; samples: number }>()
  const sampledMicroseconds = selected.estimatedMicroseconds
  const add = (target: typeof self, node: CpuNode, microseconds: number, samples: number) => {
    const key = nodeKey(node)
    const current = target.get(key) ?? { microseconds: 0, samples: 0, node }
    current.microseconds += microseconds
    current.samples += samples
    target.set(key, current)
  }
  for (const sample of selected.samples) {
    const microseconds = sample.estimatedMicroseconds
    const sampled = nodes.get(sample.nodeId)!
    add(self, sampled, microseconds, sample.samples)
    const module = moduleIdentity(location(sampled))
    const moduleValue = modules.get(module) ?? { microseconds: 0, samples: 0 }
    moduleValue.microseconds += microseconds
    moduleValue.samples += sample.samples
    modules.set(module, moduleValue)
    const admitted = new Set<string>()
    const chain: CpuNode[] = []
    let current: CpuNode | undefined = sampled
    while (current) {
      chain.push(current)
      const key = nodeKey(current)
      if (!admitted.has(key)) {
        add(inclusive, current, microseconds, sample.samples)
        admitted.add(key)
      }
      current = nodes.get(parents.get(current.id) ?? -1)
    }
    chain.reverse()
    for (let edge = 0; edge + 1 < chain.length; edge += 1) {
      const caller = chain[edge]!, callee = chain[edge + 1]!, key = `${nodeKey(caller)}\u0001${nodeKey(callee)}`
      const value = edges.get(key) ?? { caller, callee, microseconds: 0, samples: 0 }
      value.microseconds += microseconds
      value.samples += sample.samples
      edges.set(key, value)
    }
    const frames = chain.filter((node) => node.callFrame.functionName !== "(root)").slice(-12)
      .map((node) => `${node.callFrame.functionName || "(anonymous)"}@${location(node)}:${node.callFrame.lineNumber + 1}`)
    const stackKey = frames.join("\n")
    const stack = stacks.get(stackKey) ?? { frames: Object.freeze(frames), microseconds: 0, samples: 0 }
    stack.microseconds += microseconds
    stack.samples += sample.samples
    stacks.set(stackKey, stack)
  }
  return Object.freeze({
    schema: "playsrc-cpu-sampling-estimate-v2",
    attribution: "chronological sample-to-next-sample estimates; not exclusive CPU wall; no tail extrapolation",
    window: { startedMicroseconds: selected.startedMicroseconds, endedMicroseconds: selected.endedMicroseconds, endInclusive: window === undefined },
    profileWallMilliseconds: rounded((profile.endTime - profile.startTime) / 1_000),
    wallMilliseconds: rounded(selected.wallMicroseconds / 1_000),
    estimatedSampledMilliseconds: rounded(sampledMicroseconds / 1_000),
    unattributedMilliseconds: rounded(selected.unattributedMicroseconds / 1_000),
    leadingUnattributedMilliseconds: rounded(selected.leadingUnattributedMicroseconds / 1_000),
    trailingUnattributedMilliseconds: rounded(selected.trailingUnattributedMicroseconds / 1_000),
    unsampledProfileMilliseconds: rounded(selected.unsampledProfileMicroseconds / 1_000),
    outsideProfileMilliseconds: rounded(selected.outsideProfileMicroseconds / 1_000),
    signedElapsedMilliseconds: rounded(timeline.signedElapsedMicroseconds / 1_000),
    negativeDeltaCount: timeline.negativeDeltaCount,
    equalTimestampCount: timeline.equalTimestampCount,
    maximumReorderMicroseconds: timeline.maximumReorderMicroseconds,
    sampleCount: selected.sampleCount,
    profileSampleCount: timeline.rawSamples.length,
    topSelf: cpuRows(self, sampledMicroseconds),
    topInclusive: cpuRows(inclusive, sampledMicroseconds),
    topModules: Object.freeze([...modules].map(([module, value]) => Object.freeze({
      module,
      samples: value.samples,
      estimatedMilliseconds: rounded(value.microseconds / 1_000),
      percent: rounded(sampledMicroseconds === 0 ? 0 : value.microseconds * 100 / sampledMicroseconds),
    })).sort((left, right) => right.estimatedMilliseconds - left.estimatedMilliseconds).slice(0, 50)),
    topEdges: Object.freeze([...edges.values()].map((value) => Object.freeze({
      caller: `${value.caller.callFrame.functionName || "(anonymous)"}@${location(value.caller)}:${value.caller.callFrame.lineNumber + 1}`,
      callee: `${value.callee.callFrame.functionName || "(anonymous)"}@${location(value.callee)}:${value.callee.callFrame.lineNumber + 1}`,
      samples: value.samples,
      estimatedMilliseconds: rounded(value.microseconds / 1_000),
      percent: rounded(sampledMicroseconds === 0 ? 0 : value.microseconds * 100 / sampledMicroseconds),
    })).sort((left, right) => right.estimatedMilliseconds - left.estimatedMilliseconds).slice(0, 100)),
    topStacks: Object.freeze([...stacks.values()].map((value) => Object.freeze({
      frames: value.frames,
      samples: value.samples,
      estimatedMilliseconds: rounded(value.microseconds / 1_000),
      percent: rounded(sampledMicroseconds === 0 ? 0 : value.microseconds * 100 / sampledMicroseconds),
    })).sort((left, right) => right.estimatedMilliseconds - left.estimatedMilliseconds).slice(0, 100)),
  })
}

export function summarizeTrace(events: readonly TraceEvent[]) {
  const rendererThread = events.find((event) => event.ph === "M" && event.name === "thread_name" && event.args?.name === "CrRendererMain")
  const allComplete = events.filter((event) => event.ph === "X" && Number.isFinite(event.dur) && (event.dur ?? 0) >= 0)
  const complete = rendererThread
    ? allComplete.filter((event) => event.pid === rendererThread.pid && event.tid === rendererThread.tid)
    : allComplete
  const byName = new Map<string, number[]>()
  for (const event of complete) {
    const name = event.name || "(unnamed)"
    const values = byName.get(name) ?? []
    values.push((event.dur ?? 0) / 1_000)
    byName.set(name, values)
  }
  const categories = [...byName].map(([name, values]) => Object.freeze({ name, ...summarizeDistribution(values) }))
    .sort((left, right) => right.total - left.total)
  const topEvents = complete.map((event) => {
    const data = event.args?.data ?? {}
    return Object.freeze({
      name: event.name ?? "(unnamed)",
      category: event.cat ?? "",
      milliseconds: rounded((event.dur ?? 0) / 1_000),
      function: typeof data.functionName === "string" ? data.functionName : null,
      url: typeof data.url === "string" ? data.url : typeof data.scriptName === "string" ? data.scriptName : null,
    })
  }).sort((left, right) => right.milliseconds - left.milliseconds).slice(0, 100)
  const byFunction = new Map<string, { functionName: string; url: string; values: number[] }>()
  for (const event of complete.filter((value) => value.name === "FunctionCall" || value.name === "EvaluateScript")) {
    const data = event.args?.data ?? {}
    const functionName = typeof data.functionName === "string" ? data.functionName : event.name ?? "(anonymous)"
    const url = typeof data.url === "string" ? data.url : typeof data.scriptName === "string" ? data.scriptName : "(runtime)"
    const key = `${functionName}\0${url}`
    const value = byFunction.get(key) ?? { functionName, url, values: [] }
    value.values.push((event.dur ?? 0) / 1_000)
    byFunction.set(key, value)
  }
  return Object.freeze({
    eventCount: events.length,
    completeEventCount: allComplete.length,
    rendererMainCompleteEventCount: complete.length,
    rendererMainThread: rendererThread ? Object.freeze({ pid: rendererThread.pid, tid: rendererThread.tid }) : null,
    categories: Object.freeze(categories.slice(0, 100)),
    topEvents: Object.freeze(topEvents),
    topFunctions: Object.freeze([...byFunction.values()].map((value) => Object.freeze({
      function: value.functionName,
      url: value.url,
      ...summarizeDistribution(value.values),
    })).sort((left, right) => right.total - left.total).slice(0, 100)),
    longTasks: summarizeDistribution(complete.filter((event) => event.name === "RunTask" && (event.dur ?? 0) >= 50_000).map((event) => (event.dur ?? 0) / 1_000)),
  })
}

export function metricDelta(
  before: readonly Readonly<{ name: string; value: number }>[],
  after: readonly Readonly<{ name: string; value: number }>[],
) {
  const baseline = new Map(before.map((metric) => [metric.name, metric.value]))
  return Object.freeze(Object.fromEntries(after.map((metric) => [metric.name, rounded(metric.value - (baseline.get(metric.name) ?? 0))])))
}
