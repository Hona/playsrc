import { expect, test } from "bun:test"
import { execFileSync } from "node:child_process"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { checkMediaPaths, checkTrackedEvidence, isMediaPath } from "./tracked-evidence"

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

test("the git index guard rejects force-added media but ignores transient captures", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-media-index-"))
  try {
    const git = (...args: string[]) => execFileSync("git", args, { cwd: root })
    git("init", "-q")
    await writeFile(path.join(root, "media-fixtures.json"), "[]\n")
    await writeFile(path.join(root, ".gitignore"), "*.[pP][nN][gG]\n")
    await writeFile(path.join(root, "before.PNG"), "test capture bytes")
    git("add", ".gitignore", "media-fixtures.json")
    checkTrackedEvidence(root)
    git("add", "-f", "before.PNG")
    expect(() => checkTrackedEvidence(root)).toThrow("before.PNG")
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
