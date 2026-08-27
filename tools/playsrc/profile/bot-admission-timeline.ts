import { summarizeFrameTimes } from "./profile-window"

export type AdmissionEvent = Readonly<{
  stage: number; actor: number; tick: number; at: number; heapBytes: number;
  allocations: number; allocatedBytes: number; value: number;
}>
type Actor = Readonly<{ actor: number; class?: number; team?: number; model?: string; skin?: number; weapon?: number | null }>
export type AdmissionBrowserEvent = Readonly<{ at: number; stage: string; tick: string; detail: { actors?: readonly Actor[]; milliseconds?: number; displayFrame?: number } }>
export type AdmissionFrame = Readonly<{ at: number; tick: number; detail?: unknown }>

const stages = { quota: 1, request: 2, loadout: 3, navigation: 4, constructed: 5, roster: 6, respawn: 7, encoded: 8, transaction: 9, cloned: 10, collision: 11, advanced: 12, encode: 13, published: 14, rollback: 15 } as const

export function summarizeBotAdmissions(input: {
  events: readonly AdmissionEvent[]; browser: readonly AdmissionBrowserEvent[];
  frames: readonly AdmissionFrame[]; workerTimeOrigin: number; pageTimeOrigin: number;
  dropped: number; browserDropped: number;
}) {
  if (input.dropped || input.browserDropped) throw new Error("Bot admission evidence was truncated")
  if (![input.workerTimeOrigin, input.pageTimeOrigin].every(Number.isFinite)) throw new Error("Bot admission clock origins are unavailable")
  for (let index = 0; index < input.events.length; index++) {
    const row = input.events[index]!
    if (!Object.values(row).every(value => Number.isFinite(value) && value >= 0)
      || row.at < (input.events[index - 1]?.at ?? 0)) throw new Error("Bot admission timestamps or counters are invalid")
  }
  for (const timeline of [input.frames, input.browser]) {
    for (let index = 0; index < timeline.length; index++) {
      const row = timeline[index]!
      if (!Number.isFinite(row.at) || row.at < (timeline[index - 1]?.at ?? 0) || !Number.isSafeInteger(Number(row.tick)) || Number(row.tick) < 0) throw new Error("Bot publication timeline is invalid")
    }
  }
  const shift = input.workerTimeOrigin - input.pageTimeOrigin
  const cost = (first: AdmissionEvent, last: AdmissionEvent) => {
    if (last.at < first.at || last.allocations < first.allocations || last.allocatedBytes < first.allocatedBytes) throw new Error("Bot admission counters reversed")
    return { milliseconds: last.at - first.at, allocations: last.allocations - first.allocations, allocatedBytes: last.allocatedBytes - first.allocatedBytes, retainedHeapBytes: last.heapBytes - first.heapBytes }
  }
  const ticks = new Map<number, AdmissionEvent[]>()
  for (const row of input.events) {
    const rows = ticks.get(row.tick) ?? []
    if (!ticks.has(row.tick)) ticks.set(row.tick, rows)
    rows.push(row)
  }
  const seenVariants = new Set<string>()
  const requests = input.events.filter(row => row.stage === stages.request)
  const admissions = requests.map(request => {
    const rows = ticks.get(request.tick)!
    const at = request.at + shift
    const edge = (stage: number, actor = 0) => rows.find(row => row.stage === stage && row.actor === actor && row.at >= request.at)
    const loadout = edge(stages.loadout, request.actor), navigation = edge(stages.navigation, request.actor), constructed = edge(stages.constructed, request.actor)
    const rollback = edge(stages.rollback)
    const roster = edge(stages.roster), encoded = edge(stages.encoded)
    const committed = !!constructed && !!roster && !!encoded && !rollback
    const publication = committed ? input.browser.find(row => row.stage === "publication" && Number(row.tick) > request.tick && row.detail.actors?.some(actor => actor.actor === request.actor)) : undefined
    const actor = publication?.detail.actors?.find(actor => actor.actor === request.actor)
    const variant = actor ? `${actor.class}:${actor.team}` : null
    const repeatedClassTeam = variant === null ? null : seenVariants.has(variant)
    if (variant !== null) seenVariants.add(variant)
    const model = committed ? input.browser.find(row => row.stage === "model-request" && Number(row.tick) > request.tick && row.detail.actors?.some(actor => actor.actor === request.actor)) : undefined
    const submitted = committed ? input.browser.find(row => row.stage === "frame-submitted" && Number(row.tick) > request.tick && row.detail.actors?.some(actor => actor.actor === request.actor)) : undefined
    const rosterFrame = publication ? input.frames.find(frame => frame.tick >= Number(publication.tick)) : undefined
    const endingFrame = input.frames.findIndex(frame => frame.at >= at)
    const enclosingGap = endingFrame > 0 ? { from: input.frames[endingFrame - 1]!.at, to: input.frames[endingFrame]!.at, milliseconds: input.frames[endingFrame]!.at - input.frames[endingFrame - 1]!.at } : null
    const transaction = rows.find(row => row.stage === stages.transaction)
    return {
      actor: request.actor, tick: request.tick, at, committed, class: actor?.class ?? null, team: actor?.team ?? null, weapon: actor?.weapon ?? null,
      repeatedClassTeam,
      construction: constructed ? cost(request, constructed) : null,
      loadout: loadout ? cost(request, loadout) : null,
      navigation: loadout && navigation ? cost(loadout, navigation) : null,
      roster: constructed && roster ? cost(constructed, roster) : null,
      transaction: transaction && encoded ? cost(transaction, encoded) : null,
      encodedBytes: encoded?.value ?? null,
      publicationAt: publication?.at ?? null, rosterFrameAt: rosterFrame?.at ?? null,
      firstModelRequest: model ? { at: model.at, tick: model.tick, ...model.detail.actors!.find(actor => actor.actor === request.actor)! } : null,
      firstModelSubmissionAt: submitted?.at ?? null,
      // A submitted model can be occluded. Only pixel/depth evidence can establish
      // its first visible frame; do not equate roster/model publication with it.
      firstVisibleFrameAt: null,
      enclosingFrameGap: enclosingGap,
    }
  })
  const spawnTicks = new Set(requests.map(row => row.tick))
  const transactionCosts = [...ticks].flatMap(([tick, rows]) => {
    const start = rows.find(row => row.stage === stages.transaction), end = rows.find(row => row.stage === stages.encoded)
    return start && end && !rows.some(row => row.stage === stages.rollback) ? [{ tick, spawn: spawnTicks.has(tick), ...cost(start, end) }] : []
  })
  return {
    admissions,
    quotaTicks: input.events.filter(row => row.stage === stages.quota).map(row => row.tick),
    respawns: input.events.filter(row => row.stage === stages.respawn).map(row => ({ actor: row.actor, tick: row.tick, at: row.at + shift })),
    transactionCosts,
    spawnTickTimes: summarizeFrameTimes(transactionCosts.filter(row => row.spawn).map(row => row.milliseconds)),
    controlTickTimes: summarizeFrameTimes(transactionCosts.filter(row => !row.spawn).map(row => row.milliseconds)),
    interpretation: "Event alignment only; proximity to a frame gap does not establish causality. Model submission is not visible-pixel evidence.",
  }
}
