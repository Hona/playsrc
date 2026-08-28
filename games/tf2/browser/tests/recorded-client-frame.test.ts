import { expect, test } from "bun:test"
import { createClientRenderFrameClock, RecordedClientRenderFrames, type RecordedClientRenderFrame } from "../src/client-render-frame"

test("records accepted clocks only and replays exact deltas at real deadlines", async () => {
  const records: RecordedClientRenderFrame[] = []
  const original = createClientRenderFrameClock(frame => records.push(frame))
  original.prepare(10).accept()
  original.prepare(10.01) // abandoned before acceptance, not an input transaction
  original.prepare(10.02).accept()
  expect(records).toHaveLength(2)
  let now = 100
  const player = new RecordedClientRenderFrames(records, () => now, async ms => { now += ms })
  const clock = createClientRenderFrameClock()
  for (const record of records) {
    const next = await player.next(-9900)
    const actual = clock.prepare(next.nowSeconds)
    expect(actual.clientFrameSeconds).toBe(record.clientFrameSeconds)
    expect(actual.reset).toBe(record.reset)
    actual.accept(); player.accept(next.clientFrame)
  }
  expect(player.ended).toBe(true)
  expect(now).toBeCloseTo(120, 8)
})
test("a late callback stays late and cancellation rejects rather than resetting or freezing", async () => {
  let now = 200
  const frames = [{ clientFrame: 1, acceptedClientFrame: 0, nowSeconds: 10, clientFrameSeconds: 0, reset: true }]
  const player = new RecordedClientRenderFrames(frames, () => now, async ms => { now += ms })
  await player.next(-9900)
  expect(player.observations[0]?.lateMilliseconds).toBe(100)
  player.close()
  await expect(player.next(-9900)).rejects.toThrow("unavailable")
})
