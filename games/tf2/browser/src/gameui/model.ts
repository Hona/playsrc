const textEncoder = new TextEncoder()

export type Tf2GameUiStateKind =
  | "main-menu"
  | "loading"
  | "in-game"
  | "pause"
  | "disconnecting"
  | "failure"

export type Tf2UnavailableOwner =
  | "event-matchmaking"
  | "casual-matchmaking"
  | "competitive-matchmaking"
  | "mann-vs-machine"
  | "community-server-browser"
  | "community-server-creation"
  | "training"
  | "account-items"
  | "economy-store"

export type Tf2GameUiRequestIdentity =
  | "show-options"
  | "show-advanced-options"
  | "show-play-list"
  | "show-training"
  | "show-equipment"
  | "show-create-server"
  | "resume-game"
  | "disconnect"
  | "quit"
  | "open-new-user-forum"

export type Tf2MenuButtonCapability =
  | Readonly<{ kind: "inactive"; owner: Tf2UnavailableOwner }>
  | Readonly<{ kind: "request"; request: Tf2GameUiRequestIdentity }>

export type Tf2MenuButton = Readonly<{
  identity:
    | "find-game"
    | "quit"
    | "resume"
    | "disconnect"
    | "special-event"
    | "casual"
    | "competitive"
    | "mann-vs-machine"
    | "community-servers"
    | "training"
    | "create-server"
    | "items"
    | "store"
    | "options"
    | "advanced-options"
    | "new-user-forum"
    | "cancel-loading"
  text: string
  sourceCommand: string
  visibility: "visible" | "event-conditional"
  capability: Tf2MenuButtonCapability
}>

export type Tf2MenuPanel = Readonly<{
  identity: "dashboard-main" | "dashboard-pause" | "play-list" | "account" | "settings" | "loading"
  buttons: readonly Tf2MenuButton[]
}>

const inactive = (owner: Tf2UnavailableOwner): Tf2MenuButtonCapability => Object.freeze({ kind: "inactive", owner })
const request = (value: Tf2GameUiRequestIdentity): Tf2MenuButtonCapability =>
  Object.freeze({ kind: "request", request: value })
const button = (
  identity: Tf2MenuButton["identity"],
  text: string,
  sourceCommand: string,
  capability: Tf2MenuButtonCapability,
  visibility: Tf2MenuButton["visibility"] = "visible",
): Tf2MenuButton => Object.freeze({ identity, text, sourceCommand, visibility, capability })
const panel = (identity: Tf2MenuPanel["identity"], buttons: readonly Tf2MenuButton[]): Tf2MenuPanel =>
  Object.freeze({ identity, buttons: Object.freeze([...buttons]) })

const dashboardMain = panel("dashboard-main", [
  button("find-game", "Find a Game", "find_game", request("show-play-list")),
  button("quit", "QUIT", "quit", request("quit")),
])
const dashboardPause = panel("dashboard-pause", [
  button("resume", "Resume", "resume_game", request("resume-game")),
  button("find-game", "Find a Game", "find_game", request("show-play-list")),
  button("disconnect", "Disconnect", "quit", request("disconnect")),
])
const playList = panel("play-list", [
  button("special-event", "Special Event", "play_event", inactive("event-matchmaking"), "event-conditional"),
  button("casual", "Casual", "play_casual", inactive("casual-matchmaking")),
  button("competitive", "Competitive", "play_competitive", inactive("competitive-matchmaking")),
  button("mann-vs-machine", "Mann vs. Machine", "play_mvm", inactive("mann-vs-machine")),
  button("community-servers", "Community Servers", "play_community", inactive("community-server-browser")),
  button("training", "Training", "play_training", request("show-training")),
  button("create-server", "Create Server", "create_server", request("show-create-server")),
])
const account = panel("account", [
  button("items", "ITEMS", "engine open_charinfo", request("show-equipment")),
  button("store", "SHOP", "engine open_store", inactive("economy-store")),
])
const settings = panel("settings", [
  button("options", "Options", "OpenOptionsDialog", request("show-options")),
  button("advanced-options", "Advanced Options", "opentf2options", request("show-advanced-options")),
  button("new-user-forum", "New User Forum", "view_newuser_forums", request("open-new-user-forum")),
])
const loading = panel("loading", [
  button("cancel-loading", "Cancel", "Cancel", request("disconnect")),
])

