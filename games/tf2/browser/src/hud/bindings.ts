import type {
  Tf2Class,
  Tf2ConditionWords,
  Tf2HudAction,
  Tf2HudAnimation,
  Tf2HudAvailability,
  Tf2HudBinding,
  Tf2HudCommand,
  Tf2HudCrosshair,
  Tf2HudEvent,
  Tf2HudFreezePanel,
  Tf2HudHealth,
  Tf2HudKillfeedNotice,
  Tf2HudClassModel,
  Tf2HudLocalizedValue,
  Tf2HudPanelValue,
  Tf2HudPickupNotification,
  Tf2HudPlayer,
  Tf2HudPresentationCommand,
  Tf2HudPublication,
  Tf2HudScoreboard,
  Tf2HudSnapshot,
  Tf2HudUnavailableReason,
  Tf2HudWeapon,
  Tf2PlayableTeam,
  Tf2ReloadPhase,
  Tf2ScoreboardCounters,
  Tf2ScoreboardPlayer,
  Tf2ScoreboardTeam,
  Tf2Team,
} from "./contract"
import { TF2_HUD_LIMITS, Tf2HudBindingError } from "./contract"
import { tf2CustomCrosshairFile } from "./crosshair"
import {
  TF2_CLASS_IMAGES,
  TF2_GROUPED_CONDITION_PANELS,
  TF2_INDEPENDENT_CONDITION_PANELS,
} from "./inventory"

const UINT32_MAX = 0xffff_ffff
const HEALTH_WARNING_FRACTION = 0.49
const HEALTH_BONUS_POSITION_ADJUSTMENT = 35
const LOW_AMMO_WARNING_FRACTION = 0.4
const LOW_AMMO_POSITION_ADJUSTMENT = 5

function sourceRoundToInt(value: number): number {
  const sourceFloat = Math.fround(value)
  const lower = Math.floor(sourceFloat)
  const fraction = sourceFloat - lower
  if (fraction < 0.5) return lower
  if (fraction > 0.5) return lower + 1
  return lower % 2 === 0 ? lower : lower + 1
}

export function tf2HudAvailable<T>(value: T): Tf2HudAvailability<T> {
  return Object.freeze({ kind: "available", value })
}

export function tf2HudUnavailable<T>(reason: Tf2HudUnavailableReason): Tf2HudAvailability<T> {
  return Object.freeze({ kind: "unavailable", reason })
}

function malformed(message: string): never {
  throw new Tf2HudBindingError("MalformedFacts", message)
}

function bound(message: string): never {
  throw new Tf2HudBindingError("BoundExceeded", message)
}

function canonicalIdentity(value: number, subject: string): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > UINT32_MAX) malformed(`${subject} identity is invalid`)
  return value
}

function integer(value: number, subject: string, minimum = 0): number {
  if (!Number.isSafeInteger(value) || value < minimum) malformed(`${subject} is invalid`)
  return value
}

function finite(value: number, subject: string, minimum = -Infinity): number {
  if (!Number.isFinite(value) || value < minimum) malformed(`${subject} is invalid`)
  return value
}

function text(value: string, subject: string, maximum: number = TF2_HUD_LIMITS.resourceIdentityCodeUnits): string {
  if (typeof value !== "string" || value.length > maximum || value.includes("\0")) malformed(`${subject} is invalid`)
  return value
}

function copyAvailability<T, U>(
  value: Tf2HudAvailability<T>,
  copy: (available: T) => U,
  subject: string,
): Tf2HudAvailability<U> {
  if (!value || typeof value !== "object") malformed(`${subject} availability is invalid`)
  if (value.kind === "unavailable") {
    if (!["initial", "not-produced", "not-applicable", "replay-discontinuity", "missing-source-fact"].includes(value.reason)) {
      malformed(`${subject} unavailable reason is invalid`)
    }
    return tf2HudUnavailable(value.reason)
  }
  if (value.kind !== "available") malformed(`${subject} availability is invalid`)
  return tf2HudAvailable(copy(value.value))
}

function copyIntegerAvailability(value: Tf2HudAvailability<number>, subject: string): Tf2HudAvailability<number> {
  return copyAvailability(value, (item) => integer(item, subject), subject)
}

function copyHealth(value: Tf2HudHealth): Tf2HudHealth {
  const current = integer(value.current, "health current", -0x8000_0000)
  const maximum = integer(value.maximum, "health maximum", 1)
  const maximumBuffed = integer(value.maximumBuffed, "health maximum buffed", 1)
  if (maximumBuffed < maximum || maximumBuffed < current) malformed("health maximum buffed is inconsistent")
  return Object.freeze({ current, maximum, maximumBuffed })
}

function copyConditions(value: Tf2ConditionWords): Tf2ConditionWords {
  if (!Array.isArray(value) || value.length !== TF2_HUD_LIMITS.conditionWords) malformed("condition words are invalid")
  return Object.freeze(value.map((word, index) => canonicalIdentity(word, `condition word ${index}`))) as Tf2ConditionWords
}

function copyReload(value: Tf2ReloadPhase): Tf2ReloadPhase {
  if (value !== "ready" && value !== "start" && value !== "insert" && value !== "finish") {
    malformed("reload phase is invalid")
  }
  return value
}

function copyWeapon(value: Tf2HudWeapon): Tf2HudWeapon {
  const identity = canonicalIdentity(value.identity, "weapon")
  const itemDefinition = copyIntegerAvailability(value.itemDefinition, "item definition")
  const displayName = text(value.displayName, "weapon display name", 255)
  const slot = integer(value.slot, "weapon slot")
  const position = integer(value.position, "weapon position")
  if (slot >= TF2_HUD_LIMITS.loadoutWeapons || position >= TF2_HUD_LIMITS.loadoutWeapons) {
    bound("weapon slot or position exceeds the loadout bound")
  }
  if (value.ammoDisplay !== "clip-and-reserve" && value.ammoDisplay !== "total" && value.ammoDisplay !== "hidden") {
    malformed("weapon ammo display is invalid")
  }
  const clip = copyIntegerAvailability(value.clip, "weapon clip")
  const reserve = copyIntegerAvailability(value.reserve, "weapon reserve")
  const maximumClip = copyIntegerAvailability(value.maximumClip, "weapon maximum clip")
  const maximumReserve = copyIntegerAvailability(value.maximumReserve, "weapon maximum reserve")
  if (clip.kind === "available" && maximumClip.kind === "available" && clip.value > maximumClip.value) {
    malformed("weapon clip exceeds its maximum")
  }
  if (reserve.kind === "available" && maximumReserve.kind === "available" && reserve.value > maximumReserve.value) {
    malformed("weapon reserve exceeds its maximum")
  }
  return Object.freeze({
    identity,
    itemDefinition,
    displayName,
    slot,
    position,
    selectable: value.selectable === true,
    ammoDisplay: value.ammoDisplay,
    clip,
    reserve,
    maximumClip,
    maximumReserve,
    reload: copyReload(value.reload),
    drawsCrosshair: value.drawsCrosshair === true,
  })
}

