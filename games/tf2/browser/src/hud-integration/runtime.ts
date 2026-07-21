import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiPanelSnapshot,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiViewport,
} from "@playsrc/vgui"
import {
  adaptCompactSessionHud,
  bindTf2Hud,
  bindTf2HudAction,
  TF2_HUD_DYNAMIC_IMAGES,
  TF2_GROUPED_CONDITION_PANELS,
  TF2_INDEPENDENT_CONDITION_PANELS,
  tf2HudAvailable,
  tf2HudUnavailable,
  type CompactSessionHudContext,
  type CompactSessionSimulationPublication,
  type Tf2HudAction,
  type Tf2HudAvailability,
  type Tf2HudBinding,
  type Tf2HudCommand,
  type Tf2HudPanelValue,
  type Tf2HudSnapshot,
} from "../hud"
import type { Tf2VguiResources } from "../ui-integration"

export type Tf2HudIntegrationDiagnostic = Readonly<{
  code: "VguiRejected" | "PanelUnavailable" | "ValueUnavailable" | "UnsupportedPanelValue" | "AnimationUnavailable"
  subject: string
}>

export type Tf2HudIntegrationSnapshot = Readonly<{
  binding: Tf2HudBinding | null
  vgui: VguiRuntimeSnapshot
  diagnostics: readonly Tf2HudIntegrationDiagnostic[]
  animationTrace: readonly string[]
}>

export type Tf2HudIntegrationProbe = Readonly<{
  panels: readonly VguiPanelSnapshot[]
  animationTrace: readonly string[]
}>

export type Tf2HudIntegration = Readonly<{
  publish(publication: CompactSessionSimulationPublication, context: CompactSessionHudContext): Tf2HudBinding
  action(action: Tf2HudAction): Tf2HudAvailability<Tf2HudCommand>
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  probe(): Tf2HudIntegrationProbe
  snapshot(): Tf2HudIntegrationSnapshot
  setPlayerClassUsePlayerModel(value: boolean): void
  reset(reason: "map-replaced" | "disconnect"): void
  destroy(): void
}>

export type Tf2HudIntegrationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onCommand(command: Tf2HudCommand): void
}>

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 512,
  maxHierarchyDepth: 32,
  maxChildrenPerPanel: 256,
  maxResourceNodes: 2_048,
  maxResourceDepth: 32,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4_095,
  maxTextCodeUnits: 65_535,
  maxDialogVariables: 256,
  maxLocalizationTokens: 4_096,
  maxSchemeColors: 1_024,
  maxSchemeSettings: 2_048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2_048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1_024,
  maxAnimationCommands: 8_192,
  maxActiveAnimations: 2_048,
  maxDelayedCommands: 2_048,
  maxQueuedMessages: 2_048,
  maxDiagnostics: 2_048,
  maxDomNodes: 2_048,
  maxListeners: 64,
})

const HUD_LAYOUT = "scripts/hudlayout.res"
const HUD_CLASS = "resource/ui/hudplayerclass.res"
const HUD_HEALTH = "resource/ui/hudplayerhealth.res"
const HUD_AMMO = "resource/ui/hudammoweapons.res"
const HUD_WEAPONS = "resource/ui/hudweaponselection.res"
const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const node = (name: string, children: readonly VguiResourceNode[]): VguiResourceNode => Object.freeze({ name, value: null, condition: null, children: Object.freeze(children) })
const document = (source: VguiResourceDocument, suffix: string, root: VguiResourceNode): VguiResourceDocument => Object.freeze({
  logicalIdentity: `${source.logicalIdentity}/${suffix}`.toLowerCase(),
  revision: source.revision,
  root,
})

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function find(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((panel) => panel.name.toLowerCase() === name.toLowerCase() && (parent === undefined || panel.parent === parent))?.id ?? null
}

function resourceChildren(block: VguiResourceNode): readonly VguiResourceNode[] {
  return block.children.filter((child) => child.value === null && scalar(child, "ControlName") !== null)
}

function shallow(block: VguiResourceNode): VguiResourceNode {
  return Object.freeze({ ...block, children: Object.freeze(block.children.filter((child) => child.value !== null || child.name.toLowerCase() === "controlname")) })
}

