import { fetchImmutableObject, openDerivedObjectCache, type DerivedObjectCache } from "@playsrc/asset-store/browser"
import { chunksForRole, encodeResourceBatch, parseResourceCatalogBytes, parseResourceGraphBytes, parseResourceSet, resourceChunkObject, selectCatalogTarget, type ResourceCatalog, type ResourceGraph, type ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { createAudioSystem, SoundRegistry, SourceAudioWorld } from "@playsrc/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { Tf2WorkerClient, mergePublicationSnapshots, type CoverageSample, type LoadedGame, type SimulationPublication, type VisibilityResult } from "@playsrc/game-tf2-browser"
import { initializeTf2GameUiIntegration, type Tf2GameUiIntegration } from "@playsrc/game-tf2-browser/gameui-integration"
import type { Tf2GameUiRequest, Tf2LoadingPhase } from "@playsrc/game-tf2-browser/gameui"
import { initializeTf2HudIntegration, type Tf2HudIntegration } from "@playsrc/game-tf2-browser/hud-integration"
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
  tf2CrosshairHudValues,
  tf2CrosshairSettings,
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
import { encodeCommand, mapDerivedKey, type Snapshot, type Tf2Class, type Tf2Team } from "@playsrc/game-tf2-browser/codec"
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
  sourceViewOrientation,
  tf2Audio,
  tf2Camera,
  transformAttachment,
  type ModelPoseRequest,
  type PosedModel,
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
import { PhysicalBindingIndex, PhysicalButtonState, applyPointerDelta, rawPointerMovementUnsupported, rebasePointerYaw, type PhysicalBinding } from "./input"
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
const SOUND_PATHS = [
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
  optionsVisible?: boolean
  settingsPersistence?: "absent" | "loaded" | "rejected" | "stored"
  settingsApply?: string
  hostRequest?: "quit"
  presentationRandomState?: string
  presentationCharacter?: string
  cache?: "hit" | "stored"
  pointerLocked: boolean
  pointerMovement?: "raw" | "adjusted"
  consoleVisible: boolean
  blockers: readonly string[]
  fireEvents: number
  explosionEvents: number
  camera?: Camera
  initialView?: LoadedGame["initialView"]
  environment?: PresentationArtifacts["environment"]
  particleRenderItems?: number
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
  reloadHistory?: readonly string[]
  fireTickHistory?: readonly string[]
  performanceProbe?: string
  performanceDetailProbe?: string
  loadPerformanceProbe?: string
  displayFrame?: number
  displayViewRevision?: number
  displayPreparedRevision?: number
  lockerProbe?: string
  unsupportedState?: "StickyPhysicsSolverUnavailable"
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
  readonly #optionsRoot: HTMLElement
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
  #attachments = new Map<number, ReadonlySet<string>>()
  #attachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #fireAttachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #particleBatches = createParticleBatchEncoder()
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
  #hudRootCounts?: Readonly<{ playerStatus: number; ammo: number }>
  #hudContext?: SessionHudContext
  #hudContextIdentity = -1
  #playerClassUsePlayerModel = true
  #crosshairSettings?: Tf2CrosshairSettings
  #settings?: Tf2BrowserSettings
  #options?: Tf2OptionsPresentation
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
  #selectClass: Tf2Class | undefined
  #selectTeam: Tf2Team | undefined
  #selectWeapon: 1 | 2 | 3 | undefined
  #modeRequest: 0 | 1 | undefined
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
    roots: Readonly<{ vgui: HTMLElement; gameUi: HTMLElement; hud: HTMLElement; options: HTMLElement; loading: HTMLElement; startup: HTMLElement; startupVideo: HTMLVideoElement }>,
    publish: (view: ApplicationView) => void,
  ) {
    this.#canvas = canvas
    const presentationRoot = canvas.parentElement
    if (!presentationRoot || [roots.vgui, roots.gameUi, roots.hud, roots.options, roots.loading, roots.startup].some((root) => root.parentElement !== presentationRoot)) {
      throw new Error("TF2 presentation owners do not share one application mount")
    }
    this.#presentationRoot = presentationRoot
    this.#vguiRoot = roots.vgui
    this.#gameUiRoot = roots.gameUi
    this.#hudRoot = roots.hud
    this.#optionsRoot = roots.options
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
        || this.#view.optionsVisible !== prior.optionsVisible)) {
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
    const result = resolveTf2LoadingBackground({
      generation: this.#loadingPresentationGeneration,
      mapIdentity: target.target,
      viewport: this.#viewport(),
      mapPhotoLookups: target.loading.mapPhotoLocations.map((location) => Object.freeze({ location, outcome: "missing" as const })),
      backingMaterial: target.loading.stampBackground.material,
      backingTexture: target.loading.stampBackground.texture,
    })
    if (!result.ok) throw new Error(`${result.code}:${result.subject}`)
    this.#loadingBackground = result
    this.#syncLoadingPresentation()
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
        && (change.settingId !== "cl_hud_playerclass_use_playermodel" || typeof change.nextValue !== "boolean"))) {
        return reject(`browser game owner does not implement every requested effect: ${request.changes.map((change) => `${change.settingId}=${String(change.nextValue)}`).join(",")}`)
      }
      const model = request.changes.find((change) => change.settingId === "cl_hud_playerclass_use_playermodel")
      if (model) {
        this.#playerClassUsePlayerModel = model.nextValue as boolean
        this.#hudIntegration?.setPlayerClassUsePlayerModel(this.#playerClassUsePlayerModel)
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
    const set = await this.#client!.decodeResources(encodeResourceBatch(records))
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
    this.#hudRootCounts = undefined
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    this.#options?.destroy()
    this.#options = undefined
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
          systems: new Set([
            "rockettrail",
            "rocketbackblast",
            "stickybombtrail_red",
            "stickybombtrail_blue",
            "stickybomb_pulse_red",
            "stickybomb_pulse_blue",
            "muzzle_pipelauncher",
            "ExplosionCore_Wall",
            "ExplosionCore_MidAir",
          ]),
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
      const audioResources = await Promise.all(
        SOUND_PATHS.map(async (identity) => {
          const bytes = this.#dependencyEntries.get(identity)
          if (!bytes) throw new Error(`Audio dependency ${identity} is missing`)
          const buffer = await audioContext.decodeAudioData(bytes.slice().buffer)
          return Object.freeze({ identity, buffer })
        }),
      )
      this.#audio = createAudioSystem(audioContext, audioResources)
      this.#audioContext = audioContext
      this.#audioBuffers = new Map(audioResources.map((resource) => [resource.identity, resource.buffer]))
      this.#audioRegistry = new SoundRegistry([
        Object.freeze({
          logicalPath: this.#artifacts.audio.logicalPath,
          mode: "base" as const,
          preload: false,
          entries: this.#artifacts.audio.entries,
        }),
      ])
      this.#audioWorld = new SourceAudioWorld(this.#audioRegistry, { maxActiveVoices: 128 })
      finishLoadPhase("audioSetup")
      this.#requireOperation(operation)
      await this.#client!.activate(this.#generation)
      finishLoadPhase("activation")
      this.#advanceLoading("synchronizing-game-state")
      this.#snapshot = (await this.#initialPublication(this.#generation)).snapshot
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
      this.#gameUi?.dispatch({ kind: "loading-progress", phase: "complete" })
      this.#gameUi?.dispatch({ kind: "loading-succeeded" })
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
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
  }

  #resetHudIntegration(): void {
    if (!this.#uiResources || !this.#presentationRandom) throw new Error("TF2 HUD resources are unavailable")
    this.#hudContext = undefined
    this.#hudContextIdentity = -1
    if (this.#hudIntegration) {
      this.#hudIntegration.reset("map-replaced")
      return
    }
    this.#hudIntegration = initializeTf2HudIntegration({
      root: this.#hudRoot,
      resources: this.#uiResources,
      viewport: this.#viewport(),
      reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
      clock: { nowSeconds: () => this.#frameClock.current },
      random: this.#presentationRandom,
      onCommand: (command) => {
        if (command.kind === "select-weapon" && command.weapon >= 1 && command.weapon <= 3) this.#selectWeapon = command.weapon as 1 | 2 | 3
      },
    })
    const panels = this.#hudIntegration.snapshot().vgui.panels
    this.#hudRootCounts = Object.freeze({
      playerStatus: panels.filter((panel) => panel.name === "HudPlayerStatus").length,
      ammo: panels.filter((panel) => panel.name === "HudWeaponAmmo").length,
    })
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
    const vguiInput = this.#view.consoleVisible || this.#view.optionsVisible === true
    const identity = Number(respawnAllowed)
      | Number(paused) << 1
      | Number(vguiInput) << 2
      | Number(this.#playerClassUsePlayerModel) << 3
      | Number(loadingImage) << 4
      | Number(clientModeAllows) << 5
      | Number(snapshot.weapon !== null) << 6
    if (this.#hudContext && this.#hudContextIdentity === identity) return this.#hudContext
    this.#hudContextIdentity = identity
    this.#hudContext = Object.freeze({
      playerIdentity: 1,
      liveHudSuppressed: false,
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
      scoreboard: tf2HudUnavailable<Tf2HudScoreboard>("not-produced"),
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
          name: "jointeam",
          disposition: "visible" as const,
          acceptsSuggestions: true,
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
    for (const owner of [this.#canvas, this.#startupRoot, this.#loadingRoot, this.#gameUiRoot, this.#hudRoot, this.#optionsRoot, this.#vguiRoot]) {
      owner.dataset.presentationViewport = identity
      owner.dataset.presentationViewportState = "active"
    }
    this.#resizeRenderer(viewport)
    this.#console?.apply({ kind: "set-viewport", viewport })
    this.#diagnostics?.apply({ kind: "set-viewport", viewport })
    this.#gameUi?.setViewport(viewport)
    this.#syncGameUiBackgroundProbe()
    this.#hudIntegration?.setViewport(viewport)
    this.#options?.setViewport(viewport)
    this.#loadingVgui?.setViewport(viewport)
    if (this.#loadingPresentationGeneration > 0 && this.#configuration) {
      const target = this.#loadingTarget ?? this.#activeTarget
      if (!target) throw new Error("TF2 loading target is unavailable")
      const result = resolveTf2LoadingBackground({
        generation: this.#loadingPresentationGeneration,
        mapIdentity: target.target,
        viewport,
        mapPhotoLookups: target.loading.mapPhotoLocations.map((location) => Object.freeze({ location, outcome: "missing" as const })),
        backingMaterial: target.loading.stampBackground.material,
        backingTexture: target.loading.stampBackground.texture,
      })
      if (result.ok) this.#loadingBackground = result
      this.#syncLoadingPresentation(viewport)
    }
  }

  #suspendPresentationViewport(): void {
    this.#presentationViewport = undefined
    for (const owner of [this.#canvas, this.#startupRoot, this.#loadingRoot, this.#gameUiRoot, this.#hudRoot, this.#optionsRoot, this.#vguiRoot]) {
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
              ? ["jointeam red", "jointeam blue"]
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
    if (command === "jointeam" && tokens.length === 1) {
      const team = tokens[0]!.toLowerCase()
      if (team !== "red" && team !== "blue") {
        this.#output("Usage: jointeam red|blue")
        return
      }
      this.#selectTeam = team === "red" ? 2 : 3
      this.#output(`Team selection queued: ${tokens[0]}`)
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
        systems: new Set([
          "rockettrail",
          "rocketbackblast",
          "stickybombtrail_red",
          "stickybombtrail_blue",
          "stickybomb_pulse_red",
          "stickybomb_pulse_blue",
          "muzzle_pipelauncher",
          "ExplosionCore_Wall",
          "ExplosionCore_MidAir",
        ]),
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

  #gameplayTraces(snapshot: Snapshot): Pick<ApplicationView, "weaponTrace" | "authorityTrace" | "entityTrace"> {
    return Object.freeze({
      weaponTrace: snapshot.loadout.map((weapon) =>
        `${weapon.weapon}:${weapon.clip}/${weapon.reserve}:${weapon.reload}:${weapon.reloadDueTick ?? "-"}:${weapon.chargeBeginTick ?? "-"}:${weapon.firstPrimaryTick}`,
      ).join("|"),
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
    if (!this.#audioRunning || !this.#audio || !this.#audioWorld || !this.#audioRegistry || !this.#audioContext || !this.#artifacts) return
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
    for (const request of tf2Audio(snapshot)) {
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

  #command(): ArrayBuffer {
    const forward = Number(this.#buttons.held("+forward")) - Number(this.#buttons.held("+back"))
    const side = Number(this.#buttons.held("+moveleft")) - Number(this.#buttons.held("+moveright"))
    const unsupportedSticky = this.#snapshot?.class === 4 && (this.#buttons.held("+attack") || this.#firePressed)
    if (unsupportedSticky) { this.#blockers.add("Missing exact IVP sticky rigid-body solver: launch is rejected before projectile creation"); this.#set({unsupportedState:"StickyPhysicsSolverUnavailable"}) }
    const command = encodeCommand({
      forward: forward * 450,
      side: side * 450,
      yawDegrees: this.#yaw,
      pitchDegrees: this.#pitch,
      jump: this.#buttons.held("+jump") || this.#jumpPressed,
      crouch: this.#buttons.held("+duck"),
      fire: !unsupportedSticky && (this.#buttons.held("+attack") || this.#firePressed),
      detonate: this.#buttons.held("+attack2") || this.#detonatePressed,
      reload: this.#buttons.held("+reload") || this.#reloadPressed,
      selectClass: this.#selectClass,
      selectTeam: this.#selectTeam,
      selectWeapon: this.#selectWeapon,
      modeRequest: this.#modeRequest,
    })
    this.#selectClass = undefined
    this.#selectTeam = undefined
    this.#selectWeapon = undefined
    this.#modeRequest = undefined
    this.#jumpPressed = false
    this.#firePressed = false
    this.#detonatePressed = false
    this.#reloadPressed = false
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
      if (owners & HUD_FRAME_OWNER) this.#hudIntegration?.frame(timeSeconds)
    } catch (error) {
      this.#paused = true
      this.#set({ phase: "Failed", gameUi: "failure", detail: error instanceof Error ? error.message : "VGUI frame failed" })
      return
    }
    if (!this.#paused && this.#snapshot && (this.#showFps !== 0 || this.#showPos !== 0)) this.#updateDiagnostics(time)
    if (this.#paused || !this.#client || !this.#renderer || !this.#snapshot) return
    const nowSeconds=timeSeconds
    if(nowSeconds+Number.EPSILON>=this.#nextSimulationSampleSeconds){
      this.#scheduleSimulation(nowSeconds, false)
      if(this.#nextSimulationSampleSeconds===0)this.#nextSimulationSampleSeconds=nowSeconds+SIMULATION_SAMPLE_INTERVAL_SECONDS
      else do{this.#nextSimulationSampleSeconds+=SIMULATION_SAMPLE_INTERVAL_SECONDS}while(this.#nextSimulationSampleSeconds<=nowSeconds)
    }
    this.#offerDisplay()
  }

  #offerDisplay():void{
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
    }
    const geometryEvidenceRevision=profile?.geometryEvidenceRevision
    if(profile&&Number.isSafeInteger(geometryEvidenceRevision)&&geometryEvidenceRevision!==((profile.geometryEvidence as {revision?:unknown}|undefined)?.revision)&&this.#view.phase==="Ready"){
      profile.geometryEvidence=Object.freeze({revision:geometryEvidenceRevision,generation,target:this.#mapIdentity,finalReady:true,identities:Object.freeze({bsp:this.#activeTarget?.objects.bsp.sha256,resourceRoot:this.#activeTarget?.objects.resources.sha256,contentBuild:this.#resourceGraph?.contentBuild,graphTarget:this.#resourceGraph?.target,wasm:this.#configuration?.wasm.sha256,simulationTick:prepared.snapshot.tick.toString()}),camera,visibility:Object.freeze({outsideWorld:visibility.outsideWorld,eyeLeaf:visibility.eyeLeaf,leaves:Object.freeze([...visibility.leaves]),areas:Object.freeze([...visibility.areas]),pvsSurfaces:Object.freeze([...visibility.surfaces]),drawSurfaces:Object.freeze([...visibility.drawSurfaces])}),geometry:renderer.captureGeometryEvidence(camera)})
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
        performanceProbe:`${this.#phaseTimings.map(value=>value.toFixed(3)).join(",")}:${this.#wasmCalls.observe},${this.#wasmCalls.models},${this.#wasmCalls.visibility},${this.#wasmCalls.particles}:${this.#maximumScheduledSamples},${this.#maximumPublicationTicks}:${prepared.particleOutputBytes},${prepared.publication.snapshotBytes.byteLength}`,
        performanceDetailProbe:JSON.stringify({tick:prepared.snapshot.tick.toString(),selectedTicks:prepared.publication.selectedTicks,models:prepared.modelMilliseconds,projectiles:prepared.projectileMilliseconds,visibility:visibilityMilliseconds,particleWorker:prepared.particleMilliseconds,particleDecode:prepared.particleDecodeMilliseconds,audio:prepared.audioMilliseconds,particleItems:rendered.timings.particleItems,particleBatches:rendered.timings.particleBatches,dynamicItems:rendered.timings.dynamicItemsMilliseconds,world:rendered.timings.worldMilliseconds,viewmodel:rendered.timings.viewModelMilliseconds,render:renderMilliseconds,total:totalMilliseconds}),
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
      this.#recordCrouch(snapshot)
      this.#recordAuthorityBlockers(snapshot)
      const activeWeapon=snapshot.loadout.find(value=>value.weapon===snapshot.weapon),reloadObservation=activeWeapon&&`${snapshot.tick}:${activeWeapon.weapon}:${activeWeapon.clip}/${activeWeapon.reserve}:${activeWeapon.reload}`
      if(reloadObservation&&this.#reloadHistory.at(-1)!==reloadObservation){this.#reloadHistory.push(reloadObservation);if(this.#reloadHistory.length>128)this.#reloadHistory.shift()}
      for (const event of snapshot.projectileEvents) {
        if (event.type === "fire") {this.#fireEvents += 1;this.#fireTickHistory.push(`${event.kind}:${event.tick}:${event.position.join(",")}`);if(this.#fireTickHistory.length>128)this.#fireTickHistory.shift()}
        if (event.type === "explode") this.#explosionEvents += 1
      }
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
        return value.weapon === null ? undefined : this.#viewmodels!.map(value, view)
      }
      for (const batch of publication.eventBatches) {
        if (batch.snapshot.tick >= snapshot.tick || !batch.snapshot.projectileEvents.some((event) => event.type === "fire")) continue
        const fire = batch.snapshot.projectileEvents.find((event) => event.type === "fire")!
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
      const currentFire=publication.eventBatches.at(-1)?.snapshot.projectileEvents.find((event)=>event.type==="fire")
      if(currentFire&&(!currentFire.launcherPose||!viewmodel))throw new Error(`TF2 fire-tick launcher pose unavailable: ${currentFire.tick}:${currentFire.projectile}`)
      const currentViewmodelRequest=viewmodel===undefined?undefined:Object.freeze({...viewmodel.request,sampleTick:currentFire?.tick??snapshot.tick,...(currentFire?{fireView:currentFire.launcherPose!}:{})})
      const lockerRequests=[...this.#lockerAnimations].flatMap(([identity,state])=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===identity),artifact=occurrence&&this.#artifacts!.models.get(occurrence.model);if(!occurrence||!artifact){this.#blockers.add(`TF2 regenerate model presentation unavailable: ${identity}`);return []}const closed=snapshot.tick>=state.closeTick,animation=closed?state.closeAnimation:state.openAnimation,start=closed?state.closeTick:state.openTick,elapsed=Math.max(0,Number(snapshot.tick-start)*0.015),previousTick=snapshot.tick>BigInt(publication.selectedTicks)?snapshot.tick-BigInt(publication.selectedTicks):0n,previousElapsed=Math.max(0,Number(previousTick-start)*0.015);return [Object.freeze({identity,model:occurrence.model,activity:animation,previousElapsedSeconds:Math.min(previousElapsed,elapsed),elapsedSeconds:elapsed,currentTimeSeconds:Number(snapshot.tick)*0.015,frameTimeSeconds:publication.selectedTicks*0.015,planarSpeed:0,screenAspectRatio:Math.max(1,viewport.width)/Math.max(1,viewport.height),worldFarPlane:camera.far,skin:occurrence.skin,lod:0,bodygroups:Object.freeze([]),packedBody:state.body})]})
      const modelStart=performance.now(),modelRequests=[...historicalViewmodels,...(currentViewmodelRequest?[currentViewmodelRequest]:[]),...lockerRequests]
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
      if(viewmodel!==undefined&&(viewmodelPoses.length!==2||viewmodelPoses[0]?.role!=="item"||viewmodelPoses[1]?.role!=="hand"))throw new Error("Viewmodel composition output differs");const viewmodelPose=viewmodelPoses[1]
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
      const particleBatch=owners.encoder.encode(snapshot.tick,camera.position,presentation.particles)
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
        models: Object.freeze([
          ...projectileModels(presentation.models),
          ...lockerPoses.map(pose=>{const occurrence=this.#artifacts!.modelOccurrences.find(value=>value.entity===pose.identity)!;return Object.freeze({identity:pose.identity,model:pose.model,position:occurrence.origin,angles:occurrence.angles,scale:1,skin:occurrence.skin,pose})}),
          ...viewmodelPoses.map((pose, index) => Object.freeze({
            ...viewmodel!.item,
            identity: viewmodel!.item.identity + index,
            model: pose.model,
            position:pose.viewmodel!.transform.origin,angles:pose.viewmodel!.transform.angles,
            viewModelProjection:Object.freeze({kind:"viewmodel" as const,horizontalFov4By3:pose.viewmodel!.projection.unscaledHorizontalFov4By3,near:pose.viewmodel!.projection.near,depthRange:pose.viewmodel!.depthRange,drawsAfterWorld:true,opaqueBeforeTranslucent:true,optionalViewSpaceYReflection:pose.viewmodel!.reflected}),
            pose,
          })),
        ]),
        brushModels: snapshot.entityPresentation,
        collisionWorldIdentity: snapshot.collisionSnapshot.worldIdentity,
      }) satisfies Omit<Frame,"camera"|"visibility"|"deltaSeconds">
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
        fireEvents: this.#fireEvents,
        explosionEvents: this.#explosionEvents,
        particleRenderItems: particleItems.length,
        movement: snapshot.movement,
        playerFlags: snapshot.playerFlags,
        inWater: snapshot.inWater,
        movementTick: snapshot.movementTick,
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
    const code = event.button >= 0 && event.button <= 4 ? `MOUSE${event.button + 1}` : ""
    const modifiers = Number(event.shiftKey) | (Number(event.ctrlKey) << 1) | (Number(event.altKey) << 2)
    return code ? this.#boundAction(code, modifiers) : null
  }

  #activateBoundAction(identity: string, action: string): void {
    if (action === "+forward" || action === "+back" || action === "+moveleft" || action === "+moveright" || action === "+duck") {
      this.#buttons.press(identity, action)
    } else if (action === "+jump") {
      if (this.#buttons.press(identity, action)) this.#jumpPressed = true
    } else if (action === "+attack") {
      if (this.#buttons.press(identity, action)) this.#firePressed = true
    } else if (action === "+attack2") {
      if (this.#buttons.press(identity, action)) this.#detonatePressed = true
    } else if (action === "+reload") {
      if (this.#buttons.press(identity, action)) this.#reloadPressed = true
    } else if (action === "slot1") this.#selectWeapon = 1
    else if (action === "slot2") this.#selectWeapon = 2
    else if (action === "slot3") this.#selectWeapon = 3
  }

  readonly #keyDown = (event: KeyboardEvent): void => {
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
    if (this.#view.consoleVisible || this.#view.optionsVisible || this.#view.gameUi !== "in-game" || event.repeat) return
    const action = this.#keyboardAction(event)
    if (!action) return
    void this.resumeAudio()
    this.#activateBoundAction(`keyboard:${event.code}`, action)
  }

  readonly #keyUp = (event: KeyboardEvent): void => {
    this.#buttons.release(`keyboard:${event.code}`)
  }

  readonly #mouseDown = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    void this.resumeAudio()
    const action = this.#mouseAction(event)
    if (action) this.#activateBoundAction(`mouse:${event.button}`, action)
  }

  readonly #mouseUp = (event: MouseEvent): void => {
    this.#buttons.release(`mouse:${event.button}`)
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
    this.#jumpPressed = this.#firePressed = this.#detonatePressed = this.#reloadPressed = false
    this.#selectClass = undefined
    this.#selectTeam = undefined
    this.#selectWeapon = undefined
    this.#modeRequest = undefined
  }

  #publishProfileCoverage():void{
    const profile=(globalThis as typeof globalThis&{__playsrcProfile?:Record<string,unknown>}).__playsrcProfile
    if(!profile)return
    profile.coverageSamples=this.#coverageSamples
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
    if (this.#closed || this.#view.consoleVisible) return
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
    this.#loaded = undefined
    this.#snapshot = undefined
    this.#artifacts = undefined
    this.#viewmodels = undefined
    this.#viewmodelClass = undefined
    this.#attachments.clear()
    this.#attachmentTransforms.clear()
    this.#fireAttachmentTransforms.clear()
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
