import { describe, expect, test } from "bun:test"
import {
  TF2_GAMEUI_PANELS,
  TF2_LOCAL_LOADING_PHASES,
  TF2_MAIN_MENU_STATE,
  transitionTf2GameUi,
  type Tf2GameUiEvent,
  type Tf2GameUiState,
  type Tf2GameUiTransition,
  type Tf2LoadingState,
  type Tf2MenuButton,
} from "../../src/gameui"

function applied(state: Tf2GameUiState, event: Tf2GameUiEvent): Tf2GameUiTransition {
  const result = transitionTf2GameUi(state, event)
  expect(result.disposition).toBe("applied")
  return result
}

function loading(mapIdentity = "jump_beef"): Tf2LoadingState {
  return applied(TF2_MAIN_MENU_STATE, { kind: "loading-started", mapIdentity }).state as Tf2LoadingState
}

function states(): Record<Tf2GameUiState["kind"], Tf2GameUiState> {
  const load = loading()
  const inGame = applied(load, { kind: "loading-succeeded" }).state
  const pause = applied(inGame, { kind: "gameui-activated" }).state
  const disconnecting = applied(pause, { kind: "activate-button", button: "disconnect" }).state
  const failure = applied(load, { kind: "loading-failed", reason: "Failed", extendedReason: "Detail" }).state
  return {
    "main-menu": TF2_MAIN_MENU_STATE,
    loading: load,
    "in-game": inGame,
    pause,
    disconnecting,
    failure,
  }
}

function recursivelyFrozen(value: unknown, visited = new Set<unknown>()): boolean {
  if ((typeof value !== "object" && typeof value !== "function") || value === null || visited.has(value)) return true
  visited.add(value)
  if (!Object.isFrozen(value)) return false
  return Reflect.ownKeys(value).every((key) => recursivelyFrozen(Reflect.get(value, key), visited))
}

