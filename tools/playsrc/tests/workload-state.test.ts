import { expect, test } from "bun:test"
import { workloadState, assertMatchingWorkloadState, canonicalWorkloadState } from "../profile/workload-state"
import { profileMinimumRemainingMilliseconds, requireBrowserBudget } from "../src/profile-runner"

test("nominally identical rosters cannot admit different phase, poses, scene or inputs", () => {
  const frame = { schema: 3, round: { state: 4, waitingForPlayers: true, endTick: "2000" }, tick: 31, position: [1, 2, 3], yaw: 90, pitch: 0,
    modelInputs: [{ identity: 2, bones: [1, 0, 0] }], particleInputs: [{ identity: 9, position: [0, 1, 0] }],
    detail: { bots: 23, buildings: 0, pickups: 12, particleItems: 1, particleBatches: 1 } }
  const baseline = workloadState(frame)
  expect(() => assertMatchingWorkloadState(baseline, structuredClone(baseline))).not.toThrow()
  for (const change of [
    (v: any) => v.visibleSourceTick = "33",
    (v: any) => v.round.endTick = "1999",
    (v: any) => v.frame.models[0].bones[1] = 0.01,
    (v: any) => v.frame.particles.inputs[0].position[0] = 0.01,
    (v: any) => v.frame.yaw = 90.001,
  ]) { const changed = structuredClone(baseline); change(changed); expect(() => assertMatchingWorkloadState(baseline, changed)).toThrow("rejected") }
})
test("queue admission reserves the selected actual warm workflow budget", () => {
  const ordinary = profileMinimumRemainingMilliseconds("upward-training-bots", { PROFILE_UPWARD_TRAINING_WARM_RELOAD: "1" })
  const workload = profileMinimumRemainingMilliseconds("upward-training-bots", { PROFILE_COMMAND_WORKLOAD: "a".repeat(64) })
  expect(ordinary).toBe(90000); expect(workload).toBe(120000)
  expect(() => requireBrowserBudget(40124, ordinary)).toThrow("reserve")
  expect(() => requireBrowserBudget(120000, workload)).not.toThrow()
})
test("state authentication retains bigint, typed view bytes, negative zero and fractional bits", () => {
  const state = canonicalWorkloadState({ identity: 0xffff_ffff_ffff_ffffn, pose: new Float32Array([-0, 0.1]), x: -0, y: 0.1 })
  expect(() => JSON.stringify(state)).not.toThrow()
  expect(state.identity).toEqual({ uint64: "18446744073709551615" })
  expect(state.x).not.toEqual(canonicalWorkloadState(0))
  expect(state.y).not.toEqual(canonicalWorkloadState(0.10000001))
  expect(state.pose.bytes).toHaveLength(8)
})
