import { describe, expect, test } from "bun:test"
import {
  TF2_MAIN_MENU_STATE,
  transitionTf2GameUi,
  type Tf2GameUiEvent,
  type Tf2GameUiState,
} from "../../src/gameui"
import { createTf2LoadingPresentation } from "../../src/loading-presentation"
import { createTf2GameUiTransitionFixture } from "./fixture"

function expectApplied(state: Tf2GameUiState, event: Tf2GameUiEvent): Tf2GameUiState {
  const transition = transitionTf2GameUi(state, event)
  expect(transition.disposition, `${state.kind}:${event.kind}`).toBe("applied")
  return transition.state
}

function stateTable(): Record<Tf2GameUiState["kind"], Tf2GameUiState> {
  const loading = expectApplied(TF2_MAIN_MENU_STATE, { kind: "loading-started", mapIdentity: "jump_beef" })
  const active = expectApplied(loading, { kind: "loading-succeeded" })
  const pause = expectApplied(active, { kind: "gameui-activated" })
  return {
    "main-menu": TF2_MAIN_MENU_STATE,
    loading,
    "in-game": active,
    pause,
    disconnecting: expectApplied(pause, { kind: "activate-button", button: "disconnect" }),
    failure: expectApplied(loading, {
      kind: "loading-failed",
      reason: "Startup failed",
      extendedReason: "The initial map could not load",
    }),
  }
}

describe("TF2 GameUI map-command ownership", () => {
  test("accepts every configured map command after startup loading fails", () => {
    const failure = stateTable().failure
    for (const mapIdentity of ["pl_upward", "jump_beef", "ctf_2fort"] as const) {
      const transition = transitionTf2GameUi(failure, { kind: "map", mapIdentity })
      expect(transition.disposition, mapIdentity).toBe("applied")
      expect(transition.state, mapIdentity).toBe(failure)
      expect(transition.request, mapIdentity).toEqual({ kind: "load-map", mapIdentity })
    }
  })

  test("accepts replacement map commands during active gameplay", () => {
    const active = stateTable()["in-game"]
    const transition = transitionTf2GameUi(active, { kind: "map", mapIdentity: "pl_upward" })
    expect(transition.disposition).toBe("applied")
    expect(transition.state).toBe(active)
    expect(transition.request).toEqual({ kind: "load-map", mapIdentity: "pl_upward" })
  })

  test("publishes every configured map request and accepts owner loading starts in all six states", () => {
    for (const [kind, state] of Object.entries(stateTable()) as [Tf2GameUiState["kind"], Tf2GameUiState][]) {
      for (const mapIdentity of ["jump_beef", "pl_upward", "ctf_2fort"] as const) {
        const request = transitionTf2GameUi(state, { kind: "map", mapIdentity })
        expect(request.disposition, `${kind}:${mapIdentity}`).toBe("applied")
        expect(request.state, `${kind}:${mapIdentity}`).toBe(state)
        expect(request.request, `${kind}:${mapIdentity}`).toEqual({ kind: "load-map", mapIdentity })

        const started = transitionTf2GameUi(state, { kind: "loading-started", mapIdentity })
        expect(started.disposition, `${kind}:${mapIdentity}`).toBe("applied")
        expect(started.request, `${kind}:${mapIdentity}`).toBeNull()
        expect(started.state, `${kind}:${mapIdentity}`).toMatchObject({
          kind: "loading",
          mapIdentity,
          phase: "idle",
          phaseRepeat: 0,
          progress: 0,
          statusText: "",
        })
      }
    }
  })

  test("resets authoritative milestones across repeated map replacement and failure recovery", () => {
    let state = expectApplied(TF2_MAIN_MENU_STATE, { kind: "loading-started", mapIdentity: "jump_beef" })
    state = expectApplied(state, { kind: "loading-progress", phase: "ready-to-play" })
    state = expectApplied(state, { kind: "loading-started", mapIdentity: "pl_upward" })
    expect(state).toMatchObject({ kind: "loading", mapIdentity: "pl_upward", phase: "idle", progress: 0, statusText: "" })
    state = expectApplied(state, { kind: "loading-failed", reason: "Rejected", extendedReason: "owner rejected map" })
    state = expectApplied(state, { kind: "loading-started", mapIdentity: "jump_beef" })
    state = expectApplied(state, { kind: "loading-succeeded" })
    expect(state).toEqual({ kind: "in-game", mapIdentity: "jump_beef" })
  })

  test("rejects malformed map identities atomically in every state", () => {
    for (const [kind, state] of Object.entries(stateTable()) as [Tf2GameUiState["kind"], Tf2GameUiState][]) {
      for (const mapIdentity of ["", "x".repeat(96), "bad\0map"]) {
        for (const event of ["map", "loading-started"] as const) {
          expect(transitionTf2GameUi(state, { kind: event, mapIdentity }), `${kind}:${event}`).toMatchObject({
            disposition: "illegal",
            state,
            request: null,
            reason: "invalid-map-identity",
          })
        }
      }
    }
  })
})

