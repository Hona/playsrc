export const TF2_HUD_LIMITS = Object.freeze({
  eventsPerPublication: 65_536,
  scoreboardPlayers: 64,
  loadoutWeapons: 22,
  conditionWords: 5,
  deathNotices: 4,
  playerNameCodeUnits: 31,
  resourceIdentityCodeUnits: 1_024,
})

export type Tf2HudUnavailableReason =
  | "initial"
  | "not-produced"
  | "not-applicable"
  | "replay-discontinuity"
  | "missing-source-fact"

export type Tf2HudAvailability<T> =
  | Readonly<{ kind: "available"; value: T }>
  | Readonly<{ kind: "unavailable"; reason: Tf2HudUnavailableReason }>

export type Tf2Class = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
export type Tf2Team = 0 | 1 | 2 | 3
export type Tf2PlayableTeam = 2 | 3
export type Tf2ConditionWords = readonly [number, number, number, number, number]
export type Tf2ReloadPhase = "ready" | "start" | "insert" | "finish"

export type Tf2HudClassModel = Readonly<{
  identity: string
  skin: number
}>

export type Tf2HudHealth = Readonly<{
  current: number
  maximum: number
  maximumBuffed: number
}>

export type Tf2HudWeapon = Readonly<{
  identity: number
  itemDefinition: Tf2HudAvailability<number>
  displayName: string
  slot: number
  position: number
  selectable: boolean
  ammoDisplay: "clip-and-reserve" | "total" | "hidden"
  clip: Tf2HudAvailability<number>
  reserve: Tf2HudAvailability<number>
  maximumClip: Tf2HudAvailability<number>
  maximumReserve: Tf2HudAvailability<number>
  reload: Tf2ReloadPhase
  drawsCrosshair: boolean
}>

export type Tf2HudWeaponSelection = Readonly<{
  open: boolean
  selectedWeapon: Tf2HudAvailability<number>
}>

export type Tf2HudCrosshair = Readonly<{
  configured: boolean
  weaponAllows: boolean
  loadingImage: boolean
  paused: boolean
  clientModeAllows: boolean
  frozen: boolean
  localViewEntity: boolean
  vguiInput: boolean
  observerMode: "none" | "in-eye" | "roaming" | "other"
  observerCrosshair: boolean
  tfSuppressed: boolean
  countdownHidden: boolean
  texture: string
  color: readonly [number, number, number, number]
  scale: number
  weaponScale: number
}>

export type Tf2HudPlayer = Readonly<{
  identity: number
  lifecycle: "active" | "dying" | "observer"
  class: Tf2HudAvailability<Tf2Class>
  team: Tf2HudAvailability<Tf2Team>
  playerClassUsePlayerModel: boolean
  classModel: Tf2HudAvailability<Tf2HudClassModel>
  health: Tf2HudAvailability<Tf2HudHealth>
  conditions: Tf2ConditionWords
  weapons: readonly Tf2HudWeapon[]
  activeWeapon: Tf2HudAvailability<number>
  weaponSelection: Tf2HudWeaponSelection
  crosshair: Tf2HudAvailability<Tf2HudCrosshair>
  liveHudSuppressed: boolean
  respawnAllowed: boolean
}>

export type Tf2ScoreboardCounters = Readonly<{
  kills: number
  deaths: number
  assists: number
  destruction: number
  captures: number
  defenses: number
  dominations: number
  revenge: number
  healing: number
  invulns: number
  teleports: number
  headshots: number
  backstabs: number
  bonus: number
  support: number
  damage: number
}>

export type Tf2ScoreboardPlayer = Readonly<{
  identity: number
  name: string
  team: Tf2PlayableTeam
  connection: "connected" | "connecting" | "loading" | "disconnected"
  score: number
  alive: boolean
  class: Tf2HudAvailability<Tf2Class>
  ping: Tf2HudAvailability<number | "bot">
  killstreak: number
  activeDominations: number
  relationship: "none" | "dominating-local" | "dominated-by-local"
  counters: Tf2HudAvailability<Tf2ScoreboardCounters>
}>