describe("TF2 configured GameUI actions", () => {
  test("preserves panel order, text, source command, visibility, and capability", () => {
    const snapshot = Object.fromEntries(
      Object.entries(TF2_GAMEUI_PANELS).map(([key, value]) => [
        key,
        value.buttons.map(({ identity, text, sourceCommand, visibility, capability }) => ({
          identity,
          text,
          sourceCommand,
          visibility,
          capability,
        })),
      ]),
    )
    expect(snapshot).toEqual({
      dashboardMain: [
        { identity: "find-game", text: "Find a Game", sourceCommand: "find_game", visibility: "visible", capability: { kind: "request", request: "show-play-list" } },
        { identity: "quit", text: "QUIT", sourceCommand: "quit", visibility: "visible", capability: { kind: "request", request: "quit" } },
      ],
      dashboardPause: [
        { identity: "resume", text: "Resume", sourceCommand: "resume_game", visibility: "visible", capability: { kind: "request", request: "resume-game" } },
        { identity: "find-game", text: "Find a Game", sourceCommand: "find_game", visibility: "visible", capability: { kind: "request", request: "show-play-list" } },
        { identity: "disconnect", text: "Disconnect", sourceCommand: "quit", visibility: "visible", capability: { kind: "request", request: "disconnect" } },
      ],
      playList: [
        { identity: "special-event", text: "Special Event", sourceCommand: "play_event", visibility: "event-conditional", capability: { kind: "inactive", owner: "event-matchmaking" } },
        { identity: "casual", text: "Casual", sourceCommand: "play_casual", visibility: "visible", capability: { kind: "inactive", owner: "casual-matchmaking" } },
        { identity: "competitive", text: "Competitive", sourceCommand: "play_competitive", visibility: "visible", capability: { kind: "inactive", owner: "competitive-matchmaking" } },
        { identity: "mann-vs-machine", text: "Mann vs. Machine", sourceCommand: "play_mvm", visibility: "visible", capability: { kind: "inactive", owner: "mann-vs-machine" } },
        { identity: "community-servers", text: "Community Servers", sourceCommand: "play_community", visibility: "visible", capability: { kind: "inactive", owner: "community-server-browser" } },
        { identity: "training", text: "Training", sourceCommand: "play_training", visibility: "visible", capability: { kind: "request", request: "show-training" } },
        { identity: "create-server", text: "Create Server", sourceCommand: "create_server", visibility: "visible", capability: { kind: "request", request: "show-create-server" } },
      ],
      account: [
        { identity: "items", text: "ITEMS", sourceCommand: "engine open_charinfo", visibility: "visible", capability: { kind: "request", request: "show-equipment" } },
        { identity: "store", text: "SHOP", sourceCommand: "engine open_store", visibility: "visible", capability: { kind: "inactive", owner: "economy-store" } },
      ],
      settings: [
        { identity: "options", text: "Options", sourceCommand: "OpenOptionsDialog", visibility: "visible", capability: { kind: "request", request: "show-options" } },
        { identity: "advanced-options", text: "Advanced Options", sourceCommand: "opentf2options", visibility: "visible", capability: { kind: "request", request: "show-advanced-options" } },
        { identity: "new-user-forum", text: "New User Forum", sourceCommand: "view_newuser_forums", visibility: "visible", capability: { kind: "request", request: "open-new-user-forum" } },
      ],
      loading: [
        { identity: "cancel-loading", text: "Cancel", sourceCommand: "Cancel", visibility: "visible", capability: { kind: "request", request: "disconnect" } },
      ],
    })
    expect(recursivelyFrozen(TF2_GAMEUI_PANELS)).toBe(true)
  })

  test("keeps every unavailable service inactive and side-effect free", () => {
    const unavailable: readonly Tf2MenuButton["identity"][] = [
      "special-event",
      "casual",
      "competitive",
      "mann-vs-machine",
      "community-servers",
      "store",
    ]
    for (const button of unavailable) {
      const result = transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button })
      expect(result.disposition, button).toBe("inactive")
      expect(result.state, button).toBe(TF2_MAIN_MENU_STATE)
      expect(result.request, button).toBeNull()
      expect(result.reason, button).not.toBeNull()
    }
  })

  test("emits typed requests without executing owner behavior", () => {
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "show-console" }).request).toEqual({ kind: "show-console" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "find-game" }).request)
      .toEqual({ kind: "show-play-list" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "options" }).request)
      .toEqual({ kind: "show-options", page: "options" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "advanced-options" }).request)
      .toEqual({ kind: "show-options", page: "advanced-options" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "training" }).request)
      .toEqual({ kind: "show-local-match", entry: "training" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "create-server" }).request)
      .toEqual({ kind: "show-local-match", entry: "create-server" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "new-user-forum" }).request)
      .toEqual({
        kind: "open-external-link",
        identity: "new-user-forum",
        href: "https://steamcommunity.com/app/440/discussions/",
      })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "quit" }).request)
      .toEqual({ kind: "quit" })
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "map", mapIdentity: "jump_beef" }).request)
      .toEqual({ kind: "load-map", mapIdentity: "jump_beef" })
  })
})

