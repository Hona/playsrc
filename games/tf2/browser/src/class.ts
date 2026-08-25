import type { Tf2Class } from "./codec"

export type Tf2ClassName = "scout" | "sniper" | "soldier" | "demoman" | "medic" | "heavy" | "pyro" | "spy" | "engineer"

export type Tf2ClassPresentation = Readonly<{
  identity: Tf2Class
  name: Tf2ClassName
  displayName: "Scout" | "Sniper" | "Soldier" | "Demoman" | "Medic" | "Heavy" | "Pyro" | "Spy" | "Engineer"
  model: string
  hands: string
}>

const CLASS_PRESENTATIONS: Readonly<Record<Tf2Class, Tf2ClassPresentation>> = Object.freeze({
  1: Object.freeze({ identity: 1, name: "scout", displayName: "Scout", model: "models/player/scout.mdl", hands: "models/weapons/c_models/c_scout_arms.mdl" }),
  2: Object.freeze({ identity: 2, name: "sniper", displayName: "Sniper", model: "models/player/sniper.mdl", hands: "models/weapons/c_models/c_sniper_arms.mdl" }),
  3: Object.freeze({ identity: 3, name: "soldier", displayName: "Soldier", model: "models/player/soldier.mdl", hands: "models/weapons/c_models/c_soldier_arms.mdl" }),
  4: Object.freeze({ identity: 4, name: "demoman", displayName: "Demoman", model: "models/player/demo.mdl", hands: "models/weapons/c_models/c_demo_arms.mdl" }),
  5: Object.freeze({ identity: 5, name: "medic", displayName: "Medic", model: "models/player/medic.mdl", hands: "models/weapons/c_models/c_medic_arms.mdl" }),
  6: Object.freeze({ identity: 6, name: "heavy", displayName: "Heavy", model: "models/player/heavy.mdl", hands: "models/weapons/c_models/c_heavy_arms.mdl" }),
  7: Object.freeze({ identity: 7, name: "pyro", displayName: "Pyro", model: "models/player/pyro.mdl", hands: "models/weapons/c_models/c_pyro_arms.mdl" }),
  8: Object.freeze({ identity: 8, name: "spy", displayName: "Spy", model: "models/player/spy.mdl", hands: "models/weapons/c_models/c_spy_arms.mdl" }),
  9: Object.freeze({ identity: 9, name: "engineer", displayName: "Engineer", model: "models/player/engineer.mdl", hands: "models/weapons/c_models/c_engineer_arms.mdl" }),
})

export const TF2_CLASS_NAMES: readonly Tf2ClassName[] = Object.freeze(
  Object.values(CLASS_PRESENTATIONS).map((presentation) => presentation.name),
)

export function tf2ClassPresentation(identity: Tf2Class): Tf2ClassPresentation {
  return CLASS_PRESENTATIONS[identity]
}

export function tf2ClassFromName(name: string): Tf2Class | undefined {
  return Object.values(CLASS_PRESENTATIONS).find((presentation) => presentation.name === name)?.identity
}
