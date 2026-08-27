import { expect, test } from "bun:test"
import { circularProgressClip } from "../src/circular-progress"

test("circular foreground sweep starts at the top and passes each clockwise corner", () => {
  expect(circularProgressClip(0, 100, 100)).toBe("polygon(0 0, 0 0, 0 0)")
  expect(circularProgressClip(0.25, 100, 100)).toBe("polygon(50px 50px, 50px 0px, 100px 0px, 100px 50px, 100px 50px)")
  expect(circularProgressClip(0.5, 100, 100)).toBe("polygon(50px 50px, 50px 0px, 100px 0px, 100px 50px, 100px 100px, 50px 100px, 50px 100px)")
  expect(circularProgressClip(1, 100, 100)).toBe("none")
})
