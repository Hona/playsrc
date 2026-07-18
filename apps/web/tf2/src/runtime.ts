import { fetchImmutableObject, openDerivedObjectCache, type DerivedObjectCache } from "@playsrc/asset-store/browser"
import { createAudioSystem, SoundRegistry, SourceAudioWorld } from "@playsrc/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { Tf2WorkerClient, type LoadedGame } from "@playsrc/game-tf2-browser"
import { encodeCommand, mapDerivedKey, type Snapshot } from "@playsrc/game-tf2-browser/codec"
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
  tf2Hud,
  transformAttachment,
  type PosedModel,
  type Tf2Hud,
} from "@playsrc/game-tf2-browser/presentation"
import { decodeParticleRenderOutput } from "@playsrc/particle"
import { createRenderer, SOURCE_LDR, SOURCE_PC_INTEGER_HDR, type Camera, type MaterialStateInput } from "@playsrc/rendering"
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
import { consoleLimits, consoleResourceBlocker, consoleResources, diagnosticResources } from "./console-resources"
import { loadBrowserConfiguration, type BrowserConfiguration } from "./config"
import { applyPointerDelta } from "./input"

const TICK_MILLISECONDS = 15
const MAX_FRAME_TICKS = 4
const MAX_EXTERNAL_BYTES = 536_870_912
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

