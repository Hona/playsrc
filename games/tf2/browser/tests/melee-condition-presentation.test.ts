import { expect, test } from "bun:test"
import { createMeleeConditionPresenter } from "../src/presentation"
import type { Snapshot } from "../src/codec"

const frame = (tick: bigint, local: readonly number[], remote: readonly number[]): Snapshot => ({
  tick, class: 3, team: 2, lifecycle: 1, position: [0, 0, 0], conditions: local,
  bots: [{ identity: 2, class: 6, team: 3, lifecycle: 1, position: [50, 0, 10], yawDegrees: 0, conditions: remote, overheadHeight: 95 }],
}) as unknown as Snapshot
const none = [0, 0, 0, 0, 0]

test("mark particles follow published actors and stop immediately", () => {
  const presenter = createMeleeConditionPresenter()
  const state = frame(1n, [0, 1, 0, 0, 0], [1 << 30, 0, 0, 0, 0])
  const starts = presenter.map(state)
  expect(starts).toHaveLength(1)
  expect(starts[0]).toMatchObject({ kind: "start", system: "mark_for_death", ownerIdentity: 2, controlPoints: [{ position: [50, 0, 105] }] })
  expect(presenter.map({ ...state, tick: 2n }).map(request => request.kind)).toEqual(["set-control-point"])
  expect(presenter.map(frame(3n, none, none))).toEqual([
    expect.objectContaining({ kind: "stop", effectIdentity: "condition:2:mark_for_death", immediate: true }),
  ])
})

test("stealth suppresses marks until visible again", () => {
  const presenter = createMeleeConditionPresenter()
  presenter.map(frame(1n, none, [1 << 30, 1, 0, 0, 0]))
  const stopped = presenter.map(frame(2n, none, [(1 << 30) | (1 << 4), 1, 0, 0, 0]))
  expect(stopped.map(request => request.kind)).toEqual(["stop"])
  const visible = presenter.map(frame(3n, none, [1 << 30, 1, 0, 0, 0]))
  expect(visible).toHaveLength(1)
  expect(visible[0]).toMatchObject({ kind: "start", system: "mark_for_death" })
  expect(presenter.map(frame(0n, none, [1 << 30, 1, 0, 0, 0]))).toHaveLength(1)
})