function copyClass(value: Tf2Class): Tf2Class {
  if (!Number.isInteger(value) || value < 1 || value > 9) malformed("TF2 class is invalid")
  return value
}

function copyTeam(value: Tf2Team): Tf2Team {
  if (!Number.isInteger(value) || value < 0 || value > 3) malformed("TF2 team is invalid")
  return value
}

function copyVector(value: readonly [number, number, number], subject: string): readonly [number, number, number] {
  if (!Array.isArray(value) || value.length !== 3) malformed(`${subject} is invalid`)
  const output = value.map((component) => finite(component, subject)) as [number, number, number]
  const length = Math.hypot(...output)
  if (Math.abs(length - 1) > 1e-4) malformed(`${subject} is not normalized`)
  return Object.freeze(output)
}

function copyColor(value: readonly [number, number, number, number]): readonly [number, number, number, number] {
  if (!Array.isArray(value) || value.length !== 4) malformed("crosshair color is invalid")
  const output = value.map((channel) => integer(channel, "crosshair color channel"))
  if (output.some((channel) => channel > 255)) malformed("crosshair color channel exceeds 255")
  return Object.freeze(output) as readonly [number, number, number, number]
}

function copyCrosshair(value: Tf2HudCrosshair): Tf2HudCrosshair {
  if (!["none", "in-eye", "roaming", "other"].includes(value.observerMode)) malformed("observer mode is invalid")
  return Object.freeze({
    configured: value.configured === true,
    weaponAllows: value.weaponAllows === true,
    loadingImage: value.loadingImage === true,
    paused: value.paused === true,
    clientModeAllows: value.clientModeAllows === true,
    frozen: value.frozen === true,
    localViewEntity: value.localViewEntity === true,
    vguiInput: value.vguiInput === true,
    observerMode: value.observerMode,
    observerCrosshair: value.observerCrosshair === true,
    tfSuppressed: value.tfSuppressed === true,
    countdownHidden: value.countdownHidden === true,
    texture: text(value.texture, "crosshair texture"),
    color: copyColor(value.color),
    scale: finite(value.scale, "crosshair scale", 0),
    weaponScale: finite(value.weaponScale, "weapon crosshair scale", 0),
  })
}

function copyPlayer(value: Tf2HudPlayer): Tf2HudPlayer {
  if (value.lifecycle !== "active" && value.lifecycle !== "dying" && value.lifecycle !== "observer") {
    malformed("player lifecycle is invalid")
  }
  if (!Array.isArray(value.weapons)) malformed("HUD weapon list is invalid")
  if (value.weapons.length > TF2_HUD_LIMITS.loadoutWeapons) bound("HUD weapon list exceeds its bound")
  const weapons = Object.freeze(value.weapons.map(copyWeapon))
  const identities = new Set(weapons.map((weapon) => weapon.identity))
  if (identities.size !== weapons.length) malformed("HUD weapon identities are duplicated")
  const activeWeapon = copyIntegerAvailability(value.activeWeapon, "active weapon")
  if (activeWeapon.kind === "available" && !identities.has(activeWeapon.value)) malformed("active weapon is not in the loadout")
  const selectedWeapon = copyIntegerAvailability(value.weaponSelection.selectedWeapon, "selected weapon")
  if (selectedWeapon.kind === "available" && !identities.has(selectedWeapon.value)) malformed("selected weapon is not in the loadout")
  return Object.freeze({
    identity: canonicalIdentity(value.identity, "player"),
    lifecycle: value.lifecycle,
    class: copyAvailability(value.class, copyClass, "player class"),
    team: copyAvailability(value.team, copyTeam, "player team"),
    playerClassUsePlayerModel: value.playerClassUsePlayerModel === true,
    classModel: copyAvailability(value.classModel, copyClassModel, "player class model"),
    health: copyAvailability(value.health, copyHealth, "player health"),
    conditions: copyConditions(value.conditions),
    weapons,
    activeWeapon,
    weaponSelection: Object.freeze({ open: value.weaponSelection.open === true, selectedWeapon }),
    crosshair: copyAvailability(value.crosshair, copyCrosshair, "player crosshair"),
    liveHudSuppressed: value.liveHudSuppressed === true,
    respawnAllowed: value.respawnAllowed === true,
  })
}

function copyClassModel(value: Tf2HudClassModel): Tf2HudClassModel {
  return Object.freeze({
    identity: text(value.identity, "player class model"),
    skin: integer(value.skin, "player class model skin"),
  })
}

function copyCounters(value: Tf2ScoreboardCounters): Tf2ScoreboardCounters {
  const output = Object.fromEntries(
    Object.entries(value).map(([key, count]) => [key, integer(count, `scoreboard ${key}`)]),
  ) as unknown as Tf2ScoreboardCounters
  return Object.freeze(output)
}

function copyScoreboardPlayer(value: Tf2ScoreboardPlayer): Tf2ScoreboardPlayer {
  if (!["connected", "connecting", "loading", "disconnected"].includes(value.connection)) {
    malformed("scoreboard connection state is invalid")
  }
  if (!["none", "dominating-local", "dominated-by-local"].includes(value.relationship)) {
    malformed("scoreboard relationship is invalid")
  }
  const ping = copyAvailability(value.ping, (item) => item === "bot" ? item : integer(item, "scoreboard ping"), "scoreboard ping")
  const activeDominations = integer(value.activeDominations, "scoreboard dominations")
  if (activeDominations >= TF2_HUD_LIMITS.scoreboardPlayers) bound("scoreboard dominations exceed the player bound")
  return Object.freeze({
    identity: canonicalIdentity(value.identity, "scoreboard player"),
    name: text(value.name, "scoreboard player name", TF2_HUD_LIMITS.playerNameCodeUnits),
    team: copyPlayableTeam(value.team),
    connection: value.connection,
    score: integer(value.score, "scoreboard score", -0x8000_0000),
    alive: value.alive === true,
    class: copyAvailability(value.class, copyClass, "scoreboard class"),
    ping,
    killstreak: integer(value.killstreak, "scoreboard killstreak"),
    activeDominations,
    relationship: value.relationship,
    counters: copyAvailability(value.counters, copyCounters, "scoreboard counters"),
  })
}

