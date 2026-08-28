import type { SnapshotRanges } from "./snapshot-retention"
import { decodeEquippedItems } from "./equipment/codec"
import type { Tf2EquippedItem } from "./equipment/types"

const HASH = /^[0-9a-f]{64}$/
const MAX_SNAPSHOT_BYTES = 64 * 1024 * 1024
const MAX_RECORDS = 65_536
const MOVEMENT_BYTES = 96
const NO_EQUIPPED_ITEMS: readonly Tf2EquippedItem[] = Object.freeze([])

export type Tf2Class = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
export type Tf2Team = 0 | 1 | 2 | 3
export type Tf2Weapon = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 40 | 41 | 42 | 43 | 44 | 45 | 50 | 51 | 52 | 53 | 54 | 60 | 90 | 91 | 92 | 93 | 94 | 95 | 96 | 97 | 98
export const isTf2Weapon = (value: number | undefined): value is Tf2Weapon => value !== undefined && Number.isInteger(value)
  && (value >= 1 && value <= 21 || value >= 40 && value <= 45 || value >= 50 && value <= 54 || value === 60 || value >= 90 && value <= 98)
export type Tf2BuildingKind = 0 | 1 | 2
export type Tf2BuildingMode = 0 | 1
export type Tf2BuildingObject = Readonly<{ kind: Tf2BuildingKind; mode: Tf2BuildingMode }>
export type Tf2BuildingRequest = Readonly<
  | { action: "build" | "destroy"; object: Tf2BuildingObject }
  | { action: "rotate" | "cancel" }
  | { action: "hurt"; amount: number }
>
export type MovementMode = 0 | 1
export type ProjectileKind = 1 | 2 | 3 | 4
export type ProjectileTrail = 0 | 1 | 2 | 3 | 4 | 5 | 6
export type ProjectileState = 1 | 2 | 3
export type ContactKind = 1 | 2 | 3
export type BotDifficulty = 0 | 1 | 2 | 3
export type BotQuotaMode = "normal" | "fill" | "match"
export type BotConfiguration = Readonly<{
  quota: number
  maximumPlayers: number
  mode: BotQuotaMode
  difficulty: BotDifficulty
  joinAfterPlayer: boolean
  autoVacate: boolean
  offlinePractice: boolean
}>
export type BotRequest = Readonly<
  | { action: "add"; count: number; class?: Tf2Class; team?: Tf2Team; difficulty: BotDifficulty }
  | { action: "kick-all" }
  | { action: "kick-team"; team: Tf2Team }
>

export type ProjectilePhysicsResult = Readonly<{
  projectile: number
  tick: bigint
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  motionEnabled: boolean
  contact: null | Readonly<{ kind: ContactKind; normal: readonly [number, number, number] }>
}>
export type RandomStreamState = Readonly<{
  current: number
  shuffled: number
  table: readonly number[]
}>
export type Tf2RandomState = Readonly<{
  payloadWarningAvailable: readonly [number, number]
  configuredAvailable: readonly number[]
  projectileUnlockAvailable: readonly number[]
  authority: RandomStreamState
  predictedPresentation: RandomStreamState
  rocketExplosionAvailable: number
  stickyExplosionAvailable: number
  batHitWorldAvailable: number
  shovelHitWorldAvailable: number
  shovelHitFleshAvailable: number
  fistMissAvailable: number
  fistHitWorldAvailable: number
  fistHitFleshAvailable: number
  kukriHitFleshAvailable: number
  kukriHitWorldAvailable: number
  wrenchHitFleshAvailable: number
  bottleHitFleshAvailable: number
  bottleHitWorldAvailable: number
  knifeHitFleshAvailable: number

  fireAxeHitWorldAvailable: number
  fireAxeHitFleshAvailable: number
  flagEnemyStolenAvailable: number
  flagEnemyDroppedAvailable: number
  flagEnemyCapturedAvailable: number
  flagEnemyReturnedAvailable: number
  flagTeamDroppedAvailable: number
  bonesawHitFleshAvailable: number
  bonesawHitWorldAvailable: number
  overtimeAvailable: number
  controlPointAvailable: number
}>
import { configuredEquipmentSoundWaves, nativeEquipmentSounds, nativeEquipmentSoundWaves } from "./equipment/audio.generated"

function isSoundDefinition(value: number | undefined): value is number {
  return value !== undefined && (Object.hasOwn(nativeEquipmentSounds, value) || value >= 160 && value < 160 + configuredEquipmentSoundWaves.length)
}

export type RandomDraw = Readonly<{
  context: 1 | 2
  decision: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 64 | 65
  definition: number
  phase: 0 | 1 | 2
  raw: number
  result: Readonly<{ kind: "float-bits"; bits: number } | { kind: "integer"; value: number } | { kind: "rejected-integer" }>
}>
export type AudioEvent = Readonly<{
  pitchOverride?: number
  action: "play" | "stop" | "fade-in" | "fade-out"
  fadeSeconds: number
  tick: bigint
  ordinal: number
  identity: 1 | 2 | 3 | 4 | 5
  definition: number
  sourceKind: 1 | 2 | 3 | 4
  sourceIdentity: number
  ownerIdentity: number | null
  position: readonly [number, number, number]
  samples: Readonly<{ volume: number; pitch: number; wave: number; soundLevel: number }>
}>
export type RocketTraceResult = Readonly<{
  projectile: number
  tick: bigint
  end: readonly [number, number, number]
  solid: boolean
  sky: boolean
  normal: readonly [number, number, number] | null
  directTarget: number | null
}>
export type MoverResult = Readonly<{
  requestId: bigint
  entity: number
  kind: 1 | 2 | 3 | 4 | 5
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
  carry: readonly [number, number, number]
}>
export type CollisionSnapshot = Readonly<{
  worldIdentity: string
  identity: bigint
  objects: number
  bytes: Uint8Array
}>
export type Command = Readonly<{
  weaponPreferences?: Readonly<{ rememberActive: boolean; rememberLast: boolean }>
  forward: number
  side: number
  up?: number
  yawDegrees: number
  pitchDegrees: number
  jump: boolean
  crouch: boolean
  speedButton?: boolean
  fire: boolean
  detonate: boolean
  reload?: boolean
  reset?: boolean
  respawn?: boolean
  dropItem?: boolean
  nextbotStop?: boolean
  selectClass?: Tf2Class | 12
  selectTeam?: Tf2Team
  selectWeapon?: Tf2Weapon
  selectLastWeapon?: boolean
  disguise?: Readonly<{ class: Tf2Class; team: Tf2Team }>
  modeRequest?: MovementMode
  activateEntity?: number
  physicsResults?: readonly ProjectilePhysicsResult[]
  bot?: BotRequest
  building?: Tf2BuildingRequest
  botConfiguration?: BotConfiguration
  objectiveConfiguration?: Readonly<{ capturesPerRound: number; returnOnTouch: boolean }>
  botControl?: Readonly<{
    action: "teleport"
    identity: number
    position: readonly [number, number, number]
    pitchDegrees: number
    yawDegrees: number
  } | {
    action: "whack"
    identity: number
  } | {
    action: "stealth-condition"
    identity: number
    enabled: boolean
  }>
}>

export type MovementSnapshot = Readonly<{
  mode: MovementMode
  crouchPhase: 0 | 1 | 2 | 3 | 4
  waterLevel: number
  waterType: number
  jumpLatched: boolean
  previousJump: boolean
  previousCrouch: boolean
  stuckOffset: number
  grounded: boolean
  supportIdentity: bigint | null
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  viewOffset: readonly [number, number, number]
  crouchFraction: number
  crouchStartFraction: number
  crouchElapsed: number
  crouchDuration: number
  fallSpeed: number
  groundNormal: readonly [number, number, number]
  surfaceFriction: number
}>
export type MovementTick = Readonly<{
  mode: number
  crouchPhase: number
  grounded: boolean
  wishDirection: readonly [number, number, number]
  wishSpeed: number
  uncappedWishSpeed: number
  wishVelocity: readonly [number, number, number]
  jumpVelocity: readonly [number, number, number]
  climbedStep: number
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
  sweepQueries: number
  pointQueries: number
  contacts: number
  events: number
  mover: null | Readonly<{
    identity: bigint
    status: 0 | 1 | 2
    displacement: readonly [number, number, number]
    supportVelocity: readonly [number, number, number]
    blocker: bigint | null
  }>
}>

export type WeaponState = Readonly<{
  weapon: Tf2Weapon
  reload: 0 | 1 | 2 | 3
  clip: number
  reserve: number
  maximumClip: number
  maximumReserve: number
  nextPrimaryTick: bigint
  reloadDueTick: bigint | null
  chargeBeginTick: bigint | null
  firstPrimaryTick: bigint
  chargedDamage: number
  prefirePlaybackRate: number | null
}>

export type FlamePoint = Readonly<{
  slot: number
  wallsHit: number
  spawnTick: bigint
  spawnTime: number
  lifetime: number
  initialPosition: readonly [number, number, number]
  previousPosition: readonly [number, number, number]
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  attackerVelocity: readonly [number, number, number]
}>
export type ShotgunPellet = Readonly<{
  index: number
  direction: readonly [number, number, number]
  damage: number
  range: number
}>
export type WeaponActivity = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11
export type ActivityEvent = Readonly<{ tick: bigint; weapon: Tf2Weapon; activity: WeaponActivity }>
export type LifecycleEvent = Readonly<{ tick: bigint; kind: 1 | 2 | 3 | 4; class: Tf2Class; team: Tf2Team }>
export type ProjectilePhysicsRequest = Readonly<{
  operation: 1 | 2 | 3 | 4
  projectile: number
  tick: bigint
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
  gravityScale: number
  friction: number
  elasticity: number
}>
export type RocketTraceRequest = Readonly<{
  projectile: number
  tick: bigint
  start: readonly [number, number, number]
  end: readonly [number, number, number]
  mask: number
}>
export type RadiusDamageRequest = Readonly<{
  projectile: number
  kind: ProjectileKind
  source: readonly [number, number, number]
  baseDamage: number
  radius: number
  selfRadius: number
  directTarget: number | null
}>
export type MoverRequest = Readonly<{
  requestId: bigint
  entity: number
  model: number | null
  start: readonly [number, number, number]
  destination: readonly [number, number, number]
  speed: number
  opening: boolean
}>
export type ContactReconcileRequest = Readonly<{
  tick: bigint
  position: readonly [number, number, number]
  hullMins: readonly [number, number, number]
  hullMaxs: readonly [number, number, number]
}>
export type MapEffect = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5
  detail: number
  team: number | null
  contact: ContactKind | null
  subject: number
  auxiliary: number | null
  values: readonly [number, number, number, number, number, number]
}>
export type RegenerateAnimationEvent = Readonly<{
  zone: number
  associatedModel: number
  openTick: bigint
  closeTick: bigint
  body: number
  openAnimation: "open" | "close"
  closeAnimation: "open" | "close"
}>
export type AuthorityBlocker = Readonly<{
  code: 1 | 2 | 3
  classification: "Missing"
  detail: string
}>

export type Projectile = Readonly<{
  airBurst: boolean
  underwaterExplosion: boolean
  trail: ProjectileTrail
  miniRocket: boolean
  practiceExplosion: boolean
  modelVisible: boolean
  weapon: Tf2Weapon
  critical: boolean
  selfBlastOnly: boolean
  identity: number
  kind: ProjectileKind
  team: Tf2Team
  ownerIdentity: number
  launcherIdentity: number
  state: ProjectileState
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  angularVelocity: readonly [number, number, number]
  contactNormal: readonly [number, number, number] | null
  ageSeconds: number
}>

export type ProjectileEventType = "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode" | "deflect"
export type ProjectileEvent = Readonly<{
  airBurst: boolean
  underwaterExplosion: boolean
  trail: ProjectileTrail
  miniRocket: boolean
  practiceExplosion: boolean
  weapon: Tf2Weapon
  critical: boolean
  selfBlastOnly: boolean
  type: ProjectileEventType
  projectile: number
  kind: ProjectileKind
  ownerIdentity: number
  launcherIdentity: number
  team: Tf2Team
  tick: bigint
  position: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  contactNormal: readonly [number, number, number] | null
  launcherPose: Readonly<{
    eyePosition: readonly [number, number, number]
    viewOrientation: readonly [number, number, number, number]
  }> | null
}>

export type ProjectileTimelineTick = Readonly<{
  tick: bigint
  projectiles: readonly Projectile[]
  events: readonly ProjectileEvent[]
}>

export type EntityTransform = Readonly<{
  identity: number
  model: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>
export type BrushModelDrawState = Readonly<{ sourceIndex:number; model:number; worldPosition:readonly[number,number,number]; worldAngles:readonly[number,number,number]; renderMode:number;color:readonly[number,number,number,number];renderFx:number;effects:number;draw:boolean;mover:null|Readonly<{kind:1|2|3;position:1|2|3|4|5;progress:number;requestId:bigint|null;opening:boolean|null}> }>
export type StudioModelDrawState = Readonly<{sourceIndex:number;worldPosition:readonly[number,number,number];worldAngles:readonly[number,number,number];draw:boolean;skin:number}>
export type EntityPresentation=Readonly<{sourceIdentity:bigint;registryIdentity:bigint;tick:bigint;entityRevision:bigint;collisionRevision:bigint;models:readonly BrushModelDrawState[];studioModels:readonly StudioModelDrawState[];studioAnimations:readonly Readonly<{sourceIndex:number;sequence:string;elapsedSeconds:number;bounds:readonly[readonly[number,number,number],readonly[number,number,number]]}>[]}>

export type EntityEvent = Readonly<{
  sequence: bigint
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  accepted: boolean
  contact: 1 | 2 | 3 | null
  entity: number
  subject: number | null
  name: string
}>

export type GameplayEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 24
  detail: number
  subject: number
  auxiliary: number
  values: readonly [number, number, number, number]
  killingWeapon?: string
}>

export type CombatDecal = Readonly<{
  identity: number
  face: number
  reference: string
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
}>

export type CaptureFlagStatus = 0 | 1 | 2
export type CaptureFlag = Readonly<{
  identity: number
  team: Tf2Team
  status: CaptureFlagStatus
  disabled: boolean
  visibleWhenDisabled: boolean
  shotClock: boolean
  allowOwnerPickup: boolean
  trailEnabled: boolean
  captured: boolean
  skin: number
  carrier: number | null
  previousCarrier: number | null
  initialCarrier: number | null
  returnDeadline: number | null
  maximumReturnSeconds: number
  ownerPickupDeadline: number | null
  configuredReturnSeconds: number
  position: readonly [number, number, number]
  home: readonly [number, number, number]
  angles: readonly [number, number, number]
  homeAngles: readonly [number, number, number]
  model: string
  icon: string
  paperEffect: string
  trailEffect: string
}>
export type CaptureZone = Readonly<{
  identity: number
  team: Tf2Team | null
  disabled: boolean
  model: number
  origin: readonly [number, number, number]
  center: readonly [number, number, number]
  capturePoint: number
}>
export type CaptureEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  detail: number
  team: 0 | Tf2Team
  flags: number
  subject: number
  player: number | null
  auxiliary: number
  value: number
}>
export type CaptureObjectives = Readonly<{
  redCaptures: number
  blueCaptures: number
  redScore: number
  blueScore: number
  captureLimit: number
  winner: Tf2Team | null
  flags: readonly CaptureFlag[]
  zones: readonly CaptureZone[]
  events: readonly CaptureEvent[]
}>

export type ControlPoint = Readonly<{
  identity: number
  skin: number
  body: number
  owner: 0 | Tf2Team
  capturingTeam: 0 | Tf2Team
  teamInZone: 0 | Tf2Team
  locked: boolean
  visible: boolean
  modelVisible: boolean
  blocked: boolean
  mayCapture: readonly [boolean, boolean]
  remaining: number
  progress: number
  unlockAt: number | null
  captureTimes: readonly [number, number]
  playerCounts: readonly [number, number]
  requiredCappers: readonly [number, number]
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
  printName: string
  icon: string
  model: string
  overlay: string
  touching: readonly number[]
}>
export type ControlPoints = Readonly<{
  customPosition: readonly [number, number]
  capLayout: string
  localPoint: number | null
  localCaptureText: string
  points: readonly ControlPoint[]
}>

export type RoundState = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10
export type RoundEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17
  detail: number
  team: 0 | Tf2Team
  flags: number
  identity: number
  value: number
}>
export type RoundTimer = Readonly<{
  identity: number
  remaining: number
  initialSeconds: number
  setupSeconds: number
  maximumSeconds: number
  paused: boolean
  showInHud: boolean
  disabled: boolean
}>
export type RoundSnapshot = Readonly<{
  fullRound: boolean
  state: RoundState
  waitingForPlayers: boolean
  waitingRemaining: number | null
  inSetup: boolean
  inOvertime: boolean
  winningTeam: Tf2Team | null
  winReason: number
  redScore: number
  blueScore: number
  roundsPlayed: number
  timer: RoundTimer | null
  kothTimers: readonly [RoundTimer, RoundTimer] | null
  events: readonly RoundEvent[]
}>

