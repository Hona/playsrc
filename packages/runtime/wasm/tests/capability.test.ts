import { expect, spyOn, test } from "bun:test"
import { supportsSimd128 } from "../src"

test("standard SIMD admission exercises bytecode, not browser or GPU identity", () => {
  expect(supportsSimd128()).toBe(true)
  const validate = spyOn(WebAssembly, "validate").mockReturnValue(false)
  try { expect(supportsSimd128()).toBe(false); expect(validate).toHaveBeenCalledTimes(1) }
  finally { validate.mockRestore() }
})
