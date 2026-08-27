import { test, expect } from "bun:test"
import { parseEquipmentModelArtifacts } from "../src/artifacts"

function emptyRegistry(): Uint8Array {
  const bytes = new Uint8Array(72), view = new DataView(bytes.buffer)
  const header = (offset: number, name: string, version: number) => { bytes.set(new TextEncoder().encode(name), offset); view.setUint32(offset + 4, version, true) }
  header(0, "PEQM", 2); header(12, "PMST", 2); header(24, "PMDL", 4); header(36, "PMIP", 2)
  view.setUint32(48, 4, true)
  header(60, "PPTM", 3)
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

test("PMDL4 transports model Refract parameters without substituting an opaque shader", () => {
  const values: number[] = []
  const u32 = (value: number) => { const bytes = new Uint8Array(4); new DataView(bytes.buffer).setUint32(0, value, true); values.push(...bytes) }
  const float = (value: number) => { const bytes = new Uint8Array(4); new DataView(bytes.buffer).setFloat32(0, value, true); values.push(...bytes) }
  const name = new TextEncoder().encode("materials/lens.vmt")
  u32(name.length); values.push(...name, 6, 0, 9, 0); u32(0)
  values.push(...new Uint8Array(24))
  u32(0)
  for (let index = 0; index < 16; index++) float(index % 5 === 0 ? 1 : 0)
  float(0.15); float(1); float(1); float(1)
  values.push(0, 0, 0, 0, 1, 2, 1, 0, 6)
  const base = emptyRegistry(), bytes = new Uint8Array(base.length + values.length)
  bytes.set(base.subarray(0, 36)); bytes.set(values, 36); bytes.set(base.subarray(36), 36 + values.length)
  new DataView(bytes.buffer).setUint32(32, 1, true)
  const material = parseEquipmentModelArtifacts(bytes, new Map()).modelMaterials.get("materials/lens.vmt")!
  expect(material).toMatchObject({ shader: "refract", opacity: "translucent", framebuffer: "current", requiredInputs: ["current-framebuffer"], state: { kind: "refract", normalFrame: 0, blurAmount: 0, ignoreDepth: false } })
  expect((material.state as { refractAmount: number }).refractAmount).toBeCloseTo(0.15)
})
