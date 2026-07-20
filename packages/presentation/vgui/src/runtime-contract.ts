import type { Rgba } from "./contract"

export const VGUI_GENERIC_CONTROL_NAMES = Object.freeze([
  "Panel",
  "EditablePanel",
  "Label",
  "ImagePanel",
  "Button",
  "TextEntry",
  "RichText",
  "Frame",
  "ScrollBar",
  "ScrollBar_Vertical",
  "ScrollBar_Horizontal",
  "Slider",
  "ComboBox",
  "Menu",
  "MenuItem",
  "PropertySheet",
  "PropertyPage",
  "CheckButton",
  "RadioButton",
  "ProgressBar",
  "ContinuousProgressBar",
  "Divider",
  "FrameSystemButton",
  "HTML",
  "ScalableImagePanel",
  "ScrollableEditablePanel",
  "SectionedListPanel",
  "ListPanel",
  "MessageBox",
  "QueryBox",
  "URLLabel",
] as const)

export type VguiGenericControlName = typeof VGUI_GENERIC_CONTROL_NAMES[number]
export type VguiControlName = VguiGenericControlName | (string & {})
export type VguiPanelId = number

export type VguiRect = Readonly<{ x: number; y: number; width: number; height: number }>
export type VguiInsets = Readonly<{ left: number; top: number; right: number; bottom: number }>
export type VguiViewport = Readonly<{ width: number; height: number; devicePixelRatio: number }>

export type VguiRuntimeLimits = Readonly<{
  maxPanels: number
  maxHierarchyDepth: number
  maxChildrenPerPanel: number
  maxResourceNodes: number
  maxResourceDepth: number
  maxPropertiesPerPanel: number
  maxStringCodeUnits: number
  maxTextCodeUnits: number
  maxDialogVariables: number
  maxLocalizationTokens: number
  maxSchemeColors: number
  maxSchemeSettings: number
  maxSchemeBorders: number
  maxSchemeImages: number
  maxAnimationScripts: number
  maxAnimationSequences: number
  maxAnimationCommands: number
  maxActiveAnimations: number
  maxDelayedCommands: number
  maxQueuedMessages: number
  maxDiagnostics: number
  maxDomNodes: number
  maxListeners: number
}>

export type VguiResourceNode = Readonly<{
  name: string
  value: string | null
  condition: string | null
  children: readonly VguiResourceNode[]
}>

export type VguiResourceDocument = Readonly<{
  logicalIdentity: string
  revision: string
  root: VguiResourceNode
}>

export type VguiResourceSelection = Readonly<{
  activeConditions: readonly string[]
  resolutionSuffixes: readonly string[]
}>

export type VguiLocalizationToken = Readonly<{ name: string; value: string }>
export type VguiLocalization = Readonly<{
  identity: string
  revision: string
  language: string
  tokens: readonly VguiLocalizationToken[]
}>

export type VguiFontPresentation = Readonly<{
  name: string
  cssFamily: string
  sizePx: number
  lineHeightPx: number
  weight: number
  style: "normal" | "italic"
  available: boolean
  measure?: (text: string, wrapWidth: number | null) => Readonly<{ width: number; height: number }>
}>

export type VguiLineBorder = Readonly<{
  kind: "line"
  name: string
  inset: VguiInsets
  backgroundType: 0 | 1 | 2
  paintFirst: boolean
  sides: Readonly<{
    left: readonly Readonly<{ color: Rgba; startOffset: number; endOffset: number }>[]
    top: readonly Readonly<{ color: Rgba; startOffset: number; endOffset: number }>[]
    right: readonly Readonly<{ color: Rgba; startOffset: number; endOffset: number }>[]
    bottom: readonly Readonly<{ color: Rgba; startOffset: number; endOffset: number }>[]
  }>
}>

export type VguiImageBorder = Readonly<{
  kind: "image"
  name: string
  inset: VguiInsets
  backgroundType: 0 | 1 | 2
  paintFirst: boolean
  image: string
  tiled: boolean
}>

export type VguiScalableImageBorder = Readonly<{
  kind: "scalable-image"
  name: string
  inset: VguiInsets
  backgroundType: 0 | 1 | 2
  paintFirst: boolean
  image: string
  sourceCornerWidth: number
  sourceCornerHeight: number
  drawCornerWidth: number
  drawCornerHeight: number
  color: Rgba
}>