describe("TF2 GameUI transition model", () => {
  test("classifies every configured button in every state", () => {
    const fixtures = states()
    const identities = [...new Set(
      Object.values(TF2_GAMEUI_PANELS).flatMap((value) => value.buttons.map((candidate) => candidate.identity)),
    )]
    const contexts: Record<Tf2MenuButton["identity"], readonly Tf2GameUiState["kind"][]> = {
      "find-game": ["main-menu", "pause"],
      quit: ["main-menu"],
      resume: ["pause"],
      disconnect: ["pause"],
      "special-event": ["main-menu", "pause"],
      casual: ["main-menu", "pause"],
      competitive: ["main-menu", "pause"],
      "mann-vs-machine": ["main-menu", "pause"],
      "community-servers": ["main-menu", "pause"],
      training: ["main-menu", "pause"],
      "create-server": ["main-menu", "pause"],
      items: ["main-menu", "pause"],
      store: ["main-menu", "pause"],
      options: ["main-menu", "pause"],
      "advanced-options": ["main-menu", "pause"],
      "new-user-forum": ["main-menu", "pause"],
      "cancel-loading": ["loading"],
    }
    const inactive = new Set<Tf2MenuButton["identity"]>([
      "special-event",
      "casual",
      "competitive",
      "mann-vs-machine",
      "community-servers",
      "store",
    ])

    for (const identity of identities) {
      for (const [kind, state] of Object.entries(fixtures) as [Tf2GameUiState["kind"], Tf2GameUiState][]) {
        const result = transitionTf2GameUi(state, { kind: "activate-button", button: identity })
        if (contexts[identity].includes(kind)) {
          expect(result.disposition, `${identity} in ${kind}`).toBe(inactive.has(identity) ? "inactive" : "applied")
        } else {
          expect(result, `${identity} in ${kind}`).toMatchObject({
            disposition: "illegal",
            state,
            request: null,
            reason: "action-unavailable",
          })
          expect(result.state).toBe(state)
        }
      }
    }
  })

  test("classifies every lifecycle event in every state", () => {
    const fixtures = states()
    const events: readonly Readonly<{
      event: Tf2GameUiEvent
      accepted: readonly Tf2GameUiState["kind"][]
    }>[] = [
      { event: { kind: "escape" }, accepted: ["main-menu", "loading", "in-game", "pause", "disconnecting", "failure"] },
      { event: { kind: "show-console" }, accepted: ["main-menu", "in-game", "pause"] },
      { event: { kind: "map", mapIdentity: "jump_beef" }, accepted: ["main-menu", "loading", "in-game", "pause", "disconnecting", "failure"] },
      { event: { kind: "loading-started", mapIdentity: "jump_beef" }, accepted: ["main-menu", "loading", "in-game", "pause", "disconnecting", "failure"] },
      { event: { kind: "loading-progress", phase: "reading-world" }, accepted: ["loading"] },
      { event: { kind: "loading-succeeded" }, accepted: ["loading"] },
      { event: { kind: "loading-failed", reason: "Failed", extendedReason: "Detail" }, accepted: ["loading"] },
      { event: { kind: "gameui-activated" }, accepted: ["in-game"] },
      { event: { kind: "gameui-hidden" }, accepted: ["pause"] },
      { event: { kind: "teardown-confirmed" }, accepted: ["disconnecting"] },
      { event: { kind: "dismiss-failure" }, accepted: ["failure"] },
    ]

    for (const { event, accepted } of events) {
      for (const [kind, state] of Object.entries(fixtures) as [Tf2GameUiState["kind"], Tf2GameUiState][]) {
        const result = transitionTf2GameUi(state, event)
        if (accepted.includes(kind)) {
          expect(result.disposition, `${event.kind} in ${kind}`).not.toBe("illegal")
        } else {
          expect(result, `${event.kind} in ${kind}`).toMatchObject({
            disposition: "illegal",
            state,
            request: null,
            reason: "action-unavailable",
          })
          expect(result.state).toBe(state)
        }
      }
    }
  })

  test("requires owner acknowledgements for resume and disconnect completion", () => {
    const inGame = states()["in-game"]
    const pause = applied(inGame, { kind: "gameui-activated" }).state
    const resume = transitionTf2GameUi(pause, { kind: "activate-button", button: "resume" })
    expect(resume).toMatchObject({
      disposition: "applied",
      state: { kind: "pause", mapIdentity: "jump_beef", pendingRequest: "resume-game" },
      request: { kind: "resume-game" },
    })
    expect(transitionTf2GameUi(resume.state, { kind: "activate-button", button: "resume" })).toMatchObject({
      disposition: "ignored",
      state: resume.state,
      request: null,
      reason: "operation-pending",
    })
    const resumed = applied(resume.state, { kind: "gameui-hidden" }).state
    expect(resumed).toEqual({ kind: "in-game", mapIdentity: "jump_beef" })

    const disconnect = applied(pause, { kind: "activate-button", button: "disconnect" })
    expect(disconnect.request).toEqual({ kind: "disconnect" })
    expect(disconnect.state).toEqual({ kind: "disconnecting", mapIdentity: "jump_beef", origin: "pause" })
    expect(transitionTf2GameUi(disconnect.state, { kind: "activate-button", button: "disconnect" })).toMatchObject({
      disposition: "illegal",
      state: disconnect.state,
      request: null,
    })
    expect(applied(disconnect.state, { kind: "teardown-confirmed" }).state).toBe(TF2_MAIN_MENU_STATE)
  })

  test("validates map and failure C-string bounds atomically", () => {
    const acceptedMap = "m".repeat(95)
    expect(transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "map", mapIdentity: acceptedMap }).request)
      .toEqual({ kind: "load-map", mapIdentity: acceptedMap })
    for (const mapIdentity of ["", "m".repeat(96), "bad\0map", "é".repeat(48)]) {
      const result = transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "map", mapIdentity })
      expect(result).toMatchObject({ disposition: "illegal", state: TF2_MAIN_MENU_STATE, request: null, reason: "invalid-map-identity" })
      expect(result.state).toBe(TF2_MAIN_MENU_STATE)
    }

    const state = loading()
    expect(transitionTf2GameUi(state, { kind: "loading-failed", reason: "r".repeat(255), extendedReason: "" }).disposition)
      .toBe("applied")
    for (const event of [
      { kind: "loading-failed", reason: "r".repeat(256), extendedReason: "" },
      { kind: "loading-failed", reason: "bad\0reason", extendedReason: "" },
      { kind: "loading-failed", reason: "", extendedReason: "é".repeat(128) },
    ] as const) {
      const result = transitionTf2GameUi(state, event)
      expect(result).toMatchObject({ disposition: "illegal", state, request: null, reason: "invalid-failure" })
      expect(result.state).toBe(state)
    }
  })
})

