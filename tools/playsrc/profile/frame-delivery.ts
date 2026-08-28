import { summarizeFrameTimes } from "./profile-window"
import { summarizeDistribution } from "./gameui-profile"

/** Submission counters, RAF opportunities and native compositor presentation
 * are different clocks. Never infer a presentation from a RAF acknowledgement. */
export function deliveryTimeline(started: number, ended: number, timestamps: readonly number[]) {
  if (![started, ended].every(Number.isFinite) || ended <= started) throw new Error("Invalid delivery window")
  if (timestamps.some((at, index) => !Number.isFinite(at) || index > 0 && at < timestamps[index - 1]!)) throw new Error("Unordered delivery clock")
  const events = timestamps.filter(at => at >= started && at < ended)
  const boundaries = [started, ...events, ended]
  const gaps = boundaries.slice(1).map((at, index) => at - boundaries[index]!)
  const bucket = (width: number) => Array.from({ length: Math.ceil((ended - started) / width) }, (_, index) => {
    const from = started + index * width, to = Math.min(from + width, ended)
    return { started: from, ended: to, durationMilliseconds: to - from, full: to === from + width,
      count: events.filter(at => at >= from && at < to).length }
  })
  const buckets = bucket(1000), quarterSecondBuckets = bucket(250)
  const statistics = (values: number[]) => ({ ...summarizeFrameTimes(values), over500Milliseconds: values.filter(value => value > 500).length })
  return { count: events.length, perSecond: events.length * 1000 / (ended - started), buckets,
    quarterSecondBuckets, intervalStatistics: statistics(events.slice(1).map((at, index) => at - events[index]!)),
    statisticsIncludingBoundaries: statistics(gaps),
    zeroBuckets: buckets.filter(bucket => bucket.count === 0).length,
    initialGap: gaps[0]!, terminalGap: gaps.at(-1)!, maximumGapIncludingBoundaries: Math.max(...gaps), gapsIncludingBoundaries: gaps }
}

export function summarizeDeliveryMeasurement(measurement: any) {
  return {
    renderSubmissionElapsed: summarizeFrameTimes(measurement.frames.map((frame: any) => frame.detail.total)),
    modelPreparationLatency: summarizeDistribution(measurement.frames.map((frame: any) => frame.detail.models)),
    delivery: {
      completed: deliveryTimeline(measurement.started, measurement.ended, measurement.frames.map((frame: any) => frame.at)),
      raf: deliveryTimeline(measurement.started, measurement.ended, measurement.presentationCallbacks),
      scope: "Completed submissions and RAF callbacks are not compositor presentation. Per-completed-frame phase latency can start before the sample and can include waits; it is not CPU time or the full inter-frame interval.",
    },
  }
}

export function retainedDeliveryAttribution(measurement: any, report: { compositor: unknown; renderWork: unknown }) {
  const { started, ended } = measurement
  const completed = deliveryTimeline(started, ended, measurement.frames.map((frame: any) => frame.at))
  const raf = deliveryTimeline(started, ended, measurement.presentationCallbacks)
  const observe = measurement.worker.filter((call: any) => call.kind === "observe" && call.finished !== undefined
    && call.started >= started && call.finished <= ended)
  const observeTransact = observe.reduce((sum: number, call: any) => sum + call.timings.transactMilliseconds, 0)
  return { completed, raf, compositor: report.compositor,
    observe: { completeInteriorCalls: observe.length, transactMilliseconds: observeTransact, windowFraction: observeTransact / (ended - started) },
    renderWork: report.renderWork,
    caution: "Worker service durations exclude straddling calls; overlapping queue waits must not be summed as CPU work. RAF acknowledgements are not compositor evidence." }
}

