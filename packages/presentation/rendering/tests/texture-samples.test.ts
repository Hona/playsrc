import { describe, expect, test } from "bun:test"
import { sourceTextureSamples } from "../src/texture-samples"

describe("authored GPU texture sample ownership", () => {
  test("keeps authored RGBA and compressed byte planes source-backed", () => {
    const backing = new Uint8Array([9, 1, 2, 3, 4, 8])
    const plane = backing.subarray(1, 5)
    expect(sourceTextureSamples(plane, 0, "u8")).toBe(plane)
    expect(sourceTextureSamples(plane, 13, "u8")).toBe(plane)
  })

  test("converts only the selected authored RGB or BGR plane without changing source channel order", () => {
    expect([...sourceTextureSamples(new Uint8Array([1, 2, 3, 4, 5, 6]), 2, "u8")]).toEqual([
      1, 2, 3, 255, 4, 5, 6, 255,
    ])
    expect([...sourceTextureSamples(new Uint8Array([3, 2, 1, 6, 5, 4]), 3, "u8")]).toEqual([
      1, 2, 3, 255, 4, 5, 6, 255,
    ])
  })

  test("views aligned authored half-float planes without staging a second copy", () => {
    const source = new Uint8Array([9, 9, 0, 60, 0, 64])
    const plane = source.subarray(2)
    const samples = sourceTextureSamples(plane, 24, "f16")
    expect(samples).toBeInstanceOf(Uint16Array)
    expect(samples.buffer).toBe(source.buffer)
    expect([...samples]).toEqual([0x3c00, 0x4000])
  })

  test("copies only unaligned half-float planes and rejects malformed RGB input", () => {
    const source = new Uint8Array([9, 0, 60, 0, 64])
    const samples = sourceTextureSamples(source.subarray(1), 24, "f16")
    expect(samples.buffer).not.toBe(source.buffer)
    expect([...samples]).toEqual([0x3c00, 0x4000])
    expect(() => sourceTextureSamples(new Uint8Array([1, 2]), 3, "u8")).toThrow(/invalid/)
  })
})
