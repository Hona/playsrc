import { summarizeDistribution } from "./gameui-profile"
import { decodeAdmissionMetrics } from "../../../games/tf2/browser/src/admission-metrics"

type Event = ReturnType<typeof decodeAdmissionMetrics>[number]
const PHASES = [
  [9, 10, "session-checkpoint"],
  [10, 11, "collision-checkpoint"],
  [11, 12, "movers-physics-and-session-advance"],
  [12, 13, "producer-entity-presentation-and-decals"],
  [13, 8, "snapshot-encode-and-commit"],
] as const

/** The existing opt-in Rust edges bracket real transactions. Allocation deltas
 * are requested logical bytes, not residency; clock differences are wall time. */
export function summarizeObserveStages(events: readonly Event[], activeTransactions?: ReadonlySet<number>) {
  const groups: Event[][] = []
  for (const event of events) {
    if (event.actor !== 0) continue
    if (event.stage === 9) groups.push([])
    if (groups.length && [9, 10, 11, 12, 13, 8].includes(event.stage)) groups.at(-1)!.push(event)
  }
  const phases = PHASES.map(([from, to, name]) => ({ name, samples: [] as { transaction: number; tick: number; wallMilliseconds: number; allocations: number; allocatedBytes: number; liveByteDelta: number }[] }))
  const incomplete: number[] = []
  for (const [transaction, group] of groups.entries()) {
    if (activeTransactions && !activeTransactions.has(transaction)) continue
    if (group.map(event => event.stage).join() !== "9,10,11,12,13,8") { incomplete.push(transaction); continue }
    for (const [index, [from, to]] of PHASES.entries()) {
      const start = group.find(event => event.stage === from)!, end = group.find(event => event.stage === to)!
      if (end.at < start.at || end.allocations < start.allocations || end.allocatedBytes < start.allocatedBytes) throw new Error("Admission clock/allocation counters regressed")
      phases[index]!.samples.push({ transaction, tick: start.tick, wallMilliseconds: end.at - start.at,
        allocations: end.allocations - start.allocations, allocatedBytes: end.allocatedBytes - start.allocatedBytes, liveByteDelta: end.heapBytes - start.heapBytes })
    }
  }
  return { transactions: groups.length, incomplete, phases: phases.map(phase => ({ ...phase,
    wall: summarizeDistribution(phase.samples.map(sample => sample.wallMilliseconds)),
    allocations: phase.samples.reduce((sum, sample) => sum + sample.allocations, 0),
    allocatedBytes: phase.samples.reduce((sum, sample) => sum + sample.allocatedBytes, 0),
  })) }
}
