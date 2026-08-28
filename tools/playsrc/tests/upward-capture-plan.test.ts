import { expect, test } from "bun:test"
import { upwardCapturePlan, validateUpwardCapturePlan, assertMatchingCapturePlans } from "../profile/upward-capture-plan"

test("steady gameplay can retain owner replay without class input or cadence changes", () => {
  const normal = upwardCapturePlan({ PROFILE_INTEGRATED_ACCEPTANCE: "1" })
  const retained = upwardCapturePlan({ PROFILE_INTEGRATED_ACCEPTANCE: "1", PROFILE_GAMEPLAY_REPLAY: "1" })
  expect(retained).toEqual({ ...normal, gameplayReplay: "required" })
  expect(() => validateUpwardCapturePlan(retained)).not.toThrow()
  expect(() => assertMatchingCapturePlans(normal, retained)).toThrow("differ")
  expect(() => validateUpwardCapturePlan({ ...retained, gameplayReplay: "optional" })).toThrow()
  expect(upwardCapturePlan({ PROFILE_ACCEPTANCE_STOCK_ONLY: "1", PROFILE_GAMEPLAY_REPLAY: "1" }).gameplayReplay).toBeUndefined()
})

test("render owner instrumentation is explicit, bounded and comparison-plan significant", () => {
  const normal = upwardCapturePlan({}), owners = upwardCapturePlan({ PROFILE_RENDER_OWNERS: "1" })
  expect(owners.renderOwners).toBe("two-frames-after-60-v1")
  expect(normal.renderOwners).toBeUndefined()
  expect(() => validateUpwardCapturePlan(owners)).not.toThrow()
  expect(() => assertMatchingCapturePlans(normal, owners)).toThrow("plans differ")
  expect(() => assertMatchingCapturePlans(owners, normal)).toThrow("plans differ")
  expect(() => assertMatchingCapturePlans(owners, upwardCapturePlan({ PROFILE_RENDER_OWNERS: "1" }))).not.toThrow()
})

test("replacement is an explicit comparison dimension without rewriting archived v1 plans", () => {
  const ordinary = upwardCapturePlan({ PROFILE_UPWARD_CLASS_SWITCH: "1" })
  const replacement = upwardCapturePlan({ PROFILE_UPWARD_CLASS_SWITCH: "1", PROFILE_CLASS_REPLACEMENT: "1" })
  expect(replacement.replacement).toBe(true)
  expect(() => assertMatchingCapturePlans(ordinary, replacement)).toThrow("differ")
  const { replacement: _, ...fields } = ordinary
  const historical = { ...fields, schema: "playsrc-upward-capture-plan-v1" }
  expect(() => validateUpwardCapturePlan(historical)).not.toThrow()
  expect(() => assertMatchingCapturePlans(historical, historical)).not.toThrow()
  expect(() => assertMatchingCapturePlans(historical, ordinary)).toThrow("differ")
})

test("one effective plan owns existing scenario, interaction and Worker switches", () => {
  const ordinary = upwardCapturePlan({ PROFILE_UPWARD_TRAINING_INTERACTION: "1" })
  expect(ordinary).toMatchObject({ target: "pl_upward", entry: "training", interaction: "movement-weapon", workerCpu: "not-requested", sampleSeconds: 6 })
  const classes = upwardCapturePlan({ PROFILE_UPWARD_CLASS_SWITCH: "1", PROFILE_UPWARD_TRAINING_INTERACTION: "1" })
  expect(classes).toMatchObject({ interaction: "class-input", classPasses: 2, workerCpu: "required" })
  const acceptance = upwardCapturePlan({ PROFILE_INTEGRATED_ACCEPTANCE: "1" })
  expect(acceptance).toMatchObject({ exerciseClasses: false, interaction: "forward-movement", workerCpu: "required", coldTeam: "red", warmTeam: "blue" })
  const stock = upwardCapturePlan({ PROFILE_INTEGRATED_ACCEPTANCE: "1", PROFILE_UPWARD_CLASS_SWITCH: "1", PROFILE_ACCEPTANCE_STOCK_ONLY: "1" })
  expect(stock).toMatchObject({ interaction: "stock-loadouts", workerCpu: "not-requested", sampleSeconds: null, classPasses: 0 })
  const server = upwardCapturePlan({ PROFILE_STARTUP_CREATE_SERVER: "1", PROFILE_UPWARD_TRAINING_PLAYERS: "16", PROFILE_PARTICLE_COMBAT: "1", PROFILE_SAMPLE_SECONDS: "5" })
  expect(server).toMatchObject({ target: "ctf_2fort", entry: "create-server", playersOverride: null, combat: true, sampleSeconds: 5 })
  for (const plan of [ordinary, classes, acceptance, stock, server]) expect(() => validateUpwardCapturePlan(plan)).not.toThrow()
  expect(() => assertMatchingCapturePlans(classes, upwardCapturePlan({ PROFILE_UPWARD_CLASS_SWITCH: "1" }))).not.toThrow() // Suppressed interaction flag is not effective.
  expect(() => assertMatchingCapturePlans(ordinary, acceptance)).toThrow("plans differ")
  expect(() => assertMatchingCapturePlans(null, null)).toThrow("unknown")
  for (const change of [{ workerCpu: "required" }, { sampleSeconds: 11 }, { entry: "create-server" }, { classPasses: 1 }, { warmTeam: "blue" }, { alias: true }]) {
    expect(() => validateUpwardCapturePlan({ ...ordinary, ...change })).toThrow()
  }
})
