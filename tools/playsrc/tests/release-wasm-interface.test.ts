import { expect, spyOn, test } from "bun:test"
import { assertReleaseStartupAcceptance, assertReleaseWasmInterface } from "../src/deploy"

const moduleBytes = (name: number, value: number) => new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0,
  1, 5, 1, 96, 0, 1, 127,
  3, 2, 1, 0,
  7, 5, 1, 1, name, 0, 0,
  10, 6, 1, 4, 0, 65, value, 11,
])

test("release serves approved bytes without requiring identical compiler output", () => {
  const compiled = moduleBytes(102, 1)
  const approved = moduleBytes(102, 2)
  expect(compiled).not.toEqual(approved)
  expect(() => assertReleaseWasmInterface(compiled, approved)).not.toThrow()
})

test("release refuses missing or renamed binding exports and invalid modules", () => {
  expect(() => assertReleaseWasmInterface(moduleBytes(102, 1), moduleBytes(103, 1))).toThrow("import/export contract")
  expect(() => assertReleaseWasmInterface(moduleBytes(102, 1), new Uint8Array([0]))).toThrow()
})

test("startup receipt waiver requires explicit authorization for exactly v0.0.12", () => {
  const expected = { packageSha256: "a".repeat(64), wasmSha256: "b".repeat(64) }
  for (const environment of [
    {},
    { PLAYSRC_RELEASE_VERSION: "0.0.12" },
    { PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT: "true" },
    { PLAYSRC_RELEASE_VERSION: "0.0.12", PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT: "false" },
    { PLAYSRC_RELEASE_VERSION: "0.0.13", PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT: "true" },
    { PLAYSRC_RELEASE_VERSION: "0.0.11", PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT: "true" },
    { PLAYSRC_RELEASE_VERSION: "0.0.12", PLAYSRC_WAIVE_V0011_STARTUP_RECEIPT: "true" },
  ]) expect(() => assertReleaseStartupAcceptance(expected, environment)).toThrow("startup receipt")
  expect(() => assertReleaseStartupAcceptance(expected, { PLAYSRC_STATIC_STARTUP_RECEIPT: "{}" })).toThrow("Startup receipt identity mismatch")
  const warning = spyOn(console, "warn").mockImplementation(() => {})
  try {
    expect(() => assertReleaseStartupAcceptance(expected, { PLAYSRC_RELEASE_VERSION: "0.0.12", PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT: "true" })).not.toThrow()
    expect(warning).toHaveBeenCalledWith(expect.stringContaining("headed startup is not certified"))
  } finally { warning.mockRestore() }
})
