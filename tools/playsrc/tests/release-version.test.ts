import { expect, test } from "bun:test"
import { validateReleaseVersion } from "../src/release-version"

test("default next patch and explicitly selected next minor", () => {
  validateReleaseVersion("0.0.13", "0.0.12", ["v0.0.12"])
  validateReleaseVersion("0.1.0", "0.0.12", ["v0.0.12"], "minor")
  validateReleaseVersion("0.1.1", "0.1.0", ["v0.1.0"])
  validateReleaseVersion("1.3.0", "1.2.9", [], "minor")
  validateReleaseVersion("0.10.1", "0.10.0", ["v0.9.99", "v0.10.0"])
})

test("reject skipped, reused, major, floating, malformed and implicit minor versions", () => {
  for (const version of ["0.1.0", "0.0.14", "0.0.12", "1.0.0", "latest", "v0.0.13", "0.0.013", "0.0.13-rc.1"]) expect(() => validateReleaseVersion(version, "0.0.12", [])).toThrow()
  expect(() => validateReleaseVersion("0.1.1", "0.0.12", [], "minor")).toThrow()
  expect(() => validateReleaseVersion("0.1.0", "0.0.12", ["v0.1.0"], "minor")).toThrow()
  expect(() => validateReleaseVersion("0.0.13", "0.0.12", ["v0.0.14"])).toThrow()
  expect(() => validateReleaseVersion("0.0.13", "0.0.12", [], "major")).toThrow()
  expect(() => validateReleaseVersion("0.0.13", "bad", [])).toThrow()
  expect(() => validateReleaseVersion("0.1.1", "0.1.0", ["v0.2.0"])).toThrow()
  expect(() => validateReleaseVersion("0.9.1", "0.9.0", ["v0.10.0"])).toThrow()
})