export type VguiBorder = VguiLineBorder | VguiImageBorder | VguiScalableImageBorder

export type VguiImagePresentation = Readonly<{
  name: string
  logicalIdentity: string
  revision: string
  browserUrl: string
  width: number
  height: number
  frames: number
  hardwareFiltered: boolean
  material?: VguiImageMaterialPresentation
  variants?: readonly Readonly<{ frame: number; tint: Rgba; rotation: 0 | 1 | 2 | 3; browserUrl: string }>[]
}>

export type VguiImageMaterialTexture = Readonly<{
  logicalIdentity: string
  revision: string
  browserUrl: string
  width: number
  height: number
  hardwareFiltered: boolean
  colorRead: "srgb" | "linear"
}>

export type VguiImageMaterialPresentation = Readonly<{
  shader: "unlit-generic" | "unlit-two-texture"
  base: VguiImageMaterialTexture
  second: VguiImageMaterialTexture | null
  detail: VguiImageMaterialTexture | null
  detailScale: number
  detailBlendMode: 0 | 8
  detailBlendFactor: number
  detailTint: readonly [number, number, number]
  distanceAlpha: boolean
  distanceAlphaFromDetail: boolean
  softEdges: boolean
  scaleSoftEdges: boolean
  edgeSoftnessStart: number
  edgeSoftnessEnd: number
  outline: boolean
  outlineColor: readonly [number, number, number]
  outlineAlpha: number
  outlineStart0: number
  outlineStart1: number
  outlineEnd0: number
  outlineEnd1: number
  scaleOutline: boolean
  glow: boolean
  glowColor: readonly [number, number, number]
  glowAlpha: number
  glowStart: number
  glowEnd: number
  glowX: number
  glowY: number
}>

export type VguiScheme = Readonly<{
  identity: string
  revision: string
  tag: string
  colors: readonly Readonly<{ name: string; value: string }>[]
  settings: readonly Readonly<{ name: string; value: string }>[]
  fonts: readonly VguiFontPresentation[]
  borders: readonly VguiBorder[]
  images: readonly VguiImagePresentation[]
}>

export type VguiAnimationConverter =
  | "float"
  | "int"
  | "Color"
  | "bool"
  | "char"
  | "string"
  | "HFont"
  | "vgui::HFont"
  | "proportional_float"
  | "proportional_int"
  | "proportional_xpos"
  | "proportional_ypos"
  | "proportional_width"
  | "proportional_height"
  | "textureid"

export type VguiAnimationVariable = Readonly<{
  name: string
  converter: VguiAnimationConverter
  defaultValue: string
  maximumCodeUnits?: number
}>

export type VguiControlRegistration = Readonly<{
  name: string
  baseControl: VguiGenericControlName
  element: keyof HTMLElementTagNameMap
  role: string | null
  focusable: boolean
  animationVariables: readonly VguiAnimationVariable[]
  acceptedProperties: readonly string[]
}>

export type VguiAnimationInterpolator =
  | "Linear"
  | "Accel"
  | "Deaccel"
  | "Spline"
  | "Pulse"
  | "Flicker"
  | "Bias"
  | "Gain"
  | "Bounce"

export type VguiRelativeAlignment =
  | "northwest" | "north" | "northeast"
  | "west" | "center" | "east"
  | "southwest" | "south" | "southeast"
  | "nw" | "n" | "ne" | "w" | "c" | "e" | "sw" | "s" | "se"

export type VguiAnimationCommand =
  | Readonly<{
      kind: "Animate"
      panel: string
      variable: string
      target: string
      interpolator: VguiAnimationInterpolator | string
      parameter: number
      delaySeconds: number
      durationSeconds: number
      relative?: Readonly<{ panel: string; alignment: VguiRelativeAlignment }>
      condition: string | null
    }>
  | Readonly<{ kind: "RunEvent"; sequence: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "RunEventChild"; child: string; sequence: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "StopEvent"; sequence: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "StopAnimation"; panel: string; variable: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "StopPanelAnimations"; panel: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "SetFont"; panel: string; variable: string; font: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "SetTexture"; panel: string; variable: string; texture: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "SetString"; panel: string; variable: string; value: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "FireCommand"; command: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "PlaySound"; sound: string; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "SetVisible"; panel: string; visible: boolean; delaySeconds: number; condition: string | null }>
  | Readonly<{ kind: "SetInputEnabled"; panel: string; enabled: boolean; delaySeconds: number; condition: string | null }>