export type JumpCheckpoint = Readonly<{ zoneIdentity: number; index: number; tick: bigint }>
export type JumpRun = Readonly<{
  instance: number
  disposition: 0 | 1 | 2 | 3
  playerIdentity: number
  class: 3 | 4
  team: 2 | 3
  invalidation: 1 | 2 | 3 | 4 | null
  startTick: bigint
  endTick: bigint | null
  checkpoints: readonly JumpCheckpoint[]
}>
export type JumpEvent = Readonly<{
  sequence: bigint
  tick: bigint
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  detail: number
  runInstance: number
  zoneIdentity: number | null
  zoneIndex: number | null
}>
export type JumpResult = Readonly<{
  courseIdentity: bigint
  mapIdentity: string
  runInstance: number
  playerIdentity: number
  class: 3 | 4
  team: 2 | 3
  runKind: "map"
  zoneIndex: number
  disposition: "completed"
  startTick: bigint
  endTick: bigint
  elapsedTicks: bigint
  tickInterval: number
  checkpoints: readonly JumpCheckpoint[]
}>
export type JumpSnapshot = Readonly<{
  run: JumpRun | null
  events: readonly JumpEvent[]
  result: JumpResult | null
}>

export type BotSnapshot = Readonly<{
  conditions: readonly number[]
  overheadHeight: number
  equippedItems: readonly Tf2EquippedItem[]
  identity: number
  class: Tf2Class
  team: Tf2Team
  lifecycle: 1 | 2
  difficulty: BotDifficulty
  objective: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10
  health: number
  maximumHealth: number
  target: number | null
  area: number | null
  remainingPathAreas: number
  yawDegrees: number
  pitchDegrees: number
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  weapon: Readonly<{
    identity: Tf2Weapon
    reload: 0 | 1 | 2 | 3
    clip: number
    reserve: number
    maximumClip: number
    maximumReserve: number
    nextPrimaryTick: bigint
    nextReloadTick: bigint
  }> | null
  shots: number
  hits: number
  kills: number
  deaths: number
  captures: number
  carryingFlag: boolean
  animationRole: "PRIMARY" | "SECONDARY" | "MELEE"
  lastFireTick: bigint | null
  respawnTick: bigint | null
}>

export type MapPickup = Readonly<{
  identity: number
  kind: "health" | "ammo"
  size: "small" | "medium" | "full"
  team: 2 | 3 | null
  available: boolean
  disabled: boolean
  origin: readonly [number, number, number]
  angles: readonly [number, number, number]
  respawnTick: bigint | null
}>

export type ScoreboardPlayerSnapshot = Readonly<{
  identity: number
  name: string
  team: Tf2Team
  class: Tf2Class
  alive: boolean
  fake: boolean
  score: number
  kills: number
  assists: number
  deaths: number
  captures: number
  damage: number
}>

export type ScoreboardSnapshot = Readonly<{
  redScore: number
  blueScore: number
  redCount: number
  blueCount: number
  players: readonly ScoreboardPlayerSnapshot[]
}>

export type SpySnapshot = Readonly<{
  cloakMeter: number
  invisibility: number
  disguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | null
  desiredDisguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | null
  disguiseCompleteTime: number
  noAttackUntil: number
}>

export type ActorCloakState = Readonly<{ identity: number; localFactor: number; worldFactor: number; rawFactor: number; playerTint: readonly [number, number, number] }>

export type BuildingPlacement = Readonly<{ object: Tf2BuildingObject; position: readonly [number, number, number]; yawDegrees: number; valid: boolean }>
export type BuildingSnapshot = Readonly<{
  identity: number
  owner: number
  object: Tf2BuildingObject
  team: Tf2Team
  phase: 0 | 1 | 2 | 3
  level: 1 | 2 | 3
  health: number
  maximumHealth: number
  upgradeMetal: number
  shells: number
  maximumShells: number
  rockets: number
  maximumRockets: number
  dispenserMetal: number
  target: number | null
  position: readonly [number, number, number]
  yawDegrees: number
  construction: number
  rechargeEndTick: bigint | null
  startedTick: bigint
  timesUsed: number
}>

export type SoundscapeSelection = Readonly<{
  entity: number
  soundscape: number
  positionBits: number
  positions: readonly (readonly [number, number, number])[]
}>

export type Snapshot = Readonly<{
  decapitations: number
  revengeCrits: number
  weaponCrosshairScale: number
  soundscape: SoundscapeSelection
  equippedItems: readonly Tf2EquippedItem[]
  actorCloaks: readonly ActorCloakState[]
  tick: bigint
  class: Tf2Class
  team: Tf2Team
  weapon: Tf2Weapon | null
  playerFlags: number
  inWater: boolean
  health: number
  maximumHealth: number
  spy: SpySnapshot | null
  lifecycle: 1 | 2 | 3 | 4
  conditions: readonly [number, number, number, number, number]
  respawnTouchCount: number
  movement: MovementSnapshot
  viewAngleOffset: readonly [number, number, number]
  movementTick: MovementTick | null
  position: readonly [number, number, number]
  velocity: readonly [number, number, number]
  grounded: boolean
  crouched: boolean
  loadout: readonly WeaponState[]
  flamePoints: readonly FlamePoint[]
  shotgunPellets: readonly ShotgunPellet[]
  flameFiring: boolean
  projectiles: readonly Projectile[]
  projectileEvents: readonly ProjectileEvent[]
  projectileTimeline: readonly ProjectileTimelineTick[]
  entityTransforms: readonly EntityTransform[]
  entityEvents: readonly EntityEvent[]
  objectives: CaptureObjectives | null
  controlPoints: ControlPoints | null
  round: RoundSnapshot
  jump: JumpSnapshot | null
  events: readonly GameplayEvent[]
  combatDecals: readonly CombatDecal[]
  activities: readonly ActivityEvent[]
  lifecycleEvents: readonly LifecycleEvent[]
  physicsRequests: readonly ProjectilePhysicsRequest[]
  rocketTraceRequests: readonly RocketTraceRequest[]
  radiusDamageRequests: readonly RadiusDamageRequest[]
  moverRequests: readonly MoverRequest[]
  contactReconcileRequests: readonly ContactReconcileRequest[]
  mapEffects: readonly MapEffect[]
  regenerateAnimationEvents: readonly RegenerateAnimationEvent[]
  randomState: Tf2RandomState
  randomDraws: readonly RandomDraw[]
  audioEvents: readonly AudioEvent[]
  rocketTraceResults: readonly RocketTraceResult[]
  moverResults: readonly MoverResult[]
  collisionSnapshot: CollisionSnapshot
  entityPresentation: EntityPresentation
  authorityBlockers: readonly AuthorityBlocker[]
  bots: readonly BotSnapshot[]
  pickups: readonly MapPickup[]
  buildings: readonly BuildingSnapshot[]
  placement: BuildingPlacement | null
  metal: number
  scoreboard: ScoreboardSnapshot
  medigunCharge: number
  medigunTarget: number | null
  medigunReleasing: boolean
}>

export class Tf2CodecError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2CodecError"
  }
}

export type JumpCourseZone = Readonly<{
  identity: number
  triggerEntity: number
  kind: "start" | "checkpoint" | "end"
  index: number
}>

export function encodeJumpCourse(identity: bigint, mapSha256: string, zones: readonly JumpCourseZone[]): Uint8Array {
  if (identity <= 0n || identity > 0xffff_ffff_ffff_ffffn || !HASH.test(mapSha256) || zones.length > 4_096) {
    throw new Tf2CodecError("Jump course identity or bound is invalid")
  }
  const identities = new Set<number>()
  const triggers = new Set<number>()
  const checkpoints: number[] = []
  let starts = 0
  let ends = 0
  for (const zone of zones) {
    if (
      !canonicalIdentity(zone.identity) ||
      zone.identity === 0 ||
      identities.has(zone.identity) ||
      !canonicalIdentity(zone.triggerEntity) ||
      triggers.has(zone.triggerEntity) ||
      !Number.isSafeInteger(zone.index) ||
      zone.index < 1 ||
      zone.index > 0xffff_ffff
    )
      throw new Tf2CodecError("Jump zone identity is invalid")
    identities.add(zone.identity)
    triggers.add(zone.triggerEntity)
    if (zone.kind === "start" && zone.index === 1) starts += 1
    else if (zone.kind === "end" && zone.index === 1) ends += 1
    else if (zone.kind === "checkpoint") checkpoints.push(zone.index)
    else throw new Tf2CodecError("Jump zone kind or index is invalid")
  }
  checkpoints.sort((left, right) => left - right)
  if (starts !== 1 || ends !== 1 || checkpoints.some((value, index) => value !== index + 1)) {
    throw new Tf2CodecError("Jump linear topology is invalid")
  }
  const bytes = new Uint8Array(52 + zones.length * 16)
  const view = new DataView(bytes.buffer)
  bytes.set([0x50, 0x4a, 0x4d, 0x50])
  view.setUint32(4, 1, true)
  view.setBigUint64(8, identity, true)
  for (let index = 0; index < 32; index += 1) {
    bytes[16 + index] = Number.parseInt(mapSha256.slice(index * 2, index * 2 + 2), 16)
  }
  view.setUint32(48, zones.length, true)
  zones.forEach((zone, index) => {
    const at = 52 + index * 16
    view.setUint32(at, zone.identity, true)
    view.setUint32(at + 4, zone.triggerEntity, true)
    bytes[at + 8] = zone.kind === "start" ? 1 : zone.kind === "checkpoint" ? 2 : 3
    view.setUint32(at + 12, zone.index, true)
  })
  return bytes
}

function canonicalIdentity(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0 && value <= 0xffff_ffff
}

export function encodeCommand(command: Command): ArrayBuffer {
  if (command.selectLastWeapon !== undefined && typeof command.selectLastWeapon !== "boolean"
    || command.selectLastWeapon && command.selectWeapon !== undefined) throw new Tf2CodecError("command weapon selectors conflict")
  const scalars = [command.forward, command.side, command.up ?? 0, command.yawDegrees, command.pitchDegrees]
  if (!scalars.every(Number.isFinite)) throw new Tf2CodecError("command contains a non-finite scalar")
  if (command.selectClass !== undefined && (!Number.isInteger(command.selectClass) || command.selectClass < 1 || (command.selectClass > 9 && command.selectClass !== 12))) {
    throw new Tf2CodecError("command class selector is invalid")
  }
  if (command.selectTeam !== undefined && command.selectTeam !== 2 && command.selectTeam !== 3) {
    throw new Tf2CodecError("command team selector is invalid")
  }

  if (command.selectWeapon !== undefined && !isTf2Weapon(command.selectWeapon)) {

    throw new Tf2CodecError("command weapon selector is invalid")
  }
  if (command.disguise !== undefined && (!Number.isInteger(command.disguise.class) || command.disguise.class < 1 || command.disguise.class > 9 || (command.disguise.team !== 2 && command.disguise.team !== 3))) {
    throw new Tf2CodecError("command disguise selector is invalid")
  }
  if (command.modeRequest !== undefined && command.modeRequest !== 0 && command.modeRequest !== 1) {
    throw new Tf2CodecError("command mode request is invalid")
  }
  if (command.activateEntity !== undefined && !canonicalIdentity(command.activateEntity)) {
    throw new Tf2CodecError("command entity identity is invalid")
  }
  const physics = command.physicsResults ?? []
  if (physics.length > 64) {
    throw new Tf2CodecError("command external-result count exceeds its bound")
  }
  const uint64 = (value: bigint): boolean => value >= 0n && value <= 0xffff_ffff_ffff_ffffn
  for (const result of physics) {
    if (!canonicalIdentity(result.projectile) || !uint64(result.tick) ||
      !finite([...result.position, ...result.velocity, ...result.angularVelocity]) || !normalized(result.orientation) ||
      (result.contact !== null && (!([1, 2, 3] as number[]).includes(result.contact.kind) || !normalized(result.contact.normal)))) {
      throw new Tf2CodecError("command projectile Physics result is invalid")
    }
  }
  const length = 84 + physics.length * 80
  const bytes = new ArrayBuffer(length)
  const data = new Uint8Array(bytes)
  const view = new DataView(bytes)
  data.set([0x50, 0x43, 0x4d, 0x44])
  view.setUint32(4, 9, true)
  scalars.forEach((value, index) => view.setFloat32(8 + index * 4, value, true))
  let buildingFlags = 0
  if (command.building) {
    const request = command.building
    if (request.action === "build" || request.action === "destroy") {
      if (![0, 1, 2].includes(request.object.kind) || ![0, 1].includes(request.object.mode)
        || (request.object.kind !== 1 && request.object.mode !== 0)) throw new Tf2CodecError("command building object is invalid")
      buildingFlags = ((request.action === "build" ? 1 : 2) << 16) | (request.object.kind << 19) | (request.object.mode << 21)
    } else if (request.action === "rotate" || request.action === "cancel") {
      buildingFlags = (request.action === "rotate" ? 3 : 4) << 16
    } else if (request.action === "hurt") {
      if (!Number.isSafeInteger(request.amount) || request.amount < 0 || request.amount > 0xffff) throw new Tf2CodecError("command building damage is invalid")
      buildingFlags = 0x8000 | (request.amount << 16)
    }
  }
  const flags =
    buildingFlags |
    Number(command.jump) |
    (Number(command.crouch) << 1) |
    (Number(command.speedButton ?? false) << 2) |
    (Number(command.fire) << 3) |
    (Number(command.detonate) << 4) |
    (Number(command.reload ?? false) << 5) |
    (Number(command.reset ?? false) << 6) |
    (Number(command.respawn ?? false) << 7) |
    (Number(command.dropItem ?? false) << 8) |
    ((command.disguise?.class ?? 0) << 9) |
    ((command.disguise?.team ?? 0) << 13)
  view.setUint32(28, flags, true)
  const mode = command.modeRequest === undefined ? 0 : command.modeRequest + 1
  view.setUint32(
    32,
    (command.selectClass ?? 0) | ((command.selectLastWeapon ? 255 : command.selectWeapon ?? 0) << 8) | ((command.selectTeam ?? 0) << 16) | (mode << 24),
    true,
  )
  view.setUint32(36, command.activateEntity ?? 0xffff_ffff, true)
  view.setUint16(40, physics.length, true)
  let packedBot = 0
  if (command.bot) {
    if (command.bot.action === "add") {
      const { count, class: identity, team, difficulty } = command.bot
      if (!Number.isSafeInteger(count) || count < 1 || count > 31 || (identity !== undefined && (!Number.isSafeInteger(identity) || identity < 1 || identity > 9))
        || (team !== undefined && team !== 2 && team !== 3) || !Number.isSafeInteger(difficulty) || difficulty < 0 || difficulty > 3) {
        throw new Tf2CodecError("command bot addition is invalid")
      }
      packedBot = 1 | (count << 2) | ((identity ?? 0) << 7) | ((team ?? 0) << 11) | (difficulty << 13)
    } else if (command.bot.action === "kick-all") {
      packedBot = 2
    } else if (command.bot.action === "kick-team" && (command.bot.team === 2 || command.bot.team === 3)) {
      packedBot = 3 | (command.bot.team << 11)
    } else {
      throw new Tf2CodecError("command bot operation is invalid")
    }
  }
  view.setUint16(42, packedBot | (Number(command.nextbotStop ?? false) << 15), true)
  let packedConfiguration = 0
  if (command.botConfiguration) {
    const { quota, maximumPlayers, mode, difficulty, joinAfterPlayer, autoVacate, offlinePractice } = command.botConfiguration
    if (!Number.isSafeInteger(quota) || quota < 0 || quota > 31
      || !Number.isSafeInteger(maximumPlayers) || maximumPlayers < 1 || maximumPlayers > 32
      || !["normal", "fill", "match"].includes(mode)
      || !Number.isSafeInteger(difficulty) || difficulty < 0 || difficulty > 3
      || typeof joinAfterPlayer !== "boolean" || typeof autoVacate !== "boolean" || typeof offlinePractice !== "boolean") {
      throw new Tf2CodecError("command bot configuration is invalid")
    }
    packedConfiguration = (0x8000_0000 | quota | (maximumPlayers << 6)
      | (difficulty << 12) | (["normal", "fill", "match"].indexOf(mode) << 14)
      | (Number(joinAfterPlayer) << 16) | (Number(autoVacate) << 17)
      | (Number(offlinePractice) << 18)) >>> 0
  }
  view.setUint32(44, packedConfiguration, true)
  view.setUint32(48, length, true)
  if (command.weaponPreferences) {
    if (typeof command.weaponPreferences.rememberActive !== "boolean" || typeof command.weaponPreferences.rememberLast !== "boolean") throw new Tf2CodecError("invalid weapon preferences")
    view.setUint8(57, 0x80 | Number(command.weaponPreferences.rememberActive) | Number(command.weaponPreferences.rememberLast) << 1)
  }
  let packedObjectives = 0
  if (command.objectiveConfiguration) {
    const { capturesPerRound, returnOnTouch } = command.objectiveConfiguration
    if (!Number.isSafeInteger(capturesPerRound) || capturesPerRound < 0 || capturesPerRound > 0xffff
      || typeof returnOnTouch !== "boolean") throw new Tf2CodecError("command capture objective configuration is invalid")
    packedObjectives = (0x8000_0000 | capturesPerRound | (Number(returnOnTouch) << 16)) >>> 0
  }
  view.setUint32(52, packedObjectives, true)
  if (command.botControl) {
    const control = command.botControl
    if (!["teleport", "whack", "stealth-condition"].includes(control.action) || (control.action === "stealth-condition" && typeof control.enabled !== "boolean")) throw new Tf2CodecError("command bot control action is invalid")
    if (!canonicalIdentity(control.identity) || control.identity <= 1) {
      throw new Tf2CodecError("command bot control identity is invalid")
    }
    data[56] = control.action === "teleport" ? 1 : control.action === "whack" ? 2 : control.enabled ? 3 : 4
    view.setUint32(60, control.identity, true)
    if (control.action === "teleport") {
      const values = [...control.position, control.pitchDegrees, control.yawDegrees]
      if (!values.every(Number.isFinite)) throw new Tf2CodecError("command bot teleport contains a non-finite scalar")
      values.forEach((value, index) => view.setFloat32(64 + index * 4, value, true))
    }
  }
  let at = 84
  const writeVector = (value: readonly number[]): void => {
    value.forEach((scalar, index) => view.setFloat32(at + index * 4, scalar, true))
    at += value.length * 4
  }
  for (const result of physics) {
    view.setUint32(at, result.projectile, true)
    view.setBigUint64(at + 4, result.tick, true)
    data[at + 12] = Number(result.motionEnabled)
    data[at + 13] = result.contact?.kind ?? 0
    at += 16
    writeVector(result.position)
    writeVector(result.velocity)
    writeVector(result.orientation)
    writeVector(result.angularVelocity)
    writeVector(result.contact?.normal ?? [0, 0, 0])
  }
  if (at !== length) throw new Tf2CodecError("command encoding length differs")
  return bytes
}

