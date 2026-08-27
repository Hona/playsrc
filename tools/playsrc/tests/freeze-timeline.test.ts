import { expect, test } from "bun:test"
import { summarizeCompositorFreezes, summarizeFreezeTimeline, summarizePresentedContent } from "../profile/freeze-timeline"

test("retains zero rolling buckets and a one-second freeze instead of averaging it away", () => {
  const result = summarizeFreezeTimeline([0, 10, 20, 1020, 1030, 1040, 2040], { startedMilliseconds: 0, endedMilliseconds: 2000 })
  expect(result.rolling250Milliseconds.map(row => row.frames)).toEqual([3, 0, 0, 0, 3, 0, 0, 0])
  expect(result.rolling1000Milliseconds[0]!.framesPerSecond).toBe(3)
  expect(result.maximumObservedGapMilliseconds).toBe(1000)
  const stalls = result.stalls.find(row => row.thresholdMilliseconds === 500)!
  expect(stalls.count).toBe(2)
  expect(stalls.cumulativeLongGapMilliseconds).toBe(1960)
  expect(stalls.freezeMillisecondsBeyondThreshold).toBe(960)
  expect(stalls.freezeRatioBeyondThreshold).toBe(0.48)
  expect(stalls.interStallStartMilliseconds).toEqual([1020])
})

test("empty and edge-censored samples retain silence and unknown context", () => {
  const empty = summarizeFreezeTimeline([], { startedMilliseconds: 0, endedMilliseconds: 1200 })
  expect(empty.maximumActiveSilenceMilliseconds).toBe(1200)
  expect(empty.maximumObservedGapMilliseconds).toBeNull()
  expect(empty.stalls.at(-1)!.count).toBe(1)
  expect(empty.boundaryCensoredIntervals[0]).toMatchObject({ missingPreviousObservation: true, missingFollowingObservation: true })
  expect(empty.rolling250Milliseconds.at(-1)).toMatchObject({ partial: true, framesPerSecond: 0 })
  const known = summarizeFreezeTimeline([-500, 100, 1500], { startedMilliseconds: 0, endedMilliseconds: 1000 })
  expect(known.maximumObservedGapMilliseconds).toBe(1400)
  expect(known.maximumActiveSilenceMilliseconds).toBe(900)
  expect(known.boundaryCensoredIntervals).toHaveLength(2)
  expect(known.boundaryCensoredIntervals[0]!.missingPreviousObservation).toBe(false)
})

test("repeated compositor content does not become new visible game frames", () => {
  const observations = Array.from({ length: 120 }, (_, index) => ({ at: index * 10, contentIdentity: index < 10 ? "game-1" : index < 110 ? "game-2" : "game-3" }))
  const report = summarizePresentedContent(observations, { startedMilliseconds: 0, endedMilliseconds: 1200 })
  expect(report.repeatedPresentations).toBe(117)
  expect(report.changingContent!.frames).toBe(3)
  expect(report.changingContent!.maximumObservedGapMilliseconds).toBe(1000)
  expect(report.changingContent!.rolling250Milliseconds[2]!.frames).toBe(0)
})

test("missing content identity is unavailable, not fake zero FPS or inferred JS completion", () => {
  const report = summarizePresentedContent([{ at: 1, contentIdentity: null }, { at: 2, contentIdentity: "game-1" }], { startedMilliseconds: 0, endedMilliseconds: 100 })
  expect(report.evidence).toBe("incomplete-content-presentation")
  expect(report.changingContent).toBeNull()
  expect(report.unknownPresentations).toBe(1)
  expect(() => summarizeFreezeTimeline([NaN], { startedMilliseconds: 0, endedMilliseconds: 100 })).toThrow()
})

test("independent compositor streams cannot fill each other's stalls", () => {
  const report = summarizeCompositorFreezes([
    { name: "Display::FrameDisplayed", ts: 0, pid: 1, tid: 2 },
    { name: "Display::FrameDisplayed", ts: 1_000_000, pid: 1, tid: 2 },
    { name: "Display::FrameDisplayed", ts: 500_000, pid: 3, tid: 4 },
  ], { startedMicroseconds: 0, endedMicroseconds: 1_000_000 })
  expect(report.streams).toHaveLength(2)
  expect(report.streams[0]!.maximumObservedGapMilliseconds).toBe(1000)
  expect(report.gameContentPresentation).toBeNull()
})