describe("TF2 local loading model", () => {
  test("publishes the exact ordered milestone contract", () => {
    expect(TF2_LOCAL_LOADING_PHASES).toEqual([
      { identity: "idle", ordinal: 0, baseProgress: 0, repeatCount: 0, statusText: null },
      { identity: "changing-map", ordinal: 1, baseProgress: 0.02, repeatCount: 0, statusText: "Starting local game server..." },
      { identity: "starting-local-session", ordinal: 2, baseProgress: 0.02, repeatCount: 0, statusText: "Starting local game server..." },
      { identity: "reading-world", ordinal: 3, baseProgress: 0.04, repeatCount: 7, statusText: "Loading world..." },
      { identity: "checking-world", ordinal: 4, baseProgress: 0.23, repeatCount: 0, statusText: null },
      { identity: "checking-client", ordinal: 5, baseProgress: 0.23, repeatCount: 0, statusText: null },
      { identity: "building-resource-index", ordinal: 6, baseProgress: 0.23, repeatCount: 0, statusText: null },
      { identity: "preparing-world", ordinal: 7, baseProgress: 0.23, repeatCount: 0, statusText: "Initializing world..." },
      { identity: "resetting-world", ordinal: 8, baseProgress: 0.23, repeatCount: 0, statusText: null },
      { identity: "initializing-level", ordinal: 9, baseProgress: 0.34, repeatCount: 0, statusText: "Loading resources..." },
      { identity: "preparing-resources", ordinal: 10, baseProgress: 0.35, repeatCount: 239, statusText: null },
      { identity: "activating-session", ordinal: 11, baseProgress: 0.68, repeatCount: 0, statusText: null },
      { identity: "opening-local-connection", ordinal: 12, baseProgress: 0.68, repeatCount: 0, statusText: null },
      { identity: "negotiating-connection", ordinal: 13, baseProgress: 0.68, repeatCount: 0, statusText: null },
      { identity: "establishing-connection", ordinal: 14, baseProgress: 0.7, repeatCount: 0, statusText: null },
      { identity: "initializing-client-resources", ordinal: 15, baseProgress: 0.73, repeatCount: 0, statusText: "Initializing resources..." },
      { identity: "receiving-session-info", ordinal: 16, baseProgress: 0.75, repeatCount: 0, statusText: null },
      { identity: "receiving-resource-index", ordinal: 17, baseProgress: 0.77, repeatCount: 12, statusText: null },
      { identity: "creating-client-world", ordinal: 18, baseProgress: 0.84, repeatCount: 0, statusText: null },
      { identity: "sending-player-info", ordinal: 19, baseProgress: 0.88, repeatCount: 0, statusText: null },
      { identity: "synchronizing-game-state", ordinal: 20, baseProgress: 0.91, repeatCount: 0, statusText: "Initializing game data..." },
      { identity: "placing-player", ordinal: 21, baseProgress: 0.94, repeatCount: 0, statusText: null },
      { identity: "connection-ready", ordinal: 22, baseProgress: 0.97, repeatCount: 0, statusText: null },
      { identity: "ready-to-play", ordinal: 23, baseProgress: 0.99, repeatCount: 0, statusText: null },
      { identity: "complete", ordinal: 24, baseProgress: 1, repeatCount: 0, statusText: null },
    ])
    expect(recursivelyFrozen(TF2_LOCAL_LOADING_PHASES)).toBe(true)
  })

  test("interpolates repeats, retains status, ignores regression, and caps progress", () => {
    let state = loading()
    expect(state).toMatchObject({ phase: "idle", phaseRepeat: 0, progress: 0, statusText: "" })
    const unknown = transitionTf2GameUi(state, { kind: "loading-progress", phase: "unknown" as never })
    expect(unknown).toMatchObject({ disposition: "illegal", state, request: null, reason: "action-unavailable" })
    expect(unknown.state).toBe(state)
    state = applied(state, { kind: "loading-progress", phase: "reading-world" }).state as Tf2LoadingState
    expect(state).toMatchObject({ phase: "reading-world", phaseRepeat: 0, progress: 0.04, statusText: "Loading world..." })
    for (let repeat = 1; repeat <= 7; repeat++) {
      state = applied(state, { kind: "loading-progress", phase: "reading-world" }).state as Tf2LoadingState
      expect(state.phaseRepeat).toBe(repeat)
      expect(state.progress).toBeCloseTo(0.04 + (0.23 - 0.04) * (repeat / 7), 12)
    }
    const saturated = transitionTf2GameUi(state, { kind: "loading-progress", phase: "reading-world" })
    expect(saturated).toMatchObject({ disposition: "ignored", state, request: null, reason: "progress-saturated" })
    expect(saturated.state).toBe(state)

    state = applied(state, { kind: "loading-progress", phase: "checking-world" }).state as Tf2LoadingState
    expect(state.statusText).toBe("Loading world...")
    const regressed = transitionTf2GameUi(state, { kind: "loading-progress", phase: "starting-local-session" })
    expect(regressed).toMatchObject({ disposition: "ignored", state, request: null, reason: "progress-regression" })
    expect(regressed.state).toBe(state)

    state = applied(state, { kind: "loading-progress", phase: "initializing-level" }).state as Tf2LoadingState
    state = applied(state, { kind: "loading-progress", phase: "preparing-resources" }).state as Tf2LoadingState
    expect(state.statusText).toBe("Loading resources...")
    for (let repeat = 0; repeat < 239; repeat++) {
      state = applied(state, { kind: "loading-progress", phase: "preparing-resources" }).state as Tf2LoadingState
    }
    expect(state).toMatchObject({ phaseRepeat: 239, progress: 0.68, statusText: "Loading resources..." })

    state = applied(state, { kind: "loading-progress", phase: "receiving-resource-index" }).state as Tf2LoadingState
    for (let repeat = 0; repeat < 12; repeat++) {
      state = applied(state, { kind: "loading-progress", phase: "receiving-resource-index" }).state as Tf2LoadingState
    }
    expect(state).toMatchObject({ phaseRepeat: 12, progress: 0.84 })
    state = applied(state, { kind: "loading-progress", phase: "complete" }).state as Tf2LoadingState
    expect(state).toMatchObject({ phase: "complete", progress: 1, statusText: "Loading resources..." })
  })

  test("preserves exact failure and routes Cancel through pending teardown", () => {
    const state = loading("cp_badlands")
    const failure = applied(state, {
      kind: "loading-failed",
      reason: "Connection failed",
      extendedReason: "Server unavailable",
    }).state
    expect(failure).toEqual({
      kind: "failure",
      mapIdentity: "cp_badlands",
      failure: { reason: "Connection failed", extendedReason: "Server unavailable" },
    })
    expect(applied(failure, { kind: "dismiss-failure" }).state).toBe(TF2_MAIN_MENU_STATE)

    const cancelled = applied(state, { kind: "activate-button", button: "cancel-loading" })
    expect(cancelled.request).toEqual({ kind: "disconnect" })
    expect(cancelled.state).toEqual({ kind: "disconnecting", mapIdentity: "cp_badlands", origin: "loading" })
    expect(applied(cancelled.state, { kind: "teardown-confirmed" }).state).toBe(TF2_MAIN_MENU_STATE)
  })
})

