import { describe, expect, test } from "bun:test"
import { parseModelOccurrenceMatrices } from "../src/artifacts"

function section(magic = "PMTX", version = 3, littleEndian = true): Uint8Array {
  const bytes = new Uint8Array(12)
  bytes.set(new TextEncoder().encode(magic))
  new DataView(bytes.buffer).setUint32(4, version, littleEndian)
  return bytes
}

describe("StudioModel occurrence presentation artifact identity", () => {
  test("accepts only the exact current little-endian PMTX producer schema", () => {
    expect(parseModelOccurrenceMatrices(section())).toEqual([])
  })

  test("rejects stale producer versions, substituted magic, and reversed Windows-equivalent byte order", () => {
    for (const bytes of [section("PMTX", 2), section("PTF2"), section("PMTX", 3, false)]) {
      expect(() => parseModelOccurrenceMatrices(bytes)).toThrow("PMTX identity")
    }
  })

  test("rejects partial and extended cached PMTX generations", () => {
    expect(() => parseModelOccurrenceMatrices(section().subarray(0, 7))).toThrow()
    expect(() => parseModelOccurrenceMatrices(Uint8Array.from([...section(), 0]))).toThrow("PMTX trailing bytes")
  })
})
