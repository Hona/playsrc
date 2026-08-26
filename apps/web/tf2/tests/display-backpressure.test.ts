import { describe, expect, test } from "bun:test"
import { DisplayBackpressure } from "../src/display-backpressure"

describe("fixed-tick browser display backpressure", () => {
  test("coalesces thousands of completed publications without an unbounded queue", () => {
    const display = new DisplayBackpressure()
    display.begin()
    expect(display.defer()).toBe(true)
    for (let index = 0; index < 10_000; index += 1) expect(display.defer()).toBe(false)
    expect(display.pending).toBe(true)
    expect(display.complete()).toBe(false)
    expect(display.pending).toBe(false)
  })

  test("recovers a completed presentation that missed its compositor opportunity", () => {
    const display = new DisplayBackpressure()
    display.advance()
    display.begin()
    display.advance()
    display.defer()
    expect(display.complete()).toBe(true)
    display.begin()
    expect(display.complete()).toBe(false)
  })

  test("never chains repeated GPU submissions within one animation callback", () => {
    const display = new DisplayBackpressure()
    display.advance()
    display.begin()
    display.defer()
    expect(display.complete()).toBe(false)
    display.advance()
    display.begin()
    expect(display.complete()).toBe(false)
  })

  test("rejects overlapping render owners and forgets stale generations", () => {
    const display = new DisplayBackpressure()
    display.begin()
    expect(() => display.begin()).toThrow("already in progress")
    display.defer()
    display.reset()
    expect(display.pending).toBe(false)
    expect(display.complete()).toBe(false)
    display.begin()
    expect(display.complete()).toBe(false)
  })
})
