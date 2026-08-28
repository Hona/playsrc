type Interval = { scene: string; input: string; lowerMilliseconds: number; upperMilliseconds: number | null; endCensored: boolean }

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