function vector(view: DataView, offset: number): readonly [number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
  ]) as readonly [number, number, number]
}

function quaternion(view: DataView, offset: number): readonly [number, number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
    view.getFloat32(offset + 12, true),
  ]) as readonly [number, number, number, number]
}

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

function normalized(values: readonly number[]): boolean {
  if (!finite(values)) return false
  const length = Math.hypot(...values)
  return Math.abs(length - 1) <= 0.001
}

function count(value: number, label: string): number {
  if (value > MAX_RECORDS) throw new Tf2CodecError(`${label} count exceeds its bound`)
  return value
}

function movementSnapshot(bytes: ArrayBuffer, offset: number, length: number, waterType: number): MovementSnapshot {
  if (length !== MOVEMENT_BYTES) throw new Tf2CodecError("Movement snapshot length is invalid")
  const data = new Uint8Array(bytes, offset, length)
  const view = new DataView(bytes, offset, length)
  if (
    data[0] !== 0x50 ||
    data[1] !== 0x4d ||
    data[2] !== 0x4f ||
    data[3] !== 0x56 ||
    view.getUint32(4, true) !== 1 ||
    data[8]! > 1 ||
    data[9]! > 4 ||
    data[11]! > 1 ||
    data[12]! > 1 ||
    data[13]! > 1 ||
    data[15]! > 1
  )
    throw new Tf2CodecError("Movement snapshot identity or flags are invalid")
  const position = vector(view, 24)
  const velocity = vector(view, 36)
  const viewOffset = vector(view, 48)
  const groundNormal = vector(view, 80)
  const scalars = [
    ...position,
    ...velocity,
    ...viewOffset,
    view.getFloat32(60, true),
    view.getFloat32(64, true),
    view.getFloat32(68, true),
    view.getFloat32(72, true),
    view.getFloat32(76, true),
    ...groundNormal,
    view.getFloat32(92, true),
  ]
  if (!finite(scalars)) throw new Tf2CodecError("Movement snapshot contains a non-finite scalar")
  const support = view.getBigUint64(16, true)
  return Object.freeze({
    mode: data[8] as MovementMode,
    crouchPhase: data[9] as MovementSnapshot["crouchPhase"],
    waterLevel: data[10]!,
    waterType,
    jumpLatched: data[11] === 1,
    previousJump: data[12] === 1,
    previousCrouch: data[13] === 1,
    stuckOffset: data[14]!,
    grounded: data[15] === 1,
    supportIdentity: support === 0xffff_ffff_ffff_ffffn ? null : support,
    position,
    velocity,
    viewOffset,
    crouchFraction: scalars[9]!,
    crouchStartFraction: scalars[10]!,
    crouchElapsed: scalars[11]!,
    crouchDuration: scalars[12]!,
    fallSpeed: scalars[13]!,
    groundNormal,
    surfaceFriction: scalars[17]!,
  })
}

function decodeJump(bytes: ArrayBuffer, offset: number, length: number): JumpSnapshot | null {
  if (length === 0) return null
  if (length < 16 || offset + length > bytes.byteLength) throw new Tf2CodecError("Jump section is truncated")
  const data = new Uint8Array(bytes, offset, length)
  const view = new DataView(bytes, offset, length)
  if (data[0] !== 0x50 || data[1] !== 0x4a || data[2] !== 0x4f || data[3] !== 0x46 || view.getUint32(4, true) !== 1)
    throw new Tf2CodecError("Jump section identity is invalid")
  let at = 8
  const ensure = (size: number): void => {
    if (at + size > length) throw new Tf2CodecError("Jump section record exceeds its bytes")
  }
  ensure(4)
  const hasRun = data[at]
  if (hasRun === undefined || hasRun > 1) throw new Tf2CodecError("Jump run flag is invalid")
  let run: JumpRun | null = null
  if (hasRun === 0) at += 4
  else {
    ensure(36)
    const disposition = data[at + 1]
    const tf2Class = data[at + 2]
    const team = data[at + 3]
    const invalidation = data[at + 12]
    if (
      disposition === undefined ||
      disposition > 3 ||
      (tf2Class !== 3 && tf2Class !== 4) ||
      (team !== 2 && team !== 3) ||
      invalidation === undefined ||
      invalidation > 4 ||
      data[at + 13] !== 0 ||
      data[at + 14] !== 0 ||
      data[at + 15] !== 0
    )
      throw new Tf2CodecError("Jump run record is invalid")
    const checkpointCount = count(view.getUint32(at + 32, true), "Jump checkpoint")
    ensure(36 + checkpointCount * 16)
    const checkpoints: JumpCheckpoint[] = []
    for (let index = 0; index < checkpointCount; index += 1) {
      const item = at + 36 + index * 16
      checkpoints.push(
        Object.freeze({
          zoneIdentity: view.getUint32(item, true),
          index: view.getUint32(item + 4, true),
          tick: view.getBigUint64(item + 8, true),
        }),
      )
    }
    const end = view.getBigUint64(at + 24, true)
    run = Object.freeze({
      instance: view.getUint32(at + 4, true),
      disposition: disposition as JumpRun["disposition"],
      playerIdentity: view.getUint32(at + 8, true),
      class: tf2Class,
      team,
      invalidation: invalidation === 0 ? null : (invalidation as 1 | 2 | 3 | 4),
      startTick: view.getBigUint64(at + 16, true),
      endTick: end === 0xffff_ffff_ffff_ffffn ? null : end,
      checkpoints: Object.freeze(checkpoints),
    })
    at += 36 + checkpointCount * 16
  }
  ensure(4)
  const eventCount = count(view.getUint32(at, true), "Jump event")
  at += 4
  ensure(eventCount * 32)
  const events: JumpEvent[] = []
  for (let index = 0; index < eventCount; index += 1) {
    const item = at + index * 32
    const kind = data[item + 16]
    const detail = data[item + 17]
    if (kind === undefined || kind < 1 || kind > 8 || data[item + 18] !== 0 || data[item + 19] !== 0) {
      throw new Tf2CodecError("Jump event record is invalid")
    }
    const zoneIdentity = view.getUint32(item + 24, true)
    const zoneIndex = view.getUint32(item + 28, true)
    events.push(
      Object.freeze({
        sequence: view.getBigUint64(item, true),
        tick: view.getBigUint64(item + 8, true),
        kind: kind as JumpEvent["kind"],
        detail: detail!,
        runInstance: view.getUint32(item + 20, true),
        zoneIdentity: zoneIdentity === 0xffff_ffff ? null : zoneIdentity,
        zoneIndex: zoneIndex === 0xffff_ffff ? null : zoneIndex,
      }),
    )
  }
  at += eventCount * 32
  ensure(4)
  const hasResult = data[at]
  if (hasResult === undefined || hasResult > 1) throw new Tf2CodecError("Jump result flag is invalid")
  let result: JumpResult | null = null
  if (hasResult === 0) at += 4
  else {
    ensure(92)
    const tf2Class = data[at + 1]
    const team = data[at + 2]
    if (
      (tf2Class !== 3 && tf2Class !== 4) ||
      (team !== 2 && team !== 3) ||
      data[at + 3] !== 1 ||
      data[at + 56] !== 2 ||
      data[at + 57] !== 0 ||
      data[at + 58] !== 0 ||
      data[at + 59] !== 0
    ) {
      throw new Tf2CodecError("Jump result selection is invalid")
    }
    const checkpointCount = count(view.getUint32(at + 88, true), "Jump result checkpoint")
    ensure(92 + checkpointCount * 16)
    const checkpoints: JumpCheckpoint[] = []
    for (let index = 0; index < checkpointCount; index += 1) {
      const item = at + 92 + index * 16
      checkpoints.push(
        Object.freeze({
          zoneIdentity: view.getUint32(item, true),
          index: view.getUint32(item + 4, true),
          tick: view.getBigUint64(item + 8, true),
        }),
      )
    }
    const map = data.slice(at + 12, at + 44)
    result = Object.freeze({
      courseIdentity: view.getBigUint64(at + 4, true),
      mapIdentity: Array.from(map, (value) => value.toString(16).padStart(2, "0")).join(""),
      runInstance: view.getUint32(at + 44, true),
      playerIdentity: view.getUint32(at + 48, true),
      runKind: "map",
      zoneIndex: view.getUint32(at + 52, true),
      disposition: "completed",
      class: tf2Class,
      team,
      startTick: view.getBigUint64(at + 60, true),
      endTick: view.getBigUint64(at + 68, true),
      elapsedTicks: view.getBigUint64(at + 76, true),
      tickInterval: view.getFloat32(at + 84, true),
      checkpoints: Object.freeze(checkpoints),
    })
    if (!Number.isFinite(result.tickInterval) || result.tickInterval <= 0) {
      throw new Tf2CodecError("Jump result tick interval is invalid")
    }
    at += 92 + checkpointCount * 16
  }
  if (at !== length) throw new Tf2CodecError("Jump section has trailing bytes")
  return Object.freeze({ run, events: Object.freeze(events), result })
}

function decodeMovementTick(bytes: ArrayBuffer, offset: number, length: number): MovementTick | null {
  if (length < 12 || length > 144) throw new Tf2CodecError("Movement tick section length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (data[0] !== 0x50 || data[1] !== 0x4d || data[2] !== 0x54 || data[3] !== 0x4b || view.getUint32(4, true) !== 1)
    throw new Tf2CodecError("Movement tick identity is invalid")
  const present = data[8]
  if (present === 0) {
    if (length !== 12 || data[9] !== 0 || data[10] !== 0 || data[11] !== 0) throw new Tf2CodecError("Movement tick absent record is invalid")
    return null
  }
  if (present !== 1 || data[9]! > 7 || data[10]! > 4 || data[11]! > 1 || length < 104) throw new Tf2CodecError("Movement tick header is invalid")
  let at = 12
  const vec = () => { const result = vector(view, at); at += 12; return result }
  const scalar = () => { const value = view.getFloat32(at, true); at += 4; if (!Number.isFinite(value)) throw new Tf2CodecError("Movement tick scalar is invalid"); return value }
  const wishDirection = vec(), wishSpeed = scalar(), uncappedWishSpeed = scalar(), wishVelocity = vec(), jumpVelocity = vec(),
    climbedStep = scalar(), hullMins = vec(), hullMaxs = vec()
  const sweepQueries = view.getUint32(at, true), pointQueries = view.getUint32(at + 4, true), contacts = view.getUint32(at + 8, true), events = view.getUint32(at + 12, true)
  at += 16
  const hasMover = data[at]
  if (hasMover === 0) {
    if (data[at + 1] !== 0 || data[at + 2] !== 0 || data[at + 3] !== 0 || at + 4 !== length) throw new Tf2CodecError("Movement tick mover absence is invalid")
    return Object.freeze({ mode: data[9]!, crouchPhase: data[10]!, grounded: data[11] === 1, wishDirection, wishSpeed, uncappedWishSpeed, wishVelocity, jumpVelocity, climbedStep, hullMins, hullMaxs, sweepQueries, pointQueries, contacts, events, mover: null })
  }
  const status = data[at + 1]
  if (hasMover !== 1 || status === undefined || status > 2 || data[at + 2] !== 0 || data[at + 3] !== 0 || at + 44 !== length) throw new Tf2CodecError("Movement tick mover record is invalid")
  at += 4
  const identity = view.getBigUint64(at, true); at += 8
  const displacement = vec(), supportVelocity = vec(), blockerValue = view.getBigUint64(at, true)
  return Object.freeze({
    mode: data[9]!, crouchPhase: data[10]!, grounded: data[11] === 1, wishDirection, wishSpeed, uncappedWishSpeed,
    wishVelocity, jumpVelocity, climbedStep, hullMins, hullMaxs, sweepQueries, pointQueries, contacts, events,
    mover: Object.freeze({ identity, status: status as 0 | 1 | 2, displacement, supportVelocity, blocker: blockerValue === 0xffff_ffff_ffff_ffffn ? null : blockerValue }),
  })
}
function decodeEntityPresentation(bytes: ArrayBuffer, offset: number, length: number, ranges?: SnapshotRanges, snapshotOffset = 0): EntityPresentation {
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (length < 56 || new TextDecoder().decode(data.subarray(0, 4)) !== "PEBP" || view.getUint32(4, true) !== 3)
    throw new Tf2CodecError("Entity presentation identity is invalid")
  const count = view.getUint32(48, true)
  const studioOffset = 52 + count * 128
  if (studioOffset + 4 > length) throw new Tf2CodecError("Entity presentation records do not frame bytes")
  const models: BrushModelDrawState[] = []
  let prior = -1
  for (let i = 0; i < count; i++) {
    const at = 52 + i * 128
    const decode = (): BrushModelDrawState => {
      const source = view.getUint32(at + 8, true), model = view.getUint32(at + 12, true)
      const worldPosition = vector(view, at + 40), worldAngles = vector(view, at + 52)
      const renderMode = data[at + 65]!, renderFx = data[at + 66]!, draw = data[at + 67]!
      const kind = data[at + 82]!, position = data[at + 83]!, progress = view.getFloat32(at + 84, true)
      const request = view.getBigUint64(at + 88, true), opening = data[at + 120]!
      if (model === 0 || draw > 1 || kind > 3 || position > 5 || opening > 2 || !finite([...worldPosition, ...worldAngles, progress]))
        throw new Tf2CodecError("Entity presentation record is invalid")
      return Object.freeze({ sourceIndex: source, model, worldPosition, worldAngles, renderMode,
        color: Object.freeze([data[at + 76]!, data[at + 77]!, data[at + 78]!, data[at + 79]!]) as readonly [number, number, number, number],
        renderFx, effects: view.getUint16(at + 80, true), draw: draw === 1,
        mover: kind === 0 ? null : Object.freeze({ kind: kind as 1 | 2 | 3, position: position as 1 | 2 | 3 | 4 | 5,
          progress, requestId: request === 0xffff_ffff_ffff_ffffn ? null : request, opening: opening === 0 ? null : opening === 1 }) })
    }
    const model = ranges ? ranges.read("brush", i, snapshotOffset + at, 128, decode) : decode()
    if (model.sourceIndex <= prior) throw new Tf2CodecError("Entity presentation record is invalid")
    prior = model.sourceIndex
    models.push(model)
  }
  const studioCount = view.getUint32(studioOffset, true)
  if (studioCount > 65536 || studioOffset + 8 + studioCount * 32 > length) throw new Tf2CodecError("Studio presentation records do not frame bytes")
  const studioModels: StudioModelDrawState[] = []
  prior = -1
  for (let i = 0; i < studioCount; i++) {
    const at = studioOffset + 4 + i * 32
    const decode = (): StudioModelDrawState => {
      const sourceIndex = view.getUint32(at, true), worldPosition = vector(view, at + 4), worldAngles = vector(view, at + 16), draw = view.getUint32(at + 28, true)
      if ((draw >>> 1) > 65535 || !finite([...worldPosition, ...worldAngles])) throw new Tf2CodecError("Studio presentation record is invalid")
      return Object.freeze({ sourceIndex, worldPosition, worldAngles, draw: (draw & 1) !== 0, skin: draw >>> 1 })
    }
    const model = ranges ? ranges.read("studio", i, snapshotOffset + at, 32, decode) : decode()
    if (model.sourceIndex <= prior) throw new Tf2CodecError("Studio presentation source order is invalid")
    prior = model.sourceIndex
    studioModels.push(model)
  }
  let at = studioOffset + 4 + studioCount * 32
  const animationCount = view.getUint32(at, true); at += 4
  if (animationCount > studioCount) throw new Tf2CodecError("Studio animation count is invalid")
  const studioAnimations: EntityPresentation["studioAnimations"][number][] = []
  for (let i = 0; i < animationCount; i++) {
    if (at + 36 > length) throw new Tf2CodecError("Studio animation is truncated")
    const sourceIndex = view.getUint32(at, true), elapsedSeconds = view.getFloat32(at + 4, true), size = view.getUint32(at + 8, true)
    const minimum = vector(view, at + 12), maximum = vector(view, at + 24)
    at += 36
    if (!finite([elapsedSeconds, ...minimum, ...maximum]) || minimum.some((value, axis) => value > maximum[axis]!) || elapsedSeconds < 0 || size < 1 || size > 2048 || at + size > length || !studioModels.some(model => model.sourceIndex === sourceIndex) || studioAnimations.some(animation => animation.sourceIndex === sourceIndex)) throw new Tf2CodecError("Studio animation is invalid")
    const sequence = new TextDecoder("utf-8", { fatal: true }).decode(data.subarray(at, at + size)); at += size
    studioAnimations.push(Object.freeze({ sourceIndex, sequence, elapsedSeconds, bounds: Object.freeze([minimum, maximum]) as EntityPresentation["studioAnimations"][number]["bounds"] }))
  }
  if (at !== length) throw new Tf2CodecError("Studio presentation has trailing bytes")
  return Object.freeze({ sourceIdentity: view.getBigUint64(8, true), registryIdentity: view.getBigUint64(16, true),
    tick: view.getBigUint64(24, true), entityRevision: view.getBigUint64(32, true), collisionRevision: view.getBigUint64(40, true), models: ranges ? ranges.array("brushes", models) : Object.freeze(models), studioModels: ranges ? ranges.array("studios", studioModels) : Object.freeze(studioModels), studioAnimations: Object.freeze(studioAnimations) })
}

function decodeControlPoints(buffer: ArrayBuffer, offset: number, available: number): Readonly<{ points: ControlPoints | null; length: number }> {
  if (available < 12) throw new Tf2CodecError("Control point header is truncated")
  const view = new DataView(buffer, offset, available), data = new Uint8Array(buffer, offset, available)
  const length = view.getUint32(4, true), count = view.getUint32(8, true)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "PCPN" || length < 12 || length > available || count > 8) throw new Tf2CodecError("Control point header is invalid")
  if (count === 0) {
    if (length !== 12) throw new Tf2CodecError("Absent control point section has payload")
    return Object.freeze({ points: null, length })
  }
  let at = 12
  const require = (size: number): void => { if (at + size > length) throw new Tf2CodecError("Control point record is truncated") }
  const text = (): string => {
    require(2)
    const size = view.getUint16(at, true); at += 2; require(size)
    let value: string
    try { value = new TextDecoder("utf-8", { fatal: true }).decode(data.subarray(at, at + size)) }
    catch { throw new Tf2CodecError("Control point string is not UTF-8") }
    at += size; return value
  }
  require(8)
  const customPosition = Object.freeze([view.getFloat32(at, true), view.getFloat32(at + 4, true)] as const); at += 8
  if (!finite(customPosition)) throw new Tf2CodecError("Control point layout is invalid")
  const capLayout = text(), points: ControlPoint[] = []
  require(4)
  const localPointIndex = view.getInt32(at, true); at += 4
  if (localPointIndex < -1 || localPointIndex >= count) throw new Tf2CodecError("Control point local contact is invalid")
  const localCaptureText = text()
  for (let index = 0; index < count; index++) {
    require(72)
    const identity = view.getUint32(at, true), owner = data[at + 4]!, capturingTeam = data[at + 5]!, teamInZone = data[at + 6]!, flags = data[at + 7]!
    const remaining = view.getFloat32(at + 8, true), progress = view.getFloat32(at + 12, true), unlock = view.getFloat32(at + 16, true)
    const captureTimes = Object.freeze([view.getFloat32(at + 20, true), view.getFloat32(at + 24, true)] as const)
    const playerCounts = Object.freeze([view.getInt32(at + 28, true), view.getInt32(at + 32, true)] as const)
    const requiredCappers = Object.freeze([view.getInt32(at + 36, true), view.getInt32(at + 40, true)] as const)
    const position = vector(view, at + 44), angles = vector(view, at + 56)
    if (![owner, capturingTeam, teamInZone].every(team => team === 0 || team === 2 || team === 3) || flags >> 6 > 2
      || !finite([remaining, progress, unlock, ...captureTimes, ...position, ...angles]) || unlock < -1) throw new Tf2CodecError("Control point state is invalid")
    const body = view.getInt32(at + 68, true)
    if (body < 0) throw new Tf2CodecError("Control point body is invalid")
    at += 72
    const printName = text(), icon = text(), model = text(), overlay = text()
    require(4)
    const touchingCount = view.getUint32(at, true); at += 4
    if (touchingCount > 32) throw new Tf2CodecError("Control point contact limit exceeded")
    require(touchingCount * 4)
    const touching = Array.from({ length: touchingCount }, (_, i) => view.getUint32(at + i * 4, true)); at += touchingCount * 4
    points.push(Object.freeze({ identity, skin: flags >> 6, body, owner: owner as 0 | Tf2Team, capturingTeam: capturingTeam as 0 | Tf2Team, teamInZone: teamInZone as 0 | Tf2Team,
      locked: (flags & 1) !== 0, visible: (flags & 2) !== 0, modelVisible: (flags & 4) !== 0, blocked: (flags & 8) !== 0,
      mayCapture: Object.freeze([(flags & 16) !== 0, (flags & 32) !== 0] as const), remaining, progress, unlockAt: unlock === -1 ? null : unlock,
      captureTimes, playerCounts, requiredCappers, position, angles, printName, icon, model, overlay, touching: Object.freeze(touching) }))
  }
  if (at !== length) throw new Tf2CodecError("Control point section has trailing bytes")
  return Object.freeze({ points: Object.freeze({ customPosition, capLayout, localPoint: localPointIndex === -1 ? null : localPointIndex, localCaptureText, points: Object.freeze(points) }), length })
}

