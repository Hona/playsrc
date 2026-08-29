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
  at: number; generation: string; tick: string; bots: { identity: number; team: number; class: number; shots?: number; hits?: number; deaths?: number }[];
  round: { state: number; waitingForPlayers: boolean; inSetup: boolean };
  heap: { usedJSHeapSize: number } | null; gpuApi?: { live: { knownBytes: number }; created: { knownBytes: number }; writeTextureSourceBytes: number };
  failures?: unknown; losses: unknown[]
}

const combatCounters = ["shots", "hits", "deaths"] as const
const knownCounter = (value: number | undefined): value is number => Number.isSafeInteger(value) && value! >= 0

export function checkSustainedObservation(state: Observation, initial: Observation, previous: Observation) {
  if (!Number.isFinite(state.at) || state.at < previous.at || !/^\d+$/.test(state.tick) || !/^\d+$/.test(previous.tick)
    || BigInt(state.tick) < BigInt(previous.tick)) throw new Error("Sustained observation clock regressed")
  // SDK CTFBot::PhysicsSimulate can reselect class after death. A changed class
  // is a lifecycle observation, not a new actor or an interrupted soak.
  const roster = (value: Observation) => value.bots.map(bot => `${bot.identity}:${bot.team}`).sort()
  if (state.bots.length !== SUSTAINED_KOTH.bots || new Set(state.bots.map(bot => bot.identity)).size !== SUSTAINED_KOTH.bots
    || JSON.stringify(roster(state)) !== JSON.stringify(roster(initial)) || state.generation !== initial.generation
    || state.round.state !== 4 || state.round.waitingForPlayers || state.round.inSetup) throw new Error("Sustained active full roster/generation was interrupted")
  if (state.failures || state.losses.length) throw new Error("Sustained rendering reported a resource failure")
  const prior = new Map(previous.bots.map(bot => [bot.identity, bot]))
  const classes: { identity: number; from: number; to: number }[] = []
  const counterResets: { identity: number; counter: typeof combatCounters[number]; from: number; to: number }[] = []
  for (const bot of state.bots.toSorted((a, b) => a.identity - b.identity)) {
    const before = prior.get(bot.identity)
    if (!before) throw new Error("Sustained actor replaced between observations")
    if (before.class !== bot.class) classes.push({ identity: bot.identity, from: before.class, to: bot.class })
    for (const counter of combatCounters) {
      const a = before[counter], b = bot[counter]
      if (knownCounter(a) && knownCounter(b) && b < a) counterResets.push({ identity: bot.identity, counter, from: a, to: b })
    }
  }
  return { fromAt: previous.at, toAt: state.at, fromTick: previous.tick, toTick: state.tick, classes, counterResets }
}

export function sustainedTransitionHistory(records: readonly Observation[]) {
  if (!records.length || records.length > SUSTAINED_KOTH.historyRecords) throw new Error("Sustained history missing or bound exceeded")
  const initial = records[0]!
  checkSustainedObservation(initial, initial, initial)
  const transitions: ReturnType<typeof checkSustainedObservation>[] = []
  for (let index = 1; index < records.length; index++) {
    const previous = records[index - 1]!, current = records[index]!
    if (current.at <= previous.at) throw new Error("Sustained history is not ordered")
    const transition = checkSustainedObservation(current, initial, previous)
    if (transition.classes.length || transition.counterResets.length) transitions.push(transition)
  }
  return { initialRoster: initial.bots.toSorted((a, b) => a.identity - b.identity).map(({ identity, team, class: playerClass }) => ({ identity, team, class: playerClass })), transitions,
    scope: "Sparse observed transitions and counter drops, bracketed by adjacent observations. Multiple changes inside a gap are unobserved; a counter drop does not establish its cause or exact reset tick." }
}

/** Other source/input/quality gates remain required. Reject differing observed
 * class/reset histories, including different relative tick brackets; never
 * authenticate a pair using only its matching initial/final roster. */
export function assertMatchingSustainedTransitionHistories(before: readonly Observation[], after: readonly Observation[]) {
  const identity = (records: readonly Observation[]) => {
    const history = sustainedTransitionHistory(records), origin = BigInt(records[0]!.tick)
    if (records.some(record => record.bots.some(bot => !Number.isInteger(bot.class) || bot.class < 1 || bot.class > 9
      || combatCounters.some(counter => !knownCounter(bot[counter]))))) throw new Error("Sustained class/counter history incomplete")
    return { initialRoster: history.initialRoster, transitions: history.transitions.map(({ fromTick, toTick, classes, counterResets }) => ({
      fromTick: String(BigInt(fromTick) - origin), toTick: String(BigInt(toTick) - origin), classes, counterResets })) }
  }
  if (JSON.stringify(identity(before)) !== JSON.stringify(identity(after))) throw new Error("Sustained observed class/counter transition histories differ")
}

export function sustainedTrends(records: readonly Observation[], started: number, ended: number) {
  if (records.length > SUSTAINED_KOTH.historyRecords) throw new Error("Sustained history bound exceeded")
  const history = sustainedTransitionHistory(records)
  const active = records.filter(record => record.at >= started && record.at <= ended)
  if (active.some((record, index) => index > 0 && record.at <= active[index - 1]!.at)) throw new Error("Sustained history is not ordered")
  const phase = (values: readonly Observation[]) => {
    const a = values[0], b = values.at(-1)
    const combat = (key: typeof combatCounters[number]) => {
      if (!values.length || values.some(value => value.bots.some(bot => !knownCounter(bot[key])))) return null
      let observed = 0
      for (let index = 1; index < values.length; index++) {
        const prior = new Map(values[index - 1]!.bots.map(bot => [bot.identity, bot[key]!]))
        for (const bot of values[index]!.bots) { const old = prior.get(bot.identity)!, current = bot[key]!
          observed += current >= old ? current - old : current
        }
      }
      return observed
    }
    return { observations: values.length, started: a?.at ?? null, ended: b?.at ?? null,
      shots: combat("shots"), hits: combat("hits"), deaths: combat("deaths"),
      ticksPerSecond: a && b && b.at > a.at ? Number(BigInt(b.tick) - BigInt(a.tick)) * 1000 / (b.at - a.at) : null,
      jsHeapDelta: a?.heap && b?.heap ? b.heap.usedJSHeapSize - a.heap.usedJSHeapSize : null,
      gpuApiLiveBytesDelta: a?.gpuApi && b?.gpuApi ? b.gpuApi.live.knownBytes - a.gpuApi.live.knownBytes : null,
      gpuApiCreatedBytes: a?.gpuApi && b?.gpuApi ? b.gpuApi.created.knownBytes - a.gpuApi.created.knownBytes : null,
      textureUploadSourceBytes: a?.gpuApi && b?.gpuApi ? b.gpuApi.writeTextureSourceBytes - a.gpuApi.writeTextureSourceBytes : null }
  }
  return { history, whole: phase(active), early: phase(active.filter(record => record.at < started + 10_000)),
    late: phase(active.filter(record => record.at >= ended - 10_000)),
    observationDelivery: deliveryTimeline(started, ended, active.map(record => record.at)),
    counterScope: "Per-actor observed lower bounds across adjacent samples. A drop contributes only the current post-drop endpoint; activity lost before a reset or inside an unobserved reset/recovery is unknown. Missing counters remain null.",
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
