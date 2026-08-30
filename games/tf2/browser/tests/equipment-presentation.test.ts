import { expect, test } from "bun:test"
import { createRoot, byName, pointer, FakeEvent } from "../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "./gameui-integration/fixture"
import { Tf2EquipmentPresentation } from "../src/equipment/presentation"
import type { Tf2EquipmentState } from "../src/equipment/types"
import { nativeEquipment } from "./fixtures/equipment"

function navigationFixture(onEquip: (playerClass: number, slot: number, definition: number | null, signal: AbortSignal) => Promise<Tf2EquipmentState> = async () => nativeEquipment) {
  const fixture = createTf2GameUiTransitionFixture(), root = createRoot(fixture.document)
  const resources = { ...fixture.resources, clientScheme: { ...fixture.resources.clientScheme, borders: ["Econ.Button.Border.Default", "BackpackItemBorder", "BackpackItemBorder_Unique", "BackpackItemBorder_4", "BackpackItemSelectedBorder"].map(name => ({
    kind: "line" as const, name, inset: { left: 0, top: 0, right: 0, bottom: 0 }, backgroundType: 0 as const, paintFirst: false, sides: { left: [], top: [], right: [], bottom: [] },
  })) } }
  const errors: unknown[] = [], errorCurrent: boolean[] = []
  let closes = 0
  const ui = new Tf2EquipmentPresentation({ root: root as unknown as HTMLElement, resources,
    viewport: { width: 1280, height: 800, devicePixelRatio: 1 }, reducedMotion: true,
    clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0 }, onClose() { closes++ }, onPreview() {}, onError(error, current) { errors.push(error); errorCurrent.push(current) }, onEquip })
  const activate = (name: string, fraction = 0.5) => {
    const bounds = ui.snapshot().vgui.panels.find(panel => panel.name === name)!.bounds
    // Dispatch at the root: VGUI, not the DOM fixture, must find the target.
    const viewport = byName(root, "EquipmentViewport")
    pointer(viewport, "pointerdown", bounds.x + bounds.width * fraction, bounds.y + bounds.height * fraction)
    pointer(viewport, "pointerup", bounds.x + bounds.width * fraction, bounds.y + bounds.height * fraction)
    ui.frame(0)
  }
  const key = (code: string, repeat = false) => ui.handleKey({ code, repeat, preventDefault() {}, stopImmediatePropagation() {} })
  return { ui, root, activate, key, escape: (repeat = false) => key("Escape", repeat), errors, errorCurrent, closes: () => closes }
}

test("loadout Back is hit across its full button", () => {
  const { ui, activate } = navigationFixture()
  try {
    for (const fraction of [0.05, 0.5, 0.95]) {
      ui.show(nativeEquipment); activate("Class3")
      activate("BackButton", fraction)
      expect(ui.snapshot().page).toBe("classes")
    }
  } finally { ui.destroy() }
})

test("pending equip cannot swallow Back or pull a reopened page into its completion", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>()
  const { ui, activate, escape } = navigationFixture(() => pending.promise)
  try {
    ui.show(nativeEquipment); activate("Class3")
    activate("Itemslot-0"); activate("Itemitem-18")
    expect(ui.snapshot().page).toBe("slot")
    escape()
    expect(ui.snapshot().page).toBe("loadout")
    ui.hide(); ui.show(nativeEquipment)
    pending.resolve(nativeEquipment)
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    expect(ui.snapshot().page).toBe("classes")
  } finally { ui.destroy() }
})

test("held Escape leaves only the intended equipment layer", () => {
  const { ui, activate, escape } = navigationFixture()
  try {
    ui.show(nativeEquipment); activate("Class3")
    escape(); escape(true)
    expect(ui.visible()).toBe(true)
    expect(ui.snapshot().page).toBe("classes")
  } finally { ui.destroy() }
})

test("class-menu loadout closes directly, while slot cancellation restores its focused slot", () => {
  const { ui, activate, escape } = navigationFixture()
  try {
    ui.show(nativeEquipment, 3)
    activate("Itemslot-1"); escape(); ui.frame(0)
    const snapshot = ui.snapshot().vgui
    expect(snapshot.panels.find(panel => panel.id === snapshot.input.keyFocus)?.name).toBe("Itemslot-1")
    escape()
    expect(ui.visible()).toBe(false)
  } finally { ui.destroy() }
})

