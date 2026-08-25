import { tf2ClassPresentation } from "../class"
import type { Tf2Class } from "../codec"

export type Tf2ClassIdentity = Tf2Class
export type Tf2ClassSelectionIdentity = Tf2ClassIdentity | 12
export type Tf2ClassSelectionTeam = 2 | 3

export type Tf2ClassSelectionClass = Readonly<{
  identity: Tf2ClassSelectionIdentity
  menuIndex: number
  name: string
  shortName: string
  model: string
}>

const menuClass = (identity: Tf2Class, menuIndex: number): Tf2ClassSelectionClass => {
  const playerClass = tf2ClassPresentation(identity)
  return Object.freeze({
    identity,
    menuIndex,
    name: identity === 6 ? "heavyweapons" : playerClass.name,
    shortName: identity === 4 ? "demo" : playerClass.name,
    model: playerClass.model,
  })
}

export const TF2_CLASS_SELECTION_CLASSES: readonly Tf2ClassSelectionClass[] = Object.freeze([
  menuClass(1, 1), menuClass(3, 2), menuClass(7, 3), menuClass(4, 4), menuClass(6, 5),
  menuClass(9, 6), menuClass(5, 7), menuClass(2, 8), menuClass(8, 9),
  Object.freeze({ identity: 12, menuIndex: 12, name: "random", shortName: "random", model: "models/class_menu/random_class_icon.mdl" }),
])

const classesByIdentity = new Map(TF2_CLASS_SELECTION_CLASSES.map((value) => [value.identity, value]))
const classesByMenuIndex = new Map(TF2_CLASS_SELECTION_CLASSES.map((value) => [value.menuIndex, value]))
const classesByName = new Map(TF2_CLASS_SELECTION_CLASSES.map((value) => [value.name, value]))

export function tf2ClassSelectionClass(identity: number): Tf2ClassSelectionClass | null {
  return classesByIdentity.get(identity as Tf2ClassSelectionIdentity) ?? null
}

export function tf2ClassSelectionByMenuIndex(index: number): Tf2ClassSelectionClass | null {
  return classesByMenuIndex.get(index) ?? null
}

export function tf2ClassSelectionByName(name: string): Tf2ClassSelectionClass | null {
  return classesByName.get(name.toLowerCase()) ?? null
}

export function tf2ClassSelectionImage(identity: Tf2ClassSelectionIdentity, team: Tf2ClassSelectionTeam, selected: boolean): string {
  const playerClass = tf2ClassSelectionClass(identity)
  if (!playerClass || (team !== 2 && team !== 3)) throw new TypeError("TF2 class selection identity is invalid")
  return `class_sel_sm_${playerClass.shortName}_${selected ? team === 3 ? "blu" : "red" : "inactive"}`
}

export type Tf2ClassSelectionState = Readonly<{
  visible: boolean
  team: Tf2ClassSelectionTeam | null
  selected: Tf2ClassSelectionIdentity
  current: Tf2ClassIdentity | null
  initialJoin: boolean
}>

export type Tf2ClassSelectionEvent =
  | Readonly<{ kind: "show"; team: Tf2ClassSelectionTeam; current: Tf2ClassIdentity | null }>
  | Readonly<{ kind: "hover"; identity: Tf2ClassSelectionIdentity }>
  | Readonly<{ kind: "select"; identity: Tf2ClassSelectionIdentity }>
  | Readonly<{ kind: "confirm" }>
  | Readonly<{ kind: "cancel" }>
  | Readonly<{ kind: "hide" }>
  | Readonly<{ kind: "team-changed"; team: Tf2ClassSelectionTeam }>

export type Tf2ClassSelectionRequest = Readonly<{
  kind: "join-class"
  identity: Tf2ClassSelectionIdentity
  sourceCommand: string
}>

export type Tf2ClassSelectionTransition = Readonly<{
  disposition: "applied" | "ignored" | "illegal"
  state: Tf2ClassSelectionState
  request: Tf2ClassSelectionRequest | null
}>

export const TF2_CLASS_SELECTION_INITIAL_STATE: Tf2ClassSelectionState = Object.freeze({
  visible: false,
  team: null,
  selected: 6,
  current: null,
  initialJoin: false,
})

const result = (
  disposition: Tf2ClassSelectionTransition["disposition"],
  state: Tf2ClassSelectionState,
  request: Tf2ClassSelectionRequest | null = null,
): Tf2ClassSelectionTransition => Object.freeze({ disposition, state, request })

export function transitionTf2ClassSelection(state: Tf2ClassSelectionState, event: Tf2ClassSelectionEvent): Tf2ClassSelectionTransition {
  if (event.kind === "show") {
    if ((event.team !== 2 && event.team !== 3) || (event.current !== null && !tf2ClassSelectionClass(event.current))) {
      return result("illegal", state)
    }
    return result("applied", Object.freeze({
      visible: true,
      team: event.team,
      selected: event.current ?? 6,
      current: event.current,
      initialJoin: event.current === null,
    }))
  }
  if (event.kind === "hide") {
    if (!state.visible) return result("ignored", state)
    return result("applied", Object.freeze({ ...state, visible: false }))
  }
  if (!state.visible) return result("ignored", state)
  if (event.kind === "cancel") {
    if (state.initialJoin || state.current === null) return result("ignored", state)
    return result("applied", Object.freeze({ ...state, visible: false }))
  }
  if (event.kind === "team-changed") {
    if (event.team !== 2 && event.team !== 3) return result("illegal", state)
    if (event.team === state.team) return result("ignored", state)
    return result("applied", Object.freeze({ ...state, team: event.team }))
  }
  const identity = event.kind === "confirm" ? state.selected : event.identity
  const playerClass = tf2ClassSelectionClass(identity)
  if (!playerClass) return result("illegal", state)
  if (event.kind === "hover") {
    if (identity === state.selected) return result("ignored", state)
    return result("applied", Object.freeze({ ...state, selected: identity }))
  }
  const next = Object.freeze({ ...state, selected: identity, visible: false, initialJoin: false })
  return result("applied", next, Object.freeze({ kind: "join-class", identity, sourceCommand: `joinclass ${playerClass.name}` }))
}