function decodeObjectives(buffer: ArrayBuffer, offset: number, length: number): Readonly<{ objectives: CaptureObjectives | null; length: number }> {
  if (length < 12) throw new Tf2CodecError("Capture objective section is truncated")
  const data = new Uint8Array(buffer, offset, length), view = new DataView(buffer, offset, length)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "PCTF" || view.getUint32(4, true) !== 1
    || (data[8] !== 0 && data[8] !== 1) || data[9] !== 0 || data[10] !== 0 || data[11] !== 0) {
    throw new Tf2CodecError("Capture objective section identity is invalid")
  }
  if (data[8] === 0) return Object.freeze({ objectives: null, length: 12 })
  if (length < 36) throw new Tf2CodecError("Capture objective scores are truncated")
  const winner = data[22]
  if ((winner !== 0 && winner !== 2 && winner !== 3) || data[23] !== 0) {
    throw new Tf2CodecError("Capture objective winner is invalid")
  }
  const flagCount = count(view.getUint32(24, true), "capture flag")
  const zoneCount = count(view.getUint32(28, true), "capture zone")
  const eventCount = count(view.getUint32(32, true), "capture objective event")
  if (flagCount > 64 || zoneCount > 256 || eventCount > 4096) throw new Tf2CodecError("Capture objective count exceeds its bound")
  let at = 36
  const require = (bytes: number): void => {
    if (at + bytes > length) throw new Tf2CodecError("Capture objective record exceeds section bytes")
  }
  const identity = (value: number): number | null => value === 0xffff_ffff ? null : value
  const textDecoder = new TextDecoder("utf-8", { fatal: true })
  const text = (): string => {
    require(2)
    const size = view.getUint16(at, true)
    at += 2
    if (size === 0 || size > 2047) throw new Tf2CodecError("Capture objective string length is invalid")
    require(size)
    let value: string
    try { value = textDecoder.decode(data.subarray(at, at + size)) }
    catch { throw new Tf2CodecError("Capture objective string is not UTF-8") }
    at += size
    return value
  }
  const flags: CaptureFlag[] = []
  let previousFlag = -1
  for (let index = 0; index < flagCount; index += 1) {
    require(84)
    const flagIdentity = view.getUint32(at, true), team = data[at + 4], status = data[at + 5], bits = data[at + 6], skin = data[at + 7]
    const returnValue = view.getFloat32(at + 20, true), maximumReturnSeconds = view.getFloat32(at + 24, true)
    const ownerValue = view.getFloat32(at + 28, true), configuredReturnSeconds = view.getUint16(at + 32, true)
    const position = vector(view, at + 36), home = vector(view, at + 48), angles = vector(view, at + 60), homeAngles = vector(view, at + 72)
    if (flagIdentity <= previousFlag || (team !== 2 && team !== 3) || status === undefined || status > 2 || bits === undefined || bits > 0x3f
      || skin === undefined || skin > 4 || view.getUint16(at + 34, true) !== 0
      || !finite([returnValue, maximumReturnSeconds, ownerValue, ...position, ...home, ...angles, ...homeAngles])
      || returnValue < -1 || ownerValue < -1 || maximumReturnSeconds < 0) {
      throw new Tf2CodecError("Capture flag record is invalid")
    }
    previousFlag = flagIdentity
    const carrier = identity(view.getUint32(at + 8, true)), previousCarrier = identity(view.getUint32(at + 12, true)), initialCarrier = identity(view.getUint32(at + 16, true))
    if ((status === 1) !== (carrier !== null) || (status === 2) !== (returnValue !== -1)) {
      throw new Tf2CodecError("Capture flag state does not match its carrier or return deadline")
    }
    at += 84
    flags.push(Object.freeze({
      identity: flagIdentity, team, status: status as CaptureFlagStatus,
      disabled: (bits & 1) !== 0, visibleWhenDisabled: (bits & 2) !== 0,
      shotClock: (bits & 4) !== 0, allowOwnerPickup: (bits & 8) !== 0,
      trailEnabled: (bits & 16) !== 0, captured: (bits & 32) !== 0,
      skin, carrier, previousCarrier, initialCarrier,
      returnDeadline: returnValue === -1 ? null : returnValue,
      maximumReturnSeconds,
      ownerPickupDeadline: ownerValue === -1 ? null : ownerValue,
      configuredReturnSeconds, position, home, angles, homeAngles,
      model: text(), icon: text(), paperEffect: text(), trailEffect: text(),
    }))
  }
  const zones: CaptureZone[] = []
  let previousZone = -1
  for (let index = 0; index < zoneCount; index += 1) {
    require(40)
    const zoneIdentity = view.getUint32(at, true), team = data[at + 4], disabled = data[at + 5], model = view.getUint32(at + 8, true)
    const origin = vector(view, at + 12), center = vector(view, at + 24)
    if (zoneIdentity <= previousZone || (team !== 0 && team !== 2 && team !== 3) || (disabled !== 0 && disabled !== 1)
      || data[at + 6] !== 0 || data[at + 7] !== 0 || model === 0 || !finite([...origin, ...center])) {
      throw new Tf2CodecError("Capture zone record is invalid")
    }
    previousZone = zoneIdentity
    zones.push(Object.freeze({ identity: zoneIdentity, team: team === 0 ? null : team, disabled: disabled === 1, model, origin, center, capturePoint: view.getInt32(at + 36, true) }))
    at += 40
  }
  const events: CaptureEvent[] = []
  for (let index = 0; index < eventCount; index += 1) {
    require(24)
    const kind = data[at], detail = data[at + 1], team = data[at + 2], bits = data[at + 3]
    const value = view.getFloat32(at + 16, true), reserved = view.getFloat32(at + 20, true)
    if (kind === undefined || kind < 1 || kind > 8 || detail === undefined || (team !== 0 && team !== 2 && team !== 3)
      || bits === undefined || bits > 3 || !finite([value, reserved]) || reserved !== 0) {
      throw new Tf2CodecError("Capture objective event is invalid")
    }
    events.push(Object.freeze({ kind: kind as CaptureEvent["kind"], detail, team, flags: bits, subject: view.getUint32(at + 4, true), player: identity(view.getUint32(at + 8, true)), auxiliary: view.getUint32(at + 12, true), value }))
    at += 24
  }
  return Object.freeze({
    length: at,
    objectives: Object.freeze({
      redCaptures: view.getUint16(12, true), blueCaptures: view.getUint16(14, true),
      redScore: view.getUint16(16, true), blueScore: view.getUint16(18, true),
      captureLimit: view.getUint16(20, true), winner: winner === 0 ? null : winner,
      flags: Object.freeze(flags), zones: Object.freeze(zones), events: Object.freeze(events),
    }),
  })
}

function decodeRound(buffer: ArrayBuffer, offset: number, length: number): RoundSnapshot {
  if (length < 48) throw new Tf2CodecError("Round rules section is truncated")
  const data = new Uint8Array(buffer, offset, length), view = new DataView(buffer, offset, length)
  const stateFlags = data[8]!, state = stateFlags & 15, flags = data[9], winning = data[10], reason = data[11]
  const waiting = view.getFloat32(20, true), identity = view.getUint32(24, true), remaining = view.getFloat32(28, true)
  const count = view.getUint32(44, true)
  const headerLength = flags !== undefined && (flags & 128) !== 0 ? 96 : 48
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "PGRL" || view.getUint32(4, true) !== 4
    || (stateFlags & 112) !== 0 || state > 10 || flags === undefined
    || (winning !== 0 && winning !== 2 && winning !== 3) || reason === undefined
    || !finite([waiting, remaining]) || waiting < -1 || remaining < -1 || count > 4096
    || length !== headerLength + count * 12 || ((flags & 1) !== 0) !== (waiting !== -1)
    || ((flags & 8) !== 0) !== (identity !== 0xffff_ffff) || ((flags & 8) === 0 && remaining !== -1)) {
    throw new Tf2CodecError("Round rules section is invalid")
  }
  const events: RoundEvent[] = []
  const kothTimer = (at: number): RoundTimer => {
    const remaining = view.getFloat32(at + 4, true), bits = view.getUint32(at + 20, true)
    if (!Number.isFinite(remaining) || remaining < 0 || bits > 7) throw new Tf2CodecError("KOTH timer is invalid")
    return Object.freeze({ identity: view.getUint32(at, true), remaining,
      initialSeconds: view.getInt32(at + 8, true), setupSeconds: view.getInt32(at + 12, true), maximumSeconds: view.getInt32(at + 16, true),
      paused: (bits & 1) !== 0, showInHud: (bits & 2) !== 0, disabled: (bits & 4) !== 0 })
  }
  for (let index = 0; index < count; index += 1) {
    const at = headerLength + index * 12, kind = data[at], detail = data[at + 1], team = data[at + 2], bits = data[at + 3]
    if (kind === undefined || kind < 1 || kind > 17 || detail === undefined || bits === undefined
      || (team !== 0 && team !== 2 && team !== 3)) throw new Tf2CodecError("Round rules event is invalid")
    events.push(Object.freeze({ kind: kind as RoundEvent["kind"], detail, team, flags: bits, identity: view.getUint32(at + 4, true), value: view.getInt32(at + 8, true) }))
  }
  return Object.freeze({
    state: state as RoundState, fullRound: (stateFlags & 128) === 0, waitingForPlayers: (flags & 1) !== 0,
    waitingRemaining: waiting === -1 ? null : waiting, inSetup: (flags & 2) !== 0,
    inOvertime: (flags & 4) !== 0, winningTeam: winning === 0 ? null : winning,
    winReason: reason, redScore: view.getUint16(12, true), blueScore: view.getUint16(14, true),
    roundsPlayed: view.getUint32(16, true),
    kothTimers: headerLength === 96 ? Object.freeze([kothTimer(48), kothTimer(72)] as const) : null,
    timer: (flags & 8) === 0 ? null : Object.freeze({
      identity, remaining, initialSeconds: view.getInt32(32, true), setupSeconds: view.getInt32(36, true),
      maximumSeconds: view.getInt32(40, true), paused: (flags & 16) !== 0,
      showInHud: (flags & 32) !== 0, disabled: (flags & 64) !== 0,
    }),
    events: Object.freeze(events),
  })
}

function validateProjectileTransitions(events: readonly ProjectileEvent[]): void {
  const state = new Map<number, { kind: ProjectileKind; last?: ProjectileEventType; armed: boolean }>()
  for (const event of events) {
    const current = state.get(event.projectile) ?? { kind: event.kind, armed: false }
    if (current.kind !== event.kind || current.last === "fizzle" || current.last === "explode") {
      throw new Tf2CodecError("projectile event transition is illegal")
    }
    if (event.type === "fire" && current.last !== undefined) {
      throw new Tf2CodecError("projectile fire event is duplicated")
    }
    if (event.type === "impact" && current.last === "impact" && event.kind !== 2 && event.weapon !== 97) {
      throw new Tf2CodecError("projectile impact event is duplicated")
    }
    if (event.type === "stick" && (event.kind !== 2 || current.last !== "impact")) {
      throw new Tf2CodecError("projectile stick event is illegal")
    }
    if (event.type === "arm") {
      if (event.kind !== 2 || current.armed) throw new Tf2CodecError("projectile arm event is illegal")
      current.armed = true
    }
    current.last = event.type
    state.set(event.projectile, current)
  }
}

