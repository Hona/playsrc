import { deliveryTimeline } from "./frame-delivery"

/** Keep complete intervals and their intersection. A straddling request is not
 * absent, and queue/response wall time must never be added as exclusive CPU. */
export function sustainedFreezes(sample: any, from: number, to: number, timestamps: readonly number[], workers: readonly any[] = []) {
  const delivery = deliveryTimeline(from, to, timestamps)
  const events = timestamps.filter(at => at >= from && at < to)
  const bounds = [from, ...events, to]
  const intervals = bounds.slice(1).map((end, i) => ({ start: bounds[i]!, end, milliseconds: end - bounds[i]!,
    censoredStart: i === 0, censoredEnd: i === bounds.length - 2 }))
  const overlaps = (start: number, end: number, left: number, right: number) => Number.isFinite(start) && Number.isFinite(end) && start < right && end > left
  const workerTasks = workers.flatMap(capture => {
    const execution = capture.execution
    if (!execution || !Number.isFinite(sample.timeOrigin) || !Number.isFinite(execution.timeOrigin)) return []
    const offset = execution.timeOrigin - sample.timeOrigin
    return execution.tasks.map((task: any) => ({ ...task, targetId: capture.target.targetId, start: task.started + offset,
      end: task.finished === null ? to : task.finished + offset, censoredEnd: task.finished === null,
      observes: (task.observes ?? []).map((span: any) => ({ ...span, start: span.started + offset, end: span.finished + offset })),
      handoffs: task.responses.map((response: any) => ({ requestId: response.requestId, kind: response.kind, transport: response.transport,
        start: response.started + offset, end: response.finished === null ? to : response.finished + offset, censoredEnd: response.finished === null })) }))
  })
  const incidents = intervals.filter(interval => interval.milliseconds >= 500).map(interval => ({ ...interval,
    requests: sample.rpc.records.filter((call: any) => overlaps(call.sent, call.received, interval.start, interval.end)).map((call: any) => ({ ...call,
      overlapMilliseconds: Math.min(call.received, interval.end) - Math.max(call.sent, interval.start),
      straddlesWindow: call.sent < from || call.received > to })),
    unfinishedRequests: (sample.rpc.pending ?? []).filter((call: any) => call.sent < interval.end),
    workerTasks: workerTasks.filter(task => overlaps(task.start, task.end, interval.start, interval.end)),
    input: sample.inputs.filter((input: any) => overlaps(input.at, input.completedAt ?? to, interval.start, interval.end)),
  }))
  return { delivery,
    thresholds: [250, 500, 1000, 1500].map(threshold => {
      const gaps = intervals.filter(gap => gap.milliseconds >= threshold)
      return { thresholdMilliseconds: threshold, count: gaps.length, censored: gaps.filter(gap => gap.censoredStart || gap.censoredEnd).length,
        maximumMilliseconds: Math.max(0, ...gaps.map(gap => gap.milliseconds)),
        qualifyingGapMilliseconds: gaps.reduce((sum, gap) => sum + gap.milliseconds, 0),
        excessOverThresholdMilliseconds: gaps.reduce((sum, gap) => sum + gap.milliseconds - threshold, 0) }
    }), incidents,
    scope: "Qualifying gap/excess durations are wall-time freeze metrics, not CPU time or dropped-frame estimates. Whole requests and exact late Worker observe/handoff spans are retained including boundaries. Co-occurrence alone is not causal proof; absent late Worker tasks outside the detailed sample do not mean idle." }
}
