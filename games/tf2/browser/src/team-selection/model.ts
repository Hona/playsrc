export type Tf2TeamIdentity = 0 | 1 | 2 | 3
export type Tf2TeamChoice = "red" | "blue" | "spectate" | "auto"

export type Tf2TeamSelectionServerState = Readonly<{
  localTeam: Tf2TeamIdentity
  redCount: number
  blueCount: number
  redDisabled: boolean
  blueDisabled: boolean
  spectatorsVisible: boolean
  autoAssignVisible: boolean
  cancelVisible: boolean
  highlander: boolean
  teamsFull: boolean
  teamsFullArrow: boolean
}>

export type Tf2TeamSelectionState = Readonly<{
  visible: boolean
  server: Tf2TeamSelectionServerState | null
  focused: Tf2TeamChoice | null
  hovered: Tf2TeamChoice | null
}>

export type Tf2TeamSelectionEvent =
  | Readonly<{ kind: "show"; server: Tf2TeamSelectionServerState }>
  | Readonly<{ kind: "update"; server: Tf2TeamSelectionServerState }>
  | Readonly<{ kind: "select"; team: Tf2TeamChoice }>
  | Readonly<{ kind: "focus"; team: Tf2TeamChoice }>
  | Readonly<{ kind: "hover"; team: Tf2TeamChoice | null }>
  | Readonly<{ kind: "cancel" }>
  | Readonly<{ kind: "hide" }>

export type Tf2TeamSelectionRequest = Readonly<{
  kind: "join-team"
  team: Tf2TeamChoice
  sourceCommand: `jointeam ${Tf2TeamChoice}`
}>

export type Tf2TeamSelectionTransition = Readonly<{
  disposition: "applied" | "ignored" | "illegal"
  state: Tf2TeamSelectionState
  request: Tf2TeamSelectionRequest | null
}>

export function decodeTf2TeamSelectionServerState(
  localTeam: number,
  redCount: number,
  blueCount: number,
  flags: number,
): Tf2TeamSelectionServerState {
  if (![localTeam, redCount, blueCount, flags].every(Number.isSafeInteger)
    || localTeam < 0 || localTeam > 3 || flags < 0 || flags > 255) {
    throw new TypeError("TF2 authoritative team-selection snapshot is malformed")
  }
  const state: Tf2TeamSelectionServerState = Object.freeze({
    localTeam: localTeam as Tf2TeamIdentity,
    redCount,
    blueCount,
    redDisabled: Boolean(flags & 1),
    blueDisabled: Boolean(flags & 2),
    spectatorsVisible: Boolean(flags & 4),
    autoAssignVisible: Boolean(flags & 8),
    cancelVisible: Boolean(flags & 16),
    highlander: Boolean(flags & 32),
    teamsFull: Boolean(flags & 64),
    teamsFullArrow: Boolean(flags & 128),
  })
  if (!validServer(state)) throw new TypeError("TF2 authoritative team-selection snapshot is inconsistent")
  return state
}

export const TF2_TEAM_SELECTION_INITIAL_STATE: Tf2TeamSelectionState = Object.freeze({
  visible: false,
  server: null,
  focused: null,
  hovered: null,
})

const outcome = (
  disposition: Tf2TeamSelectionTransition["disposition"],
  state: Tf2TeamSelectionState,
  request: Tf2TeamSelectionRequest | null = null,
): Tf2TeamSelectionTransition => Object.freeze({ disposition, state, request })

function validServer(state: Tf2TeamSelectionServerState): boolean {
  return [0, 1, 2, 3].includes(state.localTeam)
    && Number.isSafeInteger(state.redCount) && state.redCount >= 0 && state.redCount <= 64
    && Number.isSafeInteger(state.blueCount) && state.blueCount >= 0 && state.blueCount <= 64
    && state.redCount + state.blueCount <= 64
    && state.cancelVisible === (state.localTeam !== 0)
    && (!state.teamsFull || state.highlander)
    && (!state.teamsFullArrow || (state.teamsFull && state.spectatorsVisible))
}

function available(state: Tf2TeamSelectionServerState, team: Tf2TeamChoice): boolean {
  if (team === "red") return !state.redDisabled
  if (team === "blue") return !state.blueDisabled
  if (team === "spectate") return state.spectatorsVisible
  return state.autoAssignVisible
}

function initialFocus(server: Tf2TeamSelectionServerState): Tf2TeamChoice | null {
  if (server.localTeam === 3) return "blue"
  if (server.localTeam === 2) return "red"
  if (server.autoAssignVisible) return "auto"
  if (!server.blueDisabled) return "blue"
  if (!server.redDisabled) return "red"
  return server.spectatorsVisible ? "spectate" : null
}

export function transitionTf2TeamSelection(
  state: Tf2TeamSelectionState,
  event: Tf2TeamSelectionEvent,
): Tf2TeamSelectionTransition {
  if (event.kind === "show") {
    if (!validServer(event.server)) return outcome("illegal", state)
    return outcome("applied", Object.freeze({ visible: true, server: event.server, focused: initialFocus(event.server), hovered: null }))
  }
  if (event.kind === "hide") {
    if (!state.visible) return outcome("ignored", state)
    return outcome("applied", Object.freeze({ ...state, visible: false, hovered: null }))
  }
  if (!state.visible || !state.server) return outcome("ignored", state)
  if (event.kind === "update") {
    if (!validServer(event.server)) return outcome("illegal", state)
    const focused = state.focused && available(event.server, state.focused) ? state.focused : initialFocus(event.server)
    const hovered = state.hovered && available(event.server, state.hovered) ? state.hovered : null
    return outcome("applied", Object.freeze({ ...state, server: event.server, focused, hovered }))
  }
  if (event.kind === "cancel") {
    if (!state.server.cancelVisible) return outcome("ignored", state)
    return outcome("applied", Object.freeze({ ...state, visible: false, hovered: null }))
  }
  if (event.kind === "hover") {
    if (event.team !== null && (event.team === "auto" || event.team === "spectate") && !available(state.server, event.team)) {
      return outcome("ignored", state)
    }
    if (event.team === state.hovered) return outcome("ignored", state)
    return outcome("applied", Object.freeze({ ...state, hovered: event.team }))
  }
  if (!available(state.server, event.team)) return outcome("ignored", state)
  if (event.kind === "focus") {
    if (event.team === state.focused) return outcome("ignored", state)
    return outcome("applied", Object.freeze({ ...state, focused: event.team }))
  }
  const sameTeam = event.team === "red" && state.server.localTeam === 2
    || event.team === "blue" && state.server.localTeam === 3
    || event.team === "spectate" && state.server.localTeam === 1
  const next = Object.freeze({ ...state, visible: false, hovered: null })
  if (sameTeam) return outcome("applied", next)
  return outcome("applied", next, Object.freeze({
    kind: "join-team",
    team: event.team,
    sourceCommand: `jointeam ${event.team}`,
  }))
}
