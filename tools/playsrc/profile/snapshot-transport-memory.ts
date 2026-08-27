// SimulationSnapshotStream.metrics: only these fields are cumulative. Do not
// silently turn a future gauge into a counter, or missing endpoints into zero.
const counters = ["responses", "wireBytes", "canonicalWireBytes", "restoredBytes", "fullSnapshots", "deltaSnapshots",
  "decodedRanges", "reusedRanges", "reusedBytes", "decodeMilliseconds"] as const
export type SnapshotTransportBoundary = { at: number; ownerToken: number | null; values: Record<string, number> }

export function summarizeSnapshotTransport(before: SnapshotTransportBoundary, after: SnapshotTransportBoundary) {
  if (![before.at, after.at].every(Number.isFinite) || after.at < before.at) throw new Error("Snapshot transport boundary clock invalid")
  for (const boundary of [before, after]) for (const [key, value] of Object.entries(boundary.values)) {
    if (!(key === "retainedBaselineBytes" || counters.includes(key as typeof counters[number]))) throw new Error(`Unknown snapshot metric semantics: ${key}`)
    if (!Number.isFinite(value) || value < 0 || key !== "decodeMilliseconds" && !Number.isSafeInteger(value)) throw new Error(`Invalid snapshot metric gauge/counter: ${key}`)
  }
  const sameOwner = before.ownerToken !== null && before.ownerToken === after.ownerToken
  const counterDeltas = Object.fromEntries(counters.map(key => {
    const first = before.values[key], last = after.values[key]
    return [key, sameOwner && first !== undefined && last !== undefined && last >= first ? last - first : null]
  }))
  return { schema: "playsrc-snapshot-transport-memory-v1", clock: "page performance.now milliseconds", before, after, sameOwner,
    retainedGauges: { beforeBytes: before.values.retainedBaselineBytes ?? null, afterBytes: after.values.retainedBaselineBytes ?? null },
    counterDeltas, scope: "main SimulationSnapshotStream baseline logical byteLength; not total retained heap or physical memory" }
}
