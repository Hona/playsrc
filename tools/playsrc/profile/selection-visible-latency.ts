import { summarizeFrameTimes } from "./profile-window"

export type SelectionPixelSample = Readonly<{ startedEpoch: number; endedEpoch: number; matches: boolean | null }>

/** Native pixel sampling yields bounds, never an invented exact presentation time.
 * Missing first/last partners remain censored, including a wholly frozen sample. */
export function selectionVisibleLatency(inputEpoch: number, endedEpoch: number, samples: readonly SelectionPixelSample[]) {
  if (!Number.isFinite(inputEpoch) || !Number.isFinite(endedEpoch) || endedEpoch < inputEpoch) throw new Error("Invalid selection window")
  let previous = -Infinity
  for (const sample of samples) {
    if (![sample.startedEpoch, sample.endedEpoch].every(Number.isFinite) || sample.endedEpoch < sample.startedEpoch || sample.startedEpoch < previous) throw new Error("Native pixel samples are unordered")
    previous = sample.endedEpoch
  }
  const after = samples.filter(sample => sample.endedEpoch >= inputEpoch && sample.startedEpoch <= endedEpoch)
  const first = after.find(sample => sample.matches)
  const old = after.filter(sample => sample.matches === false && (!first || sample.endedEpoch <= first.startedEpoch)).at(-1)
  const gaps = samples.slice(1).flatMap((sample, index) => sample.endedEpoch >= inputEpoch && samples[index]!.startedEpoch <= endedEpoch
    ? [sample.endedEpoch - samples[index]!.startedEpoch] : [])
  return {
    lowerMilliseconds: Math.max(0, (old?.startedEpoch ?? inputEpoch) - inputEpoch),
    upperMilliseconds: first ? Math.max(0, first.endedEpoch - inputEpoch) : null,
    startCensored: !samples.some(sample => sample.endedEpoch <= inputEpoch),
    endCensored: !first,
    ownerCensoredSamples: after.filter(sample => sample.matches === null).length,
    censoredMilliseconds: first ? 0 : endedEpoch - inputEpoch,
    captureGapBounds: summarizeFrameTimes(gaps),
    sampleCoverage: summarizeFrameTimes(after.map(sample => sample.endedEpoch - sample.startedEpoch)),
  }
}