export const TF2_GAMEUI_PANELS = Object.freeze({
  dashboardMain,
  dashboardPause,
  playList,
  account,
  settings,
  loading,
})

export const TF2_MAIN_MENU_PANELS = Object.freeze([dashboardMain, playList, account, settings])
export const TF2_PAUSE_MENU_PANELS = Object.freeze([dashboardPause, playList, account, settings])
export const TF2_LOADING_PANEL = loading

export type Tf2LoadingPhase =
  | "idle"
  | "changing-map"
  | "starting-local-session"
  | "reading-world"
  | "checking-world"
  | "checking-client"
  | "building-resource-index"
  | "preparing-world"
  | "resetting-world"
  | "initializing-level"
  | "preparing-resources"
  | "activating-session"
  | "opening-local-connection"
  | "negotiating-connection"
  | "establishing-connection"
  | "initializing-client-resources"
  | "receiving-session-info"
  | "receiving-resource-index"
  | "creating-client-world"
  | "sending-player-info"
  | "synchronizing-game-state"
  | "placing-player"
  | "connection-ready"
  | "ready-to-play"
  | "complete"

export type Tf2LoadingPhaseDescriptor = Readonly<{
  identity: Tf2LoadingPhase
  ordinal: number
  baseProgress: number
  repeatCount: number
  statusText: string | null
}>

const phase = (
  identity: Tf2LoadingPhase,
  ordinal: number,
  baseProgress: number,
  repeatCount = 0,
  statusText: string | null = null,
): Tf2LoadingPhaseDescriptor => Object.freeze({ identity, ordinal, baseProgress, repeatCount, statusText })

export const TF2_LOCAL_LOADING_PHASES: readonly Tf2LoadingPhaseDescriptor[] = Object.freeze([
  phase("idle", 0, 0),
  phase("changing-map", 1, 0.02, 0, "Starting local game server..."),
  phase("starting-local-session", 2, 0.02, 0, "Starting local game server..."),
  phase("reading-world", 3, 0.04, 7, "Loading world..."),
  phase("checking-world", 4, 0.23),
  phase("checking-client", 5, 0.23),
  phase("building-resource-index", 6, 0.23),
  phase("preparing-world", 7, 0.23, 0, "Initializing world..."),
  phase("resetting-world", 8, 0.23),
  phase("initializing-level", 9, 0.34, 0, "Loading resources..."),
  phase("preparing-resources", 10, 0.35, 239),
  phase("activating-session", 11, 0.68),
  phase("opening-local-connection", 12, 0.68),
  phase("negotiating-connection", 13, 0.68),
  phase("establishing-connection", 14, 0.7),
  phase("initializing-client-resources", 15, 0.73, 0, "Initializing resources..."),
  phase("receiving-session-info", 16, 0.75),
  phase("receiving-resource-index", 17, 0.77, 12),
  phase("creating-client-world", 18, 0.84),
  phase("sending-player-info", 19, 0.88),
  phase("synchronizing-game-state", 20, 0.91, 0, "Initializing game data..."),
  phase("placing-player", 21, 0.94),
  phase("connection-ready", 22, 0.97),
  phase("ready-to-play", 23, 0.99),
  phase("complete", 24, 1),
])

