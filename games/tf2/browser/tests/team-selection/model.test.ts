import { describe, expect, test } from "bun:test"
import {
  TF2_TEAM_SELECTION_INITIAL_STATE,
  decodeTf2TeamSelectionServerState,
  transitionTf2TeamSelection,
  type Tf2TeamSelectionServerState,
} from "../../src/team-selection"

const server = (overrides: Partial<Tf2TeamSelectionServerState> = {}): Tf2TeamSelectionServerState => Object.freeze({
  localTeam: 0,
  redCount: 0,
  blueCount: 0,
  redDisabled: false,
  blueDisabled: false,
  spectatorsVisible: true,
  autoAssignVisible: true,
  cancelVisible: false,
  highlander: false,
  teamsFull: false,
  teamsFullArrow: false,
  ...overrides,
})

describe("Source TF2 team-selection state", () => {
  test("decodes compact authoritative roster and rule facts without inventing players", () => {
    expect(decodeTf2TeamSelectionServerState(0, 0, 0, 0b00001100)).toEqual(server())
    expect(decodeTf2TeamSelectionServerState(2, 1, 0, 0b00011100)).toMatchObject({ localTeam: 2, redCount: 1, blueCount: 0, cancelVisible: true })
    expect(() => decodeTf2TeamSelectionServerState(0, 1, 64, 0b00001100)).toThrow("inconsistent")
    expect(() => decodeTf2TeamSelectionServerState(4, 0, 0, 0)).toThrow("malformed")
  })

  test("starts unassigned with authoritative empty team counts and auto-assign focus", () => {
    const shown = transitionTf2TeamSelection(TF2_TEAM_SELECTION_INITIAL_STATE, { kind: "show", server: server() })
    expect(shown.state).toMatchObject({ visible: true, focused: "auto", server: { localTeam: 0, redCount: 0, blueCount: 0, cancelVisible: false } })
    expect(transitionTf2TeamSelection(shown.state, { kind: "cancel" })).toMatchObject({ disposition: "ignored", state: shown.state })
  })

  test("sends exact authored RED, BLU, auto, and spectator commands", () => {
    for (const team of ["red", "blue", "auto", "spectate"] as const) {
      const shown = transitionTf2TeamSelection(TF2_TEAM_SELECTION_INITIAL_STATE, { kind: "show", server: server() })
      const selected = transitionTf2TeamSelection(shown.state, { kind: "select", team })
      expect(selected.state.visible).toBeFalse()
      expect(selected.request).toEqual({ kind: "join-team", team, sourceCommand: `jointeam ${team}` })
    }
  })

  test("preserves Source same-team suppression and current-team focus", () => {
    for (const [localTeam, selected] of [[2, "red"], [3, "blue"], [1, "spectate"]] as const) {
      const shown = transitionTf2TeamSelection(TF2_TEAM_SELECTION_INITIAL_STATE, {
        kind: "show", server: server({ localTeam, cancelVisible: true, redCount: localTeam === 2 ? 1 : 0, blueCount: localTeam === 3 ? 1 : 0 }),
      })
      if (localTeam !== 1) expect(shown.state.focused).toBe(selected)
      expect(transitionTf2TeamSelection(shown.state, { kind: "select", team: selected })).toMatchObject({ request: null, state: { visible: false } })
    }
  })

  test("blocks disabled or unavailable authored choices and follows live server updates", () => {
    const shown = transitionTf2TeamSelection(TF2_TEAM_SELECTION_INITIAL_STATE, {
      kind: "show", server: server({ redCount: 1, redDisabled: true, spectatorsVisible: false }),
    })
    expect(transitionTf2TeamSelection(shown.state, { kind: "select", team: "red" }).disposition).toBe("ignored")
    expect(transitionTf2TeamSelection(shown.state, { kind: "select", team: "spectate" }).disposition).toBe("ignored")
    const updated = transitionTf2TeamSelection(shown.state, {
      kind: "update", server: server({ redCount: 1, blueCount: 1 }),
    })
    expect(updated.state.server?.redDisabled).toBeFalse()
    expect(transitionTf2TeamSelection(updated.state, { kind: "select", team: "red" }).request?.sourceCommand).toBe("jointeam red")
  })

  test("rejects invented roster facts and inconsistent cancellation/full-team visibility", () => {
    for (const invalid of [
      server({ redCount: -1 }),
      server({ redCount: 33, blueCount: 33 }),
      server({ cancelVisible: true }),
      server({ teamsFull: true }),
      server({ highlander: true, teamsFull: true, teamsFullArrow: true, spectatorsVisible: false }),
    ]) {
      expect(transitionTf2TeamSelection(TF2_TEAM_SELECTION_INITIAL_STATE, { kind: "show", server: invalid }).disposition).toBe("illegal")
    }
  })
})
