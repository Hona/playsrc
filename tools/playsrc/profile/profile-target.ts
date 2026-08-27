export type HeadedProfileTarget = "jump_beef" | "pl_upward" | "ctf_2fort" | "koth_viaduct"

export function headedProfileTarget(environment: NodeJS.ProcessEnv = process.env, fallback: HeadedProfileTarget = "jump_beef"): HeadedProfileTarget {
  if (environment.PROFILE_SCENARIOS === "koth") return "koth_viaduct"
  if (environment.PROFILE_UPGRADE_ROSTER === "23") return "ctf_2fort"
  if (environment.PROFILE_UPGRADE_ROSTER === "15") return "pl_upward"
  if (environment.PROFILE_CTF_OBJECTIVES === "1" || environment.PROFILE_CTF_BOTS === "1"
    || environment.PROFILE_2FORT_VISUAL === "1" || environment.PROFILE_2FORT_MEMORY === "1"
    || environment.PROFILE_COMBAT_IMPACTS === "1"
    || ["local-practice", "2fort-match"].includes(environment.PROFILE_SCENARIOS ?? "")) {
    return "ctf_2fort"
  }
  if (environment.PROFILE_COMBAT === "1" || environment.PROFILE_UPWARD_OUTDOORS === "1" || environment.PROFILE_SKY_COHERENCE === "1" || environment.PROFILE_ROUND_RULES === "1"
    || environment.PROFILE_PICKUPS === "1" || environment.PROFILE_MATERIAL_ANIMATION === "1" || environment.PROFILE_TRACKTRAIN === "1"
    || environment.PROFILE_MEDIC_WEAPONS === "1" || ["demoman", "scoreboard", "upward-floor", "upward-training-bots"].includes(environment.PROFILE_SCENARIOS ?? "")) {
    return "pl_upward"
  }
  return fallback
}
