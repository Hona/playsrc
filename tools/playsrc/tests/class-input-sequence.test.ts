import { expect, test } from "bun:test"
import { prepareClassCapture } from "../profile/class-input-sequence"

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

test("missing class acknowledgement or insufficient bounded capture time never succeeds", async () => {
  const base = { earliestCapture: 2100, deadline: 2000, now: () => 100,
    delay: async () => { throw new Error("must not wait past deadline") }, select: async () => true }
  expect(await prepareClassCapture(base)).toBe(false)
  expect(await prepareClassCapture({ ...base, earliestCapture: 0, select: async () => false })).toBe(false)
})
