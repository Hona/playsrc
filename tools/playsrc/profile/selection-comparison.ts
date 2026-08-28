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