export function compareDeliveryEvidence(ordinary: any, ordinaryBoundary: any, traced: any, tracedBoundary: any) {
  const equal = (left: unknown, right: unknown, name: string) => {
    if (JSON.stringify(left) !== JSON.stringify(right)) throw new Error(`Delivery comparison ${name} differs`)
  }
  if (ordinary.mode !== "ordinary" || !["traced", "presentation"].includes(traced.mode)) throw new Error("Expected ordinary/traced or presentation observations")
  if (!/^[a-f0-9]{40}$/.test(ordinary.applicationCommit) || !/^[a-f0-9]{64}$/.test(ordinary.sourceFingerprint)) throw new Error("Missing comparison source identity")
  for (const [run, boundary] of [[ordinary, ordinaryBoundary], [traced, tracedBoundary]]) {
    equal(run.applicationCommit, boundary.applicationCommit, "boundary commit")
    equal(run.sourceFingerprint, boundary.sourceFingerprint, "boundary fingerprint")
    if (!boundary.browserVersion || !boundary.boundary.viewport || !boundary.boundary.state || !boundary.configuration) throw new Error("Missing comparison boundary")
  }
  for (const name of ["applicationCommit", "sourceFingerprint"]) equal(ordinary[name], traced[name], name)
  for (const name of ["browserVersion", "capturePlan"]) equal(ordinaryBoundary[name], tracedBoundary[name], name)
  for (const name of ["viewport", "userAgent", "storage"]) equal(ordinaryBoundary.boundary[name], tracedBoundary.boundary[name], name)
  for (const name of ["cameraPosition", "cameraYaw", "cameraPitch", "cameraVerticalFov", "cameraNear", "cameraFar", "hudProbe", "localMatchSettings"]) {
    equal(ordinaryBoundary.boundary.state[name], tracedBoundary.boundary.state[name], name)
  }
  const configuration = (value: any) => {
    const { assetOrigin, ...rest } = value
    const url = new URL(assetOrigin)
    if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password || url.pathname !== "/" || url.search || url.hash) throw new Error("Comparison requires normal loopback assets")
    return rest
  }
  equal(configuration(ordinaryBoundary.configuration), configuration(tracedBoundary.configuration), "configuration/content/quality")
  equal(ordinaryBoundary.boundary.instrumentation, { app: false, frame: false }, "ordinary instrumentation")
  equal(tracedBoundary.boundary.instrumentation, traced.mode === "presentation" ? { app: false, frame: false } : { app: true, frame: true }, "traced instrumentation")
  const roster = (value: string) => {
    const actors = value.split("|").map(bot => bot.split(":").slice(0, 3).join(":"))
    if (actors.length !== 15 || new Set(actors.map(actor => actor.split(":")[0])).size !== 15) throw new Error("Incomplete active bot roster")
    return actors
  }
  equal(roster(ordinary.sample.before.botProbe), roster(traced.sample.before.botProbe), "bot identity/team/class roster")
  for (const run of [ordinary, traced]) {
    const sample = run.sample, elapsed = sample.ended - sample.started
    if (elapsed < 5000 || elapsed > 10000 || sample.missedPublications || sample.lifecycle.length
      || sample.before.bots !== 15 || sample.after.bots !== 15 || sample.lastFrame - sample.firstFrame !== sample.frames.length) throw new Error("Incomplete or changed comparison window")
    if (!run.nativeAdmission?.length || run.nativeAdmission.some((record: any) => {
      const native = record.native
      return !native || native.desktop.state !== 0 || native.desktop.flags !== 1 || native.desktop.protocol !== 0
        || native.desktop.processSessionId !== native.desktop.consoleSessionId || native.windows.length !== 1
        || native.windows[0].minimized || !native.windows[0].visible || native.foreground !== native.windows[0].id
    })) throw new Error("Comparison lacks native foreground/unlocked evidence")
  }
  const summary = (run: any) => ({ completed: deliveryTimeline(run.sample.started, run.sample.ended, run.sample.frames.map((frame: any) => frame.at)),
    raf: deliveryTimeline(run.sample.started, run.sample.ended, run.sample.raf),
    ticks: run.sample.after.tick - run.sample.before.tick, initialTick: run.sample.before.tick, compositor: null })
  return { applicationCommit: ordinary.applicationCommit, ordinary: summary(ordinary), traced: summary(traced),
    note: "Matched source, browser, displayed resolution, content, persisted options, player camera and active roster/input policy. Initial simulation ticks and bot positions are outcomes, not frozen. Compositor presentation requires the independent native trace." }
}

/** Read-only observer shared by ordinary and traced modes. No Worker wrappers,
 * app profile globals, requestAnimationFrame replacement or renderer hooks. */
