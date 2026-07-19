import { expect, test } from "bun:test"
import { FramePacingController } from "../src/frame-pacing"

test("bounds frame opportunities, coalesces in-flight work, records suspension, and surfaces failure", async () => {
  let now = 10
  const pacing = new FramePacingController({ now: () => now }, 4)
  expect((await pacing.offer(0, async () => ({ submitMilliseconds: 1, presentMilliseconds: 2 }))).disposition).toBe("stopped")
  pacing.start()
  let release!: () => void
  const pending = new Promise<void>((resolve) => { release = resolve })
  const first = pacing.offer(16, async () => {
    await pending
    now = 12
    return { submitMilliseconds: 11, presentMilliseconds: 12 }
  })
  expect((await pacing.offer(17, async () => ({ submitMilliseconds: 20, presentMilliseconds: 21 }))).disposition).toBe("coalesced")
  release()
  expect((await first).disposition).toBe("accepted")
  pacing.suspend(true)
  expect((await pacing.offer(32, async () => ({ submitMilliseconds: 33, presentMilliseconds: 34 }))).disposition).toBe("suspended")
  pacing.suspend(false)
  await expect(pacing.offer(48, async () => { throw new Error("sentinel") })).rejects.toThrow(/sentinel/i)
  pacing.stop()
  await pacing.offer(64, async () => ({ submitMilliseconds: 65, presentMilliseconds: 66 }))
  expect(pacing.records()).toHaveLength(4)
  expect(pacing.records().map((record) => record.disposition)).toEqual(["accepted", "suspended", "accepted", "stopped"])
})
