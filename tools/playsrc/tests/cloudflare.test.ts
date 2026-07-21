import { describe, expect, test } from "bun:test"
import { isMissingR2Object } from "../src/cloudflare"

describe("Cloudflare publication output", () => {
  test("classifies only explicit missing-object responses", () => {
    for (const value of [
      "The specified key does not exist.",
      "NoSuchKey",
      "error code 10007",
      "Object not found",
    ]) expect(isMissingR2Object(value)).toBe(true)

    for (const value of [
      "authentication failed",
      "network timeout",
      "bucket not found",
      "permission denied",
    ]) expect(isMissingR2Object(value)).toBe(false)
  })
})