test("pending equip disables item mutations, not the actual Back hit target", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>(), calls: AbortSignal[] = []
  const { ui, activate } = navigationFixture(async (_playerClass, _slot, _definition, signal) => { calls.push(signal); return pending.promise })
  try {
    ui.show(nativeEquipment, 3); activate("Itemslot-0"); activate("Itemitem-18"); activate("Itemitem-18")
    expect(calls).toHaveLength(1)
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "Itemitem-18")!.enabled).toBe(false)
    activate("BackButton", 0.95)
    expect(ui.snapshot().page).toBe("loadout")
    expect(calls[0]!.aborted).toBe(true)
    pending.resolve(nativeEquipment)
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    expect(ui.snapshot().page).toBe("loadout")
  } finally { ui.destroy() }
})

test("failed equip reports the real error, preserves selection and permits retry", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>()
  const { ui, activate, errors } = navigationFixture(() => pending.promise)
  try {
    ui.show(nativeEquipment, 3); activate("Itemslot-0"); activate("Itemitem-18")
    const failure = new Error("model preparation failed")
    pending.reject(failure)
    await Promise.resolve(); await Promise.resolve()
    expect(errors).toEqual([failure])
    expect(ui.snapshot().page).toBe("slot")
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "Itemitem-18")!.enabled).toBe(true)
  } finally { ui.destroy() }
})

test("destroyed presentation cannot be revived by an equip completion", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>()
  const { ui, activate, errors } = navigationFixture(() => pending.promise)
  ui.show(nativeEquipment, 3); activate("Itemslot-0"); activate("Itemitem-18"); ui.destroy()
  pending.resolve({ ...nativeEquipment, revision: nativeEquipment.revision + 1 })
  await Promise.resolve(); await Promise.resolve()
  expect(ui.visible()).toBe(false)
  expect(errors).toEqual([])
})

test("late failures are reported without taking over a reopened page", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>()
  const { ui, activate, errors, errorCurrent } = navigationFixture(() => pending.promise)
  try {
    ui.show(nativeEquipment, 3); activate("Itemslot-0"); activate("Itemitem-18")
    ui.hide(); ui.show(nativeEquipment)
    const error = new Error("resource preparation rejected")
    pending.reject(error)
    await Promise.resolve(); await Promise.resolve()
    expect(errors).toEqual([error]); expect(errorCurrent).toEqual([false])
    expect(ui.snapshot().page).toBe("classes")
  } finally { ui.destroy() }
})

test("the native wrapping revision does not discard a committed equipment change", async () => {
  const weapon = nativeEquipment.inventory.find(item => item.item.definitionIndex === 127)!
  const next = { ...nativeEquipment, revision: 0, classes: nativeEquipment.classes.map(player => player.class === 3
    ? { ...player, items: player.items.map(item => item.slot === 0 ? { ...weapon.item, slot: 0 } : item) } : player) }
  const { ui, root, activate } = navigationFixture(async () => next)
  try {
    ui.show({ ...nativeEquipment, revision: 0xffff_ffff }, 3)
    activate("Itemslot-0"); activate("Itemitem-127")
    await Promise.resolve(); await Promise.resolve()
    expect(byName(root, "Itemslot-0").textContent).toContain(weapon.displayName)
  } finally { ui.destroy() }
})

test("equipment leaves composing keys to their text owner", () => {
  const { ui } = navigationFixture()
  try {
    ui.show(nativeEquipment, 3)
    expect(ui.handleKey({ code: "Escape", isComposing: true, preventDefault() { throw new Error("IME default was cancelled") }, stopImmediatePropagation() { throw new Error("IME propagation was stopped") } })).toBe(false)
    expect(ui.visible()).toBe(true)
  } finally { ui.destroy() }
})

test("owner-driven hiding cancels work without restoring a replaced page's focus", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>(), signals: AbortSignal[] = []
  const { ui, activate, closes } = navigationFixture((_playerClass, _slot, _definition, signal) => { signals.push(signal); return pending.promise })
  try {
    ui.show(nativeEquipment, 3); activate("Itemslot-0"); activate("Itemitem-18")
    ui.hide(false)
    expect(signals[0]!.aborted).toBe(true)
    pending.resolve(nativeEquipment)
    await Promise.resolve(); await Promise.resolve()
    expect(closes()).toBe(0)
    expect(ui.visible()).toBe(false)
  } finally { ui.destroy() }
})

