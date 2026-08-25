import { describe, expect, test } from "bun:test"
import { createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { createTf2GameUiTransitionFixture } from "../gameui-integration/fixture"
import {
  initializeTf2TeamSelectionIntegration,
  type Tf2TeamSelectionModelPanel,
  type Tf2TeamSelectionRequest,
  type Tf2TeamSelectionServerState,
} from "../../src/team-selection"

function fixture(overrides: Partial<Tf2TeamSelectionServerState> = {}) {
  const base = createTf2GameUiTransitionFixture()
  const requests: Tf2TeamSelectionRequest[] = []
  const models: (readonly Tf2TeamSelectionModelPanel[])[] = []
  const server: Tf2TeamSelectionServerState = Object.freeze({
    localTeam: 0, redCount: 0, blueCount: 0, redDisabled: false, blueDisabled: false,
    spectatorsVisible: true, autoAssignVisible: true, cancelVisible: false,
    highlander: false, teamsFull: false, teamsFullArrow: false, ...overrides,
  })
  const integration = initializeTf2TeamSelectionIntegration({
    root: createRoot(base.document) as unknown as HTMLElement,
    resources: base.resources,
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    reducedMotion: true,
    clock: { nowSeconds: () => 0 },
    random: { nextUnit: () => 0 },
    onRequest: (request) => requests.push(request),
    onModelPanels: (panels) => models.push(panels),
  })
  integration.dispatch({ kind: "show", server })
  return Object.freeze({ ...base, server, integration, requests, models })
}

describe("authored TF2 RED/BLU team-selection VGUI", () => {
  test("mounts the exact authored frame, four controls, and all five real model-panel descriptors", () => {
    const { integration, models } = fixture()
    const snapshot = integration.snapshot()
    for (const name of ["team", "teambutton0", "teambutton1", "teambutton2", "teambutton3", "TeamMenuSelect", "BlueCount", "RedCount"]) {
      expect(snapshot.panels.some((panel) => panel.name === name), name).toBeTrue()
    }
    expect(models.at(-1)?.map(({ name, model, fov, origin, angles }) => ({ name, model, fov, origin, angles }))).toEqual([
      { name: "MenuBG", model: "models/vgui/UI_team01.mdl", fov: 20, origin: [305, 0, -34], angles: [0, 180, 0] },
      { name: "bluedoor", model: "models/vgui/UI_team01_blue.mdl", fov: 20, origin: [305, 0, -34], angles: [0, 180, 0] },
      { name: "reddoor", model: "models/vgui/UI_team01_red.mdl", fov: 20, origin: [305, 0, -34], angles: [0, 180, 0] },
      { name: "autodoor", model: "models/vgui/UI_team01_random.mdl", fov: 20, origin: [305, 0, -34], angles: [0, 180, 0] },
      { name: "spectate", model: "models/vgui/UI_team01_spectate.mdl", fov: 20, origin: [305, 0, -34], angles: [0, 180, 0] },
    ])
    expect(snapshot.panels.find((panel) => panel.name === "CancelButton")?.effectivelyVisible).toBeFalse()
  })

  test("binds live server counts, disabled doors, spectator policy, and Highlander panels", () => {
    const { integration } = fixture({ redCount: 9, blueCount: 9, redDisabled: true, blueDisabled: true,
      spectatorsVisible: false, autoAssignVisible: false, highlander: true, teamsFull: true })
    const snapshot = integration.snapshot()
    expect(snapshot.panels.find((panel) => panel.name === "teambutton0")?.enabled).toBeTrue()
    expect(snapshot.panels.find((panel) => panel.name === "teambutton1")?.enabled).toBeTrue()
    expect(integration.modelPanels().find((panel) => panel.name === "bluedoor")?.animation).toBe("idle_disabled")
    expect(integration.modelPanels().find((panel) => panel.name === "reddoor")?.animation).toBe("idle_disabled")
    expect(snapshot.panels.find((panel) => panel.name === "teambutton2")?.effectivelyVisible).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.name === "teambutton3")?.effectivelyVisible).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.name === "HighlanderLabel")?.effectivelyVisible).toBeTrue()
    expect(snapshot.panels.find((panel) => panel.name === "TeamsFullLabel")?.effectivelyVisible).toBeTrue()
    expect(integration.modelPanels().map((panel) => panel.name)).toEqual(["MenuBG", "bluedoor", "reddoor"])
  })

  test("retains authored disabled-door hover animation without admitting its team command", () => {
    const { integration, requests } = fixture({ redCount: 1, redDisabled: true })
    integration.dispatch({ kind: "hover", team: "red" })
    expect(integration.modelPanels().find((panel) => panel.name === "reddoor")?.animation).toBe("enter_disabled")
    expect(integration.dispatch({ kind: "select", team: "red" }).disposition).toBe("ignored")
    expect(requests).toEqual([])
    integration.dispatch({ kind: "hover", team: null })
    expect(integration.modelPanels().find((panel) => panel.name === "reddoor")?.animation).toBe("exit_disabled")
  })

  test("keeps initial cancellation blocked and routes keyboard auto-assign through its exact Source command", () => {
    const { integration, requests } = fixture()
    let prevented = 0
    const event = (code: string) => ({ code, repeat: false, preventDefault() { prevented += 1 }, stopImmediatePropagation() {} })
    expect(integration.handleKey(event("Escape"), false)).toBeTrue()
    expect(integration.state().visible).toBeTrue()
    expect(integration.handleKey(event("Space"), false)).toBeTrue()
    expect(prevented).toBe(2)
    expect(requests).toEqual([{ kind: "join-team", team: "auto", sourceCommand: "jointeam auto" }])
  })

  test("moves authored focus in Source tab order and publishes real door animation names", () => {
    const { integration, requests } = fixture()
    const event = (code: string) => ({ code, repeat: false, preventDefault() {}, stopImmediatePropagation() {} })
    expect(integration.state().focused).toBe("auto")
    integration.handleKey(event("ArrowRight"), false)
    expect(integration.state().focused).toBe("spectate")
    integration.handleKey(event("ArrowRight"), false)
    expect(integration.state().focused).toBe("blue")
    integration.dispatch({ kind: "hover", team: "blue" })
    expect(integration.modelPanels().find((panel) => panel.name === "bluedoor")?.animation).toBe("enter_enabled")
    integration.handleKey(event("Enter"), false)
    expect(requests[0]?.sourceCommand).toBe("jointeam blue")
  })
})
