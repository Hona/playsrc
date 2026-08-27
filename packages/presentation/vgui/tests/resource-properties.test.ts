import { expect, test } from "bun:test"
import { resourcePropertyReader } from "../src/resource-properties"

test("scalar indexing preserves first duplicate, empty, ASCII-only and absent values", () => {
  let reads = 0
  const properties = [
    { name: "Wide", get value() { reads++; return "42" } },
    { name: "WIDE", value: "999" }, { name: "text", value: "" },
    { name: "Ä", value: "upper" }, { name: "ä", value: "lower" },
  ]
  const first = resourcePropertyReader(properties)
  for (let index = 0; index < 100; index++) {
    expect(first("wide")).toBe("42")
    expect(first("TEXT")).toBe("")
    expect(first("Ä")).toBe("upper")
    expect(first("ä")).toBe("lower")
    expect(first("missing")).toBeNull()
  }
  expect(reads).toBe(1)
})