test("backpack page keys use the same bounded command path and do not rebuild at an edge", () => {
  const { ui, activate, key } = navigationFixture()
  try {
    ui.show(nativeEquipment); activate("BackpackButton")
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "PrevPage")!.enabled).toBe(true)
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "NextPage")!.enabled).toBe(true)
    const before = ui.snapshot().vgui.panels.find(panel => panel.name === "LocalEquipment")!.id
    expect(key("PageUp")).toBe(true)
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "LocalEquipment")!.id).toBe(before)
    key("PageDown")
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "PrevPage")!.enabled).toBe(true)
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "NextPage")!.enabled).toBe(true)
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "LocalEquipment")!.id).not.toBe(before)
    key("PageUp")
    activate("PrevPage")
    expect(ui.snapshot().vgui.panels.some(panel => panel.name === `Itemitem-${nativeEquipment.inventory[50]!.item.definitionIndex}`)).toBe(true)
    activate("NextPage")
    expect(ui.snapshot().vgui.panels.some(panel => panel.name === `Itemitem-${nativeEquipment.inventory[0]!.item.definitionIndex}`)).toBe(true)
    for (const fraction of [0.05, 0.5, 0.95]) {
      activate("BackButton", fraction)
      expect(ui.snapshot().page).toBe("classes")
      activate("BackpackButton")
    }
  } finally { ui.destroy() }
})

test("class and loadout directional focus activates the real button and survives resize", () => {
  const { ui, root, key, escape } = navigationFixture()
  const focus = () => {
    const snapshot = ui.snapshot().vgui
    return snapshot.panels.find(panel => panel.id === snapshot.input.keyFocus)!.name
  }
  const enter = () => {
    const target = byName(root, focus())
    for (const type of ["keydown", "keyup"]) {
      const event = new FakeEvent(type, { key: "Enter", code: "Enter", bubbles: true })
      event.target = target
      root.ownerDocument.dispatchEvent(event)
    }
    ui.frame(0)
  }
  try {
    ui.show(nativeEquipment); ui.frame(0)
    expect(focus()).toBe("Class1")
    key("ArrowRight")
    expect(focus()).toBe("Class3")
    enter()
    expect(ui.snapshot().page).toBe("loadout")
    key("ArrowDown")
    expect(focus()).toBe("Itemslot-1")
    ui.setViewport({ width: 1440, height: 900, devicePixelRatio: 1 }); ui.frame(0)
    expect(focus()).toBe("Itemslot-1")
    enter()
    expect(ui.snapshot().page).toBe("slot")
    escape(); ui.frame(0)
    expect(focus()).toBe("Itemslot-1")
  } finally { ui.destroy() }
})

test("a held equipment Enter presses the button but changes only one page on release", () => {
  const { ui, root } = navigationFixture()
  const send = (type: string, repeat = false) => {
    const snapshot = ui.snapshot().vgui
    const name = snapshot.panels.find(panel => panel.id === snapshot.input.keyFocus)!.name
    const event = new FakeEvent(type, { key: "Enter", code: "Enter", repeat })
    event.target = byName(root, name)
    root.ownerDocument.dispatchEvent(event)
  }
  try {
    ui.show(nativeEquipment); ui.frame(0)
    send("keydown")
    expect(ui.snapshot().page).toBe("classes")
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "Class1")!.state.depressed).toBe(true)
    send("keydown", true)
    expect(ui.snapshot().page).toBe("classes")
    send("keyup")
    expect(ui.snapshot().page).toBe("loadout")
    ui.frame(0)
    send("keydown")
    root.ownerDocument.defaultView.dispatchEvent(new FakeEvent("blur"))
    expect(ui.snapshot().vgui.panels.find(panel => panel.name === "Itemslot-0")!.state.depressed).toBe(false)
    expect(ui.snapshot().page).toBe("loadout")
  } finally { ui.destroy() }
})

test("grid arrows preserve row boundaries, cross pages horizontally and select empty cells without equipping", () => {
  const calls: unknown[] = []
  const { ui, activate, key } = navigationFixture(async (...args) => { calls.push(args); return nativeEquipment })
  try {
    ui.show(nativeEquipment); activate("BackpackButton")
    key("ArrowRight")
    const first = ui.snapshot().vgui.input.keyFocus
    key("ArrowUp")
    expect(ui.snapshot().vgui.input.keyFocus).toBe(first)
    for (let index = 0; index < 10; index++) key("ArrowRight")
    expect(ui.snapshot().vgui.panels.some(panel => panel.name === `Itemitem-${nativeEquipment.inventory[50]!.item.definitionIndex}`)).toBe(true)
    key("ArrowDown"); key("ArrowDown")
    expect(calls).toEqual([])
    const snapshot = ui.snapshot().vgui
    expect(snapshot.input.keyFocus).toBeNull()
    expect(key("Enter")).toBe(true)
    expect(calls).toEqual([])
  } finally { ui.destroy() }
})