describe("TF2 GameUI Escape and pending owner operations", () => {
  test("hides source-disabled promotions, account content, and overlapping duplicate settings", () => {
    const panels = createTf2GameUiTransitionFixture().gameUi.snapshot().panels
    for (const name of [
      "EventPromo", "FriendsContainer", "ShowPromoCodesButton", "CharacterSetupButton", "GeneralStoreButton",
      "Notifications_ShowButtonPanel", "MOTD_ShowButtonPanel", "WatchStreamButton", "QuestLogButton",
      "NoGCMessage", "NoGCImage", "RankBorder", "CycleRankTypeButton", "VRBGPanel", "VRModeButton",
      "SettingsButtonSDK", "TF2SettingsButtonSDK", "icon_generator", "PartySlot0", "QueueContainer",
    ]) {
      expect(panels.find((panel) => panel.name === name)?.effectivelyVisible, name).toBeFalse()
    }
    expect(panels.find((panel) => panel.name === "SettingsButton")?.effectivelyVisible).toBeTrue()
    expect(panels.find((panel) => panel.name === "TF2SettingsButton")?.effectivelyVisible).toBeTrue()
  })

  test("classifies Escape in all six state contexts", () => {
    const expected = {
      "main-menu": { disposition: "ignored", state: "main-menu", request: null },
      loading: { disposition: "applied", state: "disconnecting", request: { kind: "disconnect" } },
      "in-game": { disposition: "applied", state: "pause", request: null },
      pause: { disposition: "applied", state: "pause", request: { kind: "resume-game" } },
      disconnecting: { disposition: "ignored", state: "disconnecting", request: null },
      failure: { disposition: "applied", state: "disconnecting", request: { kind: "disconnect" } },
    } as const

    for (const [kind, state] of Object.entries(stateTable()) as [Tf2GameUiState["kind"], Tf2GameUiState][]) {
      const result = transitionTf2GameUi(state, { kind: "escape" })
      expect(result.disposition, kind).toBe(expected[kind].disposition)
      expect(result.state.kind, kind).toBe(expected[kind].state)
      expect(result.request, kind).toEqual(expected[kind].request)
    }
  })

  test("preserves one dashboard through three owner-acknowledged pause cycles", () => {
    const { gameUi, requests } = createTf2GameUiTransitionFixture()
    gameUi.dispatch({ kind: "loading-started", mapIdentity: "jump_beef" })
    gameUi.dispatch({ kind: "loading-succeeded" })
    const dashboardIdentity = gameUi.snapshot().panels.find((panel) => panel.name === "MMDashboard")!.id

    for (let cycle = 0; cycle < 3; cycle += 1) {
      const opened = gameUi.dispatch({ kind: "escape" })
      expect(opened).toMatchObject({ disposition: "applied", state: { kind: "pause" }, request: null })
      const pausePanels = gameUi.snapshot().panels
      expect(pausePanels.filter((panel) => panel.name === "MMDashboard")).toHaveLength(1)
      expect(pausePanels.find((panel) => panel.name === "MMDashboard")).toMatchObject({ id: dashboardIdentity, effectivelyVisible: true })
      expect(pausePanels.find((panel) => panel.name === "ResumeButton")?.effectivelyVisible).toBeTrue()
      expect(pausePanels.find((panel) => panel.name === "DisconnectButton")?.effectivelyVisible).toBeTrue()
      expect(pausePanels.find((panel) => panel.name === "QuitButton")?.effectivelyVisible).toBeFalse()

      const resumed = gameUi.dispatch({ kind: "escape" })
      expect(resumed).toMatchObject({
        disposition: "applied",
        state: { kind: "pause", pendingRequest: "resume-game" },
        request: { kind: "resume-game" },
      })
      expect(requests).toHaveLength(cycle + 1)
      expect(gameUi.dispatch({ kind: "escape" })).toMatchObject({
        disposition: "ignored",
        state: resumed.state,
        request: null,
        reason: "operation-pending",
      })
      expect(gameUi.dispatch({ kind: "activate-button", button: "resume" })).toMatchObject({
        disposition: "ignored",
        state: resumed.state,
        request: null,
        reason: "operation-pending",
      })
      expect(requests).toHaveLength(cycle + 1)
      expect(gameUi.dispatch({ kind: "gameui-hidden" }).state).toEqual({ kind: "in-game", mapIdentity: "jump_beef" })
      expect(gameUi.snapshot().panels.find((panel) => panel.name === "MMDashboard")).toMatchObject({
        id: dashboardIdentity,
        effectivelyVisible: false,
      })
    }
  })

  test("retains cancellation until teardown and lets a new map supersede it", () => {
    const { gameUi, requests } = createTf2GameUiTransitionFixture()
    gameUi.dispatch({ kind: "loading-started", mapIdentity: "jump_beef" })
    const cancelled = gameUi.dispatch({ kind: "escape" })
    expect(cancelled.state).toEqual({ kind: "disconnecting", mapIdentity: "jump_beef", origin: "loading" })
    expect(requests).toEqual([{ kind: "disconnect" }])
    expect(gameUi.dispatch({ kind: "escape" })).toMatchObject({ disposition: "ignored", request: null, reason: "operation-pending" })
    gameUi.dispatch({ kind: "map", mapIdentity: "pl_upward" })
    expect(requests).toEqual([{ kind: "disconnect" }, { kind: "load-map", mapIdentity: "pl_upward" }])
    expect(gameUi.dispatch({ kind: "loading-started", mapIdentity: "pl_upward" }).state).toMatchObject({
      kind: "loading",
      mapIdentity: "pl_upward",
      progress: 0,
    })
    expect(gameUi.dispatch({ kind: "teardown-confirmed" })).toMatchObject({ disposition: "illegal", request: null })
  })
})

