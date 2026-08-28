type Interval = { scene: string; input: string; lowerMilliseconds: number; upperMilliseconds: number | null; endCensored: boolean }

export type SelectionPreparationOwner = "model" | "visible-world" | "particle"

/** Attribute complete preparation without subtracting native waits from the
 * input/Ready boundary. Summed native latency is not CPU time. */
export function selectionPreparation(measurement: any, owner: SelectionPreparationOwner) {
  const boundaries = { model: ["model-poses-ready", "pipeline-end"], "visible-world": ["pipeline-start", "visible-pipelines-ready"],
    particle: ["visible-pipelines-ready", "particle-pipelines-ready"] } as const
  const [startKind, endKind] = boundaries[owner]
  const entries = measurement.evidence.owners
  const start = entries.find((entry: any) => entry.kind === startKind), end = entries.find((entry: any) => entry.kind === endKind)
  if (!start || !end || end.at < start.at) throw new Error(`Complete ${owner} preparation ownership is required`)
  const native = measurement.evidence.gpuOperations.filter((operation: any) => operation.kind === "createRenderPipelineAsync" && operation.at >= start.at && operation.at < end.at)
  if (native.some((operation: any) => !Number.isFinite(operation.end) || operation.end < operation.at || operation.end > end.at)) throw new Error("Preparation published before recorded native readiness")
  const events = native.flatMap((operation: any) => [{ at: operation.at, delta: 1 }, { at: operation.end, delta: -1 }])
    .sort((a: any, b: any) => a.at - b.at || a.delta - b.delta)
  let active = 0, maximum = 0, covered = 0, previous = start.at
  for (const event of events) {
    if (active > 0) covered += event.at - previous
    active += event.delta; maximum = Math.max(maximum, active); previous = event.at
  }
  return { owner, wallMilliseconds: end.at - start.at, nativePipelines: native.length, maximumNativeInFlight: maximum,
    summedNativeMilliseconds: native.reduce((sum: number, operation: any) => sum + operation.end - operation.at, 0),
    nativeCoveredMilliseconds: covered }
}

export function compareSelectionIntervals(before: readonly Interval[], after: readonly Interval[]) {
  if (before.length !== 2 || after.length !== 2) throw new Error("Both trusted selection transitions are required")
  return before.map((prior, index) => {
    const next = after[index]!
    if (prior.scene !== next.scene || prior.input !== next.input || prior.endCensored || next.endCensored
      || prior.upperMilliseconds === null || next.upperMilliseconds === null) throw new Error("Matched complete native pixel intervals are required")
    const regression = next.lowerMilliseconds > prior.upperMilliseconds
    const provenReduction = next.upperMilliseconds < prior.lowerMilliseconds
    if (regression) throw new Error(`${next.scene} native latency regressed outside the capture uncertainty`)
    return { scene: next.scene, before: [prior.lowerMilliseconds, prior.upperMilliseconds], after: [next.lowerMilliseconds, next.upperMilliseconds],
      disposition: provenReduction ? "proven-reduction" : "overlapping-measurement-intervals",
      minimumReductionMilliseconds: provenReduction ? prior.lowerMilliseconds - next.upperMilliseconds : 0,
      remainingOver250Milliseconds: next.upperMilliseconds > 250 }
  })
}

export function selectionLoadingInputs(measurement: any, control: any) {
  const records = measurement.evidence.loadingIdentity
  const request = records?.find((entry: any) => entry.kind === "request")
  const resources = records?.find((entry: any) => entry.kind === "resource-input")
  if (!request || !resources || !Array.isArray(control.requests)) throw new Error("Exact loading controls were not retained")
  const objects = control.requests.filter((entry: any) => /^\/objects\/sha256\/[a-f0-9]{64}$/.test(entry.path))
  if (!objects.length || objects.some((entry: any) => entry.failed || entry.finished === undefined || entry.encodedBytes === undefined)) {
    throw new Error("Immutable object transfer controls are incomplete")
  }
  return { target: request.target, contentBuild: request.contentBuild, objects: request.objects, wasm: request.wasm,
    renderLevel: request.renderLevel, resources: { sha256: resources.sha256, byteLength: resources.byteLength },
    transfers: objects.map((entry: any) => ({ path: entry.path, method: entry.method, status: entry.status,
      encodedBytes: entry.encodedBytes, diskCache: entry.diskCache, serviceWorker: entry.serviceWorker,
      servedFromCache: entry.servedFromCache ?? false })).sort((a: any, b: any) => a.path.localeCompare(b.path)) }
}

export function selectionLoadingPressure(control: any) {
  const samples = control.pressure as Array<{ epoch: number; freeBytes: number; totalBytes: number; cpus: Record<string, number>[] }>
  if (!samples?.length) throw new Error("Loading host pressure was not retained")
  return samples.slice(1).map((next, index) => {
    const previous = samples[index]!
    if (next.cpus.length !== previous.cpus.length) throw new Error("Host CPU topology changed")
    const delta = next.cpus.map((cpu, index) => {
      const prior = previous.cpus[index]!, total = Object.keys(cpu).reduce((sum, key) => sum + cpu[key]! - prior[key]!, 0)
      return { total, busy: total - cpu.idle! + prior.idle! }
    })
    const total = delta.reduce((sum, cpu) => sum + cpu.total, 0)
    const counterFault = delta.some(cpu => !Number.isFinite(cpu.total) || cpu.total < 0 || cpu.busy < 0 || cpu.busy > cpu.total)
    return { startedEpoch: previous.epoch, endedEpoch: next.epoch, freeBytes: next.freeBytes, totalBytes: next.totalBytes,
      counterFault, busyFraction: !counterFault && total > 0 ? delta.reduce((sum, cpu) => sum + cpu.busy, 0) / total : null }
  })
}
