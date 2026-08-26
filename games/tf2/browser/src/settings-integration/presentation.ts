import { TF2_SELECTED_OPTIONS, type BindingCapture, type BindingSettingSchema, type SettingSchema, type SettingValue } from "@playsrc/settings"
import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiRequest,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiViewport,
} from "@playsrc/vgui"
import { tf2CrosshairSettings } from "../hud"
import { paintTf2AuthoredCrosshair } from "../hud-integration/crosshair"
import { tf2AuthoredCrosshairs, type Tf2UiAdvancedOption, type Tf2UiKeyboardAction } from "../ui-resources"
import type { Tf2VguiResources } from "../ui-integration"
import type { Tf2BrowserSettings, Tf2BrowserSettingsSnapshot } from "./state"

export type Tf2OptionsPage = "keyboard" | "mouse" | "audio" | "video" | "multiplayer" | "advanced"
export type Tf2OptionsPresentation = Readonly<{
  show(page?: Tf2OptionsPage): void
  hide(disposition: "cancel" | "applied"): void
  handleKey(event: KeyboardEvent): boolean
  set(settingId: string, value: SettingValue): void
  capture(settingId: string, capture: BindingCapture): Readonly<{ displacedSettingId?: string }>
  unbind(settingId: string): void
  defaults(page?: Tf2OptionsPage): void
  apply(): Promise<Tf2BrowserSettingsSnapshot>
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  snapshot(): Readonly<{
    visible: boolean
    page: Tf2OptionsPage
    bindingCapture: boolean
    selectedBinding: string | null
    settings: Tf2BrowserSettingsSnapshot
    vgui: VguiRuntimeSnapshot
  }>
  destroy(): void
}>

export type Tf2OptionsPresentationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  settings: Tf2BrowserSettings
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onPersistence(bytes: Uint8Array): void
  onApply(snapshot: Tf2BrowserSettingsSnapshot): void
  onVisibility(visible: boolean): void
}>

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 2_048, maxHierarchyDepth: 64, maxChildrenPerPanel: 512, maxResourceNodes: 4_096,
  maxResourceDepth: 64, maxPropertiesPerPanel: 256, maxStringCodeUnits: 4_095, maxTextCodeUnits: 65_535,
  maxDialogVariables: 512, maxLocalizationTokens: 4_096, maxSchemeColors: 1_024, maxSchemeSettings: 2_048,
  maxSchemeBorders: 512, maxSchemeImages: 2_048, maxAnimationScripts: 16, maxAnimationSequences: 1_024,
  maxAnimationCommands: 8_192, maxActiveAnimations: 2_048, maxDelayedCommands: 2_048,
  maxQueuedMessages: 4_096, maxDiagnostics: 4_096, maxDomNodes: 8_192, maxListeners: 64,
})
const PAGE_RESOURCE: Readonly<Record<Exclude<Tf2OptionsPage, "advanced">, string>> = Object.freeze({
  keyboard: "resource/optionssubkeyboard.res", mouse: "resource/optionssubmouse.res", audio: "resource/optionssubaudio.res",
  video: "resource/optionssubvideo.res", multiplayer: "resource/optionssubmultiplayer.res",
})
const PAGE_LABEL: Readonly<Record<Tf2OptionsPage, string>> = Object.freeze({
  keyboard: "#GameUI_Keyboard", mouse: "#GameUI_Mouse", audio: "#GameUI_Audio", video: "#GameUI_Video", multiplayer: "#GameUI_Multiplayer", advanced: "Advanced",
})
const CONTROL_SETTINGS: Readonly<Record<string, string>> = Object.freeze({
  ReverseMouse: "mouse.reverse", MouseFilter: "mouse.filter", MouseRaw: "mouse.raw-input",
  MouseAccelerationCheckbox: "mouse.custom-acceleration", Joystick: "mouse.joystick",
  JoystickSouthpaw: "mouse.joystick-southpaw", ReverseJoystick: "mouse.reverse-joystick",
  HudQuickInfo: "mouse.hud-quick-info", Slider: "mouse.sensitivity",
  MouseAccelerationSlider: "mouse.acceleration-exponent", JoystickYawSlider: "mouse.joystick-yaw-sensitivity",
  JoystickPitchSlider: "mouse.joystick-pitch-sensitivity", SFXSlider: "audio.effect-volume",
  MusicSlider: "audio.music-volume", CloseCaptionCheck: "audio.caption-mode", SoundQuality: "audio.quality",
  SpeakerSetup: "audio.speaker-layout", AudioSpokenLanguage: "audio.spoken-language",
  snd_mute_losefocus: "audio.mute-while-unfocused", HDContentButton: "video.hd-content",
  VRMode: "video.vr-enabled", DisplayModeCombo: "video.windowed", Red_Color_Slider: "multiplayer.crosshair-red",
  Green_Color_Slider: "multiplayer.crosshair-green", Blue_Color_Slider: "multiplayer.crosshair-blue",
  Scale_Slider: "multiplayer.crosshair-scale", AdvCrosshairList: "multiplayer.crosshair-file",
  DownloadFilterCheck: "multiplayer.download-filter",
})

type ControlBinding = Readonly<{
  schema: SettingSchema
  scale: number
  choices?: readonly SettingValue[]
  choiceLabels?: readonly string[]
  input: "toggle" | "slider" | "text" | "combo"
  toggleValues?: readonly [off: SettingValue, on: SettingValue]
}>
type StandardDialogIdentity = "keyboard-advanced" | "video-advanced" | "video-gamma" | "audio-credits" | "video-credits"
type StandardDialog = Readonly<{ panel: VguiPanelId; appliesSettings: boolean }>
const scalar = (node: VguiResourceNode, name: string): string | null => node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const rootNode = (name: string, children: readonly VguiResourceNode[]): VguiResourceNode => Object.freeze({ name, value: null, condition: null, children: Object.freeze(children) })
const shallow = (block: VguiResourceNode): VguiResourceNode => Object.freeze({ ...block, children: Object.freeze(block.children.filter((child) => child.value !== null || child.name.toLowerCase() === "controlname")) })
const derivedDocument = (source: VguiResourceDocument, suffix: string, root: VguiResourceNode): VguiResourceDocument => Object.freeze({ logicalIdentity: `${source.logicalIdentity}/${suffix}`.toLowerCase(), revision: source.revision, root })

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}
function panel(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((value) => value.name.toLowerCase() === name.toLowerCase() && (parent === undefined || value.parent === parent))?.id ?? null
}