function applyChildren(runtime: VguiRuntime, parent: VguiPanelId, source: VguiResourceDocument, blocks: readonly VguiResourceNode[], activeConditions: readonly string[]): void {
  const selected = blocks.filter((block) => block.value === null && scalar(block, "ControlName") !== null)
  if (selected.length === 0) return
  apply(runtime, {
    kind: "replace-resource",
    parent,
    document: document(source, `children-${parent}`, node(source.root.name, selected.map(shallow))),
    selection: { activeConditions, resolutionSuffixes: ["_hidef"] },
  })
  for (const block of selected) {
    const child = find(runtime, scalar(block, "fieldName") ?? block.name, parent)
    if (child !== null) applyChildren(runtime, child, source, resourceChildren(block), activeConditions)
  }
}

function applyPanelResource(runtime: VguiRuntime, panel: VguiPanelId, source: VguiResourceDocument, activeConditions: readonly string[]): void {
  const snapshot = runtime.snapshot().panels.find((candidate) => candidate.id === panel)!
  const self = source.root.children.find((block) => (scalar(block, "fieldName") ?? block.name).toLowerCase() === snapshot.name.toLowerCase())
  if (self && snapshot.parent !== null) {
    apply(runtime, {
      kind: "replace-resource",
      parent: snapshot.parent,
      document: document(source, `self-${panel}`, node(source.root.name, [shallow(self)])),
      selection: { activeConditions, resolutionSuffixes: ["_hidef"] },
    })
  }
  applyChildren(runtime, panel, source, source.root.children.filter((block) => block !== self), activeConditions)
}

class Integration implements Tf2HudIntegration {
  readonly #runtime: VguiRuntime
  readonly #onCommand: (command: Tf2HudCommand) => void
  readonly #diagnostics: Tf2HudIntegrationDiagnostic[] = []
  readonly #diagnosticSubjects = new Set<string>()
  readonly #animationTrace: string[] = []
  readonly #baseBounds = new Map<string, Readonly<{ x: number; y: number; width: number; height: number }>>()
  readonly #panels = new Map<string, VguiPanelId>()
  readonly #publishedValues = new Map<string, string>()
  #previous: Tf2HudAvailability<Tf2HudSnapshot> = tf2HudUnavailable("initial")
  #binding: Tf2HudBinding | null = null
  #viewport: VguiViewport
  #destroyed = false

