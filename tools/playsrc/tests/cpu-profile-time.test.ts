import { describe, expect, test } from "bun:test"
import { CPU_PROFILE_LIMITS, reconstructCpuProfile, selectCpuWindow } from "../profile/cpu-profile-time"
import { summarizeCpuProfile, type CpuProfile } from "../profile/gameui-profile"

const profile = (timeDeltas: number[], samples = timeDeltas.map((_, index) => 2 + index % 2)): CpuProfile => ({
  startTime: 1_000_000, endTime: 1_001_000, samples, timeDeltas,
  nodes: [
    { id: 1, children: [2, 3], callFrame: { functionName: "(root)", url: "", lineNumber: -1, columnNumber: -1 } },
    { id: 2, callFrame: { functionName: "A", url: "", lineNumber: 0, columnNumber: 0 } },
    { id: 3, callFrame: { functionName: "B", url: "", lineNumber: 0, columnNumber: 0 } },
  ],
})
const window = (start: number, end: number) => ({ startedMicroseconds: 1_000_000 + start, endedMicroseconds: 1_000_000 + end })

describe("bounded CDP timestamp reconstruction", () => {
  test("preserves signed/raw order and pairs nodes while sorting absolute timestamps", () => {
    const input = profile([100, 600, -400, 600])
    const original = JSON.stringify(input)
    const timeline = reconstructCpuProfile(input)
    expect(timeline.rawSamples.map(s => [s.rawIndex, s.nodeId, s.timestampMicroseconds])).toEqual([
      [0, 2, 1_000_100], [1, 3, 1_000_700], [2, 2, 1_000_300], [3, 3, 1_000_900],
    ])
    expect(timeline.intervals.map(s => [s.rawIndex, s.endedMicroseconds - s.timestampMicroseconds])).toEqual([[0, 200], [2, 400], [1, 200], [3, 0]])
    expect(timeline.negativeDeltaCount).toBe(1)
    expect(timeline.maximumReorderMicroseconds).toBe(400)
    expect(timeline.signedElapsedMicroseconds).toBe(900)
    expect(selectCpuWindow(timeline)).toMatchObject({ sampleCount: 4, estimatedMicroseconds: 800, unattributedMicroseconds: 200 })
    expect(JSON.stringify(input)).toBe(original)
  })

  test("equal times and terminal samples count without inventing intervals", () => {
    const timeline = reconstructCpuProfile(profile([0, 0, 500, 500]))
    expect(timeline.equalTimestampCount).toBe(1)
    expect(timeline.intervals.map(s => s.endedMicroseconds - s.timestampMicroseconds)).toEqual([0, 500, 500, 0])
    expect(selectCpuWindow(timeline)).toMatchObject({ sampleCount: 4, estimatedMicroseconds: 1000, unattributedMicroseconds: 0 })
    expect(selectCpuWindow(timeline, window(0, 1000)).sampleCount).toBe(3)
    expect(selectCpuWindow(timeline, window(1000, 1100))).toMatchObject({ sampleCount: 1, estimatedMicroseconds: 0, outsideProfileMicroseconds: 100 })
    const summary = summarizeCpuProfile(profile([0, 0, 500, 500]))
    expect(summary.topSelf.map(s => s.samples).reduce((a, b) => a + b, 0)).toBe(4)
    expect(summary.topEdges.map(s => s.samples).reduce((a, b) => a + b, 0)).toBe(4)
  })

  test("clips crossing intervals, not only points; adjacent windows partition all weights and points", () => {
    const timeline = reconstructCpuProfile(profile([100, 600, -400, 600]))
    const left = selectCpuWindow(timeline, window(200, 700))
    expect(left).toMatchObject({ sampleCount: 1, estimatedMicroseconds: 500, unattributedMicroseconds: 0 })
    expect(left.samples.map(s => [s.rawIndex, s.samples, s.estimatedMicroseconds])).toEqual([[0, 0, 100], [2, 1, 400]])
    const right = selectCpuWindow(timeline, window(700, 1000))
    expect(right).toMatchObject({ sampleCount: 2, estimatedMicroseconds: 200, trailingUnattributedMicroseconds: 100 })
    const combined = selectCpuWindow(timeline, window(200, 1000))
    expect(left.estimatedMicroseconds + right.estimatedMicroseconds).toBe(combined.estimatedMicroseconds)
    expect(left.sampleCount + right.sampleCount).toBe(combined.sampleCount)
    const summary = summarizeCpuProfile(profile([100, 600, -400, 600]), window(200, 700))
    expect(summary.topSelf).toHaveLength(1)
    expect(summary.topSelf[0]).toMatchObject({ function: "A", samples: 1, estimatedMilliseconds: 0.5 })
  })

  test("overlapping query windows are independent, not additive CPU measurements", () => {
    const timeline = reconstructCpuProfile(profile([100, 600, -400, 600]))
    expect(selectCpuWindow(timeline, window(200, 800)).estimatedMicroseconds).toBe(600)
    expect(selectCpuWindow(timeline, window(500, 900)).estimatedMicroseconds).toBe(400)
    expect(selectCpuWindow(timeline, window(200, 900)).estimatedMicroseconds).toBe(700)
  })

  test("signed elapsed is not the chronological maximum when the final delta is negative", () => {
    const timeline = reconstructCpuProfile(profile([100, 800, -600]))
    expect(timeline.signedElapsedMicroseconds).toBe(300)
    expect(selectCpuWindow(timeline)).toMatchObject({ estimatedMicroseconds: 800, trailingUnattributedMicroseconds: 100 })
  })

  test("truncated, single, empty and absent samples leave honest unattributed tails", () => {
    expect(selectCpuWindow(reconstructCpuProfile(profile([100, 100])))).toMatchObject({
      estimatedMicroseconds: 100, leadingUnattributedMicroseconds: 100, trailingUnattributedMicroseconds: 800, unattributedMicroseconds: 900,
    })
    for (const p of [profile([500]), profile([]), { ...profile([]), samples: undefined, timeDeltas: undefined }]) {
      expect(summarizeCpuProfile(p)).toMatchObject({ estimatedSampledMilliseconds: 0, unattributedMilliseconds: 1 })
    }
    expect(selectCpuWindow(reconstructCpuProfile(profile([])), window(0, 1200))).toMatchObject({ unsampledProfileMicroseconds: 1000, outsideProfileMicroseconds: 200 })
    expect(selectCpuWindow(reconstructCpuProfile(profile([500])), window(500, 500))).toMatchObject({ sampleCount: 0, estimatedMicroseconds: 0 })
  })

  test("rejects malformed numeric, pairing, bounds and node data instead of silently dropping samples", () => {
    const valid = profile([100, 200])
    const invalid = [
      { ...valid, startTime: NaN }, { ...valid, endTime: Infinity }, { ...valid, endTime: 0 },
      { ...valid, startTime: -1 }, { ...valid, endTime: Number.MAX_SAFE_INTEGER + 1 },
      { ...valid, samples: undefined }, { ...valid, timeDeltas: undefined }, { ...valid, timeDeltas: [100] },
      profile([NaN]), profile([Infinity]), profile([0.5]), profile([-1]), profile([1001]), profile([500, -501]),
      profile([100], [99]), profile([100], [NaN]),
      { ...valid, nodes: [valid.nodes[0], valid.nodes[0]] },
      { ...valid, nodes: [{ ...valid.nodes[0], children: [99] }, ...valid.nodes.slice(1)] },
      { ...valid, nodes: [{ ...valid.nodes[0], children: [2, 2, 3] }, ...valid.nodes.slice(1)] },
      { ...valid, nodes: valid.nodes.map(n => ({ ...n, callFrame: { ...n.callFrame, lineNumber: NaN } })) },
      { ...valid, nodes: valid.nodes.map(n => ({ ...n, children: [n.id === 3 ? 1 : n.id + 1] })) },
      { ...valid, nodes: [valid.nodes[0], { ...valid.nodes[1], children: [3] }, valid.nodes[2]] },
      { ...valid, samples: Array(CPU_PROFILE_LIMITS.samples + 1).fill(2), timeDeltas: Array(CPU_PROFILE_LIMITS.samples + 1).fill(0) },
      { ...valid, nodes: Array(CPU_PROFILE_LIMITS.nodes + 1).fill(valid.nodes[0]) },
      { ...valid, nodes: Array.from({ length: CPU_PROFILE_LIMITS.depth + 2 }, (_, index) => ({
        ...valid.nodes[0]!, id: index + 1, children: index <= CPU_PROFILE_LIMITS.depth ? [index + 2] : [],
      })) },
    ]
    for (const input of invalid) expect(() => reconstructCpuProfile(input as CpuProfile)).toThrow("Invalid CPU profile")
    const timeline = reconstructCpuProfile(valid)
    for (const w of [window(5, 4), window(NaN, 2), window(0, Infinity), window(-1_000_001, 0)]) {
      expect(() => selectCpuWindow(timeline, w)).toThrow("window bounds")
    }
  })
})