function panelElement(root: HTMLElement, name: string): HTMLElement | null {
  const pending: Element[] = [root]
  while (pending.length > 0) {
    const current = pending.pop()!
    if ((current as HTMLElement).dataset?.vguiName === name) return current as HTMLElement
    for (const child of Array.from(current.children)) pending.push(child)
  }
  return null
}
function applyPage(runtime: VguiRuntime, page: VguiPanelId, source: VguiResourceDocument, activeConditions: readonly string[], resolutionSuffixes: readonly string[]): void {
  const blocks = source.root.children.filter((block) => block.value === null && scalar(block, "ControlName") !== null)
  apply(runtime, { kind: "replace-resource", parent: page, document: derivedDocument(source, `page-${page}`, rootNode(source.root.name, blocks.map(shallow))), selection: { activeConditions, resolutionSuffixes } })
}
function applyOwned(runtime: VguiRuntime, owner: VguiPanelId, source: VguiResourceDocument, activeConditions: readonly string[], resolutionSuffixes: readonly string[]): void {
  const state = runtime.snapshot().panels.find((value) => value.id === owner)!
  const self = source.root.children.find((block) => (scalar(block, "fieldName") ?? block.name).toLowerCase() === state.name.toLowerCase())
  if (self && state.parent !== null) apply(runtime, { kind: "replace-resource", parent: state.parent, document: derivedDocument(source, `self-${owner}`, rootNode(source.root.name, [shallow(self)])), selection: { activeConditions, resolutionSuffixes } })
  const children = source.root.children.filter((block) => block !== self && block.value === null && scalar(block, "ControlName") !== null)
  if (children.length) apply(runtime, { kind: "replace-resource", parent: owner, document: derivedDocument(source, `children-${owner}`, rootNode(source.root.name, children.map(shallow))), selection: { activeConditions, resolutionSuffixes } })
}
const schemaById = (identity: string): SettingSchema => {
  const schema = TF2_SELECTED_OPTIONS.settings.find((value) => value.id === identity)
  if (!schema) throw new Error(`TF2 setting ${identity} is missing`)
  return schema
}
const schemaForCvar = (name: string): SettingSchema | null => TF2_SELECTED_OPTIONS.settings.find((schema) => schema.consoleNames.some((candidate) => candidate.toLowerCase() === name.toLowerCase())) ?? null
const normalizedControlName = (name: string): string => name.replaceAll(" ", "_")
const controlInput = (control: string): ControlBinding["input"] => {
  const folded = control.toLowerCase()
  if (folded.includes("slider")) return "slider"
  if (folded.includes("combo")) return "combo"
  if (folded.includes("check") || folded.includes("radio")) return "toggle"
  return "text"
}

class Presentation implements Tf2OptionsPresentation {
  readonly #runtime: VguiRuntime
  #advancedRuntime!: VguiRuntime
  readonly #configuration: Tf2OptionsPresentationRequest
  readonly #settings: Tf2BrowserSettings
  readonly #resources: Tf2VguiResources
  readonly #onPersistence: (bytes: Uint8Array) => void
  readonly #onApply: (snapshot: Tf2BrowserSettingsSnapshot) => void
  readonly #onVisibility: (visible: boolean) => void
  readonly #standardMount: HTMLElement
  readonly #advancedMount: HTMLElement
  readonly #pages = new Map<Tf2OptionsPage, VguiPanelId>()
  readonly #controls = new Map<VguiPanelId, ControlBinding>()
  readonly #advancedControls = new Map<VguiPanelId, ControlBinding>()
  readonly #publishedControls = new Map<VguiRuntime, Map<VguiPanelId, string>>()
  readonly #keyboardRows = new Map<number, BindingSettingSchema>()
  readonly #dialogs = new Map<StandardDialogIdentity, StandardDialog>()
  #crosshairPreview?: HTMLElement
  #crosshairPreviewPanel?: VguiPanelId
  #crosshairPreviewStyle = ""
  #crosshairPreviewFrame = 0
  #crosshairPreviewAscending = true
  #crosshairPreviewAnimated = false
  #nextCrosshairPreviewFrame = 0
  #visible = false
  #page: Tf2OptionsPage = "keyboard"
  #frame!: VguiPanelId
  #sheet!: VguiPanelId
  #advanced?: VguiPanelId
  #selectedBinding: BindingSettingSchema | null = null
  #bindingCapture = false
  #activeDialog: StandardDialogIdentity | null = null
  #destroyed = false

