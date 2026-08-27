import { expect, test } from "bun:test"
import { weaponCountMeter } from "../src/hud/weapon-count"
import { nativeEquipment } from "./fixtures/equipment"
import type { Tf2WeaponHud } from "../src/equipment/types"

test("count selection is schema-backed, keeps full counts and obeys active-only specialization", () => {
  const stock = nativeEquipment.inventory.find(item => item.item.definitionIndex === 14)!
  const source: Parameters<typeof weaponCountMeter>[0] = {
    lifecycle: 1, class: 2, weapon: 12, conditions: [0, 0, 0, 0, 0],
    equippedItems: [stock.item], decapitations: 800, revengeCrits: 35,
  }
  const catalog = (countMeter: Tf2WeaponHud["countMeter"]) => [{ ...stock,
    classSlots: stock.classSlots.map(slot => ({ ...slot, hud: { ...slot.hud!, countMeter } })),
  }]
  const heads = catalog("heads"), kills = catalog("kills"), revenge = catalog("revenge-active")
  expect(weaponCountMeter(source, heads)).toEqual({ kind: "heads", count: 800 })
  expect(weaponCountMeter({ ...source, weapon: 13 }, heads)).toEqual({ kind: "heads", count: 800 })
  expect(weaponCountMeter(source, kills)).toEqual({ kind: "kills", count: 800 })
  expect(weaponCountMeter(source, revenge)).toEqual({ kind: "crits", count: 35 })
  expect(weaponCountMeter({ ...source, weapon: 13 }, revenge)).toBeNull()
  expect(weaponCountMeter({ ...source, weapon: 13 }, catalog("revenge"))).toEqual({ kind: "crits", count: 35 })
  expect(weaponCountMeter(source, nativeEquipment.inventory)).toBeNull()
  expect(weaponCountMeter({ ...source, lifecycle: 2 }, heads)).toBeNull()
  expect(weaponCountMeter({ ...source, conditions: [0, 0, 1 << 13, 0, 0] }, heads)).toBeNull()
  expect(weaponCountMeter({ ...source, equippedItems: [] }, heads)).toBeNull()
})
