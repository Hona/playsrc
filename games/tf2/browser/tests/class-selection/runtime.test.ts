import { describe, expect, test } from "bun:test"
import { createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "../gameui-integration/fixture"
import { initializeTf2ClassSelectionIntegration, TF2_CLASS_SELECTION_CLASSES, type Tf2ClassSelectionModelPanel, type Tf2ClassSelectionRequest } from "../../src/class-selection"

function fixture(roster: readonly { fake: boolean; team: number; class: number }[] = []) {
  const base = createTf2GameUiTransitionFixture()
  const requests: Tf2ClassSelectionRequest[] = []
  const models: (readonly Tf2ClassSelectionModelPanel[])[] = []
  const integration = initializeTf2ClassSelectionIntegration({
    root: createRoot(base.document) as unknown as HTMLElement,
    resources: base.resources,
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    reducedMotion: true,
    clock: { nowSeconds: () => 0 },
    random: { nextUnit: () => 0 },
    onRequest: (request) => requests.push(request),
    onModelPanels: (panels) => models.push(panels),
    roster: () => roster,
  })
  return Object.freeze({ ...base, integration, requests, models })
}

describe("authored TF2 class selection VGUI integration", () => {
  test("projects authoritative local/bot counts and keeps admission out of the old local class", () => {
    const { integration } = fixture([
      { fake: false, team: 2, class: 3 }, { fake: true, team: 2, class: 1 },
      { fake: true, team: 2, class: 1 }, { fake: true, team: 3, class: 5 },
    ])
    integration.dispatch({ kind: "show", team: 2, current: null })
    let panels = integration.snapshot().panels
    expect(panels.find(panel => panel.name === "numSoldier")?.text).toBe("")
    expect(panels.find(panel => panel.name === "numScout")?.text).toBe("2")
    expect(panels.find(panel => panel.name === "countImage0")?.state.image).toBe("class_sel_sm_scout_red")
    expect(panels.find(panel => panel.name === "countImage1")?.effectivelyVisible).toBe(true)
    expect(panels.find(panel => panel.name === "countImage2")?.effectivelyVisible).toBe(false)
    expect(panels.find(panel => panel.name === "localPlayerImage")?.effectivelyVisible).toBe(false)
    integration.dispatch({ kind: "show", team: 2, current: 3 })
    panels = integration.snapshot().panels
    expect(panels.find(panel => panel.name === "numSoldier")?.text).toBe("1")
    expect(panels.find(panel => panel.name === "localPlayerImage")?.state.image).toBe("class_sel_sm_soldier_red")
    expect(panels.find(panel => panel.name === "CountLabel")?.effectivelyVisible).toBe(true)
    expect(panels.find(panel => panel.name === "UpArrow")?.effectivelyVisible).toBe(false)
  })
  test("mounts every authored class button and exposes both exact model-panel passes", () => {
    const { integration, models } = fixture()
    integration.dispatch({ kind: "show", team: 2, current: 3 })
    const snapshot = integration.snapshot()
    for (const selected of TF2_CLASS_SELECTION_CLASSES) {
      expect(snapshot.panels.some((panel) => panel.name === selected.name), selected.name).toBeTrue()
    }
    expect(models.at(-1)).toMatchObject([
      { name: "MenuBG", model: "models/vgui/ui_class01.mdl", skin: 0, fov: 16, origin: [365, 0, -40], angles: [0, 180, 0] },
      { name: "TFPlayerModel", model: "models/player/soldier.mdl", skin: 0, fov: 25, origin: [320, 10, -49], angles: [0, 180, 0] },
    ])
    expect(snapshot.panels.filter((panel) => ["ClassSelectionViewport", "class", "CancelButton"].includes(panel.name)).map((panel) => [panel.name, panel.visible, panel.effectivelyVisible, panel.bounds])).toEqual([
      ["ClassSelectionViewport", true, true, { x: 0, y: 0, width: 1280, height: 720 }],
      ["class", true, true, { x: 0, y: 0, width: 1280, height: 720 }],
      ["CancelButton", true, true, expect.anything()],
    ])
    expect(snapshot.panels.find((panel) => panel.name === "scout")?.bounds).toMatchObject({ x: 190, y: -7 })
    expect(snapshot.panels.find((panel) => panel.name === "ClassMenuSelect")?.bounds).toMatchObject({ x: 45, y: 660 })
    expect(snapshot.panels.find((panel) => panel.name === "ClassMenuSelect")?.effectivelyVisible).toBeFalse()
  })

  test("updates team-aware authored portrait and preview without sending a gameplay command", () => {
    const { integration, requests, models } = fixture()
    integration.dispatch({ kind: "show", team: 3, current: 3 })
    integration.dispatch({ kind: "hover", identity: 4 })
    const demo = integration.snapshot().panels.find((panel) => panel.name === "demoman")!
    const image = integration.snapshot().panels.find((panel) => panel.name === "SubImage" && panel.parent === demo.id)
    expect(image?.state.image).toBe("class_sel_sm_demo_blu")
    expect(models.at(-1)?.[1]).toMatchObject({ model: "models/player/demo.mdl", skin: 1 })
    expect(requests).toEqual([])
  })

  test("lays out proportional tip items before stretching their authored auto-resize children", () => {
    const { integration } = fixture()
    integration.dispatch({ kind: "show", team: 2, current: 6 })
    const panels = integration.snapshot().panels
    const item = panels.find((panel) => panel.name === "ClassTipsItemPanel1")!
    expect(item.bounds).toEqual({ x: 7, y: 7, width: 308, height: 45 })
    expect(panels.find((panel) => panel.name === "TipLabel" && panel.parent === item.id)?.bounds)
      .toEqual({ x: 37, y: 3, width: 278, height: 45 })
  })

  test("maps authored number keys to Source class identities and preserves initial-join cancellation", () => {
    const { integration, requests } = fixture()
    integration.dispatch({ kind: "show", team: 2, current: null })
    expect(integration.snapshot().panels.find((panel) => panel.name === "CancelButton")?.effectivelyVisible).toBeFalse()
    expect(integration.snapshot().panels.find((panel) => panel.name === "ClassMenuSelect")?.effectivelyVisible).toBeTrue()
    let prevented = false
    const key = (code: string) => ({ code, repeat: false, preventDefault() { prevented = true }, stopImmediatePropagation() {} })
    expect(integration.handleKey(key("Escape"), false)).toBeTrue()
    expect(integration.state().visible).toBeTrue()
    expect(integration.handleKey(key("Digit2"), false)).toBeTrue()
    expect(prevented).toBeTrue()
    expect(requests).toEqual([{ kind: "join-class", identity: 3, sourceCommand: "joinclass soldier" }])
    expect(integration.state().visible).toBeFalse()
  })
})
