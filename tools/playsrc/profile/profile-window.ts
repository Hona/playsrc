export const DEFAULT_PROFILE_SAMPLE_SECONDS = 6
export const MINIMUM_PROFILE_SAMPLE_SECONDS = 5
export const MAXIMUM_PROFILE_SAMPLE_SECONDS = 10

export function profileSampleSeconds(value: string | undefined = process.env.PROFILE_SAMPLE_SECONDS): number {
  if (value === undefined) return DEFAULT_PROFILE_SAMPLE_SECONDS
  const seconds = Number(value)
  if (!Number.isSafeInteger(seconds) || seconds < MINIMUM_PROFILE_SAMPLE_SECONDS || seconds > MAXIMUM_PROFILE_SAMPLE_SECONDS) {
    throw new Error(`PROFILE_SAMPLE_SECONDS must be an integer from ${MINIMUM_PROFILE_SAMPLE_SECONDS} through ${MAXIMUM_PROFILE_SAMPLE_SECONDS}`)
  }
  return seconds
}

export function divideProfileWindow(seconds: number, segments: number): readonly number[] {
  if (!Number.isSafeInteger(seconds) || seconds < MINIMUM_PROFILE_SAMPLE_SECONDS || seconds > MAXIMUM_PROFILE_SAMPLE_SECONDS) {
    throw new Error("profile sample duration is outside its real-time bounds")
  }
  if (!Number.isSafeInteger(segments) || segments < 1 || segments > seconds) {
    throw new Error("profile sample segment count is invalid")
  }
  return Object.freeze(Array.from({ length: segments }, (_, index) =>
    Math.floor(seconds / segments) + Number(index < seconds % segments)))
}

export function summarizeFrameTimes(values: readonly number[]) {
  if (values.some((value) => !Number.isFinite(value) || value < 0)) {
    throw new Error("profile frame-time sample is invalid")
  }
  const sorted = values.toSorted((left, right) => left - right)
  const percentile = (fraction: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))] ?? 0
  const rounded = (value: number) => Number(value.toFixed(3))
  return Object.freeze({
    frames: sorted.length,
    p50Milliseconds: rounded(percentile(0.5)),
    p95Milliseconds: rounded(percentile(0.95)),
    p99Milliseconds: rounded(percentile(0.99)),
    maximumMilliseconds: rounded(sorted.at(-1) ?? 0),
    over16Milliseconds: sorted.filter((value) => value > 1_000 / 60).length,
    over20Milliseconds: sorted.filter((value) => value > 20).length,
    over33Milliseconds: sorted.filter((value) => value > 1_000 / 30).length,
    over50Milliseconds: sorted.filter((value) => value > 50).length,
    over100Milliseconds: sorted.filter((value) => value > 100).length,
    over250Milliseconds: sorted.filter((value) => value > 250).length,
    over1000Milliseconds: sorted.filter((value) => value > 1_000).length,
  })
}
