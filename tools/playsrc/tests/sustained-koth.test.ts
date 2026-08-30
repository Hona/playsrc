import { expect, test } from "bun:test"
import { requireSustainedBudget, sustainedKothTarget, SUSTAINED_KOTH, summarizeSustainedWindow } from "../profile/sustained-koth"
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