function copyPlayableTeam(value: Tf2PlayableTeam): Tf2PlayableTeam {
  if (value !== 2 && value !== 3) malformed("playable team is invalid")
  return value
}

function copyScoreboardTeam(value: Tf2ScoreboardTeam, expected: Tf2PlayableTeam): Tf2ScoreboardTeam {
  if (value.team !== expected) malformed("scoreboard team identity is inconsistent")
  const playerCount = integer(value.playerCount, "scoreboard player count")
  if (playerCount > TF2_HUD_LIMITS.scoreboardPlayers) bound("scoreboard player count exceeds its bound")
  return Object.freeze({
    team: expected,
    localizedName: text(value.localizedName, "scoreboard team name", 255),
    score: integer(value.score, "scoreboard team score", -0x8000_0000),
    playerCount,
  })
}

function copyScoreboard(value: Tf2HudScoreboard): Tf2HudScoreboard {
  if (!Array.isArray(value.players) || value.players.length > TF2_HUD_LIMITS.scoreboardPlayers) {
    bound("scoreboard player list exceeds its bound")
  }
  const players = Object.freeze(value.players.map(copyScoreboardPlayer))
  if (new Set(players.map((player) => player.identity)).size !== players.length) malformed("scoreboard player identities are duplicated")
  const names = (items: readonly string[], subject: string) => {
    if (!Array.isArray(items) || items.length > TF2_HUD_LIMITS.scoreboardPlayers) bound(`${subject} exceeds its bound`)
    return Object.freeze(items.map((item) => text(item, subject, TF2_HUD_LIMITS.playerNameCodeUnits)))
  }
  const selectedPlayer = copyIntegerAvailability(value.selectedPlayer, "scoreboard selected player")
  if (selectedPlayer.kind === "available" && !players.some((player) => player.identity === selectedPlayer.value)) {
    malformed("scoreboard selected player is absent")
  }
  return Object.freeze({
    visible: value.visible === true,
    red: copyScoreboardTeam(value.red, 2),
    blue: copyScoreboardTeam(value.blue, 3),
    players,
    spectators: names(value.spectators, "scoreboard spectator name"),
    waitingToPlay: names(value.waitingToPlay, "scoreboard waiting-player name"),
    selectedPlayer,
  })
}

function copyFreezePanel(value: Tf2HudFreezePanel): Tf2HudFreezePanel {
  return Object.freeze({
    killerIdentity: copyIntegerAvailability(value.killerIdentity, "freeze-panel killer"),
    killerName: copyAvailability(value.killerName, (item) => text(item, "freeze-panel killer name", TF2_HUD_LIMITS.playerNameCodeUnits), "freeze-panel killer name"),
    killerHealth: copyAvailability(value.killerHealth, copyHealth, "freeze-panel killer health"),
    nemesisName: copyAvailability(value.nemesisName, (item) => text(item, "freeze-panel nemesis name", TF2_HUD_LIMITS.playerNameCodeUnits), "freeze-panel nemesis name"),
  })
}

function copySnapshot(value: Tf2HudSnapshot): Tf2HudSnapshot {
  if (typeof value.tick !== "bigint" || value.tick < 0n) malformed("HUD snapshot tick is invalid")
  return Object.freeze({
    tick: value.tick,
    player: copyAvailability(value.player, copyPlayer, "HUD player"),
    scoreboard: copyAvailability(value.scoreboard, copyScoreboard, "HUD scoreboard"),
    freezePanel: copyAvailability(value.freezePanel, copyFreezePanel, "HUD freeze panel"),
  })
}

function copyParticipant(value: Tf2HudKillfeedNotice["killer"], subject: string) {
  return Object.freeze({
    identity: copyIntegerAvailability(value.identity, `${subject} identity`),
    name: text(value.name, `${subject} name`, TF2_HUD_LIMITS.playerNameCodeUnits),
    team: copyTeam(value.team),
  })
}

function copyKillfeed(value: Tf2HudKillfeedNotice): Tf2HudKillfeedNotice {
  return Object.freeze({
    killer: copyParticipant(value.killer, "killfeed killer"),
    victim: copyParticipant(value.victim, "killfeed victim"),
    assister: copyAvailability(value.assister, (item) => copyParticipant(item, "killfeed assister"), "killfeed assister"),
    weaponIcon: copyAvailability(value.weaponIcon, (item) => text(item, "killfeed weapon icon", 31), "killfeed weapon icon"),
    weaponIdentity: copyIntegerAvailability(value.weaponIdentity, "killfeed weapon"),
    customKill: integer(value.customKill, "killfeed custom kill"),
    critical: value.critical === true,
    selfInflicted: value.selfInflicted === true,
    localPlayerInvolved: value.localPlayerInvolved === true,
    domination: value.domination === true,
    revenge: value.revenge === true,
    silent: value.silent === true,
  })
}

function copyPickup(value: Tf2HudPickupNotification): Tf2HudPickupNotification {
  if (!["health", "ammo", "weapon", "item"].includes(value.pickup)) malformed("pickup notification kind is invalid")
  return Object.freeze({
    pickupIdentity: canonicalIdentity(value.pickupIdentity, "pickup"),
    pickup: value.pickup,
    itemIdentity: copyAvailability(value.itemIdentity, (item) => typeof item === "number"
      ? canonicalIdentity(item, "pickup item")
      : text(item, "pickup item"), "pickup item"),
    amount: copyAvailability(value.amount, (item) => finite(item, "pickup amount", 0), "pickup amount"),
  })
}

