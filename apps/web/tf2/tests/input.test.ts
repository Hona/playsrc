import { expect, test } from "bun:test"
import {
  PhysicalBindingIndex,
  PhysicalButtonState,
  applyPointerDelta,
  pointerLockRequestRequired,
  rawPointerMovementUnsupported,
  rebasePointerYaw,
  sourceMouseButtonCode,
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

test("maps browser primary, auxiliary, secondary, and navigation buttons to exact Source bindings", () => {
  expect([0, 1, 2, 3, 4].map(sourceMouseButtonCode)).toEqual(["MOUSE1", "MOUSE3", "MOUSE2", "MOUSE4", "MOUSE5"])
  for (const button of [-1, 5, 1.5, Number.NaN]) expect(sourceMouseButtonCode(button)).toBeNull()
})

test("ordinary primary clicks never reacquire an already-owned pointer lock", () => {
  const canvas = {} as Element
  const other = {} as Element
  expect(pointerLockRequestRequired(null, canvas)).toBe(true)
  expect(pointerLockRequestRequired(other, canvas)).toBe(true)
  expect(pointerLockRequestRequired(canvas, canvas)).toBe(false)

  const buttons = new PhysicalButtonState()
  expect(buttons.press("mouse:0", "+attack")).toBe(true)
  expect(pointerLockRequestRequired(canvas, canvas)).toBe(false)
  expect(buttons.held("+attack")).toBe(true)
  expect(buttons.release("mouse:0")).toBe(true)
})

test("keeps unmodified physical bindings active under unrelated modifiers", () => {
  const base = Object.freeze([
    Object.freeze({ action: "+forward", code: "w", modifiers: 0 }),
  ] satisfies PhysicalBinding[])
  const chords = Object.freeze([
    ...base,
    Object.freeze({ action: "+use", code: "w", modifiers: 1 }),
  ] satisfies PhysicalBinding[])
  const index = new PhysicalBindingIndex()
  index.replace(base)
  expect(index.resolve("W", 1)).toEqual({ action: "+forward", match: "unmodified" })
  expect(index.resolve("w", 2)).toEqual({ action: "+forward", match: "unmodified" })
  index.replace(chords)
  expect(index.resolve("w", 1)).toEqual({ action: "+use", match: "exact" })
})

test("binding hints resolve the same current physical owner as cancellation", () => {
  const index = new PhysicalBindingIndex()
  expect(index.lookupBinding("lastinv")).toBeNull()
  index.replace([{ action: "lastinv", code: "q", modifiers: 0 }])
  expect(index.lookupBinding("lastinv")).toBe("q")
  index.replace([{ action: "lastinv", code: "F8", modifiers: 0 }])
  expect(index.lookupBinding("lastinv")).toBe("F8")
  expect(index.resolve("q", 0)).toBeNull()
  expect(index.resolve("F8", 0)?.action).toBe("lastinv")
  index.replace([{ action: "lastinv", code: "F8", modifiers: 2 }, { action: "+attack", code: "F8", modifiers: 2 }])
  expect(index.lookupBinding("lastinv")).toBeNull()
  index.clear()
  expect(index.lookupBinding("lastinv")).toBeNull()
})

test("indexes exact physical chords without rebuilding settings snapshots per input", () => {
  const index = new PhysicalBindingIndex()
  index.replace([
    { action: "+forward", code: "w", modifiers: 0 },
    { action: "+use", code: "W", modifiers: 1 },
    { action: "+attack", code: "MOUSE1", modifiers: 0 },
  ])
  expect(index.resolve("W", 0)).toEqual({ action: "+forward", match: "exact" })
  expect(index.resolve("w", 1)).toEqual({ action: "+use", match: "exact" })
  expect(index.resolve("w", 2)).toEqual({ action: "+forward", match: "unmodified" })
  expect(index.resolve("mouse1", 4)).toEqual({ action: "+attack", match: "unmodified" })
  expect(index.resolve("x", 0)).toBeNull()
  expect(index.resolve("w", 1)).toBe(index.resolve("w", 1))

  index.replace([{ action: "+back", code: "s", modifiers: 0 }])
  expect(index.resolve("w", 0)).toBeNull()
  expect(index.resolve("S", 0)).toEqual({ action: "+back", match: "exact" })
  index.clear()
  expect(index.resolve("s", 0)).toBeNull()
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

test("release identifies the original binding even after repeats, rebind and context teardown", () => {
  const buttons = new PhysicalButtonState()
  buttons.press("keyboard:Tab", "+showscores")
  buttons.press("mouse:1", "+showscores")
  for (let repeat = 0; repeat < 20; repeat++) expect(buttons.press("keyboard:Tab", "+jump")).toBe(false)
  expect(buttons.action("keyboard:Tab")).toBe("+showscores")
  expect(buttons.action("mouse:1")).toBe("+showscores")
  buttons.release("keyboard:Tab")
  expect(buttons.action("keyboard:Tab")).toBeUndefined()
  expect(buttons.held("+showscores")).toBe(true)
  buttons.clear()
  expect(buttons.action("mouse:1")).toBeUndefined()
  expect(buttons.release("mouse:1")).toBe(false)
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

test("keeps real primary-fire and jump edges while hundreds of busy clock samples coalesce", () => {
  const queue = new SimulationClockQueue()
  const buttons = new PhysicalButtonState()
  let firePressed = false
  let jumpPressed = false
  for (let sample = 0; sample < 400; sample += 1) {
    queue.push({ generation: 3, nowSeconds: sample * 0.015, suspended: false })
    if (sample === 117 && buttons.press("mouse:0", "+attack")) firePressed = true
    if (sample === 118) buttons.release("mouse:0")
    if (sample === 219 && buttons.press("keyboard:Space", "+jump")) jumpPressed = true
    if (sample === 220) buttons.release("keyboard:Space")
  }
  expect(queue.length).toBe(1)
  expect(queue.shift()).toEqual({ generation: 3, nowSeconds: 399 * 0.015, suspended: false })
  expect(buttons.held("+attack")).toBe(false)
  expect(buttons.held("+jump")).toBe(false)
  expect({ firePressed, jumpPressed }).toEqual({ firePressed: true, jumpPressed: true })
})

test("bounds unconsumed lifecycle transitions", () => {
  const queue = new SimulationClockQueue()
  for (let sample = 0; sample < MAX_PENDING_SIMULATION_CLOCK_TRANSITIONS; sample += 1) {
    queue.push({ generation: 1, nowSeconds: sample, suspended: sample % 2 === 0 })
  }
  expect(() => queue.push({ generation: 1, nowSeconds: 64, suspended: true })).toThrow()
})