export type Tf2MainMenuState = Readonly<{
  kind: "main-menu"
  panels: typeof TF2_MAIN_MENU_PANELS
}>
export type Tf2LoadingState = Readonly<{
  kind: "loading"
  mapIdentity: string
  phase: Tf2LoadingPhase
  phaseRepeat: number
  progress: number
  statusText: string
  panel: typeof TF2_LOADING_PANEL
}>
export type Tf2InGameState = Readonly<{
  kind: "in-game"
  mapIdentity: string
}>
export type Tf2PauseState = Readonly<{
  kind: "pause"
  mapIdentity: string
  panels: typeof TF2_PAUSE_MENU_PANELS
  pendingRequest?: "resume-game"
}>
export type Tf2DisconnectingState = Readonly<{
  kind: "disconnecting"
  mapIdentity: string
  origin: "loading" | "pause" | "failure"
}>
export type Tf2Failure = Readonly<{
  reason: string
  extendedReason: string
}>
export type Tf2FailureState = Readonly<{
  kind: "failure"
  mapIdentity: string
  failure: Tf2Failure
}>

export type Tf2GameUiState =
  | Tf2MainMenuState
  | Tf2LoadingState
  | Tf2InGameState
  | Tf2PauseState
  | Tf2DisconnectingState
  | Tf2FailureState

export type Tf2GameUiEvent =
  | Readonly<{ kind: "activate-button"; button: Tf2MenuButton["identity"] }>
  | Readonly<{ kind: "escape" }>
  | Readonly<{ kind: "show-console" }>
  | Readonly<{ kind: "map"; mapIdentity: string }>
  | Readonly<{ kind: "loading-started"; mapIdentity: string }>
  | Readonly<{ kind: "loading-progress"; phase: Tf2LoadingPhase }>
  | Readonly<{ kind: "loading-succeeded" }>
  | Readonly<{ kind: "loading-failed"; reason: string; extendedReason: string }>
  | Readonly<{ kind: "gameui-activated" }>
  | Readonly<{ kind: "gameui-hidden" }>
  | Readonly<{ kind: "teardown-confirmed" }>
  | Readonly<{ kind: "dismiss-failure" }>

export type Tf2GameUiRequest =
  | Readonly<{ kind: "show-console" }>
  | Readonly<{ kind: "show-options"; page: "options" | "advanced-options" }>
  | Readonly<{ kind: "show-play-list" }>
  | Readonly<{ kind: "show-local-match"; entry: "training" | "create-server" }>
  | Readonly<{ kind: "show-equipment" }>
  | Readonly<{ kind: "load-map"; mapIdentity: string }>
  | Readonly<{ kind: "resume-game" }>
  | Readonly<{ kind: "disconnect" }>
  | Readonly<{ kind: "quit" }>
  | Readonly<{
      kind: "open-external-link"
      identity: "new-user-forum"
      href: "https://steamcommunity.com/app/440/discussions/"
    }>

export type Tf2GameUiTransition = Readonly<{
  disposition: "applied" | "ignored" | "inactive" | "illegal"
  state: Tf2GameUiState
  request: Tf2GameUiRequest | null
  reason: Tf2UnavailableOwner | "action-unavailable" | "invalid-map-identity" | "invalid-failure" | "operation-pending" | "progress-regression" | "progress-saturated" | null
}>

export const TF2_MAIN_MENU_STATE: Tf2MainMenuState = Object.freeze({
  kind: "main-menu",
  panels: TF2_MAIN_MENU_PANELS,
})

const transition = (
  disposition: Tf2GameUiTransition["disposition"],
  state: Tf2GameUiState,
  request: Tf2GameUiRequest | null = null,
  reason: Tf2GameUiTransition["reason"] = null,
): Tf2GameUiTransition => Object.freeze({ disposition, state, request, reason })

const isBoundedText = (value: unknown, minimum: number, maximum: number): value is string =>
  typeof value === "string" && !value.includes("\0") && textEncoder.encode(value).byteLength >= minimum && textEncoder.encode(value).byteLength <= maximum

const isMapIdentity = (value: unknown): value is string => isBoundedText(value, 1, 95)
const isFailureText = (value: unknown): value is string => isBoundedText(value, 0, 255)

