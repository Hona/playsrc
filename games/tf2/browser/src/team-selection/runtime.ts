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
  TF2_TEAM_SELECTION_INITIAL_STATE,
  transitionTf2TeamSelection,
  type Tf2TeamChoice,
  type Tf2TeamSelectionEvent,
  type Tf2TeamSelectionRequest,
  type Tf2TeamSelectionState,
  type Tf2TeamSelectionTransition,
} from "./model"

const TEAM_SELECTION_PATH = "resource/ui/teammenu.res"
const MODEL_NAMES = ["MenuBG", "bluedoor", "reddoor", "autodoor", "spectate"] as const
const BUTTON_NAMES: Readonly<Record<Tf2TeamChoice, string>> = Object.freeze({
  blue: "teambutton0",
  red: "teambutton1",
  auto: "teambutton2",
  spectate: "teambutton3",
})
const TAB_ORDER: readonly Tf2TeamChoice[] = Object.freeze(["auto", "spectate", "blue", "red"])

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 128,
  maxHierarchyDepth: 16,
  maxChildrenPerPanel: 96,
  maxResourceNodes: 1024,
  maxResourceDepth: 16,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4095,
  maxTextCodeUnits: 65535,
  maxDialogVariables: 64,
  maxLocalizationTokens: 4096,
  maxSchemeColors: 1024,
  maxSchemeSettings: 2048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1024,
  maxAnimationCommands: 8192,
  maxActiveAnimations: 128,
  maxDelayedCommands: 128,
  maxQueuedMessages: 256,
  maxDiagnostics: 256,
  maxDomNodes: 512,
  maxListeners: 64,
})

export type Tf2TeamSelectionModelPanel = Readonly<{
  name: typeof MODEL_NAMES[number]
  model: string
  skin: number
  fov: number
  origin: readonly [number, number, number]
  angles: readonly [number, number, number]
  animation: string
  sequence: string
  animationRevision: number
  modelRevision: number
  bounds: Readonly<{ x: number; y: number; width: number; height: number }>
}>

export type Tf2TeamSelectionIntegration = Readonly<{
  state(): Tf2TeamSelectionState
  snapshot(): VguiRuntimeSnapshot
  modelPanels(): readonly Tf2TeamSelectionModelPanel[]
  dispatch(event: Tf2TeamSelectionEvent): Tf2TeamSelectionTransition
  handleKey(event: Pick<KeyboardEvent, "code" | "repeat" | "preventDefault" | "stopImmediatePropagation">, changeTeamBinding: boolean): boolean
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  destroy(): void
}>

export type Tf2TeamSelectionIntegrationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onRequest(request: Tf2TeamSelectionRequest): void
  onModelPanels(panels: readonly Tf2TeamSelectionModelPanel[]): void
}>

