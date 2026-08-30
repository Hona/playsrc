import { expect, test } from "bun:test"
import { requireSustainedBudget, sustainedKothTarget, SUSTAINED_KOTH, summarizeSustainedWindow, installSustainedObservation } from "../profile/sustained-koth"
import { parseHeadedProfile, profileMinimumRemainingMilliseconds } from "../src/profile-runner"
test("retained entropy is explicit and never forwarded as a Playwright option", () => {
  const identity = "a".repeat(64)
  expect(parseHeadedProfile(["sustained-harvest", "--sustained-entropy", identity]).sustainedEntropy).toBe(identity)
  expect(parseHeadedProfile(["sustained-viaduct", "--sustained-entropy", identity]).playwright).toEqual([])
  expect(profileMinimumRemainingMilliseconds("sustained-harvest")).toBe(165_000)
  expect(() => parseHeadedProfile(["gameplay", "--sustained-entropy", identity])).toThrow()
  expect(() => parseHeadedProfile(["sustained-harvest", "--sustained-entropy", "../bad"])).toThrow()
})
test("sustained KOTH never truncates natural soak to fit a startup overrun", () => {
  expect(SUSTAINED_KOTH.soakMilliseconds).toBe(90_000)
  expect(() => requireSustainedBudget(111_000)).not.toThrow()
  for (const value of [110_999, NaN, -1]) expect(() => requireSustainedBudget(value)).toThrow()
  expect(sustainedKothTarget("koth_viaduct")).toBe("koth_viaduct")
  expect(sustainedKothTarget("koth_harvest_final")).toBe("koth_harvest_final")
  expect(() => sustainedKothTarget("pl_upward")).toThrow()
})
test("whole-interval buckets retain empty seconds and censored input stalls", () => {
  const sample = { frames: [{ at: 1250 }], callbacks: [1250], ticks: [{ at: 1250, before: 100, tick: 104 }], rpc: { records: [] }, inputs: [{ at: 20, completedAt: null }] }
  const result = summarizeSustainedWindow(sample, 0, 3000)
  expect(result.submissions.buckets.map(bucket => bucket.count)).toEqual([0, 1, 0])
  expect(result.submissions.quarterSecondBuckets).toHaveLength(12)
  expect(result.observedTicks).toBe(4)
  expect(result.input.censored[0].milliseconds).toBe(2980)
  expect(result.submissions.terminalGap).toBe(1750)
})

test("continuous observer records real publication changes, input tails, resets and no restarted window", () => {
  let now = 100, changed: () => void = () => {}
  const events = new Map<string, Function>(), documentEvents = new Map<string, Function>()
  const main = { dataset: { snapshotTick: "10", cameraPosition: "0,0,0" } }, canvas = { dataset: { displayFrame: "1" } }
  const host: any = { performance: { now: () => now }, addEventListener: (type: string, fn: Function) => events.set(type, fn),
    document: { querySelector: (selector: string) => selector === "main" ? main : canvas, addEventListener: (type: string, fn: Function) => documentEvents.set(type, fn), visibilityState: "visible", hasFocus: () => true },
    MutationObserver: class { constructor(fn: () => void) { changed = fn } observe() {} disconnect() {} },
    requestAnimationFrame: () => 1, cancelAnimationFrame() {}, __playsrcDeliveryRpc: { start() {}, stop: () => ({ records: [], pending: [], dropped: 0 }) } }
  installSustainedObservation(host)
  const observer = host.__playsrcSustained
  expect(observer.start()).toBe(100)
  documentEvents.get("keydown")!({ type: "keydown", code: "KeyA", isTrusted: true })
  now = 115; canvas.dataset.displayFrame = "2"; main.dataset.snapshotTick = "12"; main.dataset.cameraPosition = "1,0,0"; changed()
  now = 130; main.dataset.snapshotTick = "1"; changed()
  events.get("resize")!({ type: "resize" })
  const sample = observer.stop()
  expect(sample.frames).toHaveLength(1)
  expect(sample.inputs[0].completedAt - sample.inputs[0].at).toBe(15)
  expect(sample.ticks[0]).toEqual({ at: 115, before: 10, tick: 12 })
  expect(sample.lifecycle.map((event: any) => event.type)).toEqual(["counter-reset", "resize"])
  expect(() => observer.start()).toThrow("one uninterrupted")
})
