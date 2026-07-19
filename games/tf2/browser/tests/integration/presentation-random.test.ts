import { describe, expect, test } from "bun:test"
import { createTf2PresentationRandom } from "../../src/ui-integration"

describe("TF2 presentation random stream", () => {
  test("matches the fixed shuffled Source stream and restores every field", () => {
    const random = createTf2PresentationRandom(0)
    expect(Array.from({ length: 8 }, () => random.nextInteger(0, 32_767))).toEqual([
      30_600, 363, 3_853, 14_151, 2_821, 6_296, 7_638, 18_772,
    ])
    const checkpoint = random.snapshot()
    const expected = Array.from({ length: 8 }, () => random.nextInteger(0, 17))
    random.restore(checkpoint)
    expect(Array.from({ length: 8 }, () => random.nextInteger(0, 17))).toEqual(expected)
    expect(random.snapshot().draws).toBe(checkpoint.draws + 8)
  })

  test("validates seed, interval and complete restore state", () => {
    expect(() => createTf2PresentationRandom(2 ** 40)).toThrow("seed")
    const random = createTf2PresentationRandom(12_345)
    expect(() => random.nextInteger(4, 3)).toThrow("interval")
    expect(() => random.restore({ ...random.snapshot(), shuffle: [1] })).toThrow("snapshot")
  })
})
