import { expect, test } from "bun:test"
import {
  PhysicalButtonState,
  applyPointerDelta,
  rawPointerMovementUnsupported,
  rebasePointerYaw,
  resolvePhysicalBinding,
  type PhysicalBinding,
} from "../src/input"
import {
  MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS,
  SimulationClockQueue,
} from "../src/simulation-clock"

test("positive horizontal and vertical pointer deltas turn right and down", () => {
  const ordinary = applyPointerDelta(180, -1, 64, 32)
  expect(ordinary.yaw).toBeCloseTo(175.776)
  expect(ordinary.pitch).toBeCloseTo(1.112)
  expect(applyPointerDelta(0, 88, -64, 32)).toEqual({ yaw: 4.224, pitch: 89 })
})

test("teleport yaw preserves only movement received after its command sample", () => {
  expect(rebasePointerYaw(90, 1_000, 1_064)).toBeCloseTo(85.776)
  expect(rebasePointerYaw(-170, 1_000, 936)).toBeCloseTo(-165.776)
  expect(rebasePointerYaw(45, 1_000, 1_000)).toBe(45)
})

test("rejects malformed mouse input before poisoning the view state", () => {
  expect(() => applyPointerDelta(0, 0, Number.NaN, 0)).toThrow()
  expect(() => rebasePointerYaw(0, 0, Number.POSITIVE_INFINITY)).toThrow()
})

test("retries adjusted pointer lock only when raw movement is unsupported", () => {
  expect(rawPointerMovementUnsupported({ name: "NotSupportedError" })).toBe(true)
  expect(rawPointerMovementUnsupported({ name: "NotAllowedError" })).toBe(false)
  expect(rawPointerMovementUnsupported(new Error("denied"))).toBe(false)
})

test("keeps unmodified physical bindings active under unrelated modifiers", () => {
  const base = Object.freeze([
    Object.freeze({ action: "+forward", code: "w", modifiers: 0 }),
  ] satisfies PhysicalBinding[])
  const chords = Object.freeze([
    ...base,
    Object.freeze({ action: "+use", code: "w", modifiers: 1 }),
  ] satisfies PhysicalBinding[])
  const read = (binding: PhysicalBinding): PhysicalBinding => binding

  expect(resolvePhysicalBinding("W", 1, base, read)).toEqual({ action: "+forward", match: "unmodified" })
  expect(resolvePhysicalBinding("w", 2, base, read)).toEqual({ action: "+forward", match: "unmodified" })
  expect(resolvePhysicalBinding("w", 1, chords, read)).toEqual({ action: "+use", match: "exact" })
})

test("retains simultaneous physical-key actions until their own releases", () => {
  const buttons = new PhysicalButtonState()
  expect(buttons.press("keyboard:KeyW", "+forward")).toBe(true)
  expect(buttons.press("keyboard:KeyA", "+moveleft")).toBe(true)
  expect(buttons.press("keyboard:ShiftLeft", "+duck")).toBe(true)
  expect(buttons.press("keyboard:KeyW", "+forward")).toBe(false)
  expect(buttons.held("+forward")).toBe(true)
  expect(buttons.held("+moveleft")).toBe(true)
  expect(buttons.held("+duck")).toBe(true)

  expect(buttons.release("keyboard:KeyW")).toBe(true)
  expect(buttons.held("+forward")).toBe(false)
  expect(buttons.held("+moveleft")).toBe(true)
  expect(buttons.held("+duck")).toBe(true)
  expect(buttons.release("keyboard:ShiftLeft")).toBe(true)
  expect(buttons.release("keyboard:KeyA")).toBe(true)
  expect(buttons.held("+moveleft")).toBe(false)
  expect(buttons.held("+duck")).toBe(false)
})

test("keeps an action held until every physical source releases", () => {
  const buttons = new PhysicalButtonState()
  expect(buttons.press("keyboard:ShiftLeft", "+duck")).toBe(true)
  expect(buttons.press("keyboard:ShiftRight", "+duck")).toBe(false)
  expect(buttons.release("keyboard:ShiftLeft")).toBe(false)
  expect(buttons.held("+duck")).toBe(true)
  expect(buttons.release("keyboard:ShiftRight")).toBe(true)
  expect(buttons.held("+duck")).toBe(false)

  buttons.press("mouse:0", "+attack")
  buttons.clear()
  expect(buttons.release("mouse:0")).toBe(false)
  expect(buttons.held("+attack")).toBe(false)
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