export function installDeliveryObserver(host: any = globalThis) {
  let active = false, observer: MutationObserver | undefined, raf = 0
  let frames: Array<{ at: number; frame: number; cameraPosition?: string; phaseFrame?: number; performance?: string;
    preparedRevision?: string; viewRevision?: string; snapRevision?: string }> = [], opportunities: number[] = [], lifecycle: string[] = []
  let started = 0, firstFrame = 0, lastFrame = 0, missedPublications = 0
  const state = () => {
    const data = host.document.querySelector("main")?.dataset ?? {}
    return { tick: Number(data.snapshotTick), bots: Number(data.botCount), cameraPosition: data.cameraPosition,
      cameraYaw: data.cameraYaw, cameraPitch: data.cameraPitch, cameraVerticalFov: data.cameraVerticalFov,
      hud: data.hudProbe, botProbe: data.botProbe, performanceDetail: data.performanceDetail }
  }
  let before: ReturnType<typeof state>
  let movementInput: { at: number; cameraPosition?: string } | null = null
  const changed = () => { if (active) lifecycle.push("visibility/focus changed") }
  host.addEventListener("blur", changed); host.document.addEventListener("visibilitychange", changed)
  const input = (event: any) => {
    if (event.type === "keydown" && event.code === "KeyW") movementInput = { at: host.performance.now(), cameraPosition: state().cameraPosition }
    if (active && (event.type !== "mousemove" || event.movementX || event.movementY)) lifecycle.push(`unexpected ${event.type}`)
  }
  for (const type of ["keydown", "pointerdown", "mousemove"]) host.document.addEventListener(type, input, { passive: true })
  const tick = () => { if (active) { opportunities.push(host.performance.now()); raf = host.requestAnimationFrame(tick) } }
  host.__playsrcDeliveryObserver = {
    start(at = host.performance.now()) {
      if (active) throw new Error("Delivery observer already active")
      const canvas = host.document.querySelector("canvas.world-canvas")
      if (!canvas) throw new Error("No gameplay surface")
      started = at; firstFrame = lastFrame = Number(canvas.dataset.displayFrame)
      host.__playsrcDeliveryRpc?.start(at)
      before = state()
      frames = []; opportunities = []; lifecycle = []; missedPublications = 0; active = true
      observer = new host.MutationObserver(() => {
        const frame = Number(canvas.dataset.displayFrame)
        if (frame !== lastFrame) {
          if (frame < lastFrame) lifecycle.push("completed-frame counter reset")
          missedPublications += Math.max(0, frame - lastFrame - 1)
          const data = host.document.querySelector("main")?.dataset
          frames.push({ at: host.performance.now(), frame, cameraPosition: data?.cameraPosition,
            preparedRevision: canvas.dataset.displayPreparedRevision, viewRevision: canvas.dataset.displayViewRevision, snapRevision: canvas.dataset.displaySnapRevision,
            ...(data?.performance ? { phaseFrame: Number(data.displayFrame), performance: data.performance } : {}) }); lastFrame = frame
        }
      })
      observer!.observe(canvas, { attributes: true, attributeFilter: ["data-display-frame"] })
      raf = host.requestAnimationFrame(tick)
    },
    stop(at = host.performance.now()) {
      active = false; observer?.disconnect(); host.cancelAnimationFrame(raf)
      const response = movementInput && frames.find(frame => frame.cameraPosition !== movementInput!.cameraPosition)
      return { started, ended: at, firstFrame, lastFrame, before, after: state(), frames, raf: opportunities, lifecycle, missedPublications,
        ...(host.__playsrcDeliveryRpc ? { rpc: host.__playsrcDeliveryRpc.stop() } : {}),
        movementInput, inputToChangedSubmissionMilliseconds: response && movementInput ? response.at - movementInput.at : null,
        inputCensoredMilliseconds: !response && movementInput ? at - movementInput.at : null,
        inputScope: "DOM input delivery to changed completed-frame camera publication, not physical input-to-photon latency",
        rafClock: "callback delivery performance.now; not a presentation timestamp",
        compositor: null, compositorEvidence: "not measured by this observer" }
    },
  }
}
