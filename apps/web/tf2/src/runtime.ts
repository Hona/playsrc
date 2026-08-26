import { fetchImmutableObject, openDerivedObjectCache, type DerivedObjectCache } from "@playsrc/asset-store/browser"
import { chunksForRole, parseResourceCatalogBytes, parseResourceGraphBytes, parseResourceSet, resourceChunkObject, selectCatalogTarget, type ResourceCatalog, type ResourceGraph, type ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { createAudioSystem, SoundRegistry, SourceAudioWorld } from "@playsrc/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { Tf2WorkerClient, mergePublicationSnapshots, type CoverageSample, type LoadedGame, type SimulationPublication, type VisibilityResult } from "@playsrc/game-tf2-browser"
import { initializeTf2GameUiIntegration, type Tf2GameUiIntegration } from "@playsrc/game-tf2-browser/gameui-integration"
import type { Tf2GameUiRequest, Tf2LoadingPhase } from "@playsrc/game-tf2-browser/gameui"
import {
  initializeTf2LocalMatchPresentation,
  type Tf2LocalMatchLaunch,
  type Tf2LocalMatchPresentation,
} from "@playsrc/game-tf2-browser/local-match"
import { initializeTf2HudIntegration, type Tf2HudIntegration } from "@playsrc/game-tf2-browser/hud-integration"
import {
  initializeTf2ClassSelectionIntegration,
  tf2ClassSelectionByName,
  type Tf2ClassIdentity,
  type Tf2ClassSelectionIntegration,
  type Tf2ClassSelectionModelPanel,
  type Tf2ClassSelectionRequest,
} from "@playsrc/game-tf2-browser/class-selection"
import {
  initializeTf2TeamSelectionIntegration,
  type Tf2TeamSelectionIntegration,
  type Tf2TeamSelectionModelPanel,
  type Tf2TeamSelectionRequest,
} from "@playsrc/game-tf2-browser/team-selection"
import {
  TF2_BROWSER_SETTINGS_STORAGE_KEY,
  initializeTf2BrowserSettings,
  initializeTf2OptionsPresentation,
  type Tf2BrowserSettings,
  type Tf2OptionsPresentation,
} from "@playsrc/game-tf2-browser/settings-integration"
import { createTf2PresentationRandom, initializeTf2VguiResources, type Tf2PresentationRandom, type Tf2VguiResources } from "@playsrc/game-tf2-browser/ui-integration"
import {
  TF2_CROSSHAIR_SETTINGS,
  adaptTf2Scoreboard,
  tf2CrosshairHudValues,
  tf2CrosshairSettings,
  tf2HudAvailable,
  tf2HudUnavailable,
  type SessionHudContext,
  type Tf2CrosshairSettings,
  type Tf2HudBinding,
  type Tf2HudFreezePanel,
  type Tf2HudScoreboard,
} from "@playsrc/game-tf2-browser/hud"
import {
  createTf2StartupController,
  validateTf2StartupDescriptor,
  type Tf2HiddenMenu,
  type Tf2StartupController,
  type Tf2StartupDescriptor,
  type Tf2StartupMediaSession,
  type Tf2StartupState,
} from "@playsrc/game-tf2-browser/startup-presentation"
import {
  createTf2LoadingPresentation,
  initializeTf2LoadingVguiRuntime,
  resolveTf2LoadingBackground,
  type Tf2LoadingBackgroundResult,
  type Tf2LoadingPresentation,
  type Tf2LoadingVguiRuntime,
} from "@playsrc/game-tf2-browser/loading-presentation"
import { encodeCommand, mapDerivedKey, type BotConfiguration, type BotQuotaMode, type BotRequest, type Snapshot, type Tf2Class, type Tf2Team, type Tf2Weapon, type Tf2BuildingRequest } from "@playsrc/game-tf2-browser/codec"
import { blueprintModel, buildingModel, initializeTf2EngineerPresentation, type Tf2EngineerPresentation } from "@playsrc/game-tf2-browser/engineer"
import { TF2_CLASS_NAMES, tf2ClassFromName, tf2ClassPresentation } from "@playsrc/game-tf2-browser/class"
import { parsePresentationArtifacts, type PresentationArtifacts } from "@playsrc/game-tf2-browser/artifacts"
import {
  createParticleBatchEncoder,
  createProjectilePresentationMapper,
  createViewmodelPresenter,
  decodeModelPoseOutput,
  encodeModelPoseBatch,
  projectileFrame,
  projectileModels,
  hitscanMuzzleParticles,
  combatImpactParticles,
  sourceViewOrientation,
  tf2Audio,
  tf2Camera,
  transformAttachment,
  type ModelPoseRequest,
  type PosedModel,
  type ProjectileParticleRequest,
  type Tf2AudioRequest,
} from "@playsrc/game-tf2-browser/presentation"
import { decodeParticleRenderOutput } from "@playsrc/particle"
import { createRenderer, SOURCE_LDR, SOURCE_PC_INTEGER_HDR, type Camera, type Frame, type MaterialStateInput } from "@playsrc/rendering"
import {
  initializeClientDiagnostics,
  initializeDeveloperConsole,
  type ClientDiagnosticMode,
  type ClientDiagnostics,
  type ConsoleCompletionSuggestion,
  type ConsoleCatalog,
  type ConsoleRequest,
  type DeveloperConsole,
} from "@playsrc/vgui"
import { bytesToHex } from "@noble/hashes/utils.js"
import { sha256 } from "@noble/hashes/sha2.js"
import {consoleLimits,resolveConfiguredConsoleResources,type ResolvedConsoleResources} from "./console-resources"
import { loadBrowserConfiguration, type BrowserConfiguration, type BrowserTargetConfiguration } from "./config"
import { PhysicalBindingIndex, PhysicalButtonState, applyPointerDelta, rawPointerMovementUnsupported, rebasePointerYaw, sourceMouseButtonCode, type PhysicalBinding } from "./input"
import { TF2_SELECTED_OPTIONS, type AdapterRequestResult, type SettingsAdapterRequest } from "@playsrc/settings"
import { SimulationClockQueue } from "./simulation-clock"
import {
  initializeBrowserPresentationViewportOwner,
  type ApplicationPresentationViewport,
  type PresentationViewportOwner,
} from "./presentation-viewport"
import { RequiredParticleDisplayQueue } from "./particle-display"
import { CanvasFrameDiagnostics } from "./frame-diagnostics"
import { GAME_UI_FRAME_OWNER, HUD_FRAME_OWNER, LOADING_FRAME_OWNER, OPTIONS_FRAME_OWNER, visibleFrameOwners } from "./frame-owners"
import { ExactSkyVisibilityCache, type SkyVisibilityIdentity } from "./visibility-cache"
import {
  ApplicationFrameClock,
  ApplicationOperationLedger,
  PredictedEyeInterpolation,
  composeViewmodelTransform,
  currentPresentationGeneration,
  routeApplicationEscape,
  selectAuthoredSky,
  type ApplicationOperation,
} from "./application-lifecycle"

const MAX_EXTERNAL_BYTES = 536_870_912
const SIMULATION_SAMPLE_INTERVAL_SECONDS = 0.015
const MAX_REQUIRED_PARTICLE_DISPLAY_FRAMES = 256
const BOT_MODEL_IDENTITY_BASE = 0x6000_0000
const OBJECTIVE_MODEL_IDENTITY_BASE = 0x6100_0000
const BUILDING_BLUEPRINT_IDENTITY = 0x5fff_ffff
const PARTICLE_SYSTEMS = new Set([
  "rockettrail",
  "rocketbackblast",
  "stickybombtrail_red",
  "stickybombtrail_blue",
  "stickybomb_pulse_red",
  "stickybomb_pulse_blue",
  "muzzle_pipelauncher",
  "muzzle_scattergun",
  "muzzle_pistol",
  "muzzle_shotgun",
  "new_flame",
  "pyro_blast",
  "muzzle_revolver",
  "ExplosionCore_Wall",
  "ExplosionCore_MidAir",
  "blood_impact_red_01",
  "blood_spray_red_01",
  "blood_spray_red_01_far",
  "crit_text",
  ...["scattergun", "pistol", "shotgun"].flatMap((weapon) =>
    ["red", "blue"].flatMap((team) =>
      [`bullet_${weapon}_tracer01_${team}`, `bullet_${weapon}_tracer01_${team}_crit`])),
  "bullet_tracer01_red",
  "bullet_tracer01_blue",
  "bullet_tracer01_red_crit",
  "bullet_tracer01_blue_crit",
])
const SOUND_PATHS = [
  "sound/items/smallmedkit1.wav",
  "sound/items/gunpickup2.wav",
  "sound/items/regenerate.wav",
  "sound/items/spawn_item.wav",
  "sound/weapons/rocket_shoot.wav",
  "sound/weapons/stickybomblauncher_shoot.wav",
  "sound/weapons/quake_rpg_fire_remastered.wav",
  "sound/weapons/quake_explosion_remastered.wav",
  "sound/weapons/explode1.wav",
  "sound/weapons/explode2.wav",
  "sound/weapons/explode3.wav",
  "sound/weapons/pipe_bomb1.wav",
  "sound/weapons/pipe_bomb2.wav",
  "sound/weapons/pipe_bomb3.wav",
  "sound/weapons/scatter_gun_shoot.wav",
  "sound/weapons/pistol_shoot.wav",
  "sound/weapons/cbar_miss1.wav",
  "sound/weapons/bat_hit.wav",
  "sound/weapons/cbar_hit1.wav",
  "sound/weapons/cbar_hit2.wav",
  "sound/weapons/scatter_gun_worldreload.wav",
  "sound/weapons/pistol_worldreload.wav",

  "sound/weapons/shotgun_shoot.wav",
  "sound/weapons/shotgun_worldreload.wav",
  "sound/weapons/shovel_swing.wav",
  "sound/weapons/axe_hit_flesh1.wav",
  "sound/weapons/axe_hit_flesh2.wav",
  "sound/weapons/axe_hit_flesh3.wav",
  "sound/weapons/minigun_wind_up.wav",
  "sound/weapons/minigun_wind_down.wav",
  "sound/weapons/minigun_spin.wav",
  "sound/weapons/minigun_shoot.wav",
  "sound/weapons/bat_draw_swoosh1.wav",
  "sound/weapons/bat_draw_swoosh2.wav",
  "sound/weapons/cbar_hitbod1.wav",
  "sound/weapons/cbar_hitbod2.wav",
  "sound/weapons/cbar_hitbod3.wav",
  "sound/weapons/fist_hit_world1.wav",
  "sound/weapons/fist_hit_world2.wav",
  "sound/weapons/sniper_shoot.wav",
  "sound/weapons/smg_shoot.wav",
  "sound/weapons/smg_worldreload.wav",
  "sound/weapons/machete_swing.wav",
  "sound/weapons/shotgun_empty.wav",
  "sound/weapons/pistol/pistol_empty.wav",
  "sound/weapons/wrench_swing.wav",
  "sound/weapons/wrench_hit_world.wav",
  "sound/weapons/flame_thrower_start.wav",
  "sound/weapons/flame_thrower_loop.wav",
  "sound/weapons/flame_thrower_end.wav",
  "sound/weapons/flame_thrower_airblast.wav",
  "sound/weapons/bottle_hit_flesh1.wav",
  "sound/weapons/bottle_hit_flesh2.wav",
  "sound/weapons/bottle_hit_flesh3.wav",
  "sound/weapons/bottle_hit1.wav",
  "sound/weapons/bottle_hit2.wav",
  "sound/weapons/bottle_hit3.wav",


  "sound/weapons/revolver_shoot.wav",
  "sound/weapons/revolver_worldreload.wav",
  "sound/weapons/knife_swing.wav",
  "sound/weapons/blade_hit1.wav",
  "sound/weapons/blade_hit2.wav",
  "sound/weapons/blade_hit3.wav",
  "sound/weapons/blade_hitworld.wav",
  "sound/player/spy_cloak.wav",
  "sound/player/spy_uncloak.wav",
  "sound/ui/hitsound.wav",
  "sound/ui/killsound.wav",
  "sound/player/crit_hit.wav",
  "sound/player/crit_hit2.wav",
  "sound/player/crit_hit3.wav",
  "sound/player/crit_hit4.wav",
  "sound/player/crit_hit5.wav",
  "sound/physics/plastic/plastic_box_impact_hard1.wav",
  "sound/physics/plastic/plastic_box_impact_hard2.wav",
  "sound/physics/plastic/plastic_box_impact_hard3.wav",
  "sound/physics/concrete/concrete_impact_bullet1.wav",
  "sound/physics/concrete/concrete_impact_bullet2.wav",
  "sound/physics/concrete/concrete_impact_bullet3.wav",
  "sound/physics/concrete/concrete_impact_bullet4.wav",
  "sound/physics/wood/wood_solid_impact_bullet1.wav",
  "sound/physics/wood/wood_solid_impact_bullet2.wav",
  "sound/physics/wood/wood_solid_impact_bullet3.wav",
  "sound/physics/wood/wood_solid_impact_bullet4.wav",
  "sound/physics/wood/wood_solid_impact_bullet5.wav",
  "sound/physics/metal/metal_solid_impact_bullet1.wav",
  "sound/physics/metal/metal_solid_impact_bullet2.wav",
  "sound/physics/metal/metal_solid_impact_bullet3.wav",
  "sound/physics/metal/metal_solid_impact_bullet4.wav",
  "sound/physics/surfaces/sand_impact_bullet1.wav",
  "sound/physics/surfaces/sand_impact_bullet2.wav",
  "sound/physics/surfaces/sand_impact_bullet3.wav",
  "sound/physics/surfaces/sand_impact_bullet4.wav",
  "sound/physics/glass/glass_impact_bullet1.wav",
  "sound/physics/glass/glass_impact_bullet2.wav",
  "sound/physics/glass/glass_impact_bullet3.wav",
  "sound/physics/glass/glass_impact_bullet4.wav",
  "sound/physics/flesh/flesh_impact_bullet1.wav",
  "sound/physics/flesh/flesh_impact_bullet2.wav",
  "sound/physics/flesh/flesh_impact_bullet3.wav",
  "sound/physics/flesh/flesh_impact_bullet4.wav",
  "sound/physics/flesh/flesh_impact_bullet5.wav",
] as const
const CTF_SOUND_PATHS = [
  "sound/vo/intel_enemystolen.mp3",
  "sound/vo/intel_enemystolen2.mp3",
  "sound/vo/intel_enemystolen3.mp3",
  "sound/vo/intel_enemystolen4.mp3",
  "sound/vo/intel_enemydropped.mp3",
  "sound/vo/intel_enemydropped2.mp3",
  "sound/vo/intel_enemycaptured.mp3",
  "sound/vo/intel_enemycaptured2.mp3",
  "sound/vo/intel_enemyreturned.mp3",
  "sound/vo/intel_enemyreturned2.mp3",
  "sound/vo/intel_enemyreturned3.mp3",
  "sound/vo/intel_teamstolen.mp3",
  "sound/vo/intel_teamdropped.mp3",
  "sound/vo/intel_teamdropped2.mp3",
  "sound/vo/intel_teamcaptured.mp3",
  "sound/vo/intel_teamreturned.mp3",
  "sound/misc/your_team_won.mp3",
  "sound/misc/your_team_lost.mp3",
] as const

export type ApplicationView = Readonly<{
  phase: "Startup" | "MainMenu" | "Loading" | "Ready" | "Replacing" | "Failed" | "Closed"
  generation?: number
  detail: string
  gameUi: "main-menu" | "loading" | "in-game" | "pause" | "disconnecting" | "failure"
  hudProbe?: string
  hudAnimationTrace?: string
  hudOperationProbe?: string
  hudPresentationProbe?: string
  scoreboardVisible?: boolean
  scoreboardProbe?: string
  optionsVisible?: boolean
  localMatchVisible?: boolean
  localMatchEntry?: "training" | "create-server"
  localMatchSettings?: string
  settingsPersistence?: "absent" | "loaded" | "rejected" | "stored"
  settingsApply?: string
  hostRequest?: "quit"
  presentationRandomState?: string
  presentationCharacter?: string
  cache?: "hit" | "stored"
  pointerLocked: boolean
  pointerMovement?: "raw" | "adjusted"
  consoleVisible: boolean
  classSelectionVisible?: boolean
  classSelectionTeam?: number
  classSelectionSelected?: number
  classSelectionModels?: string
  teamSelectionVisible?: boolean
  teamSelectionLocal?: number
  teamSelectionRedCount?: number
  teamSelectionBlueCount?: number
  teamSelectionModels?: string
  blockers: readonly string[]
  fireEvents: number
  explosionEvents: number
  camera?: Camera
  initialView?: LoadedGame["initialView"]
  environment?: PresentationArtifacts["environment"]
  particleRenderItems?: number
  flamePoints?: number
  environmentDrawables?: number
  visibleDecalFragments?: number
  movement?: Snapshot["movement"]
  playerFlags?: Snapshot["playerFlags"]
  inWater?: Snapshot["inWater"]
  movementTick?: Snapshot["movementTick"]
  viewmodelPose?: Readonly<{ activity: string; sequence: number; cycle: number; primitives: number; events: number }>
  modelProbes?: readonly Readonly<{ model: string; sequence: number; primitives: number; vertices: number }>[]
  audioVoices?: readonly number[]
  snapshotTick?: string
  projectileStates?: string
  decalProbe?: string
  combatDecals?: number
  modelOccurrenceCount?: number
  particleProbe?: string
  audioStarts?: readonly string[]
  viewmodelProjection?: string
  viewmodelDepthRange?: string
  viewmodelViewportRestored?: boolean
  viewmodelWorldDepthIsolated?: boolean
  viewmodelActivities?: readonly string[]
  viewmodelSequences?: string
  crouchHistory?: readonly string[]
  viewmodelTimelineProbes?: readonly string[]
  modelMatrices?: readonly Readonly<{ entity: number; model: string; matrix: readonly number[] }>[]
  decalStateProbe?: Readonly<{ materials: number; exact: number }>
  weaponTrace?: string
  spyProbe?: string
  spyWatchActivity?: string
  authorityTrace?: string
  entityTrace?: string
  modelMaterialProbe?: string
  randomAudioProbe?: string
  collisionMoverProbe?: string
  simulationProbe?: string
  brushModelProbe?: string
  waterPlanProbe?: string
  waterPasses?: readonly string[]
  waterStateRestored?: boolean
  waterNormalFrame?: number
  worldMaterialFrames?: string
  reloadHistory?: readonly string[]
  fireTickHistory?: readonly string[]
  performanceProbe?: string
  performanceDetailProbe?: string
  loadPerformanceProbe?: string
  displayFrame?: number
  displayViewRevision?: number
  displayPreparedRevision?: number
  lockerProbe?: string
  objectiveProbe?: string
  objectiveEventProbe?: string
  roundProbe?: string
  botCount?: number
  botProbe?: string
  pickupCount?: number
  pickupProbe?: string
  metal?: number
  buildingCount?: number
  buildingProbe?: string
  engineerMetal?: number
  engineerMenu?: "build" | "destroy" | "none"
  placementProbe?: string
  unsupportedState?: "StickyPhysicsSolverUnavailable" | "GrenadePhysicsSolverUnavailable"
  startupState?: Tf2StartupState["kind"]
  loadingProgress?: number
  loadingStatus?: string
  loadingBackground?: "map-photo" | "configured-generic"
  startupGestures?: number
  menuPreparation?: string
  bootstrapLoading?: boolean
  bootstrapProgress?: number
  startupMutedFallback?: boolean
}>

type Renderer = Awaited<ReturnType<typeof createRenderer>>
type Audio = ReturnType<typeof createAudioSystem>
type ProjectileMapper = ReturnType<typeof createProjectilePresentationMapper>
type LockerAnimationState=Readonly<{openTick:bigint;closeTick:bigint;body:number;openAnimation:"open"|"close";closeAnimation:"open"|"close"}>
type PreparedPresentation=Readonly<{
  generation:number
  revision:number
  snapshot:Snapshot
  publication:SimulationPublication
  visibility:VisibilityResult
  visibilityPosition:readonly[number,number,number]
  visibilityYaw:number
  visibilityPitch:number
  frame:Omit<Frame,"camera"|"visibility"|"deltaSeconds">
  modelMilliseconds:number
  projectileMilliseconds:number
  particleMilliseconds:number
  particleDecodeMilliseconds:number
  audioMilliseconds:number
  particleOutputBytes:number
}>

function changedCamera(previous: Camera | undefined, next: Camera): boolean {
  return previous === undefined
    || previous.position.some((value, index) => !Object.is(value, next.position[index]))
    || !Object.is(previous.yawDegrees, next.yawDegrees)
    || !Object.is(previous.pitchDegrees, next.pitchDegrees)
}

export class Tf2Application {
  #canvas: HTMLCanvasElement
  readonly #presentationRoot: HTMLElement
  readonly #vguiRoot: HTMLElement
  readonly #gameUiRoot: HTMLElement
  readonly #hudRoot: HTMLElement
  readonly #engineerRoot: HTMLElement
  readonly #classSelectionRoot: HTMLElement
  readonly #teamSelectionRoot: HTMLElement
  readonly #optionsRoot: HTMLElement
  readonly #localMatchRoot: HTMLElement
  readonly #loadingRoot: HTMLElement
  readonly #startupRoot: HTMLElement
  readonly #startupVideo: HTMLVideoElement
  readonly #publish: (view: ApplicationView) => void
  readonly #viewportOwner: PresentationViewportOwner
  #presentationViewport?: ApplicationPresentationViewport
  #configuration?: BrowserConfiguration
  #resourceCatalog?: ResourceCatalog
  #activeTarget?: BrowserTargetConfiguration
  #loadingTarget?: BrowserTargetConfiguration
  #resourceGraph?: ResourceGraph
  #dependencies: Uint8Array = new Uint8Array()
  #dependencyEntries = new Map<string, Uint8Array>()
  #sharedDependencyEntries = new Map<string, Uint8Array>()
  #cache?: DerivedObjectCache
  #client?: Tf2WorkerClient
  #renderer?: Renderer
  #audio?: Audio
  #audioContext?: AudioContext
  #audioRegistry?: SoundRegistry
  #audioWorld?: SourceAudioWorld
  #audioBuffers = new Map<string, AudioBuffer>()
  #audioStarts: string[] = []
  #pendingAudioRequests: Tf2AudioRequest[] = []
  #lockerAnimations = new Map<number, LockerAnimationState>()
  #reloadHistory:string[]=[]
  #fireTickHistory:string[]=[]
  #wasmCalls={observe:0,models:0,visibility:0,particles:0}
  #maximumScheduledSamples=0
  #maximumPublicationTicks=0
  #phaseTimings=[0,0,0,0,0]
  #pendingProjectileTimeline:Snapshot["projectileTimeline"][number][]=[]
  #audioRunning = false
  #artifacts?: PresentationArtifacts
  #projectiles?: ProjectileMapper
  #viewmodels?: ReturnType<typeof createViewmodelPresenter>
  #viewmodelClass?: Snapshot["class"]
  #watchActivity?: "ACT_VM_DRAW" | "ACT_VM_IDLE" | "ACT_VM_HOLSTER"
  #watchActivityTick = 0n
  #attachments = new Map<number, ReadonlySet<string>>()
  #attachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #fireAttachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #particleBatches = createParticleBatchEncoder()
  #pyroFlameEffect?: string
  #combatTracerCount = 0
  #combatDecalCount = 0
  #pyroEffectSerial = 0
  #console?: DeveloperConsole
  #pendingConsoleOutput: Readonly<{ text: string; developer: boolean }>[] = []
  #operationWatchdog?: ReturnType<typeof setTimeout>
  readonly #operations = new ApplicationOperationLedger()
  #operation = this.#operations.begin()
  readonly #frameClock = new ApplicationFrameClock()
  readonly #predictedEye = new PredictedEyeInterpolation()
  #diagnostics?: ClientDiagnostics
  #consoleResources?:ResolvedConsoleResources
  #uiResources?: Tf2VguiResources
  #presentationRandom?: Tf2PresentationRandom
  #gameUi?: Tf2GameUiIntegration
  #startup?: Tf2StartupController
  #loadingPresentation?: Tf2LoadingPresentation
  #loadingVgui?: Tf2LoadingVguiRuntime
  #loadingBackground: Extract<Tf2LoadingBackgroundResult, { ok: true }> | null = null
  #loadingPresentationGeneration = 0
  #menuPresentationDestroyed = false
  #menuRevealed = false
  #startupGestures = 0
  #startupMutedFallback = false
  #bootstrapExpectedObjects = new Set<string>()
  #bootstrapObjectProgress = new Map<string, Readonly<{ loaded: number; total: number }>>()
  readonly #gameUiRequestTasks = new Set<number>()
  #hudIntegration?: Tf2HudIntegration
  #engineer?: Tf2EngineerPresentation
  #classSelection?: Tf2ClassSelectionIntegration
  #classSelectionModelPanels: readonly Tf2ClassSelectionModelPanel[] = Object.freeze([])
  #classSelectionRenderTask?: Promise<void>
  #classSelectionRenderRevision = 0
  #teamSelection?: Tf2TeamSelectionIntegration
  #teamSelectionModelPanels: readonly Tf2TeamSelectionModelPanel[] = Object.freeze([])
  #teamSelectionRenderTask?: Promise<void>
  #teamSelectionRenderRevision = 0
  #pendingClassSelectionTeam?: 2 | 3
  readonly #teamSelectionPoses = new Map<string, PosedModel>()
  readonly #teamSelectionAnimations = new Map<string, Readonly<{ sequence: string; startedSeconds: number; previousSeconds: number }>>()
  #teamAdmission?: Readonly<{ generation: number; resolve(): void; reject(error: Error): void }>
  #hudRootCounts?: Readonly<{ playerStatus: number; ammo: number }>
  #hudContext?: SessionHudContext
  #hudContextIdentity = -1
  #hudScoreboardIdentity = ""
  #scoreboardVisible = false
  #scoreboardPingAsText = false
  #playerClassUsePlayerModel = true
  #crosshairSettings?: Tf2CrosshairSettings
  #settings?: Tf2BrowserSettings
  #options?: Tf2OptionsPresentation
  #localMatch?: Tf2LocalMatchPresentation
  #pendingLocalMatch?: Tf2LocalMatchLaunch
  #loaded?: LoadedGame
  #snapshot?: Snapshot
  #generation = 0
  #yaw = 0
  #pitch = 0
  #pointerMovementX = 0
  #viewRevision = 0
  #mouseViewRevision=0
  #authoritativeViewRevision=0
  #pointerMovement?: "raw" | "adjusted"
  #pointerRequestPending=false
  #resumePointerOnAcknowledge=false
  #lastPointerLockFailure="Pointer lock failed"
  readonly #bindings = new PhysicalBindingIndex()
  readonly #bindingValues = new Map<string, PhysicalBinding>()
  readonly #buttons = new PhysicalButtonState()
  #jumpPressed = false
  #firePressed = false
  #detonatePressed = false
  #reloadPressed = false
  #dropItem = false
  #selectClass: Tf2Class | 12 | undefined
  #selectWeapon: Tf2Weapon | undefined
  #disguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | undefined
  #modeRequest: 0 | 1 | undefined
  #botRequest: BotRequest | undefined
  #buildingRequest: Tf2BuildingRequest | undefined
  #botConfiguration: BotConfiguration | undefined
  #activeBotConfiguration: BotConfiguration | undefined
  #botDifficulty: 0 | 1 | 2 | 3 = 1
  #coverageSamples:readonly CoverageSample[]=Object.freeze([])
  #developer = 1
  #showFps: ClientDiagnosticMode = 0
  #showPos: ClientDiagnosticMode = 0
  #renderLevel: 0 | 1 | 2 = 2
  #mapIdentity = ""
  #environmentDrawables = 0
  #modelProbes: NonNullable<ApplicationView["modelProbes"]> = Object.freeze([])
  readonly #viewmodelSequenceCache = new Map<Tf2Class, string>()
  #viewmodelActivities = new Set<string>()
  #crouchHistory: string[] = []
  #viewmodelTimelineProbes: string[] = []
  #lastRandomAudioProbe = ""
  #lastCollisionMoverProbe = ""
  #animationFrame = 0
  #nextSimulationSampleSeconds=0
  readonly #simulationSamples = new SimulationClockQueue()
  #simulationBusy = false
  #simulationTask?: Promise<void>
  #pendingPresentation?:SimulationPublication
  #presentationBusy=false
  #presentationTask?:Promise<void>
  #preparedPresentation?:PreparedPresentation
  readonly #requiredParticleDisplayFrames=new RequiredParticleDisplayQueue<PreparedPresentation>(MAX_REQUIRED_PARTICLE_DISPLAY_FRAMES, 2)
  #preparedRevision=0
  #lastRenderedPreparedRevision=0
  #lastRenderedViewRevision=0
  #lastRenderedTick?:bigint
  readonly #skyVisibility = new ExactSkyVisibilityCache<VisibilityResult>()
  readonly #canvasDiagnostics = new CanvasFrameDiagnostics()
  #displayFrame=0
  #displayTask?:Promise<void>
  #fireEvents = 0
  #explosionEvents = 0
  #paused = true
  #listenersInstalled = false
  #effectVolume = 1
  #musicVolume = 1
  #masterMuted = false
  #mouseSensitivity = 3
  #reverseMouse = false
  #consoleEnabled = true
  #closed = false
  #blockers=new Set<string>()
  readonly #sharedBlockers=new Set<string>()
  #view: ApplicationView = Object.freeze({
    phase: "Startup",
    detail: "Reading local configuration",
    gameUi: "main-menu",
    pointerLocked: false,
    consoleVisible: false,
    blockers: Object.freeze([]),
    fireEvents: 0,
    explosionEvents: 0,
    bootstrapLoading: true,
    bootstrapProgress: 0,
  })

  constructor(
    canvas: HTMLCanvasElement,
    roots: Readonly<{ vgui: HTMLElement; gameUi: HTMLElement; hud: HTMLElement; engineer: HTMLElement; classSelection: HTMLElement; teamSelection: HTMLElement; options: HTMLElement; localMatch: HTMLElement; loading: HTMLElement; startup: HTMLElement; startupVideo: HTMLVideoElement }>,
    publish: (view: ApplicationView) => void,
  ) {
    this.#canvas = canvas
    const presentationRoot = canvas.parentElement
    if (!presentationRoot || [roots.vgui, roots.gameUi, roots.hud, roots.engineer, roots.classSelection, roots.teamSelection, roots.options, roots.localMatch, roots.loading, roots.startup].some((root) => root.parentElement !== presentationRoot)) {
      throw new Error("TF2 presentation owners do not share one application mount")
    }
    this.#presentationRoot = presentationRoot
    this.#vguiRoot = roots.vgui
    this.#gameUiRoot = roots.gameUi
    this.#hudRoot = roots.hud
    this.#engineerRoot = roots.engineer
    this.#classSelectionRoot = roots.classSelection
    this.#teamSelectionRoot = roots.teamSelection
    this.#optionsRoot = roots.options
    this.#localMatchRoot = roots.localMatch
    this.#loadingRoot = roots.loading
    this.#startupRoot = roots.startup
    this.#startupVideo = roots.startupVideo
    this.#publish = publish
    this.#viewportOwner = initializeBrowserPresentationViewportOwner({
      root: this.#presentationRoot,
      onViewport: (viewport) => this.#commitPresentationViewport(viewport),
      onSuspended: () => this.#suspendPresentationViewport(),
    })
    this.#gameUiRoot.hidden = true
    this.#gameUiRoot.inert = true
    this.#gameUiRoot.setAttribute("aria-hidden", "true")
    this.#startupRoot.hidden = false
  }

  #set(patch: Partial<ApplicationView>): void {
    const prior = this.#view
    const priorPhase = prior.phase
    const priorDetail = prior.detail
    const enteredFailure = patch.phase === "Failed" && prior.phase !== "Failed"
    const blockers = prior.blockers.length === this.#blockers.size
      && prior.blockers.every((blocker) => this.#blockers.has(blocker))
      ? prior.blockers
      : Object.freeze([...this.#blockers].sort())
    const changed = blockers !== prior.blockers
      || Object.entries(patch).some(([key, value]) => !Object.is(prior[key as keyof ApplicationView], value))
    if (!changed) return
    this.#view = Object.freeze({ ...prior, ...patch, blockers })
    if (this.#hudIntegration && this.#snapshot && this.#crosshairSettings
      && (this.#view.gameUi !== prior.gameUi
        || this.#view.phase !== prior.phase
        || this.#view.consoleVisible !== prior.consoleVisible
        || this.#view.classSelectionVisible !== prior.classSelectionVisible
        || this.#view.optionsVisible !== prior.optionsVisible
        || this.#view.teamSelectionVisible !== prior.teamSelectionVisible)) {
      this.#hudContext = undefined
      this.#hudContextIdentity = -1
      this.#hudIntegration.setCrosshair(this.#currentHudContext(this.#snapshot).crosshair)
    }
    this.#publish(this.#view)
    const progressChanged = (patch.phase !== undefined && patch.phase !== priorPhase) || (patch.detail !== undefined && patch.detail !== priorDetail)
    if (progressChanged) {
      if (this.#operationWatchdog) clearTimeout(this.#operationWatchdog)
      this.#operationWatchdog = undefined
      if (["Startup", "Loading", "Replacing"].includes(this.#view.phase)) {
        const phase = this.#view.phase
        const detail = this.#view.detail
        this.#output(`STATUS: ${phase}: ${detail}`)
        this.#operationWatchdog = setTimeout(() => {
          if (this.#view.phase === phase && this.#view.detail === detail) {
            this.#set({ phase: "Failed", gameUi: "failure", detail: `${phase} made no progress for 60 seconds: ${detail}` })
          }
        }, 60_000)
      }
    }
    if (enteredFailure) {
      this.#output(`FATAL: ${this.#view.detail}`)
      if (this.#console && !this.#view.consoleVisible) this.toggleConsole()
    }
  }

  #returnToMainMenu(): void {
    this.#view = Object.freeze({
      phase: "MainMenu",
      detail: "TF2 Main Menu",
      gameUi: "main-menu",
      pointerLocked: false,
      consoleVisible: false,
      blockers: Object.freeze([...this.#blockers].sort()),
      fireEvents: 0,
      explosionEvents: 0,
      optionsVisible: false,
      localMatchVisible: false,
      settingsPersistence: this.#view.settingsPersistence,
      settingsApply: this.#view.settingsApply,
      presentationRandomState: this.#presentationRandom ? JSON.stringify(this.#presentationRandom.snapshot()) : this.#view.presentationRandomState,
      presentationCharacter: this.#gameUi?.snapshot().panels.find((panel) => panel.name === "TFCharacterImage")?.state.image ?? this.#view.presentationCharacter,
      startupState: this.#view.startupState,
      startupGestures: this.#view.startupGestures,
      menuPreparation: this.#view.menuPreparation,
    })
    this.#publish(this.#view)
    this.#syncLoadingPresentation()
  }

  #beginLoadingPresentation(): void {
    if (!this.#configuration) throw new Error("TF2 loading configuration is unavailable")
    const target = this.#loadingTarget ?? this.#activeTarget
    if (!target) throw new Error("TF2 loading target is unavailable")
    this.#loadingPresentationGeneration += 1
    this.#loadingBackground = this.#resolveLoadingBackground(target, this.#viewport())
    this.#syncLoadingPresentation()
  }

  #resolveLoadingBackground(target: BrowserTargetConfiguration, viewport: ApplicationPresentationViewport): Extract<Tf2LoadingBackgroundResult, { ok: true }> {
    const image = this.#uiResources?.descriptor.images.find((candidate) =>
      candidate.configuredValue.toLowerCase() === `maps/menu_photos_${target.target}`)
    const material = target.loading.mapPhoto?.material
      ?? (image?.classification === "content-vtf" ? image.material : null)
    const provider = target.loading.mapPhoto
      ? `game-04-${material?.providerIdentity}`
      : material?.providerIdentity?.replace(/^ui-(\d+)-/u, "game-$1-")
    const result = resolveTf2LoadingBackground({
      generation: this.#loadingPresentationGeneration,
      mapIdentity: target.target,
      viewport,
      mapPhotoLookups: target.loading.mapPhotoLocations.map((location) => {
        if (!material || !provider || !location.startsWith(`${provider}:`)
          || !material.sha256 || !material.byteLength || !material.providerIdentity || !material.providerRevision) {
          return Object.freeze({ location, outcome: "missing" as const })
        }
        return Object.freeze({
          location,
          outcome: "found" as const,
          asset: Object.freeze({
            logicalPath: material.logicalPath,
            byteLength: material.byteLength,
            sha256: material.sha256,
            providerIdentity: material.providerIdentity,
            providerRevision: material.providerRevision,
          }),
        })
      }),
      backingMaterial: target.loading.stampBackground.material,
      backingTexture: target.loading.stampBackground.texture,
    })
    if (!result.ok) throw new Error(`${result.code}:${result.subject}`)
    return result
  }

  #syncLoadingPresentation(viewport = this.#viewport()): void {
    if (!this.#loadingPresentation || !this.#loadingVgui || !this.#gameUi || this.#loadingPresentationGeneration < 1) return
    const snapshot = this.#loadingPresentation.update(
      this.#loadingPresentationGeneration,
      this.#gameUi.state(),
      viewport,
      this.#loadingBackground,
    )
    if (snapshot) this.#loadingVgui.apply(snapshot)
  }

  #syncGameUiBackgroundProbe(): void {
    if (!this.#gameUi || !this.#uiResources) return
    const panel = this.#gameUi.snapshot().panels.find((candidate) => candidate.name === "GameUiBaseBackground")
    const variant = this.#uiResources.gameUiBackground.variants.find((candidate) => candidate.image === panel?.state.image)
    const element = this.#gameUiRoot.querySelector<HTMLElement>("[data-vgui-name=GameUiBaseBackground]")
    if (!panel || !variant || !element) throw new Error("TF2 GameUI base-background presentation is unavailable")
    element.setAttribute("data-tf2-gameui-base-background", "")
    element.setAttribute("data-source-image", variant.image)
    element.setAttribute("data-source-material", variant.material)
    element.setAttribute("data-source-material-sha256", variant.materialSha256)
    element.setAttribute("data-source-texture", variant.texture)
    element.setAttribute("data-source-texture-sha256", variant.textureSha256)
    element.setAttribute("data-source-list", this.#uiResources.gameUiBackground.source.logicalPath)
    element.setAttribute("data-source-list-sha256", this.#uiResources.gameUiBackground.source.sha256)
    element.setAttribute("data-background-name", this.#uiResources.gameUiBackground.backgroundName)
  }

  #advanceLoading(phase: Tf2LoadingPhase): void {
    const transition = this.#gameUi?.dispatch({ kind: "loading-progress", phase })
    if (transition?.state.kind === "loading") {
      this.#syncLoadingPresentation()
      this.#set({
        phase: "Loading",
        gameUi: "loading",
        detail: transition.state.statusText || phase,
        loadingProgress: transition.state.progress,
        loadingStatus: transition.state.statusText,
        loadingBackground: this.#loadingBackground?.disposition,
      })
    }
  }

  #ensureOptions():Tf2OptionsPresentation{
    if(this.#options)return this.#options
    if(!this.#uiResources||!this.#settings||!this.#presentationRandom)throw new Error("TF2 Options inputs are unavailable")
    this.#options=initializeTf2OptionsPresentation({
      root:this.#optionsRoot,
      resources:this.#uiResources,
      settings:this.#settings,
      viewport:this.#viewport(),
      reducedMotion:matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock:{nowSeconds:()=>this.#frameClock.admit(performance.now()/1_000)},
      random:this.#presentationRandom,
      onPersistence:(bytes)=>{
        localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY,new TextDecoder().decode(bytes))
        this.#set({settingsPersistence:"stored"})
      },
      onApply:(snapshot)=>{
        if (this.#console) this.#console.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        this.#set({settingsApply:JSON.stringify(snapshot.lastApply)})
      },
      onVisibility:(visible)=>this.#set({optionsVisible:visible}),
    })
    return this.#options
  }

  #ensureLocalMatch(): Tf2LocalMatchPresentation {
    if (this.#localMatch) return this.#localMatch
    if (!this.#uiResources || !this.#presentationRandom || !this.#configuration) {
      throw new Error("TF2 local match configured resources are unavailable")
    }
    this.#localMatch = initializeTf2LocalMatchPresentation({
      root: this.#localMatchRoot,
      resources: this.#uiResources,
      configuredMaps: this.#configuration.targets.map((target) => target.target),
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      storage: localStorage,
      onVisibility: (visible) => this.#set({
        localMatchVisible: visible,
        localMatchEntry: visible ? this.#localMatch?.snapshot().entry ?? undefined : undefined,
      }),
      onLaunch: (launch) => {
        this.#pendingLocalMatch = launch
        this.#activeBotConfiguration = launch.configuration
        this.#botDifficulty = launch.configuration.difficulty
        this.#set({ localMatchSettings: JSON.stringify(launch), localMatchVisible: false })
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        if (this.#client && this.#renderer && this.#loaded) {
          const target = this.#configuration!.targets.find((candidate) => candidate.target === launch.mapIdentity)
          if (!target) throw new Error(`Undeclared local match map ${launch.mapIdentity}`)
          void this.#switchCatalogMap(target)
          return
        }
        const transition = this.#gameUi?.dispatch({ kind: "map", mapIdentity: launch.mapIdentity })
        if (transition?.disposition !== "applied") {
          throw new Error(`TF2 local match map rejected: ${transition?.reason ?? "GameUI unavailable"}`)
        }
      },
    })
    return this.#localMatch
  }

  #deferGameUiRequest(request: Tf2GameUiRequest): void {
    const operation = this.#operation
    const handle = window.setTimeout(() => {
      this.#gameUiRequestTasks.delete(handle)
      if (this.#closed || !this.#operations.current(operation)) return
      void this.#gameUiRequest(request).catch((error) => {
        if (this.#closed || (request.kind !== "load-map" && !this.#operations.current(operation))) return
        this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "GameUI owner request failed" })
      })
    }, 0)
    this.#gameUiRequestTasks.add(handle)
  }

  async #gameUiRequest(request: Tf2GameUiRequest): Promise<void> {
    if (request.kind === "show-console") { this.toggleConsole(); return }
    if (request.kind === "show-options") {
      const options = this.#ensureOptions()
      options.show(request.page === "advanced-options" ? "advanced" : "keyboard")
      this.#set({ optionsVisible: true })
      return
    }
    if (request.kind === "show-local-match") {
      const presentation = this.#ensureLocalMatch()
      presentation.show(request.entry)
      this.#set({ localMatchVisible: true, localMatchEntry: request.entry })
      return
    }
    if (request.kind === "resume-game") {
      const restorePointer = this.#resumePointerOnAcknowledge
      this.#resumePointerOnAcknowledge = false
      const transition = this.#gameUi?.dispatch({ kind: "gameui-hidden" })
      if (transition?.disposition !== "applied") return
      this.#paused = document.hidden
      this.#nextSimulationSampleSeconds = 0
      this.#set({ gameUi: "in-game", detail: `Playing ${this.#mapIdentity}` })
      if (restorePointer && !this.#paused) void this.requestPointer()
      return
    }
    if (request.kind === "disconnect") {
      await this.#teardownGameplay()
      this.#gameUi?.dispatch({ kind: "teardown-confirmed" })
      this.#returnToMainMenu()
      return
    }
    if (request.kind === "quit") {
      this.#set({ hostRequest: "quit", detail: "Quit requested from browser host" })
      return
    }
    if (request.kind === "open-external-link") {
      window.open(request.href, "_blank", "noopener,noreferrer")
      return
    }
    if (request.kind === "load-map") {
      const target = this.#configuration?.targets.find((candidate) => candidate.target === request.mapIdentity)
      if (!target) throw new Error(`Undeclared map request ${request.mapIdentity}`)
      this.#loadingTarget = target
      const started = this.#gameUi?.dispatch({ kind: "loading-started", mapIdentity: request.mapIdentity })
      if (started?.disposition !== "applied") throw new Error("TF2 GameUI rejected loading start")
      this.#beginLoadingPresentation()
      this.#set({ phase: "Loading", gameUi: "loading", detail: "Starting local game server...", loadingProgress: 0, loadingStatus: "", loadingBackground: this.#loadingBackground?.disposition })
      this.#advanceLoading("changing-map")
      await this.#startGameplay(target)
      if (this.#loadingTarget === target) this.#loadingTarget = undefined
    }
  }

  async #applySettings(request: SettingsAdapterRequest): Promise<AdapterRequestResult> {
    const reject = (reason: string): AdapterRequestResult => Object.freeze({ requestId: request.requestId, status: "rejected", reason })
    if (request.owner === "audio") {
      const accepted = new Set(["audio.effect-volume", "audio.music-volume", "audio.master-muted", "audio.mute-while-unfocused"])
      if (request.changes.some((change) => !accepted.has(change.settingId))) return reject("browser audio owner does not implement every requested effect")
      for (const change of request.changes) {
        if (change.settingId === "audio.effect-volume") this.#effectVolume = change.nextValue as number
        else if (change.settingId === "audio.music-volume") this.#musicVolume = change.nextValue as number
        else if (change.settingId === "audio.master-muted") this.#masterMuted = change.nextValue as boolean
      }
      return Object.freeze({ requestId: request.requestId, status: "applied" })
    }
    if (request.owner === "renderer") {
      if (request.changes.some((change) => change.settingId !== "video.hdr")) return reject("browser renderer owner does not implement every requested effect")
      const value = request.changes.at(-1)?.nextValue
      if (value !== 0 && value !== 1 && value !== 2) return reject("renderer HDR value is invalid")
      this.#renderLevel = value
      if (this.#client && this.#renderer && this.#loaded) {
        try { await this.#replaceCatalogMap() }
        catch (error) { return reject(error instanceof Error ? error.message : "renderer replacement failed") }
      }
      return Object.freeze({ requestId: request.requestId, status: "applied" })
    }
    if (request.owner === "input") {
      const accepted = new Set(["mouse.sensitivity", "mouse.reverse"])
      if (request.changes.some((change) => change.kind !== "binding" && !accepted.has(change.settingId))) return reject("browser input owner does not implement every requested effect")
      if (request.changes.some((change) => change.kind === "binding"
        && !TF2_SELECTED_OPTIONS.settings.some((schema) => schema.id === change.settingId && schema.kind === "binding"))) {
        return reject("browser input binding schema is unavailable")
      }
      let bindingsChanged = false
      for (const change of request.changes) {
        if (change.settingId === "mouse.sensitivity") this.#mouseSensitivity = change.nextValue as number
        else if (change.settingId === "mouse.reverse") this.#reverseMouse = change.nextValue as boolean
        else if (change.kind === "binding") {
          const schema = TF2_SELECTED_OPTIONS.settings.find((candidate) => candidate.id === change.settingId)
          if (!schema || schema.kind !== "binding") throw new Error("validated browser input binding schema disappeared")
          const value = change.nextValue
          if (value && typeof value === "object") {
            this.#bindingValues.set(schema.id, Object.freeze({ action: schema.action, code: value.code, modifiers: value.modifiers }))
          } else {
            this.#bindingValues.delete(schema.id)
          }
          bindingsChanged = true
        }
      }
      if (bindingsChanged) this.#bindings.replace([...this.#bindingValues.values()])
      return Object.freeze({ requestId: request.requestId, status: "applied" })
    }
    if (request.owner === "application") {
      if (request.changes.some((change) => change.settingId !== "keyboard.console-enabled")) return reject("browser application owner does not implement every requested effect")
      this.#consoleEnabled = request.changes.at(-1)?.nextValue as boolean
      return Object.freeze({ requestId: request.requestId, status: "applied" })
    }
    if (request.owner === "game") {
      const crosshairIds = new Set<string>(TF2_CROSSHAIR_SETTINGS.map((setting) => setting.settingId))
      if (request.changes.some((change) => !crosshairIds.has(change.settingId)
        && (!["cl_hud_playerclass_use_playermodel", "tf_scoreboard_ping_as_text"].includes(change.settingId)
          || typeof change.nextValue !== "boolean"))) {
        return reject(`browser game owner does not implement every requested effect: ${request.changes.map((change) => `${change.settingId}=${String(change.nextValue)}`).join(",")}`)
      }
      const model = request.changes.find((change) => change.settingId === "cl_hud_playerclass_use_playermodel")
      if (model) {
        this.#playerClassUsePlayerModel = model.nextValue as boolean
        this.#hudIntegration?.setPlayerClassUsePlayerModel(this.#playerClassUsePlayerModel)
      }
      const scoreboardPing = request.changes.find((change) => change.settingId === "tf_scoreboard_ping_as_text")
      if (scoreboardPing) {
        this.#scoreboardPingAsText = scoreboardPing.nextValue as boolean
        this.#hudContext = undefined
      }
      if (request.changes.some((change) => crosshairIds.has(change.settingId))) {
        if (!this.#settings) return reject("browser crosshair settings authority is unavailable")
        const values = {
          ...this.#settings.snapshot().settings.current,
          ...Object.fromEntries(request.changes.map((change) => [change.settingId, change.nextValue])),
        }
        this.#replaceCrosshair(tf2CrosshairSettings(values))
      }
      this.#set({ hudPresentationProbe: this.#hudPresentationObservation() })
      return Object.freeze({ requestId: request.requestId, status: "applied" })
    }
    return reject(`browser game owner is unavailable for ${request.changes.map((change) => change.settingId).join(",")}`)
  }

  #verifiedDependency(logicalPath: string, byteLength: number, expectedSha256: string): Uint8Array {
    const bytes = this.#dependencyEntries.get(logicalPath)
    if (!bytes || bytes.byteLength !== byteLength || bytesToHex(sha256(bytes)) !== expectedSha256) {
      throw new Error(`Configured dependency ${logicalPath} differs`)
    }
    return bytes
  }

  #nextOperation(): ApplicationOperation {
    this.#operation = this.#operations.begin()
    return this.#operation
  }

  #requireOperation(operation: ApplicationOperation): void {
    if (this.#closed || !this.#operations.current(operation)) {
      throw new DOMException("Application operation was superseded", "AbortError")
    }
  }

  #trackBootstrapObject(sha256: string, loaded: number, total: number): void {
    if (this.#bootstrapExpectedObjects.size > 0 && !this.#bootstrapExpectedObjects.has(sha256)) return
    this.#bootstrapObjectProgress.set(sha256, Object.freeze({ loaded, total }))
    if (this.#bootstrapExpectedObjects.size === 0) return
    let loadedBytes = 0
    let expectedBytes = 0
    for (const identity of this.#bootstrapExpectedObjects) {
      const progress = this.#bootstrapObjectProgress.get(identity)
      if (!progress) continue
      loadedBytes += progress.loaded
      expectedBytes += progress.total
    }
    const percentage = expectedBytes === 0 ? 0 : Math.min(100, Math.floor(loadedBytes * 100 / expectedBytes))
    this.#set({ bootstrapLoading: true, bootstrapProgress: percentage })
  }

  async #ensureResourceRuntime(signal = this.#operation.signal): Promise<void> {
    if (!this.#configuration) throw new Error("Browser configuration is unavailable")
    if (!this.#cache) this.#cache = await openDerivedObjectCache()
    if (!this.#client) {
      const descriptor = this.#configuration.wasm
      const wasm = await fetchImmutableObject(this.#configuration.assetOrigin, descriptor, signal, fetch, (loaded, total) => this.#trackBootstrapObject(descriptor.sha256, loaded, total))
      this.#client = new Tf2WorkerClient(new GameplayWorker(), this.#cache)
      await this.#client.initialize(wasm, this.#configuration.wasm.sha256)
    }
  }

  async #resourceSet(roles: readonly string[], signal = this.#operation.signal): Promise<Uint8Array> {
    if (!this.#configuration || !this.#resourceGraph) throw new Error("Resource graph is unavailable")
    return this.#decodeResourceSet(this.#resourceGraph, roles, this.#dependencyEntries, signal)
  }

  async #decodeResourceSet(graph: ResourceGraph, roles: readonly string[], destination: Map<string, Uint8Array>, signal: AbortSignal): Promise<Uint8Array> {
    if (!this.#configuration) throw new Error("Browser configuration is unavailable")
    await this.#ensureResourceRuntime(signal)
    const chunks = new Map<string, ResourceChunkDescriptor>()
    for (const role of roles) for (const chunk of chunksForRole(graph, role)) chunks.set(chunk.encodedSha256, chunk)
    const records = await Promise.all([...chunks.values()].map(async (chunk) => {
      let bytes = await this.#cache!.read(chunk.encodedSha256)
      if (!bytes) {
        const object = resourceChunkObject(chunk)
        bytes = await fetchImmutableObject(this.#configuration!.assetOrigin, object, signal, fetch, (loaded, total) => this.#trackBootstrapObject(object.sha256, loaded, total))
        await this.#cache!.write(chunk.encodedSha256, chunk.encodedSha256, bytes)
      } else {
        this.#trackBootstrapObject(chunk.encodedSha256, bytes.byteLength, bytes.byteLength)
      }
      return Object.freeze({ descriptor: chunk, bytes })
    }))
    const set = await this.#client!.decodeResources(records)
    for (const [logicalPath, bytes] of parseResourceSet(set)) {
      const existing = destination.get(logicalPath)
      if (existing && (existing.byteLength !== bytes.byteLength || bytesToHex(sha256(existing)) !== bytesToHex(sha256(bytes)))) {
        throw new Error(`Conflicting resource ${logicalPath}`)
      }
      destination.set(logicalPath, bytes)
    }
    return set
  }

  async #prepareStartupMedia(
    descriptor: Tf2StartupDescriptor,
    events: Readonly<{ completed(): void; failed(reason: string): void }>,
  ): Promise<Tf2StartupMediaSession> {
    const validated = validateTf2StartupDescriptor(descriptor)
    if (!validated.ok) throw new Error(`${validated.code}:${validated.subject}`)
    this.#verifiedDependency(descriptor.manifest.logicalPath, descriptor.manifest.byteLength, descriptor.manifest.sha256)
    const bytes = this.#verifiedDependency(descriptor.browserRepresentation.logicalPath, descriptor.browserRepresentation.byteLength, descriptor.browserRepresentation.sha256)
    const video = this.#startupVideo
    const url = URL.createObjectURL(new Blob([bytes.slice()], { type: "video/webm" }))
    let destroyed = false
    let admitted = false
    const completed = () => events.completed()
    const failed = () => events.failed(video.error ? `MediaError:${video.error.code}` : "Startup media failed")
    video.controls = false
    video.muted = false
    video.loop = false
    video.autoplay = false
    video.playsInline = true
    video.disablePictureInPicture = true
    video.src = url
    video.addEventListener("ended", completed)
    video.addEventListener("error", failed)
    try {
      await new Promise<void>((resolve, reject) => {
        const loaded = () => { cleanup(); resolve() }
        const error = () => { cleanup(); reject(new Error(video.error ? `MediaError:${video.error.code}` : "Startup media decode failed")) }
        const cleanup = () => { video.removeEventListener("loadedmetadata", loaded); video.removeEventListener("error", error) }
        if (video.readyState >= HTMLMediaElement.HAVE_METADATA) resolve()
        else {
          video.addEventListener("loadedmetadata", loaded)
          video.addEventListener("error", error)
          video.load()
        }
      })
      const expected = descriptor.browserRepresentation
      if (video.videoWidth !== expected.video.width || video.videoHeight !== expected.video.height
        || Math.abs(video.duration * 1_000_000 - expected.durationMicroseconds) > 1_000) {
        throw new Error("Configured startup media metadata differs")
      }
    } catch (error) {
      video.removeEventListener("ended", completed)
      video.removeEventListener("error", failed)
      video.removeAttribute("src")
      video.load()
      URL.revokeObjectURL(url)
      throw error
    }
    const startPlayback = async (): Promise<"started"> => {
      await video.play()
      admitted = true
      return "started"
    }
    return Object.freeze({
      play: async () => {
        try { return await startPlayback() }
        catch (error) {
          if (error instanceof DOMException && error.name === "NotAllowedError") {
            video.muted = true
            this.#startupMutedFallback = true
            this.#set({ startupMutedFallback: true })
            this.#output("STATUS: Startup autoplay continued muted until the first interaction.", true)
            return startPlayback()
          }
          throw error
        }
      },
      admitGesture: startPlayback,
      skip: () => video.pause(),
      setVisible: (visible) => {
        if (!visible) video.pause()
        else if (admitted) void video.play().catch((error) => events.failed(error instanceof Error ? error.message : "Startup media resume failed"))
      },
      destroy: () => {
        if (destroyed) return
        destroyed = true
        admitted = false
        this.#startupMutedFallback = false
        this.#set({ startupMutedFallback: false })
        video.pause()
        video.removeEventListener("ended", completed)
        video.removeEventListener("error", failed)
        video.removeAttribute("src")
        video.load()
        URL.revokeObjectURL(url)
      },
    })
  }

  async #prepareMainMenu(): Promise<Tf2HiddenMenu> {
    if (!this.#configuration || !this.#presentationRandom) throw new Error("TF2 Main Menu inputs are unavailable")
    await this.#resourceSet(["menu"])
    this.#set({ menuPreparation: "console-resources" })
    this.#consoleResources = await resolveConfiguredConsoleResources(this.#dependencyEntries, this.#viewport().height)
    this.#blockers.add(this.#consoleResources.blocker)
    this.#set({ menuPreparation: "vgui-resources" })
    this.#uiResources = await initializeTf2VguiResources({
      dependencies: this.#dependencyEntries,
      viewportHeight: Math.max(1, Math.trunc(this.#viewport().height)),
      platform: navigator.platform.toLowerCase().startsWith("mac") ? "macos" : navigator.platform.toLowerCase().startsWith("win") ? "windows" : "linux",
    })
    this.#set({ menuPreparation: "constructing-main-menu" })
    for (const diagnostic of this.#uiResources.diagnostics) this.#blockers.add(`TF2Ui${diagnostic.code}: ${diagnostic.subject}`)
    const persisted = localStorage.getItem(TF2_BROWSER_SETTINGS_STORAGE_KEY)
    const duckBinding = TF2_SELECTED_OPTIONS.settings.find((schema) => schema.kind === "binding" && schema.action === "+duck")
    this.#settings = initializeTf2BrowserSettings({
      persistence: persisted === null ? null : new TextEncoder().encode(persisted),
      current: {
        "keyboard.console-enabled": true,
        "audio.effect-volume": this.#effectVolume,
        "audio.music-volume": this.#musicVolume,
        "audio.master-muted": this.#masterMuted,
        "mouse.sensitivity": this.#mouseSensitivity,
        "mouse.reverse": this.#reverseMouse,
        "video.hdr": this.#renderLevel,
        "cl_hud_playerclass_use_playermodel": this.#playerClassUsePlayerModel,
        ...(duckBinding ? { [duckBinding.id]: Object.freeze({ code: "SHIFT", modifiers: 0 }) } : {}),
      },
      owners: { renderer: "available", audio: "available", input: "available", game: "available", application: "available" },
      apply: (request) => this.#applySettings(request),
    })
    this.#set({ menuPreparation: "settings-ready" })
    const persistenceState = persisted === null ? "absent" : this.#settings.snapshot().persistenceDiagnostic ? "rejected" : "loaded"
    try { this.#gameUi = initializeTf2GameUiIntegration({
      root: this.#gameUiRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      presentation: {
        random: this.#presentationRandom,
        activeHoliday: this.#configuration.presentation.activeHoliday,
        activeWar: this.#configuration.presentation.activeWar,
        activeOperation: this.#configuration.presentation.activeOperation,
        freeTrial: this.#configuration.presentation.freeTrial,
      },
      onRequest: (request) => this.#deferGameUiRequest(request),
    }) } catch (error) {
      this.#set({ menuPreparation: `gameui-error:${error instanceof Error ? error.message : String(error)}` })
      throw error
    }
    this.#set({ menuPreparation: "gameui-ready" })
    this.#syncGameUiBackgroundProbe()
    for (const diagnostic of this.#gameUi.diagnostics()) this.#blockers.add(`TF2GameUi${diagnostic.code}: ${diagnostic.subject}`)
    const loadingResource = this.#uiResources.descriptor.panels.find((panel) => panel.source.logicalPath === "resource/loadingdialognobanner.res")
    const failureResource = this.#uiResources.descriptor.panels.find((panel) => panel.source.logicalPath === "resource/loadingdialogerror.res")
    if (!loadingResource || !failureResource) throw new Error("Configured TF2 loading resources are unavailable")
    this.#loadingPresentation = createTf2LoadingPresentation({ loadingResource, failureResource })
    this.#set({ menuPreparation: "loading-model-ready" })
    try {
      this.#loadingVgui = initializeTf2LoadingVguiRuntime({
        root: this.#loadingRoot,
        resources: this.#uiResources,
        viewport: this.#viewport(),
        reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
        clock: { nowSeconds: () => this.#frameClock.current },
        random: this.#presentationRandom,
        onRequest: (request) => {
          if (request.kind === "disconnect") this.#gameUi?.dispatch({ kind: "activate-button", button: "cancel-loading" })
          else {
            const transition = this.#gameUi?.dispatch({ kind: "dismiss-failure" })
            if (transition?.state.kind === "main-menu") this.#returnToMainMenu()
          }
          this.#syncLoadingPresentation()
        },
      })
    } catch (error) {
      this.#set({ menuPreparation: `loading-error:${error instanceof Error ? error.message : String(error)}` })
      throw error
    }
    this.#sharedBlockers.clear()
    for (const blocker of this.#blockers) this.#sharedBlockers.add(blocker)
    this.#set({ menuPreparation: "ready" })
    this.#sharedDependencyEntries = new Map(this.#dependencyEntries)
    this.#initializeConsole()
    const currentSettings = this.#settings.snapshot().settings.current
    this.#bindingValues.clear()
    for (const schema of TF2_SELECTED_OPTIONS.settings) {
      if (schema.kind !== "binding") continue
      const value = currentSettings[schema.id]
      if (value && typeof value === "object") {
        this.#bindingValues.set(schema.id, Object.freeze({ action: schema.action, code: value.code, modifiers: value.modifiers }))
      }
    }
    this.#bindings.replace([...this.#bindingValues.values()])
    this.#consoleEnabled = currentSettings["keyboard.console-enabled"] === true
    this.#effectVolume = currentSettings["audio.effect-volume"] as number
    this.#musicVolume = currentSettings["audio.music-volume"] as number
    this.#masterMuted = currentSettings["audio.master-muted"] === true
    this.#mouseSensitivity = currentSettings["mouse.sensitivity"] as number
    this.#reverseMouse = currentSettings["mouse.reverse"] === true
    this.#playerClassUsePlayerModel = currentSettings["cl_hud_playerclass_use_playermodel"] === true
    this.#scoreboardPingAsText = currentSettings["tf_scoreboard_ping_as_text"] === true
    this.#crosshairSettings = tf2CrosshairSettings(currentSettings)
    this.#installListeners()
    this.#animationFrame = requestAnimationFrame(this.#frame)
    this.#paused = true
    this.#set({
      settingsPersistence: persistenceState,
      optionsVisible: false,
      presentationRandomState: JSON.stringify(this.#presentationRandom.snapshot()),
      presentationCharacter: this.#gameUi.snapshot().panels.find((panel) => panel.name === "TFCharacterImage")?.state.image ?? "unavailable",
    })
    return Object.freeze({
      reveal: () => this.#revealMainMenu(),
      destroy: () => this.#destroyMenuPresentation(),
    })
  }

  #revealMainMenu(): void {
    if (this.#menuRevealed || this.#closed) return
    this.#menuRevealed = true
    this.#gameUiRoot.hidden = false
    this.#gameUiRoot.inert = false
    this.#gameUiRoot.removeAttribute("aria-hidden")
    this.#startupRoot.hidden = true
    this.#set({ phase: "MainMenu", gameUi: "main-menu", detail: "TF2 Main Menu" })
  }

  #destroyMenuPresentation(): void {
    if (this.#menuPresentationDestroyed) return
    this.#menuPresentationDestroyed = true
    this.#loadingVgui?.destroy()
    this.#loadingVgui = undefined
    this.#loadingPresentation?.destroy()
    this.#loadingPresentation = undefined
    this.#gameUi?.destroy()
    this.#gameUi = undefined
    this.#hudIntegration?.destroy()
    this.#hudIntegration = undefined
    this.#engineer?.destroy()
    this.#engineer = undefined
    this.#classSelection?.destroy()
    this.#classSelection = undefined
    this.#teamSelection?.destroy()
    this.#teamSelection = undefined
    this.#hudRootCounts = undefined
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    this.#options?.destroy()
    this.#options = undefined
    this.#localMatch?.destroy()
    this.#localMatch = undefined
    this.#uiResources?.destroy()
    this.#uiResources = undefined
    this.#console?.apply({ kind: "destroy" })
    this.#console = undefined
    this.#diagnostics?.apply({ kind: "destroy" })
    this.#diagnostics = undefined
    this.#consoleResources?.fontSet?.destroy()
    this.#consoleResources = undefined
  }

  readonly #startupKey = (event: KeyboardEvent): void => {
    const state = this.#startup?.state().kind
    this.#unmuteStartup()
    if (event.code === "Escape" && (state === "Playing" || state === "AwaitingGesture")) {
      event.preventDefault()
      event.stopImmediatePropagation()
      this.#startup?.key("Escape")
      return
    }
    if (state === "AwaitingGesture") {
      this.#startupGestures += 1
      this.#set({ startupGestures: this.#startupGestures })
      this.#startup?.gesture()
    }
  }
  readonly #startupPointer = (): void => this.#unmuteStartup()
  readonly #startupVisibility = (): void => this.#startup?.visibility(!document.hidden)
  #installStartupListeners(): void {
    window.addEventListener("keydown", this.#startupKey, true)
    window.addEventListener("pointerdown", this.#startupPointer, true)
    document.addEventListener("visibilitychange", this.#startupVisibility)
  }
  #removeStartupListeners(): void {
    window.removeEventListener("keydown", this.#startupKey, true)
    window.removeEventListener("pointerdown", this.#startupPointer, true)
    document.removeEventListener("visibilitychange", this.#startupVisibility)
  }
  #startupState(state: Tf2StartupState): void {
    if (state.kind === "Completed" || state.kind === "Skipped" || state.kind === "Failed" || state.kind === "Destroyed") this.#removeStartupListeners()
    const bootstrapLoading = !["Playing", "AwaitingGesture", "Completed", "Skipped", "Failed", "Destroyed"].includes(state.kind)
    if (state.kind === "Failed") this.#set({ phase: "Failed", gameUi: "failure", startupState: state.kind, bootstrapLoading, detail: `${state.stage}: ${state.reason}` })
    else if (state.kind !== "Completed" && state.kind !== "Skipped" && state.kind !== "Destroyed") this.#set({ phase: "Startup", startupState: state.kind, bootstrapLoading, detail: state.kind })
    else this.#set({ startupState: state.kind, bootstrapLoading })
  }

  #unmuteStartup(): void {
    if (!this.#startupMutedFallback || this.#startup?.state().kind !== "Playing") return
    this.#startupVideo.muted = false
    this.#startupMutedFallback = false
    this.#set({ startupMutedFallback: false })
  }

  admitStartupGesture(): void {
    this.#unmuteStartup()
    if (this.#startup?.state().kind !== "AwaitingGesture") return
    this.#startupGestures += 1
    this.#set({ startupGestures: this.#startupGestures })
    this.#startup.gesture()
  }

  startupKey(code: string): void {
    this.#unmuteStartup()
    if (code === "Escape") this.#startup?.key("Escape")
  }

  async start(): Promise<void> {
    try {
      await this.#viewportOwner.first()
      this.#configuration = await loadBrowserConfiguration()
      this.#activeTarget = this.#configuration.targets.find((target) => target.target === this.#configuration!.defaultTarget)
      if (!this.#activeTarget) throw new Error("Default TF2 target is absent")
      this.#presentationRandom = createTf2PresentationRandom(this.#configuration.presentation.randomSeed)
      this.#renderLevel = this.#configuration.renderLevel
      this.#mapIdentity = this.#activeTarget.target
      this.#set({ phase: "Startup", startupState: "Preparing", detail: "Preparing configured Valve startup movie" })
      const catalogDescriptor = this.#configuration.catalog
      const catalogBytes = await fetchImmutableObject(this.#configuration.assetOrigin, catalogDescriptor, this.#operation.signal, fetch, (loaded, total) => this.#trackBootstrapObject(catalogDescriptor.sha256, loaded, total))
      const catalog = parseResourceCatalogBytes(catalogBytes)
      if (catalog.application !== "tf2" || catalog.entries.length !== this.#configuration.targets.length) throw new Error("Resource catalog target table differs")
      for (const target of this.#configuration.targets) {
        const entry = selectCatalogTarget(catalog, target.target)
        if (entry.resources.sha256 !== target.objects.resources.sha256 || entry.resources.byteLength !== target.objects.resources.byteLength) throw new Error(`Resource catalog ${target.target} descriptor differs`)
      }
      this.#resourceCatalog = catalog
      const selected = selectCatalogTarget(catalog, this.#activeTarget.target)
      const graphBytes = await fetchImmutableObject(this.#configuration.assetOrigin, selected.resources, this.#operation.signal, fetch, (loaded, total) => this.#trackBootstrapObject(selected.resources.sha256, loaded, total))
      this.#resourceGraph = parseResourceGraphBytes(graphBytes)
      if (this.#resourceGraph.target !== this.#activeTarget.target || this.#resourceGraph.contentBuild !== this.#activeTarget.contentBuild) throw new Error("Resource graph target differs")
      this.#bootstrapExpectedObjects = new Set([
        catalogDescriptor.sha256,
        selected.resources.sha256,
        this.#configuration.wasm.sha256,
        ...chunksForRole(this.#resourceGraph, "startup").map((chunk) => chunk.encodedSha256),
      ])
      if (!this.#bootstrapObjectProgress.has(this.#configuration.wasm.sha256)) {
        this.#bootstrapObjectProgress.set(this.#configuration.wasm.sha256, Object.freeze({ loaded: 0, total: Number(this.#configuration.wasm.byteLength) }))
      }
      for (const chunk of chunksForRole(this.#resourceGraph, "startup")) {
        if (!this.#bootstrapObjectProgress.has(chunk.encodedSha256)) {
          this.#bootstrapObjectProgress.set(chunk.encodedSha256, Object.freeze({ loaded: 0, total: Number(chunk.encodedByteLength) }))
        }
      }
      this.#trackBootstrapObject(catalogDescriptor.sha256, catalogBytes.byteLength, catalogBytes.byteLength)
      this.#trackBootstrapObject(selected.resources.sha256, graphBytes.byteLength, graphBytes.byteLength)
      await this.#resourceSet(["startup"])
      this.#startup = createTf2StartupController({
        descriptor: this.#configuration.startup,
        policy: { benchmark: false, editMode: false, forceVr: false, developer: false, noVideo: false, allowDebug: false, healthWarningPresent: false },
        clock: { nowMicroseconds: () => Math.round(performance.now() * 1_000) },
        media: { prepare: (descriptor, events) => this.#prepareStartupMedia(descriptor, events) },
        menu: { prepareHidden: () => this.#prepareMainMenu() },
        onState: (state) => this.#startupState(state),
      })
      this.#installStartupListeners()
      this.#startup.start()
    } catch (error) {
      await this.#release()
      this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "Application startup failed" })
    }
  }

  async #startGameplay(target: BrowserTargetConfiguration): Promise<void> {
    const loadStarted=performance.now()
    let loadPhase=loadStarted
    const loadTimings:Record<string,number>={}
    const finishLoadPhase=(name:string):void=>{const now=performance.now();loadTimings[name]=now-loadPhase;loadPhase=now}
    const operation = this.#nextOperation()
    const signal = operation.signal
    try {
      if (!this.#configuration || !this.#resourceCatalog) throw new Error("Browser configuration is unavailable")
      if (this.#activeTarget?.target !== target.target) {
        const selected = selectCatalogTarget(this.#resourceCatalog, target.target)
        if (selected.resources.sha256 !== target.objects.resources.sha256 || selected.resources.byteLength !== target.objects.resources.byteLength) {
          throw new Error("Target resource root differs from catalog")
        }
        const bytes = await fetchImmutableObject(this.#configuration.assetOrigin, selected.resources, signal)
        this.#requireOperation(operation)
        const graph = parseResourceGraphBytes(bytes)
        if (graph.target !== target.target || graph.contentBuild !== target.contentBuild) {
          throw new Error("Target resource graph identity differs")
        }
        this.#resourceGraph = graph
        this.#dependencyEntries = new Map(this.#sharedDependencyEntries)
        this.#activeTarget = target
        this.#mapIdentity = target.target
      }
      this.#set({ detail: "Fetching exact BSP and gameplay WASM objects" })
      this.#advanceLoading("reading-world")
      const [bsp, resources] = await Promise.all([
        fetchImmutableObject(this.#configuration.assetOrigin, target.objects.bsp, signal),
        this.#resourceSet(["gameplay"], signal),
      ])
      this.#requireOperation(operation)
      this.#dependencies = resources
      finishLoadPhase("fetch")
      this.#advanceLoading("building-resource-index")
      await this.#ensureResourceRuntime(signal)
      finishLoadPhase("cacheOpen")
      finishLoadPhase("workerInitialize")
      const profile = this.#renderLevel === 2 ? 1 : 0
      const key = await mapDerivedKey(
        this.#activeTarget.objects.bsp.sha256,
        profile,
        this.#renderLevel,
        this.#configuration.wasm.sha256,
        this.#dependencies,
      )
      finishLoadPhase("derivedKey")
      this.#set({ detail: "Compiling direct map authority" })
      this.#advanceLoading("preparing-resources")
      this.#generation += 1
      this.#resetGenerationPresentation()
      this.#loaded = await this.#client!.stage(this.#generation, bsp, profile, this.#dependencies, key)
      this.#requireOperation(operation)
      this.#coverageSamples=await this.#client!.coverage(this.#generation)
      this.#requireOperation(operation)
      finishLoadPhase("stage")
      this.#artifacts = await parsePresentationArtifacts(this.#loaded.presentation, this.#dependencyEntries)
      finishLoadPhase("presentationParse")
      this.#resetMapBlockers()
      this.#recordVisualOutputBlockers(this.#artifacts)
      await this.#cacheModelArtifacts(this.#artifacts)
      finishLoadPhase("modelCache")
      this.#projectiles = createProjectilePresentationMapper(
        Object.freeze({
          models: new Set(this.#artifacts.models.keys()),
          systems: PARTICLE_SYSTEMS,
          attachments: this.#attachments,
          attachmentTransforms: this.#attachmentTransforms,
          fireAttachmentTransforms: this.#fireAttachmentTransforms,
          localOwnerIdentity: 1,
        }),
      )
      this.#viewmodels = createViewmodelPresenter(this.#artifacts)
      this.#viewmodelClass = undefined
      this.#applyInitialView(this.#loaded)
      finishLoadPhase("presentationSetup")
      this.#renderer = await createRenderer({
        canvas: this.#canvas,
        configuration: this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR,
        powerPreference: "high-performance",
      })
      finishLoadPhase("rendererCreate")
      this.#resizeRenderer()
      this.#advanceLoading("creating-client-world")
      const scene = await this.#renderer.loadMap({
        payload: this.#loaded.payload,
        payloadSha256: this.#loaded.payloadSha256,
        directionalTextures: this.#artifacts.directionalTextures,
        environment: this.#artifacts.environment,
        materialStates: this.#materialStates(this.#artifacts),
        particleTextures: this.#artifacts.particleTextures,
        modelOccurrences: this.#artifacts.modelOccurrences,
        modelFacing: this.#modelFacing(this.#artifacts),
        modelMaterials: this.#artifacts.modelMaterials,
        authoredTextures: this.#artifacts.authoredTextures,
        brushModels:this.#artifacts.brushModels,
        staticProps:this.#artifacts.staticProps,
        diagnostic: true,
      })
      this.#environmentDrawables = scene.environmentDrawables
      this.#canvas.dataset.staticProps=JSON.stringify(scene.staticProps)
      this.#canvas.dataset.runtimeStaticProps=JSON.stringify(scene.runtimeStaticProps)
      this.#publishProfileDisplacements(scene)
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`${diagnostic.code}: ${diagnostic.identity} — ${diagnostic.detail}`)
      }
      finishLoadPhase("rendererLoadMap")
      const AudioContextConstructor = window.AudioContext
      if (!AudioContextConstructor) throw new Error("Web Audio is unavailable")
      const audioContext = new AudioContextConstructor()
      const audioPaths = this.#artifacts.audio.documents.some((document) => document.logicalPath === "scripts/game_sounds_vo.txt")
        ? [...SOUND_PATHS, ...CTF_SOUND_PATHS]
        : SOUND_PATHS
      const audioResources = await Promise.all(
        audioPaths.map(async (identity) => {
          const bytes = this.#dependencyEntries.get(identity)
          if (!bytes) throw new Error(`Audio dependency ${identity} is missing`)
          const buffer = await audioContext.decodeAudioData(bytes.slice().buffer)
          return Object.freeze({ identity, buffer })
        }),
      )
      this.#audio = createAudioSystem(audioContext, audioResources)
      this.#audioContext = audioContext
      this.#audioBuffers = new Map(audioResources.map((resource) => [resource.identity, resource.buffer]))
      this.#audioRegistry = new SoundRegistry(this.#artifacts.audio.documents.map((document) => Object.freeze({
        logicalPath: document.logicalPath,
        mode: "base" as const,
        preload: false,
        entries: document.entries,
      })))
      this.#audioWorld = new SourceAudioWorld(this.#audioRegistry, { maxActiveVoices: 128 })
      finishLoadPhase("audioSetup")
      this.#requireOperation(operation)
      await this.#client!.activate(this.#generation)
      finishLoadPhase("activation")
      this.#paused = document.hidden
      this.#resetTeamSelection()
      this.#gameUi?.dispatch({ kind: "loading-progress", phase: "complete" })
      this.#gameUi?.dispatch({ kind: "loading-succeeded" })
      this.#syncLoadingPresentation()
      this.#set({ gameUi: "in-game", detail: "Select a team" })
      const admission = new Promise<void>((resolve, reject) => {
        this.#teamAdmission = Object.freeze({ generation: this.#generation, resolve, reject })
      })
      await this.#showTeamSelection()
      await admission
      this.#requireOperation(operation)
      this.#advanceLoading("synchronizing-game-state")
      this.#snapshot = (await this.#initialPublication(this.#generation)).snapshot
      if (this.#pendingLocalMatch?.mapIdentity === target.target) {
        this.#botConfiguration = this.#pendingLocalMatch.configuration
        this.#pendingLocalMatch = undefined
      }
      this.#requireOperation(operation)
      this.#predictedEye.reset(this.#snapshot.tick, tf2Camera(this.#snapshot, this.#yaw, this.#pitch).position)
      finishLoadPhase("initialPublication")
      this.#recordAuthorityBlockers(this.#snapshot)
      this.#recordCrouch(this.#snapshot)
      this.#recordLockerAnimations(this.#snapshot)
      this.#modelProbes = await this.#probePlayerModels(this.#artifacts)
      this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(this.#artifacts)
      finishLoadPhase("initialProbes")
      const persistence = await this.#loaded.persistence
      this.#requireOperation(operation)
      finishLoadPhase("persistence")
      this.#paused = document.hidden
      this.#resetHudIntegration()
      this.#resetClassSelection()
      this.#publishProfileCoverage()
      const loadPerformanceProbe=JSON.stringify({
        totalMilliseconds:performance.now()-loadStarted,
        application:loadTimings,
        client:{
          ...this.#loaded.timings,
          mapCacheWriteMilliseconds:persistence.mapCacheWriteMilliseconds,
          presentationCacheWriteMilliseconds:persistence.presentationCacheWriteMilliseconds,
        },
        mapBytes:this.#loaded.payload.byteLength,
        presentationBytes:this.#loaded.presentation.byteLength,
        mapCache:this.#loaded.cache,
        presentationCache:persistence.presentationCache,
        presentationCacheError:persistence.presentationCacheError,
      })
      this.#syncLoadingPresentation()
      this.#set({
        phase: "Ready",
        generation: this.#generation,
        gameUi: "in-game",
        camera: tf2Camera(this.#snapshot, this.#yaw, this.#pitch),
        detail: "Click the field to capture the mouse",
        loadPerformanceProbe,
        cache: this.#loaded.cache,
        fireEvents: 0,
        explosionEvents: 0,
        particleRenderItems: 0,
        audioStarts: Object.freeze([]),
        initialView: this.#loaded.initialView,
        environment: this.#artifacts.environment,
        environmentDrawables: this.#environmentDrawables,
        movement: this.#snapshot.movement,
        playerFlags: this.#snapshot.playerFlags,
        inWater: this.#snapshot.inWater,
        movementTick: this.#snapshot.movementTick,
        modelProbes: this.#modelProbes,
        snapshotTick: this.#snapshot.tick.toString(),
        projectileStates: this.#snapshot.projectiles.map((projectile) => `${projectile.identity}:${projectile.state}`).join(","),
        decalProbe: this.#decalProbe(this.#artifacts),
        modelOccurrenceCount: this.#artifacts.modelOccurrences.length,
        viewmodelSequences: this.#viewmodelSequences(this.#artifacts, this.#snapshot.class),
        crouchHistory: Object.freeze([...this.#crouchHistory]),
        viewmodelTimelineProbes: Object.freeze([...this.#viewmodelTimelineProbes]),
        modelMatrices: this.#modelMatrices(this.#artifacts),
        decalStateProbe: this.#decalStateProbe(this.#artifacts),
        modelMaterialProbe: this.#modelMaterialProbe(this.#artifacts),
        lockerProbe:this.#lockerProbe(this.#snapshot.tick),
        ...this.#gameplayTraces(this.#snapshot),
        ...this.#snapshotProbes(this.#snapshot),
        loadingProgress: 1,
      })
      if (this.#snapshot.team === 2 || this.#snapshot.team === 3) this.#showClassSelection(true)
    } catch (error) {
      if (!this.#operations.current(operation)) return
      await this.#teardownGameplay()
      if (this.#closed || !this.#operations.current(operation)) return
      const detail = error instanceof Error ? error.message : "Gameplay startup failed"
      this.#gameUi?.dispatch({ kind: "loading-failed", reason: "Map load failed", extendedReason: detail.slice(0, 255) })
      this.#syncLoadingPresentation()
      this.#set({ phase: "Failed", gameUi: "failure", detail })
    }
  }

  #resetGenerationPresentation(): void {
    this.#predictedEye.clear()
    this.#particleBatches = createParticleBatchEncoder()
    this.#pendingProjectileTimeline = []
    this.#teamSelectionPoses.clear()
    this.#teamSelectionAnimations.clear()
    this.#pendingClassSelectionTeam = undefined
    this.#pendingPresentation = undefined
    this.#preparedPresentation = undefined
    this.#requiredParticleDisplayFrames.reset()
    this.#skyVisibility.clear()
    this.#canvasDiagnostics.clear()
    this.#canvas.dataset.sky3dPass = ""
    this.#canvas.dataset.skyVisibilityDisposition = "not-visible"
    this.#nextSimulationSampleSeconds = 0
    this.#lastRenderedPreparedRevision = 0
    this.#lastRenderedViewRevision = 0
    this.#lastRenderedTick = undefined
    this.#resumePointerOnAcknowledge = false
    this.#fireEvents = 0
    this.#explosionEvents = 0
    this.#audioStarts = []
    this.#pendingAudioRequests = []
    this.#pyroFlameEffect = undefined
    this.#combatTracerCount = 0
    this.#combatDecalCount = 0
    this.#pyroEffectSerial = 0
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
  }

  #resetHudIntegration(): void {
    if (!this.#uiResources || !this.#presentationRandom || !this.#artifacts) throw new Error("TF2 HUD resources are unavailable")
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    this.#hudScoreboardIdentity = ""
    this.#scoreboardVisible = false
    if (this.#hudIntegration) {
      this.#hudIntegration.reset("map-replaced")
      return
    }
    const damageTexture=this.#artifacts.environment.textures.find(texture=>texture.material.toLowerCase()==="materials/vgui/damageindicator.vmt")
    if(!damageTexture)throw new Error("Authored TF2 damage indicator material is unavailable")
    this.#hudIntegration = initializeTf2HudIntegration({
      root: this.#hudRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      damageIndicator:Object.freeze({material:damageTexture.material,texture:damageTexture,
        eyePosition:()=>this.#snapshot?tf2Camera(this.#snapshot,this.#yaw,this.#pitch).position:[0,0,0],
        yawDegrees:()=>this.#yaw,random:()=>this.#presentationRandom!.nextUnit()}),
      onCommand: (command) => {

        if (command.kind === "select-weapon" && (command.weapon >= 1 && command.weapon <= 18 || command.weapon >= 40 && command.weapon <= 45 || command.weapon >= 50 && command.weapon <= 54) && command.weapon !== 54) this.#selectWeapon = command.weapon as Tf2Weapon
        else if (command.kind === "scoreboard") this.#setScoreboardVisible(command.visible)

      },
    })
    const panels = this.#hudIntegration.snapshot().vgui.panels
    this.#hudRootCounts = Object.freeze({
      playerStatus: panels.filter((panel) => panel.name === "HudPlayerStatus").length,
      ammo: panels.filter((panel) => panel.name === "HudWeaponAmmo").length,
    })
  }

  #resetClassSelection(): void {
    if (!this.#uiResources || !this.#presentationRandom) throw new Error("TF2 class selection resources are unavailable")
    if (this.#classSelection) {
      this.#classSelection.dispatch({ kind: "hide" })
      return
    }
    this.#classSelection = initializeTf2ClassSelectionIntegration({
      root: this.#classSelectionRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      onRequest: (request) => this.#classSelectionRequest(request),
      onModelPanels: (panels) => {
        this.#classSelectionModelPanels = panels
        this.#classSelectionRenderRevision += 1
        this.#set({
          classSelectionVisible: panels.length > 0,
          classSelectionTeam: this.#classSelection?.state().team ?? undefined,
          classSelectionSelected: this.#classSelection?.state().selected,
          classSelectionModels: panels.map((panel) => `${panel.name}:${panel.model}:${panel.skin}`).join("|"),
        })
      },
    })
  }

  #classSelectionRequest(request: Tf2ClassSelectionRequest): void {
    const identity = request.identity
    if (identity === 12) this.#selectClass = 12
    else this.selectClass(identity)
    this.#set({ classSelectionVisible: false, classSelectionModels: "" })
  }

  #showClassSelection(initialJoin = false): void {
    if (!this.#classSelection || !this.#snapshot || this.#view.gameUi !== "in-game"
      || (this.#snapshot.team !== 2 && this.#snapshot.team !== 3)) return
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) void document.exitPointerLock()
    this.#classSelection.dispatch({
      kind: "show",
      team: this.#snapshot.team,
      current: initialJoin ? null : this.#snapshot.class as Tf2ClassIdentity,
    })
  }

  #renderClassSelection(): void {
    if (!this.#renderer || this.#classSelectionModelPanels.length === 0 || this.#classSelectionRenderTask) return
    const renderer = this.#renderer
    const revision = this.#classSelectionRenderRevision
    const generation = this.#generation
    const panels = this.#classSelectionModelPanels.map((panel) => Object.freeze({
      identity: panel.name,
      model: panel.model,
      skin: panel.skin,
      horizontalFov4By3: panel.fov,
      origin: panel.origin,
      angles: panel.angles,
      bounds: panel.bounds,
    }))
    this.#classSelectionRenderTask = renderer.renderModelPanels(panels)
      .then((result) => {
        if (generation !== this.#generation || revision !== this.#classSelectionRenderRevision) return
        this.#set({ classSelectionModels: result.panels.map((panel) => `${panel.identity}:${panel.model}:${panel.skin}:${panel.primitives}`).join("|") })
      })
      .catch((error) => {
        if (generation !== this.#generation || !this.#classSelection?.state().visible) return
        this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "TF2 class model rendering failed" })
      })
      .finally(() => { this.#classSelectionRenderTask = undefined })
  }

  #resetTeamSelection(): void {
    if (!this.#uiResources || !this.#presentationRandom) throw new Error("TF2 team selection resources are unavailable")
    if (this.#teamSelection) {
      this.#teamSelection.dispatch({ kind: "hide" })
      return
    }
    this.#teamSelection = initializeTf2TeamSelectionIntegration({
      root: this.#teamSelectionRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      onRequest: (request) => { void this.#teamSelectionRequest(request) },
      onModelPanels: (panels) => {
        this.#teamSelectionModelPanels = panels
        this.#teamSelectionRenderRevision += 1
        const server = this.#teamSelection?.state().server
        this.#set({
          teamSelectionVisible: panels.length > 0,
          teamSelectionLocal: server?.localTeam,
          teamSelectionRedCount: server?.redCount,
          teamSelectionBlueCount: server?.blueCount,
          teamSelectionModels: panels.map((panel) => `${panel.name}:${panel.model}:${panel.animation}`).join("|"),
        })
      },
    })
  }

  async #teamSelectionRequest(request: Tf2TeamSelectionRequest): Promise<void> {
    if (!this.#client || !this.#teamSelection) return
    const generation = this.#generation
    const server = await this.#client.teamSelection(generation, request.team)
    if (generation !== this.#generation || this.#closed) return
    if (this.#snapshot && (server.localTeam === 2 || server.localTeam === 3)
      && this.#snapshot.team !== server.localTeam) {
      this.#pendingClassSelectionTeam = server.localTeam
    }
    this.#set({
      teamSelectionVisible: false,
      teamSelectionLocal: server.localTeam,
      teamSelectionRedCount: server.redCount,
      teamSelectionBlueCount: server.blueCount,
      teamSelectionModels: "",
      detail: `Team selected: ${request.team}`,
    })
    if (this.#teamAdmission?.generation === generation) {
      const admission = this.#teamAdmission
      this.#teamAdmission = undefined
      admission.resolve()
    }
  }

  async #showTeamSelection(): Promise<void> {
    if (!this.#teamSelection || !this.#client || this.#view.gameUi !== "in-game") return
    const generation = this.#generation
    const server = await this.#client.teamSelection(generation)
    if (generation !== this.#generation || this.#closed) return
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) await document.exitPointerLock()
    this.#teamSelection.dispatch({ kind: "show", server })
  }

  #renderTeamSelection(): void {
    if (!this.#renderer || !this.#client || !this.#artifacts
      || this.#teamSelectionModelPanels.length === 0 || this.#teamSelectionRenderTask) return
    const renderer = this.#renderer
    const client = this.#client
    const artifacts = this.#artifacts
    const revision = this.#teamSelectionRenderRevision
    const generation = this.#generation
    const now = this.#frameClock.current
    const authored = this.#teamSelectionModelPanels
    const viewport = this.#viewport()
    this.#teamSelectionRenderTask = (async () => {
      const requests = authored.flatMap((panel, index) => {
        if (panel.sequence === "idle") return []
        const artifact = artifacts.models.get(panel.model.toLowerCase())
        const timing = artifact?.sequences.find((sequence) => sequence.label.toLowerCase() === panel.sequence.toLowerCase())
        if (!artifact || !timing) throw new Error(`TF2 authored team-door sequence is unavailable: ${panel.model}:${panel.sequence}`)
        const prior = this.#teamSelectionAnimations.get(panel.name)
        const current = prior?.sequence === panel.sequence
          ? prior
          : Object.freeze({ sequence: panel.sequence, startedSeconds: now, previousSeconds: 0 })
        const elapsed = Math.max(0, now - current.startedSeconds)
        if (elapsed > timing.durationSeconds && this.#teamSelectionPoses.has(panel.name)) return []
        const request = Object.freeze({
          identity: 0x1000 + index,
          model: panel.model.toLowerCase(),
          activity: panel.sequence,
          previousElapsedSeconds: Math.min(current.previousSeconds, elapsed),
          elapsedSeconds: elapsed,
          currentTimeSeconds: now,
          frameTimeSeconds: Math.max(0, elapsed - current.previousSeconds),
          planarSpeed: 0,
          screenAspectRatio: viewport.width / viewport.height,
          worldFarPlane: 1000,
          skin: panel.skin,
          lod: 0,
          bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
        })
        this.#teamSelectionAnimations.set(panel.name, Object.freeze({ ...current, previousSeconds: elapsed }))
        return [Object.freeze({ panel: panel.name, request })]
      })
      if (requests.length > 0) {
        const posed = decodeModelPoseOutput(await client.models(generation, encodeModelPoseBatch(requests.map((value) => value.request))))
        if (generation !== this.#generation || !this.#teamSelection?.state().visible) return
        for (const item of posed) {
          const selected = requests.find((candidate) => candidate.request.identity === item.identity)
          if (!selected) throw new Error("TF2 team-door pose identity differs from its authored request")
          this.#teamSelectionPoses.set(selected.panel, item)
        }
      }
      const panels = authored.map((panel) => {
        const pose = this.#teamSelectionPoses.get(panel.name)
        return Object.freeze({
          identity: panel.name,
          model: panel.model,
          skin: panel.skin,
          horizontalFov4By3: panel.fov,
          origin: panel.origin,
          angles: panel.angles,
          bounds: panel.bounds,
          presentationTimeSeconds: now,
          ...(pose ? { pose: Object.freeze({ primitives: pose.primitives }) } : {}),
        })
      })
      const result = await renderer.renderModelPanels(panels)
      if (generation !== this.#generation || revision !== this.#teamSelectionRenderRevision) return
      this.#set({ teamSelectionModels: result.panels.map((panel) => `${panel.identity}:${panel.model}:${panel.skin}:${panel.primitives}`).join("|") })
    })().catch((error) => {
      if (generation !== this.#generation || !this.#teamSelection?.state().visible) return
      this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "TF2 team model rendering failed" })
    }).finally(() => { this.#teamSelectionRenderTask = undefined })
  }

  #hudPresentationObservation(
    observation?: ReturnType<Tf2HudIntegration["probe"]>,
    currentBinding?: Tf2HudBinding,
  ): string {
    const integration = this.#hudIntegration
    if (!integration || !this.#hudRootCounts) return "unavailable"
    const probe = observation ?? integration.probe()
    const binding = currentBinding ?? integration.snapshot().binding
    const panel = (name: string) => probe.panels.find((value) => value.name === name)
    const classImage = panel("PlayerStatusClassImage")
    const classModel = panel("classmodelpanel")
    const modelIdentity = binding?.values.find((value) => value.kind === "dialog-variable" && value.panel === "classmodelpanel" && value.variable === "modelIdentity")
    const conditionPanels = probe.panels.filter((value) => /(?:Bleed|Milk|Marked|Slowed|Gas|Resist|Buff|Rune|Parachute|WheelOfDoom)/u.test(value.name))
    return JSON.stringify({
      classImage: classImage ? { visible: classImage.effectivelyVisible, image: classImage.state.image } : null,
      classModel: classModel ? { visible: classModel.effectivelyVisible, model: modelIdentity?.kind === "dialog-variable" && modelIdentity.value.kind === "available" ? modelIdentity.value.value : null, scalars: classModel.state.scalarProperties } : null,
      classImageBackground: panel("PlayerStatusClassImageBG")?.state.image ?? null,
      classModelBackground: panel("classmodelpanelBG")?.state.image ?? null,
      ammoBackground: panel("HudWeaponAmmoBG")?.state.image ?? null,
      roots: this.#hudRootCounts,
      activeConditions: conditionPanels.filter((value) => value.effectivelyVisible).map((value) => value.name),
    })
  }

  #replaceCrosshair(settings: Tf2CrosshairSettings): void {
    this.#crosshairSettings = settings
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    if (this.#hudIntegration && this.#snapshot) {
      this.#hudIntegration.setCrosshair(this.#currentHudContext(this.#snapshot).crosshair)
    }
    if (this.#console) this.#console.apply({ kind: "replace-catalog", catalog: this.#catalog() })
  }

  #currentHudContext(snapshot: Snapshot): SessionHudContext {
    if (!this.#crosshairSettings) throw new Error("TF2 crosshair settings are unavailable")
    const respawnAllowed = snapshot.lifecycle === 2
    const paused = this.#view.gameUi === "pause"
    const loadingImage = this.#view.gameUi === "loading" || this.#view.phase === "Loading" || this.#view.phase === "Replacing"
    const clientModeAllows = this.#view.gameUi === "in-game"
    const classSelection = this.#view.classSelectionVisible === true
    const vguiInput = this.#view.consoleVisible || this.#view.optionsVisible === true || classSelection || this.#view.teamSelectionVisible === true
    const identity = Number(respawnAllowed)
      | Number(paused) << 1
      | Number(vguiInput) << 2
      | Number(this.#playerClassUsePlayerModel) << 3
      | Number(loadingImage) << 4
      | Number(clientModeAllows) << 5
      | Number(snapshot.weapon !== null) << 6
      | Number(this.#scoreboardVisible) << 7
      | Number(this.#scoreboardPingAsText) << 8
    const scoreboardIdentity = `${snapshot.team}:${snapshot.scoreboard.redScore}:${snapshot.scoreboard.blueScore}:${snapshot.scoreboard.redCount}:${snapshot.scoreboard.blueCount}:${snapshot.scoreboard.players.map((player) => `${player.identity},${player.team},${player.class},${Number(player.alive)},${player.score},${player.kills},${player.deaths},${player.captures},${player.damage}`).join(";")}`
    if (this.#hudContext && this.#hudContextIdentity === identity && this.#hudScoreboardIdentity === scoreboardIdentity) return this.#hudContext
    this.#hudContextIdentity = identity
    this.#hudScoreboardIdentity = scoreboardIdentity
    this.#hudContext = Object.freeze({
      playerIdentity: 1,
      liveHudSuppressed: classSelection || this.#view.teamSelectionVisible === true,
      respawnAllowed,
      weaponSelection: Object.freeze({ open: false, selectedWeapon: tf2HudUnavailable<number>("not-produced") }),
      crosshair: Object.freeze({
        configured: true,
        weaponAllows: snapshot.weapon !== null,
        loadingImage,
        paused,
        clientModeAllows,
        frozen: false,
        localViewEntity: true,
        vguiInput,
        observerMode: "none" as const,
        observerCrosshair: true,
        tfSuppressed: false,
        countdownHidden: false,
        ...tf2CrosshairHudValues(this.#crosshairSettings),
        weaponScale: 1,
      }),
      scoreboard: tf2HudAvailable<Tf2HudScoreboard>(adaptTf2Scoreboard(
        snapshot.scoreboard,
        snapshot.team,
        this.#scoreboardVisible,
        this.#mapIdentity,
        this.#scoreboardPingAsText,
      )),
      freezePanel: tf2HudUnavailable<Tf2HudFreezePanel>("not-produced"),
      playerClassUsePlayerModel: this.#playerClassUsePlayerModel,
    })
    return this.#hudContext
  }

  #initializeConsole(): void {
    if(!this.#consoleResources)throw new Error("Configured VGUI resources unavailable")
    const initialized = initializeDeveloperConsole({
      runtimeIdentity: "tf2-jump-console",
      limits: consoleLimits,
      resources:this.#consoleResources.console,
      catalog: this.#catalog(),
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      onRequest: (request) => this.#consoleRequest(request),
    })
    if (!initialized.ok) throw new Error(`VGUI console initialization failed: ${initialized.diagnostic.code}`)
    this.#console = initialized.console
    const mounted = this.#console.apply({ kind: "mount", root: this.#vguiRoot })
    if (!mounted.ok) throw new Error(`VGUI console mount failed: ${mounted.diagnostic.code}`)
    this.#console.apply({
      kind: "append-output",
      segments: [
        { kind: "developer", text: "playsrc TF2 jump practice\nType status for exact support information.\n" },
      ],
    })
    for (const output of this.#pendingConsoleOutput) this.#output(output.text, output.developer)
    this.#pendingConsoleOutput = []
    const diagnostics = initializeClientDiagnostics({
      runtimeIdentity: "tf2-client-diagnostics",
      resources:this.#consoleResources.diagnostics,
      viewport: this.#viewport(),
    })
    if (!diagnostics.ok) throw new Error(`VGUI diagnostics initialization failed: ${diagnostics.code}`)
    this.#diagnostics = diagnostics.diagnostics
    const mountedDiagnostics = this.#diagnostics.apply({ kind: "mount", root: this.#vguiRoot })
    if (!mountedDiagnostics.ok) throw new Error(`VGUI diagnostics mount failed: ${mountedDiagnostics.code}`)
  }

  #catalog(): ConsoleCatalog {
    const crosshair = this.#settings?.crosshairConVars() ?? []
    const combatSettings=["tf_dingalingaling","tf_dingalingaling_lasthit"] as const
    return Object.freeze({
      revision: `tf2-jump-catalog-developer-${this.#developer}-console-${Number(this.#consoleEnabled)}-fps-${this.#showFps}-pos-${this.#showPos}-hdr-${this.#renderLevel}-settings-${this.#settings?.snapshot().settings.revision ?? 0}`,
      items: Object.freeze([
        Object.freeze({
          kind: "command" as const,
          name: "map",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "class",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "changeteam",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "jointeam",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "changeclass",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "joinclass",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        ...["build","destroy","hurtbuilding","+attack","-attack"].map(name=>Object.freeze({kind:"command" as const,name,disposition:"visible" as const,acceptsSuggestions:false})),
        Object.freeze({
          kind: "command" as const,
          name: "dropitem",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "tf_bot_add",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "tf_bot_kick",
          disposition: "visible" as const,
          acceptsSuggestions: true,
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "tf_bot_difficulty",
          disposition: "visible" as const,
          displayValue: String(this.#botDifficulty),
        }),
        ...([
          ["tf_bot_quota", String(this.#activeBotConfiguration?.quota ?? 0)],
          ["tf_bot_quota_mode", this.#activeBotConfiguration?.mode ?? "normal"],
          ["tf_bot_join_after_player", String(Number(this.#activeBotConfiguration?.joinAfterPlayer ?? true))],
          ["tf_bot_auto_vacate", String(Number(this.#activeBotConfiguration?.autoVacate ?? true))],
          ["tf_bot_offline_practice", String(Number(this.#activeBotConfiguration?.offlinePractice ?? false))],
        ] as const).map(([name, displayValue]) => Object.freeze({
          kind: "convar" as const,
          name,
          disposition: "visible" as const,
          displayValue,
        })),
        Object.freeze({
          kind: "command" as const,
          name: "setpos",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "disguise",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "noclip",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "status",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "command" as const,
          name: "clear",
          disposition: "visible" as const,
          acceptsSuggestions: false,
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "con_enable",
          disposition: "visible" as const,
          displayValue: String(Number(this.#consoleEnabled)),
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "cl_hud_playerclass_use_playermodel",
          disposition: "visible" as const,
          displayValue: String(Number(this.#playerClassUsePlayerModel)),
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "developer",
          disposition: "visible" as const,
          displayValue: String(this.#developer),
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "cl_showfps",
          disposition: "visible" as const,
          displayValue: String(this.#showFps),
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "cl_showpos",
          disposition: "visible" as const,
          displayValue: String(this.#showPos),
        }),
        Object.freeze({
          kind: "convar" as const,
          name: "mat_hdr_level",
          disposition: "visible" as const,
          displayValue: String(this.#renderLevel),
        }),
        ...crosshair.map((setting) => Object.freeze({
          kind: "convar" as const,
          name: setting.name,
          disposition: "visible" as const,
          displayValue: setting.value,
        })),
        ...combatSettings.map(name=>Object.freeze({kind:"convar" as const,name,disposition:"visible" as const,
          displayValue:String(Number(this.#settings?.snapshot().settings.current[name]===true))})),
      ]),
    })
  }

  #viewport(): ApplicationPresentationViewport {
    if (!this.#presentationViewport) throw new Error("TF2 presentation viewport is suspended")
    return this.#presentationViewport
  }

  #resizeRenderer(viewport = this.#viewport()): void {
    this.#renderer?.resize(viewport.width, viewport.height, viewport.devicePixelRatio)
  }

  #commitPresentationViewport(viewport: ApplicationPresentationViewport): void {
    this.#presentationViewport = viewport
    const identity = `${viewport.revision}:${viewport.width}x${viewport.height}@${viewport.devicePixelRatio}`
    for (const owner of [this.#canvas, this.#startupRoot, this.#loadingRoot, this.#gameUiRoot, this.#hudRoot, this.#engineerRoot, this.#classSelectionRoot, this.#teamSelectionRoot, this.#optionsRoot, this.#localMatchRoot, this.#vguiRoot]) {
      owner.dataset.presentationViewport = identity
      owner.dataset.presentationViewportState = "active"
    }
    this.#resizeRenderer(viewport)
    this.#console?.apply({ kind: "set-viewport", viewport })
    this.#diagnostics?.apply({ kind: "set-viewport", viewport })
    this.#gameUi?.setViewport(viewport)
    this.#syncGameUiBackgroundProbe()
    this.#hudIntegration?.setViewport(viewport)
    this.#engineer?.setViewport(viewport)
    this.#classSelection?.setViewport(viewport)
    this.#teamSelection?.setViewport(viewport)
    this.#options?.setViewport(viewport)
    this.#localMatch?.setViewport(viewport)
    this.#loadingVgui?.setViewport(viewport)
    if (this.#loadingPresentationGeneration > 0 && this.#configuration) {
      const target = this.#loadingTarget ?? this.#activeTarget
      if (!target) throw new Error("TF2 loading target is unavailable")
      this.#loadingBackground = this.#resolveLoadingBackground(target, viewport)
      this.#syncLoadingPresentation(viewport)
    }
  }

  #suspendPresentationViewport(): void {
    this.#presentationViewport = undefined
    for (const owner of [this.#canvas, this.#startupRoot, this.#loadingRoot, this.#gameUiRoot, this.#hudRoot, this.#engineerRoot, this.#classSelectionRoot, this.#teamSelectionRoot, this.#optionsRoot, this.#localMatchRoot, this.#vguiRoot]) {
      delete owner.dataset.presentationViewport
      owner.dataset.presentationViewportState = "suspended"
    }
  }

  #consoleRequest(request: ConsoleRequest): void {
    if (!this.#console) return
    if (request.kind === "visibility") {
      this.#console.apply({ kind: "hide" })
      this.#set({ consoleVisible: false })
      return
    }
    if (request.kind === "completion") {
      const candidates =
        request.commandName.toLowerCase() === "map"
          ? this.#configuration.targets.map((target) => `map ${target.target}`)
          : request.commandName.toLowerCase() === "class"
            ? TF2_CLASS_NAMES.map((name) => `class ${name}`)
            : request.commandName.toLowerCase() === "jointeam"
              ? ["jointeam auto", "jointeam blue", "jointeam red", "jointeam spectate"]
              : request.commandName.toLowerCase() === "joinclass"
                ? ["scout", "soldier", "pyro", "demoman", "heavyweapons", "engineer", "medic", "sniper", "spy", "random"].map((name) => `joinclass ${name}`)
                : request.commandName.toLowerCase() === "tf_bot_add"
                  ? TF2_CLASS_NAMES.flatMap((name) => [`tf_bot_add red ${name}`, `tf_bot_add blue ${name}`])
                  : request.commandName.toLowerCase() === "tf_bot_kick"
                    ? ["tf_bot_kick all", "tf_bot_kick red", "tf_bot_kick blue"]
                    : []
      const suggestions: ConsoleCompletionSuggestion[] = candidates
        .filter((value) => value.startsWith(request.partialText.toLowerCase()))
        .slice(0, request.maxItems)
        .map((text) => Object.freeze({ text, disposition: "visible" as const }))
      this.#console.apply({
        kind: "apply-completion",
        result: { requestId: request.requestId, catalogRevision: request.catalogRevision, suggestions },
      })
      return
    }
    if (request.kind === "submission") void this.#execute(request.text)
  }

  #output(text: string, developer = false): void {
    if (!this.#console) {
      if (this.#pendingConsoleOutput.length < 256) this.#pendingConsoleOutput.push(Object.freeze({ text, developer }))
      return
    }
    this.#console.apply({
      kind: "append-output",
      segments: [{ kind: developer ? "developer" : "normal", text: `${text}\n` }],
    })
  }

  async #execute(input: string): Promise<void> {
    const tokens = input.trim().split(/\s+/u)
    const command = tokens.shift()?.toLowerCase()
    if (!command) return
    if (tokens.length > 63) {
      this.#output("Command rejected: more than 64 arguments.")
      return
    }
    if (command === "clear" && tokens.length === 0) {
      this.#console?.apply({ kind: "clear-output" })
      return
    }
    if (command === "status" && tokens.length === 0) {
      this.#output(
        `generation ${this.#generation}; map ${this.#mapIdentity}; cache ${this.#loaded?.cache}`,
        true,
      )
      for (const blocker of [...this.#blockers].sort()) this.#output(`BLOCKED: ${blocker}`)
      return
    }
    const crosshair = TF2_CROSSHAIR_SETTINGS.find((setting) => setting.name === command)
    if(command==="tf_dingalingaling"||command==="tf_dingalingaling_lasthit"){
      if(!this.#settings||tokens.length>1||(tokens[0]!==undefined&&tokens[0]!=="0"&&tokens[0]!=="1")){
        this.#output(`${command} accepts exactly 0 or 1`)
        return
      }
      if(tokens[0]!==undefined){
        this.#settings.synchronize({[command]:tokens[0]==="1"})
        localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY,new TextDecoder().decode(this.#settings.persistence()))
        this.#console?.apply({kind:"replace-catalog",catalog:this.#catalog()})
      }
      this.#output(`"${command}" = "${Number(this.#settings.snapshot().settings.current[command]===true)}"`)
      return
    }
    if (crosshair) {
      if (!this.#settings) {
        this.#output(`ERROR: ${command} settings authority is unavailable`)
        return
      }
      if (tokens.length > 1) {
        this.#output(`${command} accepts exactly one value`)
        return
      }
      if (tokens.length === 1) {
        try {
          const value = tokens[0] === '""' ? "" : tokens[0]!
          this.#settings.setCrosshairConVar(command, value)
          this.#replaceCrosshair(tf2CrosshairSettings(this.#settings.snapshot().settings.current))
          localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY, new TextDecoder().decode(this.#settings.persistence()))
          this.#set({ settingsPersistence: "stored" })
        } catch (error) {
          this.#output(`ERROR: ${error instanceof Error ? error.message : "crosshair setting failed"}`)
          return
        }
      }
      const current = this.#settings.crosshairConVars().find((setting) => setting.name === command)!
      this.#output(`"${command}" = "${current.value}" ( def. "${current.defaultValue}" )`)
      return
    }
    if (command === "developer" && tokens.length <= 1) {
      if (tokens.length === 1 && tokens[0] !== "0" && tokens[0] !== "1") {
        this.#output("developer accepts exactly 0 or 1")
        return
      }
      if (tokens[0]) {
        this.#developer = Number(tokens[0])
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`developer = ${this.#developer}`, true)
      return
    }
    if (command === "cl_hud_playerclass_use_playermodel" && tokens.length <= 1) {
      if (tokens.length === 1 && tokens[0] !== "0" && tokens[0] !== "1") {
        this.#output(`${command} accepts exactly 0 or 1`)
        return
      }
      if (tokens[0]) {
        this.#playerClassUsePlayerModel = tokens[0] === "1"
        this.#hudIntegration?.setPlayerClassUsePlayerModel(this.#playerClassUsePlayerModel)
        this.#hudContext = undefined
        this.#hudContextIdentity = -1
        if (this.#settings?.snapshot().settings.activeTransactionId === null) {
          this.#settings.synchronize({ [command]: this.#playerClassUsePlayerModel })
          localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY, new TextDecoder().decode(this.#settings.persistence()))
          this.#set({ settingsPersistence: "stored" })
        }
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        this.#set({ hudPresentationProbe: this.#hudPresentationObservation() })
      }
      this.#output(`"${command}" = "${Number(this.#playerClassUsePlayerModel)}" ( def. "1" )`)
      return
    }
    if (command === "con_enable" && tokens.length <= 1) {
      if (tokens.length === 1 && tokens[0] !== "0" && tokens[0] !== "1") {
        this.#output("con_enable accepts exactly 0 or 1")
        return
      }
      if (tokens[0]) {
        this.#consoleEnabled = tokens[0] === "1"
        if (this.#settings?.snapshot().settings.activeTransactionId === null) {
          this.#settings.synchronize({ "keyboard.console-enabled": this.#consoleEnabled })
          localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY, new TextDecoder().decode(this.#settings.persistence()))
        }
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`"con_enable" = "${Number(this.#consoleEnabled)}" ( def. "0" )`)
      return
    }
    if (command === "cl_showfps" || command === "cl_showpos") {
      if (tokens.length > 1 || (tokens.length === 1 && tokens[0] !== "0" && tokens[0] !== "1" && tokens[0] !== "2")) {
        this.#output(`${command} accepts exactly 0, 1, or 2`)
        return
      }
      if (tokens[0]) {
        const value = Number(tokens[0]) as ClientDiagnosticMode
        if (command === "cl_showfps") this.#showFps = value
        else this.#showPos = value
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        this.#updateDiagnostics(performance.now())
      }
      const value = command === "cl_showfps" ? this.#showFps : this.#showPos
      this.#output(`"${command}" = "${value}"${value === 0 ? "" : ' ( def. "0" )'} min. 0.000000 max. 2.000000`)
      return
    }
    if (command === "changeclass" && tokens.length === 0) {
      this.#showClassSelection()
      return
    }
    if (command === "joinclass" && tokens.length === 1) {
      const selected = tf2ClassSelectionByName(tokens[0]!)
      if (!selected) {
        this.#output(`Unknown TF2 class: ${tokens[0]}`)
        return
      }
      this.#classSelectionRequest({ kind: "join-class", identity: selected.identity, sourceCommand: `joinclass ${selected.name}` })
      this.#classSelection?.dispatch({ kind: "hide" })
      return
    }
    if (command === "changeteam" && tokens.length === 0) {
      void this.#showTeamSelection()
      return
    }
    if (command === "jointeam" && tokens.length === 1) {
      const team = tokens[0]!.toLowerCase()
      if (team !== "red" && team !== "blue" && team !== "auto" && team !== "spectate") {
        this.#output(`Unknown TF2 team: ${tokens[0]}`)
        return
      }
      await this.#teamSelectionRequest({ kind: "join-team", team, sourceCommand: `jointeam ${team}` })
      this.#teamSelection?.dispatch({ kind: "hide" })
      return
    }
    if (command === "dropitem" && tokens.length === 0) {
      if (!this.#snapshot?.objectives?.flags.some((flag) => flag.carrier === 1)) {
        this.#output("dropitem rejected: you are not carrying the intelligence")
        return
      }
      this.#dropItem = true
      this.#output("Intelligence drop queued")
      return
    }
    if((command==="+attack"||command==="-attack")&&tokens.length===0){if(command==="+attack"){if(this.#buttons.press("console:+attack","+attack"))this.#firePressed=true}else this.#buttons.release("console:+attack");return}
    if((command==="build"||command==="destroy")&&(tokens.length===1||tokens.length===2)){
      const kind=Number(tokens[0]),mode=Number(tokens[1]??"0")
      if(this.#snapshot?.class!==9||![0,1,2].includes(kind)||![0,1].includes(mode)||(kind!==1&&mode!==0)){this.#output(`Usage: ${command} <0|1|2> [0|1]`);return}
      this.#buildingRequest=Object.freeze({action:command,object:Object.freeze({kind:kind as 0|1|2,mode:mode as 0|1})})
      if(command==="destroy")this.#selectWeapon=42
      this.#output(`Queued ${command} ${kind} ${mode}`)
      return
    }
    if(command==="hurtbuilding"&&tokens.length===1){const amount=Number(tokens[0]);if(this.#snapshot?.class!==9||!Number.isSafeInteger(amount)||amount<0||amount>65535){this.#output("Usage: hurtbuilding <0-65535>");return}this.#buildingRequest={action:"hurt",amount};return}
    if (command === "tf_bot_difficulty" && tokens.length <= 1) {
      if (tokens.length === 1 && !["0", "1", "2", "3"].includes(tokens[0]!)) {
        this.#output("tf_bot_difficulty accepts exactly 0, 1, 2, or 3")
        return
      }
      if (tokens[0]) {
        this.#botDifficulty = Number(tokens[0]) as 0 | 1 | 2 | 3
        if (this.#activeBotConfiguration) {
          this.#activeBotConfiguration = Object.freeze({ ...this.#activeBotConfiguration, difficulty: this.#botDifficulty })
          this.#botConfiguration = this.#activeBotConfiguration
        }
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`"tf_bot_difficulty" = "${this.#botDifficulty}" ( def. "1" )`)
      return
    }
    if (["tf_bot_quota", "tf_bot_quota_mode", "tf_bot_join_after_player", "tf_bot_auto_vacate"].includes(command) && tokens.length <= 1) {
      const current = this.#activeBotConfiguration ?? Object.freeze({
        quota: 0,
        maximumPlayers: 32,
        mode: "normal" as const,
        difficulty: this.#botDifficulty,
        joinAfterPlayer: true,
        autoVacate: true,
        offlinePractice: false,
      })
      if (tokens.length === 1) {
        const value = tokens[0]!
        if (command === "tf_bot_quota") {
          if (!/^(?:0|[1-9][0-9]*)$/u.test(value) || Number(value) > 31) {
            this.#output("tf_bot_quota accepts exactly 0 through 31")
            return
          }
          this.#activeBotConfiguration = Object.freeze({ ...current, quota: Number(value) })
        } else if (command === "tf_bot_quota_mode") {
          if (!["normal", "fill", "match"].includes(value)) {
            this.#output("tf_bot_quota_mode accepts exactly normal, fill, or match")
            return
          }
          this.#activeBotConfiguration = Object.freeze({ ...current, mode: value as BotQuotaMode })
        } else {
          if (value !== "0" && value !== "1") {
            this.#output(`${command} accepts exactly 0 or 1`)
            return
          }
          this.#activeBotConfiguration = Object.freeze({
            ...current,
            ...(command === "tf_bot_join_after_player" ? { joinAfterPlayer: value === "1" } : { autoVacate: value === "1" }),
          })
        }
        if (this.#snapshot && this.#dependencyEntries.has(`maps/${this.#mapIdentity}.nav`)) {
          this.#botConfiguration = this.#activeBotConfiguration
        }
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      const next = this.#activeBotConfiguration ?? current
      const displayed = command === "tf_bot_quota" ? String(next.quota)
        : command === "tf_bot_quota_mode" ? next.mode
          : command === "tf_bot_join_after_player" ? String(Number(next.joinAfterPlayer)) : String(Number(next.autoVacate))
      const defaults = command === "tf_bot_quota" ? "0" : command === "tf_bot_quota_mode" ? "normal" : "1"
      this.#output(`"${command}" = "${displayed}" ( def. "${defaults}" )`)
      return
    }
    if (command === "tf_bot_offline_practice" && tokens.length === 0) {
      this.#output(`"tf_bot_offline_practice" = "${Number(this.#activeBotConfiguration?.offlinePractice ?? false)}" ( def. "0" )`)
      return
    }
    if (command === "tf_bot_add") {
      if (!this.#snapshot || !this.#dependencyEntries.has(`maps/${this.#mapIdentity}.nav`)) {
        this.#output("tf_bot_add rejected: the active map has no authored TF2 navigation mesh")
        return
      }
      let count = 1
      let identity: Tf2Class | undefined
      let team: Tf2Team | undefined
      let difficulty = this.#botDifficulty
      for (const raw of tokens) {
        const token = raw.toLowerCase()
        const selectedClass = tf2ClassFromName(token)
        if (selectedClass !== undefined) identity = selectedClass
        else if (token === "red") team = 2
        else if (token === "blue") team = 3
        else if (["easy", "normal", "hard", "expert"].includes(token)) {
          difficulty = ["easy", "normal", "hard", "expert"].indexOf(token) as 0 | 1 | 2 | 3
        } else if (/^[1-9][0-9]*$/u.test(token)) {
          count = Number(token)
        } else if (token !== "noquota") {
          this.#output(`Invalid argument '${raw}'`)
          return
        }
      }
      if (!Number.isSafeInteger(count) || count > 31 || count + this.#snapshot.bots.length > 31) {
        this.#output("tf_bot_add rejected: the Source player limit would be exceeded")
        return
      }
      const selectedClass = identity === undefined ? undefined : tf2ClassPresentation(identity)
      if (selectedClass && !this.#artifacts?.models.has(selectedClass.model)) {
        this.#output(`tf_bot_add rejected: authored player model is unavailable: ${selectedClass.model}`)
        return
      }
      if (this.#botRequest) {
        this.#output("tf_bot_add rejected: an earlier bot command is pending")
        return
      }
      this.#botRequest = Object.freeze({ action: "add", count, ...(identity ? { class: identity } : {}), ...(team ? { team } : {}), difficulty })
      if (this.#activeBotConfiguration) {
        this.#activeBotConfiguration = Object.freeze({
          ...this.#activeBotConfiguration,
          quota: Math.min(31, this.#activeBotConfiguration.quota + count),
        })
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`Queued ${count} ${team === 2 ? "RED" : team === 3 ? "BLU" : "auto-team"} ${selectedClass?.displayName ?? "preset-roster"} bot${count === 1 ? "" : "s"}`)
      return
    }
    if (command === "tf_bot_kick" && tokens.length === 1) {
      if (!this.#snapshot || !this.#dependencyEntries.has(`maps/${this.#mapIdentity}.nav`)) {
        this.#output("tf_bot_kick rejected: the active map has no authored TF2 navigation mesh")
        return
      }
      const token = tokens[0]!.toLowerCase()
      if (token === "all") this.#botRequest = Object.freeze({ action: "kick-all" })
      else if (token === "red" || token === "blue") this.#botRequest = Object.freeze({ action: "kick-team", team: token === "red" ? 2 : 3 })
      else {
        this.#output("Usage: tf_bot_kick all|red|blue")
        return
      }
      if (this.#activeBotConfiguration) {
        const removed = token === "all" ? this.#snapshot.bots.length
          : this.#snapshot.bots.filter((bot) => bot.team === (token === "red" ? 2 : 3)).length
        this.#activeBotConfiguration = Object.freeze({
          ...this.#activeBotConfiguration,
          quota: Math.max(0, this.#activeBotConfiguration.quota - removed),
        })
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`Queued bot removal: ${token}`)
      return
    }
    if (command === "class" && tokens.length === 1) {
      const identity = tf2ClassFromName(tokens[0]!.toLowerCase())
      if (identity === undefined) {
        this.#output(`Usage: class ${TF2_CLASS_NAMES.join("|")}`)
        return
      }
      this.selectClass(identity)
      this.#output(`Class selection queued: ${tokens[0]}`)
      return
    }
    if (command === "setpos") {
      if (!this.#client || !this.#snapshot || tokens.length < 2 || tokens.length > 3) {
        this.#output("Usage: setpos x y <z optional>")
        return
      }
      const coordinates = [Number(tokens[0]), Number(tokens[1]), tokens[2] === undefined ? this.#snapshot.position[2] : Number(tokens[2])]
      if (!coordinates.every(Number.isFinite)) {
        this.#output("setpos rejected: coordinates must be finite")
        return
      }
      const generation = this.#generation
      void this.#client.setPosition(generation, coordinates as [number, number, number]).then(() => {
        if (generation === this.#generation) this.#output(`Position set: ${coordinates.join(" ")}`)
      }, (error) => {
        if (generation === this.#generation) this.#output(`setpos rejected: ${error instanceof Error ? error.message : String(error)}`)
      })
      return
    }
    if (command === "disguise") {
      const selectedClass = Number(tokens[0]), selectedTeam = Number(tokens[1])
      if (tokens.length !== 2 || !Number.isInteger(selectedClass) || selectedClass < 1 || selectedClass > 9 || (selectedTeam !== 2 && selectedTeam !== 3)) {
        this.#output("Usage: disguise <class 1-9> <team 2|3>")
        return
      }
      if (this.#snapshot?.class !== 8) {
        this.#output("disguise rejected: only Spy can disguise")
        return
      }
      this.#disguise = Object.freeze({ class: selectedClass as Tf2Class, team: selectedTeam as Tf2Team })
      this.#output(`Disguise selection queued: ${selectedClass} ${selectedTeam}`)
      return
    }
    if (command === "noclip" && tokens.length === 0) {
      if (!this.#snapshot) {
        this.#output("noclip rejected: no authoritative snapshot")
        return
      }
      this.#modeRequest = this.#snapshot.movement.mode === 1 ? 0 : 1
      this.#output(`noclip ${this.#modeRequest === 1 ? "ON" : "OFF"} queued`)
      return
    }
    if (command === "mat_hdr_level") {
      if (tokens.length > 1 || (tokens.length === 1 && !(["0", "1", "2"] as string[]).includes(tokens[0]!))) {
        this.#output("mat_hdr_level accepts exactly 0, 1, or 2")
        return
      }
      if (tokens[0] && Number(tokens[0]) !== this.#renderLevel) {
        const prior = this.#renderLevel,
          generation = this.#generation
        this.#renderLevel = Number(tokens[0]) as 0 | 1 | 2
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        await this.#replaceCatalogMap()
        if (this.#generation === generation) {
          this.#renderLevel = prior
          this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        }
      }
      this.#output(`mat_hdr_level = ${this.#renderLevel}`)
      return
    }
    if (command === "map" && tokens.length === 1) {
      const target = this.#configuration?.targets.find((candidate) => candidate.target === tokens[0])
      if (target) {
        if (this.#view.phase === "Startup") this.#startup?.key("Escape")
        if (this.#gameUi?.state().kind === "failure") {
          const dismissed = this.#gameUi.dispatch({ kind: "dismiss-failure" })
          if (dismissed.disposition === "applied") {
            this.#set({ phase: "MainMenu", gameUi: "main-menu", detail: "TF2 Main Menu" })
          }
        }
        if (this.#client && this.#renderer && this.#loaded) await this.#switchCatalogMap(target)
        else {
          const transition = this.#gameUi?.dispatch({ kind: "map", mapIdentity: target.target })
          if (transition?.disposition !== "applied") this.#output(`ERROR: map rejected: ${transition?.reason ?? "GameUI unavailable"}`)
        }
      }
      else if (tokens[0]?.startsWith("https://")) await this.#replaceExternalMap(tokens[0])
      else this.#output(`Usage: map ${this.#configuration?.targets.map((candidate) => candidate.target).join("|") ?? "<unavailable>"}`)
      return
    }
    this.#output(`Unknown command: ${command}`)
  }

  async #replaceCatalogMap(): Promise<void> {
    if (!this.#activeTarget) return
    await this.#switchCatalogMap(this.#activeTarget)
  }

  async #switchCatalogMap(target: BrowserTargetConfiguration): Promise<void> {
    if (!this.#configuration || !this.#resourceCatalog) return
    if (!this.#client || !this.#renderer || !this.#loaded) {
      const transition = this.#gameUi?.dispatch({ kind: "map", mapIdentity: target.target })
      if (transition?.disposition !== "applied") throw new Error(`map rejected: ${transition?.reason ?? "GameUI unavailable"}`)
      return
    }
    const operation = this.#nextOperation()
    const previousGeneration = this.#generation
    const previousBlockers = new Set(this.#blockers)
    try {
      const signal = operation.signal
      this.#loadingTarget = target
      this.#beginLoadingPresentation()
      this.#set({ phase: "Replacing", detail: `Loading ${target.target} through exact catalog identity`, loadingBackground: this.#loadingBackground?.disposition })
      const selected = selectCatalogTarget(this.#resourceCatalog, target.target)
      if (selected.resources.sha256 !== target.objects.resources.sha256 || selected.resources.byteLength !== target.objects.resources.byteLength) throw new Error("Target resource root differs from catalog")
      const [bytes, graphBytes] = await Promise.all([
        fetchImmutableObject(this.#configuration.assetOrigin, target.objects.bsp, signal),
        fetchImmutableObject(this.#configuration.assetOrigin, target.objects.resources, signal),
      ])
      this.#requireOperation(operation)
      const graph = parseResourceGraphBytes(graphBytes)
      if (graph.target !== target.target || graph.contentBuild !== target.contentBuild) throw new Error("Target resource graph identity differs")
      const entries = new Map(this.#sharedDependencyEntries)
      const dependencies = await this.#decodeResourceSet(graph, ["gameplay"], entries, signal)
      this.#requireOperation(operation)
      await this.#replace(bytes, target.objects.bsp.sha256, target.target, { target, graph, dependencies, entries }, operation)
      if (this.#operations.current(operation)) this.#loadingTarget = undefined
    } catch (error) {
      if (!this.#operations.current(operation)) return
      this.#loadingTarget = undefined
      const reason=error instanceof Error?`${error.name}: ${error.message}`:String(error)
      this.#output(`ERROR: Map replacement failed: ${reason}`)
      if (this.#generation !== previousGeneration) {
        this.#paused = true
        this.#set({ phase: "Failed", gameUi: "failure", detail: `Activated map authority failed: ${reason}` })
        return
      }
      await this.#client?.discard(previousGeneration + 1).catch(() => undefined)
      this.#blockers.clear()
      for (const blocker of previousBlockers) this.#blockers.add(blocker)
      this.#paused = document.hidden
      this.#nextSimulationSampleSeconds = 0
      this.#set({ phase: "Ready", gameUi: "in-game", detail: `Prior map retained: ${reason}` })
    }
  }

  async #replaceExternalMap(value: string): Promise<void> {
    try {
      const source = await this.#externalSource(value)
      this.#set({ phase: "Replacing", detail: `Loading ephemeral ${source.name}` })
      await this.#replace(source.bytes, source.sha256, source.name)
    } catch (error) {
      this.#output(`ERROR: External map failed: ${error instanceof Error ? error.message : "unknown failure"}`)
      this.#paused = document.hidden
      this.#set({ phase: "Ready", detail: "Prior map retained" })
    }
  }

  async #externalSource(value: string): Promise<{ bytes: Uint8Array; sha256: string; name: string }> {
    if (!this.#configuration || new TextEncoder().encode(value).byteLength > 4096) {
      throw new Error("External map URL is invalid")
    }
    const url = new URL(value)
    const match = /\/(?<name>[a-z0-9_-]+)\.bsp$/.exec(url.pathname)
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      !this.#configuration.allowedExternalOrigins.includes(url.origin) ||
      !match?.groups?.name
    ) {
      throw new Error("External map URL is outside the configured HTTPS policy")
    }
    const controller = new AbortController()
    const timeout = window.setTimeout(() => controller.abort(), 30_000)
    try {
      const response = await fetch(url, {
        cache: "no-store",
        credentials: "omit",
        redirect: "error",
        signal: controller.signal,
      })
      const total = Number(response.headers.get("content-length"))
      if (
        response.status !== 200 ||
        response.redirected ||
        response.url !== url.href ||
        !Number.isSafeInteger(total) ||
        total < 1 ||
        total > MAX_EXTERNAL_BYTES ||
        !response.body
      ) {
        throw new Error("External map response metadata is invalid")
      }
      const hash = sha256.create()
      const output = new Uint8Array(total)
      const reader = response.body.getReader()
      let offset = 0
      while (true) {
        const result = await reader.read()
        if (result.done) break
        if (offset + result.value.byteLength > total) {
          await reader.cancel()
          throw new Error("External map response exceeds its declared length")
        }
        output.set(result.value, offset)
        hash.update(result.value)
        offset += result.value.byteLength
      }
      if (offset !== total) throw new Error("External map response is shorter than its declared length")
      return { bytes: output, sha256: bytesToHex(hash.digest()), name: match.groups.name }
    } finally {
      clearTimeout(timeout)
    }
  }

  async #replace(
    bytes: Uint8Array,
    bspSha256: string,
    name: string,
    candidate?: Readonly<{ target: BrowserTargetConfiguration; graph: ResourceGraph; dependencies: Uint8Array; entries: Map<string, Uint8Array> }>,
    operation?: ApplicationOperation,
  ): Promise<void> {
    const replaceStarted=performance.now();let replacePhase=replaceStarted;const replaceTimings:Record<string,number>={};const finishReplacePhase=(phase:string)=>{const now=performance.now();replaceTimings[phase]=now-replacePhase;replacePhase=now}
    if (!this.#client || !this.#renderer || !this.#loaded) throw new Error("Application is not ready")
    this.#paused = true
    this.#neutral()
    this.#predictedEye.suspend()
    this.#simulationSamples.clear()
    await this.#simulationTask
    await this.#presentationTask
    this.#pendingPresentation=undefined
    this.#preparedPresentation=undefined
    this.#requiredParticleDisplayFrames.reset()
    this.#nextSimulationSampleSeconds=0
    await this.#displayTask
    if (operation) this.#requireOperation(operation)
    const generation = this.#generation + 1
    const profile = this.#renderLevel === 2 ? 1 : 0
    const key = await mapDerivedKey(
      bspSha256,
      profile,
      this.#renderLevel,
      this.#configuration?.wasm.sha256 ?? "",
      candidate?.dependencies ?? this.#dependencies,
    )
    finishReplacePhase("derivedKey")
    const staged = await this.#client.stage(generation, bytes, profile, candidate?.dependencies ?? this.#dependencies, key)
    if (operation) this.#requireOperation(operation)
    const coverageSamples=await this.#client.coverage(generation)
    if (operation) this.#requireOperation(operation)
    finishReplacePhase("stage")
    const artifacts = await parsePresentationArtifacts(staged.presentation, candidate?.entries ?? this.#dependencyEntries)
    finishReplacePhase("presentationParse")
    this.#resetMapBlockers()
    this.#recordVisualOutputBlockers(artifacts)
    await this.#cacheModelArtifacts(artifacts)
    finishReplacePhase("modelCache")
    const prior = this.#loaded
    const priorArtifacts = this.#artifacts
    const priorConfiguration = this.#renderer.configuration
    let persistence!:Awaited<LoadedGame["persistence"]>
    try {
      if (this.#renderer.configuration.lightingProfile !== (this.#renderLevel === 2 ? "hdr" : "ldr")) {
        await this.#renderer.dispose()
        this.#renderer = await createRenderer({
          canvas: this.#canvas,
          configuration: this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR,
          powerPreference: "high-performance",
        })
        this.#resizeRenderer()
      }
      const scene = await this.#renderer.loadMap({
        payload: staged.payload,
        payloadSha256: staged.payloadSha256,
        directionalTextures: artifacts.directionalTextures,
        environment: artifacts.environment,
        materialStates: this.#materialStates(artifacts),
        particleTextures: artifacts.particleTextures,
      modelOccurrences: artifacts.modelOccurrences,
      modelFacing: this.#modelFacing(artifacts),
      modelMaterials: artifacts.modelMaterials,
      authoredTextures: artifacts.authoredTextures,
      brushModels:artifacts.brushModels,
      staticProps:artifacts.staticProps,
        diagnostic: true,
      })
      finishReplacePhase("rendererLoadMap")
      this.#environmentDrawables = scene.environmentDrawables
      this.#canvas.dataset.staticProps=JSON.stringify(scene.staticProps)
      this.#canvas.dataset.runtimeStaticProps=JSON.stringify(scene.runtimeStaticProps)
      this.#publishProfileDisplacements(scene)
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`${diagnostic.code}: ${diagnostic.identity} — ${diagnostic.detail}`)
      }
      persistence=await staged.persistence
      finishReplacePhase("persistence")
      if (operation) this.#requireOperation(operation)
      await this.#client.activate(generation)
      finishReplacePhase("activation")
    } catch (error) {
      await this.#client.discard(generation).catch(() => {})
      if (this.#renderer.configuration.lightingProfile !== priorConfiguration.lightingProfile) {
        await this.#renderer.dispose().catch(() => {})
        this.#renderer = await createRenderer({
          canvas: this.#canvas,
          configuration: priorConfiguration,
          powerPreference: "high-performance",
        })
        this.#resizeRenderer()
      }
      await this.#renderer.loadMap({
        payload: prior.payload,
        payloadSha256: prior.payloadSha256,
        directionalTextures: priorArtifacts?.directionalTextures,
        environment: priorArtifacts?.environment,
        materialStates: priorArtifacts ? this.#materialStates(priorArtifacts) : undefined,
        particleTextures: priorArtifacts?.particleTextures,
        modelOccurrences: priorArtifacts?.modelOccurrences,
        modelFacing: priorArtifacts ? this.#modelFacing(priorArtifacts) : undefined,
        modelMaterials: priorArtifacts?.modelMaterials,
        staticProps:priorArtifacts?.staticProps,
        authoredTextures: priorArtifacts?.authoredTextures,
        brushModels:priorArtifacts?.brushModels,
        diagnostic: true,
      })
      throw error
    }
    this.#generation = generation
    this.#resetGenerationPresentation()
    this.#viewmodelSequenceCache.clear()
    if (candidate) {
      this.#activeTarget = candidate.target
      this.#resourceGraph = candidate.graph
      this.#dependencies = candidate.dependencies
      this.#dependencyEntries = candidate.entries
    }
    this.#loaded = staged
    this.#coverageSamples=coverageSamples
    this.#artifacts = artifacts
    this.#lockerAnimations.clear()
    this.#reloadHistory=[]
    this.#fireTickHistory=[]
    this.#wasmCalls={observe:0,models:0,visibility:0,particles:0};this.#maximumScheduledSamples=0;this.#maximumPublicationTicks=0;this.#phaseTimings=[0,0,0,0,0]
    this.#attachments.clear()
    this.#projectiles?.dispose()
    this.#projectiles = createProjectilePresentationMapper(
      Object.freeze({
        models: new Set(artifacts.models.keys()),
        systems: PARTICLE_SYSTEMS,
        attachments: this.#attachments,
        attachmentTransforms: this.#attachmentTransforms,
        fireAttachmentTransforms: this.#fireAttachmentTransforms,
        localOwnerIdentity: 1,
      }),
    )
    this.#viewmodels = createViewmodelPresenter(artifacts)
    this.#viewmodelClass = undefined
    this.#mapIdentity = name
    this.#applyInitialView(staged)
    this.#snapshot = (await this.#initialPublication(generation)).snapshot
    if (this.#pendingLocalMatch?.mapIdentity === name) {
      this.#botConfiguration = this.#pendingLocalMatch.configuration
      this.#pendingLocalMatch = undefined
    }
    if (operation) this.#requireOperation(operation)
    this.#predictedEye.reset(this.#snapshot.tick, tf2Camera(this.#snapshot, this.#yaw, this.#pitch).position)
    this.#resetHudIntegration()
    this.#recordAuthorityBlockers(this.#snapshot)
    this.#crouchHistory = []
    this.#recordCrouch(this.#snapshot)
    this.#modelProbes = await this.#probePlayerModels(artifacts)
    this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(artifacts)
    finishReplacePhase("initialization")
    this.#audio?.reset()
    this.#audioWorld?.reset()
    this.#lastRandomAudioProbe = ""
    this.#lastCollisionMoverProbe = ""
    this.#paused = document.hidden
    this.#publishProfileCoverage()
    this.#output(`Loaded ${name}; generation ${generation}; derived cache ${staged.cache}.`, true)
    this.#set({
      phase: "Ready",
      generation,
      gameUi: "in-game",
      camera: tf2Camera(this.#snapshot, this.#yaw, this.#pitch),
      detail: `Playing ${name}`,
      loadPerformanceProbe:JSON.stringify({totalMilliseconds:performance.now()-replaceStarted,application:replaceTimings,client:{...staged.timings,mapCacheWriteMilliseconds:persistence.mapCacheWriteMilliseconds,presentationCacheWriteMilliseconds:persistence.presentationCacheWriteMilliseconds},mapBytes:staged.payload.byteLength,presentationBytes:staged.presentation.byteLength,mapCache:staged.cache,presentationCache:persistence.presentationCache,presentationCacheError:persistence.presentationCacheError}),
      cache: staged.cache,
      fireEvents: 0,
      explosionEvents: 0,
      particleRenderItems: 0,
      audioStarts: Object.freeze([]),
      initialView: staged.initialView,
      environment: artifacts.environment,
      environmentDrawables: this.#environmentDrawables,
      movement: this.#snapshot.movement,
      playerFlags: this.#snapshot.playerFlags,
      inWater: this.#snapshot.inWater,
      movementTick: this.#snapshot.movementTick,
      modelProbes: this.#modelProbes,
      snapshotTick: this.#snapshot.tick.toString(),
      projectileStates: this.#snapshot.projectiles.map((projectile) => `${projectile.identity}:${projectile.state}`).join(","),
      decalProbe: this.#decalProbe(artifacts),
      modelOccurrenceCount: artifacts.modelOccurrences.length,
      viewmodelSequences: this.#viewmodelSequences(artifacts, this.#snapshot.class),
      crouchHistory: Object.freeze([...this.#crouchHistory]),
      viewmodelTimelineProbes: Object.freeze([...this.#viewmodelTimelineProbes]),
      modelMatrices: this.#modelMatrices(artifacts),
      decalStateProbe: this.#decalStateProbe(artifacts),
      modelMaterialProbe: this.#modelMaterialProbe(artifacts),
      ...this.#gameplayTraces(this.#snapshot),
      ...this.#snapshotProbes(this.#snapshot),
    })
  }

  #applyInitialView(loaded: LoadedGame): void {
    this.#pitch = Math.max(-89, Math.min(89, loaded.initialView.angles[0]))
    this.#yaw = loaded.initialView.angles[1] % 360
    this.#pointerMovementX = 0
    this.#viewRevision += 1
  }

  async #cacheModelArtifacts(artifacts: PresentationArtifacts): Promise<void> {
    if (!this.#cache) return
    for (const artifact of artifacts.models.values()) {
      if (artifact.bytes.length === 0) continue
      try {
        const retained = await this.#cache.read(artifact.sha256)
        if (!retained) await this.#cache.write(artifact.sha256, artifact.sha256, artifact.bytes)
      } catch {
        this.#blockers.add(`ModelArtifactCacheUnavailable: ${artifact.identity}`)
      }
    }
  }

  #materialStates(artifacts: PresentationArtifacts): ReadonlyMap<string, MaterialStateInput> {
    return artifacts.materialStates
  }

  #modelFacing(artifacts: PresentationArtifacts): ReadonlyMap<string, Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>> {
    return new Map([...artifacts.models].map(([identity, artifact]) => [identity.toLowerCase(), Object.freeze({
      frontFace: artifact.descriptor.frontFace,
      cullFace: artifact.descriptor.cullFace,
    })]))
  }

  #decalProbe(artifacts: PresentationArtifacts): string {
    const zeros = artifacts.environment.textures.reduce(
      (total, texture) => total + texture.rgba.reduce((count, value, index) => count + (index % 4 === 3 && value === 0 ? 1 : 0), 0),
      0,
    )
    return `${artifacts.environment.textures.length}:${zeros}:${artifacts.environment.markFragments}`
  }

  #decalStateProbe(artifacts: PresentationArtifacts): NonNullable<ApplicationView["decalStateProbe"]> {
    const materials = new Set(
      artifacts.environment.markRecords
        .filter((mark) => mark.status === 0 && mark.enabled)
        .map((mark) => mark.material.toLowerCase()),
    )
    let exact = 0
    for (const material of materials) {
      const state = artifacts.materialStates.get(material)
      if (
        state?.blendEnabled &&
        state.blendSource === 2 &&
        state.blendDestination === 3 &&
        !state.alphaTest &&
        state.cull === 0 &&
        state.depthTest &&
        !state.depthWrite &&
        state.depthFunction === 1 &&
        state.polygonOffset === 1
      ) exact += 1
    }
    return Object.freeze({ materials: materials.size, exact })
  }

  #modelMatrices(artifacts: PresentationArtifacts): NonNullable<ApplicationView["modelMatrices"]> {
    return Object.freeze(artifacts.modelOccurrences.map((occurrence) => Object.freeze({
      entity: occurrence.entity,
      model: occurrence.model,
      matrix: Object.freeze([...occurrence.matrix]),
    })))
  }

  #modelMaterialProbe(artifacts: PresentationArtifacts): string {
    const shaders = new Map<string, number>()
    for (const material of artifacts.modelMaterials.values()) {
      shaders.set(material.shader, (shaders.get(material.shader) ?? 0) + 1)
    }
    const planes = [...artifacts.authoredTextures.values()].reduce((total, texture) => total + texture.planes.length, 0)
    return `${artifacts.modelMaterials.size}:${artifacts.authoredTextures.size}:${planes}:${[...shaders].sort().map(([shader, count]) => `${shader}=${count}`).join(",")}`
  }

  #skyController(artifacts:PresentationArtifacts):Readonly<{origin:readonly[number,number,number];scale:number;area:number;fog:import("@playsrc/game-tf2-browser/artifacts").FogArtifact}>|null{
    const controller=artifacts.environment.controllersState.find(value=>value.kind===1),state=controller?.state
    if(!state||!("origin" in state)||!("scale" in state)||!("area" in state)||!("fog" in state)||!Array.isArray(state.origin)||state.origin.length!==3||!state.origin.every(Number.isFinite)||!Number.isSafeInteger(state.scale)||state.scale<=0||!Number.isSafeInteger(state.area))return null
    return Object.freeze({origin:state.origin,scale:state.scale,area:state.area,fog:state.fog})
  }

  #mainFog(artifacts:PresentationArtifacts):import("@playsrc/game-tf2-browser/artifacts").FogArtifact|undefined{
    const index=artifacts.environment.masterFogController
    if(index===null)return undefined
    const state=artifacts.environment.controllersState[index]?.state
    return state&&"enabled" in state&&"primary" in state?state:undefined
  }

  #resetMapBlockers(): void {
    this.#blockers.clear()
    for (const blocker of this.#sharedBlockers) this.#blockers.add(blocker)
  }

  #recordVisualOutputBlockers(artifacts: PresentationArtifacts): void {
    if (
      artifacts.models.has("models/weapons/v_models/v_rocketlauncher_soldier.mdl") ||
      artifacts.models.has("models/weapons/v_models/v_stickybomb_launcher_demo.mdl")
    ) {
      this.#blockers.add("Missing TF2 stock viewmodel composition: class hand model, item c_model attachment, and animation-library join")
    }
    this.#blockers.add("Missing current model lightcache selections, game-owned eye targets, and per-draw StudioModel lighting/eye state")
  }

  #recordAuthorityBlockers(snapshot: Snapshot): void {
    for (const blocker of snapshot.authorityBlockers) this.#blockers.add(`${blocker.classification}: ${blocker.detail}`)
  }

  #gameplayTraces(snapshot: Snapshot): Pick<ApplicationView, "weaponTrace" | "spyProbe" | "authorityTrace" | "entityTrace"> {
    return Object.freeze({
      weaponTrace: snapshot.loadout.map((weapon) =>
        `${weapon.weapon}:${weapon.clip}/${weapon.reserve}:${weapon.reload}:${weapon.reloadDueTick ?? "-"}:${weapon.chargeBeginTick ?? "-"}:${weapon.firstPrimaryTick}`,
      ).join("|"),
      spyProbe: snapshot.spy ? `${snapshot.spy.cloakMeter.toFixed(3)}:${snapshot.spy.invisibility.toFixed(3)}:${snapshot.spy.disguise?.class ?? 0}:${snapshot.spy.disguise?.team ?? 0}:${snapshot.spy.desiredDisguise?.class ?? 0}:${snapshot.spy.desiredDisguise?.team ?? 0}` : undefined,
      authorityTrace: snapshot.authorityBlockers.map((blocker) => `${blocker.code}:${blocker.classification}`).join("|"),
      entityTrace: [
        snapshot.entityEvents.length,
        snapshot.entityTransforms.length,
        snapshot.moverRequests.length,
        snapshot.mapEffects.length,
        snapshot.regenerateAnimationEvents.length,
        snapshot.respawnTouchCount,
      ].join(":"),
    })
  }

  #snapshotProbes(snapshot: Snapshot): Pick<ApplicationView, "randomAudioProbe" | "collisionMoverProbe"> {
    const randomAudio = `${snapshot.randomDraws.length}:${snapshot.audioEvents.length}:${snapshot.randomState.authority.current}:${snapshot.randomState.predictedPresentation.current}:${snapshot.randomState.rocketExplosionAvailable}:${snapshot.randomState.stickyExplosionAvailable}`
    const collisionMover = `${snapshot.collisionSnapshot.identity}:${snapshot.collisionSnapshot.objects}:${snapshot.rocketTraceResults.length}:${snapshot.moverResults.length}`
    if (snapshot.randomDraws.length > 0 || snapshot.audioEvents.length > 0) this.#lastRandomAudioProbe = randomAudio
    if (snapshot.rocketTraceResults.length > 0 || (!this.#lastCollisionMoverProbe && snapshot.moverResults.length > 0)) {
      this.#lastCollisionMoverProbe = collisionMover
    }
    return Object.freeze({
      randomAudioProbe: this.#lastRandomAudioProbe || randomAudio,
      collisionMoverProbe: this.#lastCollisionMoverProbe || collisionMover,
    })
  }

  #viewmodelSequences(artifacts: PresentationArtifacts, tf2Class: Tf2Class): string {
    const cached = this.#viewmodelSequenceCache.get(tf2Class)
    if (cached !== undefined) return cached
    const identity = tf2ClassPresentation(tf2Class).hands
    const sequences = artifacts.models.get(identity)?.sequences
      .filter((sequence) => sequence.timingAvailable)
      .map((sequence) => `${sequence.activity}:${sequence.durationSeconds}`)
      .join("|") ?? ""
    this.#viewmodelSequenceCache.set(tf2Class, sequences)
    return sequences
  }

  #recordCrouch(snapshot: Snapshot): void {
    const value = `${snapshot.tick}:${snapshot.movement.crouchFraction}:${snapshot.movement.viewOffset[2]}`
    if (this.#crouchHistory.at(-1) !== value) this.#crouchHistory.push(value)
    if (this.#crouchHistory.length > 128) this.#crouchHistory.splice(0, this.#crouchHistory.length - 128)
  }

  #recordLockerAnimations(snapshot:Snapshot):void{for(const event of snapshot.regenerateAnimationEvents)this.#lockerAnimations.set(event.associatedModel,Object.freeze({openTick:event.openTick,closeTick:event.closeTick,body:event.body,openAnimation:event.openAnimation,closeAnimation:event.closeAnimation}))}
  #lockerProbe(tick:bigint):string{return [...this.#lockerAnimations].map(([identity,state])=>`${identity}:${tick>=state.closeTick?state.closeAnimation:state.openAnimation}:${state.body}:${state.openTick}-${state.closeTick}`).join("|")}

  async #probePlayerModels(artifacts: PresentationArtifacts): Promise<NonNullable<ApplicationView["modelProbes"]>> {
    if (!this.#client) return Object.freeze([])
    const requests = ["models/player/soldier.mdl", "models/player/demo.mdl"].map((model, index) => {
      const artifact = artifacts.models.get(model)
      if (!artifact || !artifact.sequences.some((sequence) => sequence.activity === "ACT_MP_STAND_PRIMARY")) {
        throw new Error(`Player model probe ${model}:ACT_MP_STAND_PRIMARY is missing`)
      }
      return Object.freeze({
        identity: 0xffff_0000 + index + 1,
        model,
        activity: "ACT_MP_STAND_PRIMARY",
        previousElapsedSeconds: 0,
        elapsedSeconds: 0,
        currentTimeSeconds:0,frameTimeSeconds:0,planarSpeed:0,screenAspectRatio:4/3,worldFarPlane:32768,
        skin: 0,
        lod: 0,
        bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
      })
    })
    const posed = decodeModelPoseOutput(await this.#client.models(this.#generation, encodeModelPoseBatch(requests)))
    return Object.freeze(posed.map((model) => Object.freeze({
      model: model.model,
      sequence: model.sequence,
      primitives: model.primitives.length,
      vertices: model.primitives.reduce((total, primitive) => total + primitive.positions.length / 3, 0),
    })))
  }

  async #probeViewmodelTimelines(artifacts: PresentationArtifacts): Promise<string[]> {
    if (!this.#client) return []
    const model = "models/weapons/c_models/c_soldier_arms.mdl"
    const itemModel = "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
    const artifact = artifacts.models.get(model)
    const itemArtifact = artifacts.models.get(itemModel)
    if (!artifact || !itemArtifact) throw new Error(`Viewmodel timeline probe ${model}+${itemModel} is missing`)
    const activities = ["ACT_PRIMARY_VM_DRAW", "ACT_PRIMARY_VM_IDLE", "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_PRIMARY_RELOAD_START", "ACT_PRIMARY_VM_RELOAD", "ACT_PRIMARY_RELOAD_FINISH"]
    const requests = activities.map((activity, index) => {
      const sequence = artifact.sequences.find((value) => value.activity === activity && value.timingAvailable)
      if (!sequence) throw new Error(`Viewmodel timeline probe ${model}:${activity} is missing`)
      return Object.freeze({
        identity: 0xfffe_0000 + index + 1,
        model,
        itemModel,
        activity,
        previousElapsedSeconds: 0,
        elapsedSeconds: sequence.durationSeconds * 0.5,
        currentTimeSeconds:sequence.durationSeconds*0.5,frameTimeSeconds:0.015,planarSpeed:0,screenAspectRatio:16/9,worldFarPlane:32768,
        phase:([0,5,1,2,3,4] as const)[index]!,reflectedViewmodel:false,ownerAlive:true,
        skin: 0,
        lod: 0,
        bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
        itemBodygroups: Object.freeze(itemArtifact.bodygroupCounts.map(() => 0)),
      })
    })
    const poses = decodeModelPoseOutput(await this.#client.models(this.#generation, encodeModelPoseBatch(requests)))
    return activities.map((activity) => {
      const parts = poses.filter((pose) => pose.activity === activity)
      if(parts.length!==2||parts[0]?.role!=="item"||parts[1]?.role!=="hand")throw new Error(`Viewmodel timeline composition ${activity} differs`);const hand=parts[1]!;return `${activity}:${hand.sequence}:${hand.cycle}:${parts.map(part=>part.primitives.length).join("+")}:${hand.events.length}:item>hand`
    })
  }

  #playAudio(snapshot: Snapshot, camera: Camera): void {
    const incoming = [...tf2Audio(snapshot)]
    for(let ordinal=0;ordinal<snapshot.events.length;ordinal++){
      const event=snapshot.events[ordinal]!
      if(event.kind!==13)continue
      const world=(event.subject&0x8000_0000)!==0
      if(!world&&event.subject===0)continue
      const position=Object.freeze([event.values[0],event.values[1],event.values[2]]) as readonly[number,number,number]
      if(Math.hypot(...position.map((value,axis)=>value-camera.position[axis]!))>=1024)continue
      const material=world?(event.auxiliary>>>24):70
      const selection=material===67||material===84?["Concrete.BulletImpact",4] as const
        :material===87?["Wood.BulletImpact",5] as const
        :material===77||material===86?["SolidMetal.BulletImpact",4] as const
        :material===68||material===78?["Dirt.BulletImpact",4] as const
        :material===89?["Glass.BulletImpact",4] as const
        :material===70||material===66?["Flesh.BulletImpact",5] as const
        :["Default.BulletImpact",3] as const
      incoming.push(Object.freeze({voiceIdentity:Number((snapshot.tick*16384n+BigInt(ordinal)+8192n)&0xffff_ffffn),definition:selection[0],source:Object.freeze({kind:"world",identity:0,ownerIdentity:null,origin:position,radius:0,sourceClass:"world"}),samples:Object.freeze({volume:0,pitch:0,wave:this.#presentationRandom!.nextInteger(0,selection[1]-1),soundLevel:0})}))
    }
    for(let ordinal=0;ordinal<snapshot.events.length;ordinal++){
      const event=snapshot.events[ordinal]!
      if(event.kind!==17||event.auxiliary!==1||event.values[2]!==1||event.subject===1)continue
      incoming.push(Object.freeze({voiceIdentity:Number((snapshot.tick*8192n+BigInt(ordinal)+4096n)&0xffff_ffffn),definition:"TFPlayer.CritHit",source:Object.freeze({kind:"entity",identity:event.subject,ownerIdentity:event.subject,origin:snapshot.bots.find(bot=>bot.identity===event.subject)?.position??camera.position,radius:0,sourceClass:"player"}),samples:Object.freeze({volume:0,pitch:0,wave:this.#presentationRandom!.nextInteger(0,4),soundLevel:0})}))
    }
    const preferences=this.#settings?.snapshot().settings.current
    if(preferences&&(preferences.tf_dingalingaling===true||preferences.tf_dingalingaling_lasthit===true)){
      for(let ordinal=0;ordinal<snapshot.events.length;ordinal++){
        const event=snapshot.events[ordinal]!
        if(event.kind!==17||event.auxiliary!==1||event.subject===1)continue
        const last=event.values[1]<=0&&preferences.tf_dingalingaling_lasthit===true
        if(!last&&preferences.tf_dingalingaling!==true)continue
        const effect=Number(preferences[last?"tf_dingalingaling_last_effect":"tf_dingalingaling_effect"]??0)
        if(effect!==0)throw new Error(`Authored TF2 ${last?"kill":"hit"} sound variant ${effect} is unavailable`)
        const minimum=Number(preferences[last?"tf_dingaling_lasthit_pitchmindmg":"tf_dingaling_pitchmindmg"]??100)
        const maximum=Number(preferences[last?"tf_dingaling_lasthit_pitchmaxdmg":"tf_dingaling_pitchmaxdmg"]??100)
        const fraction=Math.max(0,Math.min(1,(event.values[0]-10)/140))
        incoming.push(Object.freeze({voiceIdentity:Number((snapshot.tick*4096n+BigInt(ordinal))&0xffff_ffffn),definition:last?"Player.KillSoundDefaultDing":"Player.HitSoundDefaultDing",source:Object.freeze({kind:"entity",identity:1,ownerIdentity:1,origin:camera.position,radius:0,sourceClass:"player"}),samples:Object.freeze({volume:0,pitch:0,wave:0,soundLevel:0}),overrides:Object.freeze({volume:Number(preferences[last?"tf_dingaling_lasthit_volume":"tf_dingaling_volume"]??0.75),pitch:minimum+(maximum-minimum)*fraction})}))
      }
    }
    if (!this.#audioRunning || !this.#audio || !this.#audioWorld || !this.#audioRegistry || !this.#audioContext || !this.#artifacts) {
      if (incoming.length > 0) this.#pendingAudioRequests.push(...incoming)
      return
    }
    const requests = this.#pendingAudioRequests.length === 0
      ? incoming
      : [...this.#pendingAudioRequests.splice(0), ...incoming]
    const browserVoices = new Set(this.#audio.activeVoices())
    for (const voice of this.#audioWorld.voices()) if (!browserVoices.has(voice.identity)) this.#audioWorld.stop(voice.identity)
    const yaw = (camera.yawDegrees * Math.PI) / 180, pitch = (camera.pitchDegrees * Math.PI) / 180
    const listener = Object.freeze({
      identity: 1,
      revision: Number(snapshot.tick),
      origin: camera.position,
      forward: Object.freeze([Math.cos(pitch) * Math.cos(yaw), Math.cos(pitch) * Math.sin(yaw), -Math.sin(pitch)]) as readonly [number, number, number],
      right: Object.freeze([Math.sin(yaw), -Math.cos(yaw), 0]) as readonly [number, number, number],
      masterGain: 1,
      categoryGain: 1,
      muted: this.#masterMuted,
    })
    for (const request of requests) {
      const definition = this.#audioRegistry.get(request.definition)
      const resource = definition?.waves[request.samples.wave]?.resource
      const buffer = resource ? this.#audioBuffers.get(resource) : undefined
      if (!resource || !buffer) throw new Error(`Audio resource for ${request.definition} is missing`)
      const started = this.#audioWorld.start({
        voiceIdentity: request.voiceIdentity,
        definition: request.definition,
        source: request.source,
        listener,
        samples: request.samples,
        overrides: request.overrides,
        resourceDurationSeconds: buffer.duration,
        resourceLoopStartSeconds: null,
        resourceChannels: buffer.numberOfChannels,
        resourceAvailable: (identity) => this.#audioBuffers.has(identity),
        scheduledTimeSeconds: this.#audioContext.currentTime,
        delaySeconds: 0,
        mixerGain: this.#artifacts.audio.mixerGain,
        userGain: this.#effectVolume,
        doNotOverwrite: false,
      })
      for (const replaced of started.replaced) this.#audio.stop(replaced)
      this.#audio.playNeutral(started.voice)
      this.#audioStarts.push(`${started.voice.definition}:${started.voice.resource}:${started.voice.channel}:${started.voice.soundLevel}`)
    }
  }

  #updateAttachmentTransforms(snapshot: Snapshot, viewmodels: readonly PosedModel[], camera: Camera): void {
    if (!this.#artifacts) return
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
    for (const projectile of snapshot.projectiles) {
      const artifact = this.#artifacts.models.get(
        projectile.kind === 1 ? "models/weapons/w_models/w_rocket.mdl" : "models/weapons/w_models/w_stickybomb.mdl",
      )
      if (artifact) {
        this.#attachments.set(projectile.identity, new Set(artifact.attachments.keys()))
        this.#attachmentTransforms.set(
          projectile.identity,
          new Map([...artifact.attachments].map(([name, matrix]) => [
            name,
            transformAttachment(matrix, projectile.position, projectile.orientation),
          ])),
        )
      }
    }
    for (const event of snapshot.projectileEvents) {
      if (this.#attachmentTransforms.has(event.projectile)) continue
      const artifact = this.#artifacts.models.get(
        event.kind === 1 ? "models/weapons/w_models/w_rocket.mdl" : "models/weapons/w_models/w_stickybomb.mdl",
      )
      if (!artifact) continue
      this.#attachments.set(event.projectile, new Set(artifact.attachments.keys()))
      this.#attachmentTransforms.set(
        event.projectile,
        new Map([...artifact.attachments].map(([name, matrix]) => [
          name,
          transformAttachment(matrix, event.position, event.orientation),
        ])),
      )
    }
    const cameraOrientation = sourceViewOrientation(camera.pitchDegrees, camera.yawDegrees)
    const currentViewmodels = viewmodels.filter((viewmodel) => !viewmodel.attachmentsOnly)
    const viewmodelAttachments = new Map(
      currentViewmodels.flatMap((viewmodel) => viewmodel.attachments.map((attachment) => [
        attachment.name.toLowerCase(),
        transformAttachment(
          attachment.matrix,
          viewmodel.attachmentsWorld ? [0, 0, 0] : camera.position,
          viewmodel.attachmentsWorld ? [0, 0, 0, 1] : cameraOrientation,
        ),
      ] as const)),
    )
    const launchers = new Set([
      ...snapshot.projectiles.map((projectile) => projectile.launcherIdentity),
      ...snapshot.projectileEvents.map((event) => event.launcherIdentity),
      ...snapshot.events.filter((event) => event.kind === 12).map((event) => event.detail),
      ...(snapshot.weapon === null ? [] : [snapshot.weapon]),
    ])
    for (const launcher of launchers) {
      if (viewmodelAttachments.size > 0) {
        this.#attachmentTransforms.set(launcher, new Map([
          ...(this.#attachmentTransforms.get(launcher) ?? []),
          ...viewmodelAttachments,
        ]))
      }
    }
    const posedFireTicks = new Map(
      viewmodels.filter((pose) => pose.role === "item").map((pose) => [pose.sampleTick, pose]),
    )
    for (const event of snapshot.projectileEvents.filter((value) => value.type === "fire")) {
      const launcherPose = event.launcherPose
      if (event.ownerIdentity !== 1) {
        if (!launcherPose) throw new Error(`TF2 bot fire-tick launcher pose unavailable: ${event.tick}:${event.projectile}`)
        const model = event.kind === 1
          ? "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
          : "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl"
        const artifact = this.#artifacts.models.get(model)
        if (!artifact) throw new Error(`Authored TF2 bot launcher model unavailable: ${model}`)
        const transforms = new Map([...artifact.attachments].map(([name, matrix]) => [
          name.toLowerCase(),
          transformAttachment(matrix, launcherPose.eyePosition, launcherPose.viewOrientation),
        ] as const))
        this.#attachments.set(event.launcherIdentity, new Set(transforms.keys()))
        this.#attachmentTransforms.set(event.launcherIdentity, transforms)
        this.#fireAttachmentTransforms.set(event.projectile, transforms)
        const attachment = event.kind === 1 ? "backblast" : "muzzle"
        if (!transforms.has(attachment)) throw new Error(`Authored TF2 bot launcher attachment unavailable: ${model}:${attachment}`)
        continue
      }
      const firePose = posedFireTicks.get(event.tick)
      if (!launcherPose || !firePose || !firePose.attachmentsWorld) {
        throw new Error(`TF2 fire-tick launcher pose unavailable: ${event.tick}:${event.projectile}; source=${Number(!!launcherPose)}; posed=${Number(!!firePose)}; world=${Number(!!firePose?.attachmentsWorld)}; samples=${[...posedFireTicks.keys()].join(",")}`)
      }
      const transforms = new Map(firePose.attachments.map((attachment) => [
        attachment.name.toLowerCase(),
        transformAttachment(attachment.matrix, [0, 0, 0], [0, 0, 0, 1]),
      ]))
      this.#fireAttachmentTransforms.set(event.projectile, transforms)
      if (event.kind === 1 && event.ownerIdentity === 1) continue
      const attachment = event.kind === 1 ? "backblast" : "muzzle"
      if (!transforms.has(attachment)) {
        this.#blockers.add(`TF2 viewmodel attachment transform unavailable: ${attachment}`)
      }
    }
  }

  #pyroParticles(snapshot: Snapshot): readonly ProjectileParticleRequest[] {
    const requests: ProjectileParticleRequest[]=[]
    const muzzle=snapshot.weapon===null?undefined:this.#attachmentTransforms.get(snapshot.weapon)?.get("muzzle")
    const team=snapshot.team===2?"red" as const:"blue" as const
    const ownerIdentity=1
    const start=(effectIdentity:string,system:string,tick:bigint)=>{
      if(!muzzle)throw new Error(`TF2 Pyro authored muzzle attachment unavailable: ${system}`)
      requests.push(Object.freeze({
        kind:"start",identity:`${effectIdentity}:start:${tick}`,effectIdentity,eventIdentity:`${effectIdentity}:${tick}`,
        tick,projectileIdentity:snapshot.weapon!,ownerIdentity,launcherIdentity:snapshot.weapon!,team,system,
        attachment:Object.freeze({entityIdentity:snapshot.weapon!,name:"muzzle" as const}),
        controlPoints:Object.freeze([Object.freeze({index:0 as const,position:muzzle.position,orientation:muzzle.orientation,ownerIdentity})]),
      }))
    }
    const firing=snapshot.class===7&&snapshot.weapon===15&&snapshot.flameFiring
    if(!firing&&this.#pyroFlameEffect){
      const effectIdentity=this.#pyroFlameEffect
      requests.push(Object.freeze({kind:"stop",identity:`${effectIdentity}:stop:${snapshot.tick}`,effectIdentity,eventIdentity:`${effectIdentity}:stop`,tick:snapshot.tick,projectileIdentity:15,immediate:false}))
      this.#pyroFlameEffect=undefined
    }
    if(firing){
      if(!this.#pyroFlameEffect){
        this.#pyroFlameEffect=`pyro-flame:${++this.#pyroEffectSerial}`
        start(this.#pyroFlameEffect,"new_flame",snapshot.tick)
      } else if(muzzle){
        const effectIdentity=this.#pyroFlameEffect
        requests.push(Object.freeze({kind:"set-control-point",identity:`${effectIdentity}:muzzle:${snapshot.tick}`,effectIdentity,eventIdentity:`${effectIdentity}:follow`,tick:snapshot.tick,projectileIdentity:15,controlPoint:Object.freeze({index:0 as const,position:muzzle.position,orientation:muzzle.orientation,ownerIdentity})}))
      }
      for(const point of snapshot.flamePoints){
        const effectIdentity=this.#pyroFlameEffect
        const age=Math.max(0,Number(snapshot.tick)*0.015-point.spawnTime)
        const density=Math.max(0.01,1-age/point.lifetime*0.99)
        requests.push(Object.freeze({
          kind:"set-flame-control-point",identity:`${effectIdentity}:point:${snapshot.tick}:${point.slot}`,
          effectIdentity,eventIdentity:`${effectIdentity}:point`,tick:snapshot.tick,projectileIdentity:15,
          controlPoint:Object.freeze({index:point.slot+1,position:point.position,orientation:muzzle?.orientation??[0,0,0,1],velocity:Object.freeze(point.velocity.map((value,index)=>value+point.attackerVelocity[index]!) as [number,number,number]),radius:12,density,duration:point.lifetime,ownerIdentity}),
        }))
      }
    }
    for(const activity of snapshot.activities){
      if(activity.weapon===15&&activity.activity===7){
        start(`pyro-blast:${++this.#pyroEffectSerial}`,"pyro_blast",snapshot.tick)
      }else if(activity.weapon===7&&activity.activity===2){
        start(`pyro-shotgun:${++this.#pyroEffectSerial}`,"muzzle_shotgun",snapshot.tick)
      }
    }
    return requests
  }

  #command(): ArrayBuffer {
    const forward = Number(this.#buttons.held("+forward")) - Number(this.#buttons.held("+back"))
    const side = Number(this.#buttons.held("+moveleft")) - Number(this.#buttons.held("+moveright"))
    const attacking = this.#buttons.held("+attack") || this.#firePressed
    const unsupportedProjectile = this.#snapshot?.class === 4 && attacking && (this.#snapshot.weapon === 3 || this.#snapshot.weapon === 18)
    if (unsupportedProjectile) {
      const sticky = this.#snapshot?.weapon === 3
      this.#blockers.add(`Missing exact IVP ${sticky ? "sticky" : "grenade"} rigid-body solver: launch is rejected before projectile creation`)
      this.#set({ unsupportedState: sticky ? "StickyPhysicsSolverUnavailable" : "GrenadePhysicsSolverUnavailable" })
    }
    const command = encodeCommand({
      forward: forward * 450,
      side: side * 450,
      yawDegrees: this.#yaw,
      pitchDegrees: this.#pitch,
      jump: this.#buttons.held("+jump") || this.#jumpPressed,
      crouch: this.#buttons.held("+duck"),
      fire: !unsupportedProjectile && attacking,
      detonate: this.#buttons.held("+attack2") || this.#detonatePressed,
      reload: this.#buttons.held("+reload") || this.#reloadPressed,
      dropItem: this.#dropItem,
      selectClass: this.#selectClass,
      selectWeapon: this.#selectWeapon,
      disguise: this.#disguise,
      modeRequest: this.#modeRequest,
      bot: this.#botRequest,
      building: this.#buildingRequest,
      botConfiguration: this.#botConfiguration,
    })
    this.#selectClass = undefined
    this.#selectWeapon = undefined
    this.#disguise = undefined
    this.#modeRequest = undefined
    this.#botRequest = undefined
    this.#buildingRequest = undefined
    this.#botConfiguration = undefined
    this.#jumpPressed = false
    this.#firePressed = false
    this.#detonatePressed = false
    this.#reloadPressed = false
    this.#dropItem = false
    return command
  }

  readonly #frame = (time: number): void => {
    this.#animationFrame = requestAnimationFrame(this.#frame)
    let timeSeconds: number
    try {
      timeSeconds = this.#frameClock.admit(performance.now() / 1_000)
      const owners = visibleFrameOwners(this.#view, !this.#gameUiRoot.hidden)
      if (owners & GAME_UI_FRAME_OWNER) this.#gameUi?.frame(timeSeconds)
      if (owners & LOADING_FRAME_OWNER) this.#loadingVgui?.frame(timeSeconds)
      if (owners & OPTIONS_FRAME_OWNER) this.#options?.frame(timeSeconds)
      if (owners & HUD_FRAME_OWNER) { this.#hudIntegration?.frame(timeSeconds); this.#engineer?.frame(timeSeconds) }
      if (this.#view.localMatchVisible) this.#localMatch?.frame(timeSeconds)
      if (this.#classSelection?.state().visible) this.#classSelection.frame(timeSeconds)
      if (this.#teamSelection?.state().visible) this.#teamSelection.frame(timeSeconds)
    } catch (error) {
      this.#paused = true
      this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "VGUI frame failed" })
      return
    }
    if (!this.#paused && this.#snapshot && (this.#showFps !== 0 || this.#showPos !== 0)) this.#updateDiagnostics(time)
    if (this.#paused || !this.#client || !this.#renderer) return
    if (!this.#snapshot) {
      if (this.#teamSelection?.state().visible) this.#renderTeamSelection()
      return
    }
    const nowSeconds=timeSeconds
    if(nowSeconds+Number.EPSILON>=this.#nextSimulationSampleSeconds){
      this.#scheduleSimulation(nowSeconds, false)
      if(this.#nextSimulationSampleSeconds===0)this.#nextSimulationSampleSeconds=nowSeconds+SIMULATION_SAMPLE_INTERVAL_SECONDS
      else do{this.#nextSimulationSampleSeconds+=SIMULATION_SAMPLE_INTERVAL_SECONDS}while(this.#nextSimulationSampleSeconds<=nowSeconds)
    }
    if (this.#classSelection?.state().visible) this.#renderClassSelection()
    else if (this.#teamSelection?.state().visible) this.#renderTeamSelection()
    else this.#offerDisplay()
  }

  #offerDisplay():void{
    if (this.#classSelection?.state().visible || this.#teamSelection?.state().visible) return
    const required=this.#requiredParticleDisplayFrames.peek()
    const prepared=required??this.#preparedPresentation
    if(
      this.#displayTask||!prepared||this.#closed||this.#paused||
      (!required&&prepared.revision===this.#lastRenderedPreparedRevision&&this.#viewRevision===this.#lastRenderedViewRevision)
    )return
    const task=this.#renderDisplay(prepared).then(()=>{
      if(required&&prepared.generation===this.#generation)this.#requiredParticleDisplayFrames.complete(required)
    }).catch((error)=>{
      if(this.#closed||prepared.generation!==this.#generation||this.#view.phase==="Replacing")return
      this.#paused=true
      this.#predictedEye.suspend()
      this.#set({phase:"Failed",gameUi:"failure",detail:error instanceof Error?error.message:"Display frame failed"})
    })
    this.#displayTask=task
    void task.finally(()=>{if(this.#displayTask===task)this.#displayTask=undefined})
  }

  async #renderDisplay(prepared:PreparedPresentation):Promise<void>{
    const client=this.#client,renderer=this.#renderer,generation=this.#generation
    if(!prepared||!client||!renderer||prepared.generation!==generation)return
    const viewRevision=this.#viewRevision,mouseRevision=this.#mouseViewRevision,snapRevision=this.#authoritativeViewRevision,yaw=this.#yaw,pitch=this.#pitch
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    const override=profile?.displacementCameraOverride as Partial<Camera>|undefined
    const authorityCamera=tf2Camera(prepared.snapshot,yaw,pitch)
    const ordinaryCamera=Object.freeze({...authorityCamera,position:prepared.visibilityPosition})
    const camera=override&&Array.isArray(override.position)&&override.position.length===3&&override.position.every(Number.isFinite)
      &&Number.isFinite(override.yawDegrees)&&Number.isFinite(override.pitchDegrees)
      ?Object.freeze({...ordinaryCamera,position:Object.freeze([...override.position]) as readonly[number,number,number],yawDegrees:override.yawDegrees!,pitchDegrees:override.pitchDegrees!})
      :ordinaryCamera
    const viewport=this.#viewport()
    const phaseStart=performance.now(),visibilityStart=phaseStart
    const presentationTimeSeconds = Number(prepared.snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS
    const aspectRatio = viewport.width / viewport.height
    const skyController = this.#artifacts ? this.#skyController(this.#artifacts) : null
    let skyRequest: Promise<Readonly<{ camera: Camera; visibility: VisibilityResult }>> | undefined
    const prepareSky = (): Promise<Readonly<{ camera: Camera; visibility: VisibilityResult }>> => {
      if (!skyController) throw new Error("Authored 3D-sky controller disappeared")
      const skyCamera = Object.freeze({
        ...camera,
        position: Object.freeze([
          skyController.origin[0] + camera.position[0] / skyController.scale,
          skyController.origin[1] + camera.position[1] / skyController.scale,
          skyController.origin[2] + camera.position[2] / skyController.scale,
        ]) as readonly [number, number, number],
        near: 2,
        far: 32_768 * 1.732050807569,
      })
      const identity: SkyVisibilityIdentity = {
        generation,
        viewportRevision: viewport.revision,
        tick: prepared.snapshot.tick,
        position: skyCamera.position,
        origin: skyController.origin,
        area: skyController.area,
        yawDegrees: skyCamera.yawDegrees,
        pitchDegrees: skyCamera.pitchDegrees,
        verticalFovDegrees: skyCamera.verticalFovDegrees,
        near: skyCamera.near,
        far: skyCamera.far,
      }
      const cached = this.#skyVisibility.read(identity)
      if (cached) return Promise.resolve({ camera: skyCamera, visibility: cached })
      this.#wasmCalls.visibility += 1
      const request = client.visibility(generation, {
        position: skyCamera.position,
        visibilityPosition: skyController.origin,
        areaFilter: skyController.area,
        yawDegrees: skyCamera.yawDegrees,
        pitchDegrees: skyCamera.pitchDegrees,
        verticalFovDegrees: skyCamera.verticalFovDegrees,
        aspectRatio,
        near: skyCamera.near,
        far: skyCamera.far,
        presentationTimeSeconds,
      }).then((result) => {
        if (result.areas.some((area) => area !== skyController.area)) {
          throw new Error("3D-sky visibility escaped its authored area")
        }
        if (!this.#closed && generation === this.#generation && this.#presentationViewport?.revision === viewport.revision) {
          this.#skyVisibility.write(identity, result)
        }
        return { camera: skyCamera, visibility: result }
      })
      void request.catch(() => {})
      return request
    }
    if (selectAuthoredSky(prepared.visibility.sky, skyController !== null)) skyRequest = prepareSky()
    let visibility = prepared.visibility
    const viewChanged = camera.position.some((value, index) => value !== prepared.visibilityPosition[index])
      || camera.yawDegrees !== prepared.visibilityYaw
      || camera.pitchDegrees !== prepared.visibilityPitch
    if (viewChanged) {
      this.#wasmCalls.visibility += 1
      visibility = await client.visibility(generation, {
        position: camera.position,
        yawDegrees: camera.yawDegrees,
        pitchDegrees: camera.pitchDegrees,
        verticalFovDegrees: camera.verticalFovDegrees,
        aspectRatio,
        near: camera.near,
        far: camera.far,
        presentationTimeSeconds,
      })
    }
    let sky3d: Frame["sky3d"]
    if (selectAuthoredSky(visibility.sky, skyController !== null)) {
      if (!skyController) throw new Error("Authored 3D-sky controller disappeared")
      const result = await (skyRequest ?? prepareSky())
      sky3d = Object.freeze({
        camera: result.camera,
        visibility: Object.freeze({ ...result.visibility, surfaces: result.visibility.drawSurfaces }),
        fog: skyController.fog,
      })
    }
    const visibilityMilliseconds=performance.now()-visibilityStart
    if(this.#closed||this.#paused||generation!==this.#generation||renderer!==this.#renderer)return
    const deltaTicks=this.#lastRenderedTick===undefined
      ? prepared.publication.selectedTicks
      : prepared.snapshot.tick>=this.#lastRenderedTick
        ? Number(prepared.snapshot.tick-this.#lastRenderedTick)
        : prepared.publication.selectedTicks
    const models=prepared.frame.models
    const renderStart=performance.now()
    const rendered=await renderer.render({
      ...prepared.frame,
      models,
      camera,
      visibility:Object.freeze({...visibility,surfaces:visibility.drawSurfaces}),
      fog:this.#artifacts?this.#mainFog(this.#artifacts):undefined,
      sky3d,
      deltaSeconds:deltaTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,
    })
    const renderMilliseconds=performance.now()-renderStart,totalMilliseconds=performance.now()-phaseStart
    if (this.#closed || this.#paused || generation !== this.#generation || renderer !== this.#renderer) return
    this.#canvasDiagnostics.publish(this.#canvas, rendered)
    const skyDisposition=sky3d?"authored":visibility.sky===2?"controller-absent":"not-visible"
    if(this.#canvas.dataset.skyVisibilityDisposition!==skyDisposition)this.#canvas.dataset.skyVisibilityDisposition=skyDisposition
    if(profile){
      if(skyDisposition==="controller-absent")profile.controllerFreeSkyViews=Number(profile.controllerFreeSkyViews??0)+1
      profile.displacementVisibility={surfaces:[...visibility.surfaces],drawSurfaces:[...visibility.drawSurfaces],outsideWorld:visibility.outsideWorld,eyeLeaf:visibility.eyeLeaf,leaves:visibility.leaves,areas:visibility.areas};profile.displacementCamera=camera
      profile.bots=prepared.snapshot.bots.map(bot=>({...bot,weapon:bot.weapon&&{...bot.weapon,nextPrimaryTick:bot.weapon.nextPrimaryTick.toString(),nextReloadTick:bot.weapon.nextReloadTick.toString()},lastFireTick:bot.lastFireTick?.toString()??null,respawnTick:bot.respawnTick?.toString()??null,tick:prepared.snapshot.tick.toString()}))
      profile.combat={tick:prepared.snapshot.tick.toString(),health:prepared.snapshot.health,lifecycle:prepared.snapshot.lifecycle,
        scores:prepared.snapshot.scoreboard.players.map(player=>({...player,killstreak:player.kills,
          respawnTick:prepared.snapshot.bots.find(bot=>bot.identity===player.identity)?.respawnTick?.toString()??null}))}
      profile.pickups=prepared.snapshot.pickups.map(pickup=>({...pickup,respawnTick:pickup.respawnTick?.toString()??null}))
      profile.buildings=prepared.snapshot.buildings.map(building=>({...building,startedTick:building.startedTick.toString(),rechargeEndTick:building.rechargeEndTick?.toString()??null,tick:prepared.snapshot.tick.toString()}))
      profile.placement=prepared.snapshot.placement
      profile.objectives=prepared.snapshot.objectives?.flags.map(flag=>({identity:flag.identity,team:flag.team,position:flag.position}))??[]
    }
    const geometryEvidenceRevision=profile?.geometryEvidenceRevision
    if(profile&&Number.isSafeInteger(geometryEvidenceRevision)&&geometryEvidenceRevision!==((profile.geometryEvidence as {revision?:unknown}|undefined)?.revision)&&this.#view.phase==="Ready"){
      const skyGeometry=sky3d?renderer.captureGeometryEvidence(sky3d.camera,"sky3d"):null
      profile.geometryEvidence=Object.freeze({revision:geometryEvidenceRevision,generation,target:this.#mapIdentity,finalReady:true,identities:Object.freeze({bsp:this.#activeTarget?.objects.bsp.sha256,resourceRoot:this.#activeTarget?.objects.resources.sha256,contentBuild:this.#resourceGraph?.contentBuild,graphTarget:this.#resourceGraph?.target,wasm:this.#configuration?.wasm.sha256,simulationTick:prepared.snapshot.tick.toString()}),camera,visibility:Object.freeze({outsideWorld:visibility.outsideWorld,eyeLeaf:visibility.eyeLeaf,leaves:Object.freeze([...visibility.leaves]),areas:Object.freeze([...visibility.areas]),pvsSurfaces:Object.freeze([...visibility.surfaces]),drawSurfaces:Object.freeze([...visibility.drawSurfaces])}),skyGeometry,geometry:renderer.captureGeometryEvidence(camera)})
    }
    const publishPrepared=prepared.revision!==this.#lastRenderedPreparedRevision
    this.#lastRenderedPreparedRevision=prepared.revision
    this.#lastRenderedViewRevision=viewRevision
    this.#lastRenderedTick=prepared.snapshot.tick
    this.#displayFrame+=1
    this.#phaseTimings=[prepared.modelMilliseconds,visibilityMilliseconds,prepared.particleMilliseconds,renderMilliseconds,totalMilliseconds]
    this.#canvas.dataset.displayFrame=String(this.#displayFrame)
    this.#canvas.dataset.displayViewRevision=String(viewRevision)
    this.#canvas.dataset.displayPreparedRevision=String(prepared.revision)
    this.#canvas.dataset.displayCameraYaw=String(camera.yawDegrees)
    this.#canvas.dataset.displayCameraPitch=String(camera.pitchDegrees)
    if(profile){
      this.#canvas.dataset.displayCameraPosition=camera.position.join(",")
      const displayedViewmodel=models?.find(model=>model.viewModel)
      if(displayedViewmodel?.angles){
        const worldViewmodel=composeViewmodelTransform({position:displayedViewmodel.position,angles:displayedViewmodel.angles},camera)
        this.#canvas.dataset.displayViewmodelPitch=String(worldViewmodel.angles[0])
        this.#canvas.dataset.displayViewmodelYaw=String(worldViewmodel.angles[1])
        this.#canvas.dataset.displayViewmodelPosition=worldViewmodel.position.join(",")
        this.#canvas.dataset.displayViewmodelLocalPitch=String(displayedViewmodel.angles[0])
        this.#canvas.dataset.displayViewmodelLocalYaw=String(displayedViewmodel.angles[1])
        this.#canvas.dataset.displayViewmodelLocalPosition=displayedViewmodel.position.join(",")
      }
    }
    this.#canvas.dataset.displayMouseRevision=String(mouseRevision)
    this.#canvas.dataset.displaySnapRevision=String(snapRevision)
    if(publishPrepared)this.#set({
        camera,
        visibleDecalFragments:rendered.visibleProjectedMarks,
        viewmodelDepthRange:rendered.viewModelPass?.depthRange.join(","),
        viewmodelViewportRestored:rendered.viewModelPass?.viewportRestored,
        viewmodelWorldDepthIsolated:rendered.viewModelPass?.worldDepthCleared,
        waterPlanProbe:`${visibility.water.visibleWater?.eyeInVolume?"below":visibility.water.visibleWater?"above":"none"}:${visibility.water.render.cheap?"cheap":"expensive"}:${visibility.water.render.reflect?1:0}:${visibility.water.render.refract?1:0}:${visibility.water.nearPlaneIntersects?1:0}`,
        waterPasses:rendered.waterPasses,
        waterStateRestored:rendered.waterStateRestored,
        waterNormalFrame:visibility.water.visibleWater?.evaluated.normalFrame,
        worldMaterialFrames:visibility.worldMaterials.map(material=>`${material.identity}:${material.textures.find(texture=>texture.role===7)?.frame??"none"}`).join("|"),
        performanceProbe:`${this.#phaseTimings.map(value=>value.toFixed(3)).join(",")}:${this.#wasmCalls.observe},${this.#wasmCalls.models},${this.#wasmCalls.visibility},${this.#wasmCalls.particles}:${this.#maximumScheduledSamples},${this.#maximumPublicationTicks}:${prepared.particleOutputBytes},${prepared.publication.snapshotBytes.byteLength}`,
        performanceDetailProbe:JSON.stringify({tick:prepared.snapshot.tick.toString(),selectedTicks:prepared.publication.selectedTicks,bots:prepared.snapshot.bots.length,buildings:prepared.snapshot.buildings.length,pickups:prepared.snapshot.pickups.length,models:prepared.modelMilliseconds,projectiles:prepared.projectileMilliseconds,visibility:visibilityMilliseconds,particleWorker:prepared.particleMilliseconds,particleDecode:prepared.particleDecodeMilliseconds,audio:prepared.audioMilliseconds,particleItems:rendered.timings.particleItems,particleBatches:rendered.timings.particleBatches,dynamicItems:rendered.timings.dynamicItemsMilliseconds,world:rendered.timings.worldMilliseconds,viewmodel:rendered.timings.viewModelMilliseconds,render:renderMilliseconds,total:totalMilliseconds}),
        displayFrame:this.#displayFrame,
        displayViewRevision:viewRevision,
        displayPreparedRevision:prepared.revision,
      })
    else if(changedCamera(this.#view.camera,camera))this.#set({camera})
  }

  #updateDiagnostics(realTimeMilliseconds: number): void {
    if (!this.#diagnostics || !this.#snapshot || !this.#mapIdentity) return
    const camera = tf2Camera(this.#snapshot, this.#yaw, this.#pitch)
    this.#diagnostics.apply({
      kind: "present",
      frame: Object.freeze({
        realTimeMilliseconds,
        fpsMode: this.#showFps,
        positionMode: this.#showPos,
        mapIdentity: this.#mapIdentity,
        view: Object.freeze({
          position: Object.freeze([...camera.position]) as readonly [number, number, number],
          angles: Object.freeze([camera.pitchDegrees, camera.yawDegrees, 0]) as readonly [number, number, number],
        }),
        player: Object.freeze({
          position: Object.freeze([...this.#snapshot.position]) as readonly [number, number, number],
          angles: null,
          velocity: Object.freeze([...this.#snapshot.velocity]) as readonly [number, number, number],
        }),
      }),
    })
  }

  async #initialPublication(generation:number):Promise<SimulationPublication>{
    if(!this.#client)throw new Error("Simulation client is unavailable")
    const frame=()=>new Promise<number>(resolve=>requestAnimationFrame(resolve))
    const observe=async(now:number)=>{
      const sampledMovementX=this.#pointerMovementX
      const publications=await this.#client!.observe(generation,this.#frameClock.admit(now/1_000),this.#command(),false)
      for(const publication of publications)this.#applyAuthoritativeView(publication,sampledMovementX)
      return publications
    }
    let now=await frame(),publications=await observe(now)
    if(publications.length)throw new Error("Simulation baseline published gameplay")
    for(let i=0;i<32;i++){
      now=await frame()
      publications=await observe(now)
      if(publications[0])return publications[0]
    }
    throw new Error("Simulation did not publish an initial fixed tick")
  }
  #scheduleSimulation(nowSeconds:number,suspended:boolean):void{
    if(!this.#client||this.#closed)return
    try{this.#simulationSamples.push({generation:this.#generation,nowSeconds,suspended})}catch(error){this.#simulationSamples.clear();this.#paused=true;this.#set({phase:"Failed",detail:error instanceof Error?error.message:"Simulation scheduling failed"});return}
    this.#maximumScheduledSamples=Math.max(this.#maximumScheduledSamples,this.#simulationSamples.length+Number(this.#simulationBusy))
    if(!this.#simulationBusy){const task=this.#drainSimulation();this.#simulationTask=task;void task.finally(()=>{if(this.#simulationTask===task)this.#simulationTask=undefined})}
  }
  async #drainSimulation():Promise<void>{
    this.#simulationBusy=true
    let activeGeneration=this.#generation
    try{
      while(!this.#closed){
        const sample=this.#simulationSamples.shift()
        if(!sample)break
        const client=this.#client
        if(!client||sample.generation!==this.#generation)continue
        activeGeneration=sample.generation
        const sampledMovementX=this.#pointerMovementX,command=this.#command()
        this.#wasmCalls.observe++
        const publications=await client.observe(sample.generation,sample.nowSeconds,command,sample.suspended)
        if(this.#closed||sample.generation!==this.#generation||client!==this.#client)continue
        for(const publication of publications)this.#enqueuePresentation(sample.generation,publication,sampledMovementX)
      }
    }catch(error){
      if(!this.#closed&&activeGeneration===this.#generation&&this.#view.phase!=="Replacing"){
        this.#simulationSamples.clear()
        this.#paused=true
        this.#predictedEye.suspend()
        this.#set({phase:"Failed",gameUi:"failure",detail:error instanceof Error?error.message:"Simulation scheduling failed"})
      }
    }finally{this.#simulationBusy=false}
  }
  #applyAuthoritativeView(publication:SimulationPublication,sampledMovementX:number):void{
    for(const batch of publication.eventBatches){
      for(const event of batch.snapshot.events){
        if(event.kind===9&&event.detail===1){
          this.#yaw=rebasePointerYaw(event.values[3],sampledMovementX,this.#pointerMovementX)
          this.#viewRevision+=1
          this.#authoritativeViewRevision+=1
        }
      }
    }
  }
  #enqueuePresentation(generation:number,publication:SimulationPublication,sampledMovementX:number):void{
    if(generation!==this.#generation||this.#closed)return;this.#applyAuthoritativeView(publication,sampledMovementX);for(const batch of publication.eventBatches){const entry=batch.snapshot.projectileTimeline[0];if(!entry||this.#pendingProjectileTimeline.at(-1)?.tick===entry.tick)continue;if(this.#pendingProjectileTimeline.at(-1)&&this.#pendingProjectileTimeline.at(-1)!.tick>entry.tick){this.#paused=true;this.#set({phase:"Failed",detail:"Projectile presentation timeline reversed before admission"});return}this.#pendingProjectileTimeline.push(entry)}if(this.#pendingProjectileTimeline.length>4096){this.#paused=true;this.#set({phase:"Failed",detail:"Projectile presentation timeline reached its explicit limit"});return}this.#pendingPresentation=this.#pendingPresentation?this.#mergePublications(this.#pendingPresentation,publication):publication;this.#maximumPublicationTicks=Math.max(this.#maximumPublicationTicks,this.#pendingPresentation.selectedTicks);if(!this.#presentationBusy){const task=this.#drainPresentations();this.#presentationTask=task;void task.finally(()=>{if(this.#presentationTask===task)this.#presentationTask=undefined})}
  }
  #mergePublications(left:SimulationPublication,right:SimulationPublication):SimulationPublication{const snapshot=mergePublicationSnapshots([left.snapshot,right.snapshot]);return Object.freeze({...right,firstHostTick:left.firstHostTick,selectedTicks:left.selectedTicks+right.selectedTicks,eventBatches:Object.freeze([...left.eventBatches,...right.eventBatches]),snapshot})}
  async #drainPresentations():Promise<void>{this.#presentationBusy=true;try{while(this.#pendingPresentation&&!this.#closed){const value=this.#pendingPresentation;this.#pendingPresentation=undefined;await this.#present(value)}}finally{this.#presentationBusy=false}}

  async #present(publication: SimulationPublication): Promise<void> {
    if (
      !this.#client ||
      !this.#snapshot ||
      !this.#projectiles ||
      !this.#viewmodels ||
      !this.#artifacts
    )
      return
    const generation=this.#generation,client=this.#client
    const owners=Object.freeze({generation,mapper:this.#projectiles,encoder:this.#particleBatches})
    const ownsGeneration=()=>!this.#closed&&client===this.#client
      &&currentPresentationGeneration(owners,this.#generation,this.#projectiles,this.#particleBatches)
    try {
      if(!ownsGeneration())return
      const authoritySnapshot = mergePublicationSnapshots(publication.eventBatches.map(batch=>batch.snapshot))
      if(authoritySnapshot.tick!==publication.snapshot.tick)throw new Error("Simulation publication differs from its ordered event batches")
      const projectileTimeline=this.#pendingProjectileTimeline.filter(entry=>entry.tick<=authoritySnapshot.tick)
      if(projectileTimeline.length===0)throw new Error("Projectile presentation timeline has no admitted tick")
      const snapshot=Object.freeze({...authoritySnapshot,projectileTimeline:Object.freeze(projectileTimeline),projectileEvents:Object.freeze(projectileTimeline.flatMap(entry=>entry.events))}) as Snapshot
      let previousSnapshot=this.#snapshot
      for(const batch of publication.eventBatches){
        if(batch.snapshot.tick<=previousSnapshot.tick)continue
        const teleported=batch.snapshot.events.some(event=>event.kind===9&&event.detail===1)
        const lifecycleChanged=batch.snapshot.lifecycle!==previousSnapshot.lifecycle
        this.#predictedEye.admit(batch.snapshot.tick,tf2Camera(batch.snapshot,this.#yaw,this.#pitch).position,teleported||lifecycleChanged)
        previousSnapshot=batch.snapshot
      }
      this.#snapshot = snapshot
      if (this.#classSelection?.state().visible && this.#classSelection.state().team !== snapshot.team) {
        this.#classSelection.dispatch({ kind: "team-changed", team: snapshot.team })
      }
      this.#recordCrouch(snapshot)
      this.#recordAuthorityBlockers(snapshot)
      const activeWeapon=snapshot.loadout.find(value=>value.weapon===snapshot.weapon),reloadObservation=activeWeapon&&`${snapshot.tick}:${activeWeapon.weapon}:${activeWeapon.clip}/${activeWeapon.reserve}:${activeWeapon.reload}`
      if(reloadObservation&&this.#reloadHistory.at(-1)!==reloadObservation){this.#reloadHistory.push(reloadObservation);if(this.#reloadHistory.length>128)this.#reloadHistory.shift()}
      for (const event of snapshot.projectileEvents) {
        if (event.type === "fire") {this.#fireEvents += 1;this.#fireTickHistory.push(`${event.kind}:${event.tick}:${event.position.join(",")}`);if(this.#fireTickHistory.length>128)this.#fireTickHistory.shift()}
        if (event.type === "explode") this.#explosionEvents += 1
      }
      for(const event of snapshot.events)if(event.kind===12)this.#fireEvents+=1
      this.#recordLockerAnimations(snapshot)
      for (const p of snapshot.projectiles) {
        const add = (identity: number, next: ReadonlySet<string>) =>
          this.#attachments.set(identity, new Set([...(this.#attachments.get(identity) ?? []), ...next]))
        const m = this.#artifacts.models.get(
          p.kind === 1 ? "models/weapons/w_models/w_rocket.mdl" : "models/weapons/w_models/w_stickybomb.mdl",
        )
        if (m) add(p.identity, new Set(m.attachments.keys()))
        const l = this.#artifacts.models.get(
          p.kind === 1
            ? "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
            : "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
        )
        if (l) add(p.launcherIdentity, new Set(l.attachments.keys()))
      }
      const authoritativeCamera=tf2Camera(snapshot,this.#yaw,this.#pitch)
      const presentedEye=this.#predictedEye.sample(publication.interpolation)
      const camera=presentedEye?Object.freeze({...authoritativeCamera,position:presentedEye}):authoritativeCamera
      if(!ownsGeneration())return
      const viewport=this.#viewport(),view={aspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),farPlane:camera.far}
      const historicalViewmodels: ModelPoseRequest[] = []
      const mapViewmodel = (value: Snapshot) => {
        if (this.#viewmodelClass !== undefined && this.#viewmodelClass !== value.class) {
          this.#viewmodels = createViewmodelPresenter(this.#artifacts!)
        }
        this.#viewmodelClass = value.class
        return value.weapon === null || (value.class === 2 && value.weapon === 12 && (value.conditions[0] & 2) !== 0)
          ? undefined : this.#viewmodels!.map(value, view)
      }
      for (const batch of publication.eventBatches) {
        if (batch.snapshot.tick >= snapshot.tick || !batch.snapshot.projectileEvents.some((event) => event.type === "fire" && event.ownerIdentity === 1)) continue
        const fire = batch.snapshot.projectileEvents.find((event) => event.type === "fire" && event.ownerIdentity === 1)!
        if (!fire.launcherPose) throw new Error(`TF2 fire-tick launcher pose unavailable: ${fire.tick}:${fire.projectile}`)
        const historical = mapViewmodel(batch.snapshot)
        if (!historical) throw new Error(`TF2 fire-tick weapon unavailable: ${fire.tick}:${fire.projectile}`)
        historicalViewmodels.push(Object.freeze({
          ...historical.request,
          sampleTick: fire.tick,
          attachmentsOnly: true,
          fireView: fire.launcherPose,
        }))
      }
      const viewmodel=mapViewmodel(snapshot)
      const currentFire=publication.eventBatches.at(-1)?.snapshot.projectileEvents.find((event)=>event.type==="fire"&&event.ownerIdentity===1)
      if(currentFire&&(!currentFire.launcherPose||!viewmodel))throw new Error(`TF2 fire-tick launcher pose unavailable: ${currentFire.tick}:${currentFire.projectile}`)
      const currentViewmodelRequest=viewmodel===undefined?undefined:Object.freeze({...viewmodel.request,sampleTick:currentFire?.tick??snapshot.tick,...(currentFire?{fireView:currentFire.launcherPose!}:{})})
      let watchRequest: ModelPoseRequest | undefined
      if (snapshot.class !== 8) {
        this.#watchActivity = undefined
      } else {
        const cloaked = (snapshot.conditions[0] & (1 << 4)) !== 0
        if (cloaked && (this.#watchActivity === undefined || this.#watchActivity === "ACT_VM_HOLSTER")) {
          this.#watchActivity = "ACT_VM_DRAW"
          this.#watchActivityTick = snapshot.tick
        } else if (!cloaked && this.#watchActivity !== undefined && this.#watchActivity !== "ACT_VM_HOLSTER") {
          this.#watchActivity = "ACT_VM_HOLSTER"
          this.#watchActivityTick = snapshot.tick
        }
        if (this.#watchActivity !== undefined) {
          const watchModel = "models/weapons/v_models/v_watch_spy.mdl"
          const watch = this.#artifacts.models.get(watchModel)
          const sequence = watch?.sequences.find(value => value.activity === this.#watchActivity)
          if (!watch || !sequence || watch.descriptor.kind !== "viewmodel") throw new Error("Authored Spy offhand watch viewmodel is unavailable")
          const elapsed = Number(snapshot.tick - this.#watchActivityTick) * SIMULATION_SAMPLE_INTERVAL_SECONDS
          if (elapsed >= sequence.durationSeconds) {
            if (this.#watchActivity === "ACT_VM_HOLSTER") this.#watchActivity = undefined
            else if (this.#watchActivity === "ACT_VM_DRAW") {
              this.#watchActivity = "ACT_VM_IDLE"
              this.#watchActivityTick = snapshot.tick
            }
          }
          if (this.#watchActivity !== undefined) {
            const activityElapsed = Number(snapshot.tick - this.#watchActivityTick) * SIMULATION_SAMPLE_INTERVAL_SECONDS
            watchRequest = Object.freeze({
              identity: 0x7fff_ff00 + snapshot.class * 4 + 2,
              sampleTick: snapshot.tick,
              model: watchModel,
              activity: this.#watchActivity,
              previousElapsedSeconds: Math.max(0, activityElapsed - publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS),
              elapsedSeconds: activityElapsed,
              currentTimeSeconds: Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS,
              frameTimeSeconds: publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS,
              planarSpeed: Math.hypot(snapshot.velocity[0], snapshot.velocity[1]),
              screenAspectRatio: view.aspectRatio,
              worldFarPlane: view.farPlane,
              skin: snapshot.team === 2 ? 0 : 1,
              lod: 0,
              bodygroups: Object.freeze(watch.bodygroupCounts.map(() => 0)),
            })
          }
        }
      }
      const lockerRequests=[...this.#lockerAnimations].flatMap(([identity,state])=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===identity),artifact=occurrence&&this.#artifacts!.models.get(occurrence.model);if(!occurrence||!artifact){this.#blockers.add(`TF2 regenerate model presentation unavailable: ${identity}`);return []}const closed=snapshot.tick>=state.closeTick,animation=closed?state.closeAnimation:state.openAnimation,start=closed?state.closeTick:state.openTick,elapsed=Math.max(0,Number(snapshot.tick-start)*0.015),previousTick=snapshot.tick>BigInt(publication.selectedTicks)?snapshot.tick-BigInt(publication.selectedTicks):0n,previousElapsed=Math.max(0,Number(previousTick-start)*0.015);return [Object.freeze({identity,model:occurrence.model,activity:animation,previousElapsedSeconds:Math.min(previousElapsed,elapsed),elapsedSeconds:elapsed,currentTimeSeconds:Number(snapshot.tick)*0.015,frameTimeSeconds:publication.selectedTicks*0.015,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:occurrence.skin,lod:0,bodygroups:Object.freeze([]),packedBody:state.body})]})
      const livingBots=snapshot.bots.filter(bot=>bot.lifecycle===1)
      const botRequests=livingBots.map(bot=>{
        const model=tf2ClassPresentation(bot.class).model
        const artifact=this.#artifacts!.models.get(model)
        if(!artifact)throw new Error(`Authored TF2 bot player model unavailable: ${model}`)
        const weapon=bot.weapon?.identity
        const role=weapon===6||weapon===8||weapon===11||weapon===14||weapon===42||bot.class===8?"MELEE":weapon===5||weapon===7||weapon===10||weapon===13||weapon===41||bot.class===4?"SECONDARY":"PRIMARY"
        const moving=Math.hypot(bot.velocity[0],bot.velocity[1])>1
        const activity=`ACT_MP_${moving?"RUN":"STAND"}_${role}`
        if(!artifact.sequences.some(sequence=>sequence.activity===activity))throw new Error(`Authored TF2 bot player activity unavailable: ${model}:${activity}`)
        const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS
        return Object.freeze({identity:BOT_MODEL_IDENTITY_BASE+bot.identity,model,activity,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:Math.hypot(bot.velocity[0],bot.velocity[1]),screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:bot.team===2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0))})
      })
      const objectiveRequests=(snapshot.objectives?.flags??[]).flatMap(flag=>{
        if(flag.disabled&&!flag.visibleWhenDisabled||flag.carrier===1)return []
        const artifact=this.#artifacts!.models.get(flag.model)
        if(!artifact)throw new Error(`Authored TF2 intelligence model unavailable: ${flag.model}`)
        const activity=flag.status===1?"idle":"spin"
        if(!artifact.sequences.some(sequence=>sequence.label.toLowerCase()===activity))throw new Error(`Authored TF2 intelligence sequence unavailable: ${flag.model}:${activity}`)
        const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS
        return [Object.freeze({identity:OBJECTIVE_MODEL_IDENTITY_BASE+flag.identity,model:flag.model,activity,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:flag.skin,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0))})]
      })
      const buildingRequests=snapshot.buildings.map(building=>{
        const model=buildingModel(building),artifact=this.#artifacts!.models.get(model)
        if(!artifact)throw new Error(`Authored TF2 building model unavailable: ${model}`)
        const desired=building.phase===0?"ACT_OBJ_ASSEMBLING":building.phase===2?"ACT_OBJ_UPGRADING":"ACT_OBJ_RUNNING"
        const selected=artifact.sequences.find(sequence=>sequence.activity===desired)??artifact.sequences.find(sequence=>sequence.activity==="ACT_OBJ_IDLE")??artifact.sequences[0]
        if(!selected)throw new Error(`Authored TF2 building activity unavailable: ${model}:${desired}`)
        const elapsed=Math.max(0,Number(snapshot.tick-building.startedTick)*SIMULATION_SAMPLE_INTERVAL_SECONDS)
        return Object.freeze({identity:building.identity,model,activity:selected.activity||selected.label,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:building.team===2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0))})
      })
      const placementRequest=snapshot.placement?(()=>{const placement=snapshot.placement!,model=blueprintModel(placement.object),artifact=this.#artifacts!.models.get(model),sequence=artifact?.sequences[0];if(!artifact||!sequence)throw new Error(`Authored TF2 building blueprint unavailable: ${model}`);const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS;return Object.freeze({identity:BUILDING_BLUEPRINT_IDENTITY,model,activity:sequence.activity||sequence.label,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:snapshot.team===2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0))})})():undefined
      const modelStart=performance.now(),modelRequests=[...historicalViewmodels,...(currentViewmodelRequest?[currentViewmodelRequest]:[]),...(watchRequest?[watchRequest]:[]),...lockerRequests,...botRequests,...objectiveRequests,...buildingRequests,...(placementRequest?[placementRequest]:[])]
      const modelRequest=modelRequests.length===0?undefined:(this.#wasmCalls.models++,client.models(generation,encodeModelPoseBatch(modelRequests)))
      this.#wasmCalls.visibility++;const visibilityRequest=client.visibility(generation,{
        position:camera.position,
        yawDegrees:camera.yawDegrees,
        pitchDegrees:camera.pitchDegrees,
        verticalFovDegrees:camera.verticalFovDegrees,
        aspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),
        near:camera.near,
        far:camera.far,
        presentationTimeSeconds:Number(snapshot.tick)*0.015,
      });void visibilityRequest.catch(()=>{});const modelOutput=modelRequest===undefined?undefined:await modelRequest
      if(!ownsGeneration())return
      const modelPoses=modelOutput===undefined?[]:decodeModelPoseOutput(modelOutput)
      const modelMilliseconds=performance.now()-modelStart
      const viewmodelIdentities=new Set([...historicalViewmodels.map(request=>request.identity),...(viewmodel?[viewmodel.item.identity]:[])])
      const timelineViewmodelPoses = modelPoses.filter((pose) => viewmodelIdentities.has(pose.identity))
      const viewmodelPoses = currentViewmodelRequest===undefined?[]:timelineViewmodelPoses.filter((pose) => pose.identity===currentViewmodelRequest.identity&&!pose.attachmentsOnly&&pose.sampleTick===currentViewmodelRequest.sampleTick)
      const lockerPoses=modelPoses.filter(pose=>this.#lockerAnimations.has(pose.identity))
      const watchPose = watchRequest && modelPoses.find(pose => pose.identity === watchRequest!.identity)
      if (watchRequest && (!watchPose || watchPose.role !== "hand" || !watchPose.viewmodel)) throw new Error("Authored Spy offhand watch pose differs")
      const botPoses=modelPoses.filter(pose=>pose.identity>=BOT_MODEL_IDENTITY_BASE&&pose.identity<BOT_MODEL_IDENTITY_BASE+0x10000)
      const objectivePoses=modelPoses.filter(pose=>pose.identity>=OBJECTIVE_MODEL_IDENTITY_BASE&&pose.identity<OBJECTIVE_MODEL_IDENTITY_BASE+0x10000)
      if(objectivePoses.length!==objectiveRequests.length)throw new Error("TF2 intelligence pose output differs from authoritative objective state")
      const buildingPoses=modelPoses.filter(pose=>snapshot.buildings.some(building=>building.identity===pose.identity))
      const blueprintPose=modelPoses.find(pose=>pose.identity===BUILDING_BLUEPRINT_IDENTITY)
      if(buildingPoses.length!==snapshot.buildings.length||Boolean(blueprintPose)!==Boolean(snapshot.placement))throw new Error("TF2 building pose output differs from authoritative object state")
      if(botPoses.length!==livingBots.length)throw new Error("TF2 bot player pose output differs from authoritative living player state")
      if(viewmodel!==undefined&&((snapshot.weapon===11||viewmodel.standalone)?(viewmodelPoses.length!==1||viewmodelPoses[0]?.role!=="hand"):(viewmodelPoses.length!==2||viewmodelPoses.filter(pose=>pose.role==="item").length!==1||viewmodelPoses.filter(pose=>pose.role==="hand").length!==1)))throw new Error(`Viewmodel composition output differs: weapon=${snapshot.weapon}; roles=${viewmodelPoses.map(pose=>pose.role).join(",")}`);const viewmodelPose=viewmodelPoses.find(pose=>pose.role==="hand")
      if(viewmodelPose)this.#viewmodelActivities.add(viewmodelPose.activity)
      this.#updateAttachmentTransforms(snapshot, timelineViewmodelPoses, camera)
      let presentation:ReturnType<ProjectileMapper["map"]>
      const projectileStart=performance.now()
      if(!ownsGeneration())return
      try{presentation=owners.mapper.map(projectileFrame(snapshot))}catch(error){const timeline=snapshot.projectileTimeline.map(value=>`${value.tick}[${value.projectiles.map(projectile=>projectile.identity).join(",")}]{${value.events.map(event=>`${event.type}:${event.projectile}@${event.tick}`).join(",")}}`).join("|"),top=snapshot.projectileEvents.map(event=>`${event.type}:${event.projectile}@${event.tick}`).join(",");throw new Error(`${error instanceof Error?error.message:"projectile presentation failed"}; top ${top}; timeline ${timeline}`)}
      const projectileMilliseconds=performance.now()-projectileStart
      this.#pendingProjectileTimeline.splice(0,projectileTimeline.length)
      const visibility=await visibilityRequest
      if(!ownsGeneration())return
      const particleStart=performance.now()
      const pyroParticles=snapshot.class===7||this.#pyroFlameEffect?this.#pyroParticles(snapshot):[]
      const needsCriticalAttachment=publication.eventBatches.some(batch=>batch.snapshot.events.some(event=>event.kind===17&&event.auxiliary===1&&event.values[2]===1))
      const playerAttachmentTransforms=needsCriticalAttachment?new Map(botPoses.map(pose=>{
        const bot=snapshot.bots.find(value=>BOT_MODEL_IDENTITY_BASE+value.identity===pose.identity)!
        return [bot.identity,new Map(pose.attachments.map(attachment=>[attachment.name.toLowerCase(),transformAttachment(attachment.matrix,bot.position,sourceViewOrientation(0,bot.yawDegrees))]))] as const
      })):undefined
      const combatParticles=publication.eventBatches.flatMap(batch=>{
        const muzzles=snapshot.class===1||snapshot.class===3||snapshot.class===6||snapshot.class===9||snapshot.class===8
          ?hitscanMuzzleParticles(batch.snapshot,{systems:PARTICLE_SYSTEMS,attachmentTransforms:this.#attachmentTransforms}):[]
        const result=combatImpactParticles(batch.snapshot,{tracerCount:this.#combatTracerCount},{systems:PARTICLE_SYSTEMS,attachmentTransforms:this.#attachmentTransforms,playerAttachmentTransforms})
        this.#combatTracerCount=result.state.tracerCount
        return [...muzzles,...result.particles]
      })
      const supplementalParticles=[...combatParticles,...pyroParticles]
      const combinedParticles=supplementalParticles.length===0?presentation.particles:[...presentation.particles,...supplementalParticles].sort((left,right)=>left.tick<right.tick?-1:left.tick>right.tick?1:0)
      const particleBatch=owners.encoder.encode(snapshot.tick,camera.position,combinedParticles)
      if(!ownsGeneration())return
      this.#wasmCalls.particles++
      const particleOutput=await client.particles(generation,particleBatch)
      if(!ownsGeneration())return
      const particleMilliseconds=performance.now()-particleStart
      const particleDecodeStart=performance.now(),particleItems=decodeParticleRenderOutput(particleOutput,this.#artifacts.particleMaterials).items,particleDecodeMilliseconds=performance.now()-particleDecodeStart
      const audioStart=performance.now();this.#playAudio(snapshot, camera);const audioMilliseconds=performance.now()-audioStart
      if(this.#paused||!ownsGeneration())return
      const frame=Object.freeze({
        effects: Object.freeze([]),
        particles: particleItems,
        combatDecals:snapshot.combatDecals,
        maximumCombatDecals:Number(this.#settings?.snapshot().settings.current.mp_decals??200),
        models: Object.freeze([
          ...projectileModels(presentation.models),
          ...lockerPoses.map(pose=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===pose.identity)!;return Object.freeze({identity:pose.identity,model:pose.model,position:occurrence.origin,angles:occurrence.angles,scale:1,skin:occurrence.skin,pose})}),
          ...botPoses.map(pose=>{const bot=snapshot.bots.find(value=>BOT_MODEL_IDENTITY_BASE+value.identity===pose.identity);if(!bot)throw new Error("TF2 bot player pose identity is unavailable");return Object.freeze({identity:pose.identity,model:pose.model,position:bot.position,angles:Object.freeze([0,bot.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:bot.team===2?0:1,pose})}),
          ...objectivePoses.map(pose=>{const flag=snapshot.objectives?.flags.find(value=>OBJECTIVE_MODEL_IDENTITY_BASE+value.identity===pose.identity);if(!flag)throw new Error("TF2 intelligence pose identity is unavailable");const carrier=flag.carrier===null?undefined:snapshot.bots.find(bot=>bot.identity===flag.carrier);if(carrier){const carrierPose=botPoses.find(value=>value.identity===BOT_MODEL_IDENTITY_BASE+carrier.identity);const attachment=carrierPose?.attachments.find(value=>value.name.toLowerCase()==="flag");if(!attachment)throw new Error(`Authored TF2 flag attachment unavailable: ${carrier.identity}`);const transform=transformAttachment(attachment.matrix,carrier.position,sourceViewOrientation(0,carrier.yawDegrees));return Object.freeze({identity:pose.identity,model:pose.model,position:transform.position,orientation:transform.orientation,scale:1,skin:flag.skin,pose})}return Object.freeze({identity:pose.identity,model:pose.model,position:flag.position,angles:flag.angles,scale:1,skin:flag.skin,pose})}),
          ...buildingPoses.map(pose=>{const building=snapshot.buildings.find(value=>value.identity===pose.identity);if(!building)throw new Error("TF2 building pose identity is unavailable");return Object.freeze({identity:pose.identity,model:pose.model,position:building.position,angles:Object.freeze([0,building.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:building.team===2?0:1,pose})}),
          ...(blueprintPose&&snapshot.placement?[Object.freeze({identity:blueprintPose.identity,model:blueprintPose.model,position:snapshot.placement.position,angles:Object.freeze([0,snapshot.placement.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:snapshot.team===2?0:1,pose:blueprintPose})]:[]),
          ...viewmodelPoses.map((pose, index) => Object.freeze({
            ...viewmodel!.item,
            identity: viewmodel!.item.identity + index,
            model: pose.model,
            skin: viewmodel!.item.skin < (this.#artifacts!.models.get(pose.model)?.skinCount ?? 0) ? viewmodel!.item.skin : 0,
            position:pose.viewmodel!.transform.origin,angles:pose.viewmodel!.transform.angles,
            viewModelProjection:Object.freeze({kind:"viewmodel" as const,horizontalFov4By3:pose.viewmodel!.projection.unscaledHorizontalFov4By3,near:pose.viewmodel!.projection.near,depthRange:pose.viewmodel!.depthRange,drawsAfterWorld:true,opaqueBeforeTranslucent:true,optionalViewSpaceYReflection:pose.viewmodel!.reflected}),
            pose,
          })),
          ...(watchPose ? [Object.freeze({
            identity: watchPose.identity,
            model: watchPose.model,
            position: watchPose.viewmodel!.transform.origin,
            angles: watchPose.viewmodel!.transform.angles,
            scale: 1,
            skin: snapshot.team === 3 && (this.#artifacts!.models.get(watchPose.model)?.skinCount ?? 0) > 1 ? 1 : 0,
            viewModel: true,
            viewModelProjection: Object.freeze({ kind: "viewmodel" as const, horizontalFov4By3: watchPose.viewmodel!.projection.unscaledHorizontalFov4By3, near: watchPose.viewmodel!.projection.near, depthRange: watchPose.viewmodel!.depthRange, drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: false }),
            pose: watchPose,
          })] : []),
        ]),
        brushModels: snapshot.entityPresentation,
        modelVisibility: new Map(snapshot.pickups.map((pickup) => [pickup.identity, pickup.available])),
        collisionWorldIdentity: snapshot.collisionSnapshot.worldIdentity,
      }) satisfies Omit<Frame,"camera"|"visibility"|"deltaSeconds">
      this.#combatDecalCount+=snapshot.combatDecals.length
      this.#preparedRevision+=1
      const prepared=Object.freeze({
        generation,
        revision:this.#preparedRevision,
        snapshot,
        publication,
        visibility,
        visibilityPosition:camera.position,
        visibilityYaw:camera.yawDegrees,
        visibilityPitch:camera.pitchDegrees,
        frame,
        modelMilliseconds,
        projectileMilliseconds,
        particleMilliseconds,
        particleDecodeMilliseconds,
        audioMilliseconds,
        particleOutputBytes:particleOutput.byteLength,
      })
      this.#requiredParticleDisplayFrames.admit(prepared, particleItems.map(item=>item.effectIdentity))
      this.#preparedPresentation=prepared
      if(snapshot.class===9&&!this.#engineer){if(!this.#uiResources||!this.#presentationRandom)throw new Error("TF2 Engineer presentation resources are unavailable");this.#engineer=initializeTf2EngineerPresentation({root:this.#engineerRoot,resources:this.#uiResources,viewport:this.#viewport(),clock:{nowSeconds:()=>this.#frameClock.current},random:this.#presentationRandom,reducedMotion:matchMedia("(prefers-reduced-motion: reduce)").matches})}
      this.#engineer?.publish(snapshot)
      const hud = this.#hudIntegration?.publish(publication, this.#currentHudContext(snapshot))
      const hudPlayer = hud?.facts.player.kind === "available" ? hud.facts.player.value : null
      const hudHealth = hudPlayer?.health.kind === "available" ? hudPlayer.health.value.current : "unavailable"
      const hudWeaponIdentity = hudPlayer?.activeWeapon.kind === "available" ? hudPlayer.activeWeapon.value : null
      const hudWeapon = hudPlayer?.weapons.find((weapon) => weapon.identity === hudWeaponIdentity)
      const hudProbe = this.#hudIntegration?.probe()
      const hudPanel = (name: string) => hudProbe?.panels.find((panel) => panel.name === name)
      const healthPanel = hudPanel("PlayerStatusHealthImage")
      const ammoPanel = hudPanel("HudWeaponAmmo")
      const weaponPanel = hudPanel("modelpanel0")
      this.#set({
        hudProbe: hudPlayer ? `${hudHealth}:${hudPlayer.class.kind === "available" ? hudPlayer.class.value : "unavailable"}:${hudWeaponIdentity ?? "unavailable"}:${hudWeapon?.clip.kind === "available" ? hudWeapon.clip.value : "unavailable"}:${hudWeapon?.reserve.kind === "available" ? hudWeapon.reserve.value : "unavailable"}` : "unavailable",
        hudAnimationTrace: hudProbe?.animationTrace.join("|"),
        hudOperationProbe: healthPanel && ammoPanel && weaponPanel
          ? `${healthPanel.state.imageFill}:${healthPanel.bounds.x},${healthPanel.bounds.y},${healthPanel.bounds.width},${healthPanel.bounds.height}:${healthPanel.state.drawColor.join(",")}:${healthPanel.state.foregroundColor?.join(",") ?? "none"}:${ammoPanel.state.scalarProperties.reloadPhase ?? "none"}:${weaponPanel.state.scalarProperties.weaponIdentity ?? "none"}`
          : "unavailable",
        hudPresentationProbe: hudProbe && hud ? this.#hudPresentationObservation(hudProbe, hud) : "unavailable",
        scoreboardVisible: this.#scoreboardVisible,
        scoreboardProbe: hud?.scoreboard.kind === "available" ? JSON.stringify({
          map: hud.scoreboard.value.mapName,
          red: hud.scoreboard.value.red,
          blue: hud.scoreboard.value.blue,
          players: hud.scoreboard.value.players.map((player) => ({
            identity: player.identity, name: player.name, team: player.team,
            class: player.class.kind === "available" ? player.class.value : null,
            score: player.score, alive: player.alive,
            ping: player.ping.kind === "available" ? player.ping.value : null,
            kills: player.counters.kind === "available" ? player.counters.value.kills : null,
            deaths: player.counters.kind === "available" ? player.counters.value.deaths : null,
          })),
          spectators: hud.scoreboard.value.spectators,
        }) : "unavailable",
        fireEvents: this.#fireEvents,
        explosionEvents: this.#explosionEvents,
        objectiveProbe: snapshot.objectives ? `${snapshot.objectives.redCaptures}:${snapshot.objectives.blueCaptures}:${snapshot.objectives.captureLimit}:${snapshot.objectives.winner??0}:${snapshot.objectives.flags.map(flag=>`${flag.identity},${flag.team},${flag.status},${flag.carrier??0},${flag.returnDeadline??-1}`).join("|")}` : undefined,
        objectiveEventProbe: snapshot.objectives?.events.map(event=>`${event.kind}:${event.detail}:${event.team}:${event.subject}:${event.player??0}`).join("|"),
        roundProbe: `${snapshot.round.state}:${Number(snapshot.round.waitingForPlayers)}:${Number(snapshot.round.inSetup)}:${Number(snapshot.round.inOvertime)}:${snapshot.round.winningTeam??0}:${snapshot.round.redScore}:${snapshot.round.blueScore}:${snapshot.round.timer?.remaining.toFixed(2)??"none"}`,
        buildingCount:snapshot.buildings.length,
        engineerMetal:snapshot.metal,
        engineerMenu:this.#engineer?.menu()??"none",
        placementProbe:snapshot.placement?`${snapshot.placement.object.kind}:${snapshot.placement.object.mode}:${Number(snapshot.placement.valid)}:${snapshot.placement.position.join(",")}:${snapshot.placement.yawDegrees}`:"",
        buildingProbe:snapshot.buildings.map(building=>`${building.identity}:${building.object.kind}:${building.object.mode}:${building.phase}:${building.level}:${building.health.toFixed(1)}/${building.maximumHealth}:${building.upgradeMetal}:${building.shells}/${building.maximumShells}:${building.target??"none"}`).join("|"),
        botCount: snapshot.bots.length,
        pickupCount: snapshot.pickups.length,
        pickupProbe: snapshot.pickups.map(pickup=>`${pickup.identity}:${pickup.kind}:${pickup.size}:${pickup.available?1:0}:${pickup.respawnTick??"none"}:${pickup.origin.join(",")}`).join("|"),
        metal: snapshot.metal,
        botProbe: snapshot.bots.map(bot=>`${bot.identity}:${bot.team}:${bot.class}:${bot.objective}:${bot.area??"none"}:${bot.remainingPathAreas}:${bot.position.map(value=>value.toFixed(1)).join(",")}:${bot.target??"none"}:${bot.weapon?.identity??"none"}:${bot.weapon?.clip??0}:${bot.shots}:${bot.hits}:${bot.kills}:${bot.deaths}`).join("|"),
        particleRenderItems: particleItems.length,
        combatDecals:this.#combatDecalCount,
        flamePoints: snapshot.flamePoints.length,
        movement: snapshot.movement,
        playerFlags: snapshot.playerFlags,
        inWater: snapshot.inWater,
        movementTick: snapshot.movementTick,
        spyWatchActivity: watchPose?.activity,
        viewmodelPose: viewmodelPose===undefined?undefined:Object.freeze({
          activity: viewmodelPose.activity,
          sequence: viewmodelPose.sequence,
          cycle: viewmodelPose.cycle,
          primitives: viewmodelPoses.reduce((total, pose) => total + pose.primitives.length, 0),
          events: viewmodelPose.events.length,
        }),
        audioVoices: this.#audio?.activeVoices() ?? Object.freeze([]),
        snapshotTick: snapshot.tick.toString(),
        projectileStates: snapshot.projectiles.map((projectile) => `${projectile.identity}:${projectile.state}`).join(","),
        particleProbe: [...new Set(particleItems.map((item) => `${item.primitive}:${item.material}:${item.primarySheet ? "sheet" : "missing"}`))].sort().join("|"),
        audioStarts: this.#view.audioStarts?.length === this.#audioStarts.length
          ? this.#view.audioStarts
          : Object.freeze([...this.#audioStarts]),
        viewmodelProjection: viewmodel?.item.viewModelProjection ? `${viewmodel.item.viewModelProjection.horizontalFov4By3}:${viewmodel.item.viewModelProjection.near}:${viewmodel.item.viewModelProjection.depthRange.join(",")}` : undefined,
        viewmodelActivities: this.#view.viewmodelActivities?.length === this.#viewmodelActivities.size
          ? this.#view.viewmodelActivities
          : Object.freeze([...this.#viewmodelActivities]),
        viewmodelSequences: this.#viewmodelSequences(this.#artifacts, snapshot.class),
        crouchHistory: Object.freeze([...this.#crouchHistory]),
        viewmodelTimelineProbes: this.#view.viewmodelTimelineProbes,
        ...this.#gameplayTraces(snapshot),
        ...this.#snapshotProbes(snapshot),
        simulationProbe: `${publication.hostFrame}:${publication.firstHostTick}-${publication.lastHostTick}:${publication.selectedTicks}:${publication.snapshotBytes.byteLength}:${publication.eventBatches.reduce((n,e)=>n+e.bytes.byteLength,0)}`,
        brushModelProbe: `${snapshot.entityPresentation.entityRevision}:${snapshot.entityPresentation.collisionRevision}:${snapshot.entityPresentation.models.length}:${snapshot.entityPresentation.models.filter(model=>model.draw).length}`,
        reloadHistory:Object.freeze([...this.#reloadHistory]),
        fireTickHistory: this.#view.fireTickHistory?.length === this.#fireTickHistory.length
          && this.#view.fireTickHistory.at(-1) === this.#fireTickHistory.at(-1)
          ? this.#view.fireTickHistory
          : Object.freeze([...this.#fireTickHistory]),
        lockerProbe:this.#lockerProbe(snapshot.tick),
      })
      if (this.#pendingClassSelectionTeam === snapshot.team) {
        this.#pendingClassSelectionTeam = undefined
        this.#showClassSelection()
      }
      this.#offerDisplay()
    } catch (error) {
      if(this.#closed||generation!==this.#generation||this.#view.phase==="Replacing")return
      this.#paused = true
      this.#predictedEye.suspend()
      this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "Gameplay frame failed" })
    }
  }

  #installListeners(): void {
    if (this.#listenersInstalled) return
    this.#listenersInstalled = true
    window.addEventListener("keydown", this.#keyDown, true)
    window.addEventListener("keyup", this.#keyUp, true)
    window.addEventListener("mousedown", this.#mouseDown)
    window.addEventListener("mouseup", this.#mouseUp, true)
    window.addEventListener("mousemove", this.#mouseMove)
    window.addEventListener("blur", this.#blur)
    document.addEventListener("visibilitychange", this.#visibility)
    document.addEventListener("pointerlockchange", this.#pointerLock)
    document.addEventListener("pointerlockerror", this.#pointerLockError)
  }

  #removeListeners(): void {
    if (!this.#listenersInstalled) return
    this.#listenersInstalled = false
    window.removeEventListener("keydown", this.#keyDown, true)
    window.removeEventListener("keyup", this.#keyUp, true)
    window.removeEventListener("mousedown", this.#mouseDown)
    window.removeEventListener("mouseup", this.#mouseUp, true)
    window.removeEventListener("mousemove", this.#mouseMove)
    window.removeEventListener("blur", this.#blur)
    document.removeEventListener("visibilitychange", this.#visibility)
    document.removeEventListener("pointerlockchange", this.#pointerLock)
    document.removeEventListener("pointerlockerror", this.#pointerLockError)
  }

  #sourceKey(code: string): string {
    if (code.startsWith("Key")) return code.slice(3).toLowerCase()
    if (code.startsWith("Digit")) return code.slice(5)
    if (code === "Backquote") return "`"
    if (code === "Comma") return ","
    if (code === "Period") return "."
    if (code === "Space") return "SPACE"
    if (code === "Tab") return "TAB"
    if (code === "ControlLeft") return "CTRL"
    if (code === "ControlRight") return "RCTRL"
    if (code === "ShiftLeft") return "SHIFT"
    if (code === "ShiftRight") return "RSHIFT"
    if (code === "AltLeft") return "ALT"
    if (code === "AltRight") return "RALT"
    if (code.startsWith("Arrow")) return `${code.slice(5).toUpperCase()}ARROW`
    return code.toUpperCase()
  }

  #boundAction(code: string, modifiers: number): string | null {
    const resolved = this.#bindings.resolve(code, modifiers)
    return resolved?.match === "unmodified" && resolved.action === "toggleconsole" ? null : resolved?.action ?? null
  }

  #keyboardAction(event: KeyboardEvent): string | null {
    const code = this.#sourceKey(event.code)
    let modifiers = Number(event.shiftKey) | (Number(event.ctrlKey) << 1) | (Number(event.altKey) << 2)
    if (code === "SHIFT" || code === "RSHIFT") modifiers &= ~1
    if (code === "CTRL" || code === "RCTRL") modifiers &= ~2
    if (code === "ALT" || code === "RALT") modifiers &= ~4
    return this.#boundAction(code, modifiers) ?? (code === "RSHIFT" ? this.#boundAction("SHIFT", modifiers) : null)
  }

  #mouseAction(event: MouseEvent): string | null {

    const code = sourceMouseButtonCode(event.button)

    const modifiers = Number(event.shiftKey) | (Number(event.ctrlKey) << 1) | (Number(event.altKey) << 2)
    return code === null ? null : this.#boundAction(code, modifiers)
  }

  #setScoreboardVisible(visible: boolean): void {
    if (this.#scoreboardVisible === visible) return
    this.#scoreboardVisible = visible
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    this.#hudIntegration?.setScoreboardVisibility(visible)
    this.#set({ scoreboardVisible: visible })
  }

  #activateBoundAction(identity: string, action: string): void {
    if (action === "+forward" || action === "+back" || action === "+moveleft" || action === "+moveright" || action === "+duck") {
      this.#buttons.press(identity, action)
    } else if (action === "+jump") {
      if (this.#buttons.press(identity, action)) this.#jumpPressed = true
    } else if (action === "+attack") {
      if (this.#buttons.press(identity, action)) this.#firePressed = true
    } else if (action === "+attack2") {
      if(this.#snapshot?.placement){this.#buildingRequest={action:"rotate"};return}
      if (this.#buttons.press(identity, action)) this.#detonatePressed = true
    } else if (action === "+reload") {
      if (this.#buttons.press(identity, action)) this.#reloadPressed = true
    } else if (action === "+showscores") {
      if (this.#buttons.press(identity, action)) this.#setScoreboardVisible(true)

    } else if (action === "slot1") this.#selectWeapon = this.#snapshot?.class === 8 ? 50 : this.#snapshot?.class === 1 ? 4 : this.#snapshot?.class === 2 ? 12 : this.#snapshot?.class === 4 ? 18 : this.#snapshot?.class === 6 ? 9 : this.#snapshot?.class === 9 ? 40 : this.#snapshot?.class === 7 ? 15 : 1
    else if (action === "slot2") this.#selectWeapon = this.#snapshot?.class === 8 ? 52 : this.#snapshot?.class === 1 ? 5 : this.#snapshot?.class === 2 ? 13 : this.#snapshot?.class === 3 ? 7 : this.#snapshot?.class === 6 ? 10 : this.#snapshot?.class === 9 ? 41 : this.#snapshot?.class === 7 ? 7 : 3
    else if (action === "slot3") this.#selectWeapon = this.#snapshot?.class === 8 ? 51 : this.#snapshot?.class === 1 ? 6 : this.#snapshot?.class === 2 ? 14 : this.#snapshot?.class === 3 ? 8 : this.#snapshot?.class === 4 ? 17 : this.#snapshot?.class === 6 ? 11 : this.#snapshot?.class === 9 ? 42 : this.#snapshot?.class === 7 ? 16 : undefined
    else if (action === "slot4" && this.#snapshot?.class === 8) this.#selectWeapon = 53
    else if(action==="slot4"&&this.#snapshot?.class===9)this.#selectWeapon=43
    else if(action==="slot5"&&this.#snapshot?.class===9)this.#selectWeapon=44

  }

  readonly #keyDown = (event: KeyboardEvent): void => {
    if (this.#localMatch?.handleKey(event)) return
    if (!this.#view.consoleVisible && this.#classSelection?.handleKey(event, this.#keyboardAction(event) === "changeclass")) return
    if (!this.#view.consoleVisible && this.#teamSelection?.handleKey(event, this.#keyboardAction(event) === "changeteam")) return
    if (event.code === "Escape" && this.#view.optionsVisible && this.#options?.handleKey(event)) return
    if (event.code === "Escape" && !this.#view.consoleVisible) {
      const route = routeApplicationEscape({
        code: event.code,
        repeat: event.repeat,
        phase: this.#view.phase,
        gameUi: this.#view.gameUi,
        optionsVisible: this.#view.optionsVisible === true,
      })
      if (route !== "ignore") {
        event.preventDefault()
        event.stopImmediatePropagation()
        if (route === "options" && this.#options) {
          this.#options.hide("cancel")
          this.#set({ optionsVisible: false })
        } else if (route === "activate" && this.#gameUi?.state().kind === "in-game") {
          this.#neutral()
          this.#paused = true
          this.#predictedEye.suspend()
          if (document.pointerLockElement) void document.exitPointerLock()
          const transition = this.#gameUi.dispatch({ kind: "escape" })
          if (transition.disposition === "applied") this.#set({ gameUi: "pause", detail: "Game paused" })
        } else if (route === "resume" && this.#gameUi?.state().kind === "pause") {
          this.#resumePointerOnAcknowledge = true
          const transition = this.#gameUi.dispatch({ kind: "escape" })
          if (transition.disposition !== "applied") this.#resumePointerOnAcknowledge = false
        }
        return
      }
    }
    if (this.#options?.handleKey(event)) return
    if (event.code === "Backquote") {
      if (!this.#consoleEnabled || this.#keyboardAction(event) !== "toggleconsole"
        || (this.#vguiRoot.contains(event.target as Node) && !this.#view.consoleVisible)
        || this.#optionsRoot.contains(event.target as Node)) return
      event.preventDefault()
      this.toggleConsole()
      return
    }
    if (this.#view.consoleVisible || this.#view.optionsVisible || this.#view.localMatchVisible || this.#teamSelection?.state().visible
      || this.#view.gameUi !== "in-game" || event.repeat) return
    if (this.#snapshot?.class === 8 && this.#snapshot.weapon === 53 && /^Digit[1-9]$/u.test(event.code)) {
      const classes: readonly Tf2Class[] = [1, 3, 7, 4, 6, 9, 5, 2, 8]
      const selected = classes[Number(event.code.slice(5)) - 1]!
      this.#disguise = Object.freeze({ class: selected, team: this.#snapshot.team === 2 ? 3 : 2 })
      this.#selectWeapon = 50
      event.preventDefault()
      return
    }
    const action = this.#keyboardAction(event)
    if (!action) return
    if (action === "+showscores") event.preventDefault()
    if (action === "changeteam") {
      event.preventDefault()
      void this.#showTeamSelection()
      return
    }
    if(this.#engineer?.menu()&&/^slot[1-4]$/.test(action)){
      event.preventDefault()
      const request=this.#engineer.select(Number(action.slice(4)))
      if(request){this.#buildingRequest=request;if(request.action==="destroy")this.#selectWeapon=42}
      return
    }
    if (action === "changeclass") {
      event.preventDefault()
      this.#showClassSelection()
      return
    }
    void this.resumeAudio()
    this.#activateBoundAction(`keyboard:${event.code}`, action)
  }

  readonly #keyUp = (event: KeyboardEvent): void => {
    const showingScores = this.#buttons.held("+showscores")
    this.#buttons.release(`keyboard:${event.code}`)
    if (showingScores && !this.#buttons.held("+showscores")) {
      event.preventDefault()
      this.#setScoreboardVisible(false)
    }
  }

  readonly #mouseDown = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    void this.resumeAudio()
    const action = this.#mouseAction(event)
    if (action) this.#activateBoundAction(`mouse:${event.button}`, action)
  }

  readonly #mouseUp = (event: MouseEvent): void => {
    const showingScores = this.#buttons.held("+showscores")
    this.#buttons.release(`mouse:${event.button}`)
    if (showingScores && !this.#buttons.held("+showscores")) this.#setScoreboardVisible(false)
  }

  readonly #mouseMove = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    if(event.movementX===0&&event.movementY===0)return
    const scale = this.#mouseSensitivity / 3
    const movementX=event.movementX*scale
    const angles = applyPointerDelta(this.#yaw, this.#pitch, movementX, event.movementY * scale * (this.#reverseMouse ? -1 : 1))
    this.#yaw = angles.yaw
    this.#pitch = angles.pitch
    this.#pointerMovementX+=movementX
    this.#viewRevision+=1
    this.#mouseViewRevision+=1
  }

  readonly #blur = (): void => this.#neutral()
  readonly #visibility = (): void => {
    this.#paused = document.hidden || this.#view.gameUi === "pause"
    this.#predictedEye.suspend()
    this.#neutral()
    const nowSeconds=this.#frameClock.admit(performance.now()/1_000)
    this.#nextSimulationSampleSeconds=nowSeconds+SIMULATION_SAMPLE_INTERVAL_SECONDS
    if(this.#client&&this.#snapshot)this.#scheduleSimulation(nowSeconds,this.#paused)
  }
  readonly #pointerLock = (): void => {
    const locked=document.pointerLockElement===this.#canvas
    if (!locked){this.#neutral();this.#pointerMovement=undefined}
    this.#set({ pointerLocked: locked,pointerMovement:locked?this.#pointerMovement:undefined })
  }
  readonly #pointerLockError=():void=>{
    if(this.#pointerRequestPending)return
    if(document.pointerLockElement===this.#canvas)return
    this.#pointerMovement=undefined
    this.#neutral()
    this.#set({pointerLocked:false,pointerMovement:undefined,detail:this.#lastPointerLockFailure})
  }

  #neutral(): void {
    this.#buttons.clear()
    this.#setScoreboardVisible(false)
    this.#jumpPressed = this.#firePressed = this.#detonatePressed = this.#reloadPressed = false
    this.#selectClass = undefined
    this.#selectWeapon = undefined
    this.#disguise = undefined
    this.#modeRequest = undefined
  }

  #publishProfileCoverage():void{
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    if(!profile)return
    profile.coverageSamples=this.#coverageSamples
    if(this.#artifacts){
      profile.materialAnimation=Object.freeze({
        generation:this.#generation,
        target:this.#mapIdentity,
        volumes:this.#artifacts.environment.waterVolumeFacts,
        surfaces:this.#artifacts.environment.waterSurfaceFacts,
        skyController:this.#skyController(this.#artifacts),
        materials:Object.freeze([...this.#artifacts.environment.worldMaterials.values()].map(material=>Object.freeze({
          identity:material.identity,
          mapMaterial:material.mapMaterial,
          shader:material.shader,
          proxies:material.proxies,
          environmentMap:material.environmentMap,
          textures:Object.freeze(material.textures.map(texture=>Object.freeze({
            ...texture,
            frameCount:texture.logicalPath?this.#artifacts!.environment.authoredTextures.get(texture.logicalPath.toLowerCase())?.frameCount??null:null,
            mipCount:texture.logicalPath?this.#artifacts!.environment.authoredTextures.get(texture.logicalPath.toLowerCase())?.mipCount??null:null,
          }))),
        }))),
      })
    }
  }

  #publishProfileDisplacements(scene: Awaited<ReturnType<Renderer["loadMap"]>>):void{
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    const sources=profile?.displacementSources
    if(!profile||!Array.isArray(sources)||sources.length<1||sources.length>16||sources.some(source=>!Number.isSafeInteger(source)))return
    const selected=new Set<number>(sources as number[])
    profile.displacements=scene.displacements.filter(displacement=>selected.has(displacement.source)).map(displacement=>Object.freeze({
      ...displacement,
      positions:Array.from(displacement.positions),normals:Array.from(displacement.normals),indices:Array.from(displacement.indices),
    }))
  }

  selectClass(value: Tf2Class): void {
    this.#selectClass = value
  }

  async requestPointer(canvas = this.#canvas): Promise<void> {
    const connected = canvas.isConnected && canvas.getRootNode() === document
      ? canvas
      : document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
    if (!connected) {
      this.#set({ detail: "Pointer lock target is not attached to the active document" })
      return
    }
    this.#canvas = connected
    if (this.#closed || this.#view.consoleVisible || this.#classSelection?.state().visible || this.#teamSelection?.state().visible) return
    const audioAdmission=this.resumeAudio()
    const request=async(raw:boolean):Promise<"raw"|"adjusted">=>{
      const admission=this.#canvas.requestPointerLock(raw?{unadjustedMovement:true}:undefined)
      if(admission&&typeof admission.then==="function"){
        await admission
        return raw?"raw":"adjusted"
      }
      return "adjusted"
    }
    try {
      this.#pointerRequestPending=true
      try{
        try{
          this.#pointerMovement=await request(true)
        }catch(error){
          if(!rawPointerMovementUnsupported(error))throw error
          this.#pointerMovement=await request(false)
        }
      }finally{this.#pointerRequestPending=false}
      if(document.pointerLockElement===this.#canvas)this.#set({pointerLocked:true,pointerMovement:this.#pointerMovement})
    } catch (error) {
      this.#pointerMovement=undefined
      this.#lastPointerLockFailure = error instanceof Error ? `Pointer lock failed: ${error.name}:${error.message}:activation=${navigator.userActivation.isActive}` : "Pointer lock failed"
      this.#set({ detail: this.#lastPointerLockFailure })
    }
    await audioAdmission
  }

  async resumeAudio(): Promise<void> {
    if (!this.#audio || this.#closed) return
    if (this.#audioRunning && this.#audioContext?.state === "running") return
    try {
      await this.#audio.resume()
      this.#audioRunning = true
      this.#set({ detail: "Audio running" })
    } catch (error) {
      this.#blockers.add(`AudioUnavailable: ${error instanceof Error ? error.message : "resume failed"}`)
      this.#set({ detail: "Audio permission unavailable" })
    }
  }

  toggleConsole(): void {
    if (!this.#console) return
    if (this.#view.consoleVisible) {
      this.#console.apply({ kind: "hide" })
      this.#set({ consoleVisible: false })
      return
    }
    this.#neutral()
    if (document.pointerLockElement) void document.exitPointerLock()
    this.#console.apply({ kind: "activate" })
    this.#console.apply({ kind: "foreground" })
    this.#console.apply({ kind: "focus-entry" })
    this.#set({ consoleVisible: true })
  }

  async close(): Promise<void> {
    if (this.#closed) return
    await this.#release()
    this.#set({ phase: "Closed", detail: "Application closed", pointerLocked: false, pointerMovement:undefined, consoleVisible: false })
  }

  async #teardownGameplay(): Promise<void> {
    this.#paused = true
    this.#neutral()
    this.#generation += 1
    this.#simulationSamples.clear()
    await this.#simulationTask
    await this.#presentationTask
    this.#resetGenerationPresentation()
    this.#viewmodelSequenceCache.clear()
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    await this.#displayTask
    await this.#client?.shutdown().catch(() => {})
    this.#client = undefined
    this.#cache?.close()
    this.#cache = undefined
    this.#projectiles?.dispose()
    this.#projectiles = undefined
    await this.#renderer?.dispose().catch(() => {})
    this.#renderer = undefined
    await this.#audio?.close().catch(() => {})
    this.#audio = undefined
    this.#audioContext = undefined
    this.#audioRegistry = undefined
    this.#audioWorld = undefined
    this.#audioBuffers.clear()
    this.#audioRunning = false
    this.#hudIntegration?.reset("disconnect")
    this.#classSelection?.dispatch({ kind: "hide" })
    await this.#classSelectionRenderTask
    const admission = this.#teamAdmission
    this.#teamAdmission = undefined
    admission?.reject(new Error("TF2 team selection was cancelled by map replacement"))
    this.#teamSelection?.dispatch({ kind: "hide" })
    await this.#teamSelectionRenderTask
    this.#loaded = undefined
    this.#snapshot = undefined
    this.#artifacts = undefined
    this.#viewmodels = undefined
    this.#viewmodelClass = undefined
    this.#attachments.clear()
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
    this.#botRequest = undefined
    this.#botConfiguration = undefined
    if (this.#activeBotConfiguration?.offlinePractice) {
      this.#activeBotConfiguration = undefined
      this.#botDifficulty = 1
      this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
    }
    if (document.pointerLockElement === this.#canvas) {
      try { await document.exitPointerLock() } catch {}
    }
  }

  async #release(): Promise<void> {
    if (this.#closed) return
    this.#closed = true
    this.#viewportOwner.destroy()
    this.#operations.cancel()
    if (this.#operationWatchdog) clearTimeout(this.#operationWatchdog)
    this.#operationWatchdog = undefined
    this.#removeStartupListeners()
    this.#startup?.destroy()
    this.#startup = undefined
    this.#paused = true
    for (const handle of this.#gameUiRequestTasks) clearTimeout(handle)
    this.#gameUiRequestTasks.clear()
    this.#simulationSamples.clear()
    cancelAnimationFrame(this.#animationFrame)
    this.#removeListeners()
    this.#neutral()
    this.#bindings.clear()
    this.#bindingValues.clear()
    await this.#displayTask
    if (document.pointerLockElement === this.#canvas) {
      try {
        await document.exitPointerLock()
      } catch {}
    }
    await this.#teardownGameplay()
    this.#destroyMenuPresentation()
  }
}