function decodeRandomState(bytes: ArrayBuffer, offset: number, length: number): Tf2RandomState {
  if (length !== 368) throw new Tf2CodecError("TF2 random state length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "PRNG" || view.getUint32(4, true) !== 4) {
    throw new Tf2CodecError("TF2 random state identity is invalid")
  }
  let at = 8
  const stream = (): RandomStreamState => {
    const current = view.getInt32(at, true), shuffled = view.getInt32(at + 4, true)
    at += 8
    const table = Object.freeze(Array.from({ length: 32 }, () => {
      const value = view.getInt32(at, true)
      at += 4
      return value
    }))
    const initialized = current > 0 && current < 2_147_483_647 && shuffled > 0 && shuffled < 2_147_483_647 &&
      table.every((value) => value > 0 && value < 2_147_483_647)
    const uninitialized = shuffled === 0 && current <= 0 && current !== -2_147_483_648
    if (!initialized && !uninitialized) throw new Tf2CodecError("TF2 random stream state is invalid")
    return Object.freeze({ current, shuffled, table })
  }
  const authority = stream(), predictedPresentation = stream(), rocketSelections = data[at]!, stickySelections = data[at + 1]!
  const batSelections = data[at + 2]!, batHitWorldAvailable = batSelections & 3, shovelSelections = data[at + 3]!
  const fistAndBonesawFlesh = data[at + 4]!, fistAndBonesawWorld = data[at + 5]!, fistHitFleshAvailable = data[at + 6]!, kukriSelections = data[at + 7]!
  const flagEnemyStolenAvailable = data[at + 8]!, flagEnemyDroppedAvailable = data[at + 9]!, flagEnemyCapturedAvailable = data[at + 10]!, flagEnemyReturnedAvailable = data[at + 11]!, flagTeamDroppedAvailable = data[at + 12]!
  if ((rocketSelections & ~31) !== 0 || (stickySelections & ~63) !== 0 || (batHitWorldAvailable & ~3) !== 0
    || (fistAndBonesawFlesh & ~31) !== 0 || (fistAndBonesawWorld & ~15) !== 0 || (fistHitFleshAvailable & ~7) !== 0
    || (flagEnemyStolenAvailable & ~15) !== 0 || (flagEnemyDroppedAvailable & ~3) !== 0
    || (flagEnemyCapturedAvailable & ~7) !== 0 || (flagEnemyReturnedAvailable & ~7) !== 0 || (flagTeamDroppedAvailable & ~3) !== 0
    || data[at + 13]! > 15 || (data[362]! & ~7) !== 0 || data[363] !== 0 || view.getUint16(364,true)>0x3ff || view.getUint16(366,true)>0x3ff) {
    throw new Tf2CodecError("TF2 sound selection state is invalid")
  }
  const configuredAvailable = Object.freeze(Array.from(data.subarray(296, 360)))
  if (configuredAvailable.some((mask, index) => mask & ~((1 << (configuredEquipmentSoundWaves[index] ?? 0)) - 1))) throw new Tf2CodecError("configured sound selection mask is invalid")
  const projectileUnlockAvailable = Object.freeze([
    data[360]! & 7, (data[360]! >> 3) & 7,
    (data[360]! >> 6) | ((data[361]! & 1) << 2),
    (data[361]! >> 1) & 7, data[361]! >> 4, data[362]!,
  ])
  return Object.freeze({ payloadWarningAvailable: Object.freeze([view.getUint16(364,true),view.getUint16(366,true)]) as readonly [number,number], configuredAvailable, authority, predictedPresentation, projectileUnlockAvailable, rocketExplosionAvailable: rocketSelections & 7, stickyExplosionAvailable: stickySelections & 7, batHitWorldAvailable, shovelHitWorldAvailable: shovelSelections & 3, shovelHitFleshAvailable: (shovelSelections >> 2) & 7, knifeHitFleshAvailable: shovelSelections >> 5, fistMissAvailable: fistAndBonesawFlesh & 3, fistHitWorldAvailable: fistAndBonesawWorld & 3, fistHitFleshAvailable, bonesawHitFleshAvailable: fistAndBonesawFlesh >> 2, bonesawHitWorldAvailable: fistAndBonesawWorld >> 2, kukriHitFleshAvailable: kukriSelections & 7, kukriHitWorldAvailable: (kukriSelections >> 3) & 3, wrenchHitFleshAvailable: kukriSelections >> 5, fireAxeHitWorldAvailable: rocketSelections >> 3, fireAxeHitFleshAvailable: stickySelections >> 3, flagEnemyStolenAvailable, flagEnemyDroppedAvailable, flagEnemyCapturedAvailable, flagEnemyReturnedAvailable, flagTeamDroppedAvailable, bottleHitFleshAvailable: (batSelections >> 2) & 7, bottleHitWorldAvailable: batSelections >> 5, overtimeAvailable: data[at + 13]!, controlPointAvailable: view.getUint16(at + 14, true) })
}

function decodeCollisionSnapshot(bytes: ArrayBuffer, offset: number, length: number): CollisionSnapshot {
  if (length < 52 || length > 16 * 1024 * 1024) throw new Tf2CodecError("Collision snapshot length is invalid")
  const data = new Uint8Array(bytes, offset, length), view = new DataView(bytes, offset, length)
  if (new TextDecoder().decode(data.subarray(0, 4)) !== "CSNP" || view.getUint32(4, true) !== 3) {
    throw new Tf2CodecError("Collision snapshot identity is invalid")
  }
  const worldIdentity = Array.from(data.subarray(8, 40), (value) => value.toString(16).padStart(2, "0")).join("")
  if (!HASH.test(worldIdentity)) throw new Tf2CodecError("Collision world identity is invalid")
  return Object.freeze({ worldIdentity, identity: view.getBigUint64(40, true), objects: count(view.getUint32(48, true), "Collision object"), bytes: data.slice() })
}

