import { expect, test } from "bun:test"
import { requireSustainedBudget } from "../profile/sustained-koth"
import { upwardCapturePlan, validateUpwardCapturePlan } from "../profile/upward-capture-plan"
import { profileMinimumRemainingMilliseconds } from "../src/profile-runner"
import { SUSTAINED_KOTH, checkSustainedObservation, sustainedGcEvidence, sustainedTrends } from "../profile/sustained-koth-evidence"

test("sustained admission never truncates the 90 second soak to fit", () => {
  for (const budget of [NaN, 90_000, 119_999]) expect(() => requireSustainedBudget(budget)).toThrow()
  expect(() => requireSustainedBudget(120_000)).not.toThrow()
  expect(profileMinimumRemainingMilliseconds("koth-sustained-sawmill")).toBe(150_000)
  expect(profileMinimumRemainingMilliseconds("koth-sustained-lakeside")).toBe(150_000)
})

const observation = (at = 0) => ({ at, generation: "1", tick: String(Math.floor(at / 15)),
  bots: Array.from({ length: 23 }, (_, index) => ({ identity: index + 2, team: 2 + index % 2, class: 1 + index % 9, shots: 0 })),
  round: { state: 4, waitingForPlayers: false, inSetup: false }, heap: { usedJSHeapSize: 1000 }, losses: [],
  gpuApi: { live: { knownBytes: 100 }, created: { knownBytes: 100 }, writeTextureSourceBytes: 100 } })

test("full soak cannot enable deep CPU/allocation tracing or unbounded buffers", () => {
  expect(SUSTAINED_KOTH.categories).toEqual(["disabled-by-default-display.framedisplayed", "blink.user_timing", "disabled-by-default-v8.gc"])
  expect(SUSTAINED_KOTH.traceBytes).toBe(8 * 1024 * 1024)
  expect(SUSTAINED_KOTH.traceKilobytes).toBe(8192)
  expect(SUSTAINED_KOTH.seconds).toBe(90)
})

test("lifecycle admission rejects replacement, duplicate/changed roster, setup reentry, failures and regressed clocks", () => {
  const initial = observation()
  expect(() => checkSustainedObservation(observation(1000), initial)).not.toThrow()
  for (const mutation of [
    (s: any) => s.bots.pop(), (s: any) => s.bots[22] = s.bots[0], (s: any) => s.bots[0].class++,
    (s: any) => s.generation = "2", (s: any) => s.round.waitingForPlayers = true, (s: any) => s.round.inSetup = true,
    (s: any) => s.round.state = 5, (s: any) => s.at = -1, (s: any) => s.tick = "-1",
    (s: any) => s.failures = "device lost", (s: any) => s.losses.push("validation error"),
  ]) { const changed = observation(1000); mutation(changed); expect(() => checkSustainedObservation(changed, initial)).toThrow() }
})

test("early/late sparse observations retain zero buckets and distinguish live API bytes from creation/upload", () => {
  const first = observation(0), second = observation(1000), late = observation(89000), last = observation(90000)
  second.heap.usedJSHeapSize = 500
  last.gpuApi.created.knownBytes = 200; last.gpuApi.writeTextureSourceBytes = 300
  last.bots[0]!.shots = 20
  const result = sustainedTrends([first, second, late, last], 0, 90000)
  expect(result.early.jsHeapDelta).toBe(-500) // Not relabelled as a GC event.
  expect(result.late.gpuApiLiveBytesDelta).toBe(0)
  expect(result.late.gpuApiCreatedBytes).toBe(100)
  expect(result.late.textureUploadSourceBytes).toBe(200)
  expect(result.observationDelivery.buckets).toHaveLength(90)
  expect(result.observationDelivery.quarterSecondBuckets).toHaveLength(360)
  expect(result.observationDelivery.zeroBuckets).toBe(87)
  expect(result.observationDelivery.maximumGapIncludingBoundaries).toBe(88000)
  expect(result.whole.ticksPerSecond).toBeCloseTo(66.6666667)
  expect(result.whole.shots).toBe(20)
  expect(result.whole.hits).toBeNull()
  expect(() => sustainedTrends(Array.from({ length: 97 }, (_, at) => observation(at)), 0, 90000)).toThrow("bound")
  expect(() => sustainedTrends([second, first], 0, 90000)).toThrow("ordered")
  expect(sustainedTrends([{ ...first, heap: null }], 0, 90000).whole.jsHeapDelta).toBeNull()
})

test("GC evidence uses exact active process/category events, bounds records and never sums nested durations", () => {
  const window = { startedMicroseconds: 1000, endedMicroseconds: 2000, pid: 5 }
  const event = { name: "V8.GC_SCAVENGER", cat: "disabled-by-default-v8.gc", ts: 1100, dur: 20, pid: 5, tid: 7, ph: "X" }
  expect(sustainedGcEvidence([], window, true).status).toBe("unobserved-inconclusive")
  expect(sustainedGcEvidence([event], null, true).count).toBe(0)
  expect(sustainedGcEvidence([event, { ...event, pid: 6 }, { ...event, ts: 999 }, { ...event, ts: 2000 },
    { ...event, cat: "devtools.timeline" }, { ...event, name: "not GC" }], window, true).count).toBe(1)
  const nested = sustainedGcEvidence([event, { ...event, name: "V8.GC_SCAVENGER_SCAVENGE" }], window, false)
  expect(nested.status).toBe("observed-v8-gc-events")
  expect(nested.complete).toBe(false)
  expect(nested).not.toHaveProperty("pauseMilliseconds")
  const overflow = sustainedGcEvidence(Array(4097).fill(event), window, true)
  expect(overflow.count).toBe(4097); expect(overflow.records).toHaveLength(4096); expect(overflow.dropped).toBe(1)
})

test("sustained plans authenticate the target and unchanged create-server roster", () => {
  for (const target of ["koth_sawmill", "koth_lakeside_final"]) {
    const plan = upwardCapturePlan({ PROFILE_KOTH_SUSTAINED: "1", PROFILE_MAP_TARGET: target, PROFILE_SAMPLE_SECONDS: "5" })
    expect(plan).toMatchObject({ target, entry: "create-server", sustainedSeconds: 90, playersOverride: null, warmReload: false, sampleSeconds: 5 })
    expect(() => validateUpwardCapturePlan(plan)).not.toThrow()
    expect(() => validateUpwardCapturePlan({ ...plan, sustainedSeconds: 15 })).toThrow()
  }
  expect(() => upwardCapturePlan({ PROFILE_KOTH_SUSTAINED: "1", PROFILE_MAP_TARGET: "pl_upward" })).toThrow()
  for (const option of ["PROFILE_UPWARD_TRAINING_WARM_RELOAD", "PROFILE_CLASS_REPLACEMENT", "PROFILE_UPWARD_CLASS_SWITCH", "PROFILE_PARTICLE_COMBAT"])
    expect(() => upwardCapturePlan({ PROFILE_KOTH_SUSTAINED: "1", PROFILE_MAP_TARGET: "koth_sawmill", [option]: "1" })).toThrow()
})
