import { fetchImmutableObject, openDerivedObjectCache, type DerivedObjectCache } from "@playsrc/asset-store/browser"
import { createAudioSystem } from "@playsrc/audio"
import GameplayWorker from "@playsrc/game-tf2-browser/worker?worker"
import { Tf2WorkerClient, type LoadedGame } from "@playsrc/game-tf2-browser"
import { encodeCommand, mapDerivedKey, type Snapshot } from "@playsrc/game-tf2-browser/codec"
import { tf2Camera, tf2Hud, tf2Presentation, type Tf2Hud } from "@playsrc/game-tf2-browser/presentation"
import { createParticleSystem } from "@playsrc/particle"
import { createRenderer } from "@playsrc/rendering"
import {
  initializeDeveloperConsole,
  type ConsoleCompletionSuggestion,
  type ConsoleCatalog,
  type ConsoleRequest,
  type DeveloperConsole,
} from "@playsrc/vgui"
import { bytesToHex } from "@noble/hashes/utils.js"
import { sha256 } from "@noble/hashes/sha2.js"
import { consoleLimits, consoleResourceBlocker, consoleResources } from "./console-resources"
import { loadBrowserConfiguration, type BrowserConfiguration } from "./config"

const TICK_MILLISECONDS = 15
const MAX_FRAME_TICKS = 4
const MAX_EXTERNAL_BYTES = 536_870_912

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
}>

type Renderer = Awaited<ReturnType<typeof createRenderer>>
type Audio = ReturnType<typeof createAudioSystem>
type Particles = ReturnType<typeof createParticleSystem>