describe("TF2 GameUI exact transcript and immutability", () => {
  test("runs console to map to pause to owner-confirmed disconnect", () => {
    const transcript: unknown[] = []
    let state: Tf2GameUiState = TF2_MAIN_MENU_STATE

    let result = applied(state, { kind: "show-console" })
    transcript.push([state.kind, result.request?.kind, result.state.kind])
    state = result.state
    result = applied(state, { kind: "map", mapIdentity: "jump_beef" })
    transcript.push([state.kind, result.request, result.state.kind])
    state = applied(state, { kind: "loading-started", mapIdentity: "jump_beef" }).state
    transcript.push([state.kind, (state as Tf2LoadingState).mapIdentity, (state as Tf2LoadingState).progress])
    state = applied(state, { kind: "loading-progress", phase: "starting-local-session" }).state
    state = applied(state, { kind: "loading-progress", phase: "ready-to-play" }).state
    state = applied(state, { kind: "loading-succeeded" }).state
    transcript.push([state.kind, "mapIdentity" in state ? state.mapIdentity : null])
    state = applied(state, { kind: "gameui-activated" }).state
    transcript.push([state.kind, "mapIdentity" in state ? state.mapIdentity : null])
    result = applied(state, { kind: "activate-button", button: "disconnect" })
    transcript.push([result.state.kind, result.request])
    state = applied(result.state, { kind: "teardown-confirmed" }).state
    transcript.push([state.kind])

    expect(transcript).toEqual([
      ["main-menu", "show-console", "main-menu"],
      ["main-menu", { kind: "load-map", mapIdentity: "jump_beef" }, "main-menu"],
      ["loading", "jump_beef", 0],
      ["in-game", "jump_beef"],
      ["pause", "jump_beef"],
      ["disconnecting", { kind: "disconnect" }],
      ["main-menu"],
    ])
  })

  test("freezes every published value and does not mutate events", () => {
    const mapEvent = { kind: "loading-started" as const, mapIdentity: "jump_beef" }
    const loadResult = transitionTf2GameUi(TF2_MAIN_MENU_STATE, mapEvent)
    expect(mapEvent).toEqual({ kind: "loading-started", mapIdentity: "jump_beef" })
    const progressResult = transitionTf2GameUi(loadResult.state, { kind: "loading-progress", phase: "reading-world" })
    const failureResult = transitionTf2GameUi(progressResult.state, {
      kind: "loading-failed",
      reason: "failure",
      extendedReason: "detail",
    })
    const requestResult = transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "activate-button", button: "new-user-forum" })
    for (const value of [TF2_MAIN_MENU_STATE, loadResult, progressResult, failureResult, requestResult]) {
      expect(recursivelyFrozen(value)).toBe(true)
    }
  })
})
