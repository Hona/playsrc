import { describe, expect, test } from "bun:test"
import { attributeFrameTails } from "../profile/frame-tail-attribution"

describe("visible gameplay frame-tail attribution", () => {
  test("retains every long frame and correlates GC, worker scheduling, CPU stacks and actual visible input", () => {
    const result = attributeFrameTails({
      frames: [
        { at: 10, displayFrame: 1, mouseRevision: 0 },
        { at: 45, displayFrame: 2, mouseRevision: 1, detail: { total: 12 }, workerPending: 1 },
        { at: 1100, displayFrame: 3, mouseRevision: 2 },
      ],
      workers: [{ kind: "models", started: 20, finished: 42, timings: { queueMilliseconds: 7 } }],
      inputs: [{ at: 12, revision: 1, kind: "mouse" }, { at: 60, revision: 2, kind: "mouse" }],
      longAnimationFrames: [{ at: 25, duration: 60, styleAndLayoutMilliseconds: 5 }],
      trace: [
        { name: "MinorGC", ph: "X", ts: 30_000, dur: 8_000, args: { usedHeapSizeBefore: 900, usedHeapSizeAfter: 400 } },
        { name: "V8.GC_SCAVENGER_SCAVENGE", ph: "X", ts: 31_000, dur: 7_000 },
      ],
      cpu: {
        startTime: 0, endTime: 1_100_000,
        nodes: [{ id: 1, callFrame: { functionName: "render", url: "runtime.ts", lineNumber: 0, columnNumber: 0 } }],
        samples: [1, 1], timeDeltas: [30_000, 50_000],
      },
      traceOffsetMicroseconds: 0,
    })

    expect(result.frames.map(frame => frame.milliseconds)).toEqual([35, 1055])
    expect(result.frames[0]).toMatchObject({
      garbageCollection: [{ kind: "minor", milliseconds: 8 }],
      workers: [{ kind: "models", milliseconds: 22, queueMilliseconds: 7 }],
      stacks: [{ samples: 1, frames: ["render"] }],
    })
    expect(result.garbageCollection).toMatchObject({ count: 1, minor: 1, major: 0, reclaimedBytes: 500 })
    expect(result.inputToVisibleMilliseconds).toMatchObject({ count: 2, max: 1040 })
  })
})
