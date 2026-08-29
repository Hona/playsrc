import { expect, test } from "bun:test"
import { requireSustainedBudget } from "../profile/sustained-koth"
import { upwardCapturePlan, validateUpwardCapturePlan } from "../profile/upward-capture-plan"
import { profileMinimumRemainingMilliseconds } from "../src/profile-runner"

test("sustained admission never truncates the 90 second soak to fit", () => {
  for (const budget of [NaN, 90_000, 119_999]) expect(() => requireSustainedBudget(budget)).toThrow()
  expect(() => requireSustainedBudget(120_000)).not.toThrow()
  expect(profileMinimumRemainingMilliseconds("koth-sustained-sawmill")).toBe(150_000)
  expect(profileMinimumRemainingMilliseconds("koth-sustained-lakeside")).toBe(150_000)
})

test("sustained plans authenticate the target and unchanged create-server roster", () => {
  for (const target of ["koth_sawmill", "koth_lakeside_final"]) {
    const plan = upwardCapturePlan({ PROFILE_KOTH_SUSTAINED: "1", PROFILE_MAP_TARGET: target, PROFILE_SAMPLE_SECONDS: "5" })
    expect(plan).toMatchObject({ target, entry: "create-server", sustainedSeconds: 90, playersOverride: null, warmReload: false, sampleSeconds: 5 })
    expect(() => validateUpwardCapturePlan(plan)).not.toThrow()
    expect(() => validateUpwardCapturePlan({ ...plan, sustainedSeconds: 15 })).toThrow()
  }
  expect(() => upwardCapturePlan({ PROFILE_KOTH_SUSTAINED: "1", PROFILE_MAP_TARGET: "pl_upward" })).toThrow()
})
