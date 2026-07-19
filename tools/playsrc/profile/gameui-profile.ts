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
  args?: Readonly<{ data?: Readonly<Record<string, unknown>> }>
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

function cpuRows(values: ReadonlyMap<string, { microseconds: number; samples: number; node: CpuNode }>, totalMicroseconds: number) {
  return Object.freeze([...values].map(([key, value]) => Object.freeze({
    function: value.node.callFrame.functionName || "(anonymous)",
    location: location(value.node),
    line: value.node.callFrame.lineNumber + 1,
    column: value.node.callFrame.columnNumber + 1,
    samples: value.samples,
    milliseconds: rounded(value.microseconds / 1_000),
    percent: rounded(totalMicroseconds === 0 ? 0 : value.microseconds * 100 / totalMicroseconds),
    key,
  })).sort((left, right) => right.milliseconds - left.milliseconds).slice(0, 50))
}

export function summarizeCpuProfile(profile: CpuProfile) {
  const nodes = new Map(profile.nodes.map((node) => [node.id, node]))
  const parents = new Map<number, number>()
  for (const node of profile.nodes) for (const child of node.children ?? []) parents.set(child, node.id)
  const self = new Map<string, { microseconds: number; samples: number; node: CpuNode }>()
  const inclusive = new Map<string, { microseconds: number; samples: number; node: CpuNode }>()
  const samples = profile.samples ?? []
  const deltas = profile.timeDeltas ?? []
  let sampledMicroseconds = 0
  const add = (target: typeof self, node: CpuNode, microseconds: number) => {
    const key = `${node.callFrame.functionName}\0${location(node)}\0${node.callFrame.lineNumber}\0${node.callFrame.columnNumber}`
    const current = target.get(key) ?? { microseconds: 0, samples: 0, node }
    current.microseconds += microseconds
    current.samples += 1
    target.set(key, current)
  }
  for (let index = 0; index < samples.length; index += 1) {
    const microseconds = Math.max(0, deltas[index] ?? 0)
    const sampled = nodes.get(samples[index]!)
    if (!sampled || microseconds === 0) continue
    sampledMicroseconds += microseconds
    add(self, sampled, microseconds)
    const admitted = new Set<string>()
    let current: CpuNode | undefined = sampled
    while (current) {
      const key = `${current.callFrame.functionName}\0${location(current)}\0${current.callFrame.lineNumber}\0${current.callFrame.columnNumber}`
      if (!admitted.has(key)) {
        add(inclusive, current, microseconds)
        admitted.add(key)
      }
      current = nodes.get(parents.get(current.id) ?? -1)
    }
  }
  return Object.freeze({
    wallMilliseconds: rounded((profile.endTime - profile.startTime) / 1_000),
    sampledMilliseconds: rounded(sampledMicroseconds / 1_000),
    sampleCount: samples.length,
    topSelf: cpuRows(self, sampledMicroseconds),
    topInclusive: cpuRows(inclusive, sampledMicroseconds),
  })
}

export function summarizeTrace(events: readonly TraceEvent[]) {
  const complete = events.filter((event) => event.ph === "X" && Number.isFinite(event.dur) && (event.dur ?? 0) >= 0)
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
  return Object.freeze({
    eventCount: events.length,
    completeEventCount: complete.length,
    categories: Object.freeze(categories.slice(0, 100)),
    topEvents: Object.freeze(topEvents),
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
