import { expect, test } from "bun:test"
import { RequiredParticleDisplayQueue } from "../src/particle-display"

test("requires one display frame when a Particle effect first produces render items", () => {
  const queue = new RequiredParticleDisplayQueue<string>(4, 2)
  queue.admit("empty", [])
  queue.admit("trail-first", [10])
  queue.admit("trail-follow", [10])
  queue.admit("trail-plus-impact", [10, 20, 20])
  queue.admit("impact-follow", [20])
  expect(queue.peek()).toBe("trail-first")
  queue.complete("trail-first")
  expect(queue.peek()).toBe("trail-first")
  queue.complete("trail-first")
  expect(queue.peek()).toBe("trail-plus-impact")
  queue.complete("trail-plus-impact")
  expect(queue.peek()).toBe("trail-plus-impact")
  queue.complete("trail-plus-impact")
  expect(queue.peek()).toBeUndefined()
})

test("bounds and resets required Particle display frames", () => {
  const queue = new RequiredParticleDisplayQueue<string>(1, 2)
  expect(() => queue.admit("invalid", [0])).toThrow("identity is invalid")
  queue.admit("first", [1])
  expect(() => queue.admit("second", [2])).toThrow("explicit limit")
  queue.reset()
  queue.admit("replacement", [2])
  expect(queue.peek()).toBe("replacement")
})
