import { describe, expect, test } from "bun:test"
import { createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "../gameui-integration/fixture"
import { initializeTf2ClassSelectionIntegration, TF2_CLASS_SELECTION_CLASSES, type Tf2ClassSelectionModelPanel, type Tf2ClassSelectionRequest } from "../../src/class-selection"

function fixture() {
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
  })
  return Object.freeze({ ...base, integration, requests, models })
}

describe("authored TF2 class selection VGUI integration", () => {
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

  test("maps authored number keys to Source class identities and preserves initial-join cancellation", () => {
    const { integration, requests } = fixture()
    integration.dispatch({ kind: "show", team: 2, current: null })
    expect(integration.snapshot().panels.find((panel) => panel.name === "CancelButton")?.effectivelyVisible).toBeFalse()
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
