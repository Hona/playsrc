import { expect, test } from "bun:test"
import { createRoot } from "../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "./gameui-integration/fixture"
import { Tf2EquipmentPresentation } from "../src/equipment/presentation"
import type { Tf2EquipmentState } from "../src/equipment/types"

test("authored equipment roots construct class controls and class-slot panels from the Rust projection", () => {
  const fixture = createTf2GameUiTransitionFixture()
  // Structural unit fixture only; real border pixels are verified headed.
  const resources = { ...fixture.resources, clientScheme: { ...fixture.resources.clientScheme, borders: ["Econ.Button.Border.Default", "BackpackItemBorder"].map(name => ({
    kind: "line" as const, name, inset: { left: 0, top: 0, right: 0, bottom: 0 }, backgroundType: 0 as const, paintFirst: false,
    sides: { left: [], top: [], right: [], bottom: [] },
  })) } }
  const state = { revision: 0, inventory: [], classes: Array.from({ length: 9 }, (_, index) => ({ class: index + 1, items: [], baseItems: [] })), persistence: new Uint8Array(692) } as Tf2EquipmentState
  const ui = new Tf2EquipmentPresentation({ root: createRoot(fixture.document) as unknown as HTMLElement, resources,
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
    clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0 }, onClose() {}, onPreview() {}, onEquip: async () => state })
  ui.show(state)
  expect(ui.snapshot().vgui.panels.filter(panel => /^Class[1-9]$/.test(panel.name))).toHaveLength(9)
  ui.show(state, 8)
  expect(ui.snapshot().vgui.panels.filter(panel => panel.name.startsWith("Itemslot-")).map(panel => panel.name)).toEqual([
    "Itemslot-1", "Itemslot-2", "Itemslot-6", "Itemslot-4", "Itemslot-7", "Itemslot-8", "Itemslot-10", "Itemslot-9",
  ])
  expect(ui.snapshot().vgui.panels.find(panel => panel.name === "EquipmentPlayer")?.bounds).toMatchObject({ width: 405, height: 510 })
  ui.destroy()
})
