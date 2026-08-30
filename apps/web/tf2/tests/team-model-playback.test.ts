import { expect, test } from "bun:test"
import { teamModelPlayback, TeamModelSkinning } from "../src/team-model-playback"

const panel = { sequence: "hoveropen", modelRevision: 1, animationRevision: 1 }
test("samples exact terminal pose even when a render interval crosses its duration", () => {
  const initial = teamModelPlayback(undefined, panel, 1, 30 / 65)
  const middle = { ...initial.state, sampledSeconds: 0.1, previousSeconds: 0.1 }
  const terminal = teamModelPlayback(middle, panel, 2, 30 / 65)
  expect(terminal.sample).toBe(true)
  expect(terminal.elapsed).toBe(30 / 65)
  expect(teamModelPlayback({ ...terminal.state, sampledSeconds: terminal.elapsed }, panel, 3, 30 / 65).sample).toBe(false)
})
test("idle, same-sequence commands, interrupted changes and recreated panels cannot reuse an old endpoint", () => {
  const done = { ...teamModelPlayback(undefined, panel, 1, 1).state, sampledSeconds: 1 }
  for (const next of [
    { ...panel, sequence: "idle", animationRevision: 2 },
    { ...panel, animationRevision: 2 },
    { ...panel, modelRevision: 2 },
  ]) {
    const step = teamModelPlayback(done, next, 1.02, 1)
    expect(step.sample).toBe(true)
    expect(step.elapsed).toBeCloseTo(next.modelRevision === panel.modelRevision ? 0.02 : 0, 8)
    expect(step.previousElapsed).toBe(0)
  }
  expect(teamModelPlayback(undefined, { ...panel, sequence: "idle" }, 0, 0).sample).toBe(true)
})

test("short sequences keep sampling until the studio transition owner has finished fading", () => {
  const hover = { ...panel, sequence: "hover" }
  const initial = teamModelPlayback(undefined, hover, 1, 5 / 30)
  const ended = { ...initial.state, sampledSeconds: 5 / 30, transitioning: true }
  expect(teamModelPlayback(ended, hover, 1.18, 5 / 30).sample).toBe(true)
  expect(teamModelPlayback({ ...ended, transitioning: false }, hover, 1.21, 5 / 30).sample).toBe(false)
})

test("retains exact template pixels until deformation and restores idle through the same skinned owner", () => {
  const state = new TeamModelSkinning()
  const idle = { sequence: 0, cycle: 0, boneMatrices: Float32Array.of(1, 0, 0, 2), flex: [] }
  state.observe("door", idle)
  expect(state.needsPose("door")).toBe(false)
  state.observe("door", { ...idle, cycle: 1, boneMatrices: idle.boneMatrices.slice() })
  expect(state.needsPose("door")).toBe(false)
  state.observe("door", { ...idle, sequence: 1, boneMatrices: Float32Array.of(0.5, 0, 0, 2) })
  expect(state.needsPose("door")).toBe(true)
  state.observe("door", idle)
  expect(state.needsPose("door")).toBe(true)
  state.observe("disabled-first", { ...idle, sequence: 1 })
  expect(state.needsPose("disabled-first")).toBe(true)
  state.clear()
  state.observe("door", idle)
  expect(state.needsPose("door")).toBe(false)
})
