import { expect, test } from "bun:test"
import { decodeEquipmentState, decodeEquippedItems } from "../src/equipment/codec"
import nativeEquipment from "./fixtures/equipment-state.json"

function packet(): Uint8Array {
  const bytes: number[] = []
  const u32 = (value: number) => bytes.push(value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24)
  const text = (value: string) => { const encoded = new TextEncoder().encode(value); u32(encoded.length); bytes.push(...encoded) }
  const item = () => { u32(1); u32(14); u32(13); bytes.push(0, 0, 0, 0) }
  bytes.push(...new TextEncoder().encode("TFEI")); u32(6); u32(0); u32(1)
  item(); bytes.push(4, 1, 1, 0, 4, 0, 2, 0, 0, 1); text("scripts/tf_weapon_scattergun.ctx"); text("#TF_Weapon_Scattergun"); text("Scattergun"); text("backpack/weapons/w_scattergun")
  u32(1); text("Level 1 Scattergun"); text("ItemAttribLevel")
  text(""); u32(0)
  bytes.push(1); text(""); text("models/weapons/c_models/c_scattergun.mdl"); u32(0); u32(0)
  item(); item(); for (let index = 1; index < 9; index++) { u32(0); u32(0) }
  u32(692); bytes.push(...new Uint8Array(692))
  return new Uint8Array(bytes)
}

test("equipment projection preserves canonical definition identity independently of runtime weapon IDs", () => {
  const state = decodeEquipmentState(packet())
  expect(state.inventory[0]).toMatchObject({ weapon: 4, item: { itemId: 14, definitionIndex: 13, slot: 0 },
    modelPlayer: "models/weapons/c_models/c_scattergun.mdl", attachToHands: true, deathNoticeIcon: null })
  expect(state.classes[0]!.items).toEqual([state.inventory[0]!.item])
  expect(state.inventory[0]!.displayName).toBe("Scattergun")
  expect(state.inventory[0]!.description).toEqual([{ text: "Level 1 Scattergun", color: "ItemAttribLevel" }])
  expect(state.inventory[0]!.classSlots).toEqual([{ class: 1, slot: 0, weapon: 4, selectionSlot: 0,
    hud: { script: "scripts/tf_weapon_scattergun.ctx", ammoDisplay: "clip-and-reserve", bucket: 0, position: 0, drawsCrosshair: true, suppressCrosshair: false, countMeter: null } }])
  expect(state.classes.length).toBe(9)
  expect(Object.isFrozen(state.inventory)).toBe(true)
  const shared = new Uint8Array(new SharedArrayBuffer(packet().length)); shared.set(packet())
  expect(decodeEquipmentState(shared)).toEqual(state)
})

test("native inventory transports exact per-class weapon scripts and ammo contracts", () => {
  const state = decodeEquipmentState(new Uint8Array(nativeEquipment))
  const hud = (definition: number, playerClass: number) => state.inventory.find(item => item.item.definitionIndex === definition)!.classSlots.find(slot => slot.class === playerClass)!.hud
  expect(hud(10, 3)).toMatchObject({ script: "scripts/tf_weapon_shotgun_soldier.ctx", ammoDisplay: "clip-and-reserve" })
  expect(hud(12, 7)).toMatchObject({ script: "scripts/tf_weapon_shotgun_pyro.ctx", ammoDisplay: "clip-and-reserve" })
  expect(hud(15, 6)).toMatchObject({ ammoDisplay: "total" })
  expect(hud(25, 9)).toMatchObject({ ammoDisplay: "hidden", suppressCrosshair: true })
  expect(state.classes.every(playerClass => playerClass.baseItems.length > 0)).toBe(true)
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
test("local inventory owns each implemented hitscan unlock exactly once", () => {
  const state = decodeEquipmentState(new Uint8Array(nativeEquipment))
  for (const definitionIndex of [45, 1103, 425, 1153, 415, 424, 312, 41, 61, 460, 220, 402]) {
    const items = state.inventory.filter(item => item.item.definitionIndex === definitionIndex)
    expect(items).toHaveLength(1)
    expect(items[0]!.item).toMatchObject({ itemId: definitionIndex + 1, quality: 6 })
    expect(items[0]!.classSlots.length).toBeGreaterThan(0)
  }
})

test("unavailable projectile solvers and unimplemented items cannot be offered or restored as class base items", () => {
  const state = decodeEquipmentState(new Uint8Array(nativeEquipment))
  expect(state.inventory).toHaveLength(62)
  for (const definition of [19, 20, 735]) {
    expect(state.inventory.some(entry => entry.item.definitionIndex === definition)).toBe(false)
    expect(state.classes.some(player => [...player.items, ...player.baseItems].some(item => item.definitionIndex === definition))).toBe(false)
  }
})

test("the ten executable rocket and flare unlocks each have one inventory item", () => {
  const state = decodeEquipmentState(new Uint8Array(nativeEquipment))
  for (const definition of [127, 228, 237, 414, 513, 1104, 39, 351, 740, 595]) {
    expect(state.inventory.filter(entry => entry.item.definitionIndex === definition)).toHaveLength(1)
  }
})
