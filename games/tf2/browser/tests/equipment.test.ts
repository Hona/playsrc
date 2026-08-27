import { expect, test } from "bun:test"
import { decodeEquipmentState, decodeEquippedItems } from "../src/equipment/codec"

function packet(): Uint8Array {
  const bytes: number[] = []
  const u32 = (value: number) => bytes.push(value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24)
  const text = (value: string) => { const encoded = new TextEncoder().encode(value); u32(encoded.length); bytes.push(...encoded) }
  const item = () => { u32(1); u32(14); u32(13); bytes.push(0, 0, 0, 0) }
  bytes.push(...new TextEncoder().encode("TFEI")); u32(2); u32(0); u32(1)
  item(); bytes.push(4, 1, 1, 0, 4); text("#TF_Weapon_Scattergun"); text("Scattergun"); text("backpack/weapons/w_scattergun")
  u32(1); text("Level 1 Scattergun"); text("ItemAttribLevel")
  bytes.push(1); text(""); text("models/weapons/c_models/c_scattergun.mdl"); u32(0); u32(0)
  item(); for (let index = 1; index < 9; index++) u32(0)
  u32(692); bytes.push(...new Uint8Array(692))
  return new Uint8Array(bytes)
}

test("equipment projection preserves canonical definition identity independently of runtime weapon IDs", () => {
  const state = decodeEquipmentState(packet())
  expect(state.inventory[0]).toMatchObject({ weapon: 4, item: { itemId: 14, definitionIndex: 13, slot: 0 },
    modelPlayer: "models/weapons/c_models/c_scattergun.mdl", attachToHands: true, deathNoticeIcon: null })
  expect(state.classes[0]!.items).toEqual([state.inventory[0]!.item])
  expect(state.classes.length).toBe(9)
  expect(Object.isFrozen(state.inventory)).toBe(true)
  const shared = new Uint8Array(new SharedArrayBuffer(packet().length)); shared.set(packet())
  expect(decodeEquipmentState(shared)).toEqual(state)
})

test("equipment projections reject truncated records and invalid stable identities", () => {
  const bytes = packet()
  for (let length = 0; length < bytes.length; length++) expect(() => decodeEquipmentState(bytes.subarray(0, length))).toThrow()
  new DataView(bytes.buffer).setUint32(20, 0, true)
  expect(() => decodeEquipmentState(bytes)).toThrow("invalid equipped item")
})

test("equipped actor attributes preserve quality, slot and finite particle effect values", () => {
  const bytes = new Uint8Array(24), view = new DataView(bytes.buffer)
  view.setUint32(0, 1, true); view.setUint32(4, 379, true); view.setUint32(8, 378, true)
  bytes.set([5, 0, 8, 1], 12); view.setUint32(16, 134, true); view.setFloat32(20, 13, true)
  expect(decodeEquippedItems(view, 0)).toEqual({ end: 24, items: [{ itemId: 379, definitionIndex: 378, quality: 5, style: 0, slot: 8,
    attributes: [{ definition: 134, value: 13 }] }] })
  view.setFloat32(20, NaN, true)
  expect(() => decodeEquippedItems(view, 0)).toThrow("invalid item attribute")
})
