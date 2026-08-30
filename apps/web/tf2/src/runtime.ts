import { createImmutableObjectAcquirer, openDerivedObjectCache, type DerivedObjectCache, type ImmutableObjectPriority } from "@playsrc/asset-store/browser"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import { chunksForRole, partitionResourceChunkDescriptors, parseResourceCatalogBytes, parseResourceGraphBytes, parseResourceSet, resourceChunkObject, resourceSectionIdentity, selectCatalogTarget, type ResourceCatalog, type ResourceGraph, type ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { combatPoseSelection } from "./combat-pose-selection"
import {mapRendererInputs} from "./map-renderer-inputs"
import { createSourceAudioSystem, SoundRegistry, SourceAudioError, SourceAudioWorld, type PcmResource } from "@playsrc/audio"
import { tf2AudioModuleUrl } from "@playsrc/game-tf2-browser/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { encodeLegacyVisualQuery,decodeLegacyVisualViews,type LegacyVisualView } from "@playsrc/game-tf2-browser/legacy-visuals"
import { botAdmissionProfile, recordBotAdmission } from "./bot-admission-profile"
import { APPLICATION_BUILD as __PLAYSRC_APPLICATION_BUILD__, WASM_SHA256 as __PLAYSRC_WASM_SHA256__, RESOURCE_ROOTS as __PLAYSRC_RESOURCE_ROOTS__ } from "virtual:playsrc-generation"
import { TF2_PRESENTATION_SCHEMA, Tf2WorkerClient, Tf2WorkerError, mergePublicationSnapshots, type CoverageSample, type LoadedGame, type ResourceConfiguration, type SimulationPublication, type VisibilityResult } from "@playsrc/game-tf2-browser"
import { Tf2EquipmentProfile, Tf2EquipmentPresentation, equippedWeaponSlots, equipmentPipelinePoseRequests, type Tf2EquipmentPreview } from "@playsrc/game-tf2-browser/equipment"
import { selectionTransitionMark, selectionTransitionDraw } from "./selection-transition-profile"
import { teamModelPlayback, type TeamModelPlayback } from "./team-model-playback"
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
import { createClientRenderFrameClock, RecordedClientRenderFrames } from "@playsrc/game-tf2-browser/client-render-frame"
import { TF2_CLASS_NAMES, tf2ClassFromName, tf2ClassPresentation } from "@playsrc/game-tf2-browser/class"
import { parsePresentationArtifacts, parseEquipmentModelArtifacts, type PresentationArtifacts, type EquipmentModelArtifacts } from "@playsrc/game-tf2-browser/artifacts"
import {
  createParticleBatchEncoder,
  encodeLegacyParticleFrame,
  createProjectilePresentationMapper,
  createViewmodelPresenter,
  createMeleeConditionPresenter,
  classPipelinePoseRequests,
  mapPropPipelinePoseRequests,
  classPreviewBaseActivity,
  encodeModelPoseBatch,
  projectileFrame,
  projectileModelPath,
  weaponParticleColorRequests,
  projectileModels,
  hitscanMuzzleParticles,
  combatImpactParticles,
  sourceViewOrientation,
  tf2Audio,
  tf2Camera,
  studioModelFrameState,
  transformAttachment,
  type ModelPoseRequest,
  type PosedModel,
  type ProjectileParticleRequest,
  type Tf2AudioRequest,
} from "@playsrc/game-tf2-browser/presentation"
import { decodeParticleRenderOutput } from "@playsrc/particle"
import { browserFrameProfiler, createRenderer, SOURCE_LDR, SOURCE_PC_INTEGER_HDR, type Camera, type Frame, type MaterialStateInput, type ModelPanelPass } from "@playsrc/rendering"
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
import { loadBrowserConfiguration, parseBrowserConfiguration, tf2SelectableMapNames, type BrowserConfiguration, type BrowserTargetConfiguration } from "./config"
import { createApplicationGenerationRecovery, installedMapProfileIdentity, resourceGenerationMatches } from "./application-generation"
import { playStartupVideo } from "./startup-playback"
import { loadStartupMetadata, startupMetadataFacts, validateStartupMetadata } from "./startup-media"
import type { Tf2TargetName } from "@playsrc/game-tf2-browser/maps"
import { browserOwnsKey } from "@playsrc/vgui"
import { PhysicalBindingIndex, PhysicalButtonState, applyPointerDelta, pointerLockRequestRequired, rawPointerMovementUnsupported, sourceMouseButtonCode, type PhysicalBinding } from "./input"
import { TF2_BALANCED_VIDEO_SETTINGS, TF2_SELECTED_OPTIONS, tf2VideoConfiguration, tf2VideoConvars, tf2VideoSettingsFromConvars, type AdapterRequestResult, type SettingsAdapterRequest, type Tf2VideoConfiguration } from "@playsrc/settings"
import { SimulationClockQueue } from "./simulation-clock"
import {
  initializeBrowserPresentationViewportOwner,
  type ApplicationPresentationViewport,
  type PresentationViewportOwner,
} from "./presentation-viewport"
import { RequiredParticleDisplayQueue } from "./particle-display"
import { DisplayBackpressure } from "./display-backpressure"
import { mapResidency } from "./map-residency"
import { CanvasFrameDiagnostics } from "./frame-diagnostics"
import { tokenizeSourceCommand } from "./console-command"
import { GAME_UI_FRAME_OWNER, HUD_FRAME_OWNER, LOADING_FRAME_OWNER, OPTIONS_FRAME_OWNER, visibleFrameOwners } from "./frame-owners"
import { currentPresentedCamera, equivalentPresentedVisibility, presentCamera, selectPresentedCamera, type PresentedCamera } from "./presented-camera"
import {
  ApplicationFrameClock,
  ApplicationOperationLedger,
  ApplicationVisibilityWatchdog,
  PredictedEyeInterpolation,
  admitBotConfiguration,
  composeViewmodelTransform,
  currentPresentationGeneration,
  routeApplicationEscape,
  selectAuthoredSky,
  type ApplicationOperation,
} from "./application-lifecycle"

const applicationGenerationRecovery = createApplicationGenerationRecovery({
  currentBuild: __PLAYSRC_APPLICATION_BUILD__,
  storage: sessionStorage,
  visible: () => document.visibilityState === "visible",
  whenVisible: () => new Promise<void>((resolve) => {
    const changed = () => {
      if (document.visibilityState !== "visible") return
      document.removeEventListener("visibilitychange", changed)
      resolve()
    }
    document.addEventListener("visibilitychange", changed)
    changed()
  }),
  reload: () => window.location.reload(),
})

const MAX_EXTERNAL_BYTES = 536_870_912
const SIMULATION_SAMPLE_INTERVAL_SECONDS = 0.015
const MAX_REQUIRED_PARTICLE_DISPLAY_FRAMES = 256
const BOT_MODEL_IDENTITY_BASE = 0x6000_0000
const OBJECTIVE_MODEL_IDENTITY_BASE = 0x6100_0000
const BUILDING_BLUEPRINT_IDENTITY = 0x5fff_ffff

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
  classSelectionAnimation?: string
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
  waterOverlay?: string
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
  medigunCharge?: number
  medigunTarget?: number | null
  medigunReleasing?: boolean
  unsupportedState?: "RigidResourcesUnavailable"
  startupState?: Tf2StartupState["kind"]
  loadingProgress?: number
  loadingStatus?: string
  loadingBackground?: "map-photo" | "configured-generic"
  startupGestures?: number
  menuPreparation?: string
  bootstrapLoading?: boolean
  bootstrapProgress?: number
  bootFailure?: boolean
}>

