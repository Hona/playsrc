import { expect, test } from "bun:test"
import { compareDeliveryEvidence, deliveryTimeline, installDeliveryObserver, retainedDeliveryAttribution, summarizeDeliveryMeasurement } from "../profile/frame-delivery"

test("delivery keeps empty seconds and start/end stalls rather than reporting only surviving intervals", () => {
  const value = deliveryTimeline(0, 6000, [2100, 2200, 2300])
  expect(value.buckets.map(bucket => bucket.count)).toEqual([0, 0, 3, 0, 0, 0])
  expect(value.initialGap).toBe(2100)
  expect(value.terminalGap).toBe(3700)
  expect(value.maximumGapIncludingBoundaries).toBe(3700)
  expect(value.quarterSecondBuckets).toHaveLength(24)
  expect(value.statisticsIncludingBoundaries.over500Milliseconds).toBe(2)
  expect(value.statisticsIncludingBoundaries.over1000Milliseconds).toBe(2)
  expect(deliveryTimeline(10, 6010, []).gapsIncludingBoundaries).toEqual([6000])
  expect(deliveryTimeline(0, 1001, [0, 1000, 1001]).buckets.map(bucket => bucket.count)).toEqual([1, 1])
})

test("paired evidence rejects a changed source, resolution, quality, camera or active roster", () => {
  const commit = "a".repeat(40), fingerprint = "b".repeat(64)
  const sample = { started: 0, ended: 6000, firstFrame: 0, lastFrame: 1, frames: [{ at: 400, frame: 1 }], raf: [10, 20],
    before: { tick: 10, bots: 15, botProbe: Array.from({ length: 15 }, (_, index) => `${index + 2}:${2 + index % 2}:${1 + index % 9}`).join("|") }, after: { tick: 20, bots: 15 }, lifecycle: [], missedPublications: 0 }
  const nativeAdmission = [{ native: { desktop: { state: 0, flags: 1, protocol: 0, processSessionId: 3, consoleSessionId: 3 },
    foreground: 12, windows: [{ id: 12, visible: true, minimized: false }] } }]
  const ordinary = { mode: "ordinary", applicationCommit: commit, sourceFingerprint: fingerprint, sample, nativeAdmission }
  const traced = { ...ordinary, mode: "traced" }
  const boundary = { applicationCommit: commit, sourceFingerprint: fingerprint, browserVersion: "browser", capturePlan: { interaction: "forward-movement", entry: "training", target: "pl_upward" },
    configuration: { assetOrigin: "http://127.0.0.1:4000", renderLevel: 0 }, boundary: { viewport: { width: 1280, height: 720, dpr: 1 },
      userAgent: "browser", storage: {}, state: { cameraPosition: "1,2,3", cameraYaw: "0" }, instrumentation: { app: false, frame: false } } }
  const traceBoundary = { ...boundary, boundary: { ...boundary.boundary, instrumentation: { app: true, frame: true } } }
  expect(compareDeliveryEvidence(ordinary, boundary, traced, traceBoundary).ordinary.completed.zeroBuckets).toBe(5)
  const presentation = { ...traced, mode: "presentation" }
  expect(compareDeliveryEvidence(ordinary, boundary, presentation, boundary).traced.completed.count).toBe(1)
  expect(() => compareDeliveryEvidence(ordinary, boundary, presentation, traceBoundary)).toThrow("instrumentation")
  expect(() => compareDeliveryEvidence(ordinary, boundary, { ...presentation, mode: "cpu" }, boundary)).toThrow("Expected ordinary")
  for (const changed of [
    { ...traceBoundary, applicationCommit: "c".repeat(40) },
    { ...traceBoundary, configuration: { ...traceBoundary.configuration, renderLevel: 1 } },
    { ...traceBoundary, boundary: { ...traceBoundary.boundary, viewport: { width: 640, height: 480, dpr: 1 } } },
    { ...traceBoundary, boundary: { ...traceBoundary.boundary, state: { cameraPosition: "4,5,6" } } },
    { ...traceBoundary, boundary: { ...traceBoundary.boundary, state: { ...traceBoundary.boundary.state, cameraYaw: "129" } } },
  ]) expect(() => compareDeliveryEvidence(ordinary, boundary, traced, changed)).toThrow()
  expect(() => compareDeliveryEvidence(ordinary, boundary, { ...traced, sample: { ...sample, before: { ...sample.before, botProbe: "2:3:9" } } }, traceBoundary)).toThrow("roster")
  expect(() => compareDeliveryEvidence(ordinary, boundary, { ...traced, sample: { ...sample, before: { ...sample.before, bots: 23 } } }, traceBoundary)).toThrow("changed comparison")
  expect(() => compareDeliveryEvidence(ordinary, boundary, { ...traced, sample: { ...sample, dropped: 1 } }, traceBoundary)).toThrow("changed comparison")
  const fullSample = { ...sample, before: { ...sample.before, bots: 23, botProbe: Array.from({ length: 23 }, (_, index) => `${index + 2}:${2 + index % 2}:${1 + index % 9}`).join("|") }, after: { ...sample.after, bots: 23 } }
  const fullBoundary = { ...boundary, capturePlan: { ...boundary.capturePlan, entry: "create-server", target: "ctf_2fort" } }
  expect(compareDeliveryEvidence({ ...ordinary, sample: fullSample }, fullBoundary, { ...presentation, sample: fullSample }, fullBoundary).ordinary.ticks).toBe(10)
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
  { compositor: { presentedFrames: 3 }, renderWork: { p50Milliseconds: 6.625 } })
  expect(result.completed.perSecond).toBe(0.5)
  expect(result.raf.perSecond).toBe(100)
  expect(result.observe.transactMilliseconds).toBe(5800)
  expect(result.completed.terminalGap).toBe(4800)
  expect(result.compositor).toEqual({ presentedFrames: 3 })
})