function copyEvent(value: Tf2HudEvent): Tf2HudEvent {
  if (typeof value.tick !== "bigint" || value.tick < 0n) malformed("HUD event tick is invalid")
  const identity = { tick: value.tick, ordinal: integer(value.ordinal, "HUD event ordinal") }
  switch (value.kind) {
    case "health":
      if (!["damage", "heal", "pickup", "regenerate", "respawn", "state"].includes(value.cause)) malformed("health event cause is invalid")
      return Object.freeze({ ...identity, kind: value.kind, health: copyHealth(value.health), cause: value.cause })
    case "ammo":
      if (!["fire", "reload", "pickup", "regenerate", "respawn", "state"].includes(value.cause)) malformed("ammo event cause is invalid")
      return Object.freeze({ ...identity, kind: value.kind, weapon: canonicalIdentity(value.weapon, "ammo weapon"), clip: copyIntegerAvailability(value.clip, "ammo clip"), reserve: copyIntegerAvailability(value.reserve, "ammo reserve"), reload: copyReload(value.reload), cause: value.cause })
    case "weapon-selected": return Object.freeze({ ...identity, kind: value.kind, weapon: canonicalIdentity(value.weapon, "selected weapon") })
    case "class-changed": return Object.freeze({ ...identity, kind: value.kind, class: copyClass(value.class) })
    case "team-changed": return Object.freeze({ ...identity, kind: value.kind, team: copyTeam(value.team) })
    case "conditions": return Object.freeze({ ...identity, kind: value.kind, conditions: copyConditions(value.conditions) })
    case "damage": {
      const amount = integer(value.amount, "damage amount")
      if (amount > 0x7fff) malformed("damage amount exceeds the message range")
      return Object.freeze({ ...identity, kind: value.kind, amount, health: copyHealth(value.health), direction: copyAvailability(value.direction, (item) => copyVector(item, "damage direction"), "damage direction") })
    }
    case "lifecycle":
      if (!["active", "dying", "observer"].includes(value.lifecycle)) malformed("lifecycle event is invalid")
      return Object.freeze({ ...identity, kind: value.kind, lifecycle: value.lifecycle })
    case "regenerate": {
      if (!Array.isArray(value.weapons)) malformed("regenerate weapon list is invalid")
      if (value.weapons.length > TF2_HUD_LIMITS.loadoutWeapons) bound("regenerate weapon list exceeds its bound")
      const weapons = Object.freeze(value.weapons.map(copyWeapon))
      if (new Set(weapons.map((weapon) => weapon.identity)).size !== weapons.length) malformed("regenerate weapon identities are duplicated")
      return Object.freeze({ ...identity, kind: value.kind, zone: copyIntegerAvailability(value.zone, "regenerate zone"), health: copyHealth(value.health), weapons, conditions: copyConditions(value.conditions) })
    }
    case "pickup": return Object.freeze({ ...identity, kind: value.kind, notification: copyPickup(value.notification), health: copyAvailability(value.health, copyHealth, "pickup health"), weapon: copyAvailability(value.weapon, copyWeapon, "pickup weapon") })
    case "killfeed": return Object.freeze({ ...identity, kind: value.kind, notice: copyKillfeed(value.notice) })
  }
}

type HealthMode = "normal" | "bonus" | "dying"

function healthMode(health: Tf2HudHealth): HealthMode {
  if (health.current > health.maximum) return "bonus"
  if (health.current > 0 && health.current < Math.fround(health.maximum * Math.fround(HEALTH_WARNING_FRACTION))) return "dying"
  return "normal"
}

function healthAdjustment(health: Tf2HudHealth, mode: HealthMode): number {
  if (mode === "bonus") {
    const available = health.maximumBuffed - health.maximum
    if (available <= 0) return 0
    const fraction = Math.fround(Math.min(Math.fround((health.current - health.maximum) / available), 1))
    return sourceRoundToInt(Math.fround(fraction * HEALTH_BONUS_POSITION_ADJUSTMENT))
  }
  if (mode === "dying") {
    const threshold = Math.fround(health.maximum * Math.fround(HEALTH_WARNING_FRACTION))
    const fraction = Math.fround(Math.fround(threshold - health.current) / threshold)
    return sourceRoundToInt(Math.fround(fraction * HEALTH_BONUS_POSITION_ADJUSTMENT))
  }
  return 0
}

function lowAmmo(weapon: Tf2HudWeapon): boolean {
  if (
    weapon.ammoDisplay === "hidden"
    || weapon.clip.kind !== "available"
    || weapon.reserve.kind !== "available"
    || weapon.maximumClip.kind !== "available"
    || weapon.maximumReserve.kind !== "available"
  ) return false
  const maximum = weapon.maximumReserve.value + (weapon.maximumClip.value > 0 ? weapon.maximumClip.value : 0)
  const threshold = sourceRoundToInt(Math.fround(maximum * Math.fround(LOW_AMMO_WARNING_FRACTION)))
  return threshold > 0 && weapon.clip.value + weapon.reserve.value < threshold
}

function lowAmmoAdjustment(weapon: Tf2HudWeapon): number {
  if (!lowAmmo(weapon) || weapon.clip.kind !== "available" || weapon.reserve.kind !== "available" || weapon.maximumClip.kind !== "available" || weapon.maximumReserve.kind !== "available") return 0
  const threshold = sourceRoundToInt(Math.fround((weapon.maximumClip.value + weapon.maximumReserve.value) * Math.fround(LOW_AMMO_WARNING_FRACTION)))
  const fraction = Math.fround((threshold - weapon.clip.value - weapon.reserve.value) / threshold)
  return sourceRoundToInt(Math.fround(fraction * LOW_AMMO_POSITION_ADJUSTMENT))
}

function sameAvailabilityNumber(left: Tf2HudAvailability<number>, right: Tf2HudAvailability<number>): boolean {
  if (left.kind === "unavailable") return right.kind === "unavailable" && left.reason === right.reason
  return right.kind === "available" && left.value === right.value
}

function sameHealth(left: Tf2HudHealth, right: Tf2HudHealth): boolean {
  return left.current === right.current && left.maximum === right.maximum && left.maximumBuffed === right.maximumBuffed
}

function conditionActive(words: Tf2ConditionWords, condition: number): boolean {
  return (words[Math.floor(condition / 32)]! & (1 << (condition % 32))) !== 0
}

function healthFrom(snapshot: Tf2HudSnapshot): Tf2HudHealth | null {
  return snapshot.player.kind === "available" && snapshot.player.value.health.kind === "available"
    ? snapshot.player.value.health.value
    : null
}