export type Tf2ScoreboardTeam = Readonly<{
  team: Tf2PlayableTeam
  localizedName: string
  score: number
  playerCount: number
}>

export type Tf2HudScoreboard = Readonly<{
  visible: boolean
  mapName: string
  gameType: Tf2HudAvailability<"#Gametype_CTF" | "#Gametype_Escort">
  pingAsText: boolean
  red: Tf2ScoreboardTeam
  blue: Tf2ScoreboardTeam
  players: readonly Tf2ScoreboardPlayer[]
  spectators: readonly string[]
  waitingToPlay: readonly string[]
  selectedPlayer: Tf2HudAvailability<number>
}>

export type Tf2HudFreezePanel = Readonly<{
  killerIdentity: Tf2HudAvailability<number>
  killerName: Tf2HudAvailability<string>
  killerHealth: Tf2HudAvailability<Tf2HudHealth>
  nemesisName: Tf2HudAvailability<string>
}>

export type Tf2HudSnapshot = Readonly<{
  tick: bigint
  player: Tf2HudAvailability<Tf2HudPlayer>
  scoreboard: Tf2HudAvailability<Tf2HudScoreboard>
  freezePanel: Tf2HudAvailability<Tf2HudFreezePanel>
}>

type Tf2HudEventIdentity = Readonly<{ tick: bigint; ordinal: number }>

export type Tf2HudHealthCause = "damage" | "heal" | "pickup" | "regenerate" | "respawn" | "state"
export type Tf2HudAmmoCause = "fire" | "reload" | "pickup" | "regenerate" | "respawn" | "state"

export type Tf2HudKillfeedParticipant = Readonly<{
  identity: Tf2HudAvailability<number>
  name: string
  team: Tf2Team
}>

export type Tf2HudKillfeedNotice = Readonly<{
  killer: Tf2HudKillfeedParticipant
  victim: Tf2HudKillfeedParticipant
  assister: Tf2HudAvailability<Tf2HudKillfeedParticipant>
  weaponIcon: Tf2HudAvailability<string>
  weaponIdentity: Tf2HudAvailability<number>
  customKill: number
  critical: boolean
  selfInflicted: boolean
  localPlayerInvolved: boolean
  domination: boolean
  revenge: boolean
  silent: boolean
}>

export type Tf2HudPickupNotification = Readonly<{
  pickupIdentity: number
  pickup: "health" | "ammo" | "weapon" | "item"
  itemIdentity: Tf2HudAvailability<number | string>
  amount: Tf2HudAvailability<number>
}>

export type Tf2HudEvent =
  | (Tf2HudEventIdentity & Readonly<{ kind: "health"; health: Tf2HudHealth; cause: Tf2HudHealthCause }>)
  | (Tf2HudEventIdentity & Readonly<{
      kind: "ammo"
      weapon: number
      clip: Tf2HudAvailability<number>
      reserve: Tf2HudAvailability<number>
      reload: Tf2ReloadPhase
      cause: Tf2HudAmmoCause
    }>)
  | (Tf2HudEventIdentity & Readonly<{ kind: "weapon-selected"; weapon: number }>)
  | (Tf2HudEventIdentity & Readonly<{ kind: "class-changed"; class: Tf2Class }>)
  | (Tf2HudEventIdentity & Readonly<{ kind: "team-changed"; team: Tf2Team }>)
  | (Tf2HudEventIdentity & Readonly<{ kind: "conditions"; conditions: Tf2ConditionWords }>)
  | (Tf2HudEventIdentity & Readonly<{
      kind: "damage"
      amount: number
      health: Tf2HudHealth
      direction: Tf2HudAvailability<readonly [number, number, number]>
    }>)
  | (Tf2HudEventIdentity & Readonly<{
      kind: "lifecycle"
      lifecycle: "active" | "dying" | "observer"
    }>)
  | (Tf2HudEventIdentity & Readonly<{
      kind: "regenerate"
      zone: Tf2HudAvailability<number>
      health: Tf2HudHealth
      weapons: readonly Tf2HudWeapon[]
      conditions: Tf2ConditionWords
    }>)
  | (Tf2HudEventIdentity & Readonly<{
      kind: "pickup"
      notification: Tf2HudPickupNotification
      health: Tf2HudAvailability<Tf2HudHealth>
      weapon: Tf2HudAvailability<Tf2HudWeapon>
    }>)
  | (Tf2HudEventIdentity & Readonly<{ kind: "killfeed"; notice: Tf2HudKillfeedNotice }>)