test("the report producer labels model preparation and render submission as separate elapsed phases", () => {
  const value = summarizeDeliveryMeasurement({ started: 0, ended: 6000, presentationCallbacks: [10, 20], frames: [
    { at: 400, detail: { models: 140, total: 6.625 } }, { at: 900, detail: { models: 150, total: 8 } },
  ] })
  expect(value.modelPreparationLatency.total).toBe(290)
  expect(value.renderSubmissionElapsed.maximumMilliseconds).toBe(8)
  expect(value.delivery.completed.maximumGapIncludingBoundaries).toBe(5100)
  expect(value).not.toHaveProperty("botWork")
  expect(value).not.toHaveProperty("frameWork")
})

test("ordinary observation retains the existing phase publication and its own frame join", () => {
  let mutation = () => {}
  const canvas = { dataset: { displayFrame: "1" } }
  const root = { dataset: { displayFrame: "1", snapshotTick: "300", performance: "1,2,3,4,5:6,7,8,9" } }
  const host: any = { performance: { now: () => 10 }, addEventListener() {}, document: { addEventListener() {}, querySelector: (selector: string) => selector === "main" ? root : canvas },
    MutationObserver: class { constructor(fn: () => void) { mutation = fn } observe() {} disconnect() {} },
    requestAnimationFrame() { return 1 }, cancelAnimationFrame() {} }
  installDeliveryObserver(host); host.__playsrcDeliveryObserver.start()
  canvas.dataset.displayFrame = "2"
  mutation()
  expect(host.__playsrcDeliveryObserver.stop(20).frames[0]).toMatchObject({ frame: 2, phaseFrame: 1, producerTick: "300", performance: root.dataset.performance })
  expect(root.dataset.displayFrame).toBe("1") // Stale phase joins are retained, never relabeled.
})

test("one-second freezes retain zero quarters and censored silence rather than a mean-only result", () => {
  const result = deliveryTimeline(0, 4000, [10, 26, 1040, 1056, 2556])
  expect(result.intervalStatistics.maximumMilliseconds).toBe(1500)
  expect(result.intervalStatistics.over1000Milliseconds).toBe(2)
  expect(result.buckets.map(bucket => bucket.count)).toEqual([2, 2, 1, 0])
  expect(result.quarterSecondBuckets.filter(bucket => bucket.count === 0)).toHaveLength(13)
  expect(result.terminalGap).toBe(1444)
})

test("prelude and soak reuse a single observer without accumulating input listeners", () => {
  let listeners = 0
  const host: any = { performance: { now: () => 0 }, addEventListener() { listeners++ },
    document: { addEventListener() { listeners++ } } }
  installDeliveryObserver(host)
  const owner = host.__playsrcDeliveryObserver, count = listeners
  installDeliveryObserver(host)
  expect(host.__playsrcDeliveryObserver).toBe(owner)
  expect(listeners).toBe(count)
})

test("sustained observations bound frame and RAF retention and report overflow", () => {
  let mutation = () => {}, raf = () => {}, blur = () => {}
  const canvas = { dataset: { displayFrame: "1" } }
  const host: any = { performance: { now: () => 10 }, addEventListener(_name: string, fn: () => void) { blur = fn }, document: { addEventListener() {}, querySelector: () => canvas },
    MutationObserver: class { constructor(fn: () => void) { mutation = fn } observe() {} disconnect() {} },
    requestAnimationFrame(fn: () => void) { raf = fn; return 1 }, cancelAnimationFrame() {} }
  installDeliveryObserver(host); host.__playsrcDeliveryObserver.start()
  for (let index = 0; index < 20_002; index++) { canvas.dataset.displayFrame = String(index + 2); mutation(); raf(); blur() }
  const sample = host.__playsrcDeliveryObserver.stop(20)
  expect(sample.frames).toHaveLength(20_000)
  expect(sample.raf).toHaveLength(20_000)
  expect(sample.lifecycle).toHaveLength(20_000)
  expect(sample.dropped).toBe(6)
})
