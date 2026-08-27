import { expect, test } from "bun:test"
import { classInputViolations, prepareClassCapture } from "../profile/class-input-sequence"

test("class publication and Source deploy run during the native capture cooldown, not after it", async () => {
  let now = 100, selectedAt = 0
  const accepted = await prepareClassCapture({
    earliestCapture: 2100, deadline: 2300,
    now: () => now,
    delay: async milliseconds => { now += milliseconds },
    select: async () => { now += 400; selectedAt = now; return true },
  })
  expect(accepted).toBe(true)
  expect(selectedAt).toBe(500)
  expect(now).toBe(2100)
  // A real 0.5-second Source deploy can elapse without shortening it.
  expect(now - selectedAt).toBeGreaterThanOrEqual(500)
})

test("idle or secondary input and scripted DOM events cannot impersonate controlled primary edges", () => {
  const capture = { at: 10, phase: "pointer-capture", button: 0, trusted: true, controllerAction: "capture" }
  const attack = { ...capture, phase: "weapon-fire", controllerAction: "attack" }
  const key = { at: 1, phase: "key-down", key: "Comma", trusted: true, controllerAction: "select" }
  expect(classInputViolations([capture, attack, key])).toEqual([])
  for (const event of [
    { ...capture, controllerAction: "none" }, { ...attack, controllerAction: "capture" },
    { ...attack, button: 2 }, { ...attack, trusted: false },
    { ...attack, phase: "other-pointer-button", button: 2 },
    { ...key, key: "KeyW" }, { ...key, controllerAction: "none" },
  ]) expect(classInputViolations([event])).toEqual([event])
  // This classifier only reports evidence. It never dispatches/cancels input.
  expect(capture.controllerAction).toBe("capture")
})

test("missing class acknowledgement or insufficient bounded capture time never succeeds", async () => {
  const base = { earliestCapture: 2100, deadline: 2000, now: () => 100,
    delay: async () => { throw new Error("must not wait past deadline") }, select: async () => true }
  expect(await prepareClassCapture(base)).toBe(false)
  expect(await prepareClassCapture({ ...base, earliestCapture: 0, select: async () => false })).toBe(false)
})