function weaponsFrom(snapshot: Tf2HudSnapshot): Map<number, Tf2HudWeapon> {
  return new Map(snapshot.player.kind === "available" ? snapshot.player.value.weapons.map((weapon) => [weapon.identity, weapon]) : [])
}

function pushHealthAnimations(
  animations: Tf2HudAnimation[],
  prior: Tf2HudHealth | null,
  next: Tf2HudHealth,
  tick: bigint,
  ordinal: number,
): void {
  const before = prior ? healthMode(prior) : "normal"
  const after = healthMode(next)
  if (before === after) return
  const add = (sequence: Tf2HudAnimation["sequence"]) => animations.push(Object.freeze({ tick, ordinal, target: "HudPlayerHealth", sequence }))
  if (after === "bonus") {
    add("HudHealthDyingPulseStop")
    add("HudHealthBonusPulse")
  } else if (after === "dying") {
    add("HudHealthBonusPulseStop")
    add("HudHealthDyingPulse")
  } else {
    add("HudHealthBonusPulseStop")
    add("HudHealthDyingPulseStop")
  }
}

function pushAmmoAnimation(
  animations: Tf2HudAnimation[],
  prior: Tf2HudWeapon | null,
  next: Tf2HudWeapon,
  tick: bigint,
  ordinal: number,
): void {
  if (lowAmmo(prior ?? next) === lowAmmo(next) && prior !== null) return
  if (prior === null && !lowAmmo(next)) return
  animations.push(Object.freeze({
    tick,
    ordinal,
    target: "HudWeaponAmmo",
    sequence: lowAmmo(next) ? "HudLowAmmoPulse" : "HudLowAmmoPulseStop",
  }))
}

function withAmmo(weapon: Tf2HudWeapon, event: Extract<Tf2HudEvent, { kind: "ammo" }>): Tf2HudWeapon {
  return Object.freeze({ ...weapon, clip: event.clip, reserve: event.reserve, reload: event.reload })
}

