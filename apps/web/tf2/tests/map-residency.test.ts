import { expect, test } from "bun:test"
import { mapResidency } from "../src/map-residency"

test("map residency separates active, candidate and exact shared ranges without counting the whole WASM heap", () => {
  const wasm = new SharedArrayBuffer(4096)
  const active = { generation: 1, payload: new Uint8Array(50), sections: [new Uint8Array(wasm, 0, 16), new Uint8Array(wasm, 32, 32)] }
  const candidate = { generation: 2, presentation: new Uint8Array(10), sections: [new Uint8Array(wasm, 0, 16), new Uint8Array(wasm, 64, 64)] }
  const result = mapResidency(active, candidate)
  expect(result.sharedSourceBytes).toBe(16)
  expect(result.active).toEqual({ generation: 1, canonicalBytes: 50, presentationBytes: 0, sourceReferencedBytes: 48, sourceExclusiveBytes: 32 })
  expect(result.candidate).toEqual({ generation: 2, canonicalBytes: 0, presentationBytes: 10, sourceReferencedBytes: 80, sourceExclusiveBytes: 64 })
  expect(mapResidency(active).active?.sourceExclusiveBytes).toBe(48)
  expect(mapResidency(candidate).active?.sourceExclusiveBytes).toBe(80)
  expect(mapResidency(active, { ...candidate, sections: [new Uint8Array(16)] }).sharedSourceBytes).toBe(0)
})