export type VguiAnimationSequence = Readonly<{
  name: string
  condition: string | null
  commands: readonly VguiAnimationCommand[]
}>

export type VguiAnimationScript = Readonly<{
  logicalIdentity: string
  revision: string
  sequences: readonly VguiAnimationSequence[]
}>

export type VguiAnimationScriptSet = Readonly<{
  identity: string
  revision: string
  scripts: readonly VguiAnimationScript[]
  activeConditions: readonly string[]
}>

export type VguiMessageValue = string | number | boolean | null | VguiPanelId
export type VguiMessage = Readonly<{
  name: string
  fields: Readonly<Record<string, VguiMessageValue>>
}>

export type VguiRequest =
  | Readonly<{ kind: "command"; panel: VguiPanelId; command: string }>
  | Readonly<{ kind: "message"; target: VguiPanelId; source: VguiPanelId | null; message: VguiMessage }>
  | Readonly<{ kind: "sound"; panel: VguiPanelId; logicalIdentity: string }>
  | Readonly<{ kind: "external-open"; panel: VguiPanelId; url: string }>
  | Readonly<{ kind: "clipboard-read"; panel: VguiPanelId; requestId: number }>
  | Readonly<{ kind: "clipboard-write"; panel: VguiPanelId; text: string }>
  | Readonly<{ kind: "user-config"; panel: VguiPanelId; operation: "read" | "write"; identity: string }>
  | Readonly<{ kind: "cursor-lock"; panel: VguiPanelId; locked: boolean }>

export type VguiDiagnosticCode =
  | "Destroyed"
  | "ReentrantFrame"
  | "InvalidConfiguration"
  | "InvalidOperation"
  | "InvalidViewport"
  | "InvalidPanel"
  | "HierarchyCycle"
  | "PanelLimit"
  | "HierarchyLimit"
  | "ChildLimit"
  | "ResourceLimit"
  | "MalformedResource"
  | "UnknownControl"
  | "UnknownProperty"
  | "MissingReference"
  | "MalformedValue"
  | "TextLimit"
  | "RegistryLimit"
  | "MalformedScheme"
  | "MissingSchemeValue"
  | "MalformedLocalization"
  | "MissingLocalization"
  | "MalformedAnimation"
  | "MissingSequence"
  | "AnimationLimit"
  | "MessageLimit"
  | "DomLimit"
  | "ListenerLimit"
  | "DomFailure"
  | "RequestSinkFailure"

export type VguiDiagnostic = Readonly<{
  sequence: number
  code: VguiDiagnosticCode
  operation: VguiOperation["kind"] | "initialize" | "browser-event"
  subject: string
}>

export type VguiPointerButton = "left" | "right" | "middle" | "button4" | "button5"
export type VguiKey =
  | "Tab" | "Enter" | "Space" | "Escape"
  | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
  | "PageUp" | "PageDown" | "Home" | "End"
  | "Backspace" | "Delete"
  | string

export type VguiControlMutation = Readonly<{
  text?: string
  command?: string | null
  value?: number
  minimum?: number
  maximum?: number
  rangeWindow?: number
  selected?: boolean
  checked?: boolean
  activeIndex?: number | null
  items?: readonly Readonly<{ id: number; text: string; command?: string; enabled?: boolean; checked?: boolean }>[]
  progress?: number
  previousProgress?: number
  imageFill?: number
  drawColor?: Rgba
  foregroundColor?: Rgba
  scalarProperties?: Readonly<Record<string, number>>
  image?: string
  imageFrame?: number
  url?: string
  title?: string
  description?: string
  sections?: readonly VguiSectionedListSection[]
  sectionedItems?: readonly VguiSectionedListItem[]
}>

export type VguiSectionedListColumn = Readonly<{
  name: string
  text: string
  flags: number
  width: number
}>

export type VguiSectionedListSection = Readonly<{
  id: number
  name: string
  alwaysVisible: boolean
  minimumHeight: number
  columns: readonly VguiSectionedListColumn[]
}>

export type VguiSectionedListItem = Readonly<{
  id: number
  section: number
  cells: Readonly<Record<string, string>>
  enabled: boolean
}>

