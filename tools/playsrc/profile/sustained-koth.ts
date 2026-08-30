import { deliveryTimeline } from "./frame-delivery"
import { summarizeFrameTimes } from "./profile-window"

export const SUSTAINED_KOTH = Object.freeze({ soakMilliseconds: 90_000, sampleMilliseconds: 6_000, extractionMilliseconds: 15_000, minimumBrowserMilliseconds: 165_000, allocationAccounting: "late-detailed-only" })
export function requireSustainedBudget(remaining: number) {
  const required = SUSTAINED_KOTH.soakMilliseconds + SUSTAINED_KOTH.sampleMilliseconds + SUSTAINED_KOTH.extractionMilliseconds
  if (!Number.isFinite(remaining) || remaining < required) throw new Error(`Sustained KOTH needs ${required}ms after natural active-round admission; only ${remaining}ms remain. Never shorten the90s soak.`)
}
export function sustainedKothTarget(value: string | undefined) {
  if (value !== "koth_harvest_final" && value !== "koth_viaduct") throw new Error("Sustained KOTH requires configured Harvest or Viaduct")
  return value
}

/** Lightweight bounded records, not a renderer/Worker wrapper or a GC trigger. */
export function installSustainedObservation(host: any = globalThis) {
  // Includes the full175s cap at the admitted165Hz display, not just60Hz RAF.
  const limit = 32768
  let active = false, used = false, started = 0, ended = 0, dropped = 0, raf = 0
  const frames: any[] = [], callbacks: number[] = [], ticks: any[] = [], inputs: any[] = [], lifecycle: any[] = []
  let observer: MutationObserver | undefined, lastFrame = 0, lastTick = 0, missedFrames = 0
  const state = () => host.document.querySelector("main")?.dataset ?? {}
  const append = (records: any[], value: any) => { if (records.length < limit) records.push(value); else dropped++ }
  const changed = (event: any) => { if (active) append(lifecycle, { at: host.performance.now(), type: event.type, visible: host.document.visibilityState, focused: host.document.hasFocus() }) }
  host.addEventListener("blur", changed); host.addEventListener("resize", changed); host.document.addEventListener("visibilitychange", changed)
  const input = (event: any) => {
    if (!active) return
    append(inputs, { at: host.performance.now(), type: event.type, code: event.code, trusted: event.isTrusted, camera: state().cameraPosition, completedAt: null })
  }
  host.document.addEventListener("keydown", input, { passive: true })
  for (const type of ["pointerdown", "wheel", "mousemove"]) host.document.addEventListener(type, (event: any) => {
    if (active && (type !== "mousemove" || event.movementX || event.movementY)) append(lifecycle, { at: host.performance.now(), type: `unexpected-${type}` })
  }, { passive: true })
  const opportunity = () => { if (active) { append(callbacks, host.performance.now()); raf = host.requestAnimationFrame(opportunity) } }
  host.__playsrcSustained = {
    start() {
      if (used) throw new Error("Sustained observer permits one uninterrupted interval")
      const surface = host.document.querySelector("canvas.world-canvas"), main = host.document.querySelector("main")
      if (!surface || !main) throw new Error("Sustained gameplay surface is absent")
      started = host.performance.now(); active = used = true
      lastFrame = Number(surface.dataset.displayFrame); lastTick = Number(main.dataset.snapshotTick)
      host.__playsrcDeliveryRpc.start(started)
      observer = new host.MutationObserver(() => {
        const at = host.performance.now(), data = state(), nextFrame = Number(surface.dataset.displayFrame), tick = Number(data.snapshotTick)
        if (nextFrame < lastFrame || tick < lastTick) append(lifecycle, { at, type: "counter-reset" })
        if (nextFrame !== lastFrame) {
          missedFrames += Math.max(0, nextFrame - lastFrame - 1)
          append(frames, { at, frame: nextFrame, tick, camera: data.cameraPosition, performance: data.performance })
          for (const input of inputs) if (input.completedAt === null && input.camera !== data.cameraPosition) input.completedAt = at
          lastFrame = nextFrame
        }
        if (tick !== lastTick) { append(ticks, { at, before: lastTick, tick }); lastTick = tick }
      })
      observer.observe(surface, { attributes: true, attributeFilter: ["data-display-frame"] })
      observer.observe(main, { attributes: true, attributeFilter: ["data-snapshot-tick"] })
      raf = host.requestAnimationFrame(opportunity)
      return started
    },
    stop() {
      ended = host.performance.now(); active = false; observer?.disconnect(); host.cancelAnimationFrame(raf)
      return { started, ended, frames, callbacks, ticks, inputs, lifecycle, dropped, missedFrames, rpc: host.__playsrcDeliveryRpc.stop() }
    },
  }
}

export function summarizeSustainedWindow(sample: any, started: number, ended: number) {
  const ticks = sample.ticks.filter((tick: any) => tick.at >= started && tick.at < ended)
  const rpc = sample.rpc.records.filter((call: any) => call.received >= started && call.received < ended)
  const input = sample.inputs.filter((input: any) => input.at >= started && input.at < ended)
  const distribution = (key: string) => summarizeFrameTimes(rpc.map((call: any) => call.timings[key]).filter(Number.isFinite))
  return {
    submissions: deliveryTimeline(started, ended, sample.frames.map((frame: any) => frame.at)),
    raf: deliveryTimeline(started, ended, sample.callbacks),
    tickPublications: deliveryTimeline(started, ended, sample.ticks.map((tick: any) => tick.at)),
    observedTicks: ticks.reduce((sum: number, tick: any) => sum + tick.tick - tick.before, 0),
    observedTicksPerSecond: ticks.reduce((sum: number, tick: any) => sum + tick.tick - tick.before, 0) * 1000 / (ended - started),
    workerObserve: { calls: rpc.length, queue: distribution("queueMilliseconds"), service: distribution("transactMilliseconds"), roundTrip: summarizeFrameTimes(rpc.map((call: any) => call.elapsedMilliseconds)), censoredEnd: sample.rpc.pending ?? [] },
    input: { acknowledged: summarizeFrameTimes(input.filter((input: any) => input.completedAt !== null).map((input: any) => input.completedAt - input.at)),
      censored: input.filter((input: any) => input.completedAt === null).map((input: any) => ({ ...input, milliseconds: ended - input.at })) },
    scope: "Completed submissions/RAF and observed snapshot publication ticks are not physical/compositor FPS or instantaneous Worker ticks. Input is DOM delivery to changed-camera submission, not input-to-photon. Queue overlaps are not CPU time.",
  }
}
