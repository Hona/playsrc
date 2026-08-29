import { deliveryTimeline } from "./frame-delivery"
import type { RawTraceEvent } from "./compositor-evidence"

export const SUSTAINED_KOTH = Object.freeze({ seconds: 90, bots: 23, historyRecords: 96,
  traceKilobytes: 8192, traceBytes: 8 * 1024 * 1024, gcRecords: 4096,
  categories: Object.freeze(["disabled-by-default-display.framedisplayed", "blink.user_timing", "disabled-by-default-v8.gc"]),
})

export function requireSustainedBudget(remaining: number) {
  if (!Number.isFinite(remaining) || remaining < 120_000) throw new Error(`Sustained KOTH needs 120000 ms after live admission (90000 uninterrupted + deep sample/retention); only ${remaining} ms remain`)
}

type Observation = {
  at: number; generation: string; tick: string; bots: { identity: number; team: number; class?: number; shots?: number; hits?: number; deaths?: number }[];
  round: { state: number; waitingForPlayers: boolean; inSetup: boolean };
  heap: { usedJSHeapSize: number } | null; gpuApi?: { live: { knownBytes: number }; created: { knownBytes: number }; writeTextureSourceBytes: number };
  failures?: unknown; losses: unknown[]
}

export function checkSustainedObservation(state: Observation, initial: Observation) {
  if (!Number.isFinite(state.at) || state.at < initial.at || !/^\d+$/.test(state.tick) || BigInt(state.tick) < BigInt(initial.tick)) throw new Error("Sustained observation clock regressed")
  const roster = (value: Observation) => value.bots.map(bot => `${bot.identity}:${bot.team}:${bot.class}`).sort()
  if (state.bots.length !== SUSTAINED_KOTH.bots || new Set(state.bots.map(bot => bot.identity)).size !== SUSTAINED_KOTH.bots
    || JSON.stringify(roster(state)) !== JSON.stringify(roster(initial)) || state.generation !== initial.generation
    || state.round.state !== 4 || state.round.waitingForPlayers || state.round.inSetup) throw new Error("Sustained active full roster/generation was interrupted")
  if (state.failures || state.losses.length) throw new Error("Sustained rendering reported a resource failure")
}

export function sustainedTrends(records: readonly Observation[], started: number, ended: number) {
  if (records.length > SUSTAINED_KOTH.historyRecords) throw new Error("Sustained history bound exceeded")
  const active = records.filter(record => record.at >= started && record.at <= ended)
  if (active.some((record, index) => index > 0 && record.at <= active[index - 1]!.at)) throw new Error("Sustained history is not ordered")
  const phase = (values: readonly Observation[]) => {
    const a = values[0], b = values.at(-1)
    const combat = (key: "shots" | "hits" | "deaths") => a && b && a.bots.every(bot => Number.isFinite(bot[key])) && b.bots.every(bot => Number.isFinite(bot[key]))
      ? b.bots.reduce((sum, bot) => sum + bot[key]!, 0) - a.bots.reduce((sum, bot) => sum + bot[key]!, 0) : null
    return { observations: values.length, started: a?.at ?? null, ended: b?.at ?? null,
      shots: combat("shots"), hits: combat("hits"), deaths: combat("deaths"),
      ticksPerSecond: a && b && b.at > a.at ? Number(BigInt(b.tick) - BigInt(a.tick)) * 1000 / (b.at - a.at) : null,
      jsHeapDelta: a?.heap && b?.heap ? b.heap.usedJSHeapSize - a.heap.usedJSHeapSize : null,
      gpuApiLiveBytesDelta: a?.gpuApi && b?.gpuApi ? b.gpuApi.live.knownBytes - a.gpuApi.live.knownBytes : null,
      gpuApiCreatedBytes: a?.gpuApi && b?.gpuApi ? b.gpuApi.created.knownBytes - a.gpuApi.created.knownBytes : null,
      textureUploadSourceBytes: a?.gpuApi && b?.gpuApi ? b.gpuApi.writeTextureSourceBytes - a.gpuApi.writeTextureSourceBytes : null }
  }
  return { whole: phase(active), early: phase(active.filter(record => record.at < started + 10_000)),
    late: phase(active.filter(record => record.at >= ended - 10_000)),
    observationDelivery: deliveryTimeline(started, ended, active.map(record => record.at)),
    scope: "Sparse observed endpoints only; no interpolated peaks, physical GPU residency, GC inference, or long-term leak claim" }
}

/** V8 trace evidence, not heap-drop inference. Keep nested/concurrent events
 * separate: summing them would fabricate stop-the-world or GC wall time. */
export function sustainedGcEvidence(events: readonly RawTraceEvent[], window: { startedMicroseconds: number; endedMicroseconds: number; pid?: number } | null, complete: boolean) {
  const records: { name: string; ts: number; dur: number | null; pid: number; tid: number | undefined; ph: string | undefined }[] = []
  let count = 0
  if (window && Number.isSafeInteger(window.pid)) for (const event of events) {
    if (event.pid !== window.pid || !event.cat?.split(",").includes("disabled-by-default-v8.gc") || !/^V8\.GC/.test(event.name ?? "")
      || !Number.isFinite(event.ts) || event.ts! < window.startedMicroseconds || event.ts! >= window.endedMicroseconds) continue
    count++
    if (records.length < SUSTAINED_KOTH.gcRecords) records.push({ name: event.name!, ts: event.ts!,
      dur: Number.isFinite(event.dur) && event.dur! >= 0 ? event.dur! : null, pid: event.pid!, tid: event.tid, ph: event.ph })
  }
  return { status: count ? "observed-v8-gc-events" : "unobserved-inconclusive", count, records, dropped: count - records.length, complete,
    scope: "Only active page-process V8 GC events; Worker isolates are not distinguished within the process. Nested/background/partial events are not added into pause duration. No forced GC; empty/incomplete traces and heap drops prove neither collection nor its absence." }
}