export type VguiOperation =
  | Readonly<{ kind: "create-panel"; parent: VguiPanelId; control: VguiControlName; name: string; properties?: readonly Readonly<{ name: string; value: string }>[] }>
  | Readonly<{ kind: "delete-panel"; panel: VguiPanelId; deferred: boolean }>
  | Readonly<{ kind: "reparent-panel"; panel: VguiPanelId; parent: VguiPanelId }>
  | Readonly<{ kind: "set-bounds"; panel: VguiPanelId; bounds: VguiRect }>
  | Readonly<{ kind: "set-minimum-size"; panel: VguiPanelId; width: number; height: number }>
  | Readonly<{ kind: "set-panel-state"; panel: VguiPanelId; visible?: boolean; enabled?: boolean; mouseInput?: boolean; keyboardInput?: boolean; proportional?: boolean; z?: number; popup?: boolean; topmostPopup?: boolean }>
  | Readonly<{ kind: "move-to-front"; panel: VguiPanelId }>
  | Readonly<{ kind: "move-to-back"; panel: VguiPanelId }>
  | Readonly<{ kind: "replace-resource"; parent: VguiPanelId; document: VguiResourceDocument; selection: VguiResourceSelection }>
  | Readonly<{ kind: "replace-scheme"; scheme: VguiScheme }>
  | Readonly<{ kind: "replace-localization"; localization: VguiLocalization }>
  | Readonly<{ kind: "replace-animation-scripts"; scripts: VguiAnimationScriptSet }>
  | Readonly<{ kind: "set-dialog-variable"; panel: VguiPanelId; name: string; value: string | number }>
  | Readonly<{ kind: "mutate-control"; panel: VguiPanelId; mutation: VguiControlMutation }>
  | Readonly<{ kind: "request-focus"; panel: VguiPanelId | null }>
  | Readonly<{ kind: "set-default-button"; group: VguiPanelId; panel: VguiPanelId | null }>
  | Readonly<{ kind: "set-pointer-capture"; panel: VguiPanelId | null; initiatingButton: VguiPointerButton | null; pointerId: number | null }>
  | Readonly<{ kind: "set-application-modal"; panel: VguiPanelId | null }>
  | Readonly<{ kind: "set-modal-subtree"; panel: VguiPanelId | null; restrictToSubtree: boolean; outsideClickListener: VguiPanelId | null }>
  | Readonly<{ kind: "pointer-move"; x: number; y: number; pointerId: number }>
  | Readonly<{ kind: "pointer-press"; button: VguiPointerButton; x: number; y: number; pointerId: number; clicks: 1 | 2 | 3 }>
  | Readonly<{ kind: "pointer-release"; button: VguiPointerButton; x: number; y: number; pointerId: number }>
  | Readonly<{ kind: "pointer-wheel"; delta: number; x: number; y: number }>
  | Readonly<{ kind: "key-press"; key: VguiKey; shift: boolean; control: boolean; alt: boolean; meta: boolean; repeat: boolean }>
  | Readonly<{ kind: "key-typed"; key: VguiKey; shift: boolean; control: boolean; alt: boolean; meta: boolean }>
  | Readonly<{ kind: "key-release"; key: VguiKey; shift: boolean; control: boolean; alt: boolean; meta: boolean }>
  | Readonly<{ kind: "text-input"; text: string }>
  | Readonly<{ kind: "composition-start" }>
  | Readonly<{ kind: "composition-update"; text: string; caret: number }>
  | Readonly<{ kind: "composition-end"; text: string }>
  | Readonly<{ kind: "clipboard-result"; requestId: number; result: "success" | "denied" | "unavailable" | "failed"; text?: string }>
  | Readonly<{ kind: "post-message"; target: VguiPanelId; source: VguiPanelId | null; message: VguiMessage; delaySeconds: number }>
  | Readonly<{ kind: "start-animation-sequence"; parent: VguiPanelId; sequence: string; cancelable: boolean }>
  | Readonly<{ kind: "stop-animation-sequence"; parent: VguiPanelId; sequence: string }>
  | Readonly<{ kind: "set-viewport"; viewport: VguiViewport }>
  | Readonly<{ kind: "set-reduced-motion"; reduced: boolean }>
  | Readonly<{ kind: "frame"; timeSeconds: number }>
  | Readonly<{ kind: "destroy" }>

export type VguiOperationResult =
  | Readonly<{ ok: true; revision: number; panel?: VguiPanelId }>
  | Readonly<{ ok: false; diagnostic: VguiDiagnostic }>

