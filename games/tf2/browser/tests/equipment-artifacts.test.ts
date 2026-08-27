import { test, expect } from "bun:test"
import { parseEquipmentModelArtifacts } from "../src/artifacts"

function emptyRegistry(): Uint8Array {
  const bytes = new Uint8Array(72), view = new DataView(bytes.buffer)
  const header = (offset: number, name: string, version: number) => { bytes.set(new TextEncoder().encode(name), offset); view.setUint32(offset + 4, version, true) }
  header(0, "PEQM", 2); header(12, "PMST", 2); header(24, "PMDL", 2); header(36, "PMIP", 2)
  view.setUint32(48, 4, true)
  header(60, "PPTM", 2)
  return bytes
}

test("equipment registry has explicit model and particle sections without a fabricated map", () => {
  const bytes = emptyRegistry(), decoded = parseEquipmentModelArtifacts(bytes, new Map())
  expect(decoded.geometry).toEqual([])
  expect(decoded.models.size).toBe(0)
  expect(decoded.particleMaterials).toEqual([])
  expect(decoded.particleTextures).toEqual([])
  for (let size = 0; size < bytes.length; size++) expect(() => parseEquipmentModelArtifacts(bytes.subarray(0, size), new Map())).toThrow()
  new DataView(bytes.buffer).setUint32(4, 1, true)
  expect(() => parseEquipmentModelArtifacts(bytes, new Map())).toThrow("equipment model identity")
})
