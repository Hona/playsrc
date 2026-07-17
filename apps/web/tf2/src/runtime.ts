import { fetchImmutableObject, openDerivedObjectCache, type DerivedObjectCache } from "@playsrc/asset-store/browser"
import { createAudioSystem } from "@playsrc/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { Tf2WorkerClient, type LoadedGame } from "@playsrc/game-tf2-browser"
import { encodeCommand, mapDerivedKey, type Snapshot } from "@playsrc/game-tf2-browser/codec"
import { parsePresentationArtifacts, type PresentationArtifacts } from "@playsrc/game-tf2-browser/artifacts"
import {
  createParticleBatchEncoder,
  createProjectilePresentationMapper,
  createViewmodelPresenter,
  particleEffects,
  projectileFrame,
  projectileModels,
  tf2Audio,
  tf2Camera,
  tf2Hud,
  type Tf2Hud,
} from "@playsrc/game-tf2-browser/presentation"
import { decodeParticleRenderOutput } from "@playsrc/particle"
import { createRenderer, SOURCE_LDR, SOURCE_PC_INTEGER_HDR, type Camera } from "@playsrc/rendering"
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
import { jumpBeefCourse } from "./course"

const TICK_MILLISECONDS = 15
const MAX_FRAME_TICKS = 4
const MAX_EXTERNAL_BYTES = 536_870_912
const SOUND_PATHS = [
  "sound/weapons/rocket_shoot.wav",
  "sound/weapons/stickybomblauncher_shoot.wav",
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
  #audioRunning = false
  #artifacts?: PresentationArtifacts
  #projectiles?: ProjectileMapper
  #viewmodels?: ReturnType<typeof createViewmodelPresenter>
  #attachments = new Map<number, ReadonlySet<string>>()
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
  #selectClass: 1 | 2 | undefined
  #selectWeapon: 1 | 2 | 3 | undefined
  #modeRequest: 0 | 1 | undefined
  #developer = 1
  #showFps: ClientDiagnosticMode = 0
  #showPos: ClientDiagnosticMode = 0
  #renderLevel: 0 | 1 | 2 = 2
  #mapIdentity = ""
  #environmentDrawables = 0
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
      await this.#client.configureCourse(this.#generation, jumpBeefCourse(this.#configuration.bsp.sha256))
      this.#artifacts = await parsePresentationArtifacts(this.#loaded.presentation)
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
        diagnostic: true,
      })
      this.#environmentDrawables = scene.environmentDrawables
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`Missing resolved material: ${diagnostic.identity}`)
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
      await this.#client.activate(this.#generation)
      this.#snapshot = await this.#client.advance(this.#generation, this.#command(), 1)
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
    if (name === "jump_beef") await this.#client.configureCourse(generation, jumpBeefCourse(bspSha256))
    const artifacts = await parsePresentationArtifacts(staged.presentation)
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
      }),
    )
    this.#viewmodels = createViewmodelPresenter(artifacts)
    this.#particleBatches = createParticleBatchEncoder()
    this.#mapIdentity = name
    this.#applyInitialView(staged)
    this.#snapshot = await this.#client.advance(generation, this.#command(), 1)
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
    })
  }

  #applyInitialView(loaded: LoadedGame): void {
    this.#pitch = Math.max(-89, Math.min(89, loaded.initialView.angles[0]))
    this.#yaw = loaded.initialView.angles[1] % 360
  }

  async #cacheModelArtifacts(artifacts: PresentationArtifacts): Promise<void> {
    if (!this.#cache) return
    for (const artifact of artifacts.models.values()) {
      try {
        const retained = await this.#cache.read(artifact.sha256)
        if (!retained) await this.#cache.write(artifact.sha256, artifact.sha256, artifact.bytes)
      } catch {
        this.#blockers.add(`ModelArtifactCacheUnavailable: ${artifact.identity}`)
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
        if (m) add(p.identity, m.attachments)
        const l = this.#artifacts.models.get(
          p.kind === 1
            ? "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
            : "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
        )
        if (l) add(p.launcherIdentity, l.attachments)
      }
      const presentation = this.#projectiles.map(projectileFrame(snapshot))
      const viewmodel = this.#viewmodels.map(snapshot)
      const camera = tf2Camera(snapshot, this.#yaw, this.#pitch)
      const visibility = await this.#client.visibility(this.#generation, camera.position)
      const particleOutput = await this.#client.particles(
        this.#generation,
        this.#particleBatches.encode(snapshot.tick, camera.position, presentation.particles),
      )
      const particleItems = decodeParticleRenderOutput(particleOutput, this.#artifacts.particleMaterials)
      if (this.#audioRunning && this.#audio) {
        for (const request of tf2Audio(snapshot)) this.#audio.play(request)
      }
      await this.#renderer.render({
        camera,
        effects: particleEffects(particleItems),
        models: Object.freeze([...projectileModels(presentation.models), viewmodel]),
        visibility,
      })
      this.#set({
        hud: tf2Hud(snapshot),
        fireEvents: this.#fireEvents,
        explosionEvents: this.#explosionEvents,
        camera,
        particleRenderItems: particleItems.length,
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
    this.#jump = this.#crouch = this.#fire = this.#detonate = false
    this.#jumpPressed = this.#firePressed = this.#detonatePressed = false
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