function panelValues(snapshot: Tf2HudSnapshot): readonly Tf2HudPanelValue[] {
  const values: Tf2HudPanelValue[] = []
  const unavailableNumber = () => tf2HudUnavailable<number>("not-produced")
  const unavailableString = () => tf2HudUnavailable<string>("not-produced")
  const setVisible = (panel: string, value: boolean) => values.push(Object.freeze({ kind: "visible", panel, value }))
  const setDialog = (
    panel: string,
    variable: string,
    value: Tf2HudAvailability<string | number | Tf2HudLocalizedValue>,
  ) => values.push(Object.freeze({ kind: "dialog-variable", panel, variable, value }))
  const setScalar = (panel: string, property: string, value: Tf2HudAvailability<number>) => values.push(Object.freeze({ kind: "scalar", panel, property, value }))
  const setImage = (panel: string, value: Tf2HudAvailability<string>) => values.push(Object.freeze({ kind: "image", panel, value }))
  const setColor = (panel: string, property: string, color: Tf2HudAvailability<readonly [number, number, number, number]>) => {
    values.push(Object.freeze({ kind: "color", panel, property, value: color }))
  }

  const player = snapshot.player.kind === "available" ? snapshot.player.value : null
  const live = player?.lifecycle === "active" && !player.liveHudSuppressed
  setVisible("HudPlayerStatus", live)
  const health = player?.health.kind === "available" ? player.health.value : null
  const mode = health ? healthMode(health) : "normal"
  setScalar("PlayerStatusHealthImage", "fill", health ? tf2HudAvailable(Math.min(health.current / health.maximum, 1)) : unavailableNumber())
  setVisible("PlayerStatusHealthImageBG", live && health !== null && health.current > 0)
  setVisible("PlayerStatusHealthBonusImage", live && health !== null && mode !== "normal")
  setScalar("PlayerStatusHealthBonusImage", "boundsAdjustment", health ? tf2HudAvailable(healthAdjustment(health, mode)) : unavailableNumber())
  const healthColor: readonly [number, number, number, number] = mode === "dying"
    ? Object.freeze([255, 0, 0, 255])
    : Object.freeze([255, 255, 255, 255])
  setColor("PlayerStatusHealthImage", "drawColor", health ? tf2HudAvailable(healthColor) : tf2HudUnavailable("not-produced"))
  setColor("PlayerStatusHealthBonusImage", "drawColor", health && mode !== "normal" ? tf2HudAvailable(healthColor) : tf2HudUnavailable("not-applicable"))
  setDialog("HudPlayerHealth", "Health", health ? tf2HudAvailable(health.current > 0 ? health.current : "") : unavailableString())
  setDialog("HudPlayerHealth", "MaxHealth", health ? tf2HudAvailable(health.current > 0 && health.maximum - health.current >= 5 ? health.maximum : "") : unavailableString())

  const team = player?.team.kind === "available" ? player.team.value : null
  const tfClass = player?.class.kind === "available" ? player.class.value : null
  const playableTeam = team === 2 || team === 3 ? team : null
  const classIdentityAvailable = playableTeam !== null && tfClass !== null
  const usePlayerModel = live && classIdentityAvailable && player?.playerClassUsePlayerModel === true
  const useClassImage = live && classIdentityAvailable && !usePlayerModel
  setVisible("PlayerStatusClassImage", useClassImage)
  setVisible("PlayerStatusClassImageBG", useClassImage)
  setVisible("classmodelpanel", usePlayerModel)
  setVisible("classmodelpanelBG", usePlayerModel)
  setVisible("CarryingWeapon", false)
  setVisible("PlayerStatusSpyImage", false)
  setVisible("PlayerStatusSpyOutlineImage", false)
  setImage("PlayerStatusClassImage", playableTeam && tfClass ? tf2HudAvailable(TF2_CLASS_IMAGES[playableTeam][tfClass]) : unavailableString())
  setImage("PlayerStatusClassImageBG", playableTeam ? tf2HudAvailable(playableTeam === 2 ? "../hud/character_red_bg" : "../hud/character_blue_bg") : unavailableString())
  setImage("classmodelpanelBG", playableTeam ? tf2HudAvailable(playableTeam === 2 ? "../hud/character_red_bg_clipped" : "../hud/character_blue_bg_clipped") : unavailableString())
  setScalar("HudPlayerClass", "team", team === null ? unavailableNumber() : tf2HudAvailable(team))
  setScalar("HudPlayerClass", "class", tfClass === null ? unavailableNumber() : tf2HudAvailable(tfClass))
  const classModel = player?.classModel.kind === "available" ? player.classModel.value : null
  setDialog("classmodelpanel", "modelIdentity", classModel ? tf2HudAvailable(classModel.identity) : unavailableString())
  setScalar("classmodelpanel", "skin", classModel ? tf2HudAvailable(classModel.skin) : unavailableNumber())
  setScalar("classmodelpanel", "team", team === null ? unavailableNumber() : tf2HudAvailable(team))
  setScalar("classmodelpanel", "class", tfClass === null ? unavailableNumber() : tf2HudAvailable(tfClass))

  const conditions = player?.conditions ?? Object.freeze([0, 0, 0, 0, 0]) as Tf2ConditionWords
  const groups = new Set<string>()
  for (const item of TF2_GROUPED_CONDITION_PANELS) {
    const visible = live && conditionActive(conditions, item.condition) && !groups.has(item.group)
    if (visible) groups.add(item.group)
    setVisible(item.panel, visible)
    setImage(item.panel, visible && playableTeam ? tf2HudAvailable(playableTeam === 3 ? item.blueImage : item.redImage) : tf2HudUnavailable("not-applicable"))
  }
  for (const item of TF2_INDEPENDENT_CONDITION_PANELS) {
    setVisible(item.panel, live && item.conditions.some((condition) => conditionActive(conditions, condition)))
  }
  setVisible("PlayerStatus_WheelOfDoom", false)

  const activeWeaponIdentity = player?.activeWeapon.kind === "available" ? player.activeWeapon.value : null
  const activeWeapon = activeWeaponIdentity !== null
    ? player?.weapons.find((weapon) => weapon.identity === activeWeaponIdentity) ?? null
    : null
  const ammoVisible = live && activeWeapon !== null && activeWeapon.ammoDisplay !== "hidden"
  setVisible("HudWeaponAmmo", ammoVisible)
  setImage("HudWeaponAmmoBG", playableTeam ? tf2HudAvailable(playableTeam === 2 ? "../hud/ammo_red_bg" : "../hud/ammo_blue_bg") : unavailableString())
  const clipMode = activeWeapon?.ammoDisplay === "clip-and-reserve"
  const totalMode = activeWeapon?.ammoDisplay === "total"
  setVisible("AmmoInClip", ammoVisible && clipMode)
  setVisible("AmmoInClipShadow", ammoVisible && clipMode)
  setVisible("AmmoInReserve", ammoVisible && clipMode)
  setVisible("AmmoInReserveShadow", ammoVisible && clipMode)
  setVisible("AmmoNoClip", ammoVisible && totalMode)
  setVisible("AmmoNoClipShadow", ammoVisible && totalMode)
  setDialog("HudWeaponAmmo", "Ammo", activeWeapon?.clip.kind === "available" ? tf2HudAvailable(activeWeapon.clip.value) : unavailableNumber())
  setDialog("HudWeaponAmmo", "AmmoInReserve", activeWeapon?.reserve.kind === "available" ? tf2HudAvailable(activeWeapon.reserve.value) : unavailableNumber())
  setVisible("HudWeaponLowAmmoImage", ammoVisible && activeWeapon !== null && lowAmmo(activeWeapon))
  setScalar("HudWeaponLowAmmoImage", "boundsAdjustment", activeWeapon ? tf2HudAvailable(lowAmmoAdjustment(activeWeapon)) : unavailableNumber())
  setColor("HudWeaponLowAmmoImage", "foreground", ammoVisible && activeWeapon !== null && lowAmmo(activeWeapon)
    ? tf2HudAvailable(Object.freeze([255, 0, 0, 255]))
    : tf2HudUnavailable("not-applicable"))
  setScalar("HudWeaponAmmo", "reloadPhase", activeWeapon ? tf2HudAvailable(["ready", "start", "insert", "finish"].indexOf(activeWeapon.reload)) : unavailableNumber())
  setScalar("classmodelpanel", "weaponIdentity", activeWeapon ? tf2HudAvailable(activeWeapon.identity) : unavailableNumber())
  setDialog("classmodelpanel", "weaponName", activeWeapon ? tf2HudAvailable(activeWeapon.displayName) : unavailableString())
  setScalar("classmodelpanel", "itemDefinition", activeWeapon?.itemDefinition.kind === "available" ? tf2HudAvailable(activeWeapon.itemDefinition.value) : unavailableNumber())

  setVisible("HudWeaponSelection", live && player?.weaponSelection.open === true)
  for (let slot = 0; slot < 6; slot += 1) {
    const candidates = player?.weapons.filter((weapon) => weapon.slot === slot).sort((left, right) => left.position - right.position) ?? []
    const selectedIdentity = player?.weaponSelection.selectedWeapon.kind === "available"
      ? player.weaponSelection.selectedWeapon.value
      : null
    const selected = selectedIdentity !== null
      ? candidates.find((weapon) => weapon.identity === selectedIdentity)
      : undefined
    const weapon = selected ?? candidates[0]
    setScalar(`modelpanel${slot}`, "weaponIdentity", weapon ? tf2HudAvailable(weapon.identity) : unavailableNumber())
    setDialog(`modelpanel${slot}`, "weaponName", weapon ? tf2HudAvailable(weapon.displayName) : unavailableString())
    setScalar(`modelpanel${slot}`, "itemDefinition", weapon?.itemDefinition.kind === "available" ? tf2HudAvailable(weapon.itemDefinition.value) : unavailableNumber())
  }

  const crosshair = player?.crosshair.kind === "available" ? player.crosshair.value : null
  const observerEligible = crosshair?.observerMode === "in-eye" || (crosshair?.observerMode === "roaming" && crosshair.observerCrosshair)
  const crosshairVisible = crosshair !== null
    && crosshair.configured
    && !crosshair.loadingImage
    && !crosshair.paused
    && crosshair.clientModeAllows
    && !crosshair.frozen
    && crosshair.localViewEntity
    && !crosshair.vguiInput
    && !crosshair.tfSuppressed
    && !crosshair.countdownHidden
    && (player?.lifecycle === "active" || observerEligible)
    && (crosshair.weaponAllows && activeWeapon?.drawsCrosshair !== false || tf2CustomCrosshairFile(crosshair.texture) !== null)
  setVisible("HudCrosshair", crosshairVisible)
  setImage("HudCrosshair", crosshair ? tf2HudAvailable(crosshair.texture) : unavailableString())
  setColor("HudCrosshair", "drawColor", crosshair
    ? tf2HudAvailable<readonly [number, number, number, number]>(crosshair.color)
    : tf2HudUnavailable<readonly [number, number, number, number]>("not-produced"))
  setScalar("HudCrosshair", "scale", crosshair ? tf2HudAvailable(crosshair.scale * crosshair.weaponScale) : unavailableNumber())

  const freeze = snapshot.freezePanel.kind === "available" ? snapshot.freezePanel.value : null
  setVisible("FreezePanel", freeze !== null)
  setDialog("FreezePanel", "killername", freeze?.killerName.kind === "available" ? tf2HudAvailable(freeze.killerName.value) : unavailableString())
  setDialog("FreezePanel", "nemesisname", freeze?.nemesisName.kind === "available" ? tf2HudAvailable(freeze.nemesisName.value) : unavailableString())

  const scoreboard = snapshot.scoreboard.kind === "available" ? snapshot.scoreboard.value : null
  setVisible("scoreinfo", scoreboard?.visible === true)
  for (const [prefix, item] of scoreboard ? [["red", scoreboard.red], ["blue", scoreboard.blue]] as const : []) {
    setDialog("scoreinfo", `${prefix}teamscore`, tf2HudAvailable(item.score))
    setDialog("scoreinfo", `${prefix}teamname`, tf2HudAvailable(item.localizedName))
    setDialog("scoreinfo", `${prefix}teamplayercount`, tf2HudAvailable(Object.freeze({
      kind: "localized",
      token: item.playerCount === 1 ? "#TF_ScoreBoard_Player" : "#TF_ScoreBoard_Players",
      parameters: Object.freeze([item.playerCount]),
    })))
  }
  if (!scoreboard) {
    for (const variable of ["redteamscore", "redteamname", "redteamplayercount", "blueteamscore", "blueteamname", "blueteamplayercount"]) {
      setDialog("scoreinfo", variable, tf2HudUnavailable("not-produced"))
    }
  }
  return Object.freeze(values)
}