describe("TF2 loading dialog lifecycle", () => {
  test("keeps one modal through milestones, swaps error resources, and releases it once", () => {
    const fixture = createTf2GameUiTransitionFixture()
    const presentation = createTf2LoadingPresentation({
      loadingResource: fixture.resources.panelDocument("resource/loadingdialognobanner.res"),
      failureResource: fixture.resources.panelDocument("resource/loadingdialogerror.res"),
    })
    fixture.gameUi.dispatch({ kind: "loading-started", mapIdentity: "jump_beef" })
    const mounted = presentation.update(1, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)!
    fixture.loading.apply(mounted)
    const mountedPanels = fixture.loading.snapshot().panels
    const dialog = mountedPanels.find((panel) => panel.name === "LoadingDialog")!
    expect(mountedPanels.find((panel) => panel.name === "OnYourWayLabel")?.text).toBe("You're on your way to:")
    expect(mountedPanels.find((panel) => panel.name === "MapLabel")?.text).toBe("JUMP BEEF")
    expect(mountedPanels.find((panel) => panel.name === "MapImage")?.effectivelyVisible).toBeFalse()
    expect(fixture.loading.snapshot().input.applicationModal).toBe(dialog.id)
    expect(fixture.loading.snapshot().popups).toEqual([dialog.id])

    fixture.gameUi.dispatch({ kind: "loading-progress", phase: "reading-world" })
    const progressed = presentation.update(1, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)!
    expect(progressed.operations.some((operation) => operation.kind === "mount")).toBeFalse()
    fixture.loading.apply(progressed)
    expect(fixture.loading.snapshot().input.applicationModal).toBe(dialog.id)

    fixture.gameUi.dispatch({ kind: "loading-failed", reason: "Failed", extendedReason: "Detail" })
    const failure = presentation.update(1, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)!
    expect(failure.operations.filter((operation) => operation.kind === "mount")).toHaveLength(1)
    fixture.loading.apply(failure)
    expect(fixture.loading.snapshot().input.applicationModal).toBe(dialog.id)

    fixture.gameUi.dispatch({ kind: "loading-started", mapIdentity: "pl_upward" })
    const restarted = presentation.update(2, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)!
    expect(restarted.operations.filter((operation) => operation.kind === "mount")).toHaveLength(1)
    fixture.loading.apply(restarted)
    expect(fixture.loading.snapshot().panels.find((panel) => panel.name === "MapLabel")?.text).toBe("Upward")
    expect(fixture.loading.snapshot().panels.find((panel) => panel.name === "MapType")?.text).toBe("Payload")
    expect(fixture.loading.snapshot().input.applicationModal).toBe(dialog.id)

    fixture.gameUi.dispatch({ kind: "loading-succeeded" })
    const finished = presentation.update(2, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)!
    fixture.loading.apply(finished)
    expect(fixture.loading.snapshot().input.applicationModal).toBeNull()
    expect(fixture.loading.snapshot().popups).toEqual([])
    expect(fixture.loading.snapshot().panels.find((panel) => panel.id === dialog.id)).toMatchObject({
      visible: false,
      mouseInput: false,
      keyboardInput: false,
    })
    expect(presentation.update(2, fixture.gameUi.state(), { width: 1_280, height: 720 }, null)).toBeNull()
    expect(presentation.destroy()).toBeNull()
  })

  test("retains the absolute frame timeline while loading roots hide and reactivate", () => {
    const fixture = createTf2GameUiTransitionFixture(() => 0)
    fixture.gameUi.frame(4.25)
    fixture.loading.frame(4.25)
    fixture.gameUi.dispatch({ kind: "loading-started", mapIdentity: "jump_beef" })
    fixture.gameUi.frame(8.5)
    fixture.loading.frame(8.5)
    fixture.gameUi.dispatch({ kind: "loading-succeeded" })
    fixture.gameUi.frame(27.75)
    fixture.loading.frame(27.75)
    fixture.gameUi.dispatch({ kind: "loading-started", mapIdentity: "pl_upward" })
    fixture.gameUi.frame(41.5)
    fixture.loading.frame(41.5)
    expect(fixture.gameUi.snapshot().timeSeconds).toBe(41.5)
    expect(fixture.loading.snapshot().timeSeconds).toBe(41.5)
  })
})
