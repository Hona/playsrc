import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiRequest,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiScheme,
} from "../src"

declare global {
  interface Window {
    vguiRuntimeEvidence: Readonly<{
      snapshot(): unknown
      requests(): readonly VguiRequest[]
      status(): Readonly<{ ready: boolean; error: string | null }>
      showQuery(): void
      cycles(count: number): unknown
    }>
  }
}

const limits: VguiRuntimeLimits = Object.freeze({
  maxPanels: 128, maxHierarchyDepth: 16, maxChildrenPerPanel: 64, maxResourceNodes: 512, maxResourceDepth: 16,
  maxPropertiesPerPanel: 128, maxStringCodeUnits: 2048, maxTextCodeUnits: 4096, maxDialogVariables: 64,
  maxLocalizationTokens: 128, maxSchemeColors: 128, maxSchemeSettings: 128, maxSchemeBorders: 32, maxSchemeImages: 32,
  maxAnimationScripts: 8, maxAnimationSequences: 128, maxAnimationCommands: 512, maxActiveAnimations: 128,
  maxDelayedCommands: 128, maxQueuedMessages: 256, maxDiagnostics: 512, maxDomNodes: 256, maxListeners: 32,
})

const scheme: VguiScheme = Object.freeze({
  identity: "resource/browser-scheme.res",
  revision: "browser-scheme-1",
  tag: "BrowserEvidence",
  colors: Object.freeze([
    { name: "Panel.FgColor", value: "235 235 235 255" }, { name: "Panel.BgColor", value: "38 42 46 255" },
    { name: "Button.TextColor", value: "245 245 245 255" }, { name: "Button.BgColor", value: "64 71 77 255" },
    { name: "Button.ArmedTextColor", value: "255 255 255 255" }, { name: "Button.ArmedBgColor", value: "85 95 103 255" },
    { name: "Button.DepressedTextColor", value: "20 20 20 255" }, { name: "Button.DepressedBgColor", value: "232 190 93 255" },
    { name: "Label.DisabledFgColor1", value: "160 150 140 255" }, { name: "Label.DisabledFgColor2", value: "35 38 40 255" },
    { name: "Frame.BgColor", value: "48 56 64 255" }, { name: "Frame.OutOfFocusBgColor", value: "30 35 40 255" },
    { name: "FrameTitleBar.BgColor", value: "55 70 82 255" }, { name: "FrameTitleBar.DisabledBgColor", value: "39 47 54 255" },
    { name: "FrameTitleBar.TextColor", value: "245 245 245 255" }, { name: "FrameTitleBar.DisabledTextColor", value: "190 196 200 255" },
    { name: "TextEntry.TextColor", value: "240 240 240 255" }, { name: "TextEntry.BgColor", value: "15 18 20 255" },
    { name: "Slider.NobColor", value: "232 190 93 255" }, { name: "Slider.TrackColor", value: "18 20 22 255" },
    { name: "ProgressBar.FgColor", value: "111 189 111 255" }, { name: "ProgressBar.BgColor", value: "15 18 20 255" },
  ]),
  settings: Object.freeze([]),
  fonts: Object.freeze([
    { name: "Default", cssFamily: "playsrc-vgui-source-required", sizePx: 14, lineHeightPx: 16, weight: 500, style: "normal", available: false },
    { name: "Audit", cssFamily: "sans-serif", sizePx: 14, lineHeightPx: 16, weight: 500, style: "normal", available: true },
  ]),
  borders: Object.freeze([{
    kind: "line", name: "BaseBorder", inset: { left: 1, top: 1, right: 1, bottom: 1 }, backgroundType: 0, paintFirst: false,
    sides: {
      left: [{ color: [10, 10, 10, 255], startOffset: 0, endOffset: 0 }], top: [{ color: [120, 130, 140, 255], startOffset: 0, endOffset: 0 }],
      right: [{ color: [10, 10, 10, 255], startOffset: 0, endOffset: 0 }], bottom: [{ color: [10, 10, 10, 255], startOffset: 0, endOffset: 0 }],
    },
  }]),
  images: Object.freeze([{ name: "test/icon", logicalIdentity: "materials/vgui/test/icon.vtf", revision: "icon-1", browserUrl: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32'%3E%3Crect width='32' height='32' fill='%23e8be5d'/%3E%3C/svg%3E", width: 32, height: 32, frames: 1, hardwareFiltered: false }]),
})

const mount = document.getElementById("mount")
if (!mount) throw new Error("runtime evidence mount is missing")

let runtime: VguiRuntime | null = null
let query = 0
let requestLog: VguiRequest[] = []
let initializationError: string | null = null

function apply(operation: VguiOperation): number | undefined {
  const result = runtime!.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}: ${result.diagnostic.subject}`)
  return result.panel
}

function create(control: VguiOperation & { kind: "create-panel" }): number {
  const id = apply(control)
  if (!id) throw new Error(`failed to create ${control.name}`)
  return id
}

function reset(): void {
  runtime?.apply({ kind: "destroy" })
  mount.replaceChildren()
  requestLog = []
  const initialized = initializeVguiRuntime({
    runtimeIdentity: "browser-evidence",
    root: mount,
    rootControl: { control: "EditablePanel", name: "Root" },
    viewport: { width: mount.clientWidth || innerWidth, height: mount.clientHeight || innerHeight, devicePixelRatio },
    limits,
    clock: { nowSeconds: () => performance.now() / 1000 },
    random: { nextUnit: () => 0.25 },
    scheme,
    localization: { identity: "resource/browser_english.txt", revision: "browser-loc-1", language: "english", tokens: [{ name: "TITLE", value: "VGUI Control Matrix" }] },
    animationScripts: { identity: "scripts/browser-empty", revision: "empty-1", scripts: [], activeConditions: [] },
    customControls: [],
    reducedMotion: false,
    onRequest: (request) => requestLog.push(request),
  })
  if (!initialized.ok) throw new Error(initialized.diagnostic.code)
  runtime = initialized.runtime
  const root = 1
  create({ kind: "create-panel", parent: root, control: "Label", name: "Title", properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "16" }, { name: "wide", value: "300" }, { name: "tall", value: "24" }, { name: "labelText", value: "#TITLE" }, { name: "font", value: "Default" }] })
  create({ kind: "create-panel", parent: root, control: "ImagePanel", name: "Icon", properties: [{ name: "xpos", value: "330" }, { name: "ypos", value: "12" }, { name: "wide", value: "32" }, { name: "tall", value: "32" }, { name: "image", value: "test/icon" }] })
  create({ kind: "create-panel", parent: root, control: "Button", name: "Submit", properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "58" }, { name: "wide", value: "120" }, { name: "tall", value: "30" }, { name: "labelText", value: "Submit" }, { name: "command", value: "submit" }, { name: "font", value: "Default" }, { name: "border", value: "BaseBorder" }, { name: "tabPosition", value: "1" }] })
  create({ kind: "create-panel", parent: root, control: "CheckButton", name: "Toggle", properties: [{ name: "xpos", value: "160" }, { name: "ypos", value: "58" }, { name: "wide", value: "120" }, { name: "tall", value: "30" }, { name: "labelText", value: "Toggle" }, { name: "font", value: "Default" }, { name: "tabPosition", value: "2" }] })
  create({ kind: "create-panel", parent: root, control: "RadioButton", name: "RadioA", properties: [{ name: "xpos", value: "300" }, { name: "ypos", value: "58" }, { name: "wide", value: "110" }, { name: "tall", value: "30" }, { name: "labelText", value: "Radio A" }, { name: "font", value: "Default" }, { name: "TabPosition", value: "3" }, { name: "selected", value: "0" }] })
  create({ kind: "create-panel", parent: root, control: "RadioButton", name: "RadioB", properties: [{ name: "xpos", value: "420" }, { name: "ypos", value: "58" }, { name: "wide", value: "110" }, { name: "tall", value: "30" }, { name: "labelText", value: "Radio B" }, { name: "font", value: "Default" }, { name: "TabPosition", value: "3" }, { name: "selected", value: "0" }] })
  create({ kind: "create-panel", parent: root, control: "MenuItem", name: "StandaloneMenuItem", properties: [{ name: "xpos", value: "550" }, { name: "ypos", value: "58" }, { name: "wide", value: "140" }, { name: "tall", value: "30" }, { name: "labelText", value: "Menu item" }, { name: "font", value: "Default" }] })
  create({ kind: "create-panel", parent: root, control: "TextEntry", name: "Entry", properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "108" }, { name: "wide", value: "180" }, { name: "tall", value: "28" }, { name: "font", value: "Default" }, { name: "NumericInputOnly", value: "1" }, { name: "unicode", value: "1" }, { name: "tabPosition", value: "4" }] })
  create({ kind: "create-panel", parent: root, control: "Slider", name: "Volume", properties: [{ name: "xpos", value: "220" }, { name: "ypos", value: "108" }, { name: "wide", value: "200" }, { name: "tall", value: "28" }, { name: "rangeMin", value: "0" }, { name: "rangeMax", value: "10" }, { name: "value", value: "5" }, { name: "tabPosition", value: "5" }] })
  create({ kind: "create-panel", parent: root, control: "ProgressBar", name: "Progress", properties: [{ name: "xpos", value: "440" }, { name: "ypos", value: "108" }, { name: "wide", value: "180" }, { name: "tall", value: "28" }, { name: "progress", value: "0.65" }] })
  create({ kind: "create-panel", parent: root, control: "Panel", name: "PlainPanel", properties: [{ name: "xpos", value: "640" }, { name: "ypos", value: "108" }, { name: "wide", value: "100" }, { name: "tall", value: "28" }] })
  const menu = create({ kind: "create-panel", parent: root, control: "Menu", name: "Menu", properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "156" }, { name: "wide", value: "180" }, { name: "tall", value: "92" }, { name: "tabPosition", value: "6" }] })
  apply({ kind: "mutate-control", panel: menu, mutation: { items: [{ id: 1, text: "Video" }, { id: 2, text: "Audio" }, { id: 3, text: "Mouse" }] } })
  apply({ kind: "set-panel-state", panel: menu, popup: true, visible: true })
  create({ kind: "create-panel", parent: root, control: "RichText", name: "RichText", properties: [{ name: "xpos", value: "220" }, { name: "ypos", value: "156" }, { name: "wide", value: "200" }, { name: "tall", value: "92" }, { name: "text", value: "Selectable rich text" }, { name: "font", value: "Default" }] })
  const list = create({ kind: "create-panel", parent: root, control: "ListPanel", name: "List", properties: [{ name: "xpos", value: "440" }, { name: "ypos", value: "156" }, { name: "wide", value: "180" }, { name: "tall", value: "92" }, { name: "linespacing", value: "20" }] })
  apply({ kind: "mutate-control", panel: list, mutation: { items: [{ id: 1, text: "Item one" }, { id: 2, text: "Item two" }] } })
  const frame = create({ kind: "create-panel", parent: root, control: "Frame", name: "Frame", properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "280" }, { name: "wide", value: "300" }, { name: "tall", value: "180" }, { name: "title", value: "Moveable frame" }, { name: "title_font", value: "Default" }, { name: "border", value: "BaseBorder" }] })
  apply({ kind: "set-panel-state", panel: frame, visible: true })
  const sheet = create({ kind: "create-panel", parent: frame, control: "PropertySheet", name: "Sheet", properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "36" }, { name: "wide", value: "280" }, { name: "tall", value: "130" }] })
  const pageA = create({ kind: "create-panel", parent: sheet, control: "PropertyPage", name: "PageVideo" })
  const pageB = create({ kind: "create-panel", parent: sheet, control: "PropertyPage", name: "PageAudio" })
  apply({ kind: "mutate-control", panel: sheet, mutation: { items: [{ id: pageA, text: "Video" }, { id: pageB, text: "Audio" }] } })
  create({ kind: "create-panel", parent: root, control: "ScrollBar_Vertical", name: "Scroll", properties: [{ name: "xpos", value: "340" }, { name: "ypos", value: "280" }, { name: "wide", value: "20" }, { name: "tall", value: "180" }, { name: "rangeMin", value: "0" }, { name: "rangeMax", value: "100" }, { name: "rangeWindow", value: "20" }] })
  create({ kind: "create-panel", parent: root, control: "ScrollBar", name: "HorizontalScroll", properties: [{ name: "xpos", value: "390" }, { name: "ypos", value: "370" }, { name: "wide", value: "180" }, { name: "tall", value: "20" }, { name: "rangeMin", value: "0" }, { name: "rangeMax", value: "100" }, { name: "rangeWindow", value: "20" }] })
  create({ kind: "create-panel", parent: root, control: "ComboBox", name: "Combo", properties: [{ name: "xpos", value: "390" }, { name: "ypos", value: "280" }, { name: "wide", value: "180" }, { name: "tall", value: "30" }, { name: "font", value: "Default" }, { name: "tabPosition", value: "7" }] })
  create({ kind: "create-panel", parent: root, control: "URLLabel", name: "URL", properties: [{ name: "xpos", value: "390" }, { name: "ypos", value: "330" }, { name: "wide", value: "180" }, { name: "tall", value: "24" }, { name: "labelText", value: "TF2 help" }, { name: "URLText", value: "https://example.test/tf2" }, { name: "font", value: "Default" }] })
  const message = create({ kind: "create-panel", parent: root, control: "MessageBox", name: "Message", properties: [{ name: "xpos", value: "760" }, { name: "ypos", value: "300" }, { name: "wide", value: "240" }, { name: "tall", value: "120" }, { name: "title", value: "Message box" }] })
  apply({ kind: "set-panel-state", panel: message, visible: true })
  const auditButton = create({ kind: "create-panel", parent: root, control: "Button", name: "AuditDisabledButton", properties: [{ name: "xpos", value: "760" }, { name: "ypos", value: "440" }, { name: "wide", value: "220" }, { name: "tall", value: "30" }, { name: "labelText", value: "Left aligned disabled" }, { name: "font", value: "Audit" }, { name: "textAlignment", value: "west" }] })
  apply({ kind: "set-panel-state", panel: auditButton, enabled: false })
  query = create({ kind: "create-panel", parent: root, control: "QueryBox", name: "Query", properties: [{ name: "xpos", value: "430" }, { name: "ypos", value: "380" }, { name: "wide", value: "240" }, { name: "tall", value: "120" }, { name: "title", value: "Apply settings?" }] })
  apply({ kind: "frame", timeSeconds: performance.now() / 1000 })
}

window.addEventListener("resize", () => runtime?.apply({ kind: "set-viewport", viewport: { width: mount.clientWidth || innerWidth, height: mount.clientHeight || innerHeight, devicePixelRatio } }))

window.vguiRuntimeEvidence = Object.freeze({
  snapshot: () => runtime?.snapshot(),
  requests: () => Object.freeze([...requestLog]),
  status: () => Object.freeze({ ready: runtime !== null && initializationError === null, error: initializationError }),
  showQuery: () => {
    apply({ kind: "set-panel-state", panel: query, visible: true })
    apply({ kind: "set-application-modal", panel: query })
    apply({ kind: "request-focus", panel: query })
    apply({ kind: "frame", timeSeconds: performance.now() / 1000 })
  },
  cycles: (count: number) => {
    for (let index = 0; index < count; index += 1) reset()
    return { mountChildren: mount.childElementCount, snapshot: runtime?.snapshot() }
  },
})

try {
  reset()
} catch (error) {
  initializationError = error instanceof Error ? error.message : String(error)
}
