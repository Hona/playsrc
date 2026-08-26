import { describe, expect, test } from "bun:test"
import { activeGameplayTraceWindow, analyzeCompositorStalls, assertVisibleGameplayTruth, summarizeCompositorTruth } from "../profile/compositor-truth"

describe("truthful compositor presentation evidence", () => {
  test("never mislabels animation callbacks, swaps, or application frames as presentation", () => {
    expect(summarizeCompositorTruth([{ name: "RequestAnimationFrame", ts: 1 }, { name: "SwapBuffers", ts: 2 }], 5000)).toMatchObject({ evidence: "unavailable", presentedFrames: null, presentedFramesPerSecond: null, intervals: null })
  })

  test("deduplicates compositor timestamps and preserves seconds-per-frame stalls", () => {
    expect(summarizeCompositorTruth([{ name: "Display::FrameDisplayed", ts: 1_000 }, { name: "Display::FrameDisplayed", ts: 1_000 }, { name: "Display::FrameDisplayed", ts: 2_001_000 }, { name: "Graphics.Pipeline.DrawAndSwap", ts: 1_001_000 }], 5000)).toMatchObject({ evidence: "chromium-compositor-presentation-trace", presentedFrames: 2, intervals: { maximumMilliseconds: 2000 }, eventNames: ["Display::FrameDisplayed"] })
  })

  test("uses one strongest presentation event family instead of triple-counting pipeline stages", () => {
    expect(summarizeCompositorTruth([{ name: "PresentationFeedback", ts: 1_000 }, { name: "Display::FrameDisplayed", ts: 1_100 }, { name: "PresentationFeedback", ts: 17_000 }, { name: "Display::FrameDisplayed", ts: 17_100 }], 1000)).toMatchObject({ presentedFrames: 2, eventNames: ["PresentationFeedback"] })
  })

  test("recognizes current Chromium FramePresented events without accepting animation callbacks", () => {
    expect(summarizeCompositorTruth([
      { name: "RequestAnimationFrame", ts: 500 },
      { name: "FramePresented", ts: 1_000 },
      { name: "FramePresented", ts: 18_000 },
    ], 1_000)).toMatchObject({
      evidence: "chromium-compositor-presentation-trace",
      presentedFrames: 2,
      eventNames: ["FramePresented"],
      intervals: { p50Milliseconds: 17 },
    })
  })

  test("excludes compositor frames outside the real active gameplay window", () => {
    expect(summarizeCompositorTruth([{ name: "Display::FrameDisplayed", ts: 99 }, { name: "Display::FrameDisplayed", ts: 100 }, { name: "Display::FrameDisplayed", ts: 200 }, { name: "Display::FrameDisplayed", ts: 201 }], 1000, { startedMicroseconds: 100, endedMicroseconds: 200 })).toMatchObject({ presentedFrames: 2 })
    expect(() => summarizeCompositorTruth([], 1000, { startedMicroseconds: 2, endedMicroseconds: 1 })).toThrow("window is invalid")
  })

  test("attributes a real presentation gap to overlapping class lifecycle and bounded Chromium work", () => {
    expect(analyzeCompositorStalls([
      { name: "Display::FrameDisplayed", ts: 1_000_000 },
      { name: "Display::FrameDisplayed", ts: 1_550_000 },
      { name: "RunTask", ts: 1_100_000, dur: 310_000, pid: 7, tid: 8 },
      { name: "MinorGC", ts: 1_200_000, dur: 45_000, pid: 7, tid: 8 },
      { name: "BeginFrame", ts: 1_300_000, pid: 9, tid: 2 },
    ], { startedMicroseconds: 900_000, endedMicroseconds: 1_600_000 }, [
      { at: 220, phase: "selected", playerClass: 6 },
    ], 50)).toMatchObject([{
      milliseconds: 550,
      startedMilliseconds: 100,
      classes: [{ at: 220, phase: "selected", playerClass: 6 }],
      work: [
        { name: "RunTask", milliseconds: 310, overlapMilliseconds: 310, pid: 7, tid: 8 },
        { name: "MinorGC", milliseconds: 45, overlapMilliseconds: 45, pid: 7, tid: 8 },
      ],
      beginFrames: 1,
    }])
  })

  test("rejects synthetic intervals and does not attribute work outside a real presentation gap", () => {
    expect(analyzeCompositorStalls([
      { name: "Display::FrameDisplayed", ts: 1_000 },
      { name: "Display::FrameDisplayed", ts: 18_000 },
      { name: "RunTask", ts: 20_000, dur: 900_000 },
    ], { startedMicroseconds: 0, endedMicroseconds: 1_000_000 }, [], 50)).toEqual([])
  })

  test("bounds evidence to exact browser gameplay marks, excluding post-sample protocol serialization", () => {
    const events = [
      { name: "playsrc-active-gameplay-start", ts: 100_000 },
      { name: "Display::FrameDisplayed", ts: 120_000 },
      { name: "Display::FrameDisplayed", ts: 136_000 },
      { name: "playsrc-active-gameplay-end", ts: 150_000 },
      { name: "Display::FrameDisplayed", ts: 686_000 },
    ]
    const window = activeGameplayTraceWindow(events)
    expect(window).toEqual({ startedMicroseconds: 100_000, endedMicroseconds: 150_000 })
    expect(summarizeCompositorTruth(events, 50, window)).toMatchObject({ presentedFrames: 2, intervals: { maximumMilliseconds: 16 } })
    expect(() => activeGameplayTraceWindow([{ name: "playsrc-active-gameplay-start", ts: 1 }])).toThrow("gameplay marks")
  })

  test("rejects hidden, unfocused, frozen, unsubmitted, and visually static gameplay", () => {
    const valid = { visible: true, focused: true, ticks: 10, displayFrames: 8, submissions: 8, beforeSha256: "before", afterSha256: "after" }
    expect(() => assertVisibleGameplayTruth(valid)).not.toThrow()
    for (const change of [{ visible: false }, { focused: false }, { ticks: 0 }, { displayFrames: 0 }, { submissions: 0 }, { afterSha256: "before" }]) expect(() => assertVisibleGameplayTruth({ ...valid, ...change })).toThrow("Gameplay evidence rejected")
    expect(() => summarizeCompositorTruth([], 0)).toThrow("positive")
  })
})
