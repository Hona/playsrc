import { profileSampleSeconds } from "./profile-window"

/** Effective choices, not an environment dump. The capture consumes this same
 * plan that is durably retained before browser/map admission. */
function baseCapturePlan(environment: Readonly<NodeJS.ProcessEnv>) {
  const sustainedKoth = environment.PROFILE_KOTH_SUSTAINED === "1"
  if (sustainedKoth && !["koth_sawmill", "koth_lakeside_final"].includes(environment.PROFILE_MAP_TARGET ?? "")) throw new Error("Sustained KOTH requires an explicit admitted target")
  const createServer = sustainedKoth || environment.PROFILE_STARTUP_CREATE_SERVER === "1"
  const exerciseClasses = environment.PROFILE_UPWARD_CLASS_SWITCH === "1"
  const acceptance = environment.PROFILE_INTEGRATED_ACCEPTANCE === "1"
  const stockOnly = environment.PROFILE_ACCEPTANCE_STOCK_ONLY === "1"
  return Object.freeze({
    schema: "playsrc-upward-capture-plan-v1" as const,
    target: sustainedKoth ? environment.PROFILE_MAP_TARGET as "koth_sawmill" | "koth_lakeside_final" : createServer ? "ctf_2fort" as const : "pl_upward" as const,
    ...(sustainedKoth ? { sustainedSeconds: 90 as const } : {}),
    entry: createServer ? "create-server" as const : "training" as const,
    exerciseClasses, acceptance, stockOnly,
    combat: !stockOnly && environment.PROFILE_PARTICLE_COMBAT === "1",
    warmReload: environment.PROFILE_UPWARD_TRAINING_WARM_RELOAD === "1",
    playersOverride: createServer ? null : environment.PROFILE_UPWARD_TRAINING_PLAYERS || null,
    coldTeam: environment.PROFILE_ACCEPTANCE_STOCK_TEAM === "blue" ? "blue" as const : "red" as const,
    warmTeam: environment.PROFILE_ACCEPTANCE_STOCK_TEAM === "blue" || acceptance ? "blue" as const : "red" as const,
    sampleSeconds: stockOnly ? null : profileSampleSeconds(environment.PROFILE_SAMPLE_SECONDS),
    classPasses: stockOnly || !exerciseClasses ? 0 : acceptance ? 1 : 2,
    interaction: stockOnly ? "stock-loadouts" : exerciseClasses ? "class-input"
      : environment.PROFILE_UPWARD_TRAINING_INTERACTION === "1" ? "movement-weapon" : "forward-movement",
    workerCpu: !stockOnly && (exerciseClasses || acceptance) ? "required" : "not-requested",
    ...(!stockOnly && environment.PROFILE_GAMEPLAY_REPLAY === "1" ? { gameplayReplay: "required" as const } : {}),
    ...(!stockOnly && environment.PROFILE_RENDER_OWNERS === "1" ? { renderOwners: "two-frames-after-60-v1" as const } : {}),
    ...(environment.PROFILE_AUTHOR_WORKLOAD === "1" ? { workloadAuthor: 2 as const } : {}),
    ...(environment.PROFILE_COMMAND_WORKLOAD ? { commandWorkload: environment.PROFILE_COMMAND_WORKLOAD } : {}),
    ...(environment.PROFILE_AUTHOR_WORKLOAD === "1" || environment.PROFILE_COMMAND_WORKLOAD ? { workloadPhase: "equipped-primary-draw-after-secondary-idle-v1" as const } : {}),
    ...(environment.PROFILE_AUTHOR_WORKLOAD === "1" || environment.PROFILE_COMMAND_WORKLOAD ? { clientFrameWorkload: "accepted-real-clock-v1" as const } : {}),
    ...(environment.PROFILE_AUTHOR_WORKLOAD === "1" || environment.PROFILE_COMMAND_WORKLOAD ? { presentationWorkload: "recorded-publication-groups-v1" as const } : {}),
    ...(environment.PROFILE_AUTHOR_WORKLOAD === "1" || environment.PROFILE_COMMAND_WORKLOAD ? { presentationEntropy: "recorded-map-seeds-v1" as const } : {}),
  } as const)
}

