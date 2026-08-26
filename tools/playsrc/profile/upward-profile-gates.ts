import assert from "node:assert/strict"

/** Shared by headed capture and immutable offline re-analysis. No weaker replay gates. */
export function assertUpwardProfile(report: any, policy: {
  expectedBots: number; playerCount: number; classes: boolean; classPasses: 1 | 2; smooth: boolean; compositor: boolean;
  sourceUnchanged: boolean; workerCaptures: readonly { deadlineStopped?: boolean }[];
}) {
  assert.equal(report.activeBots, policy.expectedBots, "bot roster")
  assert.equal(report.teams.red + report.teams.blue, policy.playerCount, "team population")
  assert(report.completedFrames > 0, "no completed frames")
  assert(report.elapsedMilliseconds >= 5000, "active sample duration")
  assert(report.pixels.nonBlack > 20_000, "visible pixels missing")
  if (policy.classes) {
    assert.equal(report.classSwitches.requested.length, policy.classPasses * 9, "all requested class edges")
    assert.equal(report.classSwitches.observed.length, policy.classPasses * 9, "all observed class edges")
    assert.equal(report.classSwitches.timing.length, report.classSwitches.observed.length, "class timing coverage")
    assert(report.classSwitches.timing.every((item: any) => Number.isFinite(item.fireAt)), "all class fire edges")
    assert.equal(report.classSwitches.timing.filter((item: any) => item.admission === "first").length, 9, "first class admissions")
    assert.equal(report.classSwitches.timing.filter((item: any) => item.admission === "retained").length, (policy.classPasses - 1) * 9, "retained class admissions")
    assert.equal(report.classSwitches.visibleScoreboardRows, policy.playerCount, "visible scoreboard")
    assert(report.simulation.hertz >= 60, "simulation frequency")
    assert(report.compositor.intervals?.maximumMilliseconds < 250, "native compositor gap")
    assert(report.compositorSilence?.maximumActiveSilenceMilliseconds < 250, "boundary-overlapping active silence")
    assert(report.compositorSilence?.maximumCensoredBoundaryMilliseconds < 250, "censored boundary coverage")
    assert.equal(policy.workerCaptures.length, 1, "actual Worker capture")
    assert.equal(policy.workerCaptures[0]!.deadlineStopped, false, "Worker deadline")
    assert(report.workerIncidents[0]?.samples > 0, "actual Worker CPU samples")
    assert.equal(report.workerIncidents[0]?.captureComplete, true, "Worker capture completeness")
  }
  assert.equal(report.screen.visibility, "visible", "visible headed window")
  assert.equal(report.gpu.losses.length, 0, "GPU losses")
  assert.equal(report.compositorEvidence.complete, true, "complete immutable compositor evidence")
  assert.equal(policy.sourceUnchanged, true, "source identity changed during capture")
  if (policy.compositor) assert.equal(report.compositor.evidence, "chromium-compositor-presentation-trace", "native presentation evidence")
  if (policy.smooth) {
    assert(report.applicationCompletedFramesPerSecond >= 55, "application completed frame frequency")
    assert(report.simulation.hertz >= 60, "simulation frequency")
    assert(report.frameIntervals.p95Milliseconds < 35, "frame interval tail")
  }
}