test("item tooltip has visible bounds after the authored hover delay", () => {
  const fixture = createTf2GameUiTransitionFixture(), root = createRoot(fixture.document)
  const resources = { ...fixture.resources, clientScheme: { ...fixture.resources.clientScheme, borders: ["Econ.Button.Border.Default", "BackpackItemBorder", "BackpackItemMouseOverBorder"].map(name => ({
    kind: "line" as const, name, inset: { left: 0, top: 0, right: 0, bottom: 0 }, backgroundType: 0 as const, paintFirst: false,
    sides: { left: [], top: [], right: [], bottom: [] },
  })) } }
  let now = 0
  const ui = new Tf2EquipmentPresentation({ root: root as unknown as HTMLElement, resources,
    viewport: { width: 1280, height: 800, devicePixelRatio: 1 }, reducedMotion: true,
    clock: { nowSeconds: () => now }, random: { nextUnit: () => 0 }, onClose() {}, onPreview() {}, onError(error) { throw error }, onEquip: async () => nativeEquipment })
  ui.show(nativeEquipment, 3)
  byName(root, "Itemslot-0").focus()
  now = 0.1; ui.frame(now)
  expect(ui.snapshot().vgui.panels.some(panel => panel.name === "ItemTooltip")).toBe(false)
  byName(root, "Itemslot-0").dispatchEvent(new FakeEvent("pointerover", { bubbles: true }))
  now = 0.199; ui.frame(now)
  expect(ui.snapshot().vgui.panels.some(panel => panel.name === "ItemTooltip")).toBe(false)
  now = 0.2; ui.frame(now)
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
    clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0 }, onClose() {}, onPreview() {}, onError(error) { throw error }, onEquip: async () => state })
  ui.show(state)
  expect(ui.snapshot().vgui.panels.filter(panel => /^Class[1-9]$/.test(panel.name))).toHaveLength(9)
  ui.show(state, 8)
  expect(ui.snapshot().vgui.panels.filter(panel => panel.name.startsWith("Itemslot-")).map(panel => panel.name)).toEqual([
    "Itemslot-1", "Itemslot-2", "Itemslot-6", "Itemslot-4", "Itemslot-7", "Itemslot-8", "Itemslot-10", "Itemslot-9",
  ])
  expect(ui.snapshot().vgui.panels.find(panel => panel.name === "EquipmentPlayer")?.bounds).toMatchObject({ width: 405, height: 510 })
  ui.destroy()
})

test("a second backpack page enters the selected class-slot list at its first page", async () => {
  const fixture = createTf2GameUiTransitionFixture(), root = createRoot(fixture.document)
  const resources = { ...fixture.resources, clientScheme: { ...fixture.resources.clientScheme, borders: ["Econ.Button.Border.Default", "BackpackItemBorder", "BackpackItemSelectedBorder"].map(name => ({
    kind: "line" as const, name, inset: { left: 0, top: 0, right: 0, bottom: 0 }, backgroundType: 0 as const, paintFirst: false, sides: { left: [], top: [], right: [], bottom: [] },
  })) } }
  const rocket = nativeEquipment.inventory.find(item => item.item.definitionIndex === 18)!
  const bat = nativeEquipment.inventory.find(item => item.item.definitionIndex === 0)!
  // Structural inventory fixture: the class-slot contains one item, not 51.
  const state = { ...nativeEquipment, inventory: [...Array.from({ length: 50 }, (_, index) => ({ ...rocket, item: { ...rocket.item, itemId: 1001 + index, definitionIndex: 1000 + index } })), bat] }
  const equipped: unknown[] = []
  const ui = new Tf2EquipmentPresentation({ root: root as unknown as HTMLElement, resources,
    viewport: { width: 1280, height: 800, devicePixelRatio: 1 }, reducedMotion: true, clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0 },
    onClose() {}, onPreview() {}, onError(error) { throw error }, onEquip: async (playerClass, slot, definition) => { equipped.push([playerClass, slot, definition]); return state } })
  ui.show(state)
  const activate = (name: string) => {
    const element = byName(root, name), bounds = ui.snapshot().vgui.panels.find(panel => panel.name === name)!.bounds
    pointer(element, "pointerdown", bounds.x + 1, bounds.y + 1)
    pointer(element, "pointerup", bounds.x + 1, bounds.y + 1)
    ui.frame(0)
  }
  activate("BackpackButton"); activate("NextPage"); activate("Itemitem-0")
  expect(ui.snapshot().page).toBe("slot")
  expect(ui.snapshot().vgui.panels.some(panel => panel.name === "Itemitem-0")).toBe(true)
  expect(ui.snapshot().vgui.panels.some(panel => panel.name === "NextPage")).toBe(false)
  activate("Itemitem-0")
  await Promise.resolve(); await Promise.resolve()
  expect(equipped).toEqual([[1, 2, 0]])
  expect(ui.snapshot().page).toBe("loadout")
  ui.destroy()
})
