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
  modelSurface?: HTMLCanvasElement
  backgroundSurface?: HTMLCanvasElement
  roster?(): readonly Readonly<{ fake: boolean; team: number; class: number }>[]
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
  const children = node.children.filter((property) => property.value !== null || property.name.toLowerCase() === "controlname")
  // CTFClassMenu::PerformLayout sizes this localized sentence, not its .res placeholder.
  if (node.name === "CountLabel") {
    for (const name of ["auto_wide_tocontents", "auto_tall_tocontents"]) {
      children.push(Object.freeze({ name, value: "1", condition: null, children: Object.freeze([]) }))
    }
  }
  return copy(node, children)
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
  readonly #tipRows: { panel: VguiPanelId; x: number; y: number; width: number; height: number }[] = []
  readonly #tipChrome = new Map<string, VguiPanelId>()
  #tipScroll = 0
  #tipContentHeight = 0
  #tipItemsHeight = 0
  #tipScrollbarWidth = 0
  #tipDrag: { pointer: number; start: number; scroll: number; pixels: number; range: number } | undefined
  #owner: VguiPanelId
  #tips: VguiPanelId
  #state: Tf2ClassSelectionState = TF2_CLASS_SELECTION_INITIAL_STATE
  #destroyed = false
  readonly #modelSurface: HTMLCanvasElement | undefined
  readonly #backgroundSurface: HTMLCanvasElement | undefined
  #viewport: VguiViewport
  #releaseSurface: (() => void) | undefined
  readonly #roster: NonNullable<Tf2ClassSelectionIntegrationRequest["roster"]>
  #rosterKey = ""

  constructor(request: Tf2ClassSelectionIntegrationRequest) {
    this.#root = request.root
    this.#modelSurface = request.modelSurface
    this.#backgroundSurface = request.backgroundSurface
    this.#viewport = request.viewport
    this.#roster = request.roster ?? (() => [])
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
        else if (/^ScrollButtonPressed [01]$/u.test(value.command)) {
          const height = this.#runtime.snapshotPanels([this.#tips])[0]!.bounds.height
          this.#scrollTips(this.#tipScroll + Math.trunc(height / 4) * (value.command.endsWith("0") ? -1 : 1))
        }
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
      for (const [name, image, command] of [
        ["UpArrow", "chalkboard_scroll_up", "ScrollButtonPressed 0"],
        ["DownArrow", "chalkboard_scroll_down", "ScrollButtonPressed 1"],
        ["Line", "chalkboard_scroll_line", ""], ["Box", "chalkboard_scroll_box", ""],
      ] as const) {
        const control = apply(this.#runtime, { kind: "create-panel", parent: this.#tips,
          control: command ? "CExImageButton" : "ImagePanel", name,
          properties: [{ name: "visible", value: "0" }, { name: "paintbackground", value: "0" },
            ...(command ? [{ name: "command", value: command }, { name: "labelText", value: "" }, { name: "zpos", value: "501" }]
              : [{ name: "image", value: image }, { name: "scaleImage", value: "1" }])],
        })!
        this.#tipChrome.set(name, control)
        if (command) {
          const sub = apply(this.#runtime, { kind: "create-panel", parent: control, control: "ImagePanel", name: "SubImage",
            properties: [{ name: "image", value: image }, { name: "scaleImage", value: "1" }] })!
          this.#tipChrome.set(`${name}Image`, sub)
          apply(this.#runtime, { kind: "set-panel-state", panel: sub, mouseInput: false })
        } else apply(this.#runtime, { kind: "set-panel-state", panel: control, mouseInput: false })
      }
      for (const name of ["Offense", "Defense", "Support", "CountLabel", "ClassMenuSelect", "TFPlayerModel"]) {
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
        apply(this.#runtime, { kind: "set-dialog-variable", panel: this.#owner, name: variable, value: "" })
      }
    })
    request.root.addEventListener("pointermove", this.#pointerMove)
    request.root.addEventListener("wheel", this.#tipsWheel, { passive: false, capture: true })
    request.root.addEventListener("pointerdown", this.#tipsPointerDown, true)
    request.root.addEventListener("pointerup", this.#tipsPointerUp, true)
    request.root.addEventListener("pointercancel", this.#tipsPointerUp, true)
  }

  readonly #pointerMove = (event: Event): void => {
    if (!this.#state.visible) return
    const pointer = event as PointerEvent
    if (this.#tipDrag && pointer.pointerId === this.#tipDrag.pointer) {
      const drag = this.#tipDrag
      this.#scrollTips(drag.scroll + Math.trunc((pointer.clientY - drag.start) * drag.range / Math.max(1, drag.pixels)))
      event.preventDefault()
      return
    }
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
    this.#tipRows.length = 0
    this.#tipScroll = this.#tipContentHeight = this.#tipItemsHeight = 0
    for (const control of this.#tipChrome.values()) apply(this.#runtime, { kind: "set-panel-state", panel: control, visible: false })
    if (identity === 12) return
    const count = Number(this.#localization.get(`classtips_${identity}_count`) ?? "0")
    const source = this.#resources.document(CLASS_TIPS_ITEM_PATH)
    const authored = source.root.children.find((value) => value.name === "ClassTipsItemPanel")
    if (!authored || !Number.isSafeInteger(count) || count < 0 || count > 32) throw new Error("TF2 class selection authored tip source is invalid")
    const snapshot = this.#runtime.snapshot()
    const owner = snapshot.panels.find((value) => value.id === this.#owner)!
    const list = snapshot.panels.find((value) => value.id === this.#tips)!
    const scale = owner.bounds.height / 480
    const buffer = Math.trunc(5 * scale)
    const scrollbar = this.#resources.clientScheme.settings.find((value) => value.name.toLowerCase() === "scrollbar.wide")
    const scrollbarWidth = Math.trunc(Number(scrollbar?.value ?? 17) * scale)
    this.#tipScrollbarWidth = scrollbarWidth
    const itemWidth = list.bounds.width - buffer - scrollbarWidth - 12
    let position = 0
    for (let index = 1; index <= count; index += 1) {
      if (this.#localization.has(`classtips_${identity}_${index}_mvm`)) continue
      const text = this.#localization.get(`classtips_${identity}_${index}`)
      if (!text) continue
      const name = `ClassTipsItemPanel${index}`
      const item = apply(this.#runtime, {
        kind: "create-panel", parent: this.#tips, control: "CTFClassTipsItemPanel", name,
        properties: authored.children.filter((value) => value.value !== null && !["controlname", "fieldname"].includes(value.name.toLowerCase()))
          .map((value) => ({ name: value.name, value: value.value! })),
      })!
      this.#tipPanels.push(item)
      applyChildren(this.#runtime, item, source, authored.children, this.#resources.activeConditions, this.#resources.resolutionSuffixes)
      const height = this.#runtime.snapshot().panels.find((value) => value.id === item)!.bounds.height
      position += buffer
      apply(this.#runtime, { kind: "set-bounds", panel: item, bounds: { x: buffer, y: position, width: itemWidth, height } })
      this.#tipRows.push({ panel: item, x: buffer, y: position, width: itemWidth, height })
      this.#tipItemsHeight += height
      const label = panel(this.#runtime, "TipLabel", item)
      const icon = panel(this.#runtime, "TipIcon", item)
      if (label === null || icon === null) throw new Error("TF2 class selection authored tip controls are unavailable")
      apply(this.#runtime, { kind: "mutate-control", panel: label, mutation: { text } })
      const image = this.#localization.get(`classtips_${identity}_${index}_icon`)
      if (image) apply(this.#runtime, { kind: "mutate-control", panel: icon, mutation: { image } })
      else apply(this.#runtime, { kind: "set-panel-state", panel: icon, visible: false })
      position += height
    }
    this.#tipContentHeight = position + buffer
    this.#scrollTips(0)
  }

  #scrollTips(value: number): void {
    const list = this.#runtime.snapshotPanels([this.#tips])[0]!
    const range = Math.max(0, this.#tipContentHeight - list.bounds.height)
    const scroll = Math.max(0, Math.min(range, Math.trunc(value)))
    const width = this.#tipScrollbarWidth
    const track = Math.max(0, list.bounds.height - width * 2 - 1)
    const size = Math.min(track, Math.max(width - 1, track * list.bounds.height / Math.max(1, this.#tipContentHeight)))
    const start = range > 0 ? Math.trunc((track - size) * scroll / range) : 0
    const visible = this.#tipItemsHeight > list.bounds.height
    this.#runtime.deferPresentation(() => {
      for (const row of this.#tipRows) apply(this.#runtime, { kind: "set-bounds", panel: row.panel, bounds: { x: row.x, y: row.y - scroll, width: row.width, height: row.height } })
      for (const [name, y, height] of [["UpArrow", 0, width], ["DownArrow", list.bounds.height - width, width],
        ["Line", width, list.bounds.height - width * 2], ["Box", width + start, Math.trunc(start + size) - start]] as const) {
        const control = this.#tipChrome.get(name)!
        apply(this.#runtime, { kind: "set-bounds", panel: control, bounds: { x: list.bounds.width - width, y, width, height } })
        apply(this.#runtime, { kind: "set-panel-state", panel: control, visible, enabled: visible,
          alpha: name === "UpArrow" && scroll === 0 || name === "DownArrow" && scroll === range ? 90 : 255 })
        const image = this.#tipChrome.get(`${name}Image`)
        if (image) {
          apply(this.#runtime, { kind: "set-bounds", panel: image, bounds: { x: 0, y: 0, width, height } })
          apply(this.#runtime, { kind: "set-panel-state", panel: image, visible })
        }
      }
    })
    this.#tipScroll = scroll
  }

  readonly #tipsWheel = (event: WheelEvent): void => {
    if (!this.#state.visible || this.#tipContentHeight === 0) return
    const list = this.#runtime.snapshotPanels([this.#tips])[0]!
    const root = this.#root.getBoundingClientRect()
    const x = event.clientX - root.left, y = event.clientY - root.top
    const b = list.absoluteBounds
    if (x < b.x || x >= b.x + b.width || y < b.y || y >= b.y + b.height) return
    this.#scrollTips(this.#tipScroll + Math.sign(event.deltaY) * 24)
    event.preventDefault()
    event.stopImmediatePropagation()
  }

  readonly #tipsPointerDown = (event: PointerEvent): void => {
    if (!this.#state.visible || event.button !== 0) return
    const list = this.#runtime.snapshotPanels([this.#tips])[0]!
    if (this.#tipItemsHeight <= list.bounds.height) return
    const root = this.#root.getBoundingClientRect()
    const b = list.absoluteBounds, width = this.#tipScrollbarWidth
    const x = event.clientX - root.left - b.x, y = event.clientY - root.top - b.y
    if (x < b.width - width || x >= b.width || y < width || y >= b.height - width) return
    const knob = this.#runtime.snapshotPanels([this.#tipChrome.get("Box")!])[0]!.bounds
    if (y < knob.y || y >= knob.y + knob.height) this.#scrollTips(this.#tipScroll + (y < knob.y ? -1 : 1) * list.bounds.height)
    else {
      this.#tipDrag = { pointer: event.pointerId, start: event.clientY, scroll: this.#tipScroll,
        pixels: b.height - 2 * width - 1 - knob.height, range: this.#tipContentHeight - b.height }
      this.#root.setPointerCapture(event.pointerId)
    }
    event.preventDefault()
    event.stopImmediatePropagation()
  }

  readonly #tipsPointerUp = (event: PointerEvent): void => {
    if (this.#tipDrag?.pointer !== event.pointerId) return
    this.#tipDrag = undefined
    if (this.#root.hasPointerCapture(event.pointerId)) this.#root.releasePointerCapture(event.pointerId)
  }

  #updateRoster(): void {
    if (!this.#state.visible || this.#state.team === null) return
    const counts = new Array<number>(10).fill(0)
    for (const player of this.#roster()) {
      if (player.team === this.#state.team && player.class >= 1 && player.class <= 9
        && !(!player.fake && this.#state.initialJoin)) counts[player.class]! += 1
    }
    const key = `${this.#state.team}:${this.#state.current}:${counts.join(",")}`
    if (key === this.#rosterKey) return
    this.#rosterKey = key
    this.#runtime.deferPresentation(() => {
      const others: Tf2ClassSelectionIdentity[] = []
      for (const playerClass of TF2_CLASS_SELECTION_CLASSES.filter((value) => value.identity !== 12)) {
        const count = counts[playerClass.identity]!
        const variable = playerClass.identity === 6 ? "numHeavy" : `num${playerClass.name[0]!.toUpperCase()}${playerClass.name.slice(1)}`
        apply(this.#runtime, { kind: "set-dialog-variable", panel: this.#owner, name: variable, value: count > 0 ? String(count) : "" })
        for (let i = 0; i < count - Number(playerClass.identity === this.#state.current) && others.length < 11; i += 1) others.push(playerClass.identity)
      }
      for (let i = 0; i < 11; i += 1) {
        const image = panel(this.#runtime, `countImage${i}`, this.#owner)
        if (image === null) continue
        apply(this.#runtime, { kind: "set-panel-state", panel: image, visible: i < others.length, mouseInput: false })
        if (i < others.length) apply(this.#runtime, { kind: "mutate-control", panel: image, mutation: { image: tf2ClassSelectionImage(others[i]!, this.#state.team!, true) } })
      }
      const label = panel(this.#runtime, "CountLabel", this.#owner)
      if (label !== null) apply(this.#runtime, { kind: "set-panel-state", panel: label, visible: others.length > 0 })
      for (const name of ["localPlayerImage", "localPlayerBG"]) {
        const image = panel(this.#runtime, name, this.#owner)
        if (image === null) continue
        apply(this.#runtime, { kind: "set-panel-state", panel: image, visible: this.#state.current !== null, mouseInput: false })
        if (name === "localPlayerImage" && this.#state.current !== null) apply(this.#runtime, { kind: "mutate-control", panel: image,
          mutation: { image: tf2ClassSelectionImage(this.#state.current, this.#state.team!, true) } })
      }
    })
  }

  #present(): void {
    if (!this.#state.visible && this.#tipDrag) {
      if (this.#root.hasPointerCapture(this.#tipDrag.pointer)) this.#root.releasePointerCapture(this.#tipDrag.pointer)
      this.#tipDrag = undefined
    }
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
      const select = panel(this.#runtime, "ClassMenuSelect", this.#owner)
      if (select !== null) apply(this.#runtime, { kind: "set-panel-state", panel: select, visible: this.#state.initialJoin })
      const loadout = panel(this.#runtime, "EditLoadoutButton", this.#owner)
      if (loadout !== null) apply(this.#runtime, { kind: "set-panel-state", panel: loadout, visible: this.#state.selected !== 12 })
      this.#updateTips(this.#state.selected)
      this.#updateRoster()
    })
    this.#onModelPanels(this.modelPanels())
    if (this.#state.visible && this.#modelSurface && !this.#releaseSurface) {
      const background = panel(this.#runtime, "MenuBG", this.#owner)
      const player = panel(this.#runtime, "TFPlayerModel", this.#owner)
      if (background === null || player === null || !this.#backgroundSurface) throw new Error("Class-selection model surface owner is unavailable")
      const width = this.#modelSurface.style.width, height = this.#modelSurface.style.height, backgroundColor = this.#modelSurface.style.backgroundColor
      this.#modelSurface.style.backgroundColor = "transparent"
      this.#modelSurface.style.width = `${this.#viewport.width}px`
      this.#modelSurface.style.height = `${this.#viewport.height}px`
      const releaseBackground = this.#runtime.attachSurface(background, this.#backgroundSurface)
      const releasePlayer = this.#runtime.attachSurface(player, this.#modelSurface)
      this.#releaseSurface = () => {
        releasePlayer()
        releaseBackground()
        this.#modelSurface!.style.width = width
        this.#modelSurface!.style.height = height
        this.#modelSurface!.style.backgroundColor = backgroundColor
        this.#backgroundSurface!.width = 0
        this.#backgroundSurface!.height = 0
      }
    } else if (!this.#state.visible) {
      this.#releaseSurface?.()
      this.#releaseSurface = undefined
    }
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
        bounds: snapshot.absoluteBounds,
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
    if (this.#state.visible) {
      this.#updateRoster()
      apply(this.#runtime, { kind: "frame", timeSeconds })
    }
  }

  setViewport(viewport: VguiViewport): void {
    this.#viewport = viewport
    if (this.#releaseSurface && this.#modelSurface) {
      this.#modelSurface.style.width = `${viewport.width}px`
      this.#modelSurface.style.height = `${viewport.height}px`
    }
    this.#runtime.deferPresentation(() => apply(this.#runtime, { kind: "set-viewport", viewport }))
    if (this.#state.visible) this.#present()
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    if (this.#tipDrag && this.#root.hasPointerCapture(this.#tipDrag.pointer)) this.#root.releasePointerCapture(this.#tipDrag.pointer)
    this.#tipDrag = undefined
    this.#releaseSurface?.()
    this.#releaseSurface = undefined
    this.#root.removeEventListener("pointermove", this.#pointerMove)
    this.#root.removeEventListener("wheel", this.#tipsWheel, true)
    this.#root.removeEventListener("pointerdown", this.#tipsPointerDown, true)
    this.#root.removeEventListener("pointerup", this.#tipsPointerUp, true)
    this.#root.removeEventListener("pointercancel", this.#tipsPointerUp, true)
    this.#onModelPanels(Object.freeze([]))
    apply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2ClassSelectionIntegration(request: Tf2ClassSelectionIntegrationRequest): Tf2ClassSelectionIntegration {
  return Object.freeze(new Integration(request))
}
