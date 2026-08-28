import { expect, test } from "bun:test"
import { sampleSetupFrames } from "../profile/setup-frame-sample"

test("setup sampling keeps the first interval and five real seconds without mixing rAF and callback clocks", async () => {
  const times = [100, 101, 1100, 5100, 5101]
  const result = await sampleSetupFrames(() => times.shift()!, callback => { callback(90); return 0 })
  expect(result).toEqual({seconds:5.001,frames:[1,999,4000]})
  expect(times).toEqual([])
})