export type Tf2HudPublication = Readonly<{
  previous: Tf2HudAvailability<Tf2HudSnapshot>
  snapshot: Tf2HudSnapshot
  events: readonly Tf2HudEvent[]
}>

export type Tf2HudLocalizedValue = Readonly<{
  kind: "localized"
  token: string
  parameters: readonly (string | number)[]
}>

export type Tf2HudPanelValue =
  | Readonly<{ kind: "visible"; panel: string; value: boolean }>
  | Readonly<{
      kind: "dialog-variable"
      panel: string
      variable: string
      value: Tf2HudAvailability<string | number | Tf2HudLocalizedValue>
    }>
  | Readonly<{ kind: "image"; panel: string; value: Tf2HudAvailability<string> }>
  | Readonly<{
      kind: "scalar"
      panel: string
      property: string
      value: Tf2HudAvailability<number>
    }>
  | Readonly<{
      kind: "color"
      panel: string
      property: string
      value: Tf2HudAvailability<readonly [number, number, number, number]>
    }>

export type Tf2HudAnimation = Readonly<{
  tick: bigint
  ordinal: number
  target: "viewport" | string
  sequence:
    | "HudHealthBonusPulse"
    | "HudHealthBonusPulseStop"
    | "HudHealthDyingPulse"
    | "HudHealthDyingPulseStop"
    | "HudLowAmmoPulse"
    | "HudLowAmmoPulseStop"
}>

export type Tf2HudPresentationCommand =
  | Readonly<{
      kind: "damage-indicator"
      tick: bigint
      ordinal: number
      scale: number
      lifetimeSeconds: number
      direction: readonly [number, number, number]
    }>
  | Readonly<{ kind: "regenerate-notification"; tick: bigint; ordinal: number; zone: Tf2HudAvailability<number> }>
  | Readonly<{ kind: "pickup-notification"; tick: bigint; ordinal: number; notification: Tf2HudPickupNotification }>
  | Readonly<{ kind: "killfeed-notice"; tick: bigint; ordinal: number; notice: Tf2HudKillfeedNotice }>
  | Readonly<{
      kind: "lifecycle"
      tick: bigint
      ordinal: number
      lifecycle: "active" | "dying" | "observer"
    }>
  | Readonly<{ kind: "weapon-selected"; tick: bigint; ordinal: number; weapon: number }>

export type Tf2HudBinding = Readonly<{
  tick: bigint
  facts: Tf2HudSnapshot
  values: readonly Tf2HudPanelValue[]
  animations: readonly Tf2HudAnimation[]
  commands: readonly Tf2HudPresentationCommand[]
  scoreboard: Tf2HudAvailability<Tf2HudScoreboard>
}>

export type Tf2HudAction =
  | Readonly<{ kind: "select-weapon"; weapon: number }>
  | Readonly<{ kind: "respawn" }>
  | Readonly<{ kind: "scoreboard"; visible: boolean }>

export type Tf2HudCommand =
  | Readonly<{ kind: "select-weapon"; player: number; weapon: number }>
  | Readonly<{ kind: "respawn"; player: number }>
  | Readonly<{ kind: "scoreboard"; visible: boolean }>

export class Tf2HudBindingError extends Error {
  constructor(
    readonly code: "MalformedFacts" | "BoundExceeded" | "EventOrder" | "InconsistentPublication",
    message: string,
  ) {
    super(message)
    this.name = "Tf2HudBindingError"
  }
}
