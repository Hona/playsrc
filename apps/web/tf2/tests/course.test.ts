import { expect, test } from "bun:test"
import { jumpBeefCourse } from "../src/course"

test("declares one exact map-bound start, checkpoint sequence, and end", () => {
  const bytes = jumpBeefCourse("ab".repeat(32))
  const view = new DataView(bytes.buffer)
  expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("PJMP")
  expect(view.getUint32(4, true)).toBe(1)
  expect(view.getUint32(48, true)).toBe(21)
  expect(view.getUint32(52 + 4, true)).toBe(316)
  expect(view.getUint32(52 + 20 * 16 + 4, true)).toBe(257)
})