  constructor(request: Tf2HudIntegrationRequest) {
    this.#onCommand = request.onCommand
    this.#viewport = Object.freeze({ ...request.viewport })
    const availableImages = new Set(request.resources.clientScheme.images.map((image) => image.name.toLowerCase()))
    const missingImages = TF2_HUD_DYNAMIC_IMAGES.filter((image) => !availableImages.has(image.toLowerCase()))
    if (missingImages.length > 0) throw new Error(`TF2 HUD dynamic images are unavailable: ${missingImages.join(",")}`)
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-hud",
      root: request.root,
      rootControl: { control: "EditablePanel", name: "HudViewport" },
      viewport: request.viewport,
      limits: LIMITS,
      clock: request.clock,
      random: request.random,
      scheme: request.resources.clientScheme,
      localization: request.resources.localization,
      animationScripts: request.resources.animations,
      customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion,
      onRequest: () => {},
    })
    if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#runtime.deferPresentation(() => {
    apply(this.#runtime, { kind: "set-panel-state", panel: 1, proportional: true, mouseInput: false, keyboardInput: false })
    const roots = [
      ["HudPlayerStatus", "CTFHudElement"],
      ["HudWeaponAmmo", "CTFHudElement"],
      ["HudWeaponSelection", "CTFHudElement"],
      ["HudCrosshair", "CTFHudElement"],
    ] as const
    for (const [name, control] of roots) apply(this.#runtime, { kind: "create-panel", parent: 1, control, name })
    const layout = request.resources.document(HUD_LAYOUT)
    const selectedLayout = layout.root.children.filter((block) => roots.some(([name]) => name === (scalar(block, "fieldName") ?? block.name)))
    apply(this.#runtime, {
      kind: "replace-resource",
      parent: 1,
      document: document(layout, "selected", node(layout.root.name, selectedLayout.map(shallow))),
      selection: { activeConditions: request.resources.activeConditions, resolutionSuffixes: ["_hidef"] },
    })
    const status = find(this.#runtime, "HudPlayerStatus")!
    const playerClass = apply(this.#runtime, { kind: "create-panel", parent: status, control: "CTFHudElement", name: "HudPlayerClass" })!
    const playerHealth = apply(this.#runtime, { kind: "create-panel", parent: status, control: "CTFHudElement", name: "HudPlayerHealth" })!
    apply(this.#runtime, { kind: "create-panel", parent: playerHealth, control: "CTFHealthPanel", name: "PlayerStatusHealthImage" })
    applyPanelResource(this.#runtime, playerClass, request.resources.document(HUD_CLASS), request.resources.activeConditions)
    applyPanelResource(this.#runtime, playerHealth, request.resources.document(HUD_HEALTH), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudWeaponAmmo")!, request.resources.document(HUD_AMMO), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudWeaponSelection")!, request.resources.document(HUD_WEAPONS), request.resources.activeConditions)
    const panels = this.#runtime.snapshot().panels
    for (const panel of panels) {
      this.#panels.set(panel.name.toLowerCase(), panel.id)
      apply(this.#runtime, { kind: "set-panel-state", panel: panel.id, mouseInput: false, keyboardInput: false })
    }
    this.#captureBaseBounds(panels)
    })
  }

  #diagnostic(code: Tf2HudIntegrationDiagnostic["code"], subject: string): void {
    const identity = `${code}:${subject}`
    if (this.#diagnosticSubjects.has(identity) || this.#diagnostics.length >= LIMITS.maxDiagnostics) return
    this.#diagnosticSubjects.add(identity)
    this.#diagnostics.push(Object.freeze({ code, subject }))
  }

  #captureBaseBounds(panels = this.#runtime.snapshot().panels): void {
    for (const panel of panels) this.#baseBounds.set(panel.name, panel.bounds)
  }

  #value(value: Tf2HudPanelValue): void {
    const panel = this.#panels.get(value.panel.toLowerCase()) ?? null
    if (panel === null) {
      this.#diagnostic("PanelUnavailable", `${value.kind}:${value.panel}`)
      return
    }
    const identity = value.kind === "dialog-variable"
      ? `${value.kind}:${value.panel}:${value.variable}`
      : value.kind === "scalar" || value.kind === "color"
        ? `${value.kind}:${value.panel}:${value.property}`
        : `${value.kind}:${value.panel}`
    const fingerprint = JSON.stringify(value.value)
    if (this.#publishedValues.get(identity) === fingerprint) return
    if (value.kind === "visible") {
      apply(this.#runtime, { kind: "set-panel-state", panel, visible: value.value })
      this.#publishedValues.set(identity, fingerprint)
      return
    }
    if (value.value.kind === "unavailable") {
      this.#diagnostic("ValueUnavailable", `${value.kind}:${value.panel}:${value.value.reason}`)
      return
    }
    if (value.kind === "dialog-variable") {
      const available = value.value.value
      const rendered = typeof available === "object"
        ? `${available.token}:${available.parameters.join(",")}`
        : available
      apply(this.#runtime, { kind: "set-dialog-variable", panel, name: value.variable, value: rendered })
    } else if (value.kind === "image") {
      try { apply(this.#runtime, { kind: "mutate-control", panel, mutation: { image: value.value.value } }) }
      catch { this.#diagnostic("ValueUnavailable", `image:${value.panel}:${value.value.value}`); return }
    } else if (value.kind === "color") {
      const color = Object.freeze([...value.value.value]) as readonly [number, number, number, number]
      apply(this.#runtime, { kind: "mutate-control", panel, mutation: value.property.toLowerCase() === "drawcolor" ? { drawColor: color } : { foregroundColor: color } })
    } else if (value.kind === "scalar") {
      if (value.property === "fill") {
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: {
          imageFill: value.value.value,
          image: value.value.value <= 0 ? "hud/health_dead" : "hud/health_color",
        } })
      } else if (value.property === "boundsAdjustment") {
        const base = this.#baseBounds.get(value.panel)
        if (!base) {
          this.#diagnostics.push(Object.freeze({ code: "PanelUnavailable", subject: `bounds:${value.panel}` }))
          return
        }
        const amount = value.value.value
        apply(this.#runtime, { kind: "set-bounds", panel, bounds: { x: base.x - amount, y: base.y - amount, width: base.width + amount * 2, height: base.height + amount * 2 } })
      } else {
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: { scalarProperties: { [value.property]: value.value.value } } })
      }
    }
    this.#publishedValues.set(identity, fingerprint)
  }

  #applyValues(binding: Tf2HudBinding): void {
    for (const value of binding.values) this.#value(value)
  }

  publish(publication: CompactSessionSimulationPublication, context: CompactSessionHudContext): Tf2HudBinding {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    return this.#runtime.deferPresentation(() => {
    const adapted = adaptCompactSessionHud(this.#previous, publication, context)
    const binding = bindTf2Hud(adapted)
    this.#applyValues(binding)
    for (const animation of binding.animations) {
      const parent = animation.target === "viewport" ? 1 : find(this.#runtime, animation.target)
      if (parent === null) {
        this.#diagnostic("AnimationUnavailable", `${animation.target}:${animation.sequence}`)
        continue
      }
      try {
        apply(this.#runtime, { kind: "start-animation-sequence", parent, sequence: animation.sequence, cancelable: true })
        this.#animationTrace.push(`${animation.tick}:${animation.ordinal}:${animation.target}:${animation.sequence}`)
      } catch {
        this.#diagnostic("AnimationUnavailable", `${animation.target}:${animation.sequence}`)
      }
    }
    this.#previous = tf2HudAvailable(binding.facts)
    this.#binding = binding
    return binding
    })
  }

  action(action: Tf2HudAction): Tf2HudAvailability<Tf2HudCommand> {
    if (!this.#binding) return tf2HudUnavailable("initial")
    const command = bindTf2HudAction(this.#binding.facts, action)
    if (command.kind === "available") this.#onCommand(command.value)
    return command
  }

  frame(timeSeconds: number): void { apply(this.#runtime, { kind: "frame", timeSeconds }) }
  setViewport(viewport: VguiViewport): void {
    if (viewport.width === this.#viewport.width
      && viewport.height === this.#viewport.height
      && viewport.devicePixelRatio === this.#viewport.devicePixelRatio) return
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-viewport", viewport })
      this.#viewport = Object.freeze({ ...viewport })
      this.#captureBaseBounds()
      if (this.#binding) {
        this.#publishedValues.clear()
        this.#applyValues(this.#binding)
      }
    })
  }
  probe(): Tf2HudIntegrationProbe {
    const panels = [
      "PlayerStatusHealthImage", "HudWeaponAmmo", "HudWeaponAmmoBG", "modelpanel0",
      "PlayerStatusClassImage", "PlayerStatusClassImageBG", "classmodelpanel", "classmodelpanelBG",
      "PlayerStatusSpyImage", "PlayerStatusSpyOutlineImage", "PlayerStatus_WheelOfDoom",
      ...TF2_GROUPED_CONDITION_PANELS.map((item) => item.panel),
      ...TF2_INDEPENDENT_CONDITION_PANELS.map((item) => item.panel),
    ]
      .map((name) => this.#panels.get(name.toLowerCase()))
      .filter((panel): panel is VguiPanelId => panel !== undefined)
    return Object.freeze({
      panels: this.#runtime.snapshotPanels(panels),
      animationTrace: Object.freeze([...this.#animationTrace]),
    })
  }
  snapshot(): Tf2HudIntegrationSnapshot {
    return Object.freeze({
      binding: this.#binding,
      vgui: this.#runtime.snapshot(),
      diagnostics: Object.freeze([...this.#diagnostics]),
      animationTrace: Object.freeze([...this.#animationTrace]),
    })
  }
  setPlayerClassUsePlayerModel(value: boolean): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    if (typeof value !== "boolean") throw new Error("TF2 HUD player-model setting is invalid")
    const current = this.#binding
    if (!current || current.facts.player.kind !== "available") return
    const currentPlayer = current.facts.player.value
    this.#runtime.deferPresentation(() => {
      const player = Object.freeze({ ...currentPlayer, playerClassUsePlayerModel: value })
      const snapshot: Tf2HudSnapshot = Object.freeze({ ...current.facts, player: tf2HudAvailable(player) })
      const binding = bindTf2Hud(Object.freeze({
        previous: tf2HudUnavailable<Tf2HudSnapshot>("replay-discontinuity"),
        snapshot,
        events: Object.freeze([]),
      }))
      this.#publishedValues.clear()
      this.#applyValues(binding)
      this.#previous = tf2HudAvailable(binding.facts)
      this.#binding = binding
    })
  }
  reset(reason: "map-replaced" | "disconnect"): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    void reason
    this.#runtime.deferPresentation(() => {
      const unavailable = tf2HudUnavailable<never>("replay-discontinuity")
      const snapshot: Tf2HudSnapshot = Object.freeze({
        tick: 0n,
        player: unavailable,
        scoreboard: unavailable,
        freezePanel: unavailable,
      })
      const binding = bindTf2Hud(Object.freeze({ previous: unavailable, snapshot, events: Object.freeze([]) }))
      this.#publishedValues.clear()
      this.#applyValues(binding)
      this.#previous = unavailable
      this.#binding = null
      this.#animationTrace.length = 0
    })
  }
  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    apply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2HudIntegration(request: Tf2HudIntegrationRequest): Tf2HudIntegration {
  return Object.freeze(new Integration(request))
}
