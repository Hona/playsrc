import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiViewport,
} from "@playsrc/vgui"
import type { Tf2VguiResources } from "../ui-integration"
import {
  TF2_CLASS_SELECTION_CLASSES,
  TF2_CLASS_SELECTION_INITIAL_STATE,
  tf2ClassSelectionByMenuIndex,
  tf2ClassSelectionClass,
  tf2ClassSelectionImage,
  transitionTf2ClassSelection,
  type Tf2ClassIdentity,
  type Tf2ClassSelectionEvent,
  type Tf2ClassSelectionIdentity,
  type Tf2ClassSelectionRequest,
  type Tf2ClassSelectionState,
  type Tf2ClassSelectionTeam,
  type Tf2ClassSelectionTransition,
} from "./model"

const CLASS_SELECTION_PATH = "resource/ui/classselection.res"
const CLASS_TIPS_LIST_PATH = "resource/ui/classtipslist.res"
const CLASS_TIPS_ITEM_PATH = "resource/ui/classtipsitem.res"

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 256,
  maxHierarchyDepth: 24,
  maxChildrenPerPanel: 128,
  maxResourceNodes: 2048,
  maxResourceDepth: 24,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4095,
  maxTextCodeUnits: 65535,
  maxDialogVariables: 128,
  maxLocalizationTokens: 4096,
  maxSchemeColors: 1024,
  maxSchemeSettings: 2048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1024,
  maxAnimationCommands: 8192,
  maxActiveAnimations: 256,
  maxDelayedCommands: 256,
  maxQueuedMessages: 512,
  maxDiagnostics: 512,
  maxDomNodes: 1024,
  maxListeners: 64,
})

export type Tf2ClassSelectionModelPanel = Readonly<{
  name: "MenuBG" | "TFPlayerModel"
  model: string
  skin: number
  fov: number
  origin: readonly [number, number, number]
  angles: readonly [number, number, number]
  bounds: Readonly<{ x: number; y: number; width: number; height: number }>
}>

export type Tf2ClassSelectionIntegration = Readonly<{
  state(): Tf2ClassSelectionState
  snapshot(): VguiRuntimeSnapshot
  modelPanels(): readonly Tf2ClassSelectionModelPanel[]
  dispatch(event: Tf2ClassSelectionEvent): Tf2ClassSelectionTransition
  handleKey(event: Pick<KeyboardEvent, "code" | "repeat" | "preventDefault" | "stopImmediatePropagation">, changeClassBinding: boolean): boolean
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  destroy(): void
}>

export type Tf2ClassSelectionIntegrationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onRequest(request: Tf2ClassSelectionRequest): void
  onModelPanels(panels: readonly Tf2ClassSelectionModelPanel[]): void
}>