export type VguiPanelSnapshot = Readonly<{
  id: VguiPanelId
  control: VguiControlName
  name: string
  parent: VguiPanelId | null
  children: readonly VguiPanelId[]
  resourceOwner: string | null
  bounds: VguiRect
  absoluteBounds: VguiRect
  clip: VguiRect
  inset: VguiInsets
  minimumSize: readonly [number, number]
  z: number
  popup: boolean
  topmostPopup: boolean
  visible: boolean
  effectivelyVisible: boolean
  enabled: boolean
  mouseInput: boolean
  keyboardInput: boolean
  proportional: boolean
  tabPosition: number
  text: string
  accessibleName: string
  accessibleDescription: string
  role: string | null
  state: Readonly<{
    armed: boolean
    depressed: boolean
    selected: boolean
    checked: boolean
    value: number
    minimum: number
    maximum: number
    rangeWindow: number
    progress: number
    previousProgress: number
    image: string | null
    imageFill: number
    drawColor: Rgba
    foregroundColor: Rgba | null
    scalarProperties: Readonly<Record<string, number>>
    activeIndex: number | null
    caret: number
    selection: readonly [start: number, end: number]
    editable: boolean
    multiline: boolean
    numericOnly: boolean
    textHidden: boolean
    maximumCharacters: number
    frameInteraction: "move" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw" | null
    bodyText: string
    items: readonly Readonly<{ id: number; text: string; command: string | null; enabled: boolean; checked: boolean }>[]
    sections: readonly VguiSectionedListSection[]
    sectionedItems: readonly VguiSectionedListItem[]
    composition: Readonly<{ active: boolean; text: string; caret: number }>
  }>
  animationVariables: Readonly<Record<string, string | number | boolean | Rgba>>
}>

export type VguiRuntimeSnapshot = Readonly<{
  runtimeIdentity: string
  lifecycle: "live" | "destroyed"
  revision: number
  frame: number
  timeSeconds: number
  viewport: VguiViewport
  reducedMotion: boolean
  rootPanel: VguiPanelId
  panels: readonly VguiPanelSnapshot[]
  popups: readonly VguiPanelId[]
  input: Readonly<{
    pointer: readonly [number, number]
    mouseOver: VguiPanelId | null
    mouseFocus: VguiPanelId | null
    capture: VguiPanelId | null
    captureButton: VguiPointerButton | null
    keyFocus: VguiPanelId | null
    calculatedKeyFocus: VguiPanelId | null
    applicationModal: VguiPanelId | null
    modalSubtree: VguiPanelId | null
    modalRestriction: "inclusive" | "exclusive" | null
    pressedKeys: readonly string[]
    downKeys: readonly string[]
    releasedKeys: readonly string[]
    pressedButtons: readonly VguiPointerButton[]
    downButtons: readonly VguiPointerButton[]
    releasedButtons: readonly VguiPointerButton[]
  }>
  schemeIdentity: string
  localizationIdentity: string
  animationScriptIdentity: string
  activeAnimations: number
  delayedCommands: number
  queuedMessages: number
  diagnostics: readonly VguiDiagnostic[]
  trace: readonly Readonly<{ sequence: number; phase: string; panel: VguiPanelId | null; detail: string }>[]
  ownedResources: Readonly<{ nodes: number; listeners: number; observers: 0; timers: 0 }>
}>

export type VguiRuntime = Readonly<{
  apply(operation: VguiOperation): VguiOperationResult
  deferPresentation<T>(callback: () => T): T
  snapshotPanels(panels: readonly VguiPanelId[]): readonly VguiPanelSnapshot[]
  snapshot(): VguiRuntimeSnapshot
}>

export type VguiRuntimeConfiguration = Readonly<{
  runtimeIdentity: string
  root: HTMLElement
  rootControl: Readonly<{ control: "Panel" | "EditablePanel"; name: string }>
  viewport: VguiViewport
  limits: VguiRuntimeLimits
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  scheme: VguiScheme
  localization: VguiLocalization
  animationScripts: VguiAnimationScriptSet
  customControls: readonly VguiControlRegistration[]
  reducedMotion: boolean
  onRequest(request: VguiRequest): void
}>

export type VguiRuntimeInitialization =
  | Readonly<{ ok: true; runtime: VguiRuntime }>
  | Readonly<{ ok: false; diagnostic: VguiDiagnostic }>
