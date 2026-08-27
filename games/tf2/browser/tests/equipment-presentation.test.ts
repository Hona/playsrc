import { expect, test } from "bun:test"
import { createRoot, byName } from "../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "./gameui-integration/fixture"
import { Tf2EquipmentPresentation } from "../src/equipment/presentation"
import type { Tf2EquipmentState } from "../src/equipment/types"
import { nativeEquipment } from "./fixtures/equipment"

test("item tooltip has visible bounds after the authored hover delay", () => {
  const fixture = createTf2GameUiTransitionFixture(), root = createRoot(fixture.document)
  const resources = { ...fixture.resources, clientScheme: { ...fixture.resources.clientScheme, borders: ["Econ.Button.Border.Default", "BackpackItemBorder", "BackpackItemMouseOverBorder"].map(name => ({
    kind: "line" as const, name, inset: { left: 0, top: 0, right: 0, bottom: 0 }, backgroundType: 0 as const, paintFirst: false,
    sides: { left: [], top: [], right: [], bottom: [] },
  })) } }
  let now = 0
  const ui = new Tf2EquipmentPresentation({ root: root as unknown as HTMLElement, resources,
    viewport: { width: 1280, height: 800, devicePixelRatio: 1 }, reducedMotion: true,
    clock: { nowSeconds: () => now }, random: { nextUnit: () => 0 }, onClose() {}, onPreview() {}, onEquip: async () => nativeEquipment })
  ui.show(nativeEquipment, 3)
  byName(root, "Itemslot-0").focus()
  now = 0.099; ui.frame(now)
  expect(ui.snapshot().vgui.panels.some(panel => panel.name === "ItemTooltip")).toBe(false)
  now = 0.1; ui.frame(now)
  const tooltip = ui.snapshot().vgui.panels.find(panel => panel.name === "ItemTooltip")!
  expect(tooltip).toBeDefined()
  expect(tooltip.visible).toBe(true)
  expect(tooltip.bounds.width).toBeGreaterThan(0)
  expect(tooltip.bounds.height).toBeGreaterThan(0)
  expect(tooltip.bounds.x).toBeGreaterThanOrEqual(0)
  expect(tooltip.bounds.y + tooltip.bounds.height).toBeLessThanOrEqual(800)
  ui.destroy()
})

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