const loadingState = (mapIdentity: string): Tf2LoadingState => Object.freeze({
  kind: "loading",
  mapIdentity,
  phase: "idle",
  phaseRepeat: 0,
  progress: 0,
  statusText: "",
  panel: TF2_LOADING_PANEL,
})
const inGameState = (mapIdentity: string): Tf2InGameState => Object.freeze({ kind: "in-game", mapIdentity })
const pauseState = (mapIdentity: string): Tf2PauseState => Object.freeze({
  kind: "pause",
  mapIdentity,
  panels: TF2_PAUSE_MENU_PANELS,
})

const currentPanels = (state: Tf2GameUiState): readonly Tf2MenuPanel[] => {
  if (state.kind === "main-menu" || state.kind === "pause") return state.panels
  if (state.kind === "loading") return Object.freeze([state.panel])
  return Object.freeze([])
}

const findButton = (state: Tf2GameUiState, identity: Tf2MenuButton["identity"]): Tf2MenuButton | undefined => {
  for (const value of currentPanels(state)) {
    const found = value.buttons.find((candidate) => candidate.identity === identity)
    if (found) return found
  }
  return undefined
}

const buttonRequest = (identity: Tf2GameUiRequestIdentity): Tf2GameUiRequest => {
  switch (identity) {
    case "show-options": return Object.freeze({ kind: "show-options", page: "options" })
    case "show-advanced-options": return Object.freeze({ kind: "show-options", page: "advanced-options" })
    case "show-play-list": return Object.freeze({ kind: "show-play-list" })
    case "show-training": return Object.freeze({ kind: "show-local-match", entry: "training" })
    case "show-equipment": return Object.freeze({ kind: "show-equipment" })
    case "show-create-server": return Object.freeze({ kind: "show-local-match", entry: "create-server" })
    case "resume-game": return Object.freeze({ kind: "resume-game" })
    case "disconnect": return Object.freeze({ kind: "disconnect" })
    case "quit": return Object.freeze({ kind: "quit" })
    case "open-new-user-forum": return Object.freeze({
      kind: "open-external-link",
      identity: "new-user-forum",
      href: "https://steamcommunity.com/app/440/discussions/",
    })
  }
}

const activateButton = (
  state: Tf2GameUiState,
  identity: Tf2MenuButton["identity"],
): Tf2GameUiTransition => {
  const selected = findButton(state, identity)
  if (!selected) return transition("illegal", state, null, "action-unavailable")
  if (selected.capability.kind === "inactive") {
    return transition("inactive", state, null, selected.capability.owner)
  }
  if (state.kind === "pause" && state.pendingRequest === "resume-game") {
    return transition("ignored", state, null, "operation-pending")
  }
  const emitted = buttonRequest(selected.capability.request)
  if (identity === "resume" && state.kind === "pause") {
    return transition("applied", Object.freeze({ ...state, pendingRequest: "resume-game" }), emitted)
  }
  if (identity === "disconnect" && state.kind === "pause") {
    return transition("applied", Object.freeze({
      kind: "disconnecting",
      mapIdentity: state.mapIdentity,
      origin: "pause",
    }), emitted)
  }
  if (identity === "cancel-loading" && state.kind === "loading") {
    return transition("applied", Object.freeze({
      kind: "disconnecting",
      mapIdentity: state.mapIdentity,
      origin: "loading",
    }), emitted)
  }
  return transition("applied", state, emitted)
}

const escape = (state: Tf2GameUiState): Tf2GameUiTransition => {
  switch (state.kind) {
    case "main-menu":
      return transition("ignored", state)
    case "loading":
      return activateButton(state, "cancel-loading")
    case "in-game":
      return transition("applied", pauseState(state.mapIdentity))
    case "pause":
      return activateButton(state, "resume")
    case "disconnecting":
      return transition("ignored", state, null, "operation-pending")
    case "failure":
      return transition("applied", Object.freeze({
        kind: "disconnecting",
        mapIdentity: state.mapIdentity,
        origin: "failure",
      }), Object.freeze({ kind: "disconnect" }))
  }
}