type Renderer = Awaited<ReturnType<typeof createRenderer>>
type Audio = Awaited<ReturnType<typeof createSourceAudioSystem>>
type ProjectileMapper = ReturnType<typeof createProjectilePresentationMapper>
type LockerAnimationState=Readonly<{openTick:bigint;closeTick:bigint;body:number;openAnimation:"open"|"close";closeAnimation:"open"|"close"}>
type PreparedPresentation=Readonly<{
  generation:number
  revision:number
  viewportRevision:number
  snapshot:Snapshot
  publication:SimulationPublication
  visibility:VisibilityResult
  skyVisibility?:VisibilityResult
  presentedCamera:PresentedCamera
  frame:Omit<Frame,"camera"|"visibility"|"deltaSeconds">
  modelMilliseconds:number
  projectileMilliseconds:number
  particleMilliseconds:number
  particleDecodeMilliseconds:number
  audioMilliseconds:number
  particleOutputBytes:number
  hudModelPoses: readonly PosedModel[]
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
  #dependencies?: ResourceConfiguration
  #dependencyEntries = new Map<string, Uint8Array>()
  #cache?: DerivedObjectCache
  #openingCache?: Promise<DerivedObjectCache>
  #client?: Tf2WorkerClient
  #equipmentProfile?: Tf2EquipmentProfile
  #equipment?: Tf2EquipmentPresentation
  #equipmentRoot?: HTMLElement
  #equipmentPreview: Tf2EquipmentPreview | null = null
  #equipmentRenderTask?: Promise<void>
  #equipmentPreviewReset = true
  #equipmentPreviewElapsed = 0
  #equipmentPreviewStarted = 0
  #equipmentAdmissionTask?: Promise<void>
  #equipmentPreparing = false
  #equipmentAdmissionEpoch = 0
  #equipmentPanelArtifacts?: EquipmentModelArtifacts
  readonly #equipmentAdmissions = new Map<number, { definitions: Set<number>; sources: Set<number> }>()
  #resourceRuntime?: Promise<void>
  readonly #immutableObjects = createImmutableObjectAcquirer({
    concurrency: 8,
    cache: () => this.#openObjectCache(),
    onCacheEvent: (event) => {
      const profile = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
      if (!profile) return
      const statistics = (profile.immutableCache ??= { hits: 0, verifiedAtWriteHits: 0, rehashedHits: 0, hashMilliseconds: 0, misses: 0, corruptions: 0, writes: 0, hitBytes: 0, writtenBytes: 0, readMilliseconds: 0, writeMilliseconds: 0 }) as Record<string, number>
      if (event.kind === "hit") {
        statistics.hits! += 1
        statistics.hitBytes! += event.byteLength
        statistics.readMilliseconds! += event.milliseconds
        if (event.verification === "verified-at-write") statistics.verifiedAtWriteHits! += 1
        else if (event.verification === "rehash") statistics.rehashedHits! += 1
        statistics.hashMilliseconds! += event.hashMilliseconds ?? 0
      }
      else if (event.kind === "miss") statistics.misses! += 1
      else if (event.kind === "corrupt") statistics.corruptions! += 1
      else { statistics.writes! += 1; statistics.writtenBytes! += event.byteLength; statistics.writeMilliseconds! += event.milliseconds }
    },
  })
  readonly #operationProgressBytes = new Map<string, number>()
  #lastWatchdogProgress = 0
  #renderer?: Renderer
  #audio?: Audio
  #audioContext?: AudioContext
  #audioRegistry?: SoundRegistry
  #audioWorld?: SourceAudioWorld
  #audioBuffers: ReadonlyMap<string, PcmResource> = new Map()
  #audioStarts: string[] = []
  #pendingAudioRequests: Tf2AudioRequest[] = []
  #lockerAnimations = new Map<number, LockerAnimationState>()
  #reloadHistory:string[]=[]
  #fireTickHistory:string[]=[]
  #wasmCalls={observe:0,models:0,visibility:0,particles:0,acoustics:0}
  #maximumScheduledSamples=0
  #maximumPublicationTicks=0
  #phaseTimings=[0,0,0,0,0]
  #cosmeticHudKey = ""
  #cosmeticHudStarted = 0n
  #pendingProjectileTimeline:Snapshot["projectileTimeline"][number][]=[]
  #audioRunning = false
  #artifacts?: PresentationArtifacts
  #mapArtifacts?: PresentationArtifacts
  #projectiles?: ProjectileMapper
  #viewmodels?: ReturnType<typeof createViewmodelPresenter>
  #meleeConditions = createMeleeConditionPresenter()
  #viewmodelClass?: Snapshot["class"]
  #watchActivity?: "ACT_VM_DRAW" | "ACT_VM_IDLE" | "ACT_VM_HOLSTER"
  #watchActivityTick = 0n
  #watchOwner?: { generation: number; team: number; respawnTick: bigint }
  #attachments = new Map<number, ReadonlySet<string>>()
  #attachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #fireAttachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #particleBatches = createParticleBatchEncoder()
  #clientRenderFrames = createClientRenderFrameClock()
  #recordedClientFrames: RecordedClientRenderFrames | undefined
  #recordedPresentations: { inputs: NonNullable<import("@playsrc/game-tf2-browser/command-workload").CommandWorkload["presentations"]>; cursor: number; buffered: SimulationPublication[] } | undefined
  #particleSystems = new Set<string>()
  #pyroFlameEffect?: string
  #manmelterChargeEffect?: string
  #combatTracerCount = 0
  #combatDecalCount = 0
  #pyroEffectSerial = 0
  #console?: DeveloperConsole
  #pendingConsoleOutput: Readonly<{ text: string; developer: boolean }>[] = []
  readonly #operationWatchdog = new ApplicationVisibilityWatchdog({
    now: () => performance.now(),
    schedule: (callback, milliseconds) => setTimeout(callback, milliseconds),
    cancel: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
  })
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
  #bootstrapExpectedObjects = new Set<string>()
  #bootstrapObjectProgress = new Map<string, Readonly<{ loaded: number; total: number }>>()
  readonly #gameUiRequestTasks = new Set<number>()
  #hudIntegration?: Tf2HudIntegration
  #engineer?: Tf2EngineerPresentation
  #classSelection?: Tf2ClassSelectionIntegration
  #classSelectionModelPanels: readonly Tf2ClassSelectionModelPanel[] = Object.freeze([])
  #classSelectionAnimationStarted = 0
  #classSelectionAnimationElapsed = 0
  #classSelectionAnimationReset = true
  #classSelectionWeaponVisible = true
  #selectionViewportEpoch = 0
  #classSelectionBackground: HTMLCanvasElement | undefined
  #classSelectionBackgroundKey = ""
  #classSelectionBackgroundPrimitives = 0
  #classSelectionRenderTask?: Promise<void>
  #classSelectionRenderRevision = 0
  #teamSelection?: Tf2TeamSelectionIntegration
  #teamSelectionModelPanels: readonly Tf2TeamSelectionModelPanel[] = Object.freeze([])
  #teamSelectionRenderTask?: Promise<void>
  #preparingModelPipelines = false
  #teamSelectionRenderRevision = 0
  #pendingClassSelectionTeam?: 2 | 3
  readonly #teamSelectionPoses = new Map<string, PosedModel>()
  readonly #teamSelectionAnimations = new Map<string, TeamModelPlayback>()
  #teamSelectionUpdateTask: Promise<void> | undefined
  #teamSelectionUpdateTime = 0
  #teamAdmission?: Readonly<{ generation: number; resolve(): void; reject(error: Error): void }>
  #hudRootCounts?: Readonly<{ playerStatus: number; ammo: number }>
  #hudContext?: SessionHudContext
  #hudContextIdentity = -1
  #publishedScoreboard?: Tf2HudScoreboard
  #publishedScoreboardProbe = "unavailable"
  #scoreboardVisible = false
  #scoreboardPingAsText = false
  #playerClassUsePlayerModel = true
  #deathNoticeTime = 6
  #crosshairSettings?: Tf2CrosshairSettings
  #settings?: Tf2BrowserSettings
  #weaponPreferences = Object.freeze({ rememberActive: false, rememberLast: false })
  #options?: Tf2OptionsPresentation
  #localMatch?: Tf2LocalMatchPresentation
  #pendingLocalMatch?: Tf2LocalMatchLaunch
  #loaded?: LoadedGame
  #snapshot?: Snapshot
  #medicBeamTarget: number | null = null
  #medicBeamReleasing = false
  #nextBotStop = false
  #generation = 0
  #reservedGeneration = 0
  #catalogReplacement?: Promise<void>
  #yaw = 0
  #pitch = 0
  #appliedViewAngleOffset: readonly [number, number, number] = [0, 0, 0]
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
  #selectWeapon: Tf2Weapon | "last" | undefined
  #disguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | undefined
  #modeRequest: 0 | 1 | undefined
  #botRequest: BotRequest | undefined
  #botControl: NonNullable<Parameters<typeof encodeCommand>[0]["botControl"]> | undefined
  #buildingRequest: Tf2BuildingRequest | undefined
  #botConfiguration: BotConfiguration | undefined
  #activeBotConfiguration: BotConfiguration | undefined
  #botDifficulty: 0 | 1 | 2 | 3 = 1
  #objectiveConfiguration: Readonly<{ capturesPerRound: number; returnOnTouch: boolean }> | undefined
  #flagCapturesPerRound = 3
  #flagReturnOnTouch = false
  #coverageSamples:readonly CoverageSample[]=Object.freeze([])
  #developer = 1
  #showFps: ClientDiagnosticMode = 0
  #showPos: ClientDiagnosticMode = 0
  #renderLevel: 0 | 1 | 2 = 0
  #videoConfiguration: Tf2VideoConfiguration = tf2VideoConfiguration(TF2_BALANCED_VIDEO_SETTINGS)
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
  readonly #canvasDiagnostics = new CanvasFrameDiagnostics()
  #displayFrame=0
  #displayTask?:Promise<void>
  readonly #displayBackpressure=new DisplayBackpressure()
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
    document.addEventListener("visibilitychange", this.#operationVisibility)
    this.#publishOperationWatchdog()
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
    if (patch.phase === "Ready" && prior.phase !== "Ready") {
      applicationGenerationRecovery.complete()
      const generation = (globalThis as typeof globalThis & { __playsrcProfile?: { applicationGeneration?: Record<string, unknown> } }).__playsrcProfile?.applicationGeneration
      if (generation) {
        generation.readyMilliseconds = performance.now() - Number(generation.startedMilliseconds)
        generation.mapGeneration = this.#generation
        generation.staleMessages = this.#client?.staleMessages ?? 0
      }
    }
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
      this.#operationWatchdog.cancel()
      if (["Startup", "Loading", "Replacing"].includes(this.#view.phase)) {
        this.#output(`STATUS: ${this.#view.phase}: ${this.#view.detail}`)
        this.#armOperationWatchdog()
      }
      this.#publishOperationWatchdog()
    }
    if (enteredFailure) {
      console.error(`TF2 application failed: ${this.#view.detail}`)
      this.#output(`FATAL: ${this.#view.detail}`)
      if (this.#console && !this.#view.consoleVisible) this.toggleConsole()
    }
  }

  #failureDetail(error: unknown, fallback: string): string {
    const message = error instanceof Error ? error.message : fallback
    const profile = (globalThis as any).__playsrcProfile
    if (profile) {
      profile.failure = { message, stack: error instanceof Error ? error.stack : null }
      console.error("TF2 profiled failure", profile.failure)
    }
    return message
  }

  #armOperationWatchdog(): void {
    this.#operationWatchdog.cancel()
    const phase = this.#view.phase
    const detail = this.#view.detail
    this.#lastWatchdogProgress = performance.now()
    this.#operationWatchdog.arm(phase, detail, 60_000, !document.hidden, () => {
      if (this.#view.phase === phase && this.#view.detail === detail) {
        this.#set({ phase: "Failed", gameUi: "failure", detail: `${phase} made no progress for 60 seconds: ${detail}` })
      }
    })
    this.#publishOperationWatchdog()
  }

  #acquireObject(descriptor: ObjectDescriptor, signal: AbortSignal, priority: ImmutableObjectPriority = "normal"): Promise<Uint8Array> {
    if (!this.#configuration) throw new Error("Browser configuration is unavailable")
    return this.#immutableObjects(this.#configuration.assetOrigin, descriptor, {
      signal,
      priority,
      onProgress: (loaded, total) => this.#trackBootstrapObject(descriptor.sha256, loaded, total),
    })
  }

  #openObjectCache(): Promise<DerivedObjectCache> {
    if (this.#cache) return Promise.resolve(this.#cache)
    if (!this.#openingCache) {
      this.#openingCache = openDerivedObjectCache().then((cache) => {
        if (this.#closed) {
          cache.close()
          throw new DOMException("Application object cache was closed", "AbortError")
        }
        this.#cache = cache
        return cache
      }).catch((error) => {
        this.#openingCache = undefined
        throw error
      })
    }
    return this.#openingCache
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
    this.#equipmentAdmissionEpoch += 1
    if (this.#equipment?.visible()) this.#equipment.hide()
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
      configuredMaps: this.#selectableMaps(),
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
          void this.#preparedTarget(launch.mapIdentity)
            .then((target) => this.#switchCatalogMap(target))
            .catch((error) => this.#set({
              phase: "Failed",
              gameUi: "failure",
              detail: error instanceof Error ? error.message : "Local match map preparation failed",
            }))
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
    if (request.kind === "show-equipment") { await this.#showEquipment(); return }
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
      if (!this.#paused) void this.resumeAudio()
      this.#set({ gameUi: "in-game", detail: `Playing ${this.#mapIdentity}` })
      if (restorePointer && !this.#paused) void this.requestPointer()
      return
    }
    if (request.kind === "disconnect") {
      this.#nextOperation()
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
      const target = await this.#preparedTarget(request.mapIdentity)
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
      if (request.changes.some((change) => !(change.settingId in TF2_BALANCED_VIDEO_SETTINGS))) return reject("browser renderer owner does not implement every requested effect")
      const previousConfiguration = this.#videoConfiguration
      const previousLevel = this.#renderLevel
      let configuration: Tf2VideoConfiguration
      try {
        configuration = tf2VideoConfiguration({
          ...(this.#settings?.snapshot().settings.current ?? TF2_BALANCED_VIDEO_SETTINGS),
          ...Object.fromEntries(request.changes.map((change) => [change.settingId, change.nextValue])),
        })
      } catch (error) { return reject(error instanceof Error ? error.message : "renderer configuration is invalid") }
      this.#videoConfiguration = configuration
      this.#renderLevel = configuration.hdrLevel
      if (this.#client && this.#renderer && this.#loaded) {
        try { await this.#replaceCatalogMap() }
        catch (error) {
          this.#videoConfiguration = previousConfiguration
          this.#renderLevel = previousLevel
          return reject(error instanceof Error ? error.message : "renderer replacement failed")
        }
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
        && (!["cl_hud_playerclass_use_playermodel", "tf_scoreboard_ping_as_text", "tf_remember_activeweapon", "tf_remember_lastswitched"].includes(change.settingId)
          || typeof change.nextValue !== "boolean"))) {
        return reject(`browser game owner does not implement every requested effect: ${request.changes.map((change) => `${change.settingId}=${String(change.nextValue)}`).join(",")}`)
      }
      const model = request.changes.find((change) => change.settingId === "cl_hud_playerclass_use_playermodel")
      if (request.changes.some(change => change.settingId === "tf_remember_activeweapon" || change.settingId === "tf_remember_lastswitched")) {
        this.#replaceWeaponPreferences({ ...this.#settings!.snapshot().settings.current,
          ...Object.fromEntries(request.changes.map(change => [change.settingId, change.nextValue])) })
      }
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

  #replaceWeaponPreferences(settings: Readonly<Record<string, unknown>>): void {
    this.#weaponPreferences = Object.freeze({ rememberActive: settings.tf_remember_activeweapon === true, rememberLast: settings.tf_remember_lastswitched === true })
  }

  #nextOperation(): ApplicationOperation {
    this.#operation = this.#operations.begin()
    this.#operationProgressBytes.clear()
    return this.#operation
  }

  #requireOperation(operation: ApplicationOperation): void {
    if (this.#closed || !this.#operations.current(operation)) {
      throw new DOMException("Application operation was superseded", "AbortError")
    }
  }

  #reserveGeneration(): number {
    const next = Math.max(this.#generation, this.#reservedGeneration) + 1
    if (next > 0xffff_ffff) throw new Error("Application generation bound exceeded")
    return this.#reservedGeneration = next
  }

  #trackBootstrapObject(sha256: string, loaded: number, total: number): void {
    const previous = this.#operationProgressBytes.get(sha256) ?? 0
    if (loaded > previous) {
      this.#operationProgressBytes.set(sha256, loaded)
      if (["Startup", "Loading", "Replacing"].includes(this.#view.phase)
        && performance.now() - this.#lastWatchdogProgress >= 1_000) {
        this.#armOperationWatchdog()
      }
    }
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
    if (this.#resourceRuntime) {
      await this.#resourceRuntime
      return
    }
    if (this.#client) return
    if (!this.#resourceRuntime) {
      const descriptor = this.#configuration.wasm
      this.#resourceRuntime = (async () => {
        const [cache, wasm] = await Promise.all([
          this.#openObjectCache(),
          this.#acquireObject(descriptor, signal, "critical"),
        ])
        this.#cache = cache
        const candidate = new Tf2WorkerClient(new GameplayWorker(), cache, this.#configuration!.applicationBuild)
        const abort = () => candidate.abort()
        signal.addEventListener("abort", abort, { once: true })
        const handshakeStarted = performance.now()
        try {
          if (signal.aborted) throw new DOMException("Resource runtime was superseded", "AbortError")
          await candidate.initialize(wasm, descriptor.sha256)
          if (signal.aborted || this.#closed) throw new DOMException("Resource runtime was superseded", "AbortError")
          this.#client = candidate
          this.#equipmentProfile = new Tf2EquipmentProfile(candidate, localStorage)
          await this.#equipmentProfile.initialize()
        } catch (error) {
          candidate.abort()
          throw error
        } finally { signal.removeEventListener("abort", abort) }
        const generation = (globalThis as typeof globalThis & { __playsrcProfile?: { applicationGeneration?: Record<string, unknown> } }).__playsrcProfile?.applicationGeneration
        if (generation) {
          generation.worker = this.#configuration!.applicationBuild
          generation.handshakeMilliseconds = performance.now() - handshakeStarted
          generation.staleMessages = this.#client.staleMessages
        }
      })().catch((error) => {
        this.#resourceRuntime = undefined
        throw error
      })
    }
    await this.#resourceRuntime
  }

  async #resourceSet(roles: readonly string[], signal = this.#operation.signal): Promise<ResourceConfiguration | undefined> {
    if (!this.#configuration || !this.#resourceGraph) throw new Error("Resource graph is unavailable")
    return this.#decodeResourceSet(this.#resourceGraph, roles, this.#dependencyEntries, signal)
  }

  async #decodeResourceSet(graph: ResourceGraph, roles: readonly string[], destination: Map<string, Uint8Array>, signal: AbortSignal): Promise<ResourceConfiguration | undefined> {
    if (!this.#configuration) throw new Error("Browser configuration is unavailable")
    await this.#ensureResourceRuntime(signal)
    const chunks = new Map<string, ResourceChunkDescriptor>()
    for (const role of roles) for (const chunk of chunksForRole(graph, role)) chunks.set(chunk.encodedSha256, chunk)
    const groups = partitionResourceChunkDescriptors([...chunks.values()], 32 * 1024 * 1024)
    if (signal.aborted) throw new DOMException("Resource loading was superseded", "AbortError")
    const generation = roles.includes("gameplay") || roles.some(role => role.startsWith("equipment-")) ? this.#reserveGeneration() : undefined
    const graphTarget = generation === undefined ? undefined : this.#configuration.targets.find((target) => target.target === graph.target && target.contentBuild === graph.contentBuild)
    if (generation !== undefined && !graphTarget) throw new Error("Authenticated gameplay resource graph target is unavailable")
    const resourceIdentityKey = graphTarget
      ? bytesToHex(sha256(new TextEncoder().encode(`playsrc-tf2-authenticated-resource-identity-v1\0${graphTarget.objects.resources.sha256}\0${roles.toSorted().join(",")}`)))
      : undefined
    const expectedResourceBytes = 12 + [...chunks.values()].flatMap((chunk) => chunk.entries)
      .reduce((total, entry) => total + 8 + new TextEncoder().encode(entry.logicalPath).byteLength + Number(entry.byteLength), 0)
    const retainedIdentity = resourceIdentityKey ? (async () => {
      try {
        const retained = await this.#cache!.read(resourceIdentityKey)
        if (!retained) return undefined
        const parsed = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(retained.bytes)) as Record<string, unknown>
        if (Object.keys(parsed).sort().join("\0") !== "byteLength\0sha256"
          || parsed.byteLength !== expectedResourceBytes || typeof parsed.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(parsed.sha256)) {
          throw new Error("Authenticated gameplay resource identity differs")
        }
        return Object.freeze({ byteLength: parsed.byteLength as number, sha256: parsed.sha256 })
      } catch {
        await this.#cache!.remove(resourceIdentityKey)
        return undefined
      }
    })() : undefined
    const sections: Uint8Array[] = []
    const sectionIdentities: string[] = []
    const prior = generation === undefined ? undefined : this.#dependencies
    const priorSections = new Map(prior?.sectionIdentities?.map((identity, index) => [identity, index]) ?? [])
    const identities = new Map([...chunks.values()].map((chunk) => [chunk.encodedSha256, resourceSectionIdentity(chunk)]))
    const timeline = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
    const spans = timeline ? (timeline.startupSpans ??= []) as Array<Record<string, unknown>> : undefined
    const span = (kind: string, started: number, extra: Record<string, unknown> = {}): void => {
      spans?.push({ kind, roles: roles.join(","), started, finished: performance.now(), ...extra })
    }
    const acquire = (group: readonly ResourceChunkDescriptor[]) => Promise.allSettled(group.map(async (chunk) => {
      if (priorSections.has(identities.get(chunk.encodedSha256)!)) return Object.freeze({ descriptor: chunk, bytes: undefined })
      const object = resourceChunkObject(chunk)
      const started = performance.now()
      const bytes = await this.#acquireObject(object, signal, roles.includes("startup") ? "critical" : "normal")
      span("chunk-acquire", started, { bytes: bytes.byteLength, identity: chunk.encodedSha256 })
      return Object.freeze({ descriptor: chunk, bytes })
    }))
    let next = acquire(groups[0]!)
    try {
      for (let index = 0; index < groups.length; index += 1) {
        const acquired = await next
        const records = acquired.map((record) => {
          if (record.status === "rejected") throw record.reason
          return record.value
        })
        if (signal.aborted) throw new DOMException("Resource loading was superseded", "AbortError")
        if (index + 1 < groups.length) next = acquire(groups[index + 1]!)
        for (const record of records) {
          let started = performance.now()
          if (signal.aborted) throw new DOMException("Resource loading was superseded", "AbortError")
          const identity = identities.get(record.descriptor.encodedSha256)!
          const sourceIndex = priorSections.get(identity)
          const section = sourceIndex !== undefined
            ? await this.#client!.retainResourceSection(generation!, prior!, sourceIndex)
            : await this.#client!.decodeResources([{ descriptor: record.descriptor, bytes: record.bytes! }], generation)
          if (signal.aborted) throw new DOMException("Resource loading was superseded", "AbortError")
          span(sourceIndex !== undefined ? "resource-retain" : "resource-decode", started, {
            group: index,
            bytes: section.byteLength,
            identity: record.descriptor.encodedSha256,
          })
          sections.push(section)
          sectionIdentities.push(identity)
          started = performance.now()
          for (const [logicalPath, bytes] of parseResourceSet(section)) {
            const existing = destination.get(logicalPath)
            if (existing && (existing.byteLength !== bytes.byteLength || bytesToHex(sha256(existing)) !== bytesToHex(sha256(bytes)))) {
              throw new Error(`Conflicting resource ${logicalPath}`)
            }
            destination.set(logicalPath, bytes)
          }
          span("resource-index", started, { group: index, bytes: section.byteLength })
        }
      }
      if (generation === undefined) return undefined
      const identity = await retainedIdentity
      const started = performance.now()
      const resources = await this.#client!.finalizeResources(generation, sections, identity)
      if (resources.byteLength !== expectedResourceBytes) throw new Error("Authenticated gameplay resource length differs")
      span("resource-finalize", started, { bytes: resources.byteLength, groups: sections.length, cached: Boolean(identity) })
      if (!identity) {
        await this.#cache!.write(resourceIdentityKey!, null, new TextEncoder().encode(JSON.stringify({ byteLength: resources.byteLength, sha256: resources.sha256 })))
      }
      return Object.freeze({ ...resources, sectionIdentities: Object.freeze(sectionIdentities) })
    } catch (error) {
      if (generation !== undefined) await this.#client?.releaseResources(generation).catch(() => {})
      throw error
    }
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
    const url = URL.createObjectURL(new Blob([bytes], { type: "video/webm" }))
    let destroyed = false
    let admitted = false
    const generation = Number(video.dataset.startupMediaGeneration ?? 0) + 1
    video.dataset.startupMediaGeneration = String(generation)
    const chronology: unknown[] = []
    let firstFrame: number | undefined
    const observe = (event: string, frame?: { mediaTime: number; presentedFrames: number }) => {
      if (chronology.length === 16) chronology.shift()
      chronology.push({ event, at: Math.round(performance.now()), currentTime: video.currentTime, paused: video.paused,
        ...(frame ? { frame } : {}),
        ...startupMetadataFacts(video, descriptor.browserRepresentation, generation, video.currentSrc === url) })
      this.#presentationRoot.dataset.startupMediaProbe = JSON.stringify({ build: __PLAYSRC_APPLICATION_BUILD__, sha256: descriptor.browserRepresentation.sha256, chronology })
    }
    const mediaEvents = ["loadedmetadata", "durationchange", "canplay", "playing", "waiting", "stalled", "error", "abort", "emptied"]
    const mediaEvent = (event: Event) => observe(event.type)
    for (const event of mediaEvents) video.addEventListener(event, mediaEvent)
    const removeMediaObservation = () => {
      for (const event of mediaEvents) video.removeEventListener(event, mediaEvent)
      if (firstFrame !== undefined) video.cancelVideoFrameCallback?.(firstFrame)
    }
    const completed = () => events.completed()
    const failed = () => events.failed(video.error ? `MediaError:${video.error.code}` : "Startup media failed")
    video.controls = false
    video.muted = false
    video.loop = false
    video.autoplay = false
    video.playsInline = true
    video.disablePictureInPicture = true
    video.addEventListener("ended", completed)
    video.addEventListener("error", failed)
    try {
      observe("load-request")
      await loadStartupMetadata(video, url, this.#operation.signal)
      validateStartupMetadata(video, descriptor.browserRepresentation, generation, video.currentSrc === url)
      firstFrame = video.requestVideoFrameCallback?.((_now, frame) => observe("first-presented-frame", { mediaTime: frame.mediaTime, presentedFrames: frame.presentedFrames }))
    } catch (error) {
      observe("preparation-failed")
      removeMediaObservation()
      video.removeEventListener("ended", completed)
      video.removeEventListener("error", failed)
      video.removeAttribute("src")
      video.load()
      URL.revokeObjectURL(url)
      throw error
    }
    const startPlayback = async (): Promise<"started"> => {
      observe("gesture-play-request")
      await video.play()
      observe("gesture-play-resolved")
      admitted = true
      return "started"
    }
    return Object.freeze({
      play: async () => {
        observe("play-request")
        let result: "started" | "gesture-required"
        try { result = await playStartupVideo(video) }
        catch (error) { observe("play-rejected"); throw error }
        observe(result === "started" ? "play-resolved" : "gesture-required")
        admitted = result === "started"
        return result
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
        removeMediaObservation()
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
      mapTargets: [...new Set([...this.#selectableMaps(), ...this.#configuration.targets.map((target) => target.target)])],
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
        ...TF2_BALANCED_VIDEO_SETTINGS,
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
    this.#videoConfiguration = tf2VideoConfiguration(this.#settings.snapshot().settings.current)
    this.#renderLevel = this.#videoConfiguration.hdrLevel
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
    this.#dependencyEntries.clear()
    this.#initializeConsole()
    const currentSettings = this.#settings.snapshot().settings.current
    this.#replaceWeaponPreferences(currentSettings)
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
    this.#equipment?.destroy()
    this.#equipment = undefined
    this.#equipmentRoot?.remove()
    this.#equipmentRoot = undefined
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
  readonly #startupVisibility = (): void => this.#startup?.visibility(!document.hidden)
  #installStartupListeners(): void {
    window.addEventListener("keydown", this.#startupKey, true)
    document.addEventListener("visibilitychange", this.#startupVisibility)
  }
  #removeStartupListeners(): void {
    window.removeEventListener("keydown", this.#startupKey, true)
    document.removeEventListener("visibilitychange", this.#startupVisibility)
  }
  #startupState(state: Tf2StartupState): void {
    if (state.kind === "Completed" || state.kind === "Skipped" || state.kind === "Failed" || state.kind === "Destroyed") this.#removeStartupListeners()
    const bootstrapLoading = !["Playing", "AwaitingGesture", "Completed", "Skipped", "Failed", "Destroyed"].includes(state.kind)
    if (state.kind === "Failed") this.#set({ phase: "Failed", gameUi: "failure", bootFailure: true, startupState: state.kind, bootstrapLoading, detail: `${state.stage}: ${state.reason}` })
    else if (state.kind !== "Completed" && state.kind !== "Skipped" && state.kind !== "Destroyed") this.#set({ phase: "Startup", startupState: state.kind, bootstrapLoading, detail: state.kind })
    else this.#set({ startupState: state.kind, bootstrapLoading })
  }

  admitStartupGesture(): void {
    if (this.#startup?.state().kind !== "AwaitingGesture") return
    this.#startupGestures += 1
    this.#set({ startupGestures: this.#startupGestures })
    this.#startup.gesture()
  }

  startupKey(code: string): void {
    if (code === "Escape") this.#startup?.key("Escape")
  }

  async start(): Promise<void> {
    try {
      await this.#viewportOwner.first()
      const configuration = await loadBrowserConfiguration(__PLAYSRC_APPLICATION_BUILD__)
      const profile = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
      if (profile) profile.applicationGeneration = {
        bundle: __PLAYSRC_APPLICATION_BUILD__, configuration: configuration.applicationBuild,
        wasm: configuration.wasm.sha256,
        presentationSchema: TF2_PRESENTATION_SCHEMA, startedMilliseconds: performance.now(),
      }
      const generationMismatch = !resourceGenerationMatches(configuration, __PLAYSRC_WASM_SHA256__, __PLAYSRC_RESOURCE_ROOTS__)
      if (!await applicationGenerationRecovery.ensure(configuration.applicationBuild, generationMismatch)) return
      this.#configuration = configuration
      this.#activeTarget = this.#configuration.targets.find((target) => target.target === this.#configuration!.defaultTarget)
      if (!this.#activeTarget) throw new Error("Default TF2 target is absent")
      this.#presentationRandom = createTf2PresentationRandom(this.#configuration.presentation.randomSeed)
      this.#renderLevel = this.#configuration.renderLevel
      this.#videoConfiguration = tf2VideoConfiguration({ ...TF2_BALANCED_VIDEO_SETTINGS, "video.hdr": this.#renderLevel })
      this.#mapIdentity = this.#activeTarget.target
      this.#set({ phase: "Startup", startupState: "Preparing", detail: "Preparing configured Valve startup movie" })
      const catalogDescriptor = this.#configuration.catalog
      const [catalogBytes, graphBytes] = await Promise.all([
        this.#acquireObject(catalogDescriptor, this.#operation.signal, "critical"),
        this.#acquireObject(this.#activeTarget.objects.resources, this.#operation.signal, "critical"),
        this.#ensureResourceRuntime(this.#operation.signal),
      ])
      const catalog = parseResourceCatalogBytes(catalogBytes)
      if (catalog.application !== "tf2" || catalog.entries.length !== this.#configuration.targets.length) throw new Error("Resource catalog target table differs")
      for (const target of this.#configuration.targets) {
        const entry = selectCatalogTarget(catalog, target.target)
        if (entry.resources.sha256 !== target.objects.resources.sha256 || entry.resources.byteLength !== target.objects.resources.byteLength) throw new Error(`Resource catalog ${target.target} descriptor differs`)
      }
      this.#resourceCatalog = catalog
      const selected = selectCatalogTarget(catalog, this.#activeTarget.target)
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
      if (error instanceof Tf2WorkerError && error.code === "GenerationMismatch" && this.#configuration) {
        try {
          if (!await applicationGenerationRecovery.ensure(this.#configuration.applicationBuild, true)) return
        } catch (failure) { error = failure }
      }
      await this.#release()
      this.#set({ phase: "Failed", gameUi: "failure", bootFailure: true, detail: error instanceof Error ? error.message : "Application startup failed" })
    }
  }

  async #startGameplay(target: BrowserTargetConfiguration): Promise<void> {
    const loadStarted=performance.now()
    let loadPhase=loadStarted
    const loadTimings:Record<string,number>={}
    const loadingControl = (globalThis as typeof globalThis & { __playsrcProfile?: { selectionLoading?: unknown[]; captureSelectionTransitions?: boolean } }).__playsrcProfile
    const loadingRecords = loadingControl?.captureSelectionTransitions ? (loadingControl.selectionLoading ??= []) : undefined
    loadingRecords?.push({ kind: "request", at: loadStarted, epoch: performance.timeOrigin + loadStarted,
      target: target.target, contentBuild: target.contentBuild, objects: target.objects,
      wasm: this.#configuration?.wasm, renderLevel: this.#renderLevel })
    const finishLoadPhase=(name:string):void=>{const now=performance.now();loadTimings[name]=now-loadPhase;
      loadingRecords?.push({ kind: name, started: loadPhase, ended: now });loadPhase=now}
    const operation = this.#nextOperation()
    const signal = operation.signal
    try {
      if (!this.#configuration || !this.#resourceCatalog) throw new Error("Browser configuration is unavailable")
      if (this.#activeTarget?.target !== target.target) {
        const selected = selectCatalogTarget(this.#resourceCatalog, target.target)
        if (selected.resources.sha256 !== target.objects.resources.sha256 || selected.resources.byteLength !== target.objects.resources.byteLength) {
          throw new Error("Target resource root differs from catalog")
        }
        const bytes = await this.#acquireObject(selected.resources, signal, "critical")
        this.#requireOperation(operation)
        const graph = parseResourceGraphBytes(bytes)
        if (graph.target !== target.target || graph.contentBuild !== target.contentBuild) {
          throw new Error("Target resource graph identity differs")
        }
        this.#resourceGraph = graph
        this.#dependencyEntries = new Map()
        this.#activeTarget = target
        this.#mapIdentity = target.target
      }
      this.#set({ detail: "Fetching exact BSP and gameplay WASM objects" })
      this.#advanceLoading("reading-world")
      const [bsp, resources] = await Promise.all([
        this.#acquireObject(target.objects.bsp, signal, "critical"),
        this.#resourceSet(["gameplay"], signal),
      ])
      this.#requireOperation(operation)
      if (!resources) throw new Error("Gameplay resource configuration is unavailable")
      this.#dependencies = resources
      loadingRecords?.push({ kind: "resource-input", sha256: resources.sha256, byteLength: resources.byteLength })
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
        this.#activeTarget.objects.resources.sha256,
      )
      finishLoadPhase("derivedKey")
      this.#set({ detail: "Compiling direct map authority" })
      this.#advanceLoading("preparing-resources")
      this.#generation = resources.generation
      this.#resetGenerationPresentation()
      this.#loaded = await this.#client!.stage(this.#generation, bsp, profile, this.#dependencies, key)
      this.#requireOperation(operation)
      this.#coverageSamples=await this.#client!.coverage(this.#generation)
      this.#requireOperation(operation)
      finishLoadPhase("stage")
      this.#artifacts = await parsePresentationArtifacts(this.#loaded.presentation, this.#dependencyEntries)
      this.#mapArtifacts = this.#artifacts
      finishLoadPhase("presentationParse")
      this.#resetMapBlockers()
      this.#recordVisualOutputBlockers(this.#artifacts)
      await this.#cacheModelArtifacts(this.#artifacts)
      finishLoadPhase("modelCache")
      this.#projectiles = createProjectilePresentationMapper(
        Object.freeze({
          models: new Set(this.#artifacts.models.keys()),
          systems: this.#particleSystems = new Set(this.#artifacts.particleSystems),
          attachments: this.#attachments,
          attachmentTransforms: this.#attachmentTransforms,
          fireAttachmentTransforms: this.#fireAttachmentTransforms,
          localOwnerIdentity: 1,
        }),
      )
      this.#viewmodels = createViewmodelPresenter(this.#artifacts, this.#equipmentProfile!.state()!.inventory)
      this.#viewmodelClass = undefined
      this.#applyInitialView(this.#loaded)
      finishLoadPhase("presentationSetup")
      await this.#equipmentAdmissionTask?.catch(() => {})
      await this.#equipmentRenderTask
      if (this.#renderer) await this.#renderer.dispose()
      this.#renderer = await createRenderer({
        serviceAudio: this.#serviceAudio,
        canvas: this.#canvas,
        configuration: { ...(this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR), alphaMode: "premultiplied" },
        powerPreference: "high-performance",
        sampleCount: this.#videoConfiguration.antialias === 4 ? 4 : 1,
        textureQuality: { mipOffset: this.#videoConfiguration.picmip, trilinear: this.#videoConfiguration.trilinear === 1, anisotropy: this.#videoConfiguration.anisotropy },
      })
      finishLoadPhase("rendererCreate")
      this.#resizeRenderer()
      this.#advanceLoading("creating-client-world")
      const AudioContextConstructor = window.AudioContext
      if (!AudioContextConstructor) throw new Error("Web Audio is unavailable")
      const audioContext = new AudioContextConstructor({ sampleRate: 44100 })
      this.#audioContext = audioContext
      this.#audioRegistry = new SoundRegistry(this.#artifacts.audio.documents.map((document) => Object.freeze({
        logicalPath: document.logicalPath, mode: "base" as const, preload: false, entries: document.entries,
      })))
      const audioPaths = this.#audioRegistry.resources().filter(identity => {
        if (!this.#artifacts!.audio.unavailable.has(identity)) return true
        this.#blockers.add(`Authored sound precache unavailable: ${identity}`)
        return false
      })
      const audioStarted = performance.now()
      for (const identity of audioPaths) if (!this.#dependencyEntries.has(identity)) throw new Error(`Audio dependency ${identity} is missing`)
      if (!this.#presentationRandom) throw new Error("The installed client random stream is unavailable")
      const audioReady = createSourceAudioSystem(audioContext, tf2AudioModuleUrl(), this.#dependencyEntries, this.#presentationRandom,
        Boolean((globalThis as typeof globalThis & { __playsrcProfile?: unknown }).__playsrcProfile)).then(async audio => {
        if (this.#closed || !this.#operations.current(operation) || this.#audioContext !== audioContext) {
          await audio.close()
          throw new Error("Audio map construction was cancelled")
        }
        this.#audio = audio
        return audio
      })
      // Renderer preparation and audio decoding overlap, but either failure
      // still belongs to this one load transaction and its teardown owner.
      void audioReady.catch(() => {})
      const scene = await this.#renderer.loadMap({
        payload: this.#loaded.payload,
        resourceIdentity: this.#dependencies.sha256,
        payloadSha256: this.#loaded.payloadSha256,
        ...mapRendererInputs(this.#artifacts),
        materialStates: this.#materialStates(this.#artifacts),
        modelFacing: this.#modelFacing(this.#artifacts),
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
      await audioReady
      this.#requireOperation(operation)
      if (!this.#audio) throw new Error("Audio map construction has no owner")
      const startupProfile = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
      if (startupProfile) {
        const spans = (startupProfile.startupSpans ??= []) as Array<Record<string, unknown>>
        spans.push({ kind: "audio-decode", started: audioStarted, finished: performance.now(), resources: this.#audio.resources().size })
        startupProfile.audio = this.#audio
      }
      this.#audioBuffers = this.#audio.resources()
      this.#audioWorld = new SourceAudioWorld(this.#audioRegistry, { maxActiveVoices: 128 })
      finishLoadPhase("audioSetup")
      this.#requireOperation(operation)
      await this.#client!.activate(this.#generation)
      this.#requireOperation(operation)
      const applicationProfile = (globalThis as typeof globalThis & { __playsrcProfile?: { applicationGeneration?: Record<string, unknown> } }).__playsrcProfile?.applicationGeneration
      if (applicationProfile) Object.assign(applicationProfile, installedMapProfileIdentity(target, this.#generation))
      await this.#releaseEquipmentAdmissions(this.#generation)
      await this.#admitRestoredEquipment()
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
      selectionTransitionMark("initial-team-admitted", { generation: this.#generation })
      this.#requireOperation(operation)
      this.#preparingModelPipelines = true
      await Promise.all([this.#displayTask, this.#teamSelectionRenderTask, this.#classSelectionRenderTask])
      this.#requireOperation(operation)
      this.#advanceLoading("synchronizing-game-state")
      this.#snapshot = (await this.#initialPublication(this.#generation)).snapshot
      selectionTransitionMark("initial-publication", { generation: this.#generation, team: this.#snapshot.team })
      if (this.#snapshot.objectives && (this.#flagCapturesPerRound !== 3 || this.#flagReturnOnTouch)) {
        this.#objectiveConfiguration = Object.freeze({
          capturesPerRound: this.#flagCapturesPerRound,
          returnOnTouch: this.#flagReturnOnTouch,
        })
      }
      if (this.#pendingLocalMatch?.mapIdentity === target.target) {
        this.#botConfiguration = admitBotConfiguration(
          this.#pendingLocalMatch.configuration, target.target, this.#dependencyEntries,
        )
        this.#pendingLocalMatch = undefined
      }
      this.#requireOperation(operation)
      this.#predictedEye.reset(this.#snapshot.tick, tf2Camera(this.#snapshot, this.#yaw, this.#pitch).position)
      await this.#prepareGameplayPipelines(this.#snapshot.team, true)
      this.#requireOperation(operation)
      finishLoadPhase("initialPublication")
      this.#recordAuthorityBlockers(this.#snapshot)
      this.#recordCrouch(this.#snapshot)
      this.#recordLockerAnimations(this.#snapshot)
      this.#modelProbes = await this.#probePlayerModels(this.#artifacts)
      this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(this.#artifacts)
      finishLoadPhase("initialProbes")
      selectionTransitionMark("initial-probes-ready")
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
      this.#set({ phase: "Failed", gameUi: "failure", detail })
      try {
        this.#gameUi?.dispatch({ kind: "loading-failed", reason: "Map load failed", extendedReason: detail.slice(0, 255) })
        this.#syncLoadingPresentation()
      } catch (presentationError) {
        console.error("TF2 loading failure presentation failed", presentationError)
        this.#set({ bootFailure: true })
      }
    } finally {
      if (this.#operations.current(operation)) this.#preparingModelPipelines = false
    }
  }

  #resetGenerationPresentation(): void {
    this.#preparingModelPipelines = false
    this.#predictedEye.clear()
    this.#particleBatches = createParticleBatchEncoder()
    this.#recordedClientFrames?.close()
    const workloadProfile = (globalThis as any).__playsrcProfile
    const recordedFrames = (globalThis as any).__playsrcCommandWorkload?.clientFrames
    const recordedPresentations = (globalThis as any).__playsrcCommandWorkload?.presentations
    this.#recordedPresentations = recordedPresentations?.length ? { inputs: recordedPresentations, cursor: 0, buffered: [] } : undefined
    if (workloadProfile?.captureClientFrames) workloadProfile.presentationInputs = []
    this.#recordedClientFrames = recordedFrames?.length ? new RecordedClientRenderFrames(recordedFrames) : undefined
    if (workloadProfile?.captureClientFrames) workloadProfile.clientFrames = []
    this.#clientRenderFrames = createClientRenderFrameClock(workloadProfile?.captureClientFrames ? frame => {
      if (workloadProfile.clientFrames.length >= 16384) throw new Error("Client-frame workload recording overflow")
      workloadProfile.clientFrames.push(frame)
    } : undefined)
    if (this.#recordedClientFrames && workloadProfile) workloadProfile.clientFrameWorkload = this.#recordedClientFrames.observations
    this.#medicBeamTarget = null
    this.#medicBeamReleasing = false
    this.#pendingProjectileTimeline = []
    this.#teamSelectionPoses.clear()
    this.#teamSelectionAnimations.clear()
    this.#pendingClassSelectionTeam = undefined
    this.#pendingPresentation = undefined
    this.#preparedPresentation = undefined
    this.#requiredParticleDisplayFrames.reset()
    this.#canvasDiagnostics.clear()
    this.#canvas.dataset.sky3dPass = ""
    this.#canvas.dataset.skyVisibilityDisposition = "not-visible"
    this.#lastRenderedPreparedRevision = 0
    this.#lastRenderedViewRevision = 0
    this.#lastRenderedTick = undefined
    this.#resumePointerOnAcknowledge = false
    this.#fireEvents = 0
    this.#explosionEvents = 0
    this.#audioStarts = []
    this.#pendingAudioRequests = []
    this.#pyroFlameEffect = undefined
    this.#manmelterChargeEffect = undefined
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
    this.#scoreboardVisible = false
    if (this.#hudIntegration) {
      this.#hudIntegration.reset("map-replaced")
      return
    }
    const damageTexture=this.#artifacts.environment.textures.find(texture=>texture.material.toLowerCase()==="materials/vgui/damageindicator.vmt")
    if(!damageTexture)throw new Error("Authored TF2 damage indicator material is unavailable")
    selectionTransitionMark("hud-construction-start")
    this.#hudIntegration = initializeTf2HudIntegration({
      root: this.#hudRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      damageIndicator:Object.freeze({material:damageTexture.material,texture:damageTexture,
        eyePosition:()=>this.#snapshot?tf2Camera(this.#snapshot,this.#yaw,this.#pitch).position:[0,0,0],
        yawDegrees:()=>this.#yaw+(this.#snapshot?.viewAngleOffset[1]??0),random:()=>this.#presentationRandom!.nextUnit()}),
      onCommand: (command) => {

        if (command.kind === "select-weapon" && (command.weapon >= 1 && command.weapon <= 21 || command.weapon >= 40 && command.weapon <= 45 || command.weapon >= 50 && command.weapon <= 54) && command.weapon !== 54) this.#selectWeapon = command.weapon as Tf2Weapon
        else if (command.kind === "scoreboard") this.#setScoreboardVisible(command.visible)

      },
    })
    this.#hudIntegration.setDeathNoticeTime(this.#deathNoticeTime)
    selectionTransitionMark("hud-construction-end")
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
    selectionTransitionMark("class-construction-start")
    this.#classSelection = initializeTf2ClassSelectionIntegration({
      root: this.#classSelectionRoot,
      modelSurface: this.#canvas,
      backgroundSurface: this.#classSelectionBackground ??= Object.assign(document.createElement("canvas"), { className: "class-selection-background-surface" }),
      roster: () => this.#snapshot?.scoreboard.players ?? [],
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      onRequest: (request) => this.#classSelectionRequest(request),
      loadoutAvailable: () => this.#equipmentProfile?.state() !== undefined,
      onEditLoadout: identity => { void this.#showEquipment(identity) },
      onModelPanels: (panels) => {
        if (panels.length === 0 && this.#classSelectionModelPanels.length > 0) this.#releaseCosmeticPreviews()
        if (panels.length === 0) this.#classSelectionBackgroundKey = ""
        if (panels[1]?.model !== this.#classSelectionModelPanels[1]?.model || panels[1]?.skin !== this.#classSelectionModelPanels[1]?.skin) {
          this.#classSelectionAnimationStarted = this.#frameClock.current
          this.#classSelectionAnimationElapsed = 0
          this.#classSelectionAnimationReset = true
          this.#classSelectionWeaponVisible = true
        }
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
    selectionTransitionMark("class-construction-end")
  }

  async #showEquipment(playerClass?: Tf2Class): Promise<void> {
    const operation = this.#operation
    await this.#ensureResourceRuntime()
    if (this.#closed || !this.#operations.current(operation) || this.#view.gameUi === "loading") return
    const state = this.#equipmentProfile?.state()
    if (!state || !this.#uiResources || !this.#presentationRandom) throw new Error("Local equipment owner is unavailable")
    if (!this.#equipment) {
      const root = document.createElement("div")
      root.className = "equipment-layer"
      Object.assign(root.style, { position: "absolute", inset: "0", zIndex: "30", display: "none" })
      this.#classSelectionRoot.after(root)
      this.#equipmentRoot = root
      this.#equipment = new Tf2EquipmentPresentation({ root, resources: this.#uiResources, viewport: this.#viewport(),
        reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
        clock: { nowSeconds: () => this.#frameClock.current }, random: this.#presentationRandom,
        modelSurface: this.#canvas,
        onEquip: async (identity, slot, definition) => {
          const loadout = this.#equipmentProfile!.state()!.classes[identity - 1]!
          const replacement = definition ?? loadout.baseItems.find(item => item.slot === slot)?.definitionIndex
          const definitions = [...new Set([...loadout.items.filter(item => item.slot !== slot).map(item => item.definitionIndex), ...(replacement === undefined ? [] : [replacement])])]
          try {
            await this.#admitEquipment(definitions, this.#loaded ? this.#generation : 0)
            return await this.#equipmentProfile!.equip(identity, slot, definition)
          } catch (error) {
            if (error instanceof DOMException && error.name === "AbortError") return this.#equipmentProfile!.state()!
            this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "Equipment admission failed") })
            return this.#equipmentProfile!.state()!
          }
        },
        onClose: () => { this.#neutral() },
        onPreview: preview => {
          const previous = this.#equipmentPreview
          this.#equipmentPreview = preview
          if (this.#equipmentRoot) delete this.#equipmentRoot.dataset.previewModel
          if (!preview && previous) this.#releaseCosmeticPreviews()
          if (preview && (!previous || previous.class !== preview.class || previous.equippedItems !== preview.equippedItems)) {
            this.#equipmentPreviewReset = true; this.#equipmentPreviewElapsed = 0; this.#equipmentPreviewStarted = this.#frameClock.current
          }
        },
      })
    }
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) void document.exitPointerLock()
    this.#classSelection?.dispatch({ kind: "hide" })
    this.#teamSelection?.dispatch({ kind: "hide" })
    this.#equipment.show(state, playerClass)
  }

  #releaseCosmeticPreviews(): void {
    const client = this.#client, generation = this.#loaded ? this.#generation : 0
    if (!client || generation === 0 && !this.#equipmentAdmissions.has(0)) return
    void client.models(generation, encodeModelPoseBatch([])).catch(error => {
      if (!this.#closed && generation === this.#generation && this.#view.phase === "Ready") this.#set({ phase: "Failed", detail: this.#failureDetail(error, "Model preview cleanup failed") })
    })
  }

  async #releaseEquipmentAdmissions(except?: number): Promise<void> {
    for (const [generation, admission] of this.#equipmentAdmissions) {
      if (generation === except) continue
      for (const source of admission.sources) await this.#client?.releaseResources(source)
      this.#equipmentAdmissions.delete(generation)
      if (generation === 0) this.#equipmentPanelArtifacts = undefined
    }
  }

  async #admitRestoredEquipment(): Promise<void> {
    const state = this.#equipmentProfile!.state()!
    const definitions = new Set(state.classes.flatMap(loadout => loadout.items.map(item => item.definitionIndex)))
    const missing = state.inventory.filter(item => definitions.has(item.item.definitionIndex)
      && item.modelPlayer !== "" && !this.#artifacts!.models.has(item.modelPlayer)).map(item => item.item.definitionIndex)
    if (missing.length) await this.#admitEquipment(missing, this.#generation)
  }

  async #admitEquipment(definitions: readonly number[], generation: number): Promise<void> {
    const priorTask = this.#equipmentAdmissionTask
    const epoch = this.#equipmentAdmissionEpoch, operation = this.#operation
    const task = (async () => {
      await priorTask?.catch(error => { if (error?.name !== "AbortError") throw error })
      if (this.#closed || epoch !== this.#equipmentAdmissionEpoch || !this.#operations.current(operation)) throw new DOMException("Equipment admission was replaced", "AbortError")
      const previous = this.#equipmentAdmissions.get(generation)
      const wanted = [...new Set(definitions)]
      if (previous && wanted.every(definition => previous.definitions.has(definition)) && (generation !== 0 || wanted.length === previous.definitions.size)) return
      const requested = generation === 0 ? wanted : wanted.filter(definition => !previous?.definitions.has(definition))
      const resources = new Map<string, Uint8Array>()
      const configuration = await this.#decodeResourceSet(this.#resourceGraph!, requested.map(definition => `equipment-${definition}`), resources, operation.signal)
      if (!configuration) throw new Error("Equipment admission did not create a bounded resource generation")
      let retained = false
      try {
        const profile = this.#renderLevel === 2 ? 1 : 0
        const bytes = await this.#client!.admitEquipmentModels(generation, requested, configuration, profile)
        const artifacts = parseEquipmentModelArtifacts(bytes, resources)
        if (epoch !== this.#equipmentAdmissionEpoch || !this.#operations.current(operation)) throw new DOMException("Equipment admission was replaced", "AbortError")
        this.#equipmentPreparing = true
        await Promise.all([this.#displayTask, this.#equipmentRenderTask, this.#classSelectionRenderTask, this.#teamSelectionRenderTask])
        if (!this.#renderer) {
          this.#renderer = await createRenderer({ canvas: this.#canvas,
            serviceAudio: this.#serviceAudio,
            configuration: { ...(profile === 1 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR), alphaMode: "premultiplied" }, powerPreference: "high-performance",
            sampleCount: this.#videoConfiguration.antialias === 4 ? 4 : 1,
            textureQuality: { mipOffset: this.#videoConfiguration.picmip, trilinear: this.#videoConfiguration.trilinear === 1, anisotropy: this.#videoConfiguration.anisotropy } })
          this.#resizeRenderer()
        }
        if (epoch !== this.#equipmentAdmissionEpoch) throw new DOMException("Equipment admission was replaced", "AbortError")
        await this.#renderer.admitModels({ models: artifacts.geometry, modelMaterials: artifacts.modelMaterials, authoredTextures: artifacts.authoredTextures,
          materialStates: artifacts.materialStates, modelFacing: this.#modelFacing(artifacts), particleTextures: artifacts.particleTextures })
        if (generation === 0) {
          this.#equipmentPanelArtifacts = artifacts
          if (previous) for (const source of previous.sources) await this.#client!.releaseResources(source)
          this.#equipmentAdmissions.set(0, { definitions: new Set(wanted), sources: new Set([configuration.generation]) })
        } else {
          if (!this.#artifacts || generation !== this.#generation) throw new DOMException("Equipment map was replaced", "AbortError")
          this.#artifacts = Object.freeze({ ...this.#artifacts, models: new Map([...this.#artifacts.models, ...artifacts.models]),
            materialStates: new Map([...this.#artifacts.materialStates, ...artifacts.materialStates]), modelMaterials: new Map([...this.#artifacts.modelMaterials, ...artifacts.modelMaterials]),
            authoredTextures: new Map([...this.#artifacts.authoredTextures, ...artifacts.authoredTextures]) })
          this.#viewmodels?.updateArtifacts(this.#artifacts)
          this.#equipmentAdmissions.set(generation, { definitions: new Set([...(previous?.definitions ?? []), ...wanted]), sources: new Set([...(previous?.sources ?? []), configuration.generation]) })
        }
        retained = true
        const camera: Camera = { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0, verticalFovDegrees: 30, near: 1, far: 16384 * Math.sqrt(3) }
        const viewport = this.#viewport()
        const requests = equipmentPipelinePoseRequests(artifacts, viewport.width / viewport.height)
        const passes = generation === 0 ? ["panel"] as const : ["panel", "view", "world"] as const
        for (let offset = 0; offset < requests.length; offset += 32) {
          const batch = requests.slice(offset, offset + 32)
          const poses = await this.#client!.models(generation, encodeModelPoseBatch(batch))
          if (epoch !== this.#equipmentAdmissionEpoch) throw new DOMException("Equipment admission was replaced", "AbortError")
          const byIdentity = new Map(batch.map(request => [request.identity, request]))
          await this.#renderer.prepareModelPipelines(poses.flatMap(pose => {
            const request = byIdentity.get(pose.identity)
            if (!request?.lighting || !pose.lighting) throw new Error(`Equipment pipeline pose unavailable: ${pose.model}`)
            const item = { identity: pose.identity, model: pose.model, skin: request.skin,
              position: request.lighting.origin, angles: request.lighting.angles, scale: 1, pose, modelLighting: pose.lighting, eyeStates: pose.eyes }
            return passes.map(pass => ({ pass, item }))
          }), camera, generation === 0 ? undefined : this.#mainFog(this.#artifacts!))
        }
        if (artifacts.particleTextures.length) await this.#renderer.prepareParticlePipelines(camera)
      } finally {
        this.#equipmentPreparing = false
        if (!retained) await this.#client?.releaseResources(configuration.generation).catch(() => {})
      }
    })().catch(error => {
      if (this.#closed || epoch !== this.#equipmentAdmissionEpoch || !this.#operations.current(operation)) throw new DOMException("Equipment admission was replaced", "AbortError")
      throw error
    })
    this.#equipmentAdmissionTask = task
    try { await task } finally { if (this.#equipmentAdmissionTask === task) this.#equipmentAdmissionTask = undefined }
  }

  #renderEquipment(): void {
    const preview = this.#equipmentPreview, renderer = this.#renderer, client = this.#client
    if (!preview || this.#equipmentAdmissionTask || this.#equipmentPreparing) return
    const generation = this.#loaded ? this.#generation : 0
    const admission = this.#equipmentAdmissions.get(generation)
    if (!admission || !preview.equippedItems.every(item => admission.definitions.has(item.definitionIndex)) || generation === 0 && admission.definitions.size !== preview.equippedItems.length) {
      void this.#admitEquipment(preview.equippedItems.map(item => item.definitionIndex), generation).catch(error => {
        if (this.#equipmentPreview === preview && error?.name !== "AbortError") this.#set({ phase: "Failed", gameUi: "failure", detail: String(error) })
      })
      return
    }
    const artifacts = generation === 0 ? this.#equipmentPanelArtifacts : this.#artifacts
    if (!preview || !renderer || !client || !artifacts || this.#equipmentRenderTask || this.#displayTask || this.#classSelectionRenderTask || this.#teamSelectionRenderTask) return
    const now = this.#frameClock.current
    const player = tf2ClassPresentation(preview.class), artifact = artifacts.models.get(player.model)
    if (!artifact) return
    const profile = this.#equipmentProfile!.state()!
    const held = preview.equippedItems.map(item => profile.inventory.find(value => value.item.definitionIndex === item.definitionIndex)).find(item => item?.weapon !== null && item?.modelPlayer)
    const previous = this.#equipmentPreviewElapsed, elapsed = Math.max(previous, now - this.#equipmentPreviewStarted)
    this.#equipmentPreviewElapsed = elapsed
    const reset = this.#equipmentPreviewReset; this.#equipmentPreviewReset = false
    const skin = this.#snapshot?.team === 3 ? 1 : 0
    const equipmentProfile = (globalThis as any).__playsrcProfile
    const equipmentStarted = equipmentProfile?.captureEquipment ? performance.now() : undefined
    this.#equipmentRenderTask = (async () => {
      const poses = await client.models(generation, encodeModelPoseBatch([{
        identity: 0x3001, model: player.model, itemModel: held?.modelPlayer, worldItem: Boolean(held?.modelPlayer),
        itemDefinition: held?.item.definitionIndex,
        itemBodygroups: held?.modelPlayer ? artifacts.models.get(held.modelPlayer)?.bodygroupCounts.map(() => 0) : undefined,
        equippedItems: preview.equippedItems,
        modelPanel: true, modelPanelReset: reset, activity: held ? "ACT_MP_STAND_IDLE" : classPreviewBaseActivity(preview.class),
        previousElapsedSeconds: previous, elapsedSeconds: elapsed, currentTimeSeconds: now, frameTimeSeconds: elapsed - previous,
        planarSpeed: 0, screenAspectRatio: preview.bounds.width / preview.bounds.height, worldFarPlane: 16384 * Math.sqrt(3),
        skin, lod: 0, bodygroups: artifact.bodygroupCounts.map(() => 0),
        lighting: { origin: preview.origin, angles: preview.angles, cameraPosition: [0, 0, 0], cameraAngles: [0, 0, 0] },
      }]))
      if (this.#equipmentPreview !== preview || generation !== (this.#loaded ? this.#generation : 0)) return
      const pose = poses.find(pose => pose.role === "single")
      if (!pose) throw new Error("Equipment player pose is unavailable")
      const mergedModels = poses.filter(pose => pose.role !== "single" && pose.role !== "hand").map(pose => ({ model: pose.model,
        skin: skin < (artifacts.models.get(pose.model)?.skinCount ?? 0) ? skin : 0, pose, modelLighting: pose.lighting ?? undefined, eyeStates: pose.eyes }))
      await renderer.renderModelPanels([{ identity: "EquipmentPlayer", model: player.model, skin, kind: "studio", fov: preview.fov,
        origin: preview.origin, angles: preview.angles, bounds: preview.bounds, background: "clear-transparent", presentationTimeSeconds: now,
        pose, mergedModels, modelLighting: pose.lighting ?? undefined, eyeStates: pose.eyes,
        particles: poses.flatMap(pose => pose.wearable?.particleBytes.byteLength ? decodeParticleRenderOutput(pose.wearable.particleBytes, artifacts.particleMaterials).items : []),
      }])
      if (this.#equipmentRoot && this.#equipmentRoot.dataset.previewModel !== player.model) this.#equipmentRoot.dataset.previewModel = player.model
      if (equipmentStarted !== undefined && equipmentProfile.captureEquipment) equipmentProfile.equipmentFrames.push(performance.now() - equipmentStarted)
      const cosmeticProfile = (globalThis as any).__playsrcProfile
      if (cosmeticProfile?.captureCosmetics) cosmeticProfile.cosmeticPreview = { class: preview.class, model: player.model, time: now,
        wearables: poses.filter(pose => pose.wearable).map(pose => pose.model),
        particles: poses.reduce((count, pose) => count + (pose.wearable?.particleBytes.byteLength ? new DataView(pose.wearable.particleBytes.buffer, pose.wearable.particleBytes.byteOffset).getUint32(8, true) : 0), 0) }
    })().catch(error => { if (this.#equipmentPreview === preview) this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "Equipment preview failed") }) })
      .finally(() => { this.#equipmentRenderTask = undefined })
  }

  #classSelectionRequest(request: Tf2ClassSelectionRequest): void {
    selectionTransitionMark("class-request", { identity: request.identity, generation: this.#generation })
    this.#selectionViewportEpoch += 1
    this.#pendingClassSelectionTeam = undefined
    const identity = request.identity
    if (identity === 12) this.#selectClass = 12
    else this.selectClass(identity)
    this.#set({ classSelectionVisible: false, classSelectionModels: "" })
  }

  #showClassSelection(initialJoin = false): void {
    if (!this.#classSelection || !this.#snapshot || this.#view.gameUi !== "in-game"
      || (this.#snapshot.team !== 2 && this.#snapshot.team !== 3)) return
    this.#selectionViewportEpoch += 1
    this.#teamSelection?.dispatch({ kind: "hide" })
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) void document.exitPointerLock()
    selectionTransitionMark("class-show-start", { initialJoin, team: this.#snapshot.team })
    this.#classSelection.dispatch({
      kind: "show",
      team: this.#snapshot.team,
      current: initialJoin ? null : this.#snapshot.class as Tf2ClassIdentity,
    })
    selectionTransitionMark("class-show-end")
  }

  #renderClassSelection(): void {
    if (this.#preparingModelPipelines || this.#equipmentPreparing) return
    if (!this.#renderer || !this.#client || !this.#artifacts || this.#classSelectionModelPanels.length === 0 || this.#classSelectionRenderTask || this.#displayTask || this.#teamSelectionRenderTask) return
    const renderer = this.#renderer
    const client = this.#client
    const revision = this.#classSelectionRenderRevision
    const generation = this.#generation
    const authored = this.#classSelectionModelPanels
    const now = this.#frameClock.current
    const elapsed = Math.max(0, now - this.#classSelectionAnimationStarted)
    const previous = Math.min(this.#classSelectionAnimationElapsed, elapsed)
    this.#classSelectionAnimationElapsed = elapsed
    const player = authored[1]!
    const artifact = this.#artifacts.models.get(player.model)
    this.#classSelectionRenderTask = (async () => {
      const background = authored[0]!
      const backgroundKey = `${generation}:${this.#canvas.width}:${this.#canvas.height}:${JSON.stringify(background)}`
      if (backgroundKey !== this.#classSelectionBackgroundKey) {
        const animation = this.#artifacts!.models.get(background.model)?.sequences[0]
        if (animation && animation.weightedFrameCount > 1) throw new Error("Authored class-selection background requires animated-surface presentation")
        const rendered = await renderer.renderModelPanels([{ identity: background.name, model: background.model, skin: background.skin,
          kind: "entity", fov: background.fov, origin: background.origin, angles: background.angles, bounds: background.bounds, background: "opaque" }])
        const bitmap = await createImageBitmap(this.#canvas)
        if (generation !== this.#generation || revision !== this.#classSelectionRenderRevision) { bitmap.close(); return }
        const surface = this.#classSelectionBackground!
        surface.width = this.#canvas.width
        surface.height = this.#canvas.height
        surface.style.width = "100%"
        surface.style.height = "100%"
        surface.style.position = "absolute"
        const context = surface.getContext("bitmaprenderer")
        if (!context) { bitmap.close(); throw new Error("Authored class background bitmap presentation is unavailable") }
        context.transferFromImageBitmap(bitmap)
        this.#classSelectionBackgroundKey = backgroundKey
        this.#classSelectionBackgroundPrimitives = rendered.panels[0]!.primitives
      }
      let pose: PosedModel | undefined
      let carried: PosedModel | undefined
      let wearables: readonly PosedModel[] = []
      {
        if (!artifact) throw new Error(`TF2 class-selection model is unavailable: ${player.model}`)
        const selected = this.#classSelection!.state().selected
        const classSelection = selected !== 12
        const modelPanelReset = this.#classSelectionAnimationReset
        this.#classSelectionAnimationReset = false
        const randomSequence = classSelection ? undefined : artifact.sequences.find(sequence => sequence.index === 1)
        if (!classSelection && !randomSequence) throw new Error("Authored random-class idle sequence is unavailable")
        const poses = await client.models(generation, encodeModelPoseBatch([{
          identity: 0x2001 + player.skin, model: player.model, classSelection, modelPanel: true, modelPanelReset,
          equippedItems: selected === 12 ? [] : this.#equipmentProfile?.state()?.classes[selected - 1]?.items,
          activity: randomSequence?.label ?? classPreviewBaseActivity(selected as Tf2Class),
          previousElapsedSeconds: previous, elapsedSeconds: elapsed,
          currentTimeSeconds: now, frameTimeSeconds: elapsed - previous, planarSpeed: 0,
          screenAspectRatio: player.bounds.width / player.bounds.height, worldFarPlane: 16384 * Math.sqrt(3),
          skin: player.skin, lod: 0, bodygroups: artifact.bodygroupCounts.map(() => 0),
          lighting: { origin: player.origin, angles: player.angles, cameraPosition: [0, 0, 0], cameraAngles: [0, 0, 0] },
        }]))
        if (generation !== this.#generation || revision !== this.#classSelectionRenderRevision) return
        pose = poses.find((value) => value.role === "single")
        carried = poses.find((value) => value.role === "item")
        wearables = poses.filter(value => value.role === "wearable")
        if (!pose) throw new Error("TF2 class-selection pose is unavailable")
        if (classSelection && !carried) throw new Error("TF2 class-selection carried item is unavailable")
        for (const event of pose.events) {
          if (event.name === "AE_WPN_HIDE") this.#classSelectionWeaponVisible = false
          if (event.name === "AE_WPN_UNHIDE") this.#classSelectionWeaponVisible = true
        }
      }
    const panels = authored.slice(1).map((panel) => Object.freeze({
      identity: panel.name,
      model: panel.model,
      skin: panel.skin,
      kind: panel.name === "MenuBG" ? "entity" as const : "studio" as const,
      fov: panel.fov,
      origin: panel.origin,
      angles: panel.angles,
      bounds: panel.bounds,
      background: "clear-transparent" as const,
      presentationTimeSeconds: now,
      ...(pose ? { pose, modelLighting: pose.lighting ?? undefined, eyeStates: pose.eyes } : {}),
      mergedModels: [...(carried && this.#classSelectionWeaponVisible ? [carried] : []), ...wearables].map(model => ({ model: model.model,
        skin: panel.skin < (this.#artifacts!.models.get(model.model)?.skinCount ?? 0) ? panel.skin : 0, pose: model,
        modelLighting: model.lighting ?? undefined, eyeStates: model.eyes })),
      particles: wearables.flatMap(model => model.wearable?.particleBytes.byteLength ? decodeParticleRenderOutput(model.wearable.particleBytes, this.#artifacts!.particleMaterials).items : []),
    }))
      const result = await renderer.renderModelPanels(panels)
        if (generation !== this.#generation || revision !== this.#classSelectionRenderRevision) return
        selectionTransitionDraw({ scene: "class", generation, revision, team: player.skin + 2, model: pose?.model })
        this.#set({ classSelectionModels: [`MenuBG:${background.model}:${background.skin}:${this.#classSelectionBackgroundPrimitives}`, ...result.panels.map((panel) => `${panel.identity}:${panel.model}:${panel.skin}:${panel.primitives}`)].join("|"),
          classSelectionAnimation: pose ? JSON.stringify({ model: pose.model, activity: pose.activity, cycle: pose.cycle,
            flexVertices: pose.flex.reduce((count, primitive) => count + primitive.indices.length, 0),
            weapon: carried?.model, weaponVisible: this.#classSelectionWeaponVisible }) : "" })
      })()
      .catch((error) => {
        if (generation !== this.#generation || !this.#classSelection?.state().visible) return
        this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "TF2 class model rendering failed") })
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
    const selectionEpoch = ++this.#selectionViewportEpoch
    selectionTransitionMark("team-request", { team: request.team, generation })
    const server = await this.#client.teamSelection(generation, request.team)
    selectionTransitionMark("team-acknowledged", { team: server.localTeam, generation })
    if (generation !== this.#generation || this.#closed) return
    if (selectionEpoch === this.#selectionViewportEpoch && this.#snapshot && (server.localTeam === 2 || server.localTeam === 3)
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
    const epoch = ++this.#selectionViewportEpoch
    const server = await this.#client.teamSelection(generation)
    if (generation !== this.#generation || epoch !== this.#selectionViewportEpoch || this.#closed) return
    this.#classSelection?.dispatch({ kind: "hide" })
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) await document.exitPointerLock()
    this.#teamSelectionUpdateTime = 0
    this.#teamSelection.dispatch({ kind: "show", server })
  }

  #renderTeamSelection(): void {
    if (this.#preparingModelPipelines || this.#equipmentPreparing) return
    if (!this.#renderer || !this.#client || !this.#artifacts
      || this.#teamSelectionModelPanels.length === 0 || this.#teamSelectionRenderTask || this.#displayTask || this.#classSelectionRenderTask) return
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
        const artifact = artifacts.models.get(panel.model.toLowerCase())
        const timing = artifact?.sequences.find((sequence) => sequence.label.toLowerCase() === panel.sequence.toLowerCase())
        if (!artifact || !timing) throw new Error(`TF2 authored team-door sequence is unavailable: ${panel.model}:${panel.sequence}`)
        const step = teamModelPlayback(this.#teamSelectionAnimations.get(panel.name), panel, now, timing.durationSeconds)
        this.#teamSelectionAnimations.set(panel.name, step.state)
        if (!step.sample) return []
        const request = Object.freeze({
          identity: 0x1000 + index,
          entityModelPanel: true,
          modelPanelReset: step.reset,
          model: panel.model.toLowerCase(),
          activity: panel.sequence,
          previousElapsedSeconds: step.previousElapsed,
          elapsedSeconds: step.elapsed,
          currentTimeSeconds: now,
          frameTimeSeconds: step.frameTime,
          planarSpeed: 0,
          screenAspectRatio: viewport.width / viewport.height,
          worldFarPlane: 1000,
          skin: panel.skin,
          lod: 0,
          bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
        })
        return [Object.freeze({ panel: panel.name, state: step.state, request })]
      })
      if (requests.length > 0) {
        const posed = await client.models(generation, encodeModelPoseBatch(requests.map((value) => value.request)))
        if (generation !== this.#generation || revision !== this.#teamSelectionRenderRevision || !this.#teamSelection?.state().visible) return
        for (const item of posed) {
          const selected = requests.find((candidate) => candidate.request.identity === item.identity)
          if (!selected) throw new Error("TF2 team-door pose identity differs from its authored request")
          this.#teamSelectionPoses.set(selected.panel, item)
          this.#teamSelectionAnimations.set(selected.panel, { ...selected.state, sampledSeconds: selected.request.elapsedSeconds })
        }
      }
      const panels: readonly ModelPanelPass[] = authored.map((panel, index) => {
        const pose = this.#teamSelectionPoses.get(panel.name)
        return Object.freeze({
          identity: panel.name,
          model: panel.model,
          skin: panel.skin,
          kind: "entity" as const,
          fov: panel.fov,
          origin: panel.origin,
          angles: panel.angles,
          bounds: panel.bounds,
          background: index === 0 ? "opaque" as const : "transparent" as const,
          presentationTimeSeconds: now,
          ...(pose ? { pose } : {}),
        })
      })
      const result = await renderer.renderModelPanels(panels)
      if (generation !== this.#generation || revision !== this.#teamSelectionRenderRevision) return
      const doorProfile = (globalThis as any).__playsrcProfile
      if (doorProfile?.captureTeamDoors === true) {
        const frames = doorProfile.teamDoorFrames ??= []
        if (frames.length < 4096) frames.push({ at: performance.now(), now, milliseconds: result.milliseconds,
          panels: authored.map(panel => ({ name: panel.name, animation: panel.animation, sequence: panel.sequence,
            timing: this.#teamSelectionAnimations.get(panel.name),
            cycle: this.#teamSelectionPoses.get(panel.name)?.cycle,
            matrices: Array.from(this.#teamSelectionPoses.get(panel.name)?.boneMatrices ?? []) })) })
      }
      this.#set({ teamSelectionModels: result.panels.map((panel) => `${panel.identity}:${panel.model}:${panel.skin}:${panel.primitives}`).join("|") })
    })().catch((error) => {
      if (generation !== this.#generation || !this.#teamSelection?.state().visible) return
      this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "TF2 team model rendering failed") })
    }).finally(() => { this.#teamSelectionRenderTask = undefined })
  }

  #updateTeamSelection(timeSeconds: number): void {
    if (!this.#teamSelection?.state().visible || !this.#client || this.#teamSelectionUpdateTask
      || timeSeconds < this.#teamSelectionUpdateTime) return
    this.#teamSelectionUpdateTime = timeSeconds + 0.1
    const generation = this.#generation, epoch = this.#selectionViewportEpoch
    this.#teamSelectionUpdateTask = this.#client.teamSelection(generation).then(server => {
      if (generation !== this.#generation || epoch !== this.#selectionViewportEpoch || !this.#teamSelection?.state().visible) return
      if (JSON.stringify(server) !== JSON.stringify(this.#teamSelection.state().server)) this.#teamSelection.dispatch({ kind: "update", server })
    }).catch(error => {
      if (generation === this.#generation && epoch === this.#selectionViewportEpoch && this.#teamSelection?.state().visible)
        this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "TF2 team state update failed") })
    }).finally(() => { this.#teamSelectionUpdateTask = undefined })
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
    const classSelection = this.#view.classSelectionVisible === true || this.#equipment?.visible() === true
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
    const previousScoreboard = this.#hudContext?.scoreboard.kind === "available" ? this.#hudContext.scoreboard.value : undefined
    const scoreboard = adaptTf2Scoreboard(snapshot.scoreboard, snapshot.team, this.#scoreboardVisible, this.#mapIdentity, this.#scoreboardPingAsText, previousScoreboard)
    if (this.#hudContext && this.#hudContextIdentity === identity && previousScoreboard === scoreboard) return this.#hudContext
    this.#hudContextIdentity = identity
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
      scoreboard: tf2HudAvailable<Tf2HudScoreboard>(scoreboard),
      freezePanel: tf2HudUnavailable<Tf2HudFreezePanel>("not-produced"),
      playerClassUsePlayerModel: this.#playerClassUsePlayerModel,
      inventory: this.#equipmentProfile!.state()!.inventory,
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
    const combatSettings=["tf_dingalingaling","tf_dingalingaling_lasthit","tf_remember_activeweapon","tf_remember_lastswitched"] as const
    return Object.freeze({
      revision: `tf2-jump-catalog-developer-${this.#developer}-console-${Number(this.#consoleEnabled)}-fps-${this.#showFps}-pos-${this.#showPos}-hdr-${this.#renderLevel}-flagcaps-${this.#flagCapturesPerRound}-flagreturn-${Number(this.#flagReturnOnTouch)}-settings-${this.#settings?.snapshot().settings.revision ?? 0}`,
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
        ...["build","destroy","hurtbuilding","+attack","-attack","bot_teleport","bot_whack","bot_command","ent_fire"].map(name=>Object.freeze({kind:"command" as const,name,disposition:"visible" as const,acceptsSuggestions:false})),
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
          name: "nb_stop",
          disposition: "visible" as const,
          displayValue: this.#nextBotStop ? "1" : "0",
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
          ["tf_flag_caps_per_round", String(this.#flagCapturesPerRound)],
          ["tf_flag_return_on_touch", String(Number(this.#flagReturnOnTouch))],
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
        Object.freeze({ kind: "convar" as const, name: "hud_deathnotice_time", disposition: "visible" as const,
          displayValue: String(this.#deathNoticeTime) }),
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
        ...Object.entries(tf2VideoConvars(this.#videoConfiguration)).map(([name, value]) => Object.freeze({
          kind: "convar" as const,
          name,
          disposition: "visible" as const,
          displayValue: String(value),
        })),
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
    this.#equipment?.setViewport(viewport)
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
          ? this.#selectableMaps().map((target) => `map ${target}`)
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
    if (request.kind === "submission") {
      void this.#execute(request.text).catch((error) => {
        this.#output(`ERROR: ${error instanceof Error ? error.message : "Console command failed"}`)
      })
    }
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
    const tokens = [...tokenizeSourceCommand(input)]
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
    const videoConvars = tf2VideoConvars(this.#videoConfiguration)
    if (Object.hasOwn(videoConvars, command)) {
      if (tokens.length > 1 || (tokens.length === 1 && !/^-?\d+$/.test(tokens[0]!))) {
        this.#output(`${command} requires one supported integer value`)
        return
      }
      if (tokens.length === 1) {
        if (command === "mat_vsync") {
          this.#output("mat_vsync presentation synchronization is unavailable in browser WebGPU")
          return
        }
        try {
          const selected = tf2VideoSettingsFromConvars(this.#videoConfiguration, { [command]: Number(tokens[0]) })
          const current = this.#settings?.snapshot().settings.current
          if (!this.#settings || !current) throw new Error("video settings authority is unavailable")
          for (const [setting, value] of Object.entries(selected)) {
            if (current[setting] !== value) this.#settings.set(setting, value)
          }
          const applied = await this.#settings.apply()
          if (!applied.lastApply?.complete) throw new Error(applied.lastApply?.rejections[0]?.reason ?? "video settings transaction was rejected")
          localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY, new TextDecoder().decode(this.#settings.persistence()))
          this.#set({ settingsPersistence: "stored" })
          this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
        } catch (error) {
          this.#settings?.cancel()
          this.#output(`${command} rejected: ${error instanceof Error ? error.message : String(error)}`)
          return
        }
      }
      this.#output(`${command} = ${tf2VideoConvars(this.#videoConfiguration)[command]}`)
      return
    }
    const crosshair = TF2_CROSSHAIR_SETTINGS.find((setting) => setting.name === command)
    if (command === "tf_remember_activeweapon" || command === "tf_remember_lastswitched") {
      if (!this.#settings || tokens.length > 1) { this.#output(`${command} accepts one integer`); return }
      if (tokens[0] !== undefined) {
        this.#settings.synchronize({ [command]: Number.parseInt(tokens[0], 10) > 0 })
        this.#replaceWeaponPreferences(this.#settings.snapshot().settings.current)
        localStorage.setItem(TF2_BROWSER_SETTINGS_STORAGE_KEY, new TextDecoder().decode(this.#settings.persistence()))
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`"${command}" = "${Number(this.#settings.snapshot().settings.current[command] === true)}"`)
      return
    }
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
    if (command === "hud_deathnotice_time" && tokens.length <= 1) {
      if (tokens.length === 1) {
        const value = Math.fround(Number.parseFloat(tokens[0]!) || 0)
        if (!Number.isFinite(value)) { this.#output(`${command} requires finite seconds`); return }
        this.#deathNoticeTime = value
        this.#hudIntegration?.setDeathNoticeTime(value)
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`"${command}" = "${this.#deathNoticeTime}" ( def. "6" )`)
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
    if (command === "nb_stop" && tokens.length <= 1) {
      if (tokens.length === 1 && !["0", "1"].includes(tokens[0]!)) {
        this.#output("nb_stop accepts exactly 0 or 1")
        return
      }
      if (tokens.length === 1) {
        this.#nextBotStop = tokens[0] === "1"
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      this.#output(`"nb_stop" = "${this.#nextBotStop ? 1 : 0}" ( def. "0" )`)
      return
    }
    if (command === "tf_bot_difficulty" && tokens.length <= 1) {
      if (tokens.length === 1 && !["0", "1", "2", "3"].includes(tokens[0]!)) {
        this.#output("tf_bot_difficulty accepts exactly 0, 1, 2, or 3")
        return
      }
      if (tokens[0]) {
        this.#botDifficulty = Number(tokens[0]) as 0 | 1 | 2 | 3
        if (this.#activeBotConfiguration) {
          this.#activeBotConfiguration = Object.freeze({ ...this.#activeBotConfiguration, difficulty: this.#botDifficulty })
          this.#botConfiguration = admitBotConfiguration(
            this.#activeBotConfiguration, this.#mapIdentity, this.#dependencyEntries,
          )
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
        if (this.#snapshot) {
          this.#botConfiguration = admitBotConfiguration(
            this.#activeBotConfiguration, this.#mapIdentity, this.#dependencyEntries,
          )
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
    if ((command === "tf_flag_caps_per_round" || command === "tf_flag_return_on_touch") && tokens.length <= 1) {
      if (tokens.length === 1) {
        const value = tokens[0]!
        if (command === "tf_flag_caps_per_round") {
          if (!/^(?:0|[1-9][0-9]*)$/u.test(value) || Number(value) > 0xffff) {
            this.#output("tf_flag_caps_per_round accepts exactly 0 through 65535")
            return
          }
          this.#flagCapturesPerRound = Number(value)
        } else {
          if (value !== "0" && value !== "1") {
            this.#output("tf_flag_return_on_touch accepts exactly 0 or 1")
            return
          }
          this.#flagReturnOnTouch = value === "1"
        }
        if (this.#snapshot?.objectives) {
          this.#objectiveConfiguration = Object.freeze({
            capturesPerRound: this.#flagCapturesPerRound,
            returnOnTouch: this.#flagReturnOnTouch,
          })
        }
        this.#console?.apply({ kind: "replace-catalog", catalog: this.#catalog() })
      }
      const value = command === "tf_flag_caps_per_round"
        ? String(this.#flagCapturesPerRound)
        : String(Number(this.#flagReturnOnTouch))
      this.#output(`"${command}" = "${value}" ( def. "${command === "tf_flag_caps_per_round" ? "3" : "0"}" )`)
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
    if (command === "bot_command") {
      const bot = this.#snapshot?.scoreboard.players.find(player => player.fake && player.name === tokens[0])
      if (!bot || bot.class !== 8 || tokens.length !== 3 || !["addcond", "removecond"].includes(tokens[1]!) || tokens[2] !== "4" || this.#botControl) {
        this.#output("Usage: bot_command <bot name> addcond|removecond 4")
        return
      }
      this.#botControl = Object.freeze({ action: "stealth-condition", identity: bot.identity, enabled: tokens[1] === "addcond" })
      return
    }
    if (command === "bot_teleport" || command === "bot_whack") {
      const teleport = command === "bot_teleport"
      const usage = teleport
        ? "Usage: bot_teleport <bot name> <X> <Y> <Z> <Pitch> <Yaw> <Roll>"
        : "Usage: bot_whack <bot name>"
      if (tokens.length !== (teleport ? 7 : 1)) {
        this.#output(usage)
        return
      }
      const bot = this.#snapshot?.scoreboard.players.find(player => player.fake && player.name === tokens[0])
      if (!bot) {
        this.#output(`No bot with name ${tokens[0]}`)
        return
      }
      if (this.#botControl) {
        this.#output(`${command} rejected: an earlier bot control is pending`)
        return
      }
      if (teleport) {
        const values = tokens.slice(1).map(Number)
        if (!values.every(Number.isFinite) || values[5] !== 0) {
          this.#output("bot_teleport rejected: coordinates and angles must be finite and roll must be zero")
          return
        }
        this.#botControl = Object.freeze({
          action: "teleport",
          identity: bot.identity,
          position: Object.freeze([values[0]!, values[1]!, values[2]!]) as readonly [number, number, number],
          pitchDegrees: values[3]!,
          yawDegrees: values[4]!,
        })
      } else {
        this.#botControl = Object.freeze({ action: "whack", identity: bot.identity })
      }
      this.#output(`${command} queued: ${bot.name}`)
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
    if (command === "ent_fire") {
      if (!this.#client || !this.#snapshot || tokens.length < 1 || tokens.length > 4 || !Number.isFinite(Number(tokens[3] ?? 0))) {
        this.#output("Usage: ent_fire <target> [action] [value] [delay]")
        return
      }
      const generation = this.#generation
      void this.#client.fireEntityInput(generation, tokens[0]!, tokens[1] ?? "Use", tokens[2] ?? "", Number(tokens[3] ?? 0)).then(() => {
        if (generation === this.#generation) this.#output(`Entity input queued: ${tokens.join(" ")}`)
      }, error => {
        if (generation === this.#generation) this.#output(`ent_fire rejected: ${error instanceof Error ? error.message : String(error)}`)
      })
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
    if (command === "map" && tokens.length === 1) {
      if (this.#selectableMaps().includes(tokens[0] as Tf2TargetName) || this.#configuration?.targets.some((target) => target.target === tokens[0])) {
        const target = await this.#preparedTarget(tokens[0]!)
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
      else this.#output(`Usage: map ${this.#configuration ? this.#selectableMaps().join("|") : "<unavailable>"}`)
      return
    }
    this.#output(`Unknown command: ${command}`)
  }

  #selectableMaps(): readonly Tf2TargetName[] {
    return this.#configuration ? tf2SelectableMapNames(this.#configuration, window.location.origin) : []
  }

  async #preparedTarget(identity: string): Promise<BrowserTargetConfiguration> {
    if (!this.#configuration || (!this.#selectableMaps().includes(identity as Tf2TargetName) && !this.#configuration.targets.some((target) => target.target === identity))) {
      throw new Error(`Undeclared map request ${identity}`)
    }
    const ready = this.#configuration.targets.find((candidate) => candidate.target === identity)
    if (ready) return ready
    const response = await fetch(`/__playsrc/prepare-target/${identity}`, {
      method: "POST", cache: "no-store", credentials: "same-origin", redirect: "error",
    })
    if (!response.ok || response.redirected) throw new Error(`Authenticated map preparation failed: ${identity}`)
    const configuration = parseBrowserConfiguration(await response.json(), window.location.origin)
    if (configuration.applicationBuild !== this.#configuration.applicationBuild
      || configuration.defaultTarget !== this.#configuration.defaultTarget
      || configuration.wasm.sha256 !== this.#configuration.wasm.sha256
      || configuration.assetOrigin !== this.#configuration.assetOrigin) {
      throw new Error("Prepared map changed its browser application authority")
    }
    for (const previous of this.#configuration.targets) {
      const current = configuration.targets.find((candidate) => candidate.target === previous.target)
      if (!current || JSON.stringify(current) !== JSON.stringify(previous)) {
        throw new Error("Prepared map changed an existing immutable target authority")
      }
    }
    const bytes = await this.#immutableObjects(configuration.assetOrigin, configuration.catalog, {
      signal: this.#operation.signal,
      priority: "critical",
      onProgress: (loaded, total) => this.#trackBootstrapObject(configuration.catalog.sha256, loaded, total),
    })
    const catalog = parseResourceCatalogBytes(bytes)
    if (catalog.application !== "tf2" || catalog.entries.length !== configuration.targets.length) {
      throw new Error("Prepared map catalog target table differs")
    }
    for (const target of configuration.targets) {
      const entry = selectCatalogTarget(catalog, target.target)
      if (entry.resources.sha256 !== target.objects.resources.sha256 || entry.resources.byteLength !== target.objects.resources.byteLength) {
        throw new Error(`Prepared map catalog ${target.target} descriptor differs`)
      }
    }
    const target = configuration.targets.find((candidate) => candidate.target === identity)
    if (!target) throw new Error(`Prepared map ${identity} is absent from its authenticated catalog`)
    this.#configuration = configuration
    this.#resourceCatalog = catalog
    return target
  }

  async #replaceCatalogMap(): Promise<void> {
    if (!this.#activeTarget) return
    await this.#switchCatalogMap(this.#activeTarget)
  }

  #profileMapResidency(phase: string, resources?: ResourceConfiguration, loaded?: LoadedGame): void {
    const profile = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
    if (!profile) return
    const snapshotGenerations = new Set([this.#generation, ...(resources ? [resources.generation] : [])])
    const entry = {
      at: performance.now(), phase,
      ...mapResidency(
        this.#loaded && { ...this.#loaded, sections: this.#dependencies?.sections },
        resources && { ...loaded, generation: resources.generation, sections: resources.sections },
      ),
      audioDecodedSampleBytes: [...this.#audioBuffers.values()].reduce((total, buffer) => total + buffer.length * buffer.numberOfChannels * 2, 0),
      canonicalSnapshotBaselineBytes: [...snapshotGenerations].reduce((total, generation) =>
        total + (this.#client?.snapshotMetrics(generation)?.retainedBaselineBytes ?? 0), 0),
      vguiFontFaces: document.fonts.size,
    }
    ;((profile.mapResidency ??= []) as unknown[]).push(entry)
    profile.mapResidencyPhase = phase
  }

  async #switchCatalogMap(target: BrowserTargetConfiguration): Promise<void> {
    if (!this.#configuration || !this.#resourceCatalog) return
    if (!this.#client || !this.#renderer || !this.#loaded) {
      const transition = this.#gameUi?.dispatch({ kind: "map", mapIdentity: target.target })
      if (transition?.disposition !== "applied") throw new Error(`map rejected: ${transition?.reason ?? "GameUI unavailable"}`)
      return
    }
    const operation = this.#nextOperation()
    const previous = this.#catalogReplacement
    let complete!: () => void
    const retiring = new Promise<void>((resolve) => { complete = resolve })
    this.#catalogReplacement = retiring
    try {
      // Supersede immediately, but let the previous candidate retire before borrowing its
      // active source owner or staging another world. There is only one pending world.
      await previous
      if (!this.#operations.current(operation)) return
      await this.#switchCatalogMapOperation(target, operation)
    } finally {
      complete()
      if (this.#catalogReplacement === retiring) this.#catalogReplacement = undefined
    }
  }

  async #switchCatalogMapOperation(target: BrowserTargetConfiguration, operation: ApplicationOperation): Promise<void> {
    this.#requireOperation(operation)
    if (!this.#configuration || !this.#resourceCatalog || !this.#client || !this.#renderer || !this.#loaded) return
    const previousGeneration = this.#generation
    const previousBlockers = new Set(this.#blockers)
    let resourceGeneration: number | undefined
    this.#profileMapResidency("acquire")
    try {
      const signal = operation.signal
      this.#loadingTarget = target
      this.#beginLoadingPresentation()
      this.#set({ phase: "Replacing", detail: `Loading ${target.target} through exact catalog identity`, loadingBackground: this.#loadingBackground?.disposition })
      const selected = selectCatalogTarget(this.#resourceCatalog, target.target)
      if (selected.resources.sha256 !== target.objects.resources.sha256 || selected.resources.byteLength !== target.objects.resources.byteLength) throw new Error("Target resource root differs from catalog")
      const [bytes, graphBytes] = await Promise.all([
        this.#acquireObject(target.objects.bsp, signal, "critical"),
        this.#acquireObject(target.objects.resources, signal, "critical"),
      ])
      this.#requireOperation(operation)
      const graph = parseResourceGraphBytes(graphBytes)
      if (graph.target !== target.target || graph.contentBuild !== target.contentBuild) throw new Error("Target resource graph identity differs")
      const entries = new Map<string, Uint8Array>()
      const dependencies = await this.#decodeResourceSet(graph, ["gameplay"], entries, signal)
      if (!dependencies) throw new Error("Gameplay resource configuration is unavailable")
      resourceGeneration = dependencies.generation
      this.#profileMapResidency("sources-admitted", dependencies)
      this.#requireOperation(operation)
      await this.#replace(bytes, target.objects.bsp.sha256, target.target, { target, graph, dependencies, entries }, operation)
      this.#profileMapResidency("replacement-complete")
      if (this.#operations.current(operation)) this.#loadingTarget = undefined
    } catch (error) {
      if (resourceGeneration !== undefined && resourceGeneration !== this.#generation) {
        await this.#client?.discard(resourceGeneration).catch(() => undefined)
      }
      this.#profileMapResidency("candidate-discarded")
      if (!this.#operations.current(operation)) return
      this.#loadingTarget = undefined
      const reason=error instanceof Error?`${error.name}: ${error.message}`:String(error)
      this.#output(`ERROR: Map replacement failed: ${reason}`)
      if (this.#generation !== previousGeneration) {
        this.#paused = true
        this.#set({ phase: "Failed", gameUi: "failure", detail: `Activated map authority failed: ${reason}` })
        return
      }
      this.#blockers.clear()
      for (const blocker of previousBlockers) this.#blockers.add(blocker)
      this.#paused = document.hidden
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
    candidate?: Readonly<{ target: BrowserTargetConfiguration; graph: ResourceGraph; dependencies: ResourceConfiguration; entries: Map<string, Uint8Array> }>,
    operation?: ApplicationOperation,
  ): Promise<void> {
    const replaceStarted=performance.now();let replacePhase=replaceStarted;const replaceTimings:Record<string,number>={};const finishReplacePhase=(phase:string)=>{const now=performance.now();replaceTimings[phase]=now-replacePhase;replacePhase=now}
    if (!this.#client || !this.#renderer || !this.#loaded || !this.#dependencies) throw new Error("Application is not ready")
    const preparationTeam = this.#snapshot?.team
    this.#paused = true
    this.#audio?.reset()
    this.#audioWorld?.reset()
    this.#suspendAudio()
    this.#neutral()
    this.#predictedEye.suspend()
    this.#simulationSamples.clear()
    await this.#simulationTask
    await this.#presentationTask
    this.#pendingPresentation=undefined
    this.#preparedPresentation=undefined
    this.#requiredParticleDisplayFrames.reset()
    await Promise.all([this.#displayTask, this.#classSelectionRenderTask, this.#teamSelectionRenderTask, this.#equipmentRenderTask])
    await this.#equipmentAdmissionTask?.catch(error => { if (error?.name !== "AbortError") throw error })
    if (operation) this.#requireOperation(operation)
    const generation = candidate?.dependencies.generation ?? this.#reserveGeneration()
    const profile = this.#renderLevel === 2 ? 1 : 0
    const key = await mapDerivedKey(
      bspSha256,
      profile,
      this.#renderLevel,
      this.#configuration?.wasm.sha256 ?? "",
      candidate?.target.objects.resources.sha256 ?? this.#activeTarget?.objects.resources.sha256 ?? "",
    )
    finishReplacePhase("derivedKey")
    const loaded = await this.#client.stage(generation, bytes, profile, candidate?.dependencies ?? this.#dependencies, key)
    // The active renderer already verified this immutable byte identity. Keep
    // its exact backing owner rather than retain two identical map payloads.
    const samePresentation = loaded.presentationKey === this.#loaded.presentationKey
    const staged = Object.freeze({ ...loaded,
      payload: loaded.payloadSha256 === this.#loaded.payloadSha256 ? this.#loaded.payload : loaded.payload,
      presentation: samePresentation ? this.#loaded.presentation : loaded.presentation,
    })
    this.#profileMapResidency("compiled", candidate?.dependencies, staged)
    if (operation) this.#requireOperation(operation)
    const coverageSamples=await this.#client.coverage(generation)
    if (operation) this.#requireOperation(operation)
    finishReplacePhase("stage")
    let artifacts = samePresentation && this.#mapArtifacts ? this.#mapArtifacts
      : await parsePresentationArtifacts(staged.presentation, candidate?.entries ?? this.#dependencyEntries)
    this.#profileMapResidency("presentation-parsed", candidate?.dependencies, staged)
    finishReplacePhase("presentationParse")
    this.#resetMapBlockers()
    this.#recordVisualOutputBlockers(artifacts)
    await this.#cacheModelArtifacts(artifacts)
    finishReplacePhase("modelCache")
    const prior = this.#loaded
    const priorArtifacts = this.#artifacts
    const priorConfiguration = this.#renderer.configuration
    const priorSampleCount = this.#renderer.sampleCount
    const priorTextureQuality = this.#renderer.textureQuality
    let persistence!:Awaited<LoadedGame["persistence"]>
    let rendererChanged = false
    try {
      // Finish the bounded hash/Blob transaction before allocating GPU admission
      // buffers. The active scene remains available throughout this candidate gate.
      persistence = await staged.persistence
      finishReplacePhase("persistence")
      this.#profileMapResidency("cache-write-complete", candidate?.dependencies, staged)
      if (operation) this.#requireOperation(operation)
      if (this.#renderer.configuration.lightingProfile !== (this.#renderLevel === 2 ? "hdr" : "ldr")
        || this.#renderer.sampleCount !== (this.#videoConfiguration.antialias === 4 ? 4 : 1)
        || this.#renderer.textureQuality?.mipOffset !== this.#videoConfiguration.picmip
        || this.#renderer.textureQuality?.trilinear !== (this.#videoConfiguration.trilinear === 1)
        || this.#renderer.textureQuality?.anisotropy !== this.#videoConfiguration.anisotropy) {
        rendererChanged = true
        await this.#renderer.dispose()
        this.#renderer = await createRenderer({
          serviceAudio: this.#serviceAudio,
          canvas: this.#canvas,
          configuration: { ...(this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR), alphaMode: "premultiplied" },
          powerPreference: "high-performance",
          sampleCount: this.#videoConfiguration.antialias === 4 ? 4 : 1,
          textureQuality: { mipOffset: this.#videoConfiguration.picmip, trilinear: this.#videoConfiguration.trilinear === 1, anisotropy: this.#videoConfiguration.anisotropy },
        })
        this.#resizeRenderer()
      }
      rendererChanged = true
      const scene = await this.#renderer.loadMap({
        payload: staged.payload,
        resourceIdentity: (candidate?.dependencies ?? this.#dependencies).sha256,
        payloadSha256: staged.payloadSha256,
        ...mapRendererInputs(artifacts),
        materialStates: this.#materialStates(artifacts),
      modelFacing: this.#modelFacing(artifacts),
        diagnostic: true,
      })
      finishReplacePhase("rendererLoadMap")
      this.#profileMapResidency("gpu-admitted", candidate?.dependencies, staged)
      this.#environmentDrawables = scene.environmentDrawables
      this.#canvas.dataset.staticProps=JSON.stringify(scene.staticProps)
      this.#canvas.dataset.runtimeStaticProps=JSON.stringify(scene.runtimeStaticProps)
      this.#publishProfileDisplacements(scene)
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`${diagnostic.code}: ${diagnostic.identity} — ${diagnostic.detail}`)
      }
      if (operation) this.#requireOperation(operation)
      await this.#client.activate(generation)
      await this.#releaseEquipmentAdmissions(generation)
      finishReplacePhase("activation")
      this.#profileMapResidency("activated-before-prior-js-retirement", candidate?.dependencies, staged)
    } catch (error) {
      await this.#client.discard(generation).catch(() => {})
      if (!rendererChanged) throw error
      if (this.#renderer.configuration.lightingProfile !== priorConfiguration.lightingProfile
        || this.#renderer.sampleCount !== priorSampleCount
        || this.#renderer.textureQuality !== priorTextureQuality) {
        await this.#renderer.dispose().catch(() => {})
        this.#renderer = await createRenderer({
          serviceAudio: this.#serviceAudio,
          canvas: this.#canvas,
          configuration: priorConfiguration,
          powerPreference: "high-performance",
          sampleCount: priorSampleCount,
          textureQuality: priorTextureQuality,
        })
        this.#resizeRenderer()
      }
      await this.#renderer.loadMap({
        payload: prior.payload,
        resourceIdentity: this.#dependencies.sha256,
        payloadSha256: prior.payloadSha256,
        ...(priorArtifacts?mapRendererInputs(priorArtifacts):{}),
        materialStates: priorArtifacts ? this.#materialStates(priorArtifacts) : undefined,
        modelFacing: priorArtifacts ? this.#modelFacing(priorArtifacts) : undefined,
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
    } else {
      this.#dependencies = Object.freeze({ ...this.#dependencies, generation })
    }
    this.#loaded = staged
    this.#coverageSamples=coverageSamples
    this.#artifacts = artifacts
    this.#mapArtifacts = artifacts
    await this.#admitRestoredEquipment()
    artifacts = this.#artifacts!
    this.#lockerAnimations.clear()
    this.#reloadHistory=[]
    this.#fireTickHistory=[]
    this.#wasmCalls={observe:0,models:0,visibility:0,particles:0,acoustics:0};this.#maximumScheduledSamples=0;this.#maximumPublicationTicks=0;this.#phaseTimings=[0,0,0,0,0]
    this.#attachments.clear()
    this.#projectiles?.dispose()
    this.#projectiles = createProjectilePresentationMapper(
      Object.freeze({
        models: new Set(artifacts.models.keys()),
        systems: this.#particleSystems = new Set(artifacts.particleSystems),
        attachments: this.#attachments,
        attachmentTransforms: this.#attachmentTransforms,
        fireAttachmentTransforms: this.#fireAttachmentTransforms,
        localOwnerIdentity: 1,
      }),
    )
    this.#viewmodels = createViewmodelPresenter(artifacts, this.#equipmentProfile!.state()!.inventory)
    this.#viewmodelClass = undefined
    this.#mapIdentity = name
    this.#applyInitialView(staged)
    this.#snapshot = (await this.#initialPublication(generation)).snapshot
    if (this.#snapshot.objectives && (this.#flagCapturesPerRound !== 3 || this.#flagReturnOnTouch)) {
      this.#objectiveConfiguration = Object.freeze({
        capturesPerRound: this.#flagCapturesPerRound,
        returnOnTouch: this.#flagReturnOnTouch,
      })
    }
    if (this.#pendingLocalMatch?.mapIdentity === name) {
      this.#botConfiguration = admitBotConfiguration(
        this.#pendingLocalMatch.configuration, name, this.#dependencyEntries,
      )
      this.#pendingLocalMatch = undefined
    }
    if (operation) this.#requireOperation(operation)
    this.#predictedEye.reset(this.#snapshot.tick, tf2Camera(this.#snapshot, this.#yaw, this.#pitch).position)
    await this.#prepareGameplayPipelines(preparationTeam, false)
    if (operation) this.#requireOperation(operation)
    this.#resetHudIntegration()
    this.#recordAuthorityBlockers(this.#snapshot)
    this.#crouchHistory = []
    this.#recordCrouch(this.#snapshot)
    this.#modelProbes = await this.#probePlayerModels(artifacts)
    this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(artifacts)
    finishReplacePhase("initialization")
    if (this.#audio) {
      this.#audioBuffers = this.#audio.replace(this.#dependencyEntries)
      this.#audioRegistry = new SoundRegistry(artifacts.audio.documents.map(document => Object.freeze({ logicalPath: document.logicalPath, mode: "base" as const, preload: false, entries: document.entries })))
      this.#audioWorld = new SourceAudioWorld(this.#audioRegistry, { maxActiveVoices: 128 })
    }
    this.#lastRandomAudioProbe = ""
    this.#lastCollisionMoverProbe = ""
    this.#paused = document.hidden
    if (!this.#paused) void this.resumeAudio()
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
    if (this.#snapshot.team !== 2 && this.#snapshot.team !== 3) await this.#showTeamSelection()
    else this.#showClassSelection(true)
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

  #modelFacing(artifacts: Pick<PresentationArtifacts, "models">): ReadonlyMap<string, Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>> {
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
    if(!state||!("origin" in state)||!("scale" in state)||!("area" in state)||!("fog" in state)||!Array.isArray(state.origin)||state.origin.length!==3||!state.origin.every(Number.isFinite)||!Number.isSafeInteger(state.scale)||state.scale<0||!Number.isSafeInteger(state.area)||state.area<0||state.area>=255)return null
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

  async #prepareGameplayPipelines(team: number | undefined, prepareVisibleWorld: boolean): Promise<void> {
    selectionTransitionMark("pipeline-start", { team, prepareVisibleWorld })
    const renderer = this.#renderer!, client = this.#client!, artifacts = this.#artifacts!, snapshot = this.#snapshot!
    const generation = this.#generation, camera = tf2Camera(snapshot, this.#yaw, this.#pitch), viewport = this.#viewport()
    const checkOwner = () => {
      if (renderer !== this.#renderer || client !== this.#client || artifacts !== this.#artifacts || generation !== this.#generation) throw new Error("Gameplay pipeline preparation generation was replaced")
    }
    // loadMap already prepares the replacement world. The initial team-camera
    // visibility pass is separate from class/prop resource admission.
    if (prepareVisibleWorld) {
    const visibility = await client.visibility(generation, {
      position: camera.position, yawDegrees: camera.yawDegrees, pitchDegrees: camera.pitchDegrees,
      verticalFovDegrees: camera.verticalFovDegrees, aspectRatio: viewport.width / viewport.height,
      near: camera.near, far: camera.far, presentationTimeSeconds: Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS,
    })
    checkOwner()
    await renderer.prepareVisiblePipelines(camera, visibility.leaves)
    selectionTransitionMark("visible-pipelines-ready")
    checkOwner()
    }
    await renderer.prepareParticlePipelines(camera, this.#mainFog(artifacts))
    selectionTransitionMark("particle-pipelines-ready")
    checkOwner()
    await this.#prepareProjectilePipelines(camera)
    selectionTransitionMark("projectile-pipelines-ready")
    checkOwner()
    // Replacement resets the simulation's team. Keep the admitted presentation
    // team's bounded panel/view variants; world players include both teams.
    const requests = [
      ...classPipelinePoseRequests(artifacts, team === 2 ? 0 : team === 3 ? 1 : null, camera, viewport.width / viewport.height, this.#equipmentProfile!.state()!.inventory),
      ...mapPropPipelinePoseRequests(artifacts, snapshot, camera, viewport.width / viewport.height),
    ]
    if (!requests.length) return
    const byIdentity = new Map(requests.map(value => [value.request.identity, value]))
    selectionTransitionMark("model-poses-request", { requests: requests.length })
    const poses = await client.models(generation, encodeModelPoseBatch(requests.map(value => ({ ...value.request, preparation: true }))))
    selectionTransitionMark("model-poses-ready", { poses: poses.length })
    checkOwner()
    await renderer.prepareModelPipelines(poses.map(pose => {
      const preparation = byIdentity.get(pose.identity), artifact = artifacts.models.get(pose.model)
      const request = preparation?.request
      if (!preparation || !artifact || !request?.lighting || !pose.lighting) throw new Error(`Gameplay pipeline pose unavailable: ${pose.model}`)
      return { pass: preparation.pass, unposedPanel: preparation.pass === "panel" && pose.role === "single", item: {
        identity: pose.identity, model: pose.model, skin: request.skin < artifact.skinCount ? request.skin : 0,
        position: request.lighting.origin, angles: request.lighting.angles, scale: 1, pose, modelLighting: pose.lighting, eyeStates: pose.eyes } }
    }), camera, this.#mainFog(artifacts))
    selectionTransitionMark("pipeline-end")
    checkOwner()
  }

  async #prepareProjectilePipelines(camera: Camera): Promise<void> {
    const renderer = this.#renderer!, artifacts = this.#artifacts!, generation = this.#generation
    const paths = new Set(([1, 2, 3, 4] as const).map(kind => projectileModelPath(kind, false)))
    paths.add(projectileModelPath(1, true))
    const models = [...paths].flatMap(model => {
      const artifact = artifacts.models.get(model)
      if (!artifact) throw new Error(`Projectile pipeline model unavailable: ${model}`)
      return Array.from({ length: artifact.skinCount }, (_, skin) => ({ pass: "world" as const, item: {
        identity: 0, model, skin, position: camera.position, angles: [0, 0, 0] as const, scale: 1,
      } }))
    })
    await renderer.prepareModelPipelines(models, camera, this.#mainFog(artifacts))
    if (renderer !== this.#renderer || artifacts !== this.#artifacts || generation !== this.#generation) {
      throw new Error("Projectile pipeline preparation generation was replaced")
    }
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
    const posed = await this.#client.models(this.#generation, encodeModelPoseBatch(requests))
    return Object.freeze(posed.map((model) => Object.freeze({
      model: model.model,
      sequence: model.sequence,
      primitives: model.primitives.length,
      vertices: model.primitives.reduce((total, primitive) => total + primitive.vertexCount, 0),
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
    const poses = await this.#client.models(this.#generation, encodeModelPoseBatch(requests))
    return activities.map((activity) => {
      const parts = poses.filter((pose) => pose.activity === activity)
      if(parts.length!==2||parts[0]?.role!=="item"||parts[1]?.role!=="hand")throw new Error(`Viewmodel timeline composition ${activity} differs`);const hand=parts[1]!;return `${activity}:${hand.sequence}:${hand.cycle}:${parts.map(part=>part.primitives.length).join("+")}:${hand.events.length}:item>hand`
    })
  }

  readonly #serviceAudio = (): void => {
    if (!this.#paused && this.#audioRunning) this.#audio?.extraUpdate()
  }

  #audioFrame(snapshot: Snapshot, camera: Camera, hostTime: number) {
    if (!this.#audioRunning || !this.#audio || this.#paused || this.#closed) return undefined
    const yaw = camera.yawDegrees * Math.PI / 180, pitch = camera.pitchDegrees * Math.PI / 180
    const frame = this.#audio.frame(snapshot.soundscape, {
      identity: 1, revision: Number(snapshot.tick), origin: camera.position,
      forward: [Math.cos(pitch) * Math.cos(yaw), Math.cos(pitch) * Math.sin(yaw), -Math.sin(pitch)],
      right: [Math.sin(yaw), -Math.cos(yaw), 0], masterGain: 1, categoryGain: 1, muted: this.#masterMuted,
    }, Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS, hostTime, this.#masterMuted ? 0 : this.#effectVolume, [
      { domain: 1, identity: 1, origin: snapshot.position },
      ...snapshot.bots.map(bot => ({ domain: 1 as const, identity: bot.identity, origin: bot.position })),
      ...(snapshot.controlPoints?.points ?? []).map(point => ({ domain: 2 as const, identity: point.identity, origin: point.position })),
      ...snapshot.buildings.map(building => ({ domain: 2 as const, identity: building.identity, origin: building.position })),
      ...snapshot.pickups.map(pickup => ({ domain: 2 as const, identity: pickup.identity, origin: pickup.origin })),
    ])
    if (frame) this.#wasmCalls.acoustics++
    return frame
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
      if (request.action === "stop") {
        for (const identity of this.#audioWorld.stopDefinition(request.source.identity, request.definition)) this.#audio.stop(identity)
        continue
      }
      const definition = this.#audioRegistry.get(request.definition)
      const resource = definition?.waves[request.samples.wave]?.resource
      // A failed Source wave precache does not invalidate its script handle or
      // the map. Preserve the selected event/random sample; never invent audio.
      if (resource && this.#artifacts.audio.unavailable.has(resource)) continue
      const buffer = resource ? this.#audioBuffers.get(resource) : undefined
      if (!resource || !buffer) throw new Error(`Audio resource for ${request.definition} is missing`)
      const patch = this.#artifacts.audio.patches.get(resource)
      if ((request.action === "fade-in" || request.action === "fade-out") && !patch) throw new Error(`Sound patch metadata for ${request.definition} is missing`)
      let started: ReturnType<SourceAudioWorld["start"]>
      try {
        started = this.#audioWorld.start({
          voiceIdentity: request.voiceIdentity,
          definition: request.definition,
          source: request.source,
          listener,
          samples: request.samples,
          overrides: request.overrides,
          ...(request.action === "fade-in" || request.action === "fade-out" ? {
            envelope: { from: request.action === "fade-in" ? 0 : 1, to: request.action === "fade-in" ? 1 : 0, seconds: request.fadeSeconds! },
          } : {}),
          resourceDurationSeconds: buffer.duration,
          resourceLoopStartSeconds: buffer.loopStartSeconds,
          resourceChannels: buffer.numberOfChannels,
          resourceAvailable: (identity) => this.#audioBuffers.has(identity),
          scheduledTimeSeconds: this.#audioContext.currentTime,
          delaySeconds: 0,
          mixerGain: this.#artifacts.audio.mixerGain,
          userGain: this.#effectVolume,
          doNotOverwrite: false,
        })
      } catch (error) {
        if (error instanceof SourceAudioError && error.code === "Suppressed") continue
        throw error
      }
      for (const replaced of started.replaced) this.#audio.stop(replaced)
      this.#audio.playNeutral(started.voice, request.source)
      this.#audioStarts.push(`${started.voice.definition}:${started.voice.resource}:${started.voice.channel}:${started.voice.soundLevel}`)
    }
    this.#audioWorld.refreshSpatial(listener, source => {
      if (source.kind !== "entity") return undefined
      if (source.sourceClass === "tf_projectile") return snapshot.projectiles.find(projectile => projectile.identity === source.identity)?.position
      if (source.sourceClass === "tf_weapon" || source.sourceClass === "player" || source.sourceClass === "tf_player") {
        const owner = source.sourceClass === "tf_weapon" ? source.ownerIdentity ?? source.identity : source.identity
        return owner === 1 ? snapshot.position : snapshot.bots.find(bot => bot.identity === owner)?.position
      }
      if (source.sourceClass === "team_control_point") return snapshot.controlPoints?.points.find(point => point.identity === source.identity)?.position
      if (source.sourceClass === "item") return snapshot.pickups.find(pickup => pickup.identity === source.identity)?.origin
      return undefined
    })
  }

  #updateAttachmentTransforms(snapshot: Snapshot, viewmodels: readonly PosedModel[], camera: Camera): void {
    if (!this.#artifacts) return
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
    for (const projectile of snapshot.projectiles) {
      const artifact = this.#artifacts.models.get(
        projectileModelPath(projectile.kind, projectile.miniRocket),
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
        projectileModelPath(event.kind, event.miniRocket),
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
        const model = this.#equipmentProfile!.state()!.inventory.find(item => item.weapon === event.weapon)?.modelPlayer
        if (!model) throw new Error(`Authored TF2 bot launcher definition unavailable: ${event.weapon}`)
        const artifact = this.#artifacts.models.get(model)
        if (!artifact) throw new Error(`Authored TF2 bot launcher model unavailable: ${model}`)
        const transforms = new Map([...artifact.attachments].map(([name, matrix]) => [
          name.toLowerCase(),
          transformAttachment(matrix, launcherPose.eyePosition, launcherPose.viewOrientation),
        ] as const))
        this.#attachments.set(event.launcherIdentity, new Set([
          ...(this.#attachments.get(event.launcherIdentity) ?? []),
          ...transforms.keys(),
        ]))
        this.#attachmentTransforms.set(event.launcherIdentity, new Map([
          ...(this.#attachmentTransforms.get(event.launcherIdentity) ?? []),
          ...transforms,
        ]))
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
      this.#attachments.set(event.launcherIdentity, new Set([
        ...(this.#attachments.get(event.launcherIdentity) ?? []),
        ...transforms.keys(),
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
    const start=(effectIdentity:string,system:string,tick:bigint,colored=false)=>{
      if(!muzzle)throw new Error(`TF2 Pyro authored muzzle attachment unavailable: ${system}`)
      const request: Extract<ProjectileParticleRequest, { kind: "start" }> = Object.freeze({
        kind:"start",identity:`${effectIdentity}:start:${tick}`,effectIdentity,eventIdentity:`${effectIdentity}:${tick}`,
        tick,projectileIdentity:snapshot.weapon!,ownerIdentity,launcherIdentity:snapshot.weapon!,team,system,
        attachment:Object.freeze({entityIdentity:snapshot.weapon!,name:"muzzle" as const}),
        controlPoints:Object.freeze([Object.freeze({index:0 as const,position:muzzle.position,orientation:muzzle.orientation,ownerIdentity})]),
      })
      requests.push(request)
      if (colored) requests.push(...weaponParticleColorRequests(request))
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
    const stopCharge = () => {
      if (!this.#manmelterChargeEffect) return
      const effectIdentity = this.#manmelterChargeEffect
      requests.push(Object.freeze({ kind: "stop", identity: `${effectIdentity}:stop:${snapshot.tick}`, effectIdentity,
        eventIdentity: `${effectIdentity}:stop`, tick: snapshot.tick, projectileIdentity: 98, immediate: false }))
      this.#manmelterChargeEffect = undefined
    }
    const manmelterVisible = snapshot.lifecycle === 1 && snapshot.weapon === 98
    if (!manmelterVisible) stopCharge()
    for (const event of snapshot.events) {
      if (event.kind !== 24) continue
      if (event.subject === 2) { stopCharge(); continue }
      if (!manmelterVisible) continue
      if (event.subject === 1) {
        stopCharge()
        this.#manmelterChargeEffect = `manmelter-charge:${++this.#pyroEffectSerial}`
        start(this.#manmelterChargeEffect, "drg_manmelter_vacuum", snapshot.tick, true)
      } else if (event.subject === 3) {
        start(`manmelter-absorb:${++this.#pyroEffectSerial}`, "drg_manmelter_vacuum_flames", snapshot.tick, true)
      } else if (event.subject === 4) {
        start(`manmelter-idle:${++this.#pyroEffectSerial}`, "drg_bison_idle", snapshot.tick, true)
        start(`manmelter-idle:${++this.#pyroEffectSerial}`, "drg_manmelter_idle", snapshot.tick)
      }
    }
    if (this.#manmelterChargeEffect && muzzle) {
      const effectIdentity = this.#manmelterChargeEffect
      requests.push(Object.freeze({ kind: "set-control-point", identity: `${effectIdentity}:muzzle:${snapshot.tick}`, effectIdentity,
        eventIdentity: `${effectIdentity}:follow`, tick: snapshot.tick, projectileIdentity: 98,
        controlPoint: Object.freeze({ index: 0 as const, position: muzzle.position, orientation: muzzle.orientation, ownerIdentity }) }))
    }
    return requests
  }

  #command(): ArrayBuffer {
    const forward = Number(this.#buttons.held("+forward")) - Number(this.#buttons.held("+back"))
    const side = Number(this.#buttons.held("+moveleft")) - Number(this.#buttons.held("+moveright"))
    const attacking = this.#buttons.held("+attack") || this.#firePressed
    const unsupportedProjectile = this.#snapshot?.class === 4 && attacking && (this.#snapshot.weapon === 18 || this.#snapshot.weapon === 3) && this.#snapshot.authorityBlockers.some((blocker) => blocker.code === 1)
    if (unsupportedProjectile) {
      this.#blockers.add("Complete map rigid-body resources are unavailable")
      this.#set({ unsupportedState:"RigidResourcesUnavailable" })
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
      nextbotStop: this.#nextBotStop,
      selectClass: this.#selectClass,
      selectWeapon: this.#selectWeapon === "last" ? undefined : this.#selectWeapon,
      selectLastWeapon: this.#selectWeapon === "last",
      weaponPreferences: this.#weaponPreferences,
      disguise: this.#disguise,
      modeRequest: this.#modeRequest,
      bot: this.#botRequest,
      botControl: this.#botControl,
      building: this.#buildingRequest,
      botConfiguration: this.#botConfiguration,
      objectiveConfiguration: this.#objectiveConfiguration,
    })
    this.#selectClass = undefined
    this.#selectWeapon = undefined
    this.#disguise = undefined
    this.#modeRequest = undefined
    this.#botRequest = undefined
    this.#botControl = undefined
    this.#buildingRequest = undefined
    this.#botConfiguration = undefined
    this.#objectiveConfiguration = undefined
    this.#jumpPressed = false
    this.#firePressed = false
    this.#detonatePressed = false
    this.#reloadPressed = false
    this.#dropItem = false
    return command
  }

  readonly #frame = (time: number): void => {
    this.#animationFrame = requestAnimationFrame(this.#frame)
    this.#displayBackpressure.advance()
    const frameProfiler = browserFrameProfiler()
    if (frameProfiler?.active) {
      const instrumentation=frameProfiler as typeof frameProfiler & { animationCallbacks?: number[]; compositorFrames?: Record<string,unknown>[] }
      instrumentation.animationCallbacks?.push(time)
      const completed=frameProfiler.completedFrames.at(-1)
      if(completed&&instrumentation.compositorFrames&&instrumentation.compositorFrames.at(-1)?.displayFrame!==completed.displayFrame){
        instrumentation.compositorFrames.push({at:time,displayFrame:completed.displayFrame,tick:completed.tick,mouseRevision:completed.mouseRevision,submittedAt:completed.at,submissionMilliseconds:Math.max(0,time-Number(completed.at))})
      }
    }
    let timeSeconds: number
    try {
      timeSeconds = this.#frameClock.admit(performance.now() / 1_000)
      // Keep the device supplied while an asynchronous GPU submission is still
      // pending. Control/geometry updates remain one coherent audio transaction.
      if (!this.#paused && this.#audioRunning) this.#audio?.pump()
      const owners = visibleFrameOwners(this.#view, !this.#gameUiRoot.hidden)
      if (owners & GAME_UI_FRAME_OWNER) this.#gameUi?.frame(timeSeconds)
      if (owners & LOADING_FRAME_OWNER) this.#loadingVgui?.frame(timeSeconds)
      if (owners & OPTIONS_FRAME_OWNER) this.#options?.frame(timeSeconds)
      this.#hudIntegration?.frame(timeSeconds, Boolean(owners & HUD_FRAME_OWNER))
      if (owners & HUD_FRAME_OWNER) this.#engineer?.frame(timeSeconds)
      if (this.#view.localMatchVisible) this.#localMatch?.frame(timeSeconds)
      if (this.#equipment?.visible()) { this.#equipment.frame(timeSeconds); this.#renderEquipment() }
      if (this.#classSelection?.state().visible) this.#classSelection.frame(timeSeconds)
      if (this.#teamSelection?.state().visible) {
        this.#updateTeamSelection(timeSeconds)
        this.#teamSelection.frame(timeSeconds)
      }
    } catch (error) {
      this.#paused = true
      this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "VGUI frame failed") })
      return
    }
    if (!this.#paused && this.#snapshot && this.#showFps === 0 && this.#showPos !== 0) this.#updateDiagnostics(time)
    if (this.#paused || !this.#client || !this.#renderer) { this.#clientRenderFrames.suspend(); return }
    if (!this.#snapshot) {
      if (this.#teamSelection?.state().visible) this.#renderTeamSelection()
      return
    }
    // Rust owns tick accumulation. A second 15ms browser gate aliases the host
    // clock into zero/two-tick observations and loses visible publications.
    // Observe each real browser frame; the authoritative simulation stays 66.7Hz.
    this.#scheduleSimulation(timeSeconds, false)
    if (this.#classSelection?.state().visible) this.#renderClassSelection()
    else if (this.#teamSelection?.state().visible) this.#renderTeamSelection()
    else this.#offerDisplay()
  }

  #offerDisplay():void{
    if (this.#preparingModelPipelines || this.#equipmentPreparing) return
    if (this.#equipment?.visible() || this.#classSelection?.state().visible || this.#teamSelection?.state().visible) return
    const frameProfiler = browserFrameProfiler()
    if (frameProfiler?.active) frameProfiler.counters.displayOffers! += 1
    const required=this.#requiredParticleDisplayFrames.peek()
    const prepared=required??this.#preparedPresentation
    if(
      this.#displayTask||this.#classSelectionRenderTask||this.#teamSelectionRenderTask||!prepared||this.#closed||this.#paused||
      (!required&&prepared.revision===this.#lastRenderedPreparedRevision&&this.#viewRevision===this.#lastRenderedViewRevision)
    ){
      if(frameProfiler?.active){
        if(this.#displayTask){
          frameProfiler.counters.displayRejectedBusy!+=1
          if(!this.#displayBackpressure.defer())frameProfiler.counters.displayCoalesced!+=1
        }
        else if(prepared&&prepared.revision===this.#lastRenderedPreparedRevision&&this.#viewRevision===this.#lastRenderedViewRevision)frameProfiler.counters.displayRejectedUnchanged!+=1
      }
      else if(this.#displayTask)this.#displayBackpressure.defer()
      return
    }
    if(frameProfiler?.active)frameProfiler.counters.displayStarted!+=1
    this.#displayBackpressure.begin()
    const task=this.#renderDisplay(prepared).then(()=>{
      if(required&&prepared.generation===this.#generation)this.#requiredParticleDisplayFrames.complete(required)
    }).catch((error)=>{
      if(this.#closed||prepared.generation!==this.#generation||this.#view.phase==="Replacing")return
      this.#paused=true
      this.#predictedEye.suspend()
      this.#set({phase:"Failed",gameUi:"failure",detail:this.#failureDetail(error,"Display frame failed")})
    })
    this.#displayTask=task
    void task.finally(()=>{
      if(this.#displayTask!==task)return
      this.#displayTask=undefined
      if(this.#displayBackpressure.complete()&&!this.#closed&&!this.#paused){
        const profiler=browserFrameProfiler()
        if(profiler?.active)profiler.counters.displayRecovered!+=1
        this.#offerDisplay()
      }
    })
  }

  async #renderDisplay(prepared:PreparedPresentation):Promise<void>{
    const client=this.#client,renderer=this.#renderer,generation=this.#generation
    if(!prepared||!client||!renderer||prepared.generation!==generation)return
    const workloadClock = (globalThis as any).__playsrcProfile?.commandWorkload
    const recordedFrame = this.#recordedClientFrames && !this.#recordedClientFrames.ended
      ? await this.#recordedClientFrames.next(workloadClock?.epoch) : undefined
    const clientNow = recordedFrame?.nowSeconds ?? (performance.now() - (this.#recordedClientFrames ? workloadClock?.epoch ?? 0 : 0)) / 1000
    const clientFrame = this.#clientRenderFrames.prepare(clientNow)
    if (recordedFrame && (clientFrame.clientFrame !== recordedFrame.clientFrame || clientFrame.acceptedClientFrame !== recordedFrame.acceptedClientFrame
      || clientFrame.clientFrameSeconds !== recordedFrame.clientFrameSeconds || clientFrame.reset !== recordedFrame.reset)) throw new Error("Client-frame clock input differs from authenticated workload")
    const viewRevision=this.#viewRevision,mouseRevision=this.#mouseViewRevision,snapRevision=this.#authoritativeViewRevision,yaw=this.#yaw,pitch=this.#pitch
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    const override=profile?.displacementCameraOverride as Partial<Camera>|undefined
    const geometryEvidenceRevision=profile?.geometryEvidenceRevision
    const authorityCamera=tf2Camera(prepared.snapshot,yaw,pitch)
    const ordinaryCamera=Object.freeze({...authorityCamera,position:prepared.presentedCamera.main.position})
    const selectedCamera=selectPresentedCamera(ordinaryCamera,override)
    const viewport=this.#viewport()
    const phaseStart=performance.now(),visibilityStart=phaseStart
    const presentationTimeSeconds = Number(prepared.snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS
    const aspectRatio = viewport.width / viewport.height
    const skyController = this.#artifacts ? this.#skyController(this.#artifacts) : null
    const presentedCamera = presentCamera(selectedCamera, {
      generation, viewportRevision: viewport.revision, preparedRevision: prepared.revision,
      viewRevision, mouseRevision, snapRevision, tick: prepared.snapshot.tick,
    }, skyController)
    const camera = presentedCamera.main
    const audioFrame = this.#audioFrame(prepared.snapshot, camera, phaseStart / 1000)
    let visibility = prepared.visibility
    let skyVisibility = prepared.skyVisibility
    const viewChanged = !equivalentPresentedVisibility(prepared.presentedCamera, presentedCamera)
    if (viewChanged) {
      this.#wasmCalls.visibility += 1
      const mainView = {
        position: camera.position,
        yawDegrees: camera.yawDegrees,
        pitchDegrees: camera.pitchDegrees,
        verticalFovDegrees: camera.verticalFovDegrees,
        aspectRatio,
        near: camera.near,
        far: camera.far,
        presentationTimeSeconds,
      }
      const controller = presentedCamera.controller, skyCamera = presentedCamera.sky
      const views = controller && skyCamera ? [mainView, {
        position: skyCamera.position, visibilityPosition: controller.origin, areaFilter: controller.area,
        yawDegrees: skyCamera.yawDegrees, pitchDegrees: skyCamera.pitchDegrees,
        verticalFovDegrees: skyCamera.verticalFovDegrees, aspectRatio,
        near: skyCamera.near, far: skyCamera.far, presentationTimeSeconds,
      }] : [mainView]
      ;[visibility, skyVisibility] = await client.visibilityViews(generation, views, audioFrame)
    }
    else if (audioFrame) audioFrame.accept(await client.acoustics(generation, audioFrame.input))
    let sky3d: Frame["sky3d"]
    if (selectAuthoredSky(visibility.sky, skyController !== null)) {
      if (!skyController || !presentedCamera.sky || !skyVisibility) throw new Error("Authored 3D-sky visibility is unavailable")
      if (skyVisibility.areas.some(area => area !== skyController.area)) throw new Error("3D-sky visibility escaped its authored area")
      sky3d = Object.freeze({
        camera: presentedCamera.sky,
        visibility: Object.freeze({ ...skyVisibility, surfaces: skyVisibility.drawSurfaces }),
        fog: skyController.fog,
      })
    }
    const visibilityMilliseconds=performance.now()-visibilityStart
    if(this.#closed||this.#paused||renderer!==this.#renderer||this.#classSelection?.state().visible||this.#teamSelection?.state().visible||!currentPresentedCamera(presentedCamera,{
      generation:this.#generation,viewportRevision:this.#presentationViewport?.revision??-1,
      viewRevision:this.#viewRevision,mouseRevision:this.#mouseViewRevision,snapRevision:this.#authoritativeViewRevision,
    })){
      const frameProfiler=browserFrameProfiler()
      if(frameProfiler?.active)frameProfiler.counters.displayAbandoned!+=1
      return
    }
    const deltaTicks=this.#lastRenderedTick===undefined
      ? prepared.publication.selectedTicks
      : prepared.snapshot.tick>=this.#lastRenderedTick
        ? Number(prepared.snapshot.tick-this.#lastRenderedTick)
        : prepared.publication.selectedTicks
    const models=prepared.frame.models
    let particles = prepared.frame.particles
    let legacyVisuals:Frame["legacyVisuals"]
    let legacyParticleMilliseconds = 0, legacyParticleBytes = 0
    if (this.#loaded?.legacyParticleFrames && this.#artifacts) {
      const started = performance.now()
      this.#wasmCalls.particles++
      const hasVisuals=this.#artifacts.legacyVisualTextures.length>0
      const visualViews:LegacyVisualView[]=[]
      if(hasVisuals){
        if(sky3d&&skyController)visualViews.push({...sky3d.camera,kind:1,aspectRatio,presentationTimeSeconds,visibilityPosition:skyController.origin,viewportHeight:this.#canvas.height,pixelVisibility:renderer.pixelVisibilityFeedback(1)})
        if(this.#artifacts.legacyVisualTextures.some(texture=>texture.program.worldRenderable)){
          for(const pass of visibility.water.passes){
            if(!pass.drawEntities)continue
            const kind=pass.kind==="reflection"?2:pass.kind==="refraction"?3:pass.kind==="intersection"?4:0
            visualViews.push({...camera,kind,position:pass.origin,yawDegrees:pass.angles[1],pitchDegrees:pass.angles[0],aspectRatio,presentationTimeSeconds,viewportHeight:renderer.legacyVisualViewport(kind).height,pixelVisibility:renderer.pixelVisibilityFeedback(kind)})
          }
        }
        if(!visualViews.some(view=>view.kind===0))visualViews.push({...camera,kind:0,aspectRatio,presentationTimeSeconds,viewportHeight:this.#canvas.height,pixelVisibility:renderer.pixelVisibilityFeedback(0)})
      }
      const visualPayload=hasVisuals?encodeLegacyVisualQuery(visualViews,{screenWidth:renderer.legacyVisualViewport(0).width,samples:renderer.sampleCount}):new Uint8Array()
      if(profile?.legacyVisualProbe)profile.legacyVisualViews=visualViews
      const output = await client.legacyFrame(generation, encodeLegacyParticleFrame(presentationTimeSeconds, clientFrame, { ...camera, aspectRatio },visualPayload))
      if (this.#closed || this.#paused || generation !== this.#generation || renderer !== this.#renderer || this.#classSelection?.state().visible || this.#teamSelection?.state().visible
        || !currentPresentedCamera(presentedCamera,{generation:this.#generation,viewportRevision:this.#presentationViewport?.revision??-1,viewRevision:this.#viewRevision,mouseRevision:this.#mouseViewRevision,snapRevision:this.#authoritativeViewRevision})) return
      particles = [...(particles ?? []), ...decodeParticleRenderOutput(output.particles, this.#artifacts.particleMaterials).items]
      if(hasVisuals)legacyVisuals=decodeLegacyVisualViews(output.visuals)
      else if(output.visuals.byteLength)throw new Error("Unexpected legacy visual output")
      legacyParticleBytes = output.particles.byteLength + output.visuals.byteLength
      legacyParticleMilliseconds = performance.now() - started
    }
    const renderStart=performance.now()
    const hudPixelRevision = profile?.hudPixelEvidenceRevision
    const captureHudPixels = Number.isSafeInteger(hudPixelRevision) && hudPixelRevision !== (profile?.hudPixelEvidence as { revision?: number } | undefined)?.revision
    const cosmeticDepthRevision = profile?.cosmeticDepthRevision
    const captureCosmeticDepth = Number.isSafeInteger(cosmeticDepthRevision) && cosmeticDepthRevision !== (profile?.cosmeticDepthCapture as { revision?: number } | undefined)?.revision
    if (captureCosmeticDepth) renderer.requestParticleDepthEvidence()
    if(profile?.captureFrameAdmission) {
      const counts={effects:prepared.frame.effects.length,shadows:prepared.frame.shadows?.length??0,models:models?.length??0,particles:particles?.length??0,brushes:prepared.frame.brushModels?.models.length??0}
      const total=Object.values(counts).reduce((sum,count)=>sum+count,0)
      const old=profile.frameAdmission as {total:number;generation:number}|undefined
      if(!old||generation!==old.generation||total>old.total) {
        const materials=new Map<string,{count:number;minimumAge:number;maximumAge:number}>()
        for(const item of particles??[]) {
          const entry=materials.get(item.material)??{count:0,minimumAge:Infinity,maximumAge:-Infinity}
          entry.count++;entry.minimumAge=Math.min(entry.minimumAge,item.ageSeconds);entry.maximumAge=Math.max(entry.maximumAge,item.ageSeconds);materials.set(item.material,entry)
        }
        profile.frameAdmission={generation,total,counts,tick:prepared.snapshot.tick.toString(),selectedTicks:prepared.publication.selectedTicks,deltaTicks,camera,materials:[...materials].sort((a,b)=>b[1].count-a[1].count).slice(0,32)}
      }
    }
    let rendered
    try { rendered=await renderer.render({
      ...prepared.frame,
      particles,
      legacyVisuals,
      clientFrame: clientFrame.clientFrame,
      clientFrameSeconds: clientFrame.clientFrameSeconds,
      hudMaterials:this.#hudIntegration?.materialFrame(),
      ...(captureHudPixels ? { capture: { format: "image/png" as const, beforeHud: true } } : {}),
      models,
      camera,
      visibility:Object.freeze({...visibility,surfaces:visibility.drawSurfaces}),
      fog:this.#artifacts?this.#mainFog(this.#artifacts):undefined,
      sky3d,
      deltaSeconds:deltaTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,
    }) } catch (error) {
      throw new Error(`${String(error)}; frame producer=${JSON.stringify({
        generation,currentGeneration:this.#generation,closed:this.#closed,paused:this.#paused,sameRenderer:renderer===this.#renderer,
        snapshotTick:prepared.snapshot.tick.toString(),lastRenderedTick:this.#lastRenderedTick?.toString(),selectedTicks:prepared.publication.selectedTicks,deltaTicks,
        viewRevision:this.#viewRevision,mouseRevision:this.#mouseViewRevision,snapRevision:this.#authoritativeViewRevision,
        presented:{...presentedCamera.revisions,tick:presentedCamera.revisions.tick.toString()},roundState:prepared.snapshot.round.state,inSetup:prepared.snapshot.round.inSetup,waiting:prepared.snapshot.round.waitingForPlayers,peak:profile?.frameAdmission,
      })}`,{cause:error})
    }
    clientFrame.accept()
    if (recordedFrame) this.#recordedClientFrames!.accept(recordedFrame.clientFrame)
    if(this.#closed||this.#paused||generation!==this.#generation||renderer!==this.#renderer)return
    if (captureHudPixels && profile) profile.hudPixelEvidence = { revision: hudPixelRevision, before: rendered.beforeHudCapture, after: rendered.capture }
    const hudModel=this.#hudIntegration?.modelPanel()
    let hudModelMilliseconds=0
    if(hudModel){
      const hudPoses = prepared.hudModelPoses.filter(pose => pose.model === hudModel.model || pose.role !== "single")
      const playerPose = hudPoses.find(pose => pose.role === "single" && pose.model === hudModel.model)
      const result=await renderer.renderModelPanels([Object.freeze({
        identity:"classmodelpanel",model:hudModel.model,skin:hudModel.skin,
        kind:"studio" as const,fov:hudModel.fov,origin:hudModel.origin,angles:hudModel.angles,
        bounds:hudModel.bounds,background:"transparent" as const,presentationTimeSeconds,
        ...(playerPose ? { pose: playerPose, modelLighting: playerPose.lighting ?? undefined, eyeStates: playerPose.eyes,
          mergedModels: hudPoses.filter(pose => pose.role === "item" || pose.role === "wearable").map(pose => ({ model: pose.model, skin: hudModel.skin < (this.#artifacts!.models.get(pose.model)?.skinCount ?? 0) ? hudModel.skin : 0, pose, modelLighting: pose.lighting ?? undefined, eyeStates: pose.eyes })),
          particles: hudPoses.flatMap(pose => pose.wearable?.particleBytes.byteLength ? decodeParticleRenderOutput(pose.wearable.particleBytes, this.#artifacts!.particleMaterials).items : []),
        } : {}),
      })])
      hudModelMilliseconds=result.milliseconds
      if (profile) profile.hudModelPanel = { ...hudModel, panels: result.panels }
    }
    const rendererProfile=renderer.completeFrameProfile()
    if (captureCosmeticDepth && profile) {
      const pixels = this.#canvas.toDataURL("image/png")
      const buffers = await renderer.readParticleDepthEvidence()
      if (buffers) profile.cosmeticDepthCapture = { revision: cosmeticDepthRevision, camera, particles,
        tick: prepared.snapshot.tick.toString(), pixels, buffers }
    }
    if (profile?.cloakSampleActive) profile.cloakSampleCopies = Number(profile.cloakSampleCopies ?? 0) + rendered.timings.cloakFramebufferCopies
    const cloakCaptureCondition = profile?.cloakCaptureCondition as { identity: number; factor: number } | undefined
    if (profile && typeof profile.cloakCaptureRevision === "number" && profile.cloakCaptureRevision !== (profile.cloakCapture as { revision?: number } | undefined)?.revision
      && (!cloakCaptureCondition || models?.some(model => model.identity === cloakCaptureCondition.identity && model.pose?.cloak?.worldFactor === cloakCaptureCondition.factor))) {
      profile.cloakCapture = { revision: profile.cloakCaptureRevision, tick: prepared.snapshot.tick.toString(), spy: prepared.snapshot.spy,
        camera, generation, lightingProfile: renderer.configuration.lightingProfile, copies: rendered.timings.cloakFramebufferCopies,
        players: prepared.snapshot.scoreboard.players,
        bots: prepared.snapshot.bots.map(bot => ({ identity: bot.identity, team: bot.team, class: bot.class, position: bot.position, yawDegrees: bot.yawDegrees, health: bot.health, lifecycle: bot.lifecycle })),
        models: models?.map(model => ({ identity: model.identity, model: model.model, cloak: model.pose?.cloak, viewModel: model.viewModel })),
        viewGeometry: renderer.captureViewModelEvidence(camera), worldGeometry: renderer.captureWorldModelEvidence(camera),
        pixels: this.#canvas.toDataURL("image/png") }
    }
    const renderMilliseconds=performance.now()-renderStart,totalMilliseconds=performance.now()-phaseStart
    if (this.#closed || this.#paused || generation !== this.#generation || renderer !== this.#renderer) return
    this.#canvasDiagnostics.publish(this.#canvas, rendered)
    const skyDisposition=sky3d?"authored":visibility.sky===2?"controller-absent":"not-visible"
    if(this.#canvas.dataset.skyVisibilityDisposition!==skyDisposition)this.#canvas.dataset.skyVisibilityDisposition=skyDisposition
    if(profile){
      profile.videoQuality=Object.freeze({
        ...this.#videoConfiguration,
        sampleCount:renderer.sampleCount,
        lightingProfile:renderer.configuration.lightingProfile,
        presentationSynchronizationSupported:renderer.capabilities.presentationSynchronization,
        waterPasses:rendered.waterPasses,
      })
      if(skyDisposition==="controller-absent")profile.controllerFreeSkyViews=Number(profile.controllerFreeSkyViews??0)+1
      profile.displacementVisibility={surfaces:[...visibility.surfaces],drawSurfaces:[...visibility.drawSurfaces],outsideWorld:visibility.outsideWorld,eyeLeaf:visibility.eyeLeaf,leaves:visibility.leaves,areas:visibility.areas};profile.displacementCamera=camera
      profile.bots=prepared.snapshot.bots.map(bot=>({...bot,weapon:bot.weapon&&{...bot.weapon,nextPrimaryTick:bot.weapon.nextPrimaryTick.toString(),nextReloadTick:bot.weapon.nextReloadTick.toString()},lastFireTick:bot.lastFireTick?.toString()??null,respawnTick:bot.respawnTick?.toString()??null,tick:prepared.snapshot.tick.toString()}))
      profile.round=prepared.snapshot.round
      profile.controlPoints=prepared.snapshot.controlPoints
      profile.soundscape=prepared.snapshot.soundscape
      profile.player={team:prepared.snapshot.team,class:prepared.snapshot.class,position:prepared.snapshot.position,viewAngleOffset:prepared.snapshot.viewAngleOffset,camera}
      if (profile.captureSelectionTransitions === true) selectionTransitionDraw({ scene: "world", generation, team: prepared.snapshot.team, class: prepared.snapshot.class, lifecycle: prepared.snapshot.lifecycle })
      profile.combat={tick:prepared.snapshot.tick.toString(),health:prepared.snapshot.health,lifecycle:prepared.snapshot.lifecycle,
        scores:prepared.snapshot.scoreboard.players.map(player=>({...player,killstreak:player.kills,
          respawnTick:prepared.snapshot.bots.find(bot=>bot.identity===player.identity)?.respawnTick?.toString()??null}))}
      if (profile.captureMelee === true) {
        if (profile.meleeGeneration !== generation) { profile.meleeGeneration = generation; profile.meleeTimeline = []; profile.meleeRecordedTick = "-1" }
        const timeline = profile.meleeTimeline as unknown[]
        let recorded = BigInt(String(profile.meleeRecordedTick))
        for (const batch of prepared.publication.eventBatches) if (batch.snapshot.tick > recorded) {
          for (const event of batch.snapshot.events) if ([14, 15, 17, 18, 19].includes(event.kind) || profile.captureHitscan && (event.kind === 12 || event.kind === 13) || profile.captureDamageIndicators && event.kind === 6) timeline.push({ tick: batch.snapshot.tick.toString(), ...event })
          recorded = batch.snapshot.tick
        }
        profile.meleeRecordedTick = recorded.toString()
        if (timeline.length > 256) timeline.splice(0, timeline.length - 256)
        profile.melee = { tick: prepared.snapshot.tick.toString(), class: prepared.snapshot.class, weapon: prepared.snapshot.weapon, health: prepared.snapshot.health,
          lifecycle: prepared.snapshot.lifecycle, position: prepared.snapshot.position, velocity: prepared.snapshot.velocity,
          conditions: prepared.snapshot.conditions, equipment: prepared.snapshot.equippedItems,
          overlay: visibility.screenOverlay && { identity: visibility.screenOverlay.identity, frame: visibility.screenOverlay.normalFrame, tint: visibility.screenOverlay.refractTint },
          bots: prepared.snapshot.bots.map(bot => ({ identity: bot.identity, class: bot.class, team: bot.team, health: bot.health, conditions: bot.conditions, position: bot.position })) }
      }
      profile.pickups=prepared.snapshot.pickups.map(pickup=>({...pickup,respawnTick:pickup.respawnTick?.toString()??null}))
      profile.buildings=prepared.snapshot.buildings.map(building=>({...building,startedTick:building.startedTick.toString(),rechargeEndTick:building.rechargeEndTick?.toString()??null,tick:prepared.snapshot.tick.toString()}))
      profile.placement=prepared.snapshot.placement
      profile.objectives=prepared.snapshot.objectives?.flags.map(flag=>({identity:flag.identity,team:flag.team,position:flag.position}))??[]
    }
    if(profile&&Number.isSafeInteger(geometryEvidenceRevision)&&geometryEvidenceRevision===profile.geometryEvidenceRevision&&geometryEvidenceRevision!==((profile.geometryEvidence as {revision?:unknown}|undefined)?.revision)&&this.#view.phase==="Ready"){
      const skyGeometry=sky3d?renderer.captureGeometryEvidence(sky3d.camera,"sky3d"):null
      profile.geometryEvidence=Object.freeze({revision:geometryEvidenceRevision,generation,target:this.#mapIdentity,finalReady:true,identities:Object.freeze({bsp:this.#activeTarget?.objects.bsp.sha256,resourceRoot:this.#activeTarget?.objects.resources.sha256,contentBuild:this.#resourceGraph?.contentBuild,graphTarget:this.#resourceGraph?.target,wasm:this.#configuration?.wasm.sha256,simulationTick:prepared.snapshot.tick.toString()}),camera,visibility:Object.freeze({outsideWorld:visibility.outsideWorld,eyeLeaf:visibility.eyeLeaf,leaves:Object.freeze([...visibility.leaves]),areas:Object.freeze([...visibility.areas]),pvsSurfaces:Object.freeze([...visibility.surfaces]),drawSurfaces:Object.freeze([...visibility.drawSurfaces])}),skyGeometry,geometry:renderer.captureGeometryEvidence(camera)})
    }
    if (profile?.captureProjectileQueries === true) profile.projectileQueries = renderer.captureParticleVisibilityEvidence()
    if (profile?.captureProjectileGameplay === true) profile.projectileState = {
      pipebombCount: prepared.snapshot.pipebombCount, chargeProgress: prepared.snapshot.chargeProgress,
      tick: prepared.snapshot.tick.toString(), health: prepared.snapshot.health, grounded: prepared.snapshot.grounded,
      position: prepared.snapshot.position, velocity: prepared.snapshot.velocity, conditions: prepared.snapshot.conditions,
      bots: prepared.snapshot.bots.length, ropeItems: particles.filter(item => item.primitive === "rope").length,
      projectileItems: prepared.snapshot.projectiles.map(item => ({ identity: item.identity, kind: item.kind, state:item.state,position:item.position,velocity:item.velocity,orientation:item.orientation,contactNormal:item.contactNormal,trail: item.trail, miniRocket: item.miniRocket, modelVisible: item.modelVisible })),
    }
    if (profile?.captureProjectileHistory === true && !browserFrameProfiler()?.active) {
      const state = profile.projectileState as { grounded: boolean; conditions: number[]; projectileItems: { identity: number; trail: number }[] }
      const key = `${state.grounded}:${state.conditions.join(":")}:${state.projectileItems.map(item => `${item.identity}:${item.trail}`).join(",")}`
      if (profile.projectileHistoryKey !== key) {
        profile.projectileHistoryKey = key
        const history = (profile.projectileHistory ??= []) as unknown[]
        history.push({ at: performance.now(), ...state }); if (history.length > 64) history.shift()
      }
    }
    if (profile?.legacyVisualProbe) profile.legacyVisualEvidence=renderer.captureLegacyVisualEvidence()
    if (profile && Number.isSafeInteger(profile.particleEvidenceRevision)
      && (profile.particleEvidence as { revision?: number } | undefined)?.revision !== profile.particleEvidenceRevision) {
      profile.particleEvidence = { revision: profile.particleEvidenceRevision, tick: prepared.snapshot.tick.toString(), camera,
        items: particles, batches: renderer.captureParticleBatchEvidence(), visibilityQueries:renderer.captureParticleVisibilityEvidence(),materialDepth:renderer.captureMaterialDepthEvidence(), skyCamera: sky3d?.camera, geometry: renderer.captureGeometryEvidence(camera), pixels: this.#canvas.toDataURL("image/png") }
    }
    if (profile && Array.isArray(profile.doorEvidenceTargets) && this.#view.phase === "Ready") {
      const captures = (profile.doorEvidence ??= []) as Array<{ key: string }>
      for (const source of profile.doorEvidenceTargets as number[]) {
        const door = prepared.snapshot.entityPresentation.models.find(model => model.sourceIndex === source)
        if (!door?.mover || captures.length >= 24) continue
        const { position, progress } = door.mover
        if ((position === 2 || position === 4) && (progress < 0.2 || progress > 0.8)) continue
        const key = `${source}:${position === 1 && captures.some(capture => capture.key === `${source}:3`) ? "closed-after" : position}`
        if (captures.some(capture => capture.key === key)) continue
        captures.push(Object.freeze({ key, tick: prepared.snapshot.tick.toString(), camera,
          player: prepared.snapshot.movement.position, entities: prepared.snapshot.entityPresentation,
          geometry: renderer.captureGeometryEvidence(camera), pixels: this.#canvas.toDataURL("image/png") }))
      }
    }
    const publishPrepared=prepared.revision!==this.#lastRenderedPreparedRevision
    this.#lastRenderedPreparedRevision=prepared.revision
    this.#lastRenderedViewRevision=viewRevision
    this.#lastRenderedTick=prepared.snapshot.tick
    this.#displayFrame+=1
    if(profile?.stage==="outdoor"&&Array.isArray(profile.completedDisplays))profile.completedDisplays.push(performance.now())
    const frameProfiler=browserFrameProfiler()
    const admissionProfile=botAdmissionProfile()
    if(admissionProfile)recordBotAdmission(admissionProfile,"frame-submitted",prepared.snapshot.tick,{displayFrame:this.#displayFrame,actors:models?.filter(model=>model.identity>=BOT_MODEL_IDENTITY_BASE&&model.identity<BOT_MODEL_IDENTITY_BASE+0x10000).map(model=>({actor:model.identity-BOT_MODEL_IDENTITY_BASE,model:model.model,skin:model.skin}))??[]})
    const frameDetail=profile||frameProfiler?{tick:prepared.snapshot.tick.toString(),selectedTicks:prepared.publication.selectedTicks,bots:prepared.snapshot.bots.length,buildings:prepared.snapshot.buildings.length,pickups:prepared.snapshot.pickups.length,models:prepared.modelMilliseconds,projectiles:prepared.projectileMilliseconds,visibility:visibilityMilliseconds,particleWorker:prepared.particleMilliseconds,particleDecode:prepared.particleDecodeMilliseconds,legacyParticleMilliseconds,legacyParticleBytes,audio:prepared.audioMilliseconds,particleItems:rendered.timings.particleItems,particleBatches:rendered.timings.particleBatches,dynamicItems:rendered.timings.dynamicItemsMilliseconds,world:rendered.timings.worldMilliseconds,viewmodel:rendered.timings.viewModelMilliseconds,hudModel:hudModelMilliseconds,render:renderMilliseconds,total:totalMilliseconds}:undefined
    if (profile?.captureWorkloadState) profile.workloadProgress = { at: performance.now(), lastRenderedTick: Number(prepared.snapshot.tick), targetTick: profile.workloadTargetTick }
    if (profile?.captureWorkloadState && !frameProfiler?.active
      && (profile.workloadTargetTick == null || profile.workloadTargetTick === Number(prepared.snapshot.tick))) {
      // Plain consumed data only, copied while the pose lease is still owned.
      // This one retained witness is disabled before active sampling begins.
      profile.workloadFrame = structuredClone({ schema: 3, tick: Number(prepared.snapshot.tick), round: prepared.snapshot.round, playerClass: prepared.snapshot.class, weapon: prepared.snapshot.weapon,
        position: camera.position, yaw: camera.yawDegrees, pitch: camera.pitchDegrees, drawSurfaces: visibility.drawSurfaces.length,
        leaves: visibility.leaves.length, props: rendered.visibleMainStaticPropSources.length,
        skySurfaces: rendered.sky3dPass?.skySurfaces ?? 0, skyProps: rendered.sky3dPass?.skyProps ?? 0,
        mainVisibilityIdentity: visibility.cacheIdentity, skyVisibilityIdentity: sky3d?.visibility.cacheIdentity ?? null,
        sceneInputs: { legacyVisuals, effects: prepared.frame.effects, shadows: prepared.frame.shadows,
          brushModels: prepared.frame.brushModels, surfaces: visibility.drawSurfaces, staticProps: rendered.visibleMainStaticPropSources },
        modelInputs: models?.map(model => ({ ...model, pose: model.pose ? { ...model.pose,
          boneMatrices: Array.from(new Uint8Array(model.pose.boneMatrices.buffer, model.pose.boneMatrices.byteOffset, model.pose.boneMatrices.byteLength)),
          flex: model.pose.flex?.map(flex => ({ primitive: flex.primitive, indices: Array.from(flex.indices), positions: Array.from(flex.positions), normals: Array.from(flex.normals) })),
        } : null })),
        particleInputs: particles?.map(particle => ({ ...particle, stableTieIdentity: String(particle.stableTieIdentity),
          visibility: particle.visibility ? { identity: String(particle.visibility.identity), vertices: Array.from(particle.visibility.vertices), clipFraction: particle.visibility.clipFraction } : null,
          mesh: particle.mesh ? { positions: Array.from(particle.mesh.positions), uv: Array.from(particle.mesh.uv),
            colors: Array.from(particle.mesh.colors), indices: Array.from(particle.mesh.indices) } : null })),
        detail: frameDetail })
      profile.captureWorkloadState = false
      const phaseReady = profile.workloadStateReady as ((at: number) => void) | undefined
      profile.workloadStateReady = undefined
      phaseReady?.(performance.now())
    }
    if(frameProfiler?.active&&rendererProfile){
      frameProfiler.counters.completedFrames!+=1
      frameProfiler.completedFrames.push({
        at:performance.now(),displayFrame:this.#displayFrame,tick:Number(prepared.snapshot.tick),playerClass:prepared.snapshot.class,weapon:prepared.snapshot.weapon,
        preparedRevision:prepared.revision,viewRevision,mouseRevision,snapRevision,
        position:camera.position,yaw:camera.yawDegrees,pitch:camera.pitchDegrees,
        drawSurfaces:visibility.drawSurfaces.length,leaves:visibility.leaves.length,
        props:rendered.visibleMainStaticPropSources.length,
        skySurfaces:rendered.sky3dPass?.skySurfaces??0,skyProps:rendered.sky3dPass?.skyProps??0,
        mainVisibilityIdentity:visibility.cacheIdentity,skyVisibilityIdentity:sky3d?.visibility.cacheIdentity??null,
        workerPending:frameProfiler.counters.workerPending,gpuSubmissions:frameProfiler.counters.submissions,
        gpuCommandBuffers:frameProfiler.counters.commandBuffers,renderer:rendererProfile,detail:frameDetail,
      })
    }
    this.#phaseTimings=[prepared.modelMilliseconds,visibilityMilliseconds,prepared.particleMilliseconds,renderMilliseconds,totalMilliseconds]
    this.#canvas.dataset.displayFrame=String(this.#displayFrame)
    this.#canvas.dataset.displayViewRevision=String(viewRevision)
    this.#canvas.dataset.displayPreparedRevision=String(prepared.revision)
    this.#canvas.dataset.displayCameraYaw=String(camera.yawDegrees)
    this.#canvas.dataset.displayCameraPitch=String(camera.pitchDegrees)
    if(profile){
      const coherenceFrames=profile.skyCoherenceFrames
      if(Array.isArray(coherenceFrames))coherenceFrames.push(Object.freeze({
        frame:this.#displayFrame,completedAt:performance.now(),
        generation,viewportRevision:viewport.revision,preparedRevision:prepared.revision,
        viewRevision,mouseRevision,snapRevision,tick:prepared.snapshot.tick.toString(),
        main:camera,sky:sky3d?.camera??null,controller:presentedCamera.controller,
        selection:visibility.sky,disposition:skyDisposition,
        mainVisibilityIdentity:visibility.cacheIdentity,skyVisibilityIdentity:sky3d?.visibility.cacheIdentity??null,
        mainSurfaces:visibility.drawSurfaces.length,skySurfaces:rendered.sky3dPass?.skySurfaces??0,
        skyProps:rendered.sky3dPass?.skyProps??0,visibilityMilliseconds,renderMilliseconds,totalMilliseconds,
      }))
      this.#canvas.dataset.displayCameraPosition=camera.position.join(",")
      const displayedViewmodel=models?.find(model=>model.viewModel)
      const lightingRevision = profile.worldLightingEvidenceRevision
      if (Number.isSafeInteger(lightingRevision)
        && lightingRevision !== (profile.worldLighting as { revision?: number } | undefined)?.revision) {
        profile.worldLighting = Object.freeze({
          revision: lightingRevision,
          profile: renderer.configuration.lightingProfile,
          exposure: rendered.exposure,
          viewmodel: displayedViewmodel?.modelLighting ? Object.freeze({
            origin: displayedViewmodel.modelLighting.lightingOrigin,
            ambientCube: displayedViewmodel.modelLighting.ambientCube,
            localLights: displayedViewmodel.modelLighting.localLights,
            environment: displayedViewmodel.modelLighting.localEnvironment,
          }) : null,
          models: models?.filter((model) => !model.viewModel && model.modelLighting).map((model) => Object.freeze({
            identity: model.identity,
            origin: model.modelLighting!.lightingOrigin,
            localLights: model.modelLighting!.localLights.length,
            eyes: model.eyeStates?.length ?? 0,
          })) ?? [],
          geometry: renderer.captureViewModelEvidence(camera),
          worldGeometry: renderer.captureWorldModelEvidence(camera),
          depthIsolated: rendered.viewModelPass?.worldDepthCleared ?? false,
        })
      }
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
        waterNormalFrame:visibility.water.visibleWater?.evaluated?.normalFrame,
        waterOverlay:visibility.water.visibleWater?.overlay?.identity,
        worldMaterialFrames:visibility.worldMaterials.map(material=>`${material.identity}:${material.textures.find(texture=>texture.role===7)?.frame??"none"}`).join("|"),
        performanceProbe:`${this.#phaseTimings.map(value=>value.toFixed(3)).join(",")}:${this.#wasmCalls.observe},${this.#wasmCalls.models},${this.#wasmCalls.visibility},${this.#wasmCalls.particles}:${this.#maximumScheduledSamples},${this.#maximumPublicationTicks}:${prepared.particleOutputBytes},${prepared.publication.snapshotByteLength}`,
        ...(profile&&!frameProfiler?{performanceDetailProbe:JSON.stringify(frameDetail)}:{}),
        displayFrame:this.#displayFrame,
        displayViewRevision:viewRevision,
        displayPreparedRevision:prepared.revision,
      })
    else if(changedCamera(this.#view.camera,camera))this.#set({camera})
    if(this.#showFps!==0)this.#updateDiagnostics(performance.now())
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
        const selectionClass=this.#selectClass
        const sampledMovementX=this.#pointerMovementX,command=this.#command()
        if (selectionClass !== undefined) selectionTransitionMark("class-command-sent", { generation: sample.generation, identity: selectionClass })
        this.#wasmCalls.observe++
        const publications=await client.observe(sample.generation,sample.nowSeconds,command,sample.suspended)
        if (selectionClass !== undefined) selectionTransitionMark("class-command-acknowledged", { generation: sample.generation, identity: selectionClass,
          publications: publications.map(value => ({ tick: value.snapshot.tick.toString(), class: value.snapshot.class, team: value.snapshot.team })) })
        if(this.#closed||sample.generation!==this.#generation||client!==this.#client)continue
        const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
        if(profile)profile.snapshotTransport=client.snapshotMetrics(sample.generation)
        for(const publication of publications)this.#enqueuePresentation(sample.generation,publication,sampledMovementX)
        if (client.finishRecordedInput(sample.generation, this.#simulationSamples.length)) {
          // These are live caller samples that the explicit replay input owner
          // never admitted. The recorded program and all its publications have
          // completed. Resume at the next ordinary real-time sample, without a
          // game-clock reset or changing any recorded command.
          this.#simulationSamples.clear()
          break
        }
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
  #applyAuthoritativeView(publication:SimulationPublication,_sampledMovementX:number):void{
    const workload = publication.recordedCommandView
    if (workload) { this.#yaw = workload.yaw; this.#pitch = workload.pitch }
    for(const batch of publication.eventBatches){
      const offset = batch.snapshot.viewAngleOffset
      if (offset.some((value, axis) => !Object.is(value, this.#appliedViewAngleOffset[axis]))) {
        this.#appliedViewAngleOffset = offset
        this.#viewRevision+=1
        this.#authoritativeViewRevision+=1
      }
    }
  }
  #enqueuePresentation(generation:number,publication:SimulationPublication,sampledMovementX:number):void{
    if(generation!==this.#generation||this.#closed)return
    this.#applyAuthoritativeView(publication,sampledMovementX)
    for(const batch of publication.eventBatches){const entry=batch.snapshot.projectileTimeline[0];if(!entry||this.#pendingProjectileTimeline.at(-1)?.tick===entry.tick)continue;if(this.#pendingProjectileTimeline.at(-1)&&this.#pendingProjectileTimeline.at(-1)!.tick>entry.tick){this.#paused=true;this.#set({phase:"Failed",detail:"Projectile presentation timeline reversed before admission"});return}this.#pendingProjectileTimeline.push(entry)}
    if(this.#pendingProjectileTimeline.length>4096){this.#paused=true;this.#set({phase:"Failed",detail:"Projectile presentation timeline reached its explicit limit"});return}
    const workload = this.#recordedPresentations
    if (workload && workload.cursor < workload.inputs.length) {
      if (workload.buffered.length >= 1024 || workload.buffered.reduce((n, p) => n + p.snapshotByteLength, publication.snapshotByteLength) > 64 * 1024 * 1024) {
        this.#paused = true; this.#set({ phase: "Failed", detail: "Recorded presentation queue overflow" }); return
      }
      workload.buffered.push(publication)
    } else {
      this.#pendingPresentation=this.#pendingPresentation?this.#mergePublications(this.#pendingPresentation,publication):publication
      this.#maximumPublicationTicks=Math.max(this.#maximumPublicationTicks,this.#pendingPresentation.selectedTicks)
    }
    if(!this.#presentationBusy){const task=this.#drainPresentations();this.#presentationTask=task;void task.finally(()=>{if(this.#presentationTask===task)this.#presentationTask=undefined})}
  }
  #mergePublications(left:SimulationPublication,right:SimulationPublication):SimulationPublication{const snapshot=mergePublicationSnapshots([left.snapshot,right.snapshot]);return Object.freeze({...right,firstHostTick:left.firstHostTick,selectedTicks:left.selectedTicks+right.selectedTicks,eventBatches:Object.freeze([...left.eventBatches,...right.eventBatches]),snapshot})}
  async #drainPresentations():Promise<void>{
    this.#presentationBusy=true
    const generation = this.#generation
    try {
      while (!this.#closed && generation === this.#generation) {
        const workload = this.#recordedPresentations
        let value: SimulationPublication | undefined
        if (workload && workload.cursor < workload.inputs.length) {
          const expected = workload.inputs[workload.cursor]!
          const last = BigInt(expected.lastHostTick)
          if (!workload.buffered.length || workload.buffered.at(-1)!.lastHostTick < last) break
          if (workload.buffered[0]!.firstHostTick !== BigInt(expected.firstHostTick)) throw new Error("Recorded presentation begins at a different Source tick")
          while (workload.buffered.length && (value?.lastHostTick ?? 0n) < last) {
            const next = workload.buffered.shift()!
            value = value ? this.#mergePublications(value, next) : next
          }
          if (value?.lastHostTick !== last || value.selectedTicks !== expected.selectedTicks) throw new Error("Recorded presentation grouping differs")
          const epoch = (globalThis as any).__playsrcProfile.commandWorkload.epoch
          if (!Number.isFinite(epoch)) throw new Error("Recorded presentation clock is absent")
          const due = epoch + expected.atSeconds * 1000
          while (!this.#closed && generation === this.#generation && performance.now() < due) await new Promise(resolve => setTimeout(resolve, Math.min(20, due - performance.now())))
          if (this.#closed || generation !== this.#generation) break
          workload.cursor++
        } else {
          if (workload?.buffered.length) {
            for (const publication of workload.buffered) value = value ? this.#mergePublications(value, publication) : publication
            workload.buffered.length = 0
          }
          if (this.#pendingPresentation) value = value ? this.#mergePublications(value, this.#pendingPresentation) : this.#pendingPresentation
          this.#pendingPresentation = undefined
        }
        if (!value) break
        this.#maximumPublicationTicks = Math.max(this.#maximumPublicationTicks, value.selectedTicks)
        await this.#present(value)
      }
    } catch (error) {
      if (!this.#closed && generation === this.#generation) { this.#paused = true; this.#set({ phase: "Failed", detail: this.#failureDetail(error, "Recorded presentation failed") }) }
    } finally { this.#presentationBusy=false }
  }

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
    const captureProfile = (globalThis as any).__playsrcProfile
    if (captureProfile?.captureClientFrames) {
      if (captureProfile.presentationInputs.length >= 16384) throw new Error("Presentation workload recording overflow")
      captureProfile.presentationInputs.push({ atSeconds: performance.now() / 1000, firstHostTick: String(publication.firstHostTick), lastHostTick: String(publication.lastHostTick), selectedTicks: publication.selectedTicks })
    }
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
      const cosmeticProfile = (globalThis as any).__playsrcProfile
      const cosmeticEquip = cosmeticProfile?.cosmeticBotEquip
      if (cosmeticEquip && cosmeticProfile.cosmeticBotEquipResult?.revision !== cosmeticEquip.revision) {
        cosmeticProfile.cosmeticBotEquipResult = { revision: cosmeticEquip.revision, complete: false }
        for (const entry of cosmeticEquip.items) {
          const mutation = new Uint8Array(9), view = new DataView(mutation.buffer)
          mutation[0] = 2; view.setUint32(1, entry.actor, true); view.setUint32(5, entry.definition ?? 0xffff_ffff, true)
          await client.equipment(generation, mutation.buffer)
        }
        cosmeticProfile.cosmeticBotEquipResult = { revision: cosmeticEquip.revision, complete: true }
      }
      const admissionProfile=botAdmissionProfile()
      if(admissionProfile)recordBotAdmission(admissionProfile,"publication",snapshot.tick,{bytes:publication.snapshotByteLength,firstHostTick:publication.firstHostTick.toString(),lastHostTick:publication.lastHostTick.toString(),actors:snapshot.bots.map(bot=>({actor:bot.identity,class:bot.class,team:bot.team,weapon:bot.weapon?.identity??null,lifecycle:bot.lifecycle}))})
      if (this.#classSelection?.state().visible && this.#classSelection.state().team !== snapshot.team) {
        this.#classSelection.dispatch({ kind: "team-changed", team: snapshot.team })
      }
      this.#recordCrouch(snapshot)
      this.#recordAuthorityBlockers(snapshot)
      const activeWeapon=snapshot.loadout.find(value=>value.weapon===snapshot.weapon)
      if(activeWeapon){
        const reloadState=`${activeWeapon.weapon}:${activeWeapon.clip}/${activeWeapon.reserve}:${activeWeapon.reload}`
        if(!this.#reloadHistory.at(-1)?.endsWith(`:${reloadState}`)){
          this.#reloadHistory.push(`${snapshot.tick}:${reloadState}`)
          if(this.#reloadHistory.length>128)this.#reloadHistory.shift()
        }
      }
      for (const event of snapshot.projectileEvents) {
        const profile=(globalThis as any).__playsrcProfile
        if(profile?.captureProjectileGameplay===true){
          const journal=(profile.projectileEvents??=[]) as unknown[]
          journal.push({type:event.type,kind:event.kind,projectile:event.projectile,tick:event.tick.toString(),position:event.position,normal:event.contactNormal,owner:event.ownerIdentity})
          if(journal.length>512)journal.splice(0,journal.length-512)
        }
        if (event.type === "fire") {this.#fireEvents += 1;this.#fireTickHistory.push(`${event.kind}:${event.tick}:${event.position.join(",")}`);if(this.#fireTickHistory.length>128)this.#fireTickHistory.shift()}
        if (event.type === "explode") this.#explosionEvents += 1
      }
      for(const event of snapshot.events)if(event.kind===12)this.#fireEvents+=1
      this.#recordLockerAnimations(snapshot)
      for (const p of snapshot.projectiles) {
        const add = (identity: number, next: ReadonlySet<string>) =>
          this.#attachments.set(identity, new Set([...(this.#attachments.get(identity) ?? []), ...next]))
        const m = this.#artifacts.models.get(
          projectileModelPath(p.kind, p.miniRocket),
        )
        if (m) add(p.identity, new Set(m.attachments.keys()))
      }
      const authoritativeCamera=tf2Camera(snapshot,this.#yaw,this.#pitch)
      const presentedEye=this.#predictedEye.sample(publication.interpolation)
      const camera=presentedEye?Object.freeze({...authoritativeCamera,position:presentedEye}):authoritativeCamera
      const override=(globalThis as typeof globalThis&{__playsrcProfile?:{displacementCameraOverride?:Partial<Camera>}})
        .__playsrcProfile?.displacementCameraOverride
      const visibilityCamera=selectPresentedCamera(camera,override)
      const worldModelLighting = (
        origin: readonly [number, number, number],
        angles: readonly [number, number, number],
        lightingCamera: Camera = visibilityCamera,
      ): NonNullable<ModelPoseRequest["lighting"]> => Object.freeze({
        origin,
        angles,
        cameraPosition: lightingCamera.position,
        cameraAngles: Object.freeze([lightingCamera.pitchDegrees, lightingCamera.yawDegrees, 0]),
      })
      const viewModelLighting = worldModelLighting(
        camera.position,
        Object.freeze([camera.pitchDegrees, camera.yawDegrees, 0]),
        camera,
      )
      if(!ownsGeneration())return
      const viewport=this.#viewport(),view={aspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),farPlane:camera.far}
      const historicalViewmodels: ModelPoseRequest[] = []
      const mapViewmodel = (value: Snapshot) => {
        if (this.#viewmodelClass !== undefined && this.#viewmodelClass !== value.class) {
          this.#viewmodels = createViewmodelPresenter(this.#artifacts!, this.#equipmentProfile!.state()!.inventory)
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
      const currentViewmodelRequest=viewmodel===undefined?undefined:Object.freeze({...viewmodel.request,cloak:snapshot.actorCloaks.find(actor=>actor.identity===1),sampleTick:currentFire?.tick??snapshot.tick,lighting:viewModelLighting,...(currentFire?{fireView:currentFire.launcherPose!}:{})})
      let watchRequest: ModelPoseRequest | undefined
      const respawnTick = snapshot.lifecycleEvents.reduce((tick, event) => event.kind === 2 && event.tick > tick ? event.tick : tick, this.#watchOwner?.generation === generation ? this.#watchOwner.respawnTick : 0n)
      if (this.#watchOwner?.generation !== generation || this.#watchOwner.team !== snapshot.team || this.#watchOwner.respawnTick !== respawnTick) {
        this.#watchActivity = undefined
        this.#watchOwner = { generation, team: snapshot.team, respawnTick }
      }
      if (snapshot.class !== 8 || snapshot.lifecycle !== 1) {
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
              cloak: snapshot.actorCloaks.find(actor => actor.identity === 1),
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
              lighting: viewModelLighting,
            })
          }
        }
      }
      const lockerRequests=[...this.#lockerAnimations].flatMap(([identity,state])=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===identity),artifact=occurrence&&this.#artifacts!.models.get(occurrence.model);if(!occurrence||!artifact){this.#blockers.add(`TF2 regenerate model presentation unavailable: ${identity}`);return []}const closed=snapshot.tick>=state.closeTick,animation=closed?state.closeAnimation:state.openAnimation,start=closed?state.closeTick:state.openTick,elapsed=Math.max(0,Number(snapshot.tick-start)*0.015),previousTick=snapshot.tick>BigInt(publication.selectedTicks)?snapshot.tick-BigInt(publication.selectedTicks):0n,previousElapsed=Math.max(0,Number(previousTick-start)*0.015);return [Object.freeze({identity,model:occurrence.model,activity:animation,previousElapsedSeconds:Math.min(previousElapsed,elapsed),elapsedSeconds:elapsed,currentTimeSeconds:Number(snapshot.tick)*0.015,frameTimeSeconds:publication.selectedTicks*0.015,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:occurrence.skin,lod:0,bodygroups:Object.freeze([]),packedBody:state.body,lighting:worldModelLighting(occurrence.origin,occurrence.angles)})]})
      const mainVisibilityView={
        position:visibilityCamera.position,
        yawDegrees:visibilityCamera.yawDegrees,
        pitchDegrees:visibilityCamera.pitchDegrees,
        verticalFovDegrees:visibilityCamera.verticalFovDegrees,
        aspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),
        near:visibilityCamera.near,
        far:visibilityCamera.far,
        presentationTimeSeconds:Number(snapshot.tick)*0.015,
      }
      const skyController=this.#skyController(this.#artifacts)
      const preparedCamera=presentCamera(visibilityCamera,{generation,viewportRevision:viewport.revision,preparedRevision:this.#preparedRevision+1,viewRevision:this.#viewRevision,mouseRevision:this.#mouseViewRevision,snapRevision:this.#authoritativeViewRevision,tick:snapshot.tick},skyController)
      const skyCamera=preparedCamera.sky
      const visibilityViews=skyController&&skyCamera?[mainVisibilityView,{
        position:skyCamera.position,visibilityPosition:skyController.origin,areaFilter:skyController.area,
        yawDegrees:skyCamera.yawDegrees,pitchDegrees:skyCamera.pitchDegrees,verticalFovDegrees:skyCamera.verticalFovDegrees,
        aspectRatio:mainVisibilityView.aspectRatio,near:skyCamera.near,far:skyCamera.far,
        presentationTimeSeconds:mainVisibilityView.presentationTimeSeconds,
      }]:[mainVisibilityView]
      this.#wasmCalls.visibility++
      const [visibility,skyVisibility]=await client.visibilityViews(generation,visibilityViews)
      if(!visibility)throw new Error("Main-world visibility transaction returned no result")
      if(!ownsGeneration())return
      const flagCarriers=new Set((snapshot.objectives?.flags??[]).flatMap(flag=>flag.carrier===null?[]:[flag.carrier]))
      const botSelection=combatPoseSelection(snapshot.bots, publication.eventBatches.flatMap(batch=>batch.snapshot.events), bot=>bot.equippedItems.some(item=>item.definitionIndex===378)||flagCarriers.has(bot.identity)||this.#renderer!.modelVisible(
        tf2ClassPresentation(bot.class).model,
        bot.team===2?0:1,
        bot.position,
        [0,bot.yawDegrees,0],
        visibilityCamera,
        visibility.water.passes,
      ))
      const posedBots=botSelection.posed
      const botRequests=posedBots.map(bot=>{
        const model=tf2ClassPresentation(bot.class).model
        const artifact=this.#artifacts!.models.get(model)
        if(!artifact)throw new Error(`Authored TF2 bot player model unavailable: ${model}`)
        const role=bot.animationRole
        const moving=Math.hypot(bot.velocity[0],bot.velocity[1])>1
        const activity=`ACT_MP_${moving?"RUN":"STAND"}_${role}`
        if(!artifact.sequences.some(sequence=>sequence.activity===activity))throw new Error(`Authored TF2 bot player activity unavailable: ${model}:${activity}`)
        const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS
        return Object.freeze({identity:BOT_MODEL_IDENTITY_BASE+bot.identity,model,activity,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:Math.hypot(bot.velocity[0],bot.velocity[1]),screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:bot.team===2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0)),lighting:worldModelLighting(bot.position,Object.freeze([0,bot.yawDegrees,0]))})
      }).map((request, index) => {
        const bot = posedBots[index]!
        if (bot.class !== 8 || bot.lifecycle !== 1) return request
        return Object.freeze({ ...request, cloak: snapshot.actorCloaks.find(actor => actor.identity === bot.identity) })
      })
      const equippedBotRequests = botRequests.map((request, index) => Object.freeze({ ...request, actorIdentity: posedBots[index]!.identity, equippedItems: posedBots[index]!.equippedItems }))
      const objectiveRequests=(snapshot.objectives?.flags??[]).flatMap(flag=>{
        if(flag.disabled&&!flag.visibleWhenDisabled||flag.carrier===1)return []
        const artifact=this.#artifacts!.models.get(flag.model)
        if(!artifact)throw new Error(`Authored TF2 intelligence model unavailable: ${flag.model}`)
        const activity=flag.status===1?"idle":"spin"
        if(!artifact.sequences.some(sequence=>sequence.label.toLowerCase()===activity))throw new Error(`Authored TF2 intelligence sequence unavailable: ${flag.model}:${activity}`)
        const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS
        return [Object.freeze({identity:OBJECTIVE_MODEL_IDENTITY_BASE+flag.identity,model:flag.model,activity,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:flag.skin,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0)),lighting:worldModelLighting(flag.position,flag.angles)})]
      })
      const controlPointRequests=(snapshot.controlPoints?.points??[]).filter(point=>point.modelVisible).map(point=>{
        const artifact=this.#artifacts!.models.get(point.model)
        if(!artifact||!artifact.sequences.some(sequence=>sequence.label.toLowerCase()==="idle"))throw new Error(`Authored TF2 control point model/idle sequence unavailable: ${point.model}`)
        const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS
        return Object.freeze({identity:OBJECTIVE_MODEL_IDENTITY_BASE+point.identity,controlPoint:point.identity,model:point.model,activity:"idle",previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:point.skin,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map((count,index)=>index===0&&point.owner<count?point.owner:0)),lighting:worldModelLighting(point.position,point.angles)})
      })
      const buildingRequests=snapshot.buildings.map(building=>{
        const model=buildingModel(building),artifact=this.#artifacts!.models.get(model)
        if(!artifact)throw new Error(`Authored TF2 building model unavailable: ${model}`)
        const desired=building.phase===0?"ACT_OBJ_ASSEMBLING":building.phase===2?"ACT_OBJ_UPGRADING":"ACT_OBJ_RUNNING"
        const selected=artifact.sequences.find(sequence=>sequence.activity===desired)??artifact.sequences.find(sequence=>sequence.activity==="ACT_OBJ_IDLE")??artifact.sequences[0]
        if(!selected)throw new Error(`Authored TF2 building activity unavailable: ${model}:${desired}`)
        const elapsed=Math.max(0,Number(snapshot.tick-building.startedTick)*SIMULATION_SAMPLE_INTERVAL_SECONDS)
        return Object.freeze({identity:building.identity,model,activity:selected.activity||selected.label,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:building.team===2||artifact.skinCount<2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0)),lighting:worldModelLighting(building.position,Object.freeze([0,building.yawDegrees,0]))})
      })
      const placementRequest=snapshot.placement?(()=>{const placement=snapshot.placement!,model=blueprintModel(placement.object),artifact=this.#artifacts!.models.get(model),sequence=artifact?.sequences[0];if(!artifact||!sequence)throw new Error(`Authored TF2 building blueprint unavailable: ${model}`);const elapsed=Number(snapshot.tick)*SIMULATION_SAMPLE_INTERVAL_SECONDS;return Object.freeze({identity:BUILDING_BLUEPRINT_IDENTITY,model,activity:sequence.activity||sequence.label,previousElapsedSeconds:Math.max(0,elapsed-publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS),elapsedSeconds:elapsed,currentTimeSeconds:elapsed,frameTimeSeconds:publication.selectedTicks*SIMULATION_SAMPLE_INTERVAL_SECONDS,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:snapshot.team===2||artifact.skinCount<2?0:1,lod:0,bodygroups:Object.freeze(artifact.bodygroupCounts.map(()=>0)),lighting:worldModelLighting(placement.position,Object.freeze([0,placement.yawDegrees,0]))})})():undefined
      const studioRequests = snapshot.entityPresentation.studioAnimations.flatMap(animation => {
        if (this.#lockerAnimations.has(animation.sourceIndex)) return []
        const state = snapshot.entityPresentation.studioModels.find(model => model.sourceIndex === animation.sourceIndex)
        if (!state?.draw) return []
        const occurrence = this.#artifacts!.modelOccurrences.find(value => value.entity === animation.sourceIndex)
        if (!occurrence) throw new Error(`Dynamic prop occurrence unavailable: ${animation.sourceIndex}`)
        return [Object.freeze({ identity: animation.sourceIndex, model: occurrence.model, activity: animation.sequence,
          elapsedSeconds: animation.elapsedSeconds, previousElapsedSeconds: Math.max(0, animation.elapsedSeconds - publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS),
          currentTimeSeconds: Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS, frameTimeSeconds: publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS,
          planarSpeed: 0, screenAspectRatio: Math.max(1, viewport.width) / Math.max(1, viewport.height), worldFarPlane: camera.far,
          skin: state.skin, lod: 0, bodygroups: Object.freeze([]), packedBody: occurrence.body, lighting: worldModelLighting(state.worldPosition, state.worldAngles) })]
      })
      const animatedSources = new Set(studioRequests.map(request => request.identity))
      for (const state of snapshot.entityPresentation.studioModels) {
        if (!state.draw || animatedSources.has(state.sourceIndex) || this.#lockerAnimations.has(state.sourceIndex)) continue
        const occurrence = this.#artifacts!.modelOccurrences.find(value => value.entity === state.sourceIndex)
        if (!occurrence || occurrence.skin === state.skin) continue
        const model = this.#artifacts!.models.get(occurrence.model), sequence = model?.sequences[0]
        if (!model || !sequence) throw new Error(`Authored model skin transition lacks its sequence: ${state.sourceIndex}`)
        studioRequests.push(Object.freeze({ identity: state.sourceIndex, model: occurrence.model, activity: sequence.label,
          elapsedSeconds: 0, previousElapsedSeconds: 0, currentTimeSeconds: Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS,
          frameTimeSeconds: publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS, planarSpeed: 0,
          screenAspectRatio: Math.max(1,viewport.width)/Math.max(1,viewport.height), worldFarPlane: camera.far,
          skin: state.skin, lod: 0, bodygroups: Object.freeze([]), packedBody: occurrence.body, lighting: worldModelLighting(state.worldPosition,state.worldAngles) }))
      }
      const hudPortrait = this.#hudIntegration?.modelPanel()
      let hudRequest: ModelPoseRequest | undefined
      if (hudPortrait && snapshot.lifecycle === 1 && hudPortrait.model === tf2ClassPresentation(snapshot.class).model && snapshot.equippedItems.some(item => item.definitionIndex === 378)) {
        const inventory = this.#equipmentProfile!.state()!.inventory
        const heldItem = snapshot.equippedItems.find(item => inventory.some(entry => entry.item.definitionIndex === item.definitionIndex && entry.classSlots.some(slot => slot.class === snapshot.class && slot.weapon === snapshot.weapon)))
        if (!heldItem || heldItem.slot > 2) throw new Error("Equipped HUD weapon metadata is unavailable")
        const held = inventory.find(entry => entry.item.definitionIndex === heldItem.definitionIndex)!
        const itemModel = held.modelPlayer || undefined, heldModel = itemModel ? this.#artifacts.models.get(itemModel) : undefined
        if (itemModel && !heldModel) throw new Error(`Equipped HUD weapon model is unavailable: ${itemModel}`)
        const key = `${generation}:${snapshot.class}:${snapshot.team}:${snapshot.weapon}:${snapshot.equippedItems.map(item => item.itemId).join(",")}`
        const reset = key !== this.#cosmeticHudKey
        if (reset) { this.#cosmeticHudKey = key; this.#cosmeticHudStarted = snapshot.tick }
        const elapsed = Number(snapshot.tick - this.#cosmeticHudStarted) * SIMULATION_SAMPLE_INTERVAL_SECONDS
        hudRequest = { identity: 0x5fff_ff01, actorIdentity: 1, hudModel: true, modelPanel: true, modelPanelReset: reset,
          itemDefinition: heldItem.definitionIndex,
          model: hudPortrait.model, itemModel, worldItem: Boolean(itemModel), itemBodygroups: heldModel?.bodygroupCounts.map(() => 0),
          activity: "ACT_MP_STAND_IDLE", equippedItems: snapshot.equippedItems,
          previousElapsedSeconds: Math.max(0, elapsed - publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS), elapsedSeconds: elapsed,
          currentTimeSeconds: Number(snapshot.tick) * SIMULATION_SAMPLE_INTERVAL_SECONDS, frameTimeSeconds: publication.selectedTicks * SIMULATION_SAMPLE_INTERVAL_SECONDS,
          planarSpeed: 0, screenAspectRatio: hudPortrait.bounds.width / hudPortrait.bounds.height, worldFarPlane: camera.far,
          skin: hudPortrait.skin, lod: 0, bodygroups: this.#artifacts.models.get(hudPortrait.model)!.bodygroupCounts.map(() => 0),
          lighting: { origin: hudPortrait.origin, angles: hudPortrait.angles, cameraPosition: [0,0,0], cameraAngles: [0,0,0] } }
      } else { this.#cosmeticHudKey = "" }
      const modelStart=performance.now(),modelRequests=[...historicalViewmodels,...(currentViewmodelRequest?[currentViewmodelRequest]:[]),...(watchRequest?[watchRequest]:[]),...lockerRequests,...studioRequests,...equippedBotRequests,...objectiveRequests,...controlPointRequests,...buildingRequests,...(placementRequest?[placementRequest]:[]),...(hudRequest ? [hudRequest] : [])]
      if(admissionProfile)recordBotAdmission(admissionProfile,"model-request",snapshot.tick,{actors:botRequests.map(request=>({actor:request.identity-BOT_MODEL_IDENTITY_BASE,model:request.model,skin:request.skin,activity:request.activity,itemModel:"itemModel" in request?request.itemModel:null}))})
      const modelRequest=modelRequests.length===0?undefined:(this.#wasmCalls.models++,client.models(generation,encodeModelPoseBatch(modelRequests)))
      const modelOutput=modelRequest===undefined?undefined:await modelRequest
      if(!ownsGeneration())return
      const modelPoses=modelOutput===undefined?[]:modelOutput
      const hudModelPoses = modelPoses.filter(pose => pose.identity === 0x5fff_ff01)
      const modelMilliseconds=performance.now()-modelStart
      if(admissionProfile)recordBotAdmission(admissionProfile,"model-complete",snapshot.tick,{milliseconds:modelMilliseconds})
      const viewmodelIdentities=new Set([...historicalViewmodels.map(request=>request.identity),...(viewmodel?[viewmodel.item.identity]:[])])
      const timelineViewmodelPoses = modelPoses.filter((pose) => viewmodelIdentities.has(pose.identity))
      const viewmodelPoses = currentViewmodelRequest===undefined?[]:timelineViewmodelPoses.filter((pose) => pose.identity===currentViewmodelRequest.identity&&!pose.attachmentsOnly&&pose.sampleTick===currentViewmodelRequest.sampleTick)
      const weaponProfile = (globalThis as any).__playsrcProfile
      if (weaponProfile?.captureWeaponPoses) {
        const item = viewmodelPoses.find(pose => pose.role === "item")
        weaponProfile.weaponPose = item ? { model: item.model, definition: currentViewmodelRequest?.itemDefinition,
          class: snapshot.class, weapon: snapshot.weapon, ammo: snapshot.loadout.find(value => value.weapon === snapshot.weapon), tick: String(snapshot.tick) } : null
      }
      const lockerPoses=modelPoses.filter(pose=>this.#lockerAnimations.has(pose.identity))
      const studioPoses = modelPoses.filter(pose => studioRequests.some(request => request.identity === pose.identity))
      const watchPose = watchRequest && modelPoses.find(pose => pose.identity === watchRequest!.identity)
      if (watchRequest && (!watchPose || watchPose.role !== "hand" || !watchPose.viewmodel)) throw new Error("Authored Spy offhand watch pose differs")
      const botParts=modelPoses.filter(pose=>pose.identity>=BOT_MODEL_IDENTITY_BASE&&pose.identity<BOT_MODEL_IDENTITY_BASE+0x10000)
      const botPoses=botParts.filter(pose=>pose.role==="single")
      const controlPointPoses=modelPoses.filter(pose=>controlPointRequests.some(request=>request.identity===pose.identity))
      const objectivePoses=modelPoses.filter(pose=>objectiveRequests.some(request=>request.identity===pose.identity))
      if(controlPointPoses.length!==controlPointRequests.length)throw new Error("TF2 control point pose output differs from authoritative state")
      if(objectivePoses.length!==objectiveRequests.length)throw new Error("TF2 intelligence pose output differs from authoritative objective state")
      const buildingPoses=modelPoses.filter(pose=>snapshot.buildings.some(building=>building.identity===pose.identity))
      const blueprintPose=modelPoses.find(pose=>pose.identity===BUILDING_BLUEPRINT_IDENTITY)
      if(buildingPoses.length!==snapshot.buildings.length||Boolean(blueprintPose)!==Boolean(snapshot.placement))throw new Error("TF2 building pose output differs from authoritative object state")
      if(botPoses.length!==posedBots.length)throw new Error("TF2 bot player pose output differs from authoritative player state")
      if(viewmodel!==undefined&&((viewmodel.request.handsOnlyViewmodel||viewmodel.standalone)?(viewmodelPoses.length!==1||viewmodelPoses[0]?.role!=="hand"):(viewmodelPoses.length!==2||viewmodelPoses.filter(pose=>pose.role==="item").length!==1||viewmodelPoses.filter(pose=>pose.role==="hand").length!==1)))throw new Error(`Viewmodel composition output differs: weapon=${snapshot.weapon}; roles=${viewmodelPoses.map(pose=>pose.role).join(",")}`);const viewmodelPose=viewmodelPoses.find(pose=>pose.role==="hand")
      if(viewmodelPose)this.#viewmodelActivities.add(viewmodelPose.activity)
      const weaponPoseProfile = (globalThis as any).__playsrcProfile
      if (weaponPoseProfile?.captureWeaponPoses) {
        const item = viewmodelPoses.find(pose => pose.role === "item")
        weaponPoseProfile.weaponPose = item ? { model: item.model, definition: currentViewmodelRequest?.itemDefinition,
          class: snapshot.class, weapon: snapshot.weapon, ammo: snapshot.loadout.find(value => value.weapon === snapshot.weapon),
          tick: String(snapshot.tick), bones: Array.from(item.boneMatrices) } : null
        if (weaponPoseProfile.captureHitscan) {
          weaponPoseProfile.hitscan = { heads: snapshot.decapitations, conditions: snapshot.conditions,
            ammo: snapshot.loadout.find(value => value.weapon === snapshot.weapon),
            actors: botPoses.map(pose => {
              const bot = snapshot.bots.find(bot => bot.identity + BOT_MODEL_IDENTITY_BASE === pose.identity)!
              return { identity: bot.identity, attachments: Object.fromEntries(pose.attachments.map(attachment =>
                [attachment.name.toLowerCase(), transformAttachment(attachment.matrix, bot.position, sourceViewOrientation(0, bot.yawDegrees)).position])) }
            }) }
        }
      }
      this.#updateAttachmentTransforms(snapshot, timelineViewmodelPoses, camera)
      let presentation:ReturnType<ProjectileMapper["map"]>
      const projectileStart=performance.now()
      if(!ownsGeneration())return
      try{presentation=owners.mapper.map(projectileFrame(snapshot))}catch(error){const timeline=snapshot.projectileTimeline.map(value=>`${value.tick}[${value.projectiles.map(projectile=>projectile.identity).join(",")}]{${value.events.map(event=>`${event.type}:${event.projectile}@${event.tick}`).join(",")}}`).join("|"),top=snapshot.projectileEvents.map(event=>`${event.type}:${event.projectile}@${event.tick}`).join(",");throw new Error(`${error instanceof Error?error.message:"projectile presentation failed"}; top ${top}; timeline ${timeline}`)}
      const projectileMilliseconds=performance.now()-projectileStart
      this.#pendingProjectileTimeline.splice(0,projectileTimeline.length)
      const particleStart=performance.now()
      const pyroParticles=snapshot.class===7||this.#pyroFlameEffect||this.#manmelterChargeEffect?this.#pyroParticles(snapshot):[]
      const medicBeam: ProjectileParticleRequest[] = []
      if (this.#medicBeamTarget !== null && (this.#medicBeamTarget !== snapshot.medigunTarget || this.#medicBeamReleasing !== snapshot.medigunReleasing)) {
        const prior = this.#medicBeamTarget
        const replacing = prior === snapshot.medigunTarget
        medicBeam.push(Object.freeze({ kind: "stop", identity: `${snapshot.tick}:medic:${prior}:stop`, effectIdentity: `medic:1:${prior}`, eventIdentity: `${snapshot.tick}:medic:${prior}:stop`, tick: snapshot.tick, projectileIdentity: prior, immediate: replacing }))
        this.#medicBeamTarget = null
        this.#medicBeamReleasing = false
      }
      if (snapshot.medigunTarget !== null) {
        const patient = snapshot.bots.find(bot => bot.identity === snapshot.medigunTarget)
        if (!patient) throw new Error("Medi Gun patient is missing from the authoritative player roster")
        const muzzle = this.#attachmentTransforms.get(20)?.get("muzzle")
        if (!muzzle) throw new Error("Medi Gun muzzle attachment is unavailable")
        const patientPoint = Object.freeze({ index: 1 as const, position: Object.freeze([patient.position[0], patient.position[1], patient.position[2] + 41] as const), orientation: Object.freeze([0, 0, 0, 1] as const), ownerIdentity: patient.identity })
        if (this.#medicBeamTarget !== patient.identity) {
          const system = snapshot.team === 2
            ? snapshot.medigunReleasing ? "medicgun_beam_red_invun" : "medicgun_beam_red"
            : snapshot.medigunReleasing ? "medicgun_beam_blue_invun" : "medicgun_beam_blue"
          medicBeam.push(Object.freeze({ kind: "start", identity: `${snapshot.tick}:medic:${patient.identity}:start`, effectIdentity: `medic:1:${patient.identity}`, eventIdentity: `${snapshot.tick}:medic:${patient.identity}:start`, tick: snapshot.tick, projectileIdentity: patient.identity, ownerIdentity: 1, launcherIdentity: 20, team: snapshot.team === 2 ? "red" : "blue", system, attachment: Object.freeze({ entityIdentity: 20, name: "muzzle" }), controlPoints: Object.freeze([Object.freeze({ index: 0 as const, position: muzzle.position, orientation: muzzle.orientation, ownerIdentity: 1 }), patientPoint]) }))
          this.#medicBeamTarget = patient.identity
          this.#medicBeamReleasing = snapshot.medigunReleasing
        } else {
          medicBeam.push(Object.freeze({ kind: "set-control-point", identity: `${snapshot.tick}:medic:${patient.identity}:muzzle`, effectIdentity: `medic:1:${patient.identity}`, eventIdentity: `${snapshot.tick}:medic:${patient.identity}:muzzle`, tick: snapshot.tick, projectileIdentity: patient.identity, controlPoint: Object.freeze({ index: 0 as const, position: muzzle.position, orientation: muzzle.orientation, ownerIdentity: 1 }) }))
          medicBeam.push(Object.freeze({ kind: "set-control-point", identity: `${snapshot.tick}:medic:${patient.identity}:patient`, effectIdentity: `medic:1:${patient.identity}`, eventIdentity: `${snapshot.tick}:medic:${patient.identity}:patient`, tick: snapshot.tick, projectileIdentity: patient.identity, controlPoint: patientPoint }))
        }
      }
      const playerAttachmentTransforms=botSelection.criticalTargets.size?new Map(botPoses.map(pose=>{
        const bot=snapshot.bots.find(value=>BOT_MODEL_IDENTITY_BASE+value.identity===pose.identity)!
        return [bot.identity,new Map(pose.attachments.map(attachment=>[attachment.name.toLowerCase(),transformAttachment(attachment.matrix,bot.position,sourceViewOrientation(0,bot.yawDegrees))]))] as const
      })):undefined
      const playerActors=botSelection.criticalTargets.size?new Map(snapshot.bots.map(bot=>[bot.identity,bot])):undefined
      const combatParticles=publication.eventBatches.flatMap(batch=>{
        const muzzles=snapshot.class===1||snapshot.class===3||snapshot.class===6||snapshot.class===9||snapshot.class===8
          ?hitscanMuzzleParticles(batch.snapshot,{systems:this.#particleSystems,attachmentTransforms:this.#attachmentTransforms}):[]
        const result=combatImpactParticles(batch.snapshot,{tracerCount:this.#combatTracerCount},{systems:this.#particleSystems,attachmentTransforms:this.#attachmentTransforms,playerAttachmentTransforms,playerActors})
        this.#combatTracerCount=result.state.tracerCount
        return [...muzzles,...result.particles]
      })
      const conditionParticles=this.#meleeConditions.map(snapshot)
      const supplementalParticles=[...combatParticles,...pyroParticles,...medicBeam,...conditionParticles]
      const combinedParticles=supplementalParticles.length===0?presentation.particles:[...presentation.particles,...supplementalParticles].sort((left,right)=>left.tick<right.tick?-1:left.tick>right.tick?1:0)
      const particleBatch=owners.encoder.encode(snapshot.tick,camera.position,combinedParticles, {
        yawDegrees: camera.yawDegrees, pitchDegrees: camera.pitchDegrees, verticalFovDegrees: camera.verticalFovDegrees,
        width: this.#canvas.width, height: this.#canvas.height, samples: this.#renderer!.takeParticleVisibilitySamples(),
      })
      if(!ownsGeneration())return
      this.#wasmCalls.particles++
      const particleOutput=await client.particles(generation,particleBatch)
      if(!ownsGeneration())return
      const particleMilliseconds=performance.now()-particleStart
      const particleDecodeStart=performance.now(),particleItems=[...decodeParticleRenderOutput(particleOutput,this.#artifacts.particleMaterials).items,
        ...botParts.flatMap(pose => pose.wearable?.particleBytes.byteLength ? decodeParticleRenderOutput(pose.wearable.particleBytes, this.#artifacts!.particleMaterials).items : [])],particleDecodeMilliseconds=performance.now()-particleDecodeStart
      if (cosmeticProfile?.captureCosmetics) cosmeticProfile.cosmetics = {
        tick: snapshot.tick.toString(), local: snapshot.equippedItems, camera: visibilityCamera,
        actors: posedBots.map(bot => ({ identity: bot.identity, class: bot.class, team: bot.team, items: bot.equippedItems })),
        models: botParts.filter(pose => pose.wearable).map(pose => ({ actor: pose.identity - BOT_MODEL_IDENTITY_BASE, model: pose.model, item: pose.wearable!.itemId, controlPoint: [...pose.wearable!.controlPoint] })),
        particles: particleItems.filter(item => item.effectIdentity >= 0x6000_0000 && item.effectIdentity < 0x7000_0000),
      }
      const audioStart=performance.now();this.#playAudio(snapshot, camera);const audioMilliseconds=performance.now()-audioStart
      if(this.#paused||!ownsGeneration())return
      const frame=Object.freeze({
        effects: Object.freeze([]),
        particles: particleItems,
        combatDecals:snapshot.combatDecals,
        maximumCombatDecals:Number(this.#settings?.snapshot().settings.current.mp_decals??200),
        models: Object.freeze([
          ...projectileModels(presentation.models),
          ...lockerPoses.map(pose=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===pose.identity)!;return Object.freeze({identity:pose.identity,model:pose.model,position:occurrence.origin,angles:occurrence.angles,scale:1,skin:occurrence.skin,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}),
          ...studioPoses.map(pose => {
            const occurrence = this.#artifacts!.modelOccurrences.find(value => value.entity === pose.identity)!
            return Object.freeze({ identity: pose.identity, model: pose.model, ...studioModelFrameState(snapshot.entityPresentation, pose.identity), scale: 1, body: occurrence.body, pose, modelLighting: pose.lighting!, eyeStates: pose.eyes })
          }),
          ...botParts.filter(pose=>botSelection.drawn.has(pose.identity-BOT_MODEL_IDENTITY_BASE)).map(pose=>{const bot=snapshot.bots.find(value=>BOT_MODEL_IDENTITY_BASE+value.identity===pose.identity);if(!bot)throw new Error("TF2 bot player pose identity is unavailable");return Object.freeze({identity:pose.identity+(pose.wearable?0x20000+pose.wearable.itemId*0x10000:pose.role==="item"?0x10000:0),model:pose.model,position:bot.position,angles:Object.freeze([0,bot.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:bot.team===2||(this.#artifacts!.models.get(pose.model)?.skinCount??0)<2?0:1,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}),
          ...controlPointPoses.map(pose=>{const point=snapshot.controlPoints!.points.find(point=>OBJECTIVE_MODEL_IDENTITY_BASE+point.identity===pose.identity)!;return Object.freeze({identity:pose.identity,model:pose.model,position:point.position,angles:point.angles,scale:1,skin:point.skin,body:point.body,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}),
          ...objectivePoses.map(pose=>{const flag=snapshot.objectives?.flags.find(value=>OBJECTIVE_MODEL_IDENTITY_BASE+value.identity===pose.identity);if(!flag)throw new Error("TF2 intelligence pose identity is unavailable");const carrier=flag.carrier===null?undefined:snapshot.bots.find(bot=>bot.identity===flag.carrier);if(carrier){const carrierPose=botPoses.find(value=>value.identity===BOT_MODEL_IDENTITY_BASE+carrier.identity);const attachment=carrierPose?.attachments.find(value=>value.name.toLowerCase()==="flag");if(!attachment)throw new Error(`Authored TF2 flag attachment unavailable: ${carrier.identity}`);const transform=transformAttachment(attachment.matrix,carrier.position,sourceViewOrientation(0,carrier.yawDegrees));return Object.freeze({identity:pose.identity,model:pose.model,position:transform.position,orientation:transform.orientation,scale:1,skin:flag.skin,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}return Object.freeze({identity:pose.identity,model:pose.model,position:flag.position,angles:flag.angles,scale:1,skin:flag.skin,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}),
          ...buildingPoses.map(pose=>{const building=snapshot.buildings.find(value=>value.identity===pose.identity);if(!building)throw new Error("TF2 building pose identity is unavailable");return Object.freeze({identity:pose.identity,model:pose.model,position:building.position,angles:Object.freeze([0,building.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:building.team===2||(this.#artifacts!.models.get(pose.model)?.skinCount??0)<2?0:1,pose,modelLighting:pose.lighting!,eyeStates:pose.eyes})}),
          ...(blueprintPose&&snapshot.placement?[Object.freeze({identity:blueprintPose.identity,model:blueprintPose.model,position:snapshot.placement.position,angles:Object.freeze([0,snapshot.placement.yawDegrees,0]) as readonly[number,number,number],scale:1,skin:snapshot.team===2||(this.#artifacts!.models.get(blueprintPose.model)?.skinCount??0)<2?0:1,pose:blueprintPose,modelLighting:blueprintPose.lighting!,eyeStates:blueprintPose.eyes})]:[]),
          ...viewmodelPoses.map((pose, index) => Object.freeze({
            ...viewmodel!.item,
            identity: viewmodel!.item.identity + index,
            model: pose.model,
            skin: viewmodel!.item.skin < (this.#artifacts!.models.get(pose.model)?.skinCount ?? 0) ? viewmodel!.item.skin : 0,
            position:pose.viewmodel!.transform.origin,angles:pose.viewmodel!.transform.angles,
            viewModelProjection:Object.freeze({kind:"viewmodel" as const,horizontalFov4By3:pose.viewmodel!.projection.unscaledHorizontalFov4By3,near:pose.viewmodel!.projection.near,depthRange:pose.viewmodel!.depthRange,drawsAfterWorld:true,opaqueBeforeTranslucent:true,optionalViewSpaceYReflection:pose.viewmodel!.reflected}),
            pose,
            modelLighting: pose.lighting!,
            eyeStates: pose.eyes,
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
            modelLighting: watchPose.lighting!,
            eyeStates: watchPose.eyes,
          })] : []),
        ]),
        brushModels: snapshot.entityPresentation,
        studioModels: snapshot.entityPresentation.studioModels,
        modelVisibility: new Map(snapshot.pickups.map((pickup) => [pickup.identity, pickup.available])),
        collisionWorldIdentity: snapshot.collisionSnapshot.worldIdentity,
      }) satisfies Omit<Frame,"camera"|"visibility"|"deltaSeconds">
      this.#combatDecalCount+=snapshot.combatDecals.length
      this.#preparedRevision+=1
      const prepared=Object.freeze({
        generation,
        revision:this.#preparedRevision,
        viewportRevision:viewport.revision,
        snapshot,
        publication,
        visibility,
        skyVisibility,
        presentedCamera:preparedCamera,
        frame,
        modelMilliseconds,
        projectileMilliseconds,
        particleMilliseconds,
        particleDecodeMilliseconds,
        audioMilliseconds,
        particleOutputBytes:particleOutput.byteLength,
        hudModelPoses,
      })
      this.#requiredParticleDisplayFrames.admit(prepared, particleItems.map(item=>item.effectIdentity))
      this.#preparedPresentation=prepared
      if(snapshot.class===9&&!this.#engineer){if(!this.#uiResources||!this.#presentationRandom)throw new Error("TF2 Engineer presentation resources are unavailable");this.#engineer=initializeTf2EngineerPresentation({root:this.#engineerRoot,resources:this.#uiResources,viewport:this.#viewport(),clock:{nowSeconds:()=>this.#frameClock.current},random:this.#presentationRandom,reducedMotion:matchMedia("(prefers-reduced-motion: reduce)").matches,lookupBinding:action=>this.#bindings.lookupBinding(action)})}
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
      const scoreboard = hud?.scoreboard.kind === "available" ? hud.scoreboard.value : undefined
      if (scoreboard !== this.#publishedScoreboard) {
        this.#publishedScoreboard = scoreboard
        this.#publishedScoreboardProbe = scoreboard ? JSON.stringify({
          map: scoreboard.mapName, red: scoreboard.red, blue: scoreboard.blue,
          players: scoreboard.players.map((player) => ({
            identity: player.identity, name: player.name, team: player.team,
            class: player.class.kind === "available" ? player.class.value : null,
            score: player.score, alive: player.alive,
            ping: player.ping.kind === "available" ? player.ping.value : null,
            kills: player.counters.kind === "available" ? player.counters.value.kills : null,
            deaths: player.counters.kind === "available" ? player.counters.value.deaths : null,
            captures: player.counters.kind === "available" ? player.counters.value.captures : null,
            damage: player.counters.kind === "available" ? player.counters.value.damage : null,
          })), spectators: scoreboard.spectators,
        }) : "unavailable"
      }
      this.#set({
        hudProbe: hudPlayer ? `${hudHealth}:${hudPlayer.class.kind === "available" ? hudPlayer.class.value : "unavailable"}:${hudWeaponIdentity ?? "unavailable"}:${hudWeapon?.clip.kind === "available" ? hudWeapon.clip.value : "unavailable"}:${hudWeapon?.reserve.kind === "available" ? hudWeapon.reserve.value : "unavailable"}` : "unavailable",
        hudAnimationTrace: hudProbe?.animationTrace.join("|"),
        hudOperationProbe: healthPanel && ammoPanel && weaponPanel
          ? `${healthPanel.state.imageFill}:${healthPanel.bounds.x},${healthPanel.bounds.y},${healthPanel.bounds.width},${healthPanel.bounds.height}:${healthPanel.state.drawColor.join(",")}:${healthPanel.state.foregroundColor?.join(",") ?? "none"}:${ammoPanel.state.scalarProperties.reloadPhase ?? "none"}:${weaponPanel.state.scalarProperties.weaponIdentity ?? "none"}`
          : "unavailable",
        hudPresentationProbe: hudProbe && hud ? this.#hudPresentationObservation(hudProbe, hud) : "unavailable",
        scoreboardVisible: this.#scoreboardVisible,
        scoreboardProbe: this.#publishedScoreboardProbe,
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
        medigunCharge: snapshot.medigunCharge,
        medigunTarget: snapshot.medigunTarget,
        medigunReleasing: snapshot.medigunReleasing,
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
        teamSelectionLocal: snapshot.team,
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
        crouchHistory: this.#view.crouchHistory?.length === this.#crouchHistory.length
          && this.#view.crouchHistory.at(-1) === this.#crouchHistory.at(-1)
          ? this.#view.crouchHistory
          : Object.freeze([...this.#crouchHistory]),
        viewmodelTimelineProbes: this.#view.viewmodelTimelineProbes,
        ...this.#gameplayTraces(snapshot),
        ...this.#snapshotProbes(snapshot),
        simulationProbe: `${publication.hostFrame}:${publication.firstHostTick}-${publication.lastHostTick}:${publication.selectedTicks}:${publication.snapshotByteLength}:${publication.eventBatches.reduce((n,e)=>n+e.byteLength,0)}`,
        brushModelProbe: `${snapshot.entityPresentation.entityRevision}:${snapshot.entityPresentation.collisionRevision}:${snapshot.entityPresentation.models.length}:${snapshot.entityPresentation.models.filter(model=>model.draw).length}`,
        reloadHistory: this.#view.reloadHistory?.length === this.#reloadHistory.length
          && this.#view.reloadHistory.at(-1) === this.#reloadHistory.at(-1)
          ? this.#view.reloadHistory
          : Object.freeze([...this.#reloadHistory]),
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
      this.#set({ phase: "Failed", gameUi: "failure", detail: this.#failureDetail(error, "Gameplay frame failed") })
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

  #activateBoundAction(identity: string, action: string, repeat = false): boolean {
    if (action === "+forward" || action === "+back" || action === "+moveleft" || action === "+moveright" || action === "+duck") {
      if (!repeat) this.#buttons.press(identity, action)
    } else if (action === "+jump") {
      if (!repeat && this.#buttons.press(identity, action)) this.#jumpPressed = true
    } else if (action === "+attack") {
      if (!repeat && this.#buttons.press(identity, action)) this.#firePressed = true
    } else if (action === "+attack2") {
      if (!repeat) {
        if(this.#snapshot?.placement){this.#buildingRequest={action:"rotate"};return true}
        if (this.#buttons.press(identity, action)) this.#detonatePressed = true
      }
    } else if (action === "+reload") {
      if (!repeat && this.#buttons.press(identity, action)) this.#reloadPressed = true
    } else if (action === "+showscores") {
      if (!repeat && this.#buttons.action(identity) === undefined) {
        this.#buttons.press(identity, action)
        this.#setScoreboardVisible(true)
      }
    } else if (action === "lastinv") {
      if (!repeat) this.#selectWeapon = "last"
    } else if (/^slot[1-6]$/.test(action) && this.#snapshot && this.#equipmentProfile?.state()) {
      if (!repeat) this.#selectWeapon = equippedWeaponSlots(this.#snapshot, this.#equipmentProfile.state()!.inventory)
        .find(value => value.slot === Number(action.slice(4)) - 1)?.weapon
    } else return false
    return true
  }

  #gameplayKeyboardTarget(target: Node | null): boolean {
    return this.#view.gameUi === "in-game" && !this.#equipment?.visible() && !this.#view.consoleVisible
      && !this.#view.optionsVisible && !this.#view.localMatchVisible && !this.#teamSelection?.state().visible
      && !this.#classSelection?.state().visible
      && (target === this.#canvas || target === document.body || target === document.documentElement || target === this.#presentationRoot)
  }

  readonly #keyDown = (event: KeyboardEvent): void => {
    if (event.isComposing || event.keyCode === 229 || event.metaKey || !document.hasFocus()) return
    const target = event.target as Node | null
    if (target !== document.body && target !== document.documentElement && (!target || !this.#presentationRoot.contains(target))) return
    if (browserOwnsKey(event)) {
      // Preserve physical bindings (for example CTRL-duck + W-forward), but
      // never cancel browser-reserved chords. A browser focus/lock loss still
      // neutralizes the game; this is not a promise to capture such shortcuts.
      if (this.#gameplayKeyboardTarget(target)) {
        const action = this.#keyboardAction(event)
        if (action) this.#activateBoundAction(`keyboard:${event.code}`, action, event.repeat)
      }
      return
    }
    if (this.#localMatch?.handleKey(event)) return
    if (!this.#view.consoleVisible && this.#equipment?.handleKey(event)) return
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
          this.#suspendAudio()
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
    if (this.#keyboardAction(event) === "toggleconsole") {
      if (!this.#consoleEnabled
        || (this.#vguiRoot.contains(event.target as Node) && !this.#view.consoleVisible)
        || this.#optionsRoot.contains(event.target as Node)) return
      event.preventDefault()
      event.stopImmediatePropagation()
      if (!event.repeat) this.toggleConsole()
      return
    }
    // VGUI controls retain their own editing/navigation even if a control keeps
    // focus during a transition. The world and the unfocused body are gameplay.
    if (!this.#gameplayKeyboardTarget(target)) return
    if (this.#snapshot?.class === 8 && this.#snapshot.weapon === 53 && /^Digit[1-9]$/u.test(event.code)) {
      event.preventDefault()
      if (event.repeat) return
      const classes: readonly Tf2Class[] = [1, 3, 7, 4, 6, 9, 5, 2, 8]
      const selected = classes[Number(event.code.slice(5)) - 1]!
      this.#disguise = Object.freeze({ class: selected, team: this.#snapshot.team === 2 ? 3 : 2 })
      this.#selectWeapon = 50
      event.preventDefault()
      return
    }
    const action = this.#keyboardAction(event)
    if (!action) return
    if (action === "changeteam") {
      event.preventDefault()
      if (!event.repeat) void this.#showTeamSelection()
      return
    }
    if(this.#engineer?.menu()&&/^slot[1-4]$/.test(action)){
      event.preventDefault()
      if (event.repeat) return
      const request=this.#engineer.select(Number(action.slice(4)))
      if(request){this.#buildingRequest=request;if(request.action==="destroy")this.#selectWeapon=42}
      return
    }
    if (action === "changeclass") {
      event.preventDefault()
      if (!event.repeat) this.#showClassSelection()
      return
    }
    if (this.#activateBoundAction(`keyboard:${event.code}`, action, event.repeat)) {
      event.preventDefault()
      if (!event.repeat) void this.resumeAudio()
    }
  }

  readonly #keyUp = (event: KeyboardEvent): void => {
    this.#releaseBoundAction(`keyboard:${event.code}`)
  }

  readonly #mouseDown = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    void this.resumeAudio()
    const action = this.#mouseAction(event)
    if (action) this.#activateBoundAction(`mouse:${event.button}`, action)
  }

  readonly #mouseUp = (event: MouseEvent): void => {
    this.#releaseBoundAction(`mouse:${event.button}`)
  }

  #releaseBoundAction(identity: string): void {
    const action = this.#buttons.action(identity)
    this.#buttons.release(identity)
    // IN_ScoreUp hides the panel on each -showscores command, independently
    // of KeyUp retaining another physical source in in_score.
    if (action === "+showscores") this.#setScoreboardVisible(false)
  }

  readonly #mouseMove = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    if(event.movementX===0&&event.movementY===0)return
    const scale = this.#mouseSensitivity / 3
    const movementX=event.movementX*scale
    const offset = this.#snapshot?.viewAngleOffset ?? [0, 0, 0]
    const angles = applyPointerDelta(this.#yaw + offset[1]!, this.#pitch + offset[0]!, movementX, event.movementY * scale * (this.#reverseMouse ? -1 : 1))
    this.#yaw = angles.yaw - offset[1]!
    this.#pitch = angles.pitch - offset[0]!
    this.#pointerMovementX+=movementX
    this.#viewRevision+=1
    this.#mouseViewRevision+=1
    const profiler=browserFrameProfiler() as ReturnType<typeof browserFrameProfiler>&{input?:{at:number;revision:number;kind:string}[]}
    if(profiler?.active)profiler.input?.push({at:performance.now(),revision:this.#mouseViewRevision,kind:"mouse"})
  }

  readonly #blur = (): void => this.#neutral()
  readonly #operationVisibility = (): void => {
    this.#operationWatchdog.visibility(!document.hidden)
    this.#publishOperationWatchdog()
  }
  #publishOperationWatchdog(): void {
    this.#presentationRoot.dataset.operationWatchdog = JSON.stringify(this.#operationWatchdog.snapshot())
  }
  readonly #visibility = (): void => {
    this.#paused = document.hidden || this.#view.gameUi === "pause"
    if (this.#paused) this.#suspendAudio()
    else void this.resumeAudio()
    this.#predictedEye.suspend()
    this.#neutral()
    const nowSeconds=this.#frameClock.admit(performance.now()/1_000)
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
    this.#selectWeapon = undefined
    this.#disguise = undefined
    this.#modeRequest = undefined
  }

  #publishProfileCoverage():void{
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    if(!profile)return
    profile.coverageSamples=this.#coverageSamples
    if(this.#artifacts){
      const authored = new Map([
        ...this.#artifacts.environment.authoredTextures,
        ...this.#artifacts.authoredTextures,
        ...this.#artifacts.legacyVisualTextures.flatMap(texture=>[[texture.logicalPath,texture] as const,...texture.normal?[[texture.normal.logicalPath,texture.normal] as const]:[]]),
      ])
      let planes = 0, planeBytes = 0, mipLevels = 0, frames = 0, compressed = 0, compressedBytes = 0,decodedCompressedTailBytes=0
      for (const texture of authored.values()) {
        mipLevels += texture.mipCount
        frames += texture.frameCount
        if ([13, 14, 15, 20].includes(texture.sourceFormat ?? -1)) compressed += 1
        for (const plane of texture.planes) {
          planes += 1
          planeBytes += plane.rgba.byteLength
          decodedCompressedTailBytes+=plane.decodedRgba?.byteLength??0
          if ([13, 14, 15, 20].includes(texture.sourceFormat ?? -1)) compressedBytes += plane.rgba.byteLength
        }
      }
      profile.memoryAssets = Object.freeze({
        compileWasmLinearMemoryBytes: this.#loaded?.timings.wasmLinearMemoryBytes,
        compileWasmAllocatorLiveBytes: this.#loaded?.timings.wasmAllocatorLiveBytes,
        compileWasmAllocatorHighWaterBytes: this.#loaded?.timings.wasmAllocatorHighWaterBytes,
        generation: this.#generation,
        target: this.#mapIdentity,
        resourceSections: this.#dependencies?.sections.length ?? 0,
        resourceBytes: this.#dependencies?.byteLength ?? 0,
        presentationBytes: this.#loaded?.presentation.byteLength ?? 0,
        mapBytes: this.#loaded?.payload.byteLength ?? 0,
        modelCount: this.#artifacts.models.size,
        modelOccurrences: this.#artifacts.modelOccurrences.length,
        staticProps: this.#artifacts.staticProps.count,
        textures: authored.size,
        planes,
        planeBytes,
        mipLevels,
        frames,
        compressedTextures: compressed,
        compressedPlaneBytes: compressedBytes,
        decodedCompressedTailBytes,
        directionalBytes: this.#artifacts.directionalTextures.reduce((total, texture) => total
          + (texture.authored.planes.find((plane) => plane.mip === 0 && plane.frame === 0 && plane.face === 0 && plane.slice === 0)?.rgba.byteLength ?? 0), 0),
        particleBytes: this.#artifacts.particleTextures.reduce((total, texture) => total + texture.planes.reduce((bytes, plane) => bytes + plane.rgba.byteLength, 0), 0),
      })
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
    if (!pointerLockRequestRequired(document.pointerLockElement, connected)) return
    if (this.#closed || this.#equipment?.visible() || this.#view.consoleVisible || this.#classSelection?.state().visible || this.#teamSelection?.state().visible) return
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

  #suspendAudio(): void {
    const context = this.#audioContext
    if (!context || context.state !== "running") return
    this.#audioRunning = false
    // The gameplay context is separate from menu audio. Freeze its clock and
    // authored envelopes/tails while the simulation is paused, not its voices.
    void context.suspend().then(() => {
      if (!this.#paused && context === this.#audioContext) void this.resumeAudio()
    }).catch(error => {
      this.#blockers.add(`AudioUnavailable: ${error instanceof Error ? error.message : "suspend failed"}`)
    })
  }

  async resumeAudio(): Promise<void> {
    if (!this.#audio || this.#closed || this.#paused) return
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
    this.#recordedClientFrames?.close()
    if (this.#closed) return
    await this.#release()
    this.#set({ phase: "Closed", detail: "Application closed", pointerLocked: false, pointerMovement:undefined, consoleVisible: false })
  }

  async #teardownGameplay(): Promise<void> {
    this.#equipmentAdmissionEpoch += 1
    if (this.#equipment?.visible()) this.#equipment.hide()
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
    await this.#equipmentAdmissionTask?.catch(() => {})
    await Promise.all([this.#displayTask, this.#classSelectionRenderTask, this.#teamSelectionRenderTask, this.#equipmentRenderTask])
    await this.#renderer?.dispose().catch(() => {})
    this.#renderer = undefined
    await this.#releaseEquipmentAdmissions()
    await this.#resourceRuntime?.catch(() => {})
    this.#resourceRuntime = undefined
    await this.#client?.shutdown().catch(() => {})
    this.#equipmentProfile?.close()
    this.#equipmentProfile = undefined
    this.#client = undefined
    this.#cache?.close()
    this.#cache = undefined
    this.#openingCache = undefined
    this.#projectiles?.dispose()
    this.#projectiles = undefined
    if (this.#audio) await this.#audio.close().catch(() => {})
    else await this.#audioContext?.close().catch(() => {})
    const audioProfile = (globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile
    if (audioProfile) delete audioProfile.audio
    this.#audio = undefined
    this.#audioContext = undefined
    this.#audioRegistry = undefined
    this.#audioWorld = undefined
    this.#audioBuffers = new Map()
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
    this.#dependencies = undefined
    this.#dependencyEntries.clear()
    this.#snapshot = undefined
    this.#artifacts = undefined
    this.#mapArtifacts = undefined
    this.#viewmodels = undefined
    this.#viewmodelClass = undefined
    this.#attachments.clear()
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
    this.#botRequest = undefined
    this.#botControl = undefined
    this.#botConfiguration = undefined
    this.#objectiveConfiguration = undefined
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
    document.removeEventListener("visibilitychange", this.#operationVisibility)
    this.#operationWatchdog.cancel()
    this.#publishOperationWatchdog()
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