const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const object = (node: VguiResourceNode, name: string): VguiResourceNode | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value === null) ?? null
const shallow = (node: VguiResourceNode): VguiResourceNode => {
  const children = node.children.filter((child) => child.value !== null || child.name.toLowerCase() === "controlname")
  if (scalar(node, "ControlName") === "CTeamMenu") {
    children.push(Object.freeze({ name: "bgcolor_override", value: "0 0 0 0", condition: null, children: Object.freeze([]) }))
  }
  if (scalar(node, "ControlName") === "CTFTeamButton") {
    for (const name of ["defaultBgColor_override", "armedBgColor_override", "depressedBgColor_override", "selectedBgColor_override"]) {
      children.push(Object.freeze({ name, value: "0 0 0 0", condition: null, children: Object.freeze([]) }))
    }
  }
  return Object.freeze({ ...node, children: Object.freeze(children) })
}
const document = (source: VguiResourceDocument, suffix: string, children: readonly VguiResourceNode[]): VguiResourceDocument => Object.freeze({
  logicalIdentity: `${source.logicalIdentity}/${suffix}`,
  revision: source.revision,
  root: Object.freeze({ ...source.root, children: Object.freeze(children) }),
})

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`TF2 team selection ${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function find(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((value) => value.name.toLowerCase() === name.toLowerCase()
    && (parent === undefined || value.parent === parent))?.id ?? null
}

function authoredNumber(node: VguiResourceNode, name: string, optional = false): number {
  const source = scalar(node, name)
  if (optional && source === null) return 0
  const value = Number(source)
  if (source === null || !Number.isFinite(value)) throw new Error(`TF2 team selection model field is invalid: ${node.name}:${name}`)
  return value
}

class Integration implements Tf2TeamSelectionIntegration {
  readonly #runtime: VguiRuntime
  readonly #root: HTMLElement
  readonly #source: VguiResourceDocument
  readonly #onRequest: Tf2TeamSelectionIntegrationRequest["onRequest"]
  readonly #onModelPanels: Tf2TeamSelectionIntegrationRequest["onModelPanels"]
  readonly #animations = new Map<string, string>()
  readonly #animationRevisions = new Map<string, number>()
  readonly #entered = new Set<Tf2TeamChoice>()
  readonly #hoverDeadlines = new Map<Tf2TeamChoice, number>()
  readonly #disabled = new Map<Tf2TeamChoice, boolean>()
  readonly #clock: Tf2TeamSelectionIntegrationRequest["clock"]
  #modelRevision = 0
  #animationRevision = 0
  #nextTick = 0
  #owner = 0
  #state = TF2_TEAM_SELECTION_INITIAL_STATE
  #destroyed = false

  constructor(request: Tf2TeamSelectionIntegrationRequest) {
    this.#root = request.root
    this.#source = request.resources.document(TEAM_SELECTION_PATH)
    this.#onRequest = request.onRequest
    this.#onModelPanels = request.onModelPanels
    this.#clock = request.clock
    this.#nextTick = Math.floor(request.clock.nowSeconds() * 1000) + 100
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-team-selection",
      root: request.root,
      rootControl: { control: "EditablePanel", name: "TeamSelectionViewport" },
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
        if (value.command === "cancelmenu" || value.command === "vguicancel") {
          this.dispatch({ kind: "cancel" })
          return
        }
        const match = /^jointeam (red|blue|spectate|auto)$/u.exec(value.command)
        if (match) this.dispatch({ kind: "select", team: match[1] as Tf2TeamChoice })
      },
    })
    if (!initialized.ok) throw new Error(`TF2 team selection ${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: false, proportional: true })
      const frame = this.#source.root.children.find((node) => node.name === "team")
      if (!frame) throw new Error("TF2 team selection authored frame is unavailable")
      this.#owner = apply(this.#runtime, {
        kind: "create-panel", parent: 1, control: "CTeamMenu", name: "team",
      })!
      apply(this.#runtime, {
        kind: "replace-resource", parent: 1,
        document: document(this.#source, "frame", [shallow(frame)]),
        selection: { activeConditions: request.resources.activeConditions, resolutionSuffixes: request.resources.resolutionSuffixes },
      })
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#owner, proportional: true })
      const children = this.#source.root.children.filter((node) => node !== frame
        && node.name !== "Footer" && node.value === null && scalar(node, "ControlName") !== null)
      apply(this.#runtime, {
        kind: "replace-resource", parent: this.#owner,
        document: document(this.#source, "controls", children.map(shallow)),
        selection: { activeConditions: request.resources.activeConditions, resolutionSuffixes: request.resources.resolutionSuffixes },
      })
      for (const name of MODEL_NAMES) {
        const panel = find(this.#runtime, name, this.#owner)
        if (panel === null) throw new Error(`TF2 team selection authored model panel is unavailable: ${name}`)
        apply(this.#runtime, { kind: "set-panel-state", panel, mouseInput: false, keyboardInput: false })
        this.#animations.set(name, "idle_enabled")
      }
      for (const team of TAB_ORDER) {
        const panel = find(this.#runtime, BUTTON_NAMES[team], this.#owner)
        if (panel === null) throw new Error(`TF2 team selection authored button is unavailable: ${team}`)
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: { text: "" } })
      }
      for (const name of ["TeamMenuSelect", "TeamMenuAuto", "TeamMenuSpectate", "BlueCount", "RedCount", "ShadedBar"]) {
        const panel = find(this.#runtime, name, this.#owner)
        if (panel !== null) apply(this.#runtime, { kind: "set-panel-state", panel, mouseInput: false, keyboardInput: false })
      }
    })
    for (const team of TAB_ORDER) {
      request.root.querySelector?.<HTMLElement>(`[data-vgui-name="${BUTTON_NAMES[team]}"]`)
        ?.setAttribute("aria-label", team === "blue" ? "BLU" : team === "red" ? "RED" : team === "auto" ? "Auto-assign" : "Spectate")
    }
    request.root.addEventListener("pointermove", this.#pointerMove)
    request.root.addEventListener("pointerleave", this.#pointerLeave)
  }

  readonly #pointerMove = (event: Event): void => {
    if (!this.#state.visible) return
    let target = event.target as HTMLElement | null
    while (target && target !== this.#root) {
      const team = TAB_ORDER.find((value) => BUTTON_NAMES[value] === target?.dataset?.vguiName)
      if (team) {
        this.dispatch({ kind: "hover", team })
        return
      }
      target = target.parentElement
    }
    this.dispatch({ kind: "hover", team: null })
  }

  readonly #pointerLeave = (): void => {
    if (this.#state.visible) this.dispatch({ kind: "hover", team: null })
  }

  #setAnimation(name: string, animation: string): void {
    const authored = this.#source.root.children.find(node => node.name === name)
    const model = authored && object(authored, "model")
    if (!model?.children.some(child => child.name.toLowerCase() === "animation" && scalar(child, "name") === animation)) return
    this.#animations.set(name, animation)
    this.#animationRevision += 1
    this.#animationRevisions.set(name, ((this.#animationRevisions.get(name) ?? 0) + 1) >>> 0)
  }

  #enter(team: Tf2TeamChoice, entered: boolean): void {
    this.#hoverDeadlines.delete(team)
    if (entered) {
      this.#entered.add(team)
      const button = this.#source.root.children.find(node => node.name === BUTTON_NAMES[team])
      const delay = button ? Number(scalar(button, "hover") ?? -1) : -1
      if (delay > 0) this.#hoverDeadlines.set(team, Math.fround(Math.fround(this.#clock.nowSeconds()) + delay))
    } else this.#entered.delete(team)
    this.#setAnimation(this.#modelName(team), `${entered ? "enter" : "exit"}_${this.#disabled.get(team) ? "disabled" : "enabled"}`)
  }

  #tickButtons(time: number): void {
    const server = this.#state.server
    if (!server) return
    for (const team of TAB_ORDER) {
      const disabled = team === "red" ? server.redDisabled : team === "blue" ? server.blueDisabled : false
      if (disabled !== (this.#disabled.get(team) ?? false)) {
        this.#disabled.set(team, disabled)
        if (this.#entered.has(team)) this.#enter(team, true)
        else this.#setAnimation(this.#modelName(team), disabled ? "idle_disabled" : "idle_enabled")
      }
      const deadline = this.#hoverDeadlines.get(team)
      if (deadline !== undefined && deadline < Math.fround(time)) {
        this.#hoverDeadlines.delete(team)
        this.#setAnimation(this.#modelName(team), disabled ? "hover_disabled" : "hover_enabled")
      }
    }
  }

  #present(previous: Tf2TeamSelectionState, event: Tf2TeamSelectionEvent): void {
    const state = this.#state
    if (state.visible && !previous.visible) {
      this.#modelRevision += 1
      this.#entered.clear()
      this.#hoverDeadlines.clear()
      for (const name of MODEL_NAMES) this.#setAnimation(name, "idle_enabled")
    }
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: state.visible })
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#owner, visible: state.visible })
      if (!state.visible || !state.server) return
      const server = state.server
      apply(this.#runtime, { kind: "set-dialog-variable", panel: this.#owner, name: "redcount", value: String(server.redCount) })
      apply(this.#runtime, { kind: "set-dialog-variable", panel: this.#owner, name: "bluecount", value: String(server.blueCount) })
      const visible: Readonly<Record<string, boolean>> = {
        teambutton0: true,
        teambutton1: true,
        teambutton2: server.autoAssignVisible,
        teambutton3: server.spectatorsVisible,
        TeamMenuSpectate: server.spectatorsVisible,
        CancelButton: server.cancelVisible,
        HighlanderLabel: server.highlander,
        HighlanderLabelShadow: server.highlander,
        TeamsFullLabel: server.teamsFull,
        TeamsFullLabelShadow: server.teamsFull,
        TeamsFullArrow: server.teamsFullArrow,
        autodoor: server.autoAssignVisible,
        spectate: server.spectatorsVisible,
      }
      for (const [name, shown] of Object.entries(visible)) {
        const panel = find(this.#runtime, name, this.#owner)
        if (panel !== null) apply(this.#runtime, { kind: "set-panel-state", panel, visible: shown })
      }
      for (const [team, disabled] of [["red", server.redDisabled], ["blue", server.blueDisabled]] as const) {
        const button = find(this.#runtime, BUTTON_NAMES[team], this.#owner)!
        apply(this.#runtime, { kind: "set-panel-state", panel: button, enabled: true })
        this.#root.querySelector?.<HTMLElement>(`[data-vgui-name="${BUTTON_NAMES[team]}"]`)?.setAttribute("aria-disabled", String(disabled))
      }
      if (previous.hovered !== state.hovered) {
        if (previous.hovered) {
          this.#enter(previous.hovered, false)
        }
        if (state.hovered) {
          this.#enter(state.hovered, true)
        }
      }
      if (event.kind === "focus" && previous.focused !== state.focused) {
        if (previous.focused) this.#enter(previous.focused, false)
        if (state.focused) this.#enter(state.focused, true)
      }
    })
    if (state.focused && previous.focused !== state.focused) {
      this.#root.querySelector?.<HTMLElement>(`[data-vgui-name="${BUTTON_NAMES[state.focused]}"]`)?.focus?.()
    }
    this.#syncAccessibility()
    this.#onModelPanels(this.modelPanels())
  }

  #syncAccessibility(): void {
    const state = this.#state
    if (!state.visible || !state.server) return
    for (const team of TAB_ORDER) {
      const button = this.#root.querySelector?.<HTMLElement>(`[data-vgui-name="${BUTTON_NAMES[team]}"]`)
      button?.setAttribute("aria-label", team === "blue" ? "BLU" : team === "red" ? "RED" : team === "auto" ? "Auto-assign" : "Spectate")
      if (team === "red" || team === "blue") {
        button?.setAttribute("aria-disabled", String(team === "red" ? state.server.redDisabled : state.server.blueDisabled))
      }
    }
  }

  #modelName(team: Tf2TeamChoice): typeof MODEL_NAMES[number] {
    return team === "red" ? "reddoor" : team === "blue" ? "bluedoor" : team === "auto" ? "autodoor" : "spectate"
  }

  state(): Tf2TeamSelectionState { return this.#state }
  snapshot(): VguiRuntimeSnapshot { return this.#runtime.snapshot() }

  modelPanels(): readonly Tf2TeamSelectionModelPanel[] {
    if (!this.#state.visible || !this.#state.server) return Object.freeze([])
    return Object.freeze(MODEL_NAMES.flatMap((name): Tf2TeamSelectionModelPanel[] => {
      if (name === "autodoor" && !this.#state.server!.autoAssignVisible) return []
      if (name === "spectate" && !this.#state.server!.spectatorsVisible) return []
      const authored = this.#source.root.children.find((node) => node.name === name)
      const model = authored && object(authored, "model")
      const snapshot = this.#runtime.snapshot().panels.find((node) => node.name === name)
      if (!authored || !model || !snapshot) throw new Error(`TF2 team selection authored model descriptor is unavailable: ${name}`)
      const originX = scalar(model, "origin_x")
      if (originX === null || !Number.isFinite(Number(originX))) throw new Error(`TF2 team selection authored origin is invalid: ${name}`)
      const animation = this.#animations.get(name) ?? "idle_enabled"
      const authoredAnimation = model.children.find((child) => child.name.toLowerCase() === "animation"
        && child.value === null && scalar(child, "name") === animation)
      return [Object.freeze({
        name,
        model: scalar(model, "modelname") ?? "",
        skin: authoredNumber(model, "skin"),
        fov: authoredNumber(authored, "fov"),
        origin: Object.freeze([Number(originX), authoredNumber(model, "origin_y"), authoredNumber(model, "origin_z")]) as readonly [number, number, number],
        angles: Object.freeze([authoredNumber(model, "angles_x"), authoredNumber(model, "angles_y"), authoredNumber(model, "angles_z")]) as readonly [number, number, number],
        animation,
        sequence: authoredAnimation ? scalar(authoredAnimation, "sequence") ?? "" : "idle",
        animationRevision: this.#animationRevisions.get(name) ?? 0,
        modelRevision: this.#modelRevision,
        bounds: snapshot.bounds,
      })]
    }))
  }

  dispatch(event: Tf2TeamSelectionEvent): Tf2TeamSelectionTransition {
    if (this.#destroyed) throw new Error("TF2 team selection integration is destroyed")
    const previous = this.#state
    const transition = transitionTf2TeamSelection(previous, event)
    if (transition.disposition !== "applied") return transition
    this.#state = transition.state
    this.#present(previous, event)
    if (transition.request) this.#onRequest(transition.request)
    return transition
  }

  handleKey(event: Pick<KeyboardEvent, "code" | "repeat" | "preventDefault" | "stopImmediatePropagation">,
    changeTeamBinding: boolean): boolean {
    if (!this.#state.visible) return false
    if (event.repeat) {
      if (!["Space", "Enter", "NumpadEnter", "Escape", "ArrowLeft", "ArrowRight"].includes(event.code) && !changeTeamBinding) return false
      event.preventDefault()
      event.stopImmediatePropagation()
      return true
    }
    let eventToDispatch: Tf2TeamSelectionEvent | undefined
    if (event.code === "Space") eventToDispatch = { kind: "select", team: "auto" }
    else if (event.code === "Enter" || event.code === "NumpadEnter") {
      if (this.#state.focused) eventToDispatch = { kind: "select", team: this.#state.focused }
    } else if (event.code === "Escape" || changeTeamBinding) eventToDispatch = { kind: "cancel" }
    else if (event.code === "ArrowLeft" || event.code === "ArrowRight") {
      const direction = event.code === "ArrowRight" ? 1 : -1
      const current = this.#state.focused ? TAB_ORDER.indexOf(this.#state.focused) : -1
      for (let offset = 1; offset <= TAB_ORDER.length; offset += 1) {
        const index = (current + direction * offset + TAB_ORDER.length * 2) % TAB_ORDER.length
        const team = TAB_ORDER[index]!
        const transition = transitionTf2TeamSelection(this.#state, { kind: "focus", team })
        if (transition.disposition === "applied") {
          eventToDispatch = { kind: "focus", team }
          break
        }
      }
    }
    if (!eventToDispatch) return false
    this.dispatch(eventToDispatch)
    event.preventDefault()
    event.stopImmediatePropagation()
    return true
  }

  frame(timeSeconds: number): void {
    if (this.#state.visible) {
      apply(this.#runtime, { kind: "frame", timeSeconds })
      this.#syncAccessibility()
      if (Math.floor(timeSeconds * 1000) >= this.#nextTick) {
        this.#nextTick = Math.floor(timeSeconds * 1000) + 100
        const before = this.#animationRevision
        this.#tickButtons(timeSeconds)
        if (before !== this.#animationRevision) this.#onModelPanels(this.modelPanels())
      }
    }
  }

  setViewport(viewport: VguiViewport): void {
    this.#runtime.deferPresentation(() => apply(this.#runtime, { kind: "set-viewport", viewport }))
    if (this.#state.visible) this.#onModelPanels(this.modelPanels())
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.#root.removeEventListener("pointermove", this.#pointerMove)
    this.#root.removeEventListener("pointerleave", this.#pointerLeave)
    this.#onModelPanels(Object.freeze([]))
    apply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2TeamSelectionIntegration(request: Tf2TeamSelectionIntegrationRequest): Tf2TeamSelectionIntegration {
  return Object.freeze(new Integration(request))
}
