import { describe, expect, test } from "bun:test"
import { createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "../gameui-integration/fixture"
import { initializeTf2LocalMatchPresentation, type Tf2LocalMatchLaunch } from "../../src/local-match"

function fixture() {
  const existing = createTf2GameUiTransitionFixture()
  const launches: Tf2LocalMatchLaunch[] = []
  const visible: boolean[] = []
  const stored = new Map<string, string>()
  const presentation = initializeTf2LocalMatchPresentation({
    root: createRoot(existing.document) as unknown as HTMLElement,
    resources: existing.resources,
    configuredMaps: ["jump_beef", "pl_upward", "ctf_2fort"],
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    reducedMotion: true,
    clock: { nowSeconds: () => 0 },
    random: { nextUnit: () => 0 },
    storage: { getItem: (key) => stored.get(key) ?? null, setItem: (key, value) => { stored.set(key, value) } },
    onVisibility: (value) => visible.push(value),
    onLaunch: (launch) => launches.push(launch),
  })
  return Object.freeze({ presentation, launches, visible, stored })
}

describe("TF2 authored local-match VGUI roots", () => {
  test("constructs the real training pages, source create-server pages, map choices, and bot controls", () => {
    const { presentation, visible } = fixture()
    presentation.show("training")
    expect(presentation.snapshot()).toMatchObject({ visible: true, entry: "training", page: "training-mode" })
    const training = presentation.snapshot().vgui.panels
    expect(training.find((panel) => panel.name === "ModeSelectionPanel")?.effectivelyVisible).toBeTrue()
    expect(training.find((panel) => panel.name === "OfflinePractice_ModeSelectionPanel")?.effectivelyVisible).toBeFalse()
    expect(training.find((panel) => panel.name === "OfflinePractice_MapSelectionPanel")?.effectivelyVisible).toBeFalse()
    expect(training.find((panel) => panel.name === "DifficultyComboBox")?.state.items.map((item) => item.text))
      .toEqual(["Easy", "Normal", "Hard", "Expert"])
    presentation.hide()

    presentation.show("create-server")
    const server = presentation.snapshot().vgui.panels
    expect(server.find((panel) => panel.name === "CreateMultiplayerGameDialog")?.effectivelyVisible).toBeTrue()
    expect(server.find((panel) => panel.name === "ServerPage")?.effectivelyVisible).toBeTrue()
    expect(server.find((panel) => panel.name === "GameplayPage")?.effectivelyVisible).toBeFalse()
    expect(server.find((panel) => panel.name === "MapList")?.state.items.map((item) => item.text))
      .toEqual(["pl_upward", "ctf_2fort"])
    expect(server.find((panel) => panel.name === "TeamFillComboBox")?.state.items.map((item) => item.text))
      .toEqual(["normal", "fill", "match"])
    expect(visible).toEqual([true, false, true])
    presentation.destroy()
  })

  test("closes the actual visible modal on Escape without routing gameplay input", () => {
    const { presentation, visible } = fixture()
    presentation.show("create-server")
    let prevented = false
    let stopped = false
    expect(presentation.handleKey({
      code: "Escape",
      preventDefault: () => { prevented = true },
      stopImmediatePropagation: () => { stopped = true },
    })).toBeTrue()
    expect({ prevented, stopped, visible }).toEqual({ prevented: true, stopped: true, visible: [true, false] })
    expect(presentation.snapshot().visible).toBeFalse()
  })
})