export class Tf2Application {
  readonly #canvas: HTMLCanvasElement
  readonly #vguiRoot: HTMLElement
  readonly #publish: (view: ApplicationView) => void
  #configuration?: BrowserConfiguration
  #dependencies = new Uint8Array()
  #cache?: DerivedObjectCache
  #client?: Tf2WorkerClient
  #renderer?: Renderer
  #audio?: Audio
  #particles?: Particles
  #console?: DeveloperConsole
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
  #developer = 1
  #animationFrame = 0
  #lastFrame = 0
  #accumulator = 0
  #frameBusy = false
  #fireEvents = 0
  #explosionEvents = 0
  #paused = true
  #closed = false
  #blockers = new Set<string>([
    consoleResourceBlocker,
    "Twelve world base textures and one exact first-style LDR lightmap atlas resolve; water shaders, directional bump-lightmaps, animated styles, and target filtering remain diagnostic.",
    "Static prop and exact rocket/sticky StudioModel geometry is available; first-person viewmodels and model animation/skin selection remain unavailable.",
    "TF2 PCF definitions and event context are unresolved; missing particle events emit diagnostics and no substitute effect.",
    "TF2 sound scripts and decoded resource buffers are unresolved; missing audio events create no Web Audio node or substitute sound.",
    "Jump course timers/checkpoints, trigger_multiple hint I/O, moving platforms, doors, and trigger_hurt are not implemented; exact brush trigger_teleport contacts are active and preserve velocity.",
  ])
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
      this.#set({ detail: "Fetching exact BSP and gameplay WASM objects" })
      const [bsp, wasm, dependencies] = await Promise.all([
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.bsp),
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.wasm),
        fetchImmutableObject(this.#configuration.assetOrigin, this.#configuration.dependencies),
      ])
      this.#dependencies = dependencies
      this.#cache = await openDerivedObjectCache()
      this.#client = new Tf2WorkerClient(new GameplayWorker(), this.#cache)
      await this.#client.initialize(wasm, this.#configuration.wasm.sha256)
      const key = await mapDerivedKey(this.#configuration.bsp.sha256, 0, this.#dependencies)
      this.#set({ detail: "Compiling direct map authority" })
      this.#generation = 1
      this.#loaded = await this.#client.load(this.#generation, bsp, 0, this.#dependencies, key)
      this.#renderer = await createRenderer(this.#canvas)
      this.resize()
      const scene = await this.#renderer.loadMap(
        this.#loaded.payload,
        this.#loaded.payloadSha256,
        true,
      )
      for (const diagnostic of scene.diagnostics) {
        this.#blockers.add(`Missing resolved material: ${diagnostic.identity}`)
      }
      this.#particles = createParticleSystem([])
      const AudioContextConstructor = window.AudioContext
      if (!AudioContextConstructor) throw new Error("Web Audio is unavailable")
      this.#audio = createAudioSystem(new AudioContextConstructor(), [])
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
      segments: [{ kind: "developer", text: "playsrc TF2 jump practice\nType status for exact support information.\n" }],
    })
  }

  #catalog(): ConsoleCatalog {
    return Object.freeze({
      revision: `tf2-jump-catalog-developer-${this.#developer}`,
      items: Object.freeze([
        Object.freeze({ kind: "command" as const, name: "map", disposition: "visible" as const, acceptsSuggestions: true }),
        Object.freeze({ kind: "command" as const, name: "class", disposition: "visible" as const, acceptsSuggestions: true }),
        Object.freeze({ kind: "command" as const, name: "status", disposition: "visible" as const, acceptsSuggestions: false }),
        Object.freeze({ kind: "command" as const, name: "clear", disposition: "visible" as const, acceptsSuggestions: false }),
        Object.freeze({ kind: "convar" as const, name: "developer", disposition: "visible" as const, displayValue: String(this.#developer) }),
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
      const candidates = request.commandName.toLowerCase() === "map"
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
      this.#output(`generation ${this.#generation}; map ${this.#configuration?.target}; cache ${this.#loaded?.cache}`, true)
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
      url.protocol !== "https:"
      || url.username
      || url.password
      || url.search
      || url.hash
      || !this.#configuration.allowedExternalOrigins.includes(url.origin)
      || !match?.groups?.name
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
        response.status !== 200
        || response.redirected
        || response.url !== url.href
        || !Number.isSafeInteger(total)
        || total < 1
        || total > MAX_EXTERNAL_BYTES
        || !response.body
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
    const key = await mapDerivedKey(bspSha256, 0, this.#dependencies)
    const staged = await this.#client.stage(generation, bytes, 0, this.#dependencies, key)
    const prior = this.#loaded
    try {
      await this.#renderer.loadMap(staged.payload, staged.payloadSha256, true)
      await this.#client.activate(generation)
    } catch (error) {
      await this.#client.discard(generation).catch(() => {})
      await this.#renderer.loadMap(prior.payload, prior.payloadSha256, true)
      throw error
    }
    this.#generation = generation
    this.#loaded = staged
    this.#snapshot = await this.#client.advance(generation, this.#command(), 1)
    this.#particles?.reset(this.#snapshot.tick)
    this.#paused = document.hidden
    this.#lastFrame = performance.now()
    this.#accumulator = 0
    this.#output(`Loaded ${name}; generation ${generation}; derived cache ${staged.cache}.`, true)
    this.#set({
      phase: "Ready",
      detail: `Playing ${name}`,
      hud: tf2Hud(this.#snapshot),
      cache: staged.cache,
    })
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
    })
    this.#selectClass = undefined
    this.#selectWeapon = undefined
    this.#jumpPressed = false
    this.#firePressed = false
    this.#detonatePressed = false
    return command
  }

  readonly #frame = (time: number): void => {
    this.#animationFrame = requestAnimationFrame(this.#frame)
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
    void this.#advance(ticks).finally(() => { this.#frameBusy = false })
  }

  async #advance(ticks: number): Promise<void> {
    if (!this.#client || !this.#renderer || !this.#snapshot || !this.#particles) return
    try {
      const snapshot = await this.#client.advance(this.#generation, this.#command(), ticks)
      this.#snapshot = snapshot
      for (const event of snapshot.events) {
        if (event.kind === 8 && event.detail === 1) this.#yaw = event.values[3]
        if (event.kind === 3) this.#fireEvents += 1
        if (event.kind === 4) this.#explosionEvents += 1
      }
      const particleItems = this.#particles.advance(snapshot.tick)
      const presentation = tf2Presentation(snapshot, particleItems, false)
      for (const diagnostic of presentation.diagnostics) {
        this.#blockers.add(diagnostic.code === "MissingProjectileModel"
          ? `${diagnostic.code}: ${diagnostic.identity}`
          : `${diagnostic.code}: exact event mapping inputs are unavailable`)
      }
      await this.#renderer.render({
        camera: tf2Camera(snapshot, this.#yaw, this.#pitch),
        effects: presentation.effects,
        models: presentation.models,
      })
      this.#set({
        hud: tf2Hud(snapshot),
        fireEvents: this.#fireEvents,
        explosionEvents: this.#explosionEvents,
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
    }
    else if (event.code === "ControlLeft" || event.code === "ControlRight") this.#crouch = true
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
    else if (event.code === "ControlLeft" || event.code === "ControlRight") this.#crouch = false
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
    this.#yaw = (this.#yaw + event.movementX * 0.08) % 360
    this.#pitch = Math.max(-89, Math.min(89, this.#pitch + event.movementY * 0.08))
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
  }

  selectClass(value: 1 | 2): void {
    this.#selectClass = value
  }

  async requestPointer(): Promise<void> {
    if (this.#closed || this.#console?.snapshot().visible) return
    try {
      await this.#canvas.requestPointerLock()
    } catch (error) {
      this.#set({ detail: error instanceof Error ? error.message : "Pointer lock failed" })
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
    await this.#client?.shutdown().catch(() => {})
    this.#cache?.close()
    this.#particles?.dispose()
    this.#renderer?.dispose()
    await this.#audio?.close().catch(() => {})
  }
}
