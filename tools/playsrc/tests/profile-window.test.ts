import { describe, expect, test } from "bun:test"
import { divideProfileWindow, profileSampleSeconds, summarizeFrameTimes } from "../profile/profile-window"

describe("headed gameplay profile windows", () => {
  test("defaults to six real seconds and accepts only five through ten seconds", () => {
    expect(profileSampleSeconds(undefined)).toBe(6)
    expect(profileSampleSeconds("5")).toBe(5)
    expect(profileSampleSeconds("10")).toBe(10)
    for (const value of ["", "4", "11", "5.5", "NaN"]) {
      expect(() => profileSampleSeconds(value)).toThrow("PROFILE_SAMPLE_SECONDS must be an integer from 5 through 10")
    }
  })

  test("partitions one bounded sample without extending elapsed gameplay", () => {
    expect(divideProfileWindow(6, 3)).toEqual([2, 2, 2])
    expect(divideProfileWindow(5, 3)).toEqual([2, 2, 1])
    expect(divideProfileWindow(10, 1)).toEqual([10])
    expect(() => divideProfileWindow(4, 1)).toThrow("outside its real-time bounds")
    expect(() => divideProfileWindow(6, 0)).toThrow("segment count is invalid")
    expect(() => divideProfileWindow(6, 7)).toThrow("segment count is invalid")
  })

  test("summarizes frame regressions without changing simulation or rendering", () => {
    expect(summarizeFrameTimes([40, 12, 17, 8, 34])).toEqual({
      frames: 5,
      p50Milliseconds: 17,
      p95Milliseconds: 40,
      p99Milliseconds: 40,
      maximumMilliseconds: 40,
      over16Milliseconds: 3,
      over33Milliseconds: 2,
    })
    expect(summarizeFrameTimes([])).toEqual({
      frames: 0,
      p50Milliseconds: 0,
      p95Milliseconds: 0,
      p99Milliseconds: 0,
      maximumMilliseconds: 0,
      over16Milliseconds: 0,
      over33Milliseconds: 0,
    })
    expect(() => summarizeFrameTimes([Number.NaN])).toThrow("frame-time sample is invalid")
  })
})