function dependencyEntries(bytes: Uint8Array): Map<string, Uint8Array> {
  if (bytes.byteLength < 12 || new TextDecoder().decode(bytes.subarray(0, 4)) !== "PSDB") {
    throw new Error("Source dependency bundle is malformed")
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (view.getUint32(4, true) !== 1) throw new Error("Source dependency bundle version is invalid")
  const count = view.getUint32(8, true)
  let offset = 12
  const result = new Map<string, Uint8Array>()
  const field = (): Uint8Array => {
    if (offset + 4 > bytes.byteLength) throw new Error("Source dependency field is truncated")
    const length = view.getUint32(offset, true)
    offset += 4
    if (offset + length > bytes.byteLength) throw new Error("Source dependency field is truncated")
    const value = bytes.subarray(offset, offset + length)
    offset += length
    return value
  }
  const decoder = new TextDecoder("utf-8", { fatal: true })
  for (let index = 0; index < count; index += 1) {
    const path = decoder.decode(field())
    if (!path || path !== path.toLowerCase() || result.has(path)) {
      throw new Error("Source dependency identity is malformed")
    }
    result.set(path, field())
  }
  if (offset !== bytes.byteLength) throw new Error("Source dependency bundle has trailing bytes")
  return result
}

export type ApplicationView = Readonly<{
  phase: "Loading" | "Ready" | "Replacing" | "Failed" | "Closed"
  detail: string
  hud?: Tf2Hud
  cache?: "hit" | "stored"
  pointerLocked: boolean
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
}>

type Renderer = Awaited<ReturnType<typeof createRenderer>>
type Audio = ReturnType<typeof createAudioSystem>
type ProjectileMapper = ReturnType<typeof createProjectilePresentationMapper>

export class Tf2Application {
  #canvas: HTMLCanvasElement
  readonly #vguiRoot: HTMLElement
  readonly #publish: (view: ApplicationView) => void
  #configuration?: BrowserConfiguration
  #dependencies = new Uint8Array()
  #dependencyEntries = new Map<string, Uint8Array>()
  #cache?: DerivedObjectCache
  #client?: Tf2WorkerClient
  #renderer?: Renderer
  #audio?: Audio
  #audioContext?: AudioContext
  #audioRegistry?: SoundRegistry
  #audioWorld?: SourceAudioWorld
  #audioBuffers = new Map<string, AudioBuffer>()
  #audioStarts: string[] = []
  #audioRunning = false
  #artifacts?: PresentationArtifacts
  #projectiles?: ProjectileMapper
  #viewmodels?: ReturnType<typeof createViewmodelPresenter>
  #attachments = new Map<number, ReadonlySet<string>>()
  #attachmentTransforms = new Map<number, ReadonlyMap<string, ReturnType<typeof transformAttachment>>>()
  #particleBatches = createParticleBatchEncoder()
  #console?: DeveloperConsole
  #diagnostics?: ClientDiagnostics
  #loaded?: LoadedGame
  #snapshot?: Snapshot
  #generation = 0
  #yaw = 0
  #pitch = 0
  #forward = false
  #back = false
  #left = false
  #right = false
  #jump = false
  #jumpPressed = false
  #crouch = false
  #fire = false
  #firePressed = false
  #detonate = false
  #detonatePressed = false
  #reload = false
  #reloadPressed = false
  #selectClass: 1 | 2 | undefined
  #selectWeapon: 1 | 2 | 3 | undefined
  #modeRequest: 0 | 1 | undefined
  #developer = 1
  #showFps: ClientDiagnosticMode = 0
  #showPos: ClientDiagnosticMode = 0
  #renderLevel: 0 | 1 | 2 = 2
  #mapIdentity = ""
  #environmentDrawables = 0
  #modelProbes: NonNullable<ApplicationView["modelProbes"]> = Object.freeze([])
  #viewmodelActivities = new Set<string>()
  #crouchHistory: string[] = []
  #viewmodelTimelineProbes: string[] = []
  #lastRandomAudioProbe = ""
  #lastCollisionMoverProbe = ""
  #animationFrame = 0
  #lastFrame = 0
  #accumulator = 0
  #frameBusy = false
  #fireEvents = 0
  #explosionEvents = 0
  #paused = true
  #closed = false
  #blockers = new Set<string>([consoleResourceBlocker])
  #view: ApplicationView = Object.freeze({
    phase: "Loading",
    detail: "Reading local configuration",
    pointerLocked: false,
    consoleVisible: false,
    blockers: Object.freeze([]),
    fireEvents: 0,
    explosionEvents: 0,
  })

  constructor(canvas: HTMLCanvasElement, vguiRoot: HTMLElement, publish: (view: ApplicationView) => void) {
    this.#canvas = canvas
    this.#vguiRoot = vguiRoot
    this.#publish = publish
  }

  #set(patch: Partial<ApplicationView>): void {
    this.#view = Object.freeze({
      ...this.#view,
      ...patch,
      blockers: Object.freeze([...this.#blockers].sort()),
    })
    this.#publish(this.#view)
  }

  async start(): Promise<void> {
    try {
      this.#configuration = await loadBrowserConfiguration()
      this.#renderLevel = this.#configuration.renderLevel
      this.#mapIdentity = this.#configuration.target
      this.#set({ detail: "Fetching exact BSP and gameplay WASM objects" })
      const [bsp, wasm, dependencies] = await Promise.all([
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.bsp),
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.wasm),
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.dependencies),
      ])
      this.#dependencies = dependencies
      this.#dependencyEntries = dependencyEntries(dependencies)
      this.#cache = await openDerivedObjectCache()
      this.#client = new Tf2WorkerClient(new GameplayWorker(), this.#cache)
      await this.#client.initialize(wasm, this.#configuration.wasm.sha256)
      const profile = this.#renderLevel === 2 ? 1 : 0
      const key = await mapDerivedKey(
        this.#configuration.bsp.sha256,
        profile,
        this.#renderLevel,
        this.#configuration.wasm.sha256,
        this.#dependencies,
      )
      this.#set({ detail: "Compiling direct map authority" })
      this.#generation = 1
      this.#loaded = await this.#client.stage(this.#generation, bsp, profile, this.#dependencies, key)
      this.#artifacts = await parsePresentationArtifacts(this.#loaded.presentation)
      this.#recordVisualOutputBlockers(this.#artifacts)
      await this.#cacheModelArtifacts(this.#artifacts)
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
          localOwnerIdentity: 1,
        }),
      )
      this.#viewmodels = createViewmodelPresenter(this.#artifacts)
      this.#applyInitialView(this.#loaded)
      this.#renderer = await createRenderer({
        canvas: this.#canvas,
        configuration: this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR,
        powerPreference: "high-performance",
      })
      this.resize()
      const scene = await this.#renderer.loadMap({
        payload: this.#loaded.payload,
        payloadSha256: this.#loaded.payloadSha256,
        modelTextures: this.#artifacts.textures,
        directionalTextures: this.#artifacts.directionalTextures,
        environment: this.#artifacts.environment,
        materialStates: this.#materialStates(this.#artifacts),
        particleTextures: this.#artifacts.particleTextures,
        modelOccurrences: this.#artifacts.modelOccurrences,
        modelMaterials: this.#artifacts.modelMaterials,
        authoredTextures: this.#artifacts.authoredTextures,
        diagnostic: true,
      })
      this.#environmentDrawables = scene.environmentDrawables
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`${diagnostic.code}: ${diagnostic.identity} — ${diagnostic.detail}`)
      }
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
      await this.#client.activate(this.#generation)
      this.#snapshot = await this.#client.advance(this.#generation, this.#command(), 1)
      this.#recordAuthorityBlockers(this.#snapshot)
      this.#recordCrouch(this.#snapshot)
      this.#modelProbes = await this.#probePlayerModels(this.#artifacts)
      this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(this.#artifacts)
      this.#initializeConsole()
      this.#installListeners()
      this.#paused = document.hidden
      this.#lastFrame = performance.now()
      this.#animationFrame = requestAnimationFrame(this.#frame)
      this.#set({
        phase: "Ready",
        detail: "Click the field to capture the mouse",
        hud: tf2Hud(this.#snapshot),
        cache: this.#loaded.cache,
        initialView: this.#loaded.initialView,
        environment: this.#artifacts.environment,
        environmentDrawables: this.#environmentDrawables,
        movement: this.#snapshot.movement,
        movementTick: this.#snapshot.movementTick,
        modelProbes: this.#modelProbes,
        snapshotTick: this.#snapshot.tick.toString(),
        projectileStates: this.#snapshot.projectiles.map((projectile) => `${projectile.identity}:${projectile.state}`).join(","),
        decalProbe: this.#decalProbe(this.#artifacts),
        modelOccurrenceCount: this.#artifacts.modelOccurrences.length,
        viewmodelSequences: this.#viewmodelSequences(this.#artifacts, 1),
        crouchHistory: Object.freeze([...this.#crouchHistory]),
        viewmodelTimelineProbes: Object.freeze([...this.#viewmodelTimelineProbes]),
        modelMatrices: this.#modelMatrices(this.#artifacts),
        decalStateProbe: this.#decalStateProbe(this.#artifacts),
        modelMaterialProbe: this.#modelMaterialProbe(this.#artifacts),
        ...this.#gameplayTraces(this.#snapshot),
        ...this.#snapshotProbes(this.#snapshot),
      })
    } catch (error) {
      await this.#release()
      this.#set({ phase: "Failed", detail: error instanceof Error ? error.message : "Application startup failed" })
    }
  }

  #initializeConsole(): void {
    const initialized = initializeDeveloperConsole({
      runtimeIdentity: "tf2-jump-console",
      limits: consoleLimits,
      resources: consoleResources,
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
    const diagnostics = initializeClientDiagnostics({
      runtimeIdentity: "tf2-client-diagnostics",
      resources: diagnosticResources,
      viewport: this.#viewport(),
    })
    if (!diagnostics.ok) throw new Error(`VGUI diagnostics initialization failed: ${diagnostics.code}`)
    this.#diagnostics = diagnostics.diagnostics
    const mountedDiagnostics = this.#diagnostics.apply({ kind: "mount", root: this.#vguiRoot })
    if (!mountedDiagnostics.ok) throw new Error(`VGUI diagnostics mount failed: ${mountedDiagnostics.code}`)
  }

  #catalog(): ConsoleCatalog {
    return Object.freeze({
      revision: `tf2-jump-catalog-developer-${this.#developer}-fps-${this.#showFps}-pos-${this.#showPos}-hdr-${this.#renderLevel}`,
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
      ]),
    })
  }

  #viewport() {
    const bounds = this.#vguiRoot.getBoundingClientRect()
    return {
      width: Math.max(1, bounds.width),
      height: Math.max(1, bounds.height),
      devicePixelRatio: window.devicePixelRatio,
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
          ? ["map jump_beef"]
          : request.commandName.toLowerCase() === "class"
            ? ["class soldier", "class demoman"]
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
    this.#console?.apply({
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
        `generation ${this.#generation}; map ${this.#configuration?.target}; cache ${this.#loaded?.cache}`,
        true,
      )
      for (const blocker of [...this.#blockers].sort()) this.#output(`BLOCKED: ${blocker}`)
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
      if (tokens[0]?.toLowerCase() === "soldier") this.selectClass(1)
      else if (tokens[0]?.toLowerCase() === "demoman") this.selectClass(2)
      else {
        this.#output("Usage: class soldier|demoman")
        return
      }
      this.#output(`Class selection queued: ${tokens[0]}`)
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
      if (tokens[0] === "jump_beef") await this.#replaceCatalogMap()
      else if (tokens[0]?.startsWith("https://")) await this.#replaceExternalMap(tokens[0])
      else this.#output("Usage: map jump_beef")
      return
    }
    this.#output(`Unknown command: ${command}`)
  }

  async #replaceCatalogMap(): Promise<void> {
    if (!this.#configuration) return
    try {
      this.#set({ phase: "Replacing", detail: "Reloading jump_beef through exact catalog identity" })
      const bytes = await fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.bsp)
      await this.#replace(bytes, this.#configuration.bsp.sha256, "jump_beef")
    } catch (error) {
      this.#output(`Map replacement failed: ${error instanceof Error ? error.message : "unknown failure"}`)
      this.#paused = document.hidden
      this.#lastFrame = performance.now()
      this.#set({ phase: "Ready", detail: "Prior map retained" })
    }
  }

  async #replaceExternalMap(value: string): Promise<void> {
    try {
      const source = await this.#externalSource(value)
      this.#set({ phase: "Replacing", detail: `Loading ephemeral ${source.name}` })
      await this.#replace(source.bytes, source.sha256, source.name)
    } catch (error) {
      this.#output(`External map failed: ${error instanceof Error ? error.message : "unknown failure"}`)
      this.#paused = document.hidden
      this.#lastFrame = performance.now()
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

  async #replace(bytes: Uint8Array, bspSha256: string, name: string): Promise<void> {
    if (!this.#client || !this.#renderer || !this.#loaded) throw new Error("Application is not ready")
    this.#paused = true
    this.#neutral()
    const generation = this.#generation + 1
    const profile = this.#renderLevel === 2 ? 1 : 0
    const key = await mapDerivedKey(
      bspSha256,
      profile,
      this.#renderLevel,
      this.#configuration?.wasm.sha256 ?? "",
      this.#dependencies,
    )
    const staged = await this.#client.stage(generation, bytes, profile, this.#dependencies, key)
    const artifacts = await parsePresentationArtifacts(staged.presentation)
    this.#recordVisualOutputBlockers(artifacts)
    await this.#cacheModelArtifacts(artifacts)
    const prior = this.#loaded
    const priorArtifacts = this.#artifacts
    const priorConfiguration = this.#renderer.configuration
    try {
      if (this.#renderer.configuration.lightingProfile !== (this.#renderLevel === 2 ? "hdr" : "ldr")) {
        await this.#renderer.dispose()
        this.#renderer = await createRenderer({
          canvas: this.#canvas,
          configuration: this.#renderLevel === 2 ? SOURCE_PC_INTEGER_HDR : SOURCE_LDR,
          powerPreference: "high-performance",
        })
        this.resize()
      }
      const scene = await this.#renderer.loadMap({
        payload: staged.payload,
        payloadSha256: staged.payloadSha256,
        modelTextures: artifacts.textures,
        directionalTextures: artifacts.directionalTextures,
        environment: artifacts.environment,
        materialStates: this.#materialStates(artifacts),
        particleTextures: artifacts.particleTextures,
      modelOccurrences: artifacts.modelOccurrences,
      modelMaterials: artifacts.modelMaterials,
      authoredTextures: artifacts.authoredTextures,
        diagnostic: true,
      })
      this.#environmentDrawables = scene.environmentDrawables
      await this.#client.activate(generation)
    } catch (error) {
      await this.#client.discard(generation).catch(() => {})
      if (this.#renderer.configuration.lightingProfile !== priorConfiguration.lightingProfile) {
        await this.#renderer.dispose().catch(() => {})
        this.#renderer = await createRenderer({
          canvas: this.#canvas,
          configuration: priorConfiguration,
          powerPreference: "high-performance",
        })
        this.resize()
      }
      await this.#renderer.loadMap({
        payload: prior.payload,
        payloadSha256: prior.payloadSha256,
        modelTextures: priorArtifacts?.textures,
        directionalTextures: priorArtifacts?.directionalTextures,
        environment: priorArtifacts?.environment,
        materialStates: priorArtifacts ? this.#materialStates(priorArtifacts) : undefined,
        particleTextures: priorArtifacts?.particleTextures,
        modelOccurrences: priorArtifacts?.modelOccurrences,
        diagnostic: true,
      })
      throw error
    }
    this.#generation = generation
    this.#loaded = staged
    this.#artifacts = artifacts
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
        localOwnerIdentity: 1,
      }),
    )
    this.#viewmodels = createViewmodelPresenter(artifacts)
    this.#particleBatches = createParticleBatchEncoder()
    this.#mapIdentity = name
    this.#applyInitialView(staged)
    this.#snapshot = await this.#client.advance(generation, this.#command(), 1)
    this.#recordAuthorityBlockers(this.#snapshot)
    this.#crouchHistory = []
    this.#recordCrouch(this.#snapshot)
    this.#modelProbes = await this.#probePlayerModels(artifacts)
    this.#viewmodelTimelineProbes = await this.#probeViewmodelTimelines(artifacts)
    this.#audio?.reset()
    this.#audioWorld?.reset()
    this.#lastRandomAudioProbe = ""
    this.#lastCollisionMoverProbe = ""
    this.#paused = document.hidden
    this.#lastFrame = performance.now()
    this.#accumulator = 0
    this.#output(`Loaded ${name}; generation ${generation}; derived cache ${staged.cache}.`, true)
    this.#set({
      phase: "Ready",
      detail: `Playing ${name}`,
      hud: tf2Hud(this.#snapshot),
      cache: staged.cache,
      initialView: staged.initialView,
      environment: artifacts.environment,
      environmentDrawables: this.#environmentDrawables,
      movement: this.#snapshot.movement,
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
    const states = new Map<string, MaterialStateInput>(artifacts.materialStates)
    for (const texture of artifacts.particleTextures) {
      const state = artifacts.materialStates.get(texture.materialPath.toLowerCase())
      if (state) states.set(texture.material.toLowerCase(), state)
    }
    return states
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

  #recordVisualOutputBlockers(artifacts: PresentationArtifacts): void {
    if (
      artifacts.models.has("models/weapons/v_models/v_rocketlauncher_soldier.mdl") ||
      artifacts.models.has("models/weapons/v_models/v_stickybomb_launcher_demo.mdl")
    ) {
      this.#blockers.add("Missing TF2 stock viewmodel composition: class hand model, item c_model attachment, and animation-library join")
    }
    const mipmapped = [...artifacts.materialStates].filter(([identity, state]) =>
      !artifacts.modelMaterials.has(identity) && state.samplingAvailable && state.mipmapped,
    ).length
    if (mipmapped > 0) this.#blockers.add(`Missing authored texture mip planes for ${mipmapped} world/environment material states`)
    this.#blockers.add("Missing current model lightcache selections, game-owned eye targets, and per-draw StudioModel lighting/eye state")
    this.#blockers.add("Missing decoded profile-qualified sky and cubemap subresources")
    this.#blockers.add("Missing complete Water material and reflection/refraction view inputs")
    this.#blockers.add("Missing current fog-controller state and transition inputs")
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

  #viewmodelSequences(artifacts: PresentationArtifacts, tf2Class: 1 | 2): string {
    const identity = tf2Class === 1
      ? "models/weapons/c_models/c_soldier_arms.mdl"
      : "models/weapons/c_models/c_demo_arms.mdl"
    return artifacts.models.get(identity)?.sequences
      .filter((sequence) => sequence.timingAvailable)
      .map((sequence) => `${sequence.activity}:${sequence.durationSeconds}`)
      .join("|") ?? ""
  }

  #recordCrouch(snapshot: Snapshot): void {
    const value = `${snapshot.tick}:${snapshot.movement.crouchFraction}:${snapshot.movement.viewOffset[2]}`
    if (this.#crouchHistory.at(-1) !== value) this.#crouchHistory.push(value)
    if (this.#crouchHistory.length > 128) this.#crouchHistory.splice(0, this.#crouchHistory.length - 128)
  }

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
        skin: 0,
        lod: 0,
        bodygroups: Object.freeze(artifact.bodygroupCounts.map(() => 0)),
        itemBodygroups: Object.freeze(itemArtifact.bodygroupCounts.map(() => 0)),
      })
    })
    const poses = decodeModelPoseOutput(await this.#client.models(this.#generation, encodeModelPoseBatch(requests)))
    return activities.map((activity) => {
      const parts = poses.filter((pose) => pose.activity === activity)
      if (parts.length !== 2 || parts[0]?.role !== "hand" || parts[1]?.role !== "item") throw new Error(`Viewmodel timeline composition ${activity} differs`)
      return `${activity}:${parts[0].sequence}:${parts[0].cycle}:${parts.map((part) => part.primitives.length).join("+")}:${parts[0].events.length}`
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
      muted: false,
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
        userGain: 1,
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
    const viewmodelAttachments = new Map(
      viewmodels.flatMap((viewmodel) => viewmodel.attachments).map((attachment) => [
        attachment.name.toLowerCase(),
        transformAttachment(attachment.matrix, camera.position, cameraOrientation),
      ]),
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
    for (const event of snapshot.projectileEvents.filter((value) => value.type === "fire")) {
      if (event.kind === 1 && event.ownerIdentity === 1) continue
      const attachment = event.kind === 1 ? "backblast" : "muzzle"
      if (!this.#attachmentTransforms.get(event.launcherIdentity)?.has(attachment)) {
        this.#blockers.add(`TF2 viewmodel attachment transform unavailable: ${attachment}`)
      }
    }
  }

  #command(): ArrayBuffer {
    const forward = Number(this.#forward) - Number(this.#back)
    const side = Number(this.#left) - Number(this.#right)
    const command = encodeCommand({
      forward: forward * 450,
      side: side * 450,
      yawDegrees: this.#yaw,
      pitchDegrees: this.#pitch,
      jump: this.#jump || this.#jumpPressed,
      crouch: this.#crouch,
      fire: this.#fire || this.#firePressed,
      detonate: this.#detonate || this.#detonatePressed,
      reload: this.#reload || this.#reloadPressed,
      selectClass: this.#selectClass,
      selectWeapon: this.#selectWeapon,
      modeRequest: this.#modeRequest,
    })
    this.#selectClass = undefined
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
    if (!this.#paused && this.#snapshot && (this.#showFps !== 0 || this.#showPos !== 0)) this.#updateDiagnostics(time)
    if (this.#paused || this.#frameBusy || !this.#client || !this.#renderer || !this.#snapshot) {
      this.#lastFrame = time
      return
    }
    const elapsed = Math.min(100, Math.max(0, time - this.#lastFrame))
    this.#lastFrame = time
    this.#accumulator += elapsed
    const ticks = Math.min(MAX_FRAME_TICKS, Math.floor(this.#accumulator / TICK_MILLISECONDS))
    if (ticks < 1) return
    this.#accumulator -= ticks * TICK_MILLISECONDS
    this.#frameBusy = true
    void this.#advance(ticks).finally(() => {
      this.#frameBusy = false
    })
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

  async #advance(ticks: number): Promise<void> {
    if (
      !this.#client ||
      !this.#renderer ||
      !this.#snapshot ||
      !this.#projectiles ||
      !this.#viewmodels ||
      !this.#artifacts
    )
      return
    try {
      const snapshot = await this.#client.advance(this.#generation, this.#command(), ticks)
      this.#snapshot = snapshot
      this.#recordCrouch(snapshot)
      this.#recordAuthorityBlockers(snapshot)
      for (const event of snapshot.events) {
        if (event.kind === 9 && event.detail === 1) this.#yaw = event.values[3]
      }
      for (const event of snapshot.projectileEvents) {
        if (event.type === "fire") this.#fireEvents += 1
        if (event.type === "explode") this.#explosionEvents += 1
      }
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
      const viewmodel = this.#viewmodels.map(snapshot)
      const camera = tf2Camera(snapshot, this.#yaw, this.#pitch)
      const modelPoses = decodeModelPoseOutput(
        await this.#client.models(this.#generation, encodeModelPoseBatch([viewmodel.request])),
      )
      const viewmodelPoses = modelPoses.filter((pose) => pose.identity === viewmodel.item.identity)
      if (viewmodelPoses.length !== 2 || viewmodelPoses[0]?.role !== "hand" || viewmodelPoses[1]?.role !== "item") throw new Error("Viewmodel composition output differs")
      const viewmodelPose = viewmodelPoses[0]!
      this.#viewmodelActivities.add(viewmodelPose.activity)
      this.#updateAttachmentTransforms(snapshot, viewmodelPoses, camera)
      const presentation = this.#projectiles.map(projectileFrame(snapshot))
      const visibility = await this.#client.visibility(this.#generation, camera.position)
      const particleOutput = await this.#client.particles(
        this.#generation,
        this.#particleBatches.encode(snapshot.tick, camera.position, presentation.particles),
      )
      const particleItems = decodeParticleRenderOutput(particleOutput, this.#artifacts.particleMaterials)
      this.#playAudio(snapshot, camera)
      const rendered = await this.#renderer.render({
        camera,
        effects: Object.freeze([]),
        particles: particleItems,
        models: Object.freeze([
          ...projectileModels(presentation.models),
          ...viewmodelPoses.map((pose, index) => Object.freeze({
            ...viewmodel.item,
            identity: viewmodel.item.identity + index,
            model: pose.model,
            pose,
          })),
        ]),
        visibility,
        deltaSeconds: ticks * 0.015,
      })
      this.#set({
        hud: tf2Hud(snapshot),
        fireEvents: this.#fireEvents,
        explosionEvents: this.#explosionEvents,
        camera,
        particleRenderItems: particleItems.length,
        visibleDecalFragments: rendered.visibleProjectedMarks,
        movement: snapshot.movement,
        movementTick: snapshot.movementTick,
        viewmodelPose: Object.freeze({
          activity: viewmodelPose.activity,
          sequence: viewmodelPose.sequence,
          cycle: viewmodelPose.cycle,
          primitives: viewmodelPoses.reduce((total, pose) => total + pose.primitives.length, 0),
          events: viewmodelPose.events.length,
        }),
        audioVoices: this.#audio.activeVoices(),
        snapshotTick: snapshot.tick.toString(),
        projectileStates: snapshot.projectiles.map((projectile) => `${projectile.identity}:${projectile.state}`).join(","),
        particleProbe: [...new Set(particleItems.map((item) => `${item.primitive}:${item.material}:${item.primarySheet ? "sheet" : "missing"}`))].sort().join("|"),
        audioStarts: Object.freeze([...this.#audioStarts]),
        viewmodelProjection: viewmodel.item.viewModelProjection ? `${viewmodel.item.viewModelProjection.horizontalFov4By3}:${viewmodel.item.viewModelProjection.near}:${viewmodel.item.viewModelProjection.depthRange.join(",")}` : undefined,
        viewmodelDepthRange: rendered.viewModelPass?.depthRange.join(","),
        viewmodelViewportRestored: rendered.viewModelPass?.viewportRestored,
        viewmodelActivities: Object.freeze([...this.#viewmodelActivities]),
        viewmodelSequences: this.#viewmodelSequences(this.#artifacts, snapshot.class),
        crouchHistory: Object.freeze([...this.#crouchHistory]),
        viewmodelTimelineProbes: Object.freeze([...this.#viewmodelTimelineProbes]),
        ...this.#gameplayTraces(snapshot),
        ...this.#snapshotProbes(snapshot),
      })
    } catch (error) {
      this.#paused = true
      this.#set({ phase: "Failed", detail: error instanceof Error ? error.message : "Gameplay frame failed" })
    }
  }

  #installListeners(): void {
    window.addEventListener("keydown", this.#keyDown)
    window.addEventListener("keyup", this.#keyUp)
    window.addEventListener("mousedown", this.#mouseDown)
    window.addEventListener("mouseup", this.#mouseUp)
    window.addEventListener("mousemove", this.#mouseMove)
    window.addEventListener("resize", this.#resize)
    window.addEventListener("blur", this.#blur)
    document.addEventListener("visibilitychange", this.#visibility)
    document.addEventListener("pointerlockchange", this.#pointerLock)
  }

  #removeListeners(): void {
    window.removeEventListener("keydown", this.#keyDown)
    window.removeEventListener("keyup", this.#keyUp)
    window.removeEventListener("mousedown", this.#mouseDown)
    window.removeEventListener("mouseup", this.#mouseUp)
    window.removeEventListener("mousemove", this.#mouseMove)
    window.removeEventListener("resize", this.#resize)
    window.removeEventListener("blur", this.#blur)
    document.removeEventListener("visibilitychange", this.#visibility)
    document.removeEventListener("pointerlockchange", this.#pointerLock)
  }

  readonly #keyDown = (event: KeyboardEvent): void => {
    if (event.code === "Backquote") {
      if (this.#vguiRoot.contains(event.target as Node)) return
      event.preventDefault()
      this.toggleConsole()
      return
    }
    if (this.#console?.snapshot().visible || event.repeat) return
    if (event.code === "KeyW") this.#forward = true
    else if (event.code === "KeyS") this.#back = true
    else if (event.code === "KeyA") this.#left = true
    else if (event.code === "KeyD") this.#right = true
    else if (event.code === "Space") {
      this.#jump = true
      this.#jumpPressed = true
    } else if (event.code === "ShiftLeft" || event.code === "ShiftRight") this.#crouch = true
    else if (event.code === "KeyR") {
      this.#reload = true
      this.#reloadPressed = true
    }
    else if (event.code === "Digit1") this.selectClass(1)
    else if (event.code === "Digit2") this.selectClass(2)
    else if (event.code === "Digit3") this.#selectWeapon = 2
  }

  readonly #keyUp = (event: KeyboardEvent): void => {
    if (event.code === "KeyW") this.#forward = false
    else if (event.code === "KeyS") this.#back = false
    else if (event.code === "KeyA") this.#left = false
    else if (event.code === "KeyD") this.#right = false
    else if (event.code === "Space") this.#jump = false
    else if (event.code === "ShiftLeft" || event.code === "ShiftRight") this.#crouch = false
    else if (event.code === "KeyR") this.#reload = false
  }

  readonly #mouseDown = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    if (event.button === 0) {
      this.#fire = true
      this.#firePressed = true
    }
    if (event.button === 2) {
      this.#detonate = true
      this.#detonatePressed = true
    }
  }

  readonly #mouseUp = (event: MouseEvent): void => {
    if (event.button === 0) this.#fire = false
    if (event.button === 2) this.#detonate = false
  }

  readonly #mouseMove = (event: MouseEvent): void => {
    if (document.pointerLockElement !== this.#canvas) return
    const angles = applyPointerDelta(this.#yaw, this.#pitch, event.movementX, event.movementY)
    this.#yaw = angles.yaw
    this.#pitch = angles.pitch
  }

  readonly #resize = (): void => this.resize()
  readonly #blur = (): void => this.#neutral()
  readonly #visibility = (): void => {
    this.#paused = document.hidden
    this.#neutral()
    this.#lastFrame = performance.now()
  }
  readonly #pointerLock = (): void => {
    if (document.pointerLockElement !== this.#canvas) this.#neutral()
    this.#set({ pointerLocked: document.pointerLockElement === this.#canvas })
  }

  #neutral(): void {
    this.#forward = this.#back = this.#left = this.#right = false
    this.#jump = this.#crouch = this.#fire = this.#detonate = this.#reload = false
    this.#jumpPressed = this.#firePressed = this.#detonatePressed = this.#reloadPressed = false
    this.#modeRequest = undefined
  }

  selectClass(value: 1 | 2): void {
    this.#selectClass = value
  }

  async requestPointer(canvas = this.#canvas): Promise<void> {
    this.#canvas = canvas
    if (this.#closed || this.#console?.snapshot().visible) return
    try {
      await this.#canvas.requestPointerLock()
    } catch (error) {
      this.#set({ detail: error instanceof Error ? error.message : "Pointer lock failed" })
    }
  }

  async resumeAudio(): Promise<void> {
    if (!this.#audio || this.#closed) return
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
    if (this.#console.snapshot().visible) {
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

  resize(): void {
    if (!this.#renderer) return
    const bounds = this.#canvas.getBoundingClientRect()
    this.#renderer.resize(bounds.width, bounds.height, window.devicePixelRatio)
    this.#console?.apply({ kind: "set-viewport", viewport: this.#viewport() })
    this.#diagnostics?.apply({ kind: "set-viewport", viewport: this.#viewport() })
  }

  async close(): Promise<void> {
    if (this.#closed) return
    await this.#release()
    this.#set({ phase: "Closed", detail: "Application closed", pointerLocked: false, consoleVisible: false })
  }

  async #release(): Promise<void> {
    if (this.#closed) return
    this.#closed = true
    this.#paused = true
    cancelAnimationFrame(this.#animationFrame)
    this.#removeListeners()
    this.#neutral()
    if (document.pointerLockElement === this.#canvas) {
      try {
        await document.exitPointerLock()
      } catch {}
    }
    this.#console?.apply({ kind: "destroy" })
    this.#diagnostics?.apply({ kind: "destroy" })
    await this.#client?.shutdown().catch(() => {})
    this.#cache?.close()
    this.#projectiles?.dispose()
    await this.#renderer?.dispose().catch(() => {})
    await this.#audio?.close().catch(() => {})
  }
}
