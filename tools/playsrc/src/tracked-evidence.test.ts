import { expect, test } from "bun:test"
import { checkMediaPaths, isMediaPath } from "./tracked-evidence"

test("generated captures are rejected anywhere, regardless of extension case", () => {
  for (const file of ["before.PNG", "docs/after.JpEg", "tasks/research/demo.MOV", "evidence/clip.WeBm", "capture.AVIF", "test-output/frame.TIFF"]) {
    expect(isMediaPath(file)).toBe(true)
    expect(() => checkMediaPaths([file], [])).toThrow("must remain transient")
  }
  checkMediaPaths(["evidence/measurements.json", "docs/evidence.md"], [])
})

test("reviewed source fixtures require a tracked consumer and a reason", () => {
  const files = ["tests/fixtures/decoder.png", "tests/decoder.test.ts"]
  const fixture = { path: files[0], reason: "Authored decoder input", consumers: [files[1]] }
  checkMediaPaths(files, [fixture])
  for (const invalid of [{ ...fixture, reason: "" }, { ...fixture, consumers: [] }, { ...fixture, consumers: ["missing.ts"] }, { ...fixture, consumers: [files[0]] }]) {
    expect(() => checkMediaPaths(files, [invalid])).toThrow("Invalid")
  }
  expect(() => checkMediaPaths(files, [fixture, fixture])).toThrow("Invalid")
  expect(() => checkMediaPaths([], [fixture])).toThrow("Invalid")
})