  constructor(request: Tf2OptionsPresentationRequest) {
    this.#configuration = request
    this.#settings = request.settings
    this.#resources = request.resources
    this.#onPersistence = request.onPersistence
    this.#onApply = request.onApply
    this.#onVisibility = request.onVisibility
    this.#standardMount = request.root.ownerDocument.createElement("div")
    this.#advancedMount = request.root.ownerDocument.createElement("div")
    for (const [mount, identity] of [[this.#standardMount, "standard"], [this.#advancedMount, "advanced"]] as const) {
      mount.dataset.tf2OptionsMount = identity
      mount.style.position = "absolute"
      mount.style.inset = "0"
      mount.style.width = "100%"
      mount.style.height = "100%"
      mount.style.display = "none"
      request.root.append(mount)
    }
    const configured = (runtimeIdentity: string, scheme: Tf2VguiResources["sourceScheme"], advanced: boolean) => initializeVguiRuntime({
      runtimeIdentity, root: advanced ? this.#advancedMount : this.#standardMount, rootControl: { control: "EditablePanel", name: advanced ? "AdvancedOptionsViewport" : "OptionsViewport" },
      viewport: request.viewport, limits: LIMITS, clock: request.clock, random: request.random, scheme,
      localization: request.resources.localization, animationScripts: request.resources.animations,
      customControls: request.resources.customControls, reducedMotion: request.reducedMotion,
      onRequest: (value) => this.#request(value, advanced),
    })
    const standard = configured("tf2-options", request.resources.sourceScheme, false)
    if (!standard.ok) {
      this.#standardMount.remove()
      this.#advancedMount.remove()
      throw new Error(`${standard.diagnostic.code}:${standard.diagnostic.subject}`)
    }
    this.#runtime = standard.runtime
    this.#runtime.deferPresentation(() => {
    const x = Math.max(0, Math.trunc((request.viewport.width - 512) / 2))
    const y = Math.max(0, Math.trunc((request.viewport.height - 406) / 2))
    this.#frame = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "Frame", name: "OptionsDialog", properties: [
      { name: "title", value: "#GameUI_Options" }, { name: "moveable", value: "1" }, { name: "sizeable", value: "0" },
    ] })!
    apply(this.#runtime, { kind: "set-bounds", panel: this.#frame, bounds: { x, y, width: 512, height: 406 } })
    this.#sheet = apply(this.#runtime, { kind: "create-panel", parent: this.#frame, control: "PropertySheet", name: "Sheet", properties: [{ name: "tabwidth", value: "84" }] })!
    apply(this.#runtime, { kind: "set-bounds", panel: this.#sheet, bounds: { x: 8, y: 31, width: 496, height: 339 } })
    for (const [index, pageName] of (["keyboard", "mouse", "audio", "video", "multiplayer"] as const).entries()) {
      const identity = apply(this.#runtime, { kind: "create-panel", parent: this.#sheet, control: "PropertyPage", name: `${PAGE_LABEL[pageName]}Page` })!
      apply(this.#runtime, { kind: "set-bounds", panel: identity, bounds: { x: 0, y: 28, width: 496, height: 311 } })
      this.#pages.set(pageName, identity)
      const source = this.#resources.document(PAGE_RESOURCE[pageName])
      applyPage(this.#runtime, identity, source, this.#resources.activeConditions, this.#resources.resolutionSuffixes)
      this.#bindControls(source)
      if (index !== 0) apply(this.#runtime, { kind: "set-panel-state", panel: identity, visible: false })
    }
    apply(this.#runtime, { kind: "mutate-control", panel: this.#sheet, mutation: { items: [...this.#pages].map(([pageName, id]) => ({ id, text: PAGE_LABEL[pageName], enabled: true })) } })
    for (const [name, text, command, offset] of [["OKButton", "#PropertyDialog_OK", "OK", 240], ["CancelButton", "#PropertyDialog_Cancel", "Cancel", 320], ["ApplyButton", "#PropertyDialog_Apply", "Apply", 400]] as const) {
      const id = apply(this.#runtime, { kind: "create-panel", parent: this.#frame, control: "Button", name, properties: [{ name: "labelText", value: text }, { name: "command", value: command }, { name: "actionsignallevel", value: "1" }] })!
      apply(this.#runtime, { kind: "set-bounds", panel: id, bounds: { x: offset, y: 374, width: 72, height: 24 } })
    }
    this.#configureKeyboard()
    this.#configureStandardDialogs()

    apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: false })
    this.#publishValues()
    })
    const preview = panel(this.#runtime, "AdvCrosshairImage")
    const previewHost = panelElement(this.#standardMount, "AdvCrosshairImage")
    if (preview === null || !previewHost) throw new Error("Configured TF2 Multiplayer crosshair preview is unavailable")
    this.#crosshairPreviewPanel = preview
    this.#crosshairPreview = request.root.ownerDocument.createElement("div")
    this.#crosshairPreview.dataset.tf2Crosshair = "preview"
    this.#crosshairPreview.style.position = "absolute"
    this.#crosshairPreview.style.pointerEvents = "none"
    previewHost.append(this.#crosshairPreview)
    this.#publishCrosshairPreview()
  }

  #bindControls(source: VguiResourceDocument): void {
    for (const block of source.root.children) {
      const cvar = scalar(block, "cvar_name")
      const field = scalar(block, "fieldName") ?? block.name
      const mapped = CONTROL_SETTINGS[normalizedControlName(field)] ?? CONTROL_SETTINGS[field]
      const schema = cvar ? schemaForCvar(cvar) : mapped ? schemaById(mapped) : null
      const identity = panel(this.#runtime, field)
      if (!schema || identity === null) continue
      const scale = schema.kind === "float" || schema.kind === "integer" ? 100 : 1
      const controlName = scalar(block, "ControlName") ?? ""
      const input = controlInput(controlName)
      const choices = input === "combo" && schema.id === "multiplayer.crosshair-file"
        ? Object.freeze(["", ...tf2AuthoredCrosshairs.styles.map((style) => style.file)])
        : input === "combo" && schema.kind === "enum"
          ? Object.freeze(schema.options.map((option) => option.value))
          : input === "combo" && schema.kind === "boolean"
            ? Object.freeze([false, true])
            : undefined
      const choiceLabels = schema.id === "multiplayer.crosshair-file"
        ? Object.freeze(["None", ...tf2AuthoredCrosshairs.styles.map((style) => style.file)])
        : choices
          ? schema.kind === "enum"
            ? Object.freeze(schema.options.map((option) => option.label))
            : Object.freeze(["#gameui_disabled", "#gameui_enabled"])
          : undefined
      this.#controls.set(identity, Object.freeze({
        schema,
        scale,
        input,
        choices,
        choiceLabels,
        ...(input === "toggle" && schema.kind === "enum" ? { toggleValues: Object.freeze([schema.options[0]!.value, schema.options[Math.min(1, schema.options.length - 1)]!.value]) as readonly [SettingValue, SettingValue] } : {}),
      }))
      if (input === "combo") apply(this.#runtime, { kind: "mutate-control", panel: identity, mutation: { editable: false } })
      if (schema.kind === "float" || schema.kind === "integer") {
        const minimum = Number(scalar(block, "minvalue") ?? schema.minimum)
        const maximum = Number(scalar(block, "maxvalue") ?? schema.maximum)
        apply(this.#runtime, { kind: "mutate-control", panel: identity, mutation: { minimum: minimum * scale, maximum: maximum * scale } })
      }
    }
  }

  #configureKeyboard(): void {
    const list = panel(this.#runtime, "listpanel_keybindlist")
    if (list === null) throw new Error("Configured keyboard list is missing")
    const grouped = new Map<number, Tf2UiKeyboardAction[]>()
    for (const row of this.#resources.descriptor.keyboardActions) grouped.set(row.section, [...(grouped.get(row.section) ?? []), row])
    const sections = [...grouped].map(([id, rows]) => ({
      id,
      name: rows[0]!.sectionName,
      alwaysVisible: false,
      minimumHeight: 0,
      columns: [
        { name: "Action", text: rows[0]!.sectionName, flags: 0x04, width: 286 },
        { name: "Key", text: "#GameUI_KeyButton", flags: 0x04, width: 128 },
      ],
    }))
    let id = 1
    const items = this.#resources.descriptor.keyboardActions.map((row) => {
      const schema = TF2_SELECTED_OPTIONS.settings.find((value): value is BindingSettingSchema => value.kind === "binding" && value.action.toLowerCase() === row.binding.toLowerCase())
      if (!schema) throw new Error(`Configured keyboard action ${row.binding} has no setting`)
      this.#keyboardRows.set(id, schema)
      return { id: id++, section: row.section, cells: { Action: row.description, Key: "" }, enabled: true }
    })
    apply(this.#runtime, { kind: "mutate-control", panel: list, mutation: { sections, sectionedItems: items } })
  }

  #binding(schemaId: string, input: ControlBinding["input"], choices?: readonly SettingValue[], choiceLabels?: readonly string[]): ControlBinding {
    const schema = schemaById(schemaId)
    return Object.freeze({
      schema,
      scale: input === "slider" ? 100 : 1,
      input,
      choices,
      choiceLabels,
    })
  }

  #bindNamed(parent: VguiPanelId, name: string, binding: ControlBinding): void {
    const identity = panel(this.#runtime, name, parent)
    if (identity === null) throw new Error(`Configured Options control ${name} is missing`)
    this.#controls.set(identity, binding)
    if ((binding.schema.kind === "float" || binding.schema.kind === "integer") && binding.input === "slider") {
      apply(this.#runtime, { kind: "mutate-control", panel: identity, mutation: { minimum: binding.schema.minimum * binding.scale, maximum: binding.schema.maximum * binding.scale } })
    }
  }

  #createDialog(
    identity: StandardDialogIdentity,
    logicalPath: string,
    name: string,
    control: string,
    title: string,
    width: number,
    height: number,
    appliesSettings: boolean,
  ): VguiPanelId {
    const created = apply(this.#runtime, { kind: "create-panel", parent: 1, control, name, properties: [
      { name: "title", value: title }, { name: "sizeable", value: "0" }, { name: "moveable", value: "1" },
    ] })!
    applyOwned(this.#runtime, created, this.#resources.document(logicalPath), this.#resources.activeConditions, this.#resources.resolutionSuffixes)
    const viewport = this.#runtime.snapshot().viewport
    apply(this.#runtime, { kind: "set-bounds", panel: created, bounds: {
      x: Math.max(0, Math.trunc((viewport.width - width) / 2)),
      y: Math.max(0, Math.trunc((viewport.height - height) / 2)),
      width,
      height,
    } })
    apply(this.#runtime, { kind: "set-panel-state", panel: created, visible: false, popup: true })
    this.#dialogs.set(identity, Object.freeze({ panel: created, appliesSettings }))
    return created
  }

  #configureStandardDialogs(): void {
    for (const [name, command] of [["GammaButton", "OpenGammaDialog"], ["AdvancedButton", "OpenAdvanced"], ["ThirdPartyVideoCredits", "OpenThirdPartyVideoCreditsDialog"]] as const) {
      const identity = panel(this.#runtime, name)
      if (identity !== null) apply(this.#runtime, { kind: "mutate-control", panel: identity, mutation: { command } })
    }
    const speakerTest = panel(this.#runtime, "TestSpeakers")
    if (speakerTest !== null) apply(this.#runtime, { kind: "set-panel-state", panel: speakerTest, enabled: false })

    const keyboard = this.#createDialog("keyboard-advanced", "resource/optionssubkeyboardadvanceddlg.res", "OptionsSubKeyboardAdvancedDlg", "Frame", "#GameUI_KeyboardAdvanced_Title", 280, 140, true)
    this.#bindNamed(keyboard, "ConsoleCheck", this.#binding("keyboard.console-enabled", "toggle"))
    this.#bindNamed(keyboard, "FastSwitchCheck", this.#binding("hud_fastswitch", "toggle"))

    const video = this.#createDialog("video-advanced", "resource/optionssubvideoadvanceddlg.res", "OptionsSubVideoAdvancedDlg", "COptionsSubVideoAdvancedDlg", "#GameUI_VideoAdvanced_Title", 660, 430, true)
    for (const [name, schemaId, labels] of [
      ["ModelDetail", "video.model-detail", ["#gameui_low", "#gameui_medium", "#gameui_high"]],
      ["TextureDetail", "video.texture-detail", ["#gameui_low", "#gameui_medium", "#gameui_high", "#gameui_ultra"]],
      ["FilteringMode", "video.filtering", ["#GameUI_Bilinear", "#GameUI_Trilinear", "#GameUI_Anisotropic2X", "#GameUI_Anisotropic4X", "#GameUI_Anisotropic8X", "#GameUI_Anisotropic16X"]],
      ["ShadowDetail", "video.shadow-detail", ["#gameui_low", "#gameui_medium", "#gameui_high"]],
      ["ShaderDetail", "video.shader-detail", ["#gameui_low", "#gameui_high"]],
      ["HDR", "video.hdr", ["#GameUI_hdr_level0", "#GameUI_hdr_level1", "#GameUI_hdr_level2"]],
      ["WaterDetail", "video.water-detail", ["#gameui_noreflections", "#gameui_reflectonlyworld", "#gameui_reflectall"]],
    ] as const) {
      const schema = schemaById(schemaId)
      if (schema.kind !== "enum") throw new Error(`Configured Video control ${name} is not an enum setting`)
      this.#bindNamed(video, name, this.#binding(schemaId, "combo", Object.freeze(schema.options.map((option) => option.value)), labels))
    }
    for (const [name, schemaId] of [["VSync", "video.vsync"], ["Multicore", "video.multicore"], ["ColorCorrection", "video.color-correction"], ["MotionBlur", "video.motion-blur"]] as const) {
      this.#bindNamed(video, name, this.#binding(schemaId, "combo", Object.freeze([false, true]), Object.freeze(["#gameui_disabled", "#gameui_enabled"])))
    }
    this.#bindNamed(video, "FovSlider", this.#binding("video.field-of-view", "slider"))
    for (const name of ["dxlabel", "AntialiasingMode", "Bloom"]) {
      const identity = panel(this.#runtime, name, video)
      if (identity !== null) apply(this.#runtime, { kind: "set-panel-state", panel: identity, enabled: false })
    }

    const gamma = this.#createDialog("video-gamma", "resource/optionssubvideogammadlg.res", "OptionsSubVideoGammaDlg", "CGammaDialog", "#GameUI_AdjustGamma_Title", 400, 260, true)
    this.#bindNamed(gamma, "Gamma", this.#binding("video.gamma", "slider"))
    const gammaOk = panel(this.#runtime, "OKButton", gamma)
    if (gammaOk !== null) apply(this.#runtime, { kind: "mutate-control", panel: gammaOk, mutation: { command: "GammaOK" } })

    this.#createDialog("audio-credits", "resource/optionssubaudiothirdpartydlg.res", "OptionsSubAudioThirdPartyDlg", "Frame", "#GameUI_ThirdPartyAudio_Title", 500, 200, false)
    this.#createDialog("video-credits", "resource/optionssubvideothirdpartydlg.res", "OptionsSubVideoThirdPartyDlg", "Frame", "#GameUI_ThirdPartyVideo_Title", 500, 200, false)
  }

  #openDialog(identity: StandardDialogIdentity): void {
    if (this.#activeDialog !== null) this.#closeDialog(false)
    const dialog = this.#dialogs.get(identity)!
    this.#activeDialog = identity
    apply(this.#runtime, { kind: "set-panel-state", panel: dialog.panel, visible: true })
    apply(this.#runtime, { kind: "set-application-modal", panel: dialog.panel })
    apply(this.#runtime, { kind: "request-focus", panel: dialog.panel })
    this.#publishValues()
  }

  #closeDialog(applied: boolean): void {
    if (this.#activeDialog === null) return
    const dialog = this.#dialogs.get(this.#activeDialog)!
    apply(this.#runtime, { kind: "set-application-modal", panel: null })
    apply(this.#runtime, { kind: "set-panel-state", panel: dialog.panel, visible: false })
    this.#activeDialog = null
    if (!applied) this.#publishValues()
  }

  #ensureAdvanced(): VguiRuntime {
    if (this.#advancedRuntime) return this.#advancedRuntime
    const request = this.#configuration
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-advanced-options", root: this.#advancedMount, rootControl: { control: "EditablePanel", name: "AdvancedOptionsViewport" },
      viewport: request.viewport, limits: LIMITS, clock: request.clock, random: request.random, scheme: request.resources.clientScheme,
      localization: request.resources.localization, animationScripts: request.resources.animations, customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion, onRequest: (value) => this.#request(value, true),
    })
    if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#advancedRuntime = initialized.runtime
    this.#advancedRuntime.deferPresentation(() => {
      this.#advanced = apply(this.#advancedRuntime!, { kind: "create-panel", parent: 1, control: "CTFAdvancedOptionsDialog", name: "TFAdvancedOptionsDialog" })!
      apply(this.#advancedRuntime!, { kind: "set-panel-state", panel: this.#advanced!, proportional: true })
      const source = this.#resources.document("resource/ui/tfadvancedoptionsdialog.res")
      apply(this.#advancedRuntime!, { kind: "create-panel", parent: this.#advanced!, control: "CPanelListPanel", name: "PanelListPanel" })
      applyOwned(this.#advancedRuntime!, this.#advanced!, source, this.#resources.activeConditions, this.#resources.resolutionSuffixes)
      this.#configureAdvanced()
      apply(this.#advancedRuntime!, { kind: "set-panel-state", panel: 1, visible: false })
      this.#publishControls(this.#advancedRuntime!, this.#advancedControls)
    })
    return this.#advancedRuntime
  }

  #configureAdvanced(): void {
    const list = panel(this.#advancedRuntime, "PanelListPanel", this.#advanced)
    if (list === null) throw new Error("Configured Advanced Options panel list is missing")
    const content = apply(this.#advancedRuntime, { kind: "create-panel", parent: list, control: "EditablePanel", name: "AdvancedRows" })!
    const rootState = this.#advancedRuntime.snapshot().panels.find((value) => value.id === this.#advanced)!
    const controlWidth = Number(rootState.animationVariables.control_w || 500)
    const controlHeight = Number(rootState.animationVariables.control_h || 25)
    const sliderHeight = Number(rootState.animationVariables.slider_h || 25)
    let y = 0
    let category = ""
    for (const [index, row] of this.#resources.descriptor.advancedOptions.entries()) {
      if (row.category !== category) {
        category = row.category
        const header = apply(this.#advancedRuntime, { kind: "create-panel", parent: content, control: "Label", name: `AdvancedCategory${index}`, properties: [
          { name: "labelText", value: category }, { name: "font", value: "HudFontSmallBold" },
          { name: "border", value: "OptionsCategoryBorder" }, { name: "textAlignment", value: "west" },
          { name: "textinsetx", value: "5" }, { name: "fgcolor_override", value: "TanLight" },
        ] })!
        apply(this.#advancedRuntime, { kind: "set-bounds", panel: header, bounds: { x: 0, y, width: controlWidth, height: controlHeight } })
        y += controlHeight
      }
      const schema = schemaById(row.identity)
      const height = row.kind === "SLIDER" ? sliderHeight : controlHeight
      const rowPanel = apply(this.#advancedRuntime, { kind: "create-panel", parent: content, control: "Panel", name: `AdvancedRow${index}` })!
      apply(this.#advancedRuntime, { kind: "set-bounds", panel: rowPanel, bounds: { x: 0, y, width: controlWidth, height } })
      const control = this.#advancedControl(rowPanel, row, schema, height)
      let choices: readonly SettingValue[] | undefined
      if (row.kind === "LIST") {
        if (schema.kind !== "enum") throw new Error(`Configured Advanced Options list ${row.identity} is not an enum setting`)
        choices = Object.freeze(row.choices.map((choice) => {
          const option = schema.options.find((candidate) => String(candidate.value) === choice.value)
          if (!option) throw new Error(`Configured Advanced Options choice ${row.identity}:${choice.value} is undeclared`)
          return option.value
        }))
      }
      this.#advancedControls.set(control, Object.freeze({
        schema,
        scale: row.kind === "SLIDER" ? 100 : 1,
        choices,
        choiceLabels: row.kind === "LIST" ? Object.freeze(row.choices.map((choice) => choice.label)) : undefined,
        input: row.kind === "BOOL" ? "toggle" : row.kind === "LIST" ? "combo" : row.kind === "SLIDER" ? "slider" : "text",
      }))
      y += height
    }
    apply(this.#advancedRuntime, { kind: "set-bounds", panel: content, bounds: { x: 0, y: 0, width: controlWidth, height: y } })
  }

  #advancedControl(parent: VguiPanelId, row: Tf2UiAdvancedOption, schema: SettingSchema, height: number): VguiPanelId {
    const width = this.#advancedRuntime.snapshot().panels.find((value) => value.id === parent)!.bounds.width
    if (row.kind === "BOOL") {
      const control = apply(this.#advancedRuntime, { kind: "create-panel", parent, control: "CheckButton", name: "DescCheckButton", properties: [
        { name: "labelText", value: row.prompt }, { name: "font", value: "HudFontSmallestBold" }, { name: "textAlignment", value: "west" },
        { name: "defaultFgColor_override", value: "TanDark" }, { name: "armedFgColor_override", value: "TanDark" },
        { name: "depressedFgColor_override", value: "TanDark" }, { name: "selectedFgColor_override", value: "TanDark" },
        ...(row.tooltip ? [{ name: "tooltiptext", value: row.tooltip }] : []),
      ] })!
      apply(this.#advancedRuntime, { kind: "set-bounds", panel: control, bounds: { x: 0, y: 4, width, height: height - 8 } })
      return control
    }
    const prompt = apply(this.#advancedRuntime, { kind: "create-panel", parent, control: "Label", name: "DescLabel", properties: [
      { name: "labelText", value: row.prompt }, { name: "font", value: "HudFontSmallestBold" },
      { name: "textAlignment", value: "west" }, { name: "textinsetx", value: "5" }, { name: "fgcolor_override", value: "TanDark" },
      ...(row.tooltip ? [{ name: "tooltiptext", value: row.tooltip }] : []),
    ] })!
    apply(this.#advancedRuntime, { kind: "set-bounds", panel: prompt, bounds: { x: 0, y: 4, width: width / 2 + 20, height: height - 8 } })
    const x = width / 2 + 20
    const controlWidth = width / 2 - 20
    if (row.kind === "LIST") {
      const control = apply(this.#advancedRuntime, { kind: "create-panel", parent, control: "ComboBox", name: "DescComboBox", properties: [{ name: "editable", value: "0" }, { name: "numLines", value: "5" }] })!
      apply(this.#advancedRuntime, { kind: "set-bounds", panel: control, bounds: { x, y: 4, width: controlWidth, height: height - 8 } })
      apply(this.#advancedRuntime, { kind: "mutate-control", panel: control, mutation: { items: row.choices.map((choice, index) => ({ id: index, text: choice.label, enabled: true })) } })
      return control
    }
    if (row.kind === "SLIDER") {
      const control = apply(this.#advancedRuntime, { kind: "create-panel", parent, control: "CCvarSlider", name: "DescSlider" })!
      apply(this.#advancedRuntime, { kind: "set-bounds", panel: control, bounds: { x, y: 4, width: controlWidth, height: height - 8 } })
      apply(this.#advancedRuntime, { kind: "mutate-control", panel: control, mutation: { minimum: row.minimum! * 100, maximum: row.maximum! * 100 } })
      return control
    }
    const control = apply(this.#advancedRuntime, { kind: "create-panel", parent, control: "TextEntry", name: "DescTextEntry", properties: [{ name: "font", value: "HudFontSmallestBold" }, { name: "bgcolor_override", value: "0 0 0 255" }] })!
    apply(this.#advancedRuntime, { kind: "set-bounds", panel: control, bounds: { x, y: 4, width: controlWidth, height: height - 8 } })
    return control
  }

  #bindingText(schema: BindingSettingSchema, values: Readonly<Record<string, SettingValue>>): string {
    const value = values[schema.id]
    if (!value || typeof value !== "object") return ""
    const modifiers = [value.modifiers & 1 ? "Shift" : "", value.modifiers & 2 ? "Ctrl" : "", value.modifiers & 4 ? "Alt" : ""].filter(Boolean)
    return [...modifiers, value.code].join("+")
  }

  #publishControls(runtime: VguiRuntime, bindings: ReadonlyMap<VguiPanelId, ControlBinding>): void {
    runtime.deferPresentation(() => {
      const values = this.#settings.snapshot().settings.pending ?? this.#settings.snapshot().settings.current
      const published = this.#publishedControls.get(runtime) ?? new Map<VguiPanelId, string>()
      this.#publishedControls.set(runtime, published)
      for (const [id, binding] of bindings) {
        const { schema, scale, choices, choiceLabels, input, toggleValues } = binding
        const value = values[schema.id]
        if (choices) {
          const index = choices.findIndex((choice) => choice === value)
          apply(runtime, { kind: "mutate-control", panel: id, mutation: { items: choices.map((_, item) => ({ id: item, text: choiceLabels?.[item] ?? String(choices[item]), enabled: true })), activeIndex: index < 0 ? null : index } })
          published.set(id, `combo:${index < 0 ? "null" : index}`)
        }
        else if (schema.kind === "boolean" && typeof value === "boolean") { apply(runtime, { kind: "mutate-control", panel: id, mutation: { checked: value, selected: value } }); published.set(id, `toggle:${value}`) }
        else if (toggleValues) { const checked = value === toggleValues[1]; apply(runtime, { kind: "mutate-control", panel: id, mutation: { checked, selected: checked } }); published.set(id, `toggle:${checked}`) }
        else if ((schema.kind === "float" || schema.kind === "integer") && typeof value === "number") { apply(runtime, { kind: "mutate-control", panel: id, mutation: input === "slider" ? { value: value * scale, text: String(value) } : { text: String(value) } }); published.set(id, input === "slider" ? `slider:${Math.max(schema.minimum * scale, Math.min(schema.maximum * scale, value * scale))}` : `text:${String(value)}`) }
        else if (schema.kind === "string" && typeof value === "string") { apply(runtime, { kind: "mutate-control", panel: id, mutation: { text: value } }); published.set(id, `text:${value}`) }
        else if (schema.kind === "enum") {
          const options = schema.options.map((option) => option.value)
          const index = options.findIndex((option) => String(option) === String(value))
          apply(runtime, { kind: "mutate-control", panel: id, mutation: { items: schema.options.map((option, item) => ({ id: item, text: option.label, enabled: true })), activeIndex: index < 0 ? null : index } })
          published.set(id, `combo:${index < 0 ? "null" : index}`)
        }
      }
    })
  }

  #publishKeyboard(): void {
    const list = panel(this.#runtime, "listpanel_keybindlist")
    if (list === null) return
    const snapshot = this.#runtime.snapshot().panels.find((value) => value.id === list)!
    const values = this.#settings.snapshot().settings.pending ?? this.#settings.snapshot().settings.current
    this.#runtime.deferPresentation(() => apply(this.#runtime, { kind: "mutate-control", panel: list, mutation: { sectionedItems: snapshot.state.sectionedItems.map((item) => ({ ...item, cells: { ...item.cells, Key: this.#bindingText(this.#keyboardRows.get(item.id)!, values) } })) } }))
  }

  #publishCrosshairPreview(timeSeconds?: number): void {
    if (!this.#crosshairPreview || this.#crosshairPreviewPanel === undefined) return
    const values = this.#settings.snapshot().settings.pending ?? this.#settings.snapshot().settings.current
    const settings = tf2CrosshairSettings(values)
    const style = tf2AuthoredCrosshairs.styles.find((candidate) => candidate.file === settings.file)
    if (!style) {
      if (this.#crosshairPreviewStyle === "" && this.#crosshairPreview.style.display === "none") return
      this.#crosshairPreview.style.display = "none"
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#crosshairPreviewPanel, visible: false })
      this.#crosshairPreviewStyle = ""
      this.#crosshairPreviewAnimated = false
      return
    }
    const changed = this.#crosshairPreviewStyle !== style.file
    if (!changed && timeSeconds !== undefined
      && (style.frames.length <= 1 || timeSeconds < this.#nextCrosshairPreviewFrame)) return
    if (changed) {
      this.#crosshairPreviewStyle = style.file
      this.#crosshairPreviewFrame = 0
      this.#crosshairPreviewAscending = true
      this.#crosshairPreviewAnimated = style.frames.length > 1
      this.#nextCrosshairPreviewFrame = (timeSeconds ?? this.#configuration.clock.nowSeconds()) + 0.2
    } else if (timeSeconds !== undefined && style.frames.length > 1 && timeSeconds >= this.#nextCrosshairPreviewFrame) {
      this.#nextCrosshairPreviewFrame = timeSeconds + 0.2
      let frame = this.#crosshairPreviewFrame + (this.#crosshairPreviewAscending ? 1 : -1)
      if (frame >= style.frames.length) {
        this.#crosshairPreviewAscending = false
        frame -= 1
      } else if (frame < 0) {
        this.#crosshairPreviewAscending = true
        frame += 1
      }
      this.#crosshairPreviewFrame = frame
    }
    const state = this.#runtime.snapshot().panels.find((candidate) => candidate.id === this.#crosshairPreviewPanel)
    if (!state) throw new Error("Configured TF2 Multiplayer crosshair preview disappeared")
    const width = settings.scale / 48 * state.bounds.width
    const half = Math.trunc(width / 2)
    this.#crosshairPreview.style.left = `${Math.trunc(state.bounds.width / 2) - half}px`
    this.#crosshairPreview.style.top = `${Math.trunc(state.bounds.height / 2) - half}px`
    this.#crosshairPreview.style.width = `${Math.trunc(width)}px`
    this.#crosshairPreview.style.height = `${Math.trunc(width)}px`
    this.#crosshairPreview.dataset.crosshairStyle = style.file
    paintTf2AuthoredCrosshair(
      this.#crosshairPreview,
      style,
      Object.freeze([settings.red, settings.green, settings.blue, 255]),
      this.#crosshairPreviewFrame,
    )
    this.#crosshairPreview.style.display = "block"
    apply(this.#runtime, { kind: "set-panel-state", panel: this.#crosshairPreviewPanel, visible: true })
  }

  #publishValues(): void {
    this.#publishControls(this.#runtime, this.#controls)
    if (this.#advancedRuntime) this.#publishControls(this.#advancedRuntime, this.#advancedControls)
    this.#publishKeyboard()
    this.#publishCrosshairPreview()
  }

  #stageControls(runtime: VguiRuntime, bindings: ReadonlyMap<VguiPanelId, ControlBinding>): void {
    const snapshots = new Map(runtime.snapshot().panels.map((value) => [value.id, value]))
    const pending = this.#settings.snapshot().settings.pending ?? this.#settings.snapshot().settings.current
    const published = this.#publishedControls.get(runtime)
    for (const [id, binding] of bindings) {
      const { schema, scale, choices, input, toggleValues } = binding
      const control = snapshots.get(id)
      if (!control) continue
      const fingerprint = choices ? `combo:${control.state.activeIndex === null ? "null" : control.state.activeIndex}`
        : toggleValues || schema.kind === "boolean" ? `toggle:${control.state.checked}`
          : (schema.kind === "float" || schema.kind === "integer") && input === "slider" ? `slider:${control.state.value}`
            : `text:${control.text}`
      if (published?.get(id) === fingerprint) continue
      let value: SettingValue | undefined
      if (choices && control.state.activeIndex !== null) value = choices[control.state.activeIndex]
      else if (toggleValues) value = toggleValues[control.state.checked ? 1 : 0]
      else if (schema.kind === "boolean") value = control.state.checked
      else if (schema.kind === "float") value = input === "slider" ? control.state.value / scale : Number(control.text.trim())
      else if (schema.kind === "integer") value = input === "slider" ? Math.trunc(control.state.value / scale) : Number(control.text.trim())
      else if (schema.kind === "string") value = control.text
      else if (schema.kind === "enum" && control.state.activeIndex !== null) value = schema.options[control.state.activeIndex]?.value
      if (schema.kind === "enum" && value !== undefined && !schema.options.some((option) => option.value === value)) {
        throw new Error(`TF2 setting ${schema.id} produced undeclared enum value ${String(value)}`)
      }
      if (value !== undefined && pending[schema.id] !== value) this.#settings.set(schema.id, value)
    }
  }

  #request(request: VguiRequest, advanced: boolean): void {
    if (request.kind === "message" && request.source !== null) {
      const list = panel(this.#runtime, "listpanel_keybindlist")
      if (!advanced && list === request.source && request.message.name === "ItemSelected") {
        const identity = Number(request.message.fields.itemID)
        this.#selectedBinding = this.#keyboardRows.get(identity) ?? null
      }
      const control = !advanced ? this.#controls.get(request.source) : undefined
      if (control?.schema.id.startsWith("multiplayer.crosshair-")
        && ["SliderMoved", "TextChanged", "ItemSelected"].includes(request.message.name)) {
        this.#stageControls(this.#runtime, new Map([[request.source, control]]))
        this.#publishCrosshairPreview()
      }
      return
    }
    if (request.kind !== "command") return
    if (request.command === "OpenAdvanced") this.#openDialog("video-advanced")
    else if (request.command === "OpenGammaDialog") this.#openDialog("video-gamma")
    else if (request.command === "OpenThirdPartyVideoCreditsDialog") this.#openDialog("video-credits")
    else if (request.command === "ShowThirdPartyAudioCredits") this.#openDialog("audio-credits")
    else if (request.command === "Advanced") this.#openDialog("keyboard-advanced")
    else if ((request.command === "OK" || request.command === "Ok" || request.command === "GammaOK") && this.#activeDialog !== null) {
      const dialog = this.#dialogs.get(this.#activeDialog)!
      if (!dialog.appliesSettings) this.#closeDialog(true)
      else void this.#apply(false).then((result) => { if (result.lastApply?.complete) this.#closeDialog(true) })
    }
    else if ((request.command === "Cancel" || request.command === "Close") && this.#activeDialog !== null) this.#closeDialog(false)
    else if (request.command === "Cancel" || request.command === "Close") this.hide("cancel")
    else if (request.command === "Apply") void this.#apply(advanced)
    else if (request.command === "OK" || request.command === "Ok") void this.#apply(advanced).then((result) => { if (result.lastApply?.complete) this.hide("applied") })
    else if (request.command === "Defaults") { this.defaults(this.#page); this.#publishValues() }
    else if (request.command === "ChangeKey") this.#bindingCapture = this.#selectedBinding !== null
    else if (request.command === "ClearKey" && this.#selectedBinding) { this.#settings.unbind(this.#selectedBinding.id); this.#publishKeyboard() }
  }

  show(page: Tf2OptionsPage = "keyboard"): void {
    if (this.#destroyed) throw new Error("TF2 Options presentation is destroyed")
    this.#settings.begin()
    this.#visible = true
    this.#onVisibility(true)
    this.#page = page
    const advanced = page === "advanced"
    const advancedRuntime = advanced ? this.#ensureAdvanced() : this.#advancedRuntime
    this.#standardMount.style.display = advanced ? "none" : "block"
    this.#advancedMount.style.display = advanced ? "block" : "none"
    apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: !advanced })
    if (advancedRuntime) apply(advancedRuntime, { kind: "set-panel-state", panel: 1, visible: advanced })
    apply(this.#runtime, { kind: "set-panel-state", panel: this.#frame, visible: !advanced })
    if (advancedRuntime && this.#advanced !== undefined) apply(advancedRuntime, { kind: "set-panel-state", panel: this.#advanced, visible: advanced })
    if (!advanced) {
      const pageId = this.#pages.get(page)!
      apply(this.#runtime, { kind: "mutate-control", panel: this.#sheet, mutation: { activeIndex: [...this.#pages.keys()].indexOf(page) } })
      for (const id of this.#pages.values()) apply(this.#runtime, { kind: "set-panel-state", panel: id, visible: id === pageId })
    }
    this.#publishValues()
  }

  hide(disposition: "cancel" | "applied"): void {
    if (this.#activeDialog !== null) this.#closeDialog(false)
    if (disposition === "cancel") this.#settings.cancel()
    this.#visible = false
    this.#bindingCapture = false
    this.#onVisibility(false)
    this.#standardMount.style.display = "none"
    this.#advancedMount.style.display = "none"
    apply(this.#runtime, { kind: "set-panel-state", panel: 1, visible: false })
    if (this.#advancedRuntime) apply(this.#advancedRuntime, { kind: "set-panel-state", panel: 1, visible: false })
  }

  handleKey(event: KeyboardEvent): boolean {
    if (this.#visible && this.#activeDialog !== null && event.code === "Escape") {
      event.preventDefault()
      this.#closeDialog(false)
      return true
    }
    if (!this.#visible || !this.#bindingCapture || !this.#selectedBinding) return false
    event.preventDefault()
    if (event.code === "Escape") { this.#bindingCapture = false; return true }
    const code = event.code.startsWith("Key") ? event.code.slice(3).toLowerCase()
      : event.code.startsWith("Digit") ? event.code.slice(5)
        : event.code === "Backquote" ? "`" : event.code === "Space" ? "SPACE"
          : event.code === "Tab" ? "TAB" : event.code.startsWith("Arrow") ? `${event.code.slice(5).toUpperCase()}ARROW`
            : event.code.toUpperCase()
    const result = this.#settings.capture(this.#selectedBinding.id, { code, shift: event.shiftKey, control: event.ctrlKey, alt: event.altKey })
    this.#bindingCapture = false
    this.#publishKeyboard()
    return !!result
  }

  set(settingId: string, value: SettingValue): void { this.#settings.set(settingId, value); this.#publishValues() }
  capture(settingId: string, capture: BindingCapture): Readonly<{ displacedSettingId?: string }> { const result = this.#settings.capture(settingId, capture); this.#publishKeyboard(); return result }
  unbind(settingId: string): void { this.#settings.unbind(settingId); this.#publishKeyboard() }
  defaults(page?: Tf2OptionsPage): void {
    const ids = page ? TF2_SELECTED_OPTIONS.settings.filter((schema) => schema.page === page).map((schema) => schema.id) : undefined
    this.#settings.defaults(ids)
    this.#publishValues()
  }
  async #apply(advanced: boolean): Promise<Tf2BrowserSettingsSnapshot> {
    if (advanced) this.#stageControls(this.#ensureAdvanced(), this.#advancedControls)
    else this.#stageControls(this.#runtime, this.#controls)
    const result = await this.#settings.apply()
    this.#onApply(result)
    if (result.lastApply?.complete) this.#onPersistence(this.#settings.persistence())
    this.#publishValues()
    return result
  }
  async apply(): Promise<Tf2BrowserSettingsSnapshot> { return this.#apply(this.#page === "advanced") }
  frame(timeSeconds: number): void {
    if(!this.#visible)return
    apply(this.#page==="advanced"?this.#ensureAdvanced():this.#runtime, { kind: "frame", timeSeconds })
    if (this.#page === "multiplayer" && this.#crosshairPreviewAnimated && timeSeconds >= this.#nextCrosshairPreviewFrame) {
      this.#publishCrosshairPreview(timeSeconds)
    }
    if (this.#activeDialog !== null) {
      const dialog = this.#dialogs.get(this.#activeDialog)!
      if (!this.#runtime.snapshot().panels.find((value) => value.id === dialog.panel)?.visible) {
        this.#activeDialog = null
        this.#publishValues()
      }
    }
  }
  setViewport(viewport: VguiViewport): void {
    apply(this.#runtime, { kind: "set-viewport", viewport })
    if (this.#advancedRuntime) apply(this.#advancedRuntime, { kind: "set-viewport", viewport })
    this.#publishCrosshairPreview()
  }
  snapshot() { return Object.freeze({ visible: this.#visible, page: this.#page, bindingCapture: this.#bindingCapture, selectedBinding: this.#selectedBinding?.id ?? null, settings: this.#settings.snapshot(), vgui: this.#page === "advanced" ? this.#ensureAdvanced().snapshot() : this.#runtime.snapshot() }) }
  destroy(): void { if (!this.#destroyed) { this.#destroyed = true; apply(this.#runtime, { kind: "destroy" }); if (this.#advancedRuntime) apply(this.#advancedRuntime, { kind: "destroy" }); this.#standardMount.remove(); this.#advancedMount.remove() } }
}

export function initializeTf2OptionsPresentation(request: Tf2OptionsPresentationRequest): Tf2OptionsPresentation {
  return Object.freeze(new Presentation(request))
}