const advanceLoading = (state: Tf2LoadingState, identity: Tf2LoadingPhase): Tf2GameUiTransition => {
  const currentIndex = TF2_LOCAL_LOADING_PHASES.findIndex((value) => value.identity === state.phase)
  const nextIndex = TF2_LOCAL_LOADING_PHASES.findIndex((value) => value.identity === identity)
  if (nextIndex < 0) return transition("illegal", state, null, "action-unavailable")
  if (nextIndex < currentIndex) return transition("ignored", state, null, "progress-regression")

  const descriptor = TF2_LOCAL_LOADING_PHASES[nextIndex]
  if (!descriptor) return transition("illegal", state, null, "action-unavailable")
  const repeat = nextIndex === currentIndex
    ? Math.min(state.phaseRepeat + 1, descriptor.repeatCount)
    : 0
  let progress = descriptor.baseProgress
  if (descriptor.repeatCount > 1 && repeat > 0) {
    const following = TF2_LOCAL_LOADING_PHASES[nextIndex + 1]
    if (following) progress += (following.baseProgress - progress) * (repeat / descriptor.repeatCount)
  }
  const statusText = descriptor.statusText ?? state.statusText
  if (
    identity === state.phase
    && repeat === state.phaseRepeat
    && progress === state.progress
    && statusText === state.statusText
  ) return transition("ignored", state, null, "progress-saturated")

  return transition("applied", Object.freeze({
    ...state,
    phase: identity,
    phaseRepeat: repeat,
    progress,
    statusText,
  }))
}

export function transitionTf2GameUi(state: Tf2GameUiState, event: Tf2GameUiEvent): Tf2GameUiTransition {
  switch (event.kind) {
    case "activate-button":
      return activateButton(state, event.button)
    case "escape":
      return escape(state)
    case "show-console":
      if (state.kind !== "main-menu" && state.kind !== "in-game" && state.kind !== "pause") {
        return transition("illegal", state, null, "action-unavailable")
      }
      return transition("applied", state, Object.freeze({ kind: "show-console" }))
    case "map":
      if (!isMapIdentity(event.mapIdentity)) return transition("illegal", state, null, "invalid-map-identity")
      return transition("applied", state, Object.freeze({ kind: "load-map", mapIdentity: event.mapIdentity }))
    case "loading-started":
      if (!isMapIdentity(event.mapIdentity)) return transition("illegal", state, null, "invalid-map-identity")
      return transition("applied", loadingState(event.mapIdentity))
    case "loading-progress":
      return state.kind === "loading"
        ? advanceLoading(state, event.phase)
        : transition("illegal", state, null, "action-unavailable")
    case "loading-succeeded":
      return state.kind === "loading"
        ? transition("applied", inGameState(state.mapIdentity))
        : transition("illegal", state, null, "action-unavailable")
    case "loading-failed":
      if (state.kind !== "loading") return transition("illegal", state, null, "action-unavailable")
      if (!isFailureText(event.reason) || !isFailureText(event.extendedReason)) {
        return transition("illegal", state, null, "invalid-failure")
      }
      return transition("applied", Object.freeze({
        kind: "failure",
        mapIdentity: state.mapIdentity,
        failure: Object.freeze({ reason: event.reason, extendedReason: event.extendedReason }),
      }))
    case "gameui-activated":
      return state.kind === "in-game"
        ? transition("applied", pauseState(state.mapIdentity))
        : transition("illegal", state, null, "action-unavailable")
    case "gameui-hidden":
      return state.kind === "pause"
        ? transition("applied", inGameState(state.mapIdentity))
        : transition("illegal", state, null, "action-unavailable")
    case "teardown-confirmed":
      return state.kind === "disconnecting"
        ? transition("applied", TF2_MAIN_MENU_STATE)
        : transition("illegal", state, null, "action-unavailable")
    case "dismiss-failure":
      return state.kind === "failure"
        ? transition("applied", TF2_MAIN_MENU_STATE)
        : transition("illegal", state, null, "action-unavailable")
  }
}