const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const object = (node: VguiResourceNode, name: string): VguiResourceNode | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value === null) ?? null
const copy = (node: VguiResourceNode, children = node.children): VguiResourceNode => Object.freeze({ ...node, children: Object.freeze(children) })
const synthetic = (source: VguiResourceDocument, suffix: string, children: readonly VguiResourceNode[]): VguiResourceDocument => Object.freeze({
  logicalIdentity: `${source.logicalIdentity}/${suffix}`,
  revision: source.revision,
  root: copy(source.root, children),
})

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`TF2 class selection ${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function panel(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((candidate) => candidate.name.toLowerCase() === name.toLowerCase()
    && (parent === undefined || candidate.parent === parent))?.id ?? null
}

function shallow(node: VguiResourceNode): VguiResourceNode {
  return copy(node, node.children.filter((property) => property.value !== null || property.name.toLowerCase() === "controlname"))
}

function applyChildren(runtime: VguiRuntime, parent: VguiPanelId, source: VguiResourceDocument,
  children: readonly VguiResourceNode[], conditions: readonly string[], resolutionSuffixes: readonly string[]): void {
  const selected = children.filter((child) => child.value === null && scalar(child, "ControlName") !== null)
  if (selected.length === 0) return
  apply(runtime, {
    kind: "replace-resource",
    parent,
    document: synthetic(source, `level-${parent}`, selected.map(shallow)),
    selection: { activeConditions: conditions, resolutionSuffixes },
  })
  for (const child of selected) {
    const childPanel = panel(runtime, scalar(child, "fieldName") ?? child.name, parent)
    if (childPanel === null) continue
    applyChildren(runtime, childPanel, source, child.children, conditions, resolutionSuffixes)
    const parentSnapshot = runtime.snapshot().panels.find((candidate) => candidate.id === parent)
    if (parentSnapshot?.control === "CExImageButton" && scalar(child, "ControlName") === "ImagePanel") {
      apply(runtime, { kind: "set-panel-state", panel: childPanel, mouseInput: false, keyboardInput: false })
    }
  }
}

function finiteScalar(node: VguiResourceNode, name: string): number {
  const value = Number(scalar(node, name))
  if (!Number.isFinite(value)) throw new Error(`TF2 class selection authored model field is invalid: ${node.name}:${name}`)
  return value
}

class Integration implements Tf2ClassSelectionIntegration {
  readonly #runtime: VguiRuntime
  readonly #root: HTMLElement
  readonly #resources: Tf2VguiResources
  readonly #onRequest: Tf2ClassSelectionIntegrationRequest["onRequest"]
  readonly #onModelPanels: Tf2ClassSelectionIntegrationRequest["onModelPanels"]
  readonly #localization: ReadonlyMap<string, string>
  readonly #source: VguiResourceDocument
  readonly #buttons = new Map<Tf2ClassSelectionIdentity, VguiPanelId>()
  readonly #images = new Map<Tf2ClassSelectionIdentity, VguiPanelId>()
  readonly #tipPanels: VguiPanelId[] = []
  #owner: VguiPanelId
  #tips: VguiPanelId
  #state: Tf2ClassSelectionState = TF2_CLASS_SELECTION_INITIAL_STATE
  #destroyed = false

  constructor(request: Tf2ClassSelectionIntegrationRequest) {
    this.#root = request.root
    this.#resources = request.resources
    this.#onRequest = request.onRequest
    this.#onModelPanels = request.onModelPanels
    this.#source = request.resources.document(CLASS_SELECTION_PATH)
    this.#localization = new Map(request.resources.localization.tokens.map((token) => [token.name.toLowerCase(), token.value]))
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-class-selection",
      root: request.root,
      rootControl: { control: "EditablePanel", name: "ClassSelectionViewport" },
      viewport: request.viewport,
      limits: LIMITS,
      clock: request.clock,
      random: request.random,
      scheme: request.resources.clientScheme,
      localization: request.resources.localization,
      animationScripts: request.resources.animations,
      customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion,
      onRequest: (value) => {
        if (value.kind !== "command") return
        if (/^select (?:[1-9]|12)$/u.test(value.command)) {
          this.dispatch({ kind: "select", identity: Number(value.command.slice(7)) as Tf2ClassSelectionIdentity })
        } else if (value.command === "vguicancel" || value.command === "close") this.dispatch({ kind: "cancel" })
      },
    })
    if (!initialized.ok) throw new Error(`TF2 class selection ${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#owner = 0
    this.#tips = 0
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-panel-state", panel: 1, proportional: true, visible: false })
      const ownerBlock = this.#source.root.children.find((node) => node.name === "class")
      if (!ownerBlock) throw new Error("TF2 class selection authored frame is unavailable")
      this.#owner = apply(this.#runtime, {
        kind: "create-panel", parent: 1, control: "Frame", name: "class",
        properties: [{ name: "settitlebarvisible", value: "0" }, { name: "moveable", value: "0" }, { name: "sizeable", value: "0" }],
      })!
      apply(this.#runtime, {
        kind: "replace-resource", parent: 1,
        document: synthetic(this.#source, "frame", [shallow(ownerBlock)]),
        selection: { activeConditions: request.resources.activeConditions, resolutionSuffixes: request.resources.resolutionSuffixes },
      })
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#owner, proportional: true })
      applyChildren(this.#runtime, this.#owner, this.#source,
        this.#source.root.children.filter((value) => value !== ownerBlock), request.resources.activeConditions, request.resources.resolutionSuffixes)
      for (const playerClass of TF2_CLASS_SELECTION_CLASSES) {
        const button = panel(this.#runtime, playerClass.name, this.#owner)
        if (button === null) throw new Error(`TF2 class selection authored button is unavailable: ${playerClass.name}`)
        const image = panel(this.#runtime, "SubImage", button)
        if (image === null) throw new Error(`TF2 class selection authored image is unavailable: ${playerClass.name}`)
        this.#buttons.set(playerClass.identity, button)
        this.#images.set(playerClass.identity, image)
      }
      const tipsOwner = panel(this.#runtime, "ClassTipsPanel", this.#owner)
      if (tipsOwner === null) throw new Error("TF2 class selection authored tips owner is unavailable")
      const tipsSource = request.resources.document(CLASS_TIPS_LIST_PATH)
      applyChildren(this.#runtime, tipsOwner, tipsSource, tipsSource.root.children, request.resources.activeConditions, request.resources.resolutionSuffixes)
      this.#tips = panel(this.#runtime, "ClassTipsListPanel", tipsOwner) ?? 0
      if (this.#tips === 0) throw new Error("TF2 class selection authored tips list is unavailable")
      for (const name of ["Offense", "Defense", "Support", "CountLabel", "ClassMenuSelect"]) {
        const label = panel(this.#runtime, name, this.#owner)
        if (label !== null) apply(this.#runtime, { kind: "set-panel-state", panel: label, mouseInput: false, keyboardInput: false })
      }
      for (const name of ["EditLoadoutButton", "ResetButton"]) {
        const button = panel(this.#runtime, name, this.#owner)
        if (button !== null) apply(this.#runtime, { kind: "set-panel-state", panel: button, enabled: false })
      }
      for (const playerClass of TF2_CLASS_SELECTION_CLASSES.filter((value) => value.identity !== 12)) {
        const variable = playerClass.name === "heavyweapons" ? "numHeavy"
          : `num${playerClass.name[0]!.toUpperCase()}${playerClass.name.slice(1)}`
        apply(this.#runtime, { kind: "set-dialog-variable", panel: this.#owner, name: variable, value: "0" })
      }
    })
    request.root.addEventListener("pointermove", this.#pointerMove)
  }

  readonly #pointerMove = (event: Event): void => {
    if (!this.#state.visible) return
    let target = event.target as HTMLElement | null
    while (target && target !== this.#root) {
      const name = target.dataset?.vguiName
      const playerClass = name ? TF2_CLASS_SELECTION_CLASSES.find((value) => value.name === name.toLowerCase()) : undefined
      if (playerClass) {
        this.dispatch({ kind: "hover", identity: playerClass.identity })
        return
      }
      target = target.parentElement
    }
  }

  #updateTips(identity: Tf2ClassSelectionIdentity): void {
    for (const value of this.#tipPanels.splice(0)) apply(this.#runtime, { kind: "delete-panel", panel: value, deferred: false })
    if (identity === 12) return
    const count = Number(this.#localization.get(`classtips_${identity}_count`) ?? "0")
    const source = this.#resources.document(CLASS_TIPS_ITEM_PATH)
    const authored = source.root.children.find((value) => value.name === "ClassTipsItemPanel")
    if (!authored || !Number.isSafeInteger(count) || count < 0 || count > 32) throw new Error("TF2 class selection authored tip source is invalid")
    let position = 0
    for (let index = 1; index <= count; index += 1) {
      const text = this.#localization.get(`classtips_${identity}_${index}`)
      if (!text) continue
      const name = `ClassTipsItemPanel${index}`
      const item = apply(this.#runtime, { kind: "create-panel", parent: this.#tips, control: "CTFClassTipsItemPanel", name })!
      this.#tipPanels.push(item)
      apply(this.#runtime, { kind: "set-bounds", panel: item, bounds: { x: 0, y: position, width: 235, height: 30 } })
      applyChildren(this.#runtime, item, source, authored.children, this.#resources.activeConditions, this.#resources.resolutionSuffixes)
      const label = panel(this.#runtime, "TipLabel", item)
      const icon = panel(this.#runtime, "TipIcon", item)
      if (label === null || icon === null) throw new Error("TF2 class selection authored tip controls are unavailable")
      apply(this.#runtime, { kind: "mutate-control", panel: label, mutation: { text } })
      const image = this.#localization.get(`classtips_${identity}_${index}_icon`)
      if (image) apply(this.#runtime, { kind: "mutate-control", panel: icon, mutation: { image } })
      else apply(this.#runtime, { kind: "set-panel-state", panel: icon, visible: false })
      position += 30
    }
  }

  #present(): void {
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: this.#state.visible })
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#owner, visible: this.#state.visible })
      if (!this.#state.visible || this.#state.team === null) return
      for (const playerClass of TF2_CLASS_SELECTION_CLASSES) {
        const selected = playerClass.identity === this.#state.selected
        apply(this.#runtime, { kind: "mutate-control", panel: this.#buttons.get(playerClass.identity)!, mutation: { selected } })
        apply(this.#runtime, { kind: "mutate-control", panel: this.#images.get(playerClass.identity)!, mutation: {
          image: tf2ClassSelectionImage(playerClass.identity, this.#state.team, selected),
        } })
      }
      const cancel = panel(this.#runtime, "CancelButton", this.#owner)
      if (cancel !== null) apply(this.#runtime, { kind: "set-panel-state", panel: cancel, visible: !this.#state.initialJoin })
      const loadout = panel(this.#runtime, "EditLoadoutButton", this.#owner)
      if (loadout !== null) apply(this.#runtime, { kind: "set-panel-state", panel: loadout, visible: this.#state.selected !== 12 })
      this.#updateTips(this.#state.selected)
    })
    this.#onModelPanels(this.modelPanels())
  }

  state(): Tf2ClassSelectionState { return this.#state }
  snapshot(): VguiRuntimeSnapshot { return this.#runtime.snapshot() }

  modelPanels(): readonly Tf2ClassSelectionModelPanel[] {
    if (!this.#state.visible || this.#state.team === null) return Object.freeze([])
    const selected = tf2ClassSelectionClass(this.#state.selected)
    if (!selected) throw new Error("TF2 class selection preview identity is invalid")
    return Object.freeze((["MenuBG", "TFPlayerModel"] as const).map((name) => {
      const authored = this.#source.root.children.find((node) => node.name === name)
      const model = authored && object(authored, "model")
      const snapshot = this.#runtime.snapshot().panels.find((node) => node.name === name)
      if (!authored || !model || !snapshot) throw new Error(`TF2 class selection authored model panel is unavailable: ${name}`)
      return Object.freeze({
        name,
        model: name === "MenuBG" ? (scalar(model, "modelname") ?? "").toLowerCase() : selected.model,
        skin: name === "MenuBG" ? finiteScalar(model, "skin") : this.#state.team === 2 ? 0 : 1,
        fov: finiteScalar(authored, "fov"),
        origin: Object.freeze([finiteScalar(model, "origin_x"), finiteScalar(model, "origin_y"), finiteScalar(model, "origin_z")]) as readonly [number, number, number],
        angles: Object.freeze([finiteScalar(model, "angles_x"), finiteScalar(model, "angles_y"), finiteScalar(model, "angles_z")]) as readonly [number, number, number],
        bounds: snapshot.bounds,
      })
    }))
  }

  dispatch(event: Tf2ClassSelectionEvent): Tf2ClassSelectionTransition {
    if (this.#destroyed) throw new Error("TF2 class selection integration is destroyed")
    const transition = transitionTf2ClassSelection(this.#state, event)
    if (transition.disposition !== "applied") return transition
    this.#state = transition.state
    this.#present()
    if (transition.request) this.#onRequest(transition.request)
    return transition
  }

  handleKey(event: Pick<KeyboardEvent, "code" | "repeat" | "preventDefault" | "stopImmediatePropagation">,
    changeClassBinding: boolean): boolean {
    if (!this.#state.visible || event.repeat) return false
    let transition: Tf2ClassSelectionTransition | undefined
    const digit = /^(?:Digit|Numpad)([0-9])$/u.exec(event.code)
    if (digit && digit[1] !== "0") {
      const selected = tf2ClassSelectionByMenuIndex(Number(digit[1]))
      if (selected) transition = this.dispatch({ kind: "select", identity: selected.identity })
    } else if (event.code === "Enter" || event.code === "NumpadEnter" || event.code === "Space") {
      transition = this.dispatch({ kind: "confirm" })
    } else if (event.code === "Escape" || digit?.[1] === "0" || changeClassBinding) {
      transition = this.dispatch({ kind: "cancel" })
    }
    if (!transition) return false
    event.preventDefault()
    event.stopImmediatePropagation()
    return true
  }

  frame(timeSeconds: number): void {
    if (this.#state.visible) apply(this.#runtime, { kind: "frame", timeSeconds })
  }

  setViewport(viewport: VguiViewport): void {
    this.#runtime.deferPresentation(() => apply(this.#runtime, { kind: "set-viewport", viewport }))
    if (this.#state.visible) this.#onModelPanels(this.modelPanels())
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.#root.removeEventListener("pointermove", this.#pointerMove)
    this.#onModelPanels(Object.freeze([]))
    apply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2ClassSelectionIntegration(request: Tf2ClassSelectionIntegrationRequest): Tf2ClassSelectionIntegration {
  return Object.freeze(new Integration(request))
}
