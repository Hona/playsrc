/** Submission counters, RAF opportunities and native compositor presentation
 * are different clocks. Never infer a presentation from a RAF acknowledgement. */
export function deliveryTimeline(started: number, ended: number, timestamps: readonly number[]) {
  if (![started, ended].every(Number.isFinite) || ended <= started) throw new Error("Invalid delivery window")
  if (timestamps.some((at, index) => !Number.isFinite(at) || index > 0 && at < timestamps[index - 1]!)) throw new Error("Unordered delivery clock")
  const events = timestamps.filter(at => at >= started && at < ended)
  const boundaries = [started, ...events, ended]
  const gaps = boundaries.slice(1).map((at, index) => at - boundaries[index]!)
  const buckets = Array.from({ length: Math.ceil((ended - started) / 1000) }, (_, index) => {
    const from = started + index * 1000, to = Math.min(from + 1000, ended)
    return { started: from, ended: to, count: events.filter(at => at >= from && at < to).length }
  })
  return { count: events.length, perSecond: events.length * 1000 / (ended - started), buckets,
    zeroBuckets: buckets.filter(bucket => bucket.count === 0).length,
    initialGap: gaps[0]!, terminalGap: gaps.at(-1)!, maximumGapIncludingBoundaries: Math.max(...gaps), gapsIncludingBoundaries: gaps }
}

export function retainedDeliveryAttribution(measurement: any, report: any) {
  const { started, ended } = measurement
  const completed = deliveryTimeline(started, ended, measurement.frames.map((frame: any) => frame.at))
  const raf = deliveryTimeline(started, ended, measurement.presentationCallbacks)
  const observe = measurement.worker.filter((call: any) => call.kind === "observe" && call.finished !== undefined
    && call.started >= started && call.finished <= ended)
  const observeTransact = observe.reduce((sum: number, call: any) => sum + call.timings.transactMilliseconds, 0)
  return { completed, raf, compositor: report.compositor,
    observe: { completeInteriorCalls: observe.length, transactMilliseconds: observeTransact, windowFraction: observeTransact / (ended - started) },
    renderWork: report.frameWork,
    caution: "Worker service durations exclude straddling calls; overlapping queue waits must not be summed as CPU work. RAF acknowledgements are not compositor evidence." }
}

/** Read-only observer shared by ordinary and traced modes. No Worker wrappers,
 * app profile globals, requestAnimationFrame replacement or renderer hooks. */
export function installDeliveryObserver(host: any = globalThis) {
  let active = false, observer: MutationObserver | undefined, raf = 0
  let frames: Array<{ at: number; frame: number }> = [], opportunities: number[] = [], lifecycle: string[] = []
  let started = 0, firstFrame = 0, lastFrame = 0, missedPublications = 0
  const state = () => {
    const data = host.document.querySelector("main")?.dataset ?? {}
    return { tick: Number(data.snapshotTick), bots: Number(data.botCount), cameraPosition: data.cameraPosition,
      cameraAngles: data.cameraAngles, hud: data.hudProbe, botProbe: data.botProbe, performanceDetail: data.performanceDetail }
  }
  let before: ReturnType<typeof state>
  const changed = () => { if (active) lifecycle.push("visibility/focus changed") }
  host.addEventListener("blur", changed); host.document.addEventListener("visibilitychange", changed)
  const input = (event: any) => { if (active && (event.type !== "mousemove" || event.movementX || event.movementY)) lifecycle.push(`unexpected ${event.type}`) }
  for (const type of ["keydown", "pointerdown", "mousemove"]) host.document.addEventListener(type, input, { passive: true })
  const tick = () => { if (active) { opportunities.push(host.performance.now()); raf = host.requestAnimationFrame(tick) } }
  host.__playsrcDeliveryObserver = {
    start(at = host.performance.now()) {
      if (active) throw new Error("Delivery observer already active")
      const canvas = host.document.querySelector("canvas.world-canvas")
      if (!canvas) throw new Error("No gameplay surface")
      started = at; firstFrame = lastFrame = Number(canvas.dataset.displayFrame)
      before = state()
      frames = []; opportunities = []; lifecycle = []; missedPublications = 0; active = true
      observer = new host.MutationObserver(() => {
        const frame = Number(canvas.dataset.displayFrame)
        if (frame !== lastFrame) {
          if (frame < lastFrame) lifecycle.push("completed-frame counter reset")
          missedPublications += Math.max(0, frame - lastFrame - 1)
          frames.push({ at: host.performance.now(), frame }); lastFrame = frame
        }
      })
      observer!.observe(canvas, { attributes: true, attributeFilter: ["data-display-frame"] })
      raf = host.requestAnimationFrame(tick)
    },
    stop(at = host.performance.now()) {
      active = false; observer?.disconnect(); host.cancelAnimationFrame(raf)
      return { started, ended: at, firstFrame, lastFrame, before, after: state(), frames, raf: opportunities, lifecycle, missedPublications,
        rafClock: "callback delivery performance.now; not a presentation timestamp",
        compositor: null, compositorEvidence: "not measured by this observer" }
    },
  }
}