export function decodeSnapshot(bytes: ArrayBuffer | Uint8Array, ranges?: SnapshotRanges): Snapshot {
  if (bytes.byteLength < 184 || bytes.byteLength > MAX_SNAPSHOT_BYTES) {
    throw new Tf2CodecError("snapshot byte length is invalid")
  }
  const data = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)
  const buffer = data.buffer as ArrayBuffer
  const base = data.byteOffset
  const view = new DataView(buffer, base, data.byteLength)
  if (data[0] !== 0x50 || data[1] !== 0x53 || data[2] !== 0x53 || data[3] !== 0x4e || view.getUint32(4, true) !== 31)
    throw new Tf2CodecError("snapshot identity is invalid")
  const tf2Class = data[16]
  const team = data[17]
  const weapon = data[18]
  const lifecycle = data[28]
  if (
    (tf2Class === undefined || tf2Class < 1 || tf2Class > 9) ||
    (team === undefined || team > 3) ||
    (weapon === undefined || weapon !== 0 && !isTf2Weapon(weapon)) ||
    (team === 0 && lifecycle !== 3) ||
    (team === 1 && lifecycle !== 4) ||
    ((team === 2 || team === 3) && lifecycle !== 1 && lifecycle !== 2) ||
    (team < 2 && weapon !== 0) ||
    data[19]! > 1 || (tf2Class !== 8 && (data[29] !== 0 || data[30] !== 0 || data[31] !== 0))
  )
    throw new Tf2CodecError("snapshot selection is invalid")
  const health = view.getFloat32(20, true)
  const maximumHealth = view.getFloat32(24, true)
  if (!finite([health, maximumHealth]) || health < 0 || maximumHealth <= 0) {
    throw new Tf2CodecError("snapshot health is invalid")
  }
  const readConditions = () => Object.freeze(Array.from({ length: 5 }, (_, index) => view.getUint32(32 + index * 4, true))) as
    readonly [number, number, number, number, number]
  const conditions = ranges ? ranges.read("conditions", 0, 32, 20, readConditions) : readConditions()
  const loadoutCount = count(view.getUint32(56, true), "loadout")
  const projectileCount = count(view.getUint32(60, true), "projectile")
  const projectileEventCount = count(view.getUint32(64, true), "projectile event")
  const entityTransformCount = count(view.getUint32(68, true), "entity transform")
  const entityEventCount = count(view.getUint32(72, true), "entity event")
  const gameplayEventCount = count(view.getUint32(76, true), "gameplay event")
  const jumpLength = view.getUint32(80, true)
  const movementLength = view.getUint32(84, true)
  const activityCount = count(view.getUint32(88, true), "activity")
  const lifecycleEventCount = count(view.getUint32(92, true), "lifecycle event")
  const physicsRequestCount = count(view.getUint32(96, true), "Physics request")
  const rocketRequestCount = count(view.getUint32(100, true), "rocket trace request")
  const radiusRequestCount = count(view.getUint32(104, true), "radius damage request")
  const moverRequestCount = count(view.getUint32(108, true), "mover request")
  const contactRequestCount = count(view.getUint32(112, true), "contact reconcile request")
  const mapEffectCount = count(view.getUint32(116, true), "map effect")
  const regenerateEventCount = count(view.getUint32(120, true), "regenerate animation event")
  const blockerCount = count(view.getUint32(124, true), "authority blocker")
  const randomDrawCount = count(view.getUint32(128, true), "random draw")
  const audioEventCount = count(view.getUint32(132, true), "audio event")
  const rocketResultCount = count(view.getUint32(136, true), "rocket result")
  const moverResultCount = count(view.getUint32(140, true), "mover result")
  const collisionSnapshotLength = view.getUint32(144, true)
  const randomStateLength = view.getUint32(148, true)
  const entityPresentationLength=view.getUint32(152,true),movementTickLength=view.getUint32(156,true)
  const playerFlags=view.getUint32(160,true),waterType=view.getUint32(164,true)
  const flamePointCount=count(view.getUint32(168,true),"flame point")
  const shotgunPelletCount=count(view.getUint32(172,true),"shotgun pellet")
  const flameFlags=view.getUint32(176,true)
  if (entityPresentationLength<52||movementTickLength<12||180 + movementLength > bytes.byteLength||waterType&~0x30||flameFlags>1||flamePointCount>30||shotgunPelletCount>10) {
    throw new Tf2CodecError("snapshot extension header is invalid")
  }
  const movement = movementSnapshot(buffer, base + 180, movementLength, waterType)
  if (movement.mode !== data[19]||movement.waterLevel>3||(movement.waterLevel===0)!==(waterType===0)) {
    throw new Tf2CodecError("Movement mode or water projection differs")
  }
  let at = 180 + movementLength
  const requireBytes = (length: number, label: string): void => {
    if (!Number.isSafeInteger(at + length) || at + length > bytes.byteLength) {
      throw new Tf2CodecError(`${label} records exceed snapshot bytes`)
    }
  }

  requireBytes(loadoutCount * 48, "loadout")
  const loadout: WeaponState[] = []
  let cloakMeter = 0, invisibility = 0, disguiseCompleteTime = 0, noAttackUntil = 0
  for (let index = 0; index < loadoutCount; index += 1) {
    const item = at + index * 48
    const itemWeapon = data[item]
    const reload = data[item + 1]
    if (
      itemWeapon === undefined ||
      itemWeapon < 1 ||

      !isTf2Weapon(itemWeapon) ||

      reload === undefined ||
      reload > 3 ||
      data[item + 2] !== 0 ||
      data[item + 3] !== 0 ||
      !Number.isFinite(view.getFloat32(item + 44, true)) || view.getFloat32(item + 44, true) < 0
      || (itemWeapon === 9 ? view.getFloat32(item + 44, true) <= 0 : tf2Class !== 8 && view.getFloat32(item + 44, true) > 150)
    )
      throw new Tf2CodecError("loadout record is invalid")
    if (tf2Class === 8) {
      const scalar = view.getFloat32(item + 44, true)
      if (!Number.isFinite(scalar) || scalar < 0) throw new Tf2CodecError("Spy loadout state is invalid")
      if (itemWeapon === 54) cloakMeter = scalar
      else if (itemWeapon === 53) invisibility = scalar
      else if (itemWeapon === 51) disguiseCompleteTime = scalar
      else if (itemWeapon === 52) noAttackUntil = scalar
      else if (scalar !== 0) throw new Tf2CodecError("Spy loadout state is invalid")
    }
    const readWeapon = () => Object.freeze({
        weapon: itemWeapon as Tf2Weapon,
        reload: reload as WeaponState["reload"],
        clip: view.getUint16(item + 4, true),
        reserve: view.getUint16(item + 6, true),
        maximumClip: view.getUint16(item + 8, true),
        maximumReserve: view.getUint16(item + 10, true),
        nextPrimaryTick: view.getBigUint64(item + 12, true),
        reloadDueTick: view.getBigUint64(item + 20, true) === 0xffff_ffff_ffff_ffffn ? null : view.getBigUint64(item + 20, true),
        chargeBeginTick: view.getBigUint64(item + 28, true) === 0xffff_ffff_ffff_ffffn ? null : view.getBigUint64(item + 28, true),
        firstPrimaryTick: view.getBigUint64(item + 36, true),
        chargedDamage: itemWeapon === 9 ? 0 : view.getFloat32(item + 44, true),
        prefirePlaybackRate: itemWeapon === 9 ? view.getFloat32(item + 44, true) : null,
      })
    loadout.push(ranges ? ranges.read("loadout", index, item, 48, readWeapon) : readWeapon())
  }
  if ((weapon === 0 && loadout.length !== 0)
    || (weapon !== 0 && !loadout.some((entry) => entry.weapon === weapon))) {
    throw new Tf2CodecError("snapshot active weapon does not match its loadout")
  }
  const activeDisguiseClass = data[29]!, activeDisguiseTeam = data[30]!, desiredDisguiseClass = data[31]! & 15, desiredDisguiseTeam = data[31]! >> 4
  if (tf2Class === 8 && (
    cloakMeter > 100 || invisibility > 1 ||
    (activeDisguiseClass === 0) !== (activeDisguiseTeam === 0) ||
    (activeDisguiseClass !== 0 && (activeDisguiseClass > 9 || (activeDisguiseTeam !== 2 && activeDisguiseTeam !== 3))) ||
    (desiredDisguiseClass === 0) !== (desiredDisguiseTeam === 0) ||
    (desiredDisguiseClass !== 0 && (desiredDisguiseClass > 9 || (desiredDisguiseTeam !== 2 && desiredDisguiseTeam !== 3))) ||
    (lifecycle === 1 && weapon !== 0 && !loadout.some((entry) => entry.weapon === 54))
  )) throw new Tf2CodecError("Spy snapshot state is invalid")
  const spy: SpySnapshot | null = tf2Class !== 8 ? null : Object.freeze({
    cloakMeter,
    invisibility,
    disguise: activeDisguiseClass === 0 ? null : Object.freeze({ class: activeDisguiseClass as Tf2Class, team: activeDisguiseTeam as Tf2Team }),
    desiredDisguise: desiredDisguiseClass === 0 ? null : Object.freeze({ class: desiredDisguiseClass as Tf2Class, team: desiredDisguiseTeam as Tf2Team }),
    disguiseCompleteTime,
    noAttackUntil,
  })
  at += loadoutCount * 48

  requireBytes(flamePointCount * 80, "flame point")
  const flamePoints: FlamePoint[] = []
  for(let index=0;index<flamePointCount;index+=1){
    const item=at+index*80,slot=data[item]!,wallsHit=data[item+1]!
    if(slot>=30||data[item+2]!==0||data[item+3]!==0)throw new Tf2CodecError("flame point record is invalid")
    const vector=(offset:number)=>Object.freeze([view.getFloat32(item+offset,true),view.getFloat32(item+offset+4,true),view.getFloat32(item+offset+8,true)]) as readonly[number,number,number]
    const spawnTime=view.getFloat32(item+12,true),lifetime=view.getFloat32(item+16,true),initialPosition=vector(20),previousPosition=vector(32),position=vector(44),velocity=vector(56),attackerVelocity=vector(68)
    if(!finite([spawnTime,lifetime,...initialPosition,...previousPosition,...position,...velocity,...attackerVelocity])||lifetime<=0)throw new Tf2CodecError("flame point record is invalid")
    flamePoints.push(Object.freeze({slot,wallsHit,spawnTick:view.getBigUint64(item+4,true),spawnTime,lifetime,initialPosition,previousPosition,position,velocity,attackerVelocity}))
  }
  at+=flamePointCount*80
  requireBytes(shotgunPelletCount*24,"shotgun pellet")
  const shotgunPellets:ShotgunPellet[]=[]
  for(let index=0;index<shotgunPelletCount;index+=1){
    const item=at+index*24,pellet=data[item]!
    if(pellet>=10||data[item+1]!==0||data[item+2]!==0||data[item+3]!==0)throw new Tf2CodecError("shotgun pellet record is invalid")
    const direction=Object.freeze([view.getFloat32(item+4,true),view.getFloat32(item+8,true),view.getFloat32(item+12,true)]) as readonly[number,number,number]
    const damage=view.getFloat32(item+16,true),range=view.getFloat32(item+20,true)
    if(!finite([...direction,damage,range])||damage<=0||range<=0)throw new Tf2CodecError("shotgun pellet record is invalid")
    shotgunPellets.push(Object.freeze({index:pellet,direction,damage,range}))
  }
  at+=shotgunPelletCount*24

  requireBytes(projectileCount * 84, "projectile")
  const projectiles: Projectile[] = []
  for (let index = 0; index < projectileCount; index += 1) {
    const item = at + index * 84
    const appearance = data[item + 4]!
    const kind = appearance & 7
    const miniRocket = (appearance & 8) !== 0
    const trail = (appearance >> 4) & 7
    const practiceExplosion = (appearance & 128) !== 0
    const projectileTeam = data[item + 5]
    const packedState = data[item + 6]!
    const flags = data[item + 7]!
    const state = packedState & 7
    const hasNormal = flags & 1
    const weapon = (packedState >> 3) | (((flags >> 2) & 3) << 5)
    const critical = (flags & 2) !== 0
    const selfBlastOnly = (flags & 16) !== 0
    const modelVisible = (flags & 32) !== 0
    const airBurst = (flags & 64) !== 0
    const underwaterExplosion = (flags & 128) !== 0
    const position = vector(view, item + 16)
    const velocity = vector(view, item + 28)
    const orientation = quaternion(view, item + 40)
    const angularVelocity = vector(view, item + 56)
    const rawNormal = vector(view, item + 68)
    const ageSeconds = view.getFloat32(item + 80, true)
    if (
      (kind !== 1 && kind !== 2 && kind !== 3 && kind !== 4) ||
      !isTf2Weapon(weapon) || trail > 6 ||
      (projectileTeam !== 2 && projectileTeam !== 3) ||
      state === undefined ||
      state < 1 ||
      state > 3 ||
      hasNormal === undefined ||
      hasNormal > 1 ||
      !finite([...position, ...velocity, ...angularVelocity, ageSeconds]) ||
      !normalized(orientation) ||
      ageSeconds < 0 ||
      (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) ||
      (state !== 1 && (kind !== 2 || hasNormal !== 1))
    )
      throw new Tf2CodecError("projectile record is invalid")
    projectiles.push(
      Object.freeze({
        identity: view.getUint32(item, true),
        trail: trail as ProjectileTrail,
        miniRocket,
        practiceExplosion,
        modelVisible,
        airBurst,
        underwaterExplosion,
        weapon,
        critical,
        selfBlastOnly,
        kind,
        team: projectileTeam,
        ownerIdentity: view.getUint32(item + 8, true),
        launcherIdentity: view.getUint32(item + 12, true),
        state: state as ProjectileState,
        position,
        velocity,
        orientation,
        angularVelocity,
        contactNormal: hasNormal === 1 ? rawNormal : null,
        ageSeconds,
      }),
    )
  }
  at += projectileCount * 84

  requireBytes(projectileEventCount * 92, "projectile event")
  const projectileEvents: ProjectileEvent[] = []
  const eventNames: readonly ProjectileEventType[] = ["fire", "impact", "stick", "arm", "fizzle", "explode", "deflect"]
  for (let index = 0; index < projectileEventCount; index += 1) {
    const item = at + index * 92
    const packedEvent = data[item]!
    const eventCode = packedEvent & 7
    const appearance = data[item + 1]!
    const kind = appearance & 7
    const miniRocket = (appearance & 8) !== 0
    const trail = (appearance >> 4) & 7
    const practiceExplosion = (appearance & 128) !== 0
    const projectileTeam = data[item + 2]
    const flags = data[item + 3]
    const weapon = (packedEvent >> 3) | ((((flags ?? 0) >> 3) & 3) << 5)
    const critical = ((flags ?? 0) & 4) !== 0
    const selfBlastOnly = ((flags ?? 0) & 32) !== 0
    const airBurst = ((flags ?? 0) & 128) !== 0
    const underwaterExplosion = ((flags ?? 0) & 64) !== 0
    const hasNormal = flags === undefined ? undefined : flags & 1
    const hasLauncherPose = flags === undefined ? undefined : (flags >> 1) & 1
    const position = vector(view, item + 24)
    const orientation = quaternion(view, item + 36)
    const rawNormal = vector(view, item + 52)
    const eyePosition = vector(view, item + 64)
    const viewOrientation = quaternion(view, item + 76)
    if (
      eventCode === undefined ||
      eventCode < 1 ||
      eventCode > 7 || trail > 6 ||
      (kind !== 1 && kind !== 2 && kind !== 3 && kind !== 4) ||
      !isTf2Weapon(weapon) ||
      (projectileTeam !== 2 && projectileTeam !== 3) ||
      flags === undefined ||
      hasNormal === undefined ||
      hasLauncherPose === undefined ||
      !finite(position) ||
      !normalized(orientation) ||
      (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) ||
      ((eventCode === 2 || eventCode === 3) && hasNormal !== 1) ||
      (eventCode === 1) !== (hasLauncherPose === 1) ||
      (hasLauncherPose === 1 && (!finite(eyePosition) || !normalized(viewOrientation))) ||
      (hasLauncherPose === 0 && [...eyePosition, ...viewOrientation].some((value) => value !== 0))
    )
      throw new Tf2CodecError("projectile event record is invalid")
    projectileEvents.push(
      Object.freeze({
        type: eventNames[eventCode - 1]!,
        trail: trail as ProjectileTrail,
        miniRocket,
        practiceExplosion,
        weapon,
        critical,
        selfBlastOnly,
        airBurst,
        underwaterExplosion,
        projectile: view.getUint32(item + 4, true),
        kind,
        ownerIdentity: view.getUint32(item + 8, true),
        launcherIdentity: view.getUint32(item + 12, true),
        team: projectileTeam,
        tick: view.getBigUint64(item + 16, true),
        position,
        orientation,
        contactNormal: hasNormal === 1 ? rawNormal : null,
        launcherPose: hasLauncherPose === 1
          ? Object.freeze({ eyePosition, viewOrientation })
          : null,
      }),
    )
  }
  validateProjectileTransitions(projectileEvents)
  at += projectileEventCount * 92

  requireBytes(entityTransformCount * 32, "entity transform")
  const readTransforms = () => {
  const entityTransforms: EntityTransform[] = []
  for (let index = 0; index < entityTransformCount; index += 1) {
    const item = at + index * 32
    const position = vector(view, item + 8)
    const angles = vector(view, item + 20)
    if (!finite([...position, ...angles])) throw new Tf2CodecError("entity transform scalar is invalid")
    entityTransforms.push(
      Object.freeze({
        identity: view.getUint32(item, true),
        model: view.getUint32(item + 4, true),
        position,
        angles,
      }),
    )
  }
  return Object.freeze(entityTransforms)
  }
  const entityTransforms = ranges ? ranges.read("transforms", 0, at, entityTransformCount * 32, readTransforms) : readTransforms()
  at += entityTransformCount * 32

  const entityEvents: EntityEvent[] = []
  const decoder = new TextDecoder("utf-8", { fatal: true })
  for (let index = 0; index < entityEventCount; index += 1) {
    requireBytes(24, "entity event")
    const kind = data[at + 8]
    const accepted = data[at + 9]
    const contact = data[at + 10]
    const nameLength = view.getUint32(at + 20, true)
    if (
      kind === undefined ||
      kind < 1 ||
      kind > 8 ||
      accepted === undefined ||
      accepted > 1 ||
      contact === undefined ||
      contact > 3 ||
      data[at + 11] !== 0 ||
      nameLength > 2_047
    )
      throw new Tf2CodecError("entity event record is invalid")
    requireBytes(24 + nameLength, "entity event")
    let name: string
    try {
      name = decoder.decode(data.subarray(at + 24, at + 24 + nameLength))
    } catch {
      throw new Tf2CodecError("entity event name is invalid UTF-8")
    }
    const subject = view.getUint32(at + 16, true)
    entityEvents.push(
      Object.freeze({
        sequence: view.getBigUint64(at, true),
        kind: kind as EntityEvent["kind"],
        accepted: accepted === 1,
        contact: contact === 0 ? null : (contact as 1 | 2 | 3),
        entity: view.getUint32(at + 12, true),
        subject: subject === 0xffff_ffff ? null : subject,
        name,
      }),
    )
    at += 24 + nameLength
  }

  requireBytes(gameplayEventCount * 28, "gameplay event")
  const events: GameplayEvent[] = []
  for (let index = 0; index < gameplayEventCount; index += 1) {
    requireBytes(28, "gameplay event")
    const item = at
    const kind = data[item]
    const death = kind === 18
    const readValue = (offset: number) => death ? view.getUint32(offset, true) : view.getFloat32(offset, true)
    const values = Object.freeze([
      readValue(item + 12),
      readValue(item + 16),
      readValue(item + 20),
      readValue(item + 24),
    ]) as readonly [number, number, number, number]
    const nameLength = view.getUint16(item + 2, true)
    if (kind === 17 && ![0, 1, 2].includes(values[2])) throw new Tf2CodecError("damage critical kind is invalid")
    if (kind === undefined || kind < 1 || kind > 19 && kind !== 24 || ((kind === 15 || kind === 16) && data[item + 1]! > 2) || (kind === 24 && (data[item + 1] !== 98 || view.getUint32(item + 4, true) < 1 || view.getUint32(item + 4, true) > 4)) || (death ? nameLength < 1 || nameLength > 255 : nameLength !== 0) || !finite(values))
      throw new Tf2CodecError("gameplay event record is invalid")
    requireBytes(28 + nameLength, "death notice weapon")
    const killingWeapon = death ? new TextDecoder("utf-8", { fatal: true }).decode(data.subarray(item + 28, item + 28 + nameLength)) : undefined
    if (killingWeapon !== undefined && !/^[a-zA-Z0-9_]+$/u.test(killingWeapon)) throw new Tf2CodecError("death notice weapon is invalid")
    events.push(
      Object.freeze({
        kind: kind as GameplayEvent["kind"],
        detail: data[item + 1]!,
        subject: view.getUint32(item + 4, true),
        auxiliary: view.getUint32(item + 8, true),
        values,
        ...(killingWeapon === undefined ? {} : { killingWeapon }),
      }),
    )
    at += 28 + nameLength
  }

  requireBytes(activityCount * 16, "activity")
  const activities: ActivityEvent[] = []
  for (let index = 0; index < activityCount; index += 1) {
    const item = at + index * 16
    const itemWeapon = data[item + 8]
    const activity = data[item + 9]

    if (itemWeapon === undefined || !isTf2Weapon(itemWeapon) || activity === undefined || activity < 1 || activity > 13 ||

      !data.subarray(item + 10, item + 16).every((value) => value === 0)) {
      throw new Tf2CodecError("activity record is invalid")
    }
    activities.push(Object.freeze({
      tick: view.getBigUint64(item, true),
      weapon: itemWeapon as Tf2Weapon,
      activity: activity as WeaponActivity,
    }))
  }
  at += activityCount * 16

  requireBytes(lifecycleEventCount * 16, "lifecycle event")
  const lifecycleEvents: LifecycleEvent[] = []
  for (let index = 0; index < lifecycleEventCount; index += 1) {
    const item = at + index * 16
    const kind = data[item + 8], itemClass = data[item + 9], itemTeam = data[item + 10]
    if (kind === undefined || kind < 1 || kind > 4 || itemClass === undefined || itemClass < 1 || itemClass > 9 ||
      (itemTeam === undefined || itemTeam > 3) || !data.subarray(item + 11, item + 16).every((value) => value === 0)) {
      throw new Tf2CodecError("lifecycle event record is invalid")
    }
    lifecycleEvents.push(Object.freeze({
      tick: view.getBigUint64(item, true),
      kind: kind as LifecycleEvent["kind"],
      class: itemClass as Tf2Class,
      team: itemTeam,
    }))
  }
  at += lifecycleEventCount * 16

  requireBytes(physicsRequestCount * 104, "Physics request")
  const physicsRequests: ProjectilePhysicsRequest[] = []
  for (let index = 0; index < physicsRequestCount; index += 1) {
    const item = at + index * 104
    const operation = data[item]
    const position = vector(view, item + 16), velocity = vector(view, item + 28), orientation = quaternion(view, item + 40),
      angularVelocity = vector(view, item + 56), hullMins = vector(view, item + 68), hullMaxs = vector(view, item + 80)
    const gravityScale = view.getFloat32(item + 92, true), friction = view.getFloat32(item + 96, true), elasticity = view.getFloat32(item + 100, true)
    if (operation === undefined || operation < 1 || operation > 4 || !data.subarray(item + 1, item + 4).every((value) => value === 0) ||
      !finite([...position, ...velocity, ...angularVelocity, ...hullMins, ...hullMaxs, gravityScale, friction, elasticity]) ||
      !normalized(orientation) || hullMins.some((value, axis) => value > hullMaxs[axis]!)) {
      throw new Tf2CodecError("Physics request record is invalid")
    }
    physicsRequests.push(Object.freeze({ operation: operation as ProjectilePhysicsRequest["operation"], projectile: view.getUint32(item + 4, true),
      tick: view.getBigUint64(item + 8, true), position, velocity, orientation, angularVelocity, hullMins, hullMaxs, gravityScale, friction, elasticity }))
  }
  at += physicsRequestCount * 104

  requireBytes(rocketRequestCount * 44, "rocket trace request")
  const rocketTraceRequests: RocketTraceRequest[] = []
  for (let index = 0; index < rocketRequestCount; index += 1) {
    const item = at + index * 44, start = vector(view, item + 16), end = vector(view, item + 28)
    if (view.getUint32(item + 4, true) !== 0 || !finite([...start, ...end])) throw new Tf2CodecError("rocket trace request record is invalid")
    rocketTraceRequests.push(Object.freeze({ projectile: view.getUint32(item, true), tick: view.getBigUint64(item + 8, true), start, end, mask: view.getUint32(item + 40, true) }))
  }
  at += rocketRequestCount * 44

  requireBytes(radiusRequestCount * 36, "radius damage request")
  const radiusDamageRequests: RadiusDamageRequest[] = []
  for (let index = 0; index < radiusRequestCount; index += 1) {
    const item = at + index * 36, kind = data[item + 4], source = vector(view, item + 8)
    const values = [view.getFloat32(item + 20, true), view.getFloat32(item + 24, true), view.getFloat32(item + 28, true)]
    if ((kind !== 1 && kind !== 2 && kind !== 4) || !data.subarray(item + 5, item + 8).every((value) => value === 0) || !finite([...source, ...values])) {
      throw new Tf2CodecError("radius damage request record is invalid")
    }
    const directTarget = view.getUint32(item + 32, true)
    radiusDamageRequests.push(Object.freeze({ projectile: view.getUint32(item, true), kind, source, baseDamage: values[0]!, radius: values[1]!,
      selfRadius: values[2]!, directTarget: directTarget === 0xffff_ffff ? null : directTarget }))
  }
  at += radiusRequestCount * 36

  requireBytes(moverRequestCount * 48, "mover request")
  const moverRequests: MoverRequest[] = []
  for (let index = 0; index < moverRequestCount; index += 1) {
    const item = at + index * 48, start = vector(view, item + 16), destination = vector(view, item + 28), speed = view.getFloat32(item + 40, true)
    const opening = data[item + 44]
    if (opening === undefined || opening > 1 || !data.subarray(item + 45, item + 48).every((value) => value === 0) || !finite([...start, ...destination, speed])) {
      throw new Tf2CodecError("mover request record is invalid")
    }
    const model = view.getUint32(item + 12, true)
    moverRequests.push(Object.freeze({ requestId: view.getBigUint64(item, true), entity: view.getUint32(item + 8, true),
      model: model === 0xffff_ffff ? null : model, start, destination, speed, opening: opening === 1 }))
  }
  at += moverRequestCount * 48

  requireBytes(contactRequestCount * 44, "contact reconcile request")
  const contactReconcileRequests: ContactReconcileRequest[] = []
  for (let index = 0; index < contactRequestCount; index += 1) {
    const item = at + index * 44, position = vector(view, item + 8), hullMins = vector(view, item + 20), hullMaxs = vector(view, item + 32)
    if (!finite([...position, ...hullMins, ...hullMaxs]) || hullMins.some((value, axis) => value > hullMaxs[axis]!)) {
      throw new Tf2CodecError("contact reconcile request record is invalid")
    }
    contactReconcileRequests.push(Object.freeze({ tick: view.getBigUint64(item, true), position, hullMins, hullMaxs }))
  }
  at += contactRequestCount * 44

  requireBytes(mapEffectCount * 40, "map effect")
  const mapEffects: MapEffect[] = []
  for (let index = 0; index < mapEffectCount; index += 1) {
    const item = at + index * 40, kind = data[item], detail = data[item + 1], rawTeam = data[item + 2], rawContact = data[item + 3]
    const values = Object.freeze(Array.from({ length: 6 }, (_, value) => view.getFloat32(item + 16 + value * 4, true))) as
      readonly [number, number, number, number, number, number]
    if (kind === undefined || kind < 1 || kind > 5 || detail === undefined || detail > 1 || rawTeam === undefined ||
      (rawTeam !== 0 && rawTeam !== 2 && rawTeam !== 3) || rawContact === undefined || rawContact > 3 ||
      view.getUint32(item + 12, true) !== 0 || !finite(values)) throw new Tf2CodecError("map effect record is invalid")
    const auxiliary = view.getUint32(item + 8, true)
    const semantic = kind === 1
      ? rawTeam === 0 && rawContact === 0 && auxiliary !== 0xffff_ffff &&
        (detail === 1 || values.slice(3).every((value) => value === 0))
      : kind === 2
        ? detail === 0 && rawTeam === 0 && rawContact >= 1 && auxiliary === 0xffff_ffff &&
          values.slice(1).every((value) => value === 0)
        : kind === 3
          ? rawTeam === 0 && rawContact === 0 && auxiliary === 0xffff_ffff && values.slice(3).every((value) => value === 0)
          : kind === 4
            ? detail === 0 && rawContact === 0 && values.every((value) => value === 0)
            : detail === 0 && rawContact >= 1 && auxiliary === 0xffff_ffff && values.every((value) => value === 0)
    if (!semantic) throw new Tf2CodecError("map effect semantics are invalid")
    mapEffects.push(Object.freeze({ kind: kind as MapEffect["kind"], detail, team: rawTeam === 0 ? null : rawTeam,
      contact: rawContact === 0 ? null : rawContact as ContactKind, subject: view.getUint32(item + 4, true),
      auxiliary: auxiliary === 0xffff_ffff ? null : auxiliary, values }))
  }
  at += mapEffectCount * 40

  requireBytes(regenerateEventCount * 32, "regenerate animation event")
  const regenerateAnimationEvents: RegenerateAnimationEvent[] = []
  for (let index = 0; index < regenerateEventCount; index += 1) {
    const item = at + index * 32,openAnimation=data[item+28],closeAnimation=data[item+29]
    if((openAnimation!==1&&openAnimation!==2)||(closeAnimation!==1&&closeAnimation!==2)||data[item+30]!==0||data[item+31]!==0)throw new Tf2CodecError("regenerate animation event is invalid")
    regenerateAnimationEvents.push(Object.freeze({ zone: view.getUint32(item, true), associatedModel: view.getUint32(item + 4, true),
      openTick: view.getBigUint64(item + 8, true), closeTick: view.getBigUint64(item + 16, true),body:view.getInt32(item+24,true),openAnimation:openAnimation===1?"open":"close",closeAnimation:closeAnimation===1?"open":"close" }))
  }
  at += regenerateEventCount * 32

  requireBytes(blockerCount * 4, "authority blocker")
  const authorityBlockers: AuthorityBlocker[] = []
  const blockerDetails = new Map<number, string>([
    [1, "TF2 sticky IVP solver unavailable: current body/contact transition"],
    [2, "Tempus core and configured Jump course contract unavailable"],
    [3, "Payload cart visible model requires its authored breakable constraint and VPhysics rigid-body authority"],
  ])
  const blockerCodes = new Set<number>()
  for (let index = 0; index < blockerCount; index += 1) {
    const item = at + index * 4, code = data[item], classification = data[item + 1]
    const detail = code === undefined ? undefined : blockerDetails.get(code)
    if (!detail || classification !== 1 || data[item + 2] !== 0 || data[item + 3] !== 0 || blockerCodes.has(code)) {
      throw new Tf2CodecError("authority blocker record is invalid")
    }
    blockerCodes.add(code)
    authorityBlockers.push(Object.freeze({ code: code as 1 | 2 | 3, classification: "Missing", detail }))
  }
  if ((blockerCodes.size !== 2 && blockerCodes.size !== 3) || !blockerCodes.has(1) || !blockerCodes.has(2)) {
    throw new Tf2CodecError("authority blocker set is incomplete")
  }
  at += blockerCount * 4

  requireBytes(randomStateLength, "TF2 random state")
  const readRandomState = () => decodeRandomState(buffer, base + at, randomStateLength)
  const randomState = ranges ? ranges.read("random", 0, at, randomStateLength, readRandomState) : readRandomState()
  at += randomStateLength

  requireBytes(randomDrawCount * 16, "random draw")
  const randomDraws: RandomDraw[] = []
  for (let index = 0; index < randomDrawCount; index += 1) {
    const item = at + index * 16, context = data[item], decision = data[item + 1], definition = data[item + 2], phase = data[item + 3]
    const raw = view.getInt32(item + 4, true), resultKind = data[item + 8], resultValue = view.getUint32(item + 12, true)
    const soundDecision = decision !== undefined && decision >= 1 && decision <= 4
    if (
      (context !== 1 && context !== 2) || decision === undefined || decision < 1 || decision > 14 && decision !== 64 && decision !== 65 ||
      (soundDecision ? !isSoundDefinition(definition) || (phase !== 1 && phase !== 2) : definition !== 0 || phase !== 0 || context !== 1 && decision !== 14 && decision !== 65) ||
      raw <= 0 || raw >= 2_147_483_647 || resultKind === undefined || resultKind < 1 || resultKind > 3 ||
      data[item + 9] !== 0 || data[item + 10] !== 0 || data[item + 11] !== 0 ||
      ((decision === 3 || decision === 7 || decision === 8 || decision === 13 || decision === 14) ? resultKind === 1 : resultKind !== 1) ||
      (resultKind === 3 && resultValue !== 0)
    ) throw new Tf2CodecError("random draw record is invalid")
    const result: RandomDraw["result"] = resultKind === 1
      ? Object.freeze({ kind: "float-bits", bits: resultValue })
      : resultKind === 2
        ? Object.freeze({ kind: "integer", value: view.getInt32(item + 12, true) })
        : Object.freeze({ kind: "rejected-integer" })
    randomDraws.push(Object.freeze({ context, decision, definition: definition as RandomDraw["definition"], phase: phase as RandomDraw["phase"], raw, result }))
  }
  at += randomDrawCount * 16

  requireBytes(audioEventCount * 52, "audio event")
  const audioEvents: AudioEvent[] = []
  const nextOrdinal = new Map<bigint, number>()
  for (let index = 0; index < audioEventCount; index += 1) {
    const item = at + index * 52, tick = view.getBigUint64(item, true), ordinal = view.getUint16(item + 8, true), identity = data[item + 10],
      definition = data[item + 11], sourceKind = data[item + 12], hasOwner = data[item + 13], wave = data[item + 14]
    const sourceIdentity = view.getUint32(item + 16, true), rawOwner = view.getUint32(item + 20, true), position = vector(view, item + 24)
    const volume = view.getFloat32(item + 36, true), pitch = view.getFloat32(item + 40, true), soundLevel = view.getFloat32(item + 44, true)
    const action = data[item + 15]!, fadeSeconds = view.getFloat32(item + 48, true)
    const expectedOrdinal = nextOrdinal.get(tick) ?? 0, waveCount = definition === undefined ? 0 : definition >= 160 ? configuredEquipmentSoundWaves[definition - 160] ?? 0 : nativeEquipmentSoundWaves[definition] ?? 0
    if (
      (identity === undefined || identity < 1 || identity > 5) || !isSoundDefinition(definition) ||
      (sourceKind !== 1 && sourceKind !== 2 && sourceKind !== 3 && sourceKind !== 4) || (hasOwner !== 0 && hasOwner !== 1) || action > 4 ||
      ordinal !== expectedOrdinal || !canonicalIdentity(sourceIdentity) ||
      (hasOwner === 0 ? rawOwner !== 0xffff_ffff : !canonicalIdentity(rawOwner)) ||
      !finite([...position, volume, pitch, soundLevel]) || [volume, pitch, soundLevel].some((value) => value < 0 || value >= 1) ||
      wave === undefined || wave >= waveCount || !Number.isFinite(fadeSeconds) || (action === 4 ? fadeSeconds < 1 || fadeSeconds > 255 : action < 2 ? fadeSeconds !== 0 : fadeSeconds <= 0)
    ) throw new Tf2CodecError("audio event record is invalid")
    nextOrdinal.set(tick, expectedOrdinal + 1)
    audioEvents.push(Object.freeze({
      tick, ordinal, identity, definition, sourceKind, sourceIdentity,
      action: (["play", "stop", "fade-in", "fade-out", "play"] as const)[action]!, fadeSeconds: action === 4 ? 0 : fadeSeconds,
      ...(action === 4 ? { pitchOverride: fadeSeconds } : {}),
      ownerIdentity: hasOwner === 1 ? rawOwner : null,
      position,
      samples: Object.freeze({ volume, pitch, wave, soundLevel }),
    }))
  }
  at += audioEventCount * 52

  requireBytes(rocketResultCount * 44, "rocket result")
  const rocketTraceResults: RocketTraceResult[] = []
  for (let index = 0; index < rocketResultCount; index += 1) {
    const item = at + index * 44, solid = data[item + 12], sky = data[item + 13], hasNormal = data[item + 14], hasTarget = data[item + 15]
    const end = vector(view, item + 16), rawNormal = vector(view, item + 28), rawTarget = view.getUint32(item + 40, true)
    if (
      solid === undefined || solid > 1 || sky === undefined || sky > 1 || hasNormal === undefined || hasNormal > 1 || hasTarget === undefined || hasTarget > 1 ||
      !canonicalIdentity(view.getUint32(item, true)) || !finite([...end, ...rawNormal]) ||
      (sky === 1 && solid !== 1) || (solid === 1 && sky === 0 && hasNormal !== 1) ||
      (hasNormal === 0 && rawNormal.some((value) => value !== 0)) || (hasNormal === 1 && !normalized(rawNormal)) ||
      (hasTarget === 0 ? rawTarget !== 0xffff_ffff : !canonicalIdentity(rawTarget)) || (solid === 0 && hasTarget === 1)
    ) throw new Tf2CodecError("rocket result record is invalid")
    rocketTraceResults.push(Object.freeze({
      projectile: view.getUint32(item, true), tick: view.getBigUint64(item + 4, true), end,
      solid: solid === 1, sky: sky === 1, normal: hasNormal === 1 ? rawNormal : null,
      directTarget: hasTarget === 1 ? rawTarget : null,
    }))
  }
  at += rocketResultCount * 44

  requireBytes(moverResultCount * 52, "mover result")
  const moverResults: MoverResult[] = []
  for (let index = 0; index < moverResultCount; index += 1) {
    const item = at + index * 52, kind = data[item + 12], position = vector(view, item + 16), angles = vector(view, item + 28), carry = vector(view, item + 40)
    if (!canonicalIdentity(view.getUint32(item + 8, true)) || kind === undefined || kind < 1 || kind > 5 ||
      data[item + 13] !== 0 || data[item + 14] !== 0 || data[item + 15] !== 0 || !finite([...position, ...angles, ...carry])) {
      throw new Tf2CodecError("mover result record is invalid")
    }
    moverResults.push(Object.freeze({ requestId: view.getBigUint64(item, true), entity: view.getUint32(item + 8, true), kind, position, angles, carry }))
  }
  at += moverResultCount * 52

  requireBytes(collisionSnapshotLength, "Collision snapshot")
  const collisionSnapshot = decodeCollisionSnapshot(buffer, base + at, collisionSnapshotLength)
  at += collisionSnapshotLength
  requireBytes(jumpLength, "Jump")
  const jump = decodeJump(buffer, base + at, jumpLength)
  at += jumpLength
  requireBytes(movementTickLength,"Movement tick");const movementTick=decodeMovementTick(buffer,base+at,movementTickLength);at+=movementTickLength
  requireBytes(entityPresentationLength,"Entity presentation");const entityPresentation=decodeEntityPresentation(buffer,base+at,entityPresentationLength,ranges,at);at+=entityPresentationLength
  requireBytes(4, "bot count")
  const botCount = view.getUint32(at, true)
  at += 4
  if (botCount > 31) throw new Tf2CodecError("bot count exceeds its bound")
  requireBytes(botCount * 128, "bot")
  const bots: BotSnapshot[] = []
  let previousBot = 1
  for (let index = 0; index < botCount; index += 1) {
    const item = at + index * 128
    const readBot = (): BotSnapshot => {
    const identity = view.getUint32(item, true), botClass = data[item + 4], botTeam = data[item + 5]
    const lifecycle = data[item + 6], difficulty = data[item + 7], objective = data[item + 8]
    const health = view.getInt32(item + 12, true), maximumHealth = view.getInt32(item + 16, true)
    const target = view.getUint32(item + 20, true), area = view.getUint32(item + 24, true)
    const yawDegrees = view.getFloat32(item + 32, true), position = vector(view, item + 36), velocity = vector(view, item + 48)
    const pitchDegrees = view.getFloat32(item + 60, true), weapon = data[item + 64], reload = data[item + 65], carryingFlag = data[item + 66], animationRole = data[item + 67]
    const clip = view.getUint16(item + 68, true), reserve = view.getUint16(item + 70, true)
    const maximumClip = view.getUint16(item + 72, true), maximumReserve = view.getUint16(item + 74, true)
    const lastFireTick = view.getBigUint64(item + 96, true), respawnTick = view.getBigUint64(item + 104, true)
    const nextPrimaryTick = view.getBigUint64(item + 112, true), nextReloadTick = view.getBigUint64(item + 120, true)
    if (botClass === undefined || botClass < 1 || botClass > 9 || (botTeam !== 2 && botTeam !== 3)
      || (lifecycle !== 1 && lifecycle !== 2) || difficulty === undefined || difficulty > 3
      || objective === undefined || objective < 1 || objective > 10
      || data[item + 9] !== 0 || data[item + 10] !== 0 || data[item + 11] !== 0
      || animationRole === undefined || animationRole < 1 || animationRole > 3
      || weapon === undefined || weapon !== 0 && (!isTf2Weapon(weapon) || weapon >= 43 && weapon <= 45) || reload === undefined || reload > 3 || carryingFlag === undefined || carryingFlag > 1
      || health < 0 || maximumHealth < 1 || health > Math.max(maximumHealth, Math.floor(maximumHealth * 1.5 / 5) * 5) || clip > maximumClip || reserve > maximumReserve
      || (weapon === 0 && (reload !== 0 || clip !== 0 || reserve !== 0 || maximumClip !== 0 || maximumReserve !== 0 || nextPrimaryTick !== 0n || nextReloadTick !== 0n))
      || (lifecycle === 1 && respawnTick !== 0xffff_ffff_ffff_ffffn)
      || (lifecycle === 2 && (health !== 0 || respawnTick === 0xffff_ffff_ffff_ffffn))
      || !finite([yawDegrees, pitchDegrees, ...position, ...velocity])) throw new Tf2CodecError("bot snapshot record is invalid")
    const readBotWeapon = () => weapon === 0 ? null : Object.freeze({ identity: weapon as Tf2Weapon, reload: reload as 0 | 1 | 2 | 3, clip, reserve, maximumClip, maximumReserve, nextPrimaryTick, nextReloadTick })
    return Object.freeze({
      identity, class: botClass, team: botTeam, lifecycle, difficulty: difficulty as BotDifficulty,
      equippedItems: NO_EQUIPPED_ITEMS,
      objective: objective as BotSnapshot["objective"], health, maximumHealth,
      target: target === 0xffff_ffff ? null : target, area: area === 0xffff_ffff ? null : area,
      remainingPathAreas: view.getUint32(item + 28, true), yawDegrees, pitchDegrees, position, velocity,
      weapon: ranges ? ranges.read(`bot-weapon/${identity}`, 0, item + 64, 64, readBotWeapon) : readBotWeapon(),
      shots: view.getUint32(item + 76, true), hits: view.getUint32(item + 80, true), kills: view.getUint32(item + 84, true), deaths: view.getUint32(item + 88, true), captures: view.getUint32(item + 92, true),
      carryingFlag: carryingFlag === 1,
      animationRole: animationRole === 1 ? "PRIMARY" : animationRole === 2 ? "SECONDARY" : "MELEE",
      lastFireTick: lastFireTick === 0xffff_ffff_ffff_ffffn ? null : lastFireTick,
      respawnTick: respawnTick === 0xffff_ffff_ffff_ffffn ? null : respawnTick,
    })
    }
    const bot = ranges ? ranges.read("bot", index, item, 128, readBot) : readBot()
    if (bot.identity <= previousBot) throw new Tf2CodecError("bot snapshot record is invalid")
    previousBot = bot.identity
    bots.push(bot)
  }
  at += botCount * 128
  const readControlPoints = () => decodeControlPoints(buffer, base + at, bytes.byteLength - at)
  const pointResult = ranges ? ranges.section("control-points", at, bytes.byteLength - at, readControlPoints) : readControlPoints()
  at += pointResult.length
  const controlPoints = pointResult.points
  const readObjectives = () => decodeObjectives(buffer, base + at, bytes.byteLength - at)
  const objectiveResult = ranges ? ranges.section("objectives", at, bytes.byteLength - at, readObjectives) : readObjectives()
  at += objectiveResult.length
  const objectives = objectiveResult.objectives
  requireBytes(8, "pickup header")
  const metal = view.getUint32(at, true)
  const pickupCount = view.getUint32(at + 4, true)
  at += 8
  if (metal > 200 || pickupCount > 1024) throw new Tf2CodecError("pickup header exceeds its bound")
  requireBytes(pickupCount * 40, "pickup")
  const readPickups = () => {
  const pickups: MapPickup[] = []
  let previousPickup = -1
  for (let index = 0; index < pickupCount; index += 1) {
    const item = at + index * 40
    const identity = view.getUint32(item, true), kind = data[item + 4], size = data[item + 5]
    const team = data[item + 6], flags = data[item + 7]
    const origin = vector(view, item + 8), angles = vector(view, item + 20)
    const respawnTick = view.getBigUint64(item + 32, true)
    if (identity <= previousPickup || (kind !== 1 && kind !== 2) || size === undefined || size > 2
      || (team !== 0 && team !== 2 && team !== 3) || flags === undefined || flags > 3
      || ((flags & 1) !== 0 && respawnTick !== 0xffff_ffff_ffff_ffffn)
      || ((flags & 1) !== 0 && (flags & 2) !== 0) || !finite([...origin, ...angles])) {
      throw new Tf2CodecError("pickup snapshot record is invalid")
    }
    previousPickup = identity
    pickups.push(Object.freeze({
      identity,
      kind: kind === 1 ? "health" : "ammo",
      size: (["small", "medium", "full"] as const)[size]!,
      team: team === 0 ? null : team,
      available: (flags & 1) !== 0,
      disabled: (flags & 2) !== 0,
      origin,
      angles,
      respawnTick: respawnTick === 0xffff_ffff_ffff_ffffn ? null : respawnTick,
    }))
  }
  return Object.freeze(pickups)
  }
  const pickups = ranges ? ranges.read("pickups", 0, at, pickupCount * 40, readPickups) : readPickups()
  at += pickupCount * 40
  const scoreboardStart = at
  const readScoreboard = () => {
  requireBytes(12, "scoreboard")
  const redScore = view.getInt32(at, true), blueScore = view.getInt32(at + 4, true)
  const redCount = data[at + 8]!, blueCount = data[at + 9]!, playerCount = data[at + 10]!
  if (redCount > 32 || blueCount > 32 || playerCount < 1 || playerCount > 32 || data[at + 11] !== 0) {
    throw new Tf2CodecError("scoreboard header is invalid")
  }
  at += 12
  const scoreboardPlayers: ScoreboardPlayerSnapshot[] = []
  let previousPlayer = 0
  for (let index = 0; index < playerCount; index += 1) {
    requireBytes(33, "scoreboard player")
    const identity = view.getUint32(at, true), playerClass = data[at + 4]!, playerTeam = data[at + 5]!
    const alive = data[at + 6]!, fake = data[at + 7]!, nameLength = data[at + 32]!
    if (identity <= previousPlayer || playerClass < 1 || playerClass > 9 || playerTeam > 3
      || alive > 1 || fake > 1 || nameLength < 1 || nameLength > 31
      || (identity === 1) !== (index === 0) || (identity === 1) === (fake === 1)) {
      throw new Tf2CodecError("scoreboard player record is invalid")
    }
    requireBytes(33 + nameLength, "scoreboard player name")
    let name: string
    try { name = decoder.decode(data.subarray(at + 33, at + 33 + nameLength)) }
    catch { throw new Tf2CodecError("scoreboard player name is invalid") }
    scoreboardPlayers.push(Object.freeze({
      identity,
      name,
      team: playerTeam as Tf2Team,
      class: playerClass as Tf2Class,
      alive: alive === 1,
      fake: fake === 1,
      score: view.getInt32(at + 8, true),
      kills: view.getUint32(at + 12, true),
      deaths: view.getUint32(at + 16, true),
      captures: view.getUint32(at + 20, true),
      damage: view.getUint32(at + 24, true),
      assists: view.getUint32(at + 28, true),
    }))
    previousPlayer = identity
    at += 33 + nameLength
  }
  const scoreboard: ScoreboardSnapshot = Object.freeze({
    redScore, blueScore, redCount, blueCount, players: Object.freeze(scoreboardPlayers),
  })
  return Object.freeze({ length: at - scoreboardStart, scoreboard })
  }
  const scoreboardResult = ranges ? ranges.section("scoreboard", at, bytes.byteLength - at, readScoreboard) : readScoreboard()
  at = scoreboardStart + scoreboardResult.length
  const scoreboard = scoreboardResult.scoreboard
  // Cross-section joins must still run when a previously validated section is reused.
  if (scoreboard.players.filter((player) => player.team === 2).length !== scoreboard.redCount
    || scoreboard.players.filter((player) => player.team === 3).length !== scoreboard.blueCount
    || scoreboard.players.length !== botCount + 1
    || scoreboard.players.slice(1).some((player, index) => {
      const bot = bots[index]!
      return player.identity !== bot.identity || player.team !== bot.team || player.class !== bot.class
        || player.alive !== (bot.lifecycle === 1)
    })) throw new Tf2CodecError("scoreboard roster differs from player authority")
  requireBytes(4, "building header")
  const buildingCount = data[at]!, hasPlacement = data[at + 1]!
  at += 4
  if (buildingCount > 4 || hasPlacement > 1 || data[at - 2] !== 0 || data[at - 1] !== 0) throw new Tf2CodecError("building header is invalid")
  let placement: BuildingPlacement | null = null
  if (hasPlacement === 1) {
    requireBytes(20, "building placement")
    const kind = data[at]!, mode = data[at + 1]!, valid = data[at + 2]!
    const position = vector(view, at + 4), yawDegrees = view.getFloat32(at + 16, true)
    if (kind > 2 || mode > 1 || (kind !== 1 && mode !== 0) || valid > 1 || data[at + 3] !== 0 || !finite([...position, yawDegrees])) throw new Tf2CodecError("building placement is invalid")
    placement = Object.freeze({ object: Object.freeze({ kind: kind as Tf2BuildingKind, mode: mode as Tf2BuildingMode }), position, yawDegrees, valid: valid === 1 })
    at += 20
  }
  const buildings: BuildingSnapshot[] = []
  for (let index = 0; index < buildingCount; index += 1) {
    requireBytes(76, "building")
    const identity = view.getUint32(at, true), owner = view.getUint32(at + 4, true)
    const kind = data[at + 8]!, mode = data[at + 9]!, buildingTeam = data[at + 10]!, phase = data[at + 11]!, level = data[at + 12]!
    const maximumHealth = view.getUint16(at + 14, true), buildingHealth = view.getFloat32(at + 16, true)
    const target = view.getUint32(at + 32, true), position = vector(view, at + 36), yawDegrees = view.getFloat32(at + 48, true), construction = view.getFloat32(at + 52, true)
    if (kind > 2 || mode > 1 || (kind !== 1 && mode !== 0) || (buildingTeam !== 2 && buildingTeam !== 3) || phase > 3 || level < 1 || level > 3 || data[at + 13] !== 0
      || !finite([buildingHealth, ...position, yawDegrees, construction]) || buildingHealth <= 0 || buildingHealth > maximumHealth || construction < 0 || construction > 1) {
      throw new Tf2CodecError("building snapshot record is invalid")
    }
    const rechargeEndTick = view.getBigUint64(at + 56, true)
    buildings.push(Object.freeze({ identity, owner, object: Object.freeze({ kind: kind as Tf2BuildingKind, mode: mode as Tf2BuildingMode }), team: buildingTeam, phase: phase as BuildingSnapshot["phase"], level: level as BuildingSnapshot["level"], health: buildingHealth, maximumHealth,
      upgradeMetal: view.getUint16(at + 20, true), shells: view.getUint16(at + 22, true), maximumShells: view.getUint16(at + 24, true), rockets: view.getUint16(at + 26, true), maximumRockets: view.getUint16(at + 28, true), dispenserMetal: view.getUint16(at + 30, true),
      target: target === 0xffff_ffff ? null : target, position, yawDegrees, construction, rechargeEndTick: rechargeEndTick === 0xffff_ffff_ffff_ffffn ? null : rechargeEndTick, startedTick: view.getBigUint64(at + 64, true), timesUsed: view.getUint32(at + 72, true) }))
    at += 76
  }
  requireBytes(48,"Round rules header")
   const roundLength=((data[at+9]!&128)!==0?96:48)+view.getUint32(at+44,true)*12
  requireBytes(roundLength,"Round rules")
  const round=decodeRound(buffer,base+at,roundLength)
  at+=roundLength
  requireBytes(12, "Medi Gun state")
  const medigunCharge = view.getFloat32(at, true), rawMedigunTarget = view.getUint32(at + 4, true), medigunFlags = view.getUint32(at + 8, true)
  if (!Number.isFinite(medigunCharge) || medigunCharge < 0 || medigunCharge > 1 || medigunFlags > 1 || (rawMedigunTarget !== 0xffff_ffff && !bots.some(bot => bot.identity === rawMedigunTarget))) throw new Tf2CodecError("Medi Gun state is invalid")
  const medigunTarget = rawMedigunTarget === 0xffff_ffff ? null : rawMedigunTarget
  at += 12
  requireBytes(4,"combat decal count")
  const decalCount=count(view.getUint32(at,true),"combat decal")
  at+=4
  const combatDecals:CombatDecal[]=[]
  for(let index=0;index<decalCount;index++){
    requireBytes(20,"combat decal header")
    const identity=view.getUint32(at,true),face=view.getUint32(at+4,true)
    const vertices=count(view.getUint32(at+8,true),"combat decal vertex")
    const triangles=count(view.getUint32(at+12,true),"combat decal triangle")
    const referenceLength=view.getUint32(at+16,true)
    at+=20
    if(identity===0||vertices<3||vertices>128||triangles<1||triangles>128||referenceLength<1||referenceLength>1024)throw new Tf2CodecError("combat decal header is invalid")
    requireBytes(referenceLength,"combat decal reference")
    const reference=decoder.decode(data.subarray(at,at+referenceLength))
    at+=referenceLength
    requireBytes(vertices*32+triangles*12,"combat decal geometry")
    const positions=new Float32Array(vertices*3),normals=new Float32Array(vertices*3),uv=new Float32Array(vertices*2)
    for(let vertex=0;vertex<vertices;vertex++){
      for(let axis=0;axis<3;axis++)positions[vertex*3+axis]=view.getFloat32(at+axis*4,true)
      for(let axis=0;axis<3;axis++)normals[vertex*3+axis]=view.getFloat32(at+12+axis*4,true)
      for(let axis=0;axis<2;axis++)uv[vertex*2+axis]=view.getFloat32(at+24+axis*4,true)
      at+=32
    }
    const indices=new Uint32Array(triangles*3)
    for(let vertex=0;vertex<indices.length;vertex++){
      indices[vertex]=view.getUint32(at,true)
      if(indices[vertex]!>=vertices)throw new Tf2CodecError("combat decal triangle index is invalid")
      at+=4
    }
    if(!finite([...positions,...normals,...uv]))throw new Tf2CodecError("combat decal geometry is non-finite")
    combatDecals.push(Object.freeze({identity,face,reference,positions,normals,uv,indices}))
  }
  requireBytes(4, "actor cloak count")
  const cloakCount = view.getUint32(at, true); at += 4
  if (cloakCount > 32 || cloakCount !== Number(spy !== null) + bots.filter(bot => bot.class === 8).length) throw new Tf2CodecError("actor cloak roster differs")
  requireBytes(cloakCount * 28, "actor cloak records")
  const actorCloaks: ActorCloakState[] = []
  for (let index = 0; index < cloakCount; index++) {
    const identity = view.getUint32(at, true)
    const values = Array.from({ length: 6 }, (_, component) => view.getFloat32(at + 4 + component * 4, true))
    if (identity < 1 || (index > 0 && identity <= actorCloaks[index - 1]!.identity) || (identity === 1 ? spy === null || (health > 0 && values[2] !== spy.invisibility) : !bots.some(bot => bot.identity === identity && bot.class === 8)) || values.some(value => !Number.isFinite(value) || value < 0 || value > 1)) throw new Tf2CodecError("actor cloak record is invalid")
    actorCloaks.push(Object.freeze({ identity, localFactor: values[0]!, worldFactor: values[1]!, rawFactor: values[2]!, playerTint: Object.freeze(values.slice(3)) as readonly [number, number, number] }))
    at += 28
  }
  const readEquipment = (key: string) => {
    const decode = () => {
      const result = decodeEquippedItems(view, at)
      return { items: result.items, length: result.end - at }
    }
    const result = ranges ? ranges.section(key, at, bytes.byteLength - at, decode) : decode()
    at += result.length
    return result.items
  }
  const equippedItems = readEquipment("equipment")
  const equippedBots = bots.map((bot, index) => {
    const equippedItems = readEquipment(`bot-equipment/${bot.identity}`)
    if (at + 24 > bytes.byteLength) throw new Tf2CodecError("bot condition record is truncated")
    const decode = () => {
      const conditions = Object.freeze(Array.from({ length: 5 }, (_, word) => view.getUint32(at + word * 4, true)))
      const overheadHeight = view.getFloat32(at + 20, true)
      if (conditions[4]! >>> 3 || !Number.isFinite(overheadHeight) || overheadHeight <= 0) throw new Tf2CodecError("bot condition record is invalid")
      return Object.freeze({ conditions, overheadHeight })
    }
    const condition = ranges ? ranges.read("bot-conditions", index, at, 24, decode) : decode()
    at += 24
    const compose = () => Object.freeze({ ...bot, equippedItems, ...condition })
    return ranges ? ranges.compose(`bot-projection/${bot.identity}`, [bot, equippedItems, condition], compose) : compose()
  })
  requireBytes(12, "view angle correction")
  const viewAngleOffset = vector(view, at)
  if (!finite(viewAngleOffset)) throw new Tf2CodecError("view angle correction is invalid")
  at += 12
  requireBytes(8, "player weapon counters")
  const decapitations = view.getInt32(at, true), revengeCrits = view.getInt32(at + 4, true)
  if (decapitations < 0 || revengeCrits < 0 || revengeCrits > 35) throw new Tf2CodecError("player weapon counters are invalid")
  at += 8
  requireBytes(4, "weapon crosshair scale")
  const weaponCrosshairScale = view.getFloat32(at, true)
  if (!Number.isFinite(weaponCrosshairScale) || weaponCrosshairScale < 0) throw new Tf2CodecError("weapon crosshair scale is invalid")
  at += 4
  requireBytes(108, "soundscape selection")
  const readSoundscape = (): SoundscapeSelection => {
    const entity = view.getInt32(at, true)
    const soundscape = view.getInt32(at + 4, true)
    const positionBits = view.getUint32(at + 8, true)
    const positions = Array.from({ length: 8 }, (_, index) => Object.freeze(vector(view, at + 12 + index * 12)))
    if (entity < 0 || soundscape < -1 || positionBits > 255 || positions.some(position => !finite(position))) {
      throw new Tf2CodecError("soundscape selection is invalid")
    }
    return Object.freeze({ entity, soundscape, positionBits, positions: Object.freeze(positions) })
  }
  const soundscape = ranges ? ranges.section("soundscape", at, 108, readSoundscape) : readSoundscape()
  at += 108
  if(at!==bytes.byteLength)throw new Tf2CodecError("snapshot has trailing bytes")
  if(entityPresentation.collisionRevision!==collisionSnapshot.identity)throw new Tf2CodecError("Entity presentation revision join is invalid")

  const tick = view.getBigUint64(8, true)
  const frozenProjectiles = Object.freeze(projectiles)
  const frozenProjectileEvents = Object.freeze(projectileEvents)
  return Object.freeze({
    tick,
    soundscape,
    actorCloaks: Object.freeze(actorCloaks),
    equippedItems,
    decapitations,
    revengeCrits,
    weaponCrosshairScale,
    class: tf2Class as Tf2Class,
    team,
    weapon: weapon === 0 ? null : weapon as Tf2Weapon,
    playerFlags,
    inWater: (playerFlags & 0x400) !== 0,
    health,
    maximumHealth,
    spy,
    lifecycle: lifecycle as 1 | 2 | 3 | 4,
    conditions,
    respawnTouchCount: view.getUint32(52, true),
    movement,
    viewAngleOffset,
    movementTick,
    position: movement.position,
    velocity: movement.velocity,
    grounded: movement.grounded,
    crouched: movement.crouchPhase >= 2,
    loadout: ranges ? ranges.array("loadout", loadout) : Object.freeze(loadout),
    flamePoints: Object.freeze(flamePoints),
    shotgunPellets: Object.freeze(shotgunPellets),
    flameFiring: flameFlags === 1,
    projectiles: frozenProjectiles,
    projectileEvents: frozenProjectileEvents,
    projectileTimeline: Object.freeze([
      Object.freeze({ tick, projectiles: frozenProjectiles, events: frozenProjectileEvents }),
    ]),
    entityTransforms: Object.freeze(entityTransforms),
    entityEvents: Object.freeze(entityEvents),
    objectives,
    controlPoints,
    round,
    jump,
    events: Object.freeze(events),
    combatDecals:Object.freeze(combatDecals),
    activities: Object.freeze(activities),
    lifecycleEvents: Object.freeze(lifecycleEvents),
    physicsRequests: Object.freeze(physicsRequests),
    rocketTraceRequests: Object.freeze(rocketTraceRequests),
    radiusDamageRequests: Object.freeze(radiusDamageRequests),
    moverRequests: Object.freeze(moverRequests),
    contactReconcileRequests: Object.freeze(contactReconcileRequests),
    mapEffects: Object.freeze(mapEffects),
    regenerateAnimationEvents: Object.freeze(regenerateAnimationEvents),
    randomState,
    randomDraws: Object.freeze(randomDraws),
    audioEvents: Object.freeze(audioEvents),
    rocketTraceResults: Object.freeze(rocketTraceResults),
    moverResults: Object.freeze(moverResults),
    collisionSnapshot,
    entityPresentation,
    authorityBlockers: Object.freeze(authorityBlockers),
    bots: ranges ? ranges.array("bots", equippedBots) : Object.freeze(equippedBots),
    pickups: Object.freeze(pickups),
    buildings: Object.freeze(buildings),
    placement,
    metal,
    scoreboard,
    medigunCharge,
    medigunTarget,
    medigunReleasing: medigunFlags === 1,
  })
}

export async function mapDerivedKey(
  bspSha256: string,
  profile: 0 | 1,
  renderLevel: 0 | 1 | 2,
  compilerSha256: string,
  resourceRootSha256: string,
): Promise<string> {
  if (!HASH.test(bspSha256) || !HASH.test(compilerSha256) || !HASH.test(resourceRootSha256)
    || (renderLevel === 2) !== (profile === 1)) {
    throw new Tf2CodecError("BSP, compiler, resource root, or render profile identity is invalid")
  }
  const identity = new TextEncoder().encode(
    `playsrc-map-runtime-11\n${bspSha256}\n${compilerSha256}\n${profile}\n${renderLevel}\n${resourceRootSha256}\n`,
  )
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", identity))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
