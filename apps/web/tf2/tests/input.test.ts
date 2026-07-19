import { expect, test } from "bun:test"
import { applyPointerDelta } from "../src/input"
import {
  MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS,
  SimulationClockQueue,
} from "../src/simulation-clock"

test("positive horizontal and vertical pointer deltas turn right and down", () => {
  const ordinary = applyPointerDelta(180, -1, 64, 32)
  expect(ordinary.yaw).toBeCloseTo(174.88)
  expect(ordinary.pitch).toBeCloseTo(1.56)
  expect(applyPointerDelta(0, 88, -64, 32)).toEqual({ yaw: 5.12, pitch: 89 })
})

test("coalesces a 400-sample continuous browser clock into its latest value", () => {
  const queue = new SimulationClockQueue()
  for (let sample = 0; sample < 400; sample += 1) {
    queue.push({ generation: 1, nowSeconds: sample / 400, suspended: false })
  }
  expect(queue.length).toBe(1)
  expect(queue.shift()).toEqual({ generation: 1, nowSeconds: 399 / 400, suspended: false })
})

test("preserves suspension transitions while coalescing each continuous state", () => {
  const queue = new SimulationClockQueue()
  queue.push({ generation: 1, nowSeconds: 1, suspended: false })
  queue.push({ generation: 1, nowSeconds: 2, suspended: false })
  queue.push({ generation: 1, nowSeconds: 3, suspended: true })
  queue.push({ generation: 1, nowSeconds: 4, suspended: true })
  queue.push({ generation: 1, nowSeconds: 5, suspended: false })
  expect([queue.shift(), queue.shift(), queue.shift()]).toEqual([
    { generation: 1, nowSeconds: 2, suspended: false },
    { generation: 1, nowSeconds: 4, suspended: true },
    { generation: 1, nowSeconds: 5, suspended: false },
  ])
})

test("bounds unconsumed lifecycle transitions", () => {
  const queue = new SimulationClockQueue()
  for (let sample = 0; sample < MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS; sample += 1) {
    queue.push({ generation: 1, nowSeconds: sample, suspended: sample % 2 === 0 })
  }
  expect(() => queue.push({ generation: 1, nowSeconds: 64, suspended: true })).toThrow()
})
