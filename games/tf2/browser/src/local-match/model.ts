import type { BotConfiguration, BotDifficulty, BotQuotaMode } from "../codec"
import type { Tf2UiPanelDocument, Tf2UiResourceNode } from "../ui-resources"

export const TF2_BOT_DIFFICULTIES = Object.freeze(["Easy", "Normal", "Hard", "Expert"] as const)
export const TF2_BOT_QUOTA_MODES = Object.freeze(["normal", "fill", "match"] as const)
export const TF2_OFFLINE_PRACTICE_STORAGE_KEY = "playsrc.tf2.OfflinePracticeConfig.v1"

export type Tf2LocalMatchMap = Readonly<{
  identity: string
  displayName: string
  mode: "payload" | "king-of-the-hill" | "capture-the-flag" | "custom"
  minimumPlayers: number
  maximumPlayers: number
}>

export type Tf2OfflinePracticeDefaults = Readonly<{
  difficulty: BotDifficulty
  minimumPlayers: number
  maximumPlayers: number
  suggestedPlayers: number
  map: string
}>

export type Tf2OfflinePracticeCatalog = Readonly<{
  defaults: Tf2OfflinePracticeDefaults
  maps: readonly Tf2LocalMatchMap[]
}>

export type Tf2LocalMatchSettings = Readonly<{
  mapIdentity: string
  difficulty: BotDifficulty
  playerCount: number
  quotaMode: BotQuotaMode
}>

export type Tf2LocalMatchLaunch = Readonly<{
  entry: "training" | "create-server"
  mapIdentity: string
  configuration: BotConfiguration
}>

const scalar = (node: Tf2UiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null

const object = (node: Tf2UiResourceNode, name: string): Tf2UiResourceNode | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value === null) ?? null

function integer(node: Tf2UiResourceNode, name: string, minimum: number, maximum: number): number {
  const value = Number(scalar(node, name))
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new Error(`TF2 offline practice configured ${node.name}:${name} is invalid`)
  }
  return value
}

export function createTf2OfflinePracticeCatalog(
  source: Tf2UiPanelDocument,
  configuredMaps: readonly string[],
): Tf2OfflinePracticeCatalog {
  if (source.source.logicalPath !== "resource/offline_practice.res" || source.roots.length !== 1) {
    throw new Error("TF2 offline practice configured map source is invalid")
  }
  const root = source.roots[0]!
  const defaults = object(root, "defaults")
  const maps = object(root, "maps")
  if (!defaults || !maps) throw new Error("TF2 offline practice configured sections are unavailable")
  const difficultyText = scalar(defaults, "difficulty")?.toLowerCase()
  const difficulty = TF2_BOT_DIFFICULTIES.findIndex((value) => value.toLowerCase() === difficultyText)
  if (difficulty < 0) throw new Error("TF2 offline practice configured difficulty is invalid")
  const declared = new Set(configuredMaps)
  const selected = maps.children.flatMap((node): Tf2LocalMatchMap[] => {
    if (node.value !== null || !declared.has(node.name)) return []
    const mode = node.name.startsWith("pl_") ? "payload" : node.name.startsWith("koth_") ? "king-of-the-hill" : null
    if (!mode) return []
    const displayName = scalar(node, "name")
    if (!displayName) throw new Error(`TF2 offline practice configured map name is invalid: ${node.name}`)
    const minimumPlayers = integer(node, "min_players", 1, 32)
    const maximumPlayers = integer(node, "max_players", minimumPlayers, 32)
    return [Object.freeze({ identity: node.name, displayName, mode, minimumPlayers, maximumPlayers })]
  })
  return Object.freeze({
    defaults: Object.freeze({
      difficulty: difficulty as BotDifficulty,
      minimumPlayers: integer(defaults, "min_players", 1, 32),
      maximumPlayers: integer(defaults, "max_players", 1, 32),
      suggestedPlayers: integer(defaults, "suggested_players", 1, 31),
      map: scalar(defaults, "map") ?? "",
    }),
    maps: Object.freeze(selected),
  })
}

export function createTf2LocalMatchMaps(
  configuredMaps: readonly string[],
  practice: Tf2OfflinePracticeCatalog,
): readonly Tf2LocalMatchMap[] {
  return Object.freeze(configuredMaps.flatMap((identity): Tf2LocalMatchMap[] => {
    const authored = practice.maps.find((map) => map.identity === identity)
    if (authored) return [authored]
    if (identity === "ctf_2fort") {
      return [Object.freeze({ identity, displayName: "2FORT", mode: "capture-the-flag", minimumPlayers: 1, maximumPlayers: 24 })]
    }
    return [Object.freeze({
      identity,
      displayName: identity,
      mode: "custom",
      minimumPlayers: practice.defaults.minimumPlayers,
      maximumPlayers: practice.defaults.maximumPlayers,
    })]
  }))
}

export function tf2LocalMatchLaunch(
  entry: Tf2LocalMatchLaunch["entry"],
  settings: Tf2LocalMatchSettings,
  map: Tf2LocalMatchMap,
): Tf2LocalMatchLaunch {
  if (settings.mapIdentity !== map.identity || !Number.isSafeInteger(settings.playerCount)) {
    throw new Error("TF2 local match map or player count is invalid")
  }
  if (!Number.isSafeInteger(settings.difficulty) || settings.difficulty < 0 || settings.difficulty > 3
    || !TF2_BOT_QUOTA_MODES.includes(settings.quotaMode)) {
    throw new Error("TF2 local match bot settings are invalid")
  }
   if (entry === "training" && map.mode !== "payload" && map.mode !== "king-of-the-hill") {
    throw new Error("TF2 offline practice excludes maps absent from its configured mode catalog")
  }
  const count = Math.max(1, Math.min(31, settings.playerCount))
  const configuration: BotConfiguration = entry === "training"
    ? Object.freeze({
        quota: count - 1,
        maximumPlayers: map.maximumPlayers,
        mode: "normal",
        difficulty: settings.difficulty,
        joinAfterPlayer: true,
        autoVacate: false,
        offlinePractice: true,
      })
    : Object.freeze({
        quota: Math.max(0, Math.min(31, settings.playerCount)),
        maximumPlayers: map.maximumPlayers,
        mode: settings.quotaMode,
        difficulty: settings.difficulty,
        joinAfterPlayer: true,
        autoVacate: false,
        offlinePractice: false,
      })
  return Object.freeze({ entry, mapIdentity: map.identity, configuration })
}
