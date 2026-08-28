import { expect, test } from "bun:test"
import { retireTransferredInputs } from "../src/transferred-input"

test("message input backing stores retire only after exact bounded WASM copies", () => {
  const source = Uint8Array.from([1, 2, 3, 4]).buffer
  const target = new Uint8Array(8).fill(9)
  target.set(new Uint8Array(source), 2)
  retireTransferredInputs([source])
  expect([...target]).toEqual([9, 9, 1, 2, 3, 4, 9, 9])
  expect(source.byteLength).toBe(0)
  expect(source.detached).toBe(true)
  const rejected = Uint8Array.from([5, 6]).buffer
  expect(() => target.set(new Uint8Array(rejected), 7)).toThrow()
  expect([...new Uint8Array(rejected)]).toEqual([5, 6])
})

test("batch input aliases remain readable until all copies finish and retire once", () => {
  const source = Uint8Array.from([7, 8]).buffer
  const target = new Uint8Array(4)
  target.set(new Uint8Array(source), 0)
  target.set(new Uint8Array(source), 2)
  retireTransferredInputs([source, source])
  expect([...target]).toEqual([7, 8, 7, 8])
  expect(source.detached).toBe(true)
})