export function bindTf2Hud(publication: Tf2HudPublication): Tf2HudBinding {
  if (!publication || typeof publication !== "object") malformed("HUD publication is invalid")
  const previous = copyAvailability(publication.previous, copySnapshot, "previous HUD snapshot")
  const snapshot = copySnapshot(publication.snapshot)
  if (!Array.isArray(publication.events)) malformed("HUD event stream is invalid")
  if (publication.events.length > TF2_HUD_LIMITS.eventsPerPublication) bound("HUD event stream exceeds its bound")
  const events = Object.freeze(publication.events.map(copyEvent))
  if (previous.kind === "available" && snapshot.tick <= previous.value.tick) {
    throw new Tf2HudBindingError("EventOrder", "continuous HUD snapshot tick did not advance")
  }
  let priorTick: bigint | null = null
  let expectedOrdinal = 0
  for (const event of events) {
    if (event.tick > snapshot.tick || (previous.kind === "available" && event.tick <= previous.value.tick)) {
      throw new Tf2HudBindingError("EventOrder", "HUD event is outside its publication interval")
    }
    if (priorTick === null || event.tick !== priorTick) {
      if (priorTick !== null && event.tick < priorTick) throw new Tf2HudBindingError("EventOrder", "HUD event ticks are not ordered")
      priorTick = event.tick
      expectedOrdinal = 0
    }
    if (event.ordinal !== expectedOrdinal) throw new Tf2HudBindingError("EventOrder", "HUD event ordinals are not contiguous")
    expectedOrdinal += 1
  }

  const animations: Tf2HudAnimation[] = []
  const commands: Tf2HudPresentationCommand[] = []
  let rollingHealth = previous.kind === "available" ? healthFrom(previous.value) : null
  const rollingWeapons = previous.kind === "available" ? weaponsFrom(previous.value) : new Map<number, Tf2HudWeapon>()
  let rollingActiveWeapon = previous.kind === "available" && previous.value.player.kind === "available"
    && previous.value.player.value.activeWeapon.kind === "available"
    ? previous.value.player.value.activeWeapon.value
    : null
  let indicatorWeapon = rollingActiveWeapon === null ? null : rollingWeapons.get(rollingActiveWeapon) ?? null
  let lastHealth: Tf2HudHealth | null = null
  const lastAmmo = new Map<number, Tf2HudWeapon>()
  let lastWeapon: number | null = null
  let lastLifecycle: Tf2HudPlayer["lifecycle"] | null = null
  let lastConditions: Tf2ConditionWords | null = null
  let lastClass: Tf2Class | null = null
  let lastTeam: Tf2Team | null = null

  const transitionHealth = (next: Tf2HudHealth, event: Pick<Tf2HudEvent, "tick" | "ordinal">) => {
    pushHealthAnimations(animations, rollingHealth, next, event.tick, event.ordinal)
    rollingHealth = next
    lastHealth = next
  }
  const transitionWeapon = (next: Tf2HudWeapon, event: Pick<Tf2HudEvent, "tick" | "ordinal">) => {
    rollingWeapons.set(next.identity, next)
    lastAmmo.set(next.identity, next)
    if (next.identity === rollingActiveWeapon) {
      pushAmmoAnimation(animations, indicatorWeapon, next, event.tick, event.ordinal)
      indicatorWeapon = next
    }
  }

  for (const event of events) {
    switch (event.kind) {
      case "health": transitionHealth(event.health, event); break
      case "damage": {
        transitionHealth(event.health, event)
        if (event.direction.kind === "available") {
          const scale = Math.min(event.amount, 100)
          commands.push(Object.freeze({ kind: "damage-indicator", tick: event.tick, ordinal: event.ordinal, scale, lifetimeSeconds: Math.fround(1 + Math.fround(scale / 100)), direction: event.direction.value }))
        }
        break
      }
      case "ammo": {
        const source = rollingWeapons.get(event.weapon) ?? weaponsFrom(snapshot).get(event.weapon)
        if (!source) throw new Tf2HudBindingError("InconsistentPublication", "ammo event weapon is absent")
        transitionWeapon(withAmmo(source, event), event)
        break
      }
      case "weapon-selected":
        lastWeapon = event.weapon
        rollingActiveWeapon = event.weapon
        {
          const selected = rollingWeapons.get(event.weapon) ?? weaponsFrom(snapshot).get(event.weapon)
          if (!selected) throw new Tf2HudBindingError("InconsistentPublication", "selected weapon is absent")
          pushAmmoAnimation(animations, indicatorWeapon, selected, event.tick, event.ordinal)
          indicatorWeapon = selected
        }
        commands.push(Object.freeze({ kind: "weapon-selected", tick: event.tick, ordinal: event.ordinal, weapon: event.weapon }))
        break
      case "class-changed": lastClass = event.class; break
      case "team-changed": lastTeam = event.team; break
      case "conditions": lastConditions = event.conditions; break
      case "lifecycle":
        lastLifecycle = event.lifecycle
        commands.push(Object.freeze({ kind: "lifecycle", tick: event.tick, ordinal: event.ordinal, lifecycle: event.lifecycle }))
        break
      case "regenerate":
        transitionHealth(event.health, event)
        for (const weapon of event.weapons) transitionWeapon(weapon, event)
        lastConditions = event.conditions
        commands.push(Object.freeze({ kind: "regenerate-notification", tick: event.tick, ordinal: event.ordinal, zone: event.zone }))
        break
      case "pickup":
        if (event.health.kind === "available") transitionHealth(event.health.value, event)
        if (event.weapon.kind === "available") transitionWeapon(event.weapon.value, event)
        commands.push(Object.freeze({ kind: "pickup-notification", tick: event.tick, ordinal: event.ordinal, notification: event.notification }))
        break
      case "killfeed":
        commands.push(Object.freeze({ kind: "killfeed-notice", tick: event.tick, ordinal: event.ordinal, notice: event.notice }))
        break
    }
  }

  if (rollingHealth === null) {
    const finalHealth = healthFrom(snapshot)
    if (finalHealth) {
      const last = events.at(-1)
      const ordinal = last?.tick === snapshot.tick ? last.ordinal + 1 : 0
      pushHealthAnimations(animations, null, finalHealth, snapshot.tick, ordinal)
    }
  }

  if (snapshot.player.kind === "available") {
    const final = snapshot.player.value
    if (lastHealth && (final.health.kind !== "available" || !sameHealth(lastHealth, final.health.value))) {
      throw new Tf2HudBindingError("InconsistentPublication", "final HUD health differs from the event stream")
    }
    for (const [identity, event] of lastAmmo) {
      const weapon = final.weapons.find((item) => item.identity === identity)
      if (!weapon || !sameAvailabilityNumber(event.clip, weapon.clip) || !sameAvailabilityNumber(event.reserve, weapon.reserve) || event.reload !== weapon.reload) {
        throw new Tf2HudBindingError("InconsistentPublication", "final HUD ammo differs from the event stream")
      }
    }
    if (lastWeapon !== null && (final.activeWeapon.kind !== "available" || final.activeWeapon.value !== lastWeapon)) throw new Tf2HudBindingError("InconsistentPublication", "final active weapon differs from the event stream")
    if (lastLifecycle !== null && final.lifecycle !== lastLifecycle) throw new Tf2HudBindingError("InconsistentPublication", "final lifecycle differs from the event stream")
    if (lastClass !== null && (final.class.kind !== "available" || final.class.value !== lastClass)) throw new Tf2HudBindingError("InconsistentPublication", "final class differs from the event stream")
    if (lastTeam !== null && (final.team.kind !== "available" || final.team.value !== lastTeam)) throw new Tf2HudBindingError("InconsistentPublication", "final team differs from the event stream")
    if (lastConditions && final.conditions.some((word, index) => word !== lastConditions![index])) throw new Tf2HudBindingError("InconsistentPublication", "final conditions differ from the event stream")

    const finalActiveIdentity = final.activeWeapon.kind === "available" ? final.activeWeapon.value : null
    const finalActiveWeapon = finalActiveIdentity === null
      ? null
      : final.weapons.find((weapon) => weapon.identity === finalActiveIdentity) ?? null
    if (finalActiveWeapon && rollingActiveWeapon !== finalActiveWeapon.identity) {
      const last = events.at(-1)
      const ordinal = last?.tick === snapshot.tick ? last.ordinal + 1 : 0
      pushAmmoAnimation(animations, indicatorWeapon, finalActiveWeapon, snapshot.tick, ordinal)
      rollingActiveWeapon = finalActiveWeapon.identity
      indicatorWeapon = finalActiveWeapon
    } else if (finalActiveWeapon && indicatorWeapon === null) {
      const last = events.at(-1)
      const ordinal = last?.tick === snapshot.tick ? last.ordinal + 1 : 0
      pushAmmoAnimation(animations, null, finalActiveWeapon, snapshot.tick, ordinal)
      indicatorWeapon = finalActiveWeapon
    }
  }

  return Object.freeze({
    tick: snapshot.tick,
    facts: snapshot,
    values: panelValues(snapshot),
    animations: Object.freeze(animations),
    commands: Object.freeze(commands),
    scoreboard: snapshot.scoreboard,
  })
}

