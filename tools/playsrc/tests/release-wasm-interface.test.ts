import { expect, test } from "bun:test"
import { assertReleaseWasmInterface } from "../src/deploy"

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
