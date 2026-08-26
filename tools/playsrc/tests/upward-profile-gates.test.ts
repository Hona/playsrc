import { expect, test } from "bun:test"
import { assertUpwardProfile } from "../profile/upward-profile-gates"

const policy = { expectedBots: 15, playerCount: 16, classes: true, classPasses: 2 as const, smooth: true, compositor: true, sourceUnchanged: true, workerCaptures: [{ deadlineStopped: false }] }
function report() {
  return {
    activeBots: 15, teams: { red: 8, blue: 8 }, completedFrames: 560, elapsedMilliseconds: 10_000, pixels: { nonBlack: 30_000 },
    classSwitches: { requested: Array(18).fill(1), observed: Array(18).fill(1), visibleScoreboardRows: 16,
      timing: Array.from({ length: 18 }, (_, index) => ({ admission: index < 9 ? "first" : "retained", fireAt: index + 1 })) },
    simulation: { hertz: 66.667 }, compositor: { evidence: "chromium-compositor-presentation-trace", intervals: { maximumMilliseconds: 133.332 } },
    compositorSilence: { maximumActiveSilenceMilliseconds: 133.332, maximumCensoredBoundaryMilliseconds: 0 },
    workerIncidents: [{ samples: 4000, captureComplete: true }], screen: { visibility: "visible" }, gpu: { losses: [] },
    compositorEvidence: { complete: true }, applicationCompletedFramesPerSecond: 56, frameIntervals: { p95Milliseconds: 25 },
  }
}
test("headed and offline gates reject duration, cadence, pixels, lost edges, missing native data and boundary stalls", () => {
  expect(() => assertUpwardProfile(report(), policy)).not.toThrow()
  const changes: Array<(value: any) => void> = [
    value => { value.elapsedMilliseconds = 4999 }, value => { value.simulation.hertz = 59.9 },
    value => { value.pixels.nonBlack = 20000 }, value => { value.classSwitches.observed.pop() },
    value => { value.classSwitches.timing[0].fireAt = null }, value => { value.classSwitches.timing[0].fireAt = undefined },
    value => { value.classSwitches.visibleScoreboardRows = 15 }, value => { value.compositor.intervals = undefined },
    value => { value.compositorSilence.maximumActiveSilenceMilliseconds = 250 }, value => { value.compositorSilence.maximumCensoredBoundaryMilliseconds = 250 },
    value => { value.compositorEvidence.complete = false }, value => { value.applicationCompletedFramesPerSecond = 54.99 },
    value => { value.frameIntervals.p95Milliseconds = 35 }, value => { value.workerIncidents[0].captureComplete = false },
    value => { value.screen.visibility = "hidden" }, value => { value.gpu.losses.push("lost") },
  ]
  for (const change of changes) { const value = report(); change(value); expect(() => assertUpwardProfile(value, policy)).toThrow() }
  expect(() => assertUpwardProfile(report(), { ...policy, sourceUnchanged: false })).toThrow()
  expect(() => assertUpwardProfile(report(), { ...policy, workerCaptures: [{ deadlineStopped: true }] })).toThrow()
})