export function bindTf2HudAction(snapshotInput: Tf2HudSnapshot, action: Tf2HudAction): Tf2HudAvailability<Tf2HudCommand> {
  const snapshot = copySnapshot(snapshotInput)
  if (!action || typeof action !== "object") malformed("HUD action is invalid")
  if (action.kind === "scoreboard") return tf2HudAvailable(Object.freeze({ kind: "scoreboard", visible: action.visible === true }))
  if (snapshot.player.kind === "unavailable") return tf2HudUnavailable("not-applicable")
  const player = snapshot.player.value
  if (action.kind === "respawn") {
    return player.lifecycle === "dying" && player.respawnAllowed
      ? tf2HudAvailable(Object.freeze({ kind: "respawn", player: player.identity }))
      : tf2HudUnavailable("not-applicable")
  }
  if (action.kind === "select-weapon") {
    const identity = canonicalIdentity(action.weapon, "HUD action weapon")
    const weapon = player.weapons.find((item) => item.identity === identity)
    return player.lifecycle === "active" && weapon?.selectable === true
      ? tf2HudAvailable(Object.freeze({ kind: "select-weapon", player: player.identity, weapon: identity }))
      : tf2HudUnavailable("not-applicable")
  }
  return malformed("HUD action kind is invalid")
}
