import { expect, test } from "bun:test"
import { deliveryTimeline, installDeliveryObserver, retainedDeliveryAttribution } from "../profile/frame-delivery"

test("delivery keeps empty seconds and start/end stalls rather than reporting only surviving intervals", () => {
  const value = deliveryTimeline(0, 6000, [2100, 2200, 2300])
  expect(value.buckets.map(bucket => bucket.count)).toEqual([0, 0, 3, 0, 0, 0])
  expect(value.initialGap).toBe(2100)
  expect(value.terminalGap).toBe(3700)
  expect(value.maximumGapIncludingBoundaries).toBe(3700)
  expect(deliveryTimeline(10, 6010, []).gapsIncludingBoundaries).toEqual([6000])
  expect(deliveryTimeline(0, 1001, [0, 1000, 1001]).buckets.map(bucket => bucket.count)).toEqual([1, 1])
})

test("passive observer counts unchanged RAF surfaces separately and detects missed publications", () => {
  let now = 0, callback = () => {}, mutation = () => {}
  const canvas = { dataset: { displayFrame: "2" } }
  const host: any = { performance: { now: () => now }, addEventListener() {}, document: { addEventListener() {}, querySelector: () => canvas },
    MutationObserver: class { constructor(fn: () => void) { mutation = fn } observe() {} disconnect() {} },
    requestAnimationFrame(fn: () => void) { callback = fn; return 1 }, cancelAnimationFrame() {} }
  installDeliveryObserver(host)
  host.__playsrcDeliveryObserver.start()
  now = 16; callback(); now = 32; callback()
  canvas.dataset.displayFrame = "4"; now = 40; mutation()
  const result = host.__playsrcDeliveryObserver.stop(100)
  expect(result.raf).toEqual([16, 32])
  expect(result.frames).toEqual([{ at: 40, frame: 4 }])
  expect(result.missedPublications).toBe(1)
  expect(result.compositor).toBeNull()
  expect(host.__playsrcFrameProfiler).toBeUndefined()
  expect(host.__playsrcProfile).toBeUndefined()
})

test("fast RAF and cheap render work cannot turn a Worker-bound stale surface into fast gameplay", () => {
  const result = retainedDeliveryAttribution({ started: 0, ended: 6000,
    frames: [400, 800, 1200].map(at => ({ at })), presentationCallbacks: Array.from({ length: 600 }, (_, index) => index * 10),
    worker: [{ kind: "observe", started: 0, finished: 5900, timings: { transactMilliseconds: 5800 } },
      { kind: "observe", started: 5950, finished: 6100, timings: { transactMilliseconds: 150 } }] },
  { compositor: { presentedFrames: 3 }, frameWork: { p50Milliseconds: 6.625 } })
  expect(result.completed.perSecond).toBe(0.5)
  expect(result.raf.perSecond).toBe(100)
  expect(result.observe.transactMilliseconds).toBe(5800)
  expect(result.completed.terminalGap).toBe(4800)
  expect(result.compositor.presentedFrames).toBe(3)
})