export function upwardCapturePlan(environment: Readonly<NodeJS.ProcessEnv>) {
  const base = baseCapturePlan(environment)
  if (base.commandWorkload && !/^[0-9a-f]{64}$/.test(base.commandWorkload)) throw new Error("Invalid command workload identity")
  if ((base.workloadAuthor || base.commandWorkload) && (!base.warmReload || base.exerciseClasses || base.combat || base.stockOnly
    || environment.PROFILE_CLASS_REPLACEMENT === "1" || base.workloadAuthor && base.commandWorkload)) throw new Error("Incompatible command workload capture plan")
  return Object.freeze({ ...base, schema: "playsrc-upward-capture-plan-v2" as const,
    replacement: !base.stockOnly && environment.PROFILE_CLASS_REPLACEMENT === "1" })
}

export type UpwardCapturePlan = ReturnType<typeof upwardCapturePlan> | ReturnType<typeof baseCapturePlan>

export function validateUpwardCapturePlan(value: any): asserts value is UpwardCapturePlan {
  if (!value || !["playsrc-upward-capture-plan-v1", "playsrc-upward-capture-plan-v2"].includes(value.schema)
     || !["pl_upward", "ctf_2fort", "koth_sawmill", "koth_lakeside_final"].includes(value.target) || !["training", "create-server"].includes(value.entry)
    || ["exerciseClasses", "acceptance", "stockOnly", "combat", "warmReload"].some(key => typeof value[key] !== "boolean")
    || !(value.playersOverride === null || typeof value.playersOverride === "string")
    || !["red", "blue"].includes(value.coldTeam) || !["red", "blue"].includes(value.warmTeam)
    || ![0, 1, 2].includes(value.classPasses)
    || !["stock-loadouts", "class-input", "movement-weapon", "forward-movement"].includes(value.interaction)
    || !["required", "not-requested"].includes(value.workerCpu)) throw new Error("Invalid effective capture plan")
  // Reuse the exact resolver to reject internally contradictory plans.
  if (value.schema === "playsrc-upward-capture-plan-v2" && typeof value.replacement !== "boolean") throw new Error("Invalid replacement capture plan")
  const resolve = value.schema === "playsrc-upward-capture-plan-v1" ? baseCapturePlan : upwardCapturePlan
  const resolved = resolve({
    PROFILE_KOTH_SUSTAINED: value.sustainedSeconds === 90 ? "1" : "0",
    PROFILE_MAP_TARGET: value.target,
    PROFILE_STARTUP_CREATE_SERVER: value.entry === "create-server" ? "1" : "0",
    PROFILE_UPWARD_CLASS_SWITCH: value.exerciseClasses ? "1" : "0",
    PROFILE_INTEGRATED_ACCEPTANCE: value.acceptance ? "1" : "0",
    PROFILE_ACCEPTANCE_STOCK_ONLY: value.stockOnly ? "1" : "0",
    PROFILE_PARTICLE_COMBAT: value.combat ? "1" : "0",
    PROFILE_UPWARD_TRAINING_WARM_RELOAD: value.warmReload ? "1" : "0",
    PROFILE_UPWARD_TRAINING_PLAYERS: value.playersOverride ?? undefined,
    PROFILE_ACCEPTANCE_STOCK_TEAM: value.coldTeam,
    PROFILE_SAMPLE_SECONDS: String(value.sampleSeconds),
    PROFILE_UPWARD_TRAINING_INTERACTION: value.interaction === "movement-weapon" ? "1" : "0",
    PROFILE_RENDER_OWNERS: value.renderOwners ? "1" : "0",
    PROFILE_GAMEPLAY_REPLAY: value.gameplayReplay ? "1" : "0",
    PROFILE_CLASS_REPLACEMENT: value.replacement ? "1" : "0",
    PROFILE_AUTHOR_WORKLOAD: value.workloadAuthor === 2 ? "1" : "0",
    PROFILE_COMMAND_WORKLOAD: value.commandWorkload,
  })
  if (Object.keys(value).length !== Object.keys(resolved).length
    || Object.entries(resolved).some(([key, expected]) => value[key] !== expected)) throw new Error("Inconsistent effective capture plan")
}

export function assertMatchingCapturePlans(before: unknown, after: unknown) {
  if (before == null || after == null) throw new Error("Historical capture plan unknown; comparison not admitted")
  validateUpwardCapturePlan(before)
  validateUpwardCapturePlan(after)
  if (Object.keys(before).length !== Object.keys(after).length
    || Object.keys(before).some(key => before[key as keyof UpwardCapturePlan] !== after[key as keyof UpwardCapturePlan])) throw new Error("Effective capture comparison plans differ")
}
