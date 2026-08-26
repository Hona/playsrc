import { describe, expect, test } from "bun:test"
import { assertCompatibleBun, SetupError } from "../src/setup"

describe("declared Bun toolchain compatibility", () => {
  test("accepts the exact declared public version across executable revisions", () => {
    expect(() => assertCompatibleBun("1.4.0", "a".repeat(40))).not.toThrow()
    expect(() => assertCompatibleBun("1.4.0", "b".repeat(40))).not.toThrow()
  })

  test("rejects different public versions with truthful executable diagnostics", () => {
    expect(() => assertCompatibleBun("1.4.1", "custom-revision")).toThrow(SetupError)
    expect(() => assertCompatibleBun("1.4.1", "custom-revision"))
      .toThrow("Bun 1.4.0 is required; found 1.4.1 (custom-revision)")
  })
})
