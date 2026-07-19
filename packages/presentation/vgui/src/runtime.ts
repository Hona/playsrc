/*
 * Source-facing behavior in this file is adapted from Valve Source SDK 2013
 * under the Source 1 SDK License. See ../LICENSE.source-sdk-2013 and the
 * repository's included thirdpartylegalnotices.txt.
 */
import type { Rgba } from "./contract"
import {
  VGUI_GENERIC_CONTROL_NAMES,
  type VguiAnimationCommand,
  type VguiAnimationConverter,
  type VguiAnimationInterpolator,
  type VguiAnimationScriptSet,
  type VguiAnimationVariable,
  type VguiBorder,
  type VguiControlMutation,
  type VguiControlName,
  type VguiControlRegistration,
  type VguiDiagnostic,
  type VguiDiagnosticCode,
  type VguiFontPresentation,
  type VguiImagePresentation,
  type VguiLocalization,
  type VguiMessage,
  type VguiMessageValue,
  type VguiOperation,
  type VguiOperationResult,
  type VguiPanelId,
  type VguiPanelSnapshot,
  type VguiPointerButton,
  type VguiRect,
  type VguiRequest,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiResourceSelection,
  type VguiRuntime,
  type VguiRuntimeConfiguration,
  type VguiRuntimeInitialization,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiScheme,
  type VguiViewport,
} from "./runtime-contract"
import { VGUI_CSS } from "./style"

const IDENTITY = /^[a-z0-9][a-z0-9./_-]{0,511}$/u
const RUNTIME_IDENTITY = /^[a-z0-9][a-z0-9_-]{0,127}$/u
const NAME = /^[^\u0000-\u001f\u007f]{1,255}$/u
const INTEGER = /^[+-]?\d+$/u
const FLOAT = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/iu
const URL = /^https?:\/\//u
const EMPTY_INSETS = Object.freeze({ left: 0, top: 0, right: 0, bottom: 0 })
const TRANSPARENT: Rgba = Object.freeze([0, 0, 0, 0])
const WHITE: Rgba = Object.freeze([255, 255, 255, 255])

const BASE_ANIMATION_VARIABLES: readonly VguiAnimationVariable[] = Object.freeze([
  Object.freeze({ name: "alpha", converter: "float" as const, defaultValue: "255" }),
  Object.freeze({ name: "PaintBackgroundType", converter: "int" as const, defaultValue: "0" }),
  Object.freeze({ name: "Texture1", converter: "textureid" as const, defaultValue: "vgui/hud/8x800corner1" }),
  Object.freeze({ name: "Texture2", converter: "textureid" as const, defaultValue: "vgui/hud/8x800corner2" }),
  Object.freeze({ name: "Texture3", converter: "textureid" as const, defaultValue: "vgui/hud/8x800corner3" }),
  Object.freeze({ name: "Texture4", converter: "textureid" as const, defaultValue: "vgui/hud/8x800corner4" }),
])

const FRAME_ANIMATION_VARIABLES: readonly VguiAnimationVariable[] = Object.freeze([
  Object.freeze({ name: "titletextinsetX", converter: "proportional_int" as const, defaultValue: "0" }),
  Object.freeze({ name: "titletextinsetY", converter: "int" as const, defaultValue: "0" }),
])

const SCROLLBAR_ANIMATION_VARIABLES: readonly VguiAnimationVariable[] = Object.freeze([
  Object.freeze({ name: "autohide_buttons", converter: "bool" as const, defaultValue: "0" }),
])

const PROPERTY_SHEET_ANIMATION_VARIABLES: readonly VguiAnimationVariable[] = Object.freeze([
  Object.freeze({ name: "yoffset", converter: "proportional_int" as const, defaultValue: "0" }),
  Object.freeze({ name: "tabxindent", converter: "proportional_int" as const, defaultValue: "0" }),
  Object.freeze({ name: "tabxdelta", converter: "proportional_int" as const, defaultValue: "0" }),
  Object.freeze({ name: "tabxfittotext", converter: "bool" as const, defaultValue: "1" }),
  Object.freeze({ name: "tabheight", converter: "int" as const, defaultValue: "28" }),
  Object.freeze({ name: "tabheight_small", converter: "int" as const, defaultValue: "14" }),
])

const BASE_PROPERTIES = new Set([
  "ControlName", "fieldName", "xpos", "ypos", "wide", "tall", "zpos", "proportionalToParent", "usetitlesafe",
  "AutoResize", "PinnedCornerOffsetX", "PinnedCornerOffsetY", "UnpinnedCornerOffsetX", "UnpinnedCornerOffsetY", "PinCorner",
  "pin_to_sibling", "pin_corner_to_sibling", "pin_to_sibling_corner", "navUp", "navDown", "navLeft", "navRight", "navToRelay",
  "navActivate", "navBack", "visible", "enabled", "mouseinputenabled", "keyboardinputenabled", "tabPosition", "TabPosition",
  "SubTabPosition", "tooltiptext", "paintbackground", "paintborder", "border", "border_override", "normalborder_override",
  "activeborder_override", "IgnoreScheme", "actionsignallevel", "RoundedCorners", "ForceStereoRenderToFrameBuffer", "alpha",
  "PaintBackgroundType", "Texture1", "Texture2", "Texture3", "Texture4", "fgcolor_override", "bgcolor_override", "skip_autoresize",
])

const OBJECT_PROPERTIES = new Set(["Button", "UpButton", "DownButton", "Slider", "Scrollbar", "tabskv"])

const GENERIC_SCHEME_COLOR_LOOKUPS = Object.freeze([
  "Panel.FgColor", "Panel.BgColor",
  "Label.TextDullColor", "Label.TextColor", "Label.TextBrightColor", "Label.SelectedTextColor", "Label.BgColor", "Label.DisabledFgColor1", "Label.DisabledFgColor2",
  "Button.TextColor", "Button.BgColor", "Button.ArmedTextColor", "Button.ArmedBgColor", "Button.DepressedTextColor", "Button.DepressedBgColor", "Button.SelectedTextColor", "Button.SelectedBgColor",
  "CheckButton.TextColor", "CheckButton.SelectedTextColor", "CheckButton.BgColor", "CheckButton.Border1", "CheckButton.Border2", "CheckButton.Check",
  "RadioButton.TextColor", "RadioButton.SelectedTextColor", "RadioButton.ArmedTextColor",
  "Menu.TextColor", "Menu.BgColor", "Menu.ArmedTextColor", "Menu.ArmedBgColor",
  "TextEntry.TextColor", "TextEntry.BgColor", "TextEntry.CursorColor", "TextEntry.DisabledTextColor", "TextEntry.DisabledBgColor", "TextEntry.SelectedTextColor", "TextEntry.SelectedBgColor", "TextEntry.OutOfFocusSelectedBgColor",
  "RichText.TextColor", "RichText.BgColor", "RichText.SelectedTextColor", "RichText.SelectedBgColor",
  "Frame.BgColor", "Frame.OutOfFocusBgColor", "FrameTitleBar.TextColor", "FrameTitleBar.BgColor", "FrameTitleBar.DisabledTextColor", "FrameTitleBar.DisabledBgColor",
  "ProgressBar.FgColor", "ProgressBar.BgColor", "Slider.NobColor", "Slider.TextColor", "Slider.TrackColor", "Slider.DisabledTextColor1", "Slider.DisabledTextColor2",
  "ScrollBarSlider.FgColor", "ScrollBarSlider.BgColor", "ListPanel.TextColor", "ListPanel.BgColor", "ListPanel.SelectedTextColor", "ListPanel.SelectedBgColor", "ListPanel.SelectedOutOfFocusBgColor",
])

const CONTROL_PROPERTIES: Readonly<Record<string, ReadonlySet<string>>> = Object.freeze({
  Label: new Set(["labelText", "text", "font", "dulltext", "brighttext", "allcaps", "textAlignment", "textinsetx", "textinsety", "use_proportional_insets", "associate", "wrap", "centerwrap", "auto_wide_tocontents", "auto_tall_tocontents", "disabledfgcolor2_override"]),
  ImagePanel: new Set(["image", "drawcolor", "fillcolor", "scaleImage", "scaleProportional", "scaleAmount", "tileImage", "tileHorizontally", "tileVertically", "positionImage", "rotation", "imagecolor", "imageAlignment", "preserveAspectRatio", "filtered", "fillcolor_override", "drawcolor_override"]),
  Button: new Set(["labelText", "text", "font", "textAlignment", "command", "default", "selected", "stayselectedonclick", "stay_armed_on_click", "button_activation_type", "sound_armed", "sound_depressed", "sound_released", "defaultFgColor_override", "defaultBgColor_override", "armedFgColor_override", "armedBgColor_override", "depressedFgColor_override", "depressedBgColor_override", "selectedFgColor_override", "selectedBgColor_override", "keyboardFocusColor_override"]),
  TextEntry: new Set(["text", "font", "editable", "maxchars", "NumericInputOnly", "selectallonfirstfocus", "textHidden", "unicode", "multiline", "catchenter", "sendnewlines"]),
  RichText: new Set(["text", "textfile", "font", "maxchars", "scrollbar"]),
  Frame: new Set(["title", "title_font", "settitlebarvisible", "setclosebuttonvisible", "clientinsetx_override", "moveable", "sizeable", "deleteSelfOnClose", "titletextinsetX", "titletextinsetY", "infocus_bgcolor_override", "outoffocus_bgcolor_override", "titlebarbgcolor_override", "titlebardisabledbgcolor_override", "titlebarfgcolor_override", "titlebardisabledfgcolor_override"]),
  ScrollBar: new Set(["nobuttons", "UpButton", "DownButton", "Slider", "ButtonBorder", "autohide_buttons", "vertical", "rangeMin", "rangeMax", "rangeWindow", "value"]),
  Slider: new Set(["rangeMin", "rangeMax", "numTicks", "thumbwidth", "leftText", "rightText", "value", "inverted", "drag_on_reposition"]),
  ComboBox: new Set(["Button", "border_override", "editable", "maxchars", "NumericInputOnly", "selectallonfirstfocus", "textHidden", "unicode", "numLines", "text"]),
  Menu: new Set(["border", "itemheight", "numVisibleLines", "fixedWidth"]),
  MenuItem: new Set(["labelText", "text", "font", "command", "checkable", "checked", "cascade", "accelerator"]),
  PropertySheet: new Set(["tabwidth", "tabskv", "transition_time", "tabxindent", "tabxdelta", "tabxfittotext", "tabheight", "tabheight_small", "yoffset"]),
  PropertyPage: new Set([]),
  CheckButton: new Set(["labelText", "text", "font", "command", "default", "selected", "smallcheckimage"]),
  RadioButton: new Set(["labelText", "text", "font", "command", "default", "selected", "TabPosition", "SubTabPosition"]),
  ProgressBar: new Set(["progress", "variable", "analogValue", "direction", "segment_gap", "segment_width", "bar_inset", "margin"]),
  ListPanel: new Set(["sectiongap", "linegap", "linespacing", "show_columns", "autohide_scrollbar", "multiselect"]),
  MessageBox: new Set(["title", "labelText", "text", "okcommand", "cancelcommand", "noautoclose"]),
  QueryBox: new Set(["title", "labelText", "text", "okcommand", "cancelcommand"]),
  URLLabel: new Set(["labelText", "text", "font", "URLText", "textAlignment"]),
  Panel: new Set([]),
  EditablePanel: new Set([]),
})

type ControlItem = {
  id: number
  text: string
  command: string | null
  enabled: boolean
  checked: boolean
}

type PanelState = {
  id: VguiPanelId
  control: VguiControlName
  registration: VguiControlRegistration
  name: string
  parent: VguiPanelId | null
  children: VguiPanelId[]
  tieOrder: number
  resourceOwner: string | null
  properties: Map<string, string>
  bounds: { x: number; y: number; width: number; height: number }
  absoluteBounds: { x: number; y: number; width: number; height: number }
  clip: { x: number; y: number; width: number; height: number }
  inset: { left: number; top: number; right: number; bottom: number }
  minimumWidth: number
  minimumHeight: number
  autoResize: 0 | 1 | 2 | 3
  pinCorner: 0 | 1 | 2 | 3
  pinOffsetX: number
  pinOffsetY: number
  resizeOffsetX: number
  resizeOffsetY: number
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
  subTabPosition: number
  nav: Map<string, string>
  actionTargets: VguiPanelId[]
  dialogVariables: Map<string, string>
  defaultButton: VguiPanelId | null
  currentDefaultButton: VguiPanelId | null
  textSource: string
  text: string
  bodyTextSource: string
  bodyText: string
  accessibleName: string
  accessibleDescription: string
  tooltip: string
  command: string | null
  url: string | null
  border: string | null
  font: string | null
  image: string | null
  drawColor: Rgba
  fillColor: Rgba
  armed: boolean
  depressed: boolean
  selected: boolean
  checked: boolean
  checkable: boolean
  staySelected: boolean
  stayArmed: boolean
  activation: 0 | 1 | 2
  value: number
  minimum: number
  maximum: number
  rangeWindow: number
  numTicks: number
  thumbWidth: number
  dragging: boolean
  dragStartValue: number
  dragStartCoordinate: number
  frameInteraction: "move" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw" | null
  frameStartBounds: { x: number; y: number; width: number; height: number }
  frameStartPointer: readonly [number, number]
  frameClosePressed: boolean
  progress: number
  progressVariable: string | null
  items: ControlItem[]
  itemElements: Map<number, HTMLElement>
  chromeElements: Map<string, HTMLElement>
  pressedItem: number | null
  activeIndex: number | null
  caret: number
  selectionStart: number
  selectionEnd: number
  editable: boolean
  multiline: boolean
  numericOnly: boolean
  allowUnicode: boolean
  textHidden: boolean
  maximumCharacters: number
  selectAllOnFirstFocus: boolean
  firstFocus: boolean
  compositionActive: boolean
  compositionText: string
  compositionCaret: number
  animationDefinitions: Map<string, VguiAnimationVariable>
  animationValues: Map<string, string | number | boolean | Rgba>
  element: HTMLElement
}

type QueuedMessage = {
  order: number
  due: number
  target: VguiPanelId
  source: VguiPanelId | null
  message: VguiMessage
}

type DelayedAnimationCommand = {
  order: number
  due: number
  sequence: string
  parent: VguiPanelId
  cancelable: boolean
  command: Exclude<VguiAnimationCommand, { kind: "Animate" }>
}

type AnimationValue = readonly [number, number, number, number]

type ActiveAnimation = {
  order: number
  sequence: string
  parent: VguiPanelId
  panel: VguiPanelId
  variable: string
  cancelable: boolean
  start: number
  end: number
  interpolator: string
  parameter: number
  relative: Readonly<{ panel: string; alignment: string }> | null
  started: boolean
  startValue: AnimationValue
  endValue: AnimationValue
}

type SequenceState = {
  name: string
  commands: readonly VguiAnimationCommand[]
  duration: number
}

type ListenerRecord = Readonly<{
  target: EventTarget
  type: string
  listener: EventListener
  options?: AddEventListenerOptions | boolean
}>

type ResourceControlPlan = Readonly<{
  blockName: string
  control: VguiControlName
  properties: readonly Readonly<{ name: string; value: string }>[]
  existing: VguiPanelId | null
}>

class RuntimeFault extends Error {
  constructor(
    readonly code: VguiDiagnosticCode,
    readonly subject: string,
  ) {
    super(`${code}: ${subject}`)
  }
}

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/gu, (character) => character.toLowerCase())
}

function sameName(left: string, right: string): boolean {
  return asciiFold(left) === asciiFold(right)
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value)
}

function safeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value)
}

function validViewport(viewport: VguiViewport): boolean {
  return !!viewport
    && safeInteger(viewport.width)
    && safeInteger(viewport.height)
    && viewport.width > 0
    && viewport.height > 0
    && viewport.width <= 32767
    && viewport.height <= 32767
    && finite(viewport.devicePixelRatio)
    && viewport.devicePixelRatio > 0
    && viewport.devicePixelRatio <= 16
}

function validLimits(limits: VguiRuntimeLimits): boolean {
  if (!limits || typeof limits !== "object") return false
  return Object.values(limits).every((value) => safeInteger(value) && value > 0)
}

function validString(value: unknown, maximum: number, allowEmpty = true): value is string {
  return typeof value === "string"
    && (allowEmpty || value.length > 0)
    && value.length <= maximum
    && !value.includes("\0")
}

function freezeMessage(message: VguiMessage): VguiMessage {
  return Object.freeze({ name: message.name, fields: Object.freeze({ ...message.fields }) })
}

function cloneNode(node: VguiResourceNode): VguiResourceNode {
  return {
    name: node.name,
    value: node.value,
    condition: node.condition,
    children: node.children.map(cloneNode),
  }
}

function firstScalar(node: VguiResourceNode, name: string): string | null {
  const found = node.children.find((child) => sameName(child.name, name) && child.value !== null)
  return found?.value ?? null
}

function parseBoolean(value: string, subject: string): boolean {
  if (sameName(value, "true") || value === "1") return true
  if (sameName(value, "false") || value === "0" || value === "") return false
  if (INTEGER.test(value)) return Number.parseInt(value, 10) !== 0
  throw new RuntimeFault("MalformedValue", subject)
}

function parseInteger(value: string, subject: string): number {
  if (!INTEGER.test(value)) throw new RuntimeFault("MalformedValue", subject)
  const result = Number(value)
  if (!Number.isSafeInteger(result) || result < -2147483648 || result > 2147483647) {
    throw new RuntimeFault("MalformedValue", subject)
  }
  return result
}

function parseFloatValue(value: string, subject: string): number {
  if (!FLOAT.test(value)) throw new RuntimeFault("MalformedValue", subject)
  const result = Number(value)
  if (!Number.isFinite(result)) throw new RuntimeFault("MalformedValue", subject)
  return result
}

function parseColorLiteral(value: string, fallbackAlpha: number): Rgba | null {
  const parts = value.trim().split(/\s+/u)
  if (parts.length < 3 || parts.length > 4 || parts.some((part) => !INTEGER.test(part))) return null
  const channels = parts.map(Number)
  if (channels.some((channel) => channel < 0 || channel > 255)) return null
  return Object.freeze([channels[0], channels[1], channels[2], channels[3] ?? fallbackAlpha]) as Rgba
}

function rgba(color: Rgba): string {
  return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${color[3] / 255})`
}

function rectIntersection(left: VguiRect, right: VguiRect): VguiRect {
  const x = Math.max(left.x, right.x)
  const y = Math.max(left.y, right.y)
  const rightEdge = Math.max(x, Math.min(left.x + left.width, right.x + right.width))
  const bottomEdge = Math.max(y, Math.min(left.y + left.height, right.y + right.height))
  return { x, y, width: rightEdge - x, height: bottomEdge - y }
}

function inside(rect: VguiRect, x: number, y: number): boolean {
  return x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}

function defaultAnimationVariables(control: string): readonly VguiAnimationVariable[] {
  if (sameName(control, "Frame")) return [...BASE_ANIMATION_VARIABLES, ...FRAME_ANIMATION_VARIABLES]
  if (sameName(control, "ScrollBar") || sameName(control, "ScrollBar_Vertical") || sameName(control, "ScrollBar_Horizontal")) {
    return [...BASE_ANIMATION_VARIABLES, ...SCROLLBAR_ANIMATION_VARIABLES]
  }
  if (sameName(control, "PropertySheet")) return [...BASE_ANIMATION_VARIABLES, ...PROPERTY_SHEET_ANIMATION_VARIABLES]
  return BASE_ANIMATION_VARIABLES
}

function elementForControl(control: string): keyof HTMLElementTagNameMap {
  if (["Button", "CheckButton", "RadioButton"].some((name) => sameName(control, name))) return "button"
  if (sameName(control, "TextEntry")) return "textarea"
  if (sameName(control, "ComboBox")) return "input"
  if (sameName(control, "ImagePanel")) return "img"
  if (sameName(control, "URLLabel")) return "a"
  if (sameName(control, "Menu")) return "ul"
  if (sameName(control, "MenuItem")) return "li"
  if (sameName(control, "ListPanel")) return "div"
  return "div"
}

function roleForControl(control: string): string | null {
  if (sameName(control, "Label")) return null
  if (sameName(control, "ImagePanel")) return "img"
  if (sameName(control, "Button")) return "button"
  if (sameName(control, "CheckButton")) return "checkbox"
  if (sameName(control, "RadioButton")) return "radio"
  if (sameName(control, "TextEntry")) return "textbox"
  if (sameName(control, "RichText")) return "document"
  if (sameName(control, "Frame") || sameName(control, "MessageBox") || sameName(control, "QueryBox")) return "dialog"
  if (sameName(control, "ScrollBar")) return "scrollbar"
  if (sameName(control, "Slider")) return "slider"
  if (sameName(control, "ComboBox")) return "combobox"
  if (sameName(control, "Menu")) return "menu"
  if (sameName(control, "MenuItem")) return "menuitem"
  if (sameName(control, "PropertySheet")) return "tablist"
  if (sameName(control, "PropertyPage")) return "tabpanel"
  if (sameName(control, "ProgressBar")) return "progressbar"
  if (sameName(control, "ListPanel")) return "grid"
  if (sameName(control, "URLLabel")) return "link"
  return null
}

function focusableControl(control: string): boolean {
  return [
    "Button", "CheckButton", "RadioButton", "TextEntry", "RichText", "Frame", "ScrollBar", "Slider", "ComboBox", "Menu",
    "MenuItem", "PropertySheet", "PropertyPage", "ListPanel", "MessageBox", "QueryBox", "URLLabel",
  ].some((name) => sameName(control, name))
}

function genericRegistration(control: string): VguiControlRegistration {
  const sourceControl = asSourceControl(control)
  return Object.freeze({
    name: control,
    element: elementForControl(sourceControl),
    role: roleForControl(sourceControl),
    focusable: focusableControl(sourceControl),
    animationVariables: Object.freeze(defaultAnimationVariables(control)),
    acceptedProperties: Object.freeze([]),
  })
}

function asSourceControl(control: VguiControlName): string {
  if (sameName(control, "ScrollBar_Vertical") || sameName(control, "ScrollBar_Horizontal")) return "ScrollBar"
  return control
}

function propertyAllowed(control: string, registration: VguiControlRegistration, property: string): boolean {
  if ([...BASE_PROPERTIES].some((name) => sameName(name, property))) return true
  const sourceControl = asSourceControl(control)
  const inherited: string[] = [sourceControl]
  if (["Button", "CheckButton", "RadioButton", "MenuItem", "URLLabel"].some((name) => sameName(sourceControl, name))) inherited.push("Label")
  if (["CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(sourceControl, name))) inherited.push("Button")
  if (sameName(sourceControl, "ComboBox")) inherited.push("TextEntry")
  if (["MessageBox", "QueryBox"].some((name) => sameName(sourceControl, name))) inherited.push("Frame", "EditablePanel")
  if (["Frame", "PropertyPage"].some((name) => sameName(sourceControl, name))) inherited.push("EditablePanel")
  for (const identity of inherited) {
    const properties = Object.entries(CONTROL_PROPERTIES).find(([name]) => sameName(name, identity))?.[1]
    if (properties && [...properties].some((name) => sameName(name, property))) return true
  }
  return registration.acceptedProperties.some((name) => sameName(name, property))
}

function validRegistration(registration: VguiControlRegistration, limits: VguiRuntimeLimits): boolean {
  return !!registration
    && validString(registration.name, 255, false)
    && validString(registration.element, 32, false)
    && (registration.role === null || validString(registration.role, 64, false))
    && typeof registration.focusable === "boolean"
    && Array.isArray(registration.animationVariables)
    && registration.animationVariables.length <= limits.maxPropertiesPerPanel
    && Array.isArray(registration.acceptedProperties)
    && registration.acceptedProperties.length <= limits.maxPropertiesPerPanel
}

function validResourceNode(
  node: VguiResourceNode,
  limits: VguiRuntimeLimits,
  seen: Set<VguiResourceNode>,
  depth: number,
  count: { value: number },
): boolean {
  if (!node || typeof node !== "object" || seen.has(node) || depth > limits.maxResourceDepth) return false
  if (!validString(node.name, limits.maxStringCodeUnits, false)) return false
  if (node.value !== null && !validString(node.value, limits.maxStringCodeUnits)) return false
  if (node.condition !== null && !validString(node.condition, 255, false)) return false
  if (!Array.isArray(node.children)) return false
  count.value += 1
  if (count.value > limits.maxResourceNodes) return false
  seen.add(node)
  const valid = node.children.every((child) => validResourceNode(child, limits, seen, depth + 1, count))
  seen.delete(node)
  return valid
}

function conditionActive(condition: string | null, active: ReadonlySet<string>): boolean {
  if (condition === null) return true
  let normalized = condition.trim()
  if (normalized.startsWith("[")) normalized = normalized.slice(1)
  if (normalized.endsWith("]")) normalized = normalized.slice(0, -1)
  const negated = normalized.startsWith("!")
  if (negated) normalized = normalized.slice(1)
  if (normalized.startsWith("$")) normalized = normalized.slice(1)
  const selected = active.has(asciiFold(normalized))
  return negated ? !selected : selected
}

function filterConditions(node: VguiResourceNode, active: ReadonlySet<string>): VguiResourceNode | null {
  if (!conditionActive(node.condition, active)) return null
  return {
    name: node.name,
    value: node.value,
    condition: null,
    children: node.children
      .map((child) => filterConditions(child, active))
      .filter((child): child is VguiResourceNode => child !== null),
  }
}

function applyResolutionSuffix(node: VguiResourceNode, suffix: string): VguiResourceNode {
  const children = node.children.map((child) => applyResolutionSuffix(child, suffix))
  const foldedSuffix = asciiFold(suffix)
  for (let index = 0; index < children.length; index += 1) {
    const child = children[index]
    if (!asciiFold(child.name).endsWith(foldedSuffix)) continue
    const plainName = child.name.slice(0, child.name.length - suffix.length)
    const original = children.findIndex((candidate, candidateIndex) => candidateIndex !== index && sameName(candidate.name, plainName))
    if (original >= 0) {
      children.splice(original, 1)
      if (original < index) index -= 1
    }
    children[index] = { ...child, name: plainName }
  }
  return { ...node, children }
}

function promoteConditionBlocks(node: VguiResourceNode, activeConditions: readonly string[]): VguiResourceNode {
  let children = node.children.map((child) => promoteConditionBlocks(child, activeConditions))
  for (const condition of activeConditions) {
    const block = children.find((child) => sameName(child.name, condition) && child.value === null)
    if (!block) continue
    for (const override of block.children) {
      const existing = children.findIndex((child) => sameName(child.name, override.name))
      if (existing >= 0) children[existing] = { ...children[existing], value: override.value, children: override.children }
      else children.push(cloneNode(override))
    }
  }
  return { ...node, children }
}

function selectedResourceRoot(document: VguiResourceDocument, selection: VguiResourceSelection): VguiResourceNode {
  const active = new Set(selection.activeConditions.map(asciiFold))
  let root = filterConditions(document.root, active)
  if (!root) throw new RuntimeFault("MalformedResource", document.logicalIdentity)
  for (const suffix of selection.resolutionSuffixes) root = applyResolutionSuffix(root, suffix)
  return promoteConditionBlocks(root, selection.activeConditions)
}

function cloneItems(items: readonly ControlItem[]): readonly Readonly<ControlItem>[] {
  return Object.freeze(items.map((item) => Object.freeze({ ...item })))
}

function blankAnimationValue(): AnimationValue {
  return Object.freeze([0, 0, 0, 0])
}

export function initializeVguiRuntime(configuration: VguiRuntimeConfiguration): VguiRuntimeInitialization {
  try {
    return Object.freeze({ ok: true, runtime: new SourceVguiRuntime(configuration) })
  } catch (error) {
    const fault = error instanceof RuntimeFault ? error : new RuntimeFault("InvalidConfiguration", "configuration")
    return Object.freeze({
      ok: false,
      diagnostic: Object.freeze({ sequence: 1, code: fault.code, operation: "initialize", subject: fault.subject }),
    })
  }
}

class SourceVguiRuntime implements VguiRuntime {
  private readonly runtimeIdentity: string
  private readonly root: HTMLElement
  private readonly document: Document
  private readonly limits: VguiRuntimeLimits
  private readonly clock: VguiRuntimeConfiguration["clock"]
  private readonly random: VguiRuntimeConfiguration["random"]
  private readonly onRequest: VguiRuntimeConfiguration["onRequest"]
  private readonly style: HTMLStyleElement
  private readonly host: HTMLElement
  private readonly panels = new Map<VguiPanelId, PanelState>()
  private readonly auxiliaryNodes = new Set<HTMLElement>()
  private readonly registrations = new Map<string, VguiControlRegistration>()
  private readonly listeners: ListenerRecord[] = []
  private readonly queuedMessages: QueuedMessage[] = []
  private readonly delayedCommands: DelayedAnimationCommand[] = []
  private readonly activeAnimations: ActiveAnimation[] = []
  private readonly deferredDeletes = new Set<VguiPanelId>()
  private readonly pendingRequests: VguiRequest[] = []
  private readonly pendingClipboardReads = new Map<number, VguiPanelId>()
  private readonly trace: { sequence: number; phase: string; panel: VguiPanelId | null; detail: string }[] = []
  private readonly diagnostics: VguiDiagnostic[] = []
  private readonly popups: VguiPanelId[] = []
  private readonly pendingPressedKeys = new Set<string>()
  private readonly pendingReleasedKeys = new Set<string>()
  private readonly downKeys = new Set<string>()
  private readonly pressedKeys = new Set<string>()
  private readonly releasedKeys = new Set<string>()
  private readonly pendingPressedButtons = new Set<VguiPointerButton>()
  private readonly pendingReleasedButtons = new Set<VguiPointerButton>()
  private readonly downButtons = new Set<VguiPointerButton>()
  private readonly pressedButtons = new Set<VguiPointerButton>()
  private readonly releasedButtons = new Set<VguiPointerButton>()
  private readonly localization = new Map<string, string>()
  private readonly colors = new Map<string, string>()
  private readonly settings = new Map<string, string>()
  private readonly fonts = new Map<string, VguiFontPresentation>()
  private readonly borders = new Map<string, VguiBorder>()
  private readonly images = new Map<string, VguiImagePresentation>()
  private readonly sequences = new Map<string, SequenceState>()
  private scheme: VguiScheme
  private localizationRecord: VguiLocalization
  private animationScripts: VguiAnimationScriptSet
  private viewport: VguiViewport
  private reducedMotion: boolean
  private destroyed = false
  private inFrame = false
  private revision = 0
  private frame = 0
  private timeSeconds = 0
  private nextPanelId = 1
  private nextOrder = 1
  private nextRequestId = 1
  private nextTrace = 1
  private nextDiagnostic = 1
  private pointerX = 0
  private pointerY = 0
  private pointerId: number | null = null
  private mouseOver: VguiPanelId | null = null
  private mouseFocus: VguiPanelId | null = null
  private capture: VguiPanelId | null = null
  private captureButton: VguiPointerButton | null = null
  private keyFocus: VguiPanelId | null = null
  private calculatedKeyFocus: VguiPanelId | null = null
  private requestedFocus: VguiPanelId | null = null
  private clearFocusRequested = false
  private applicationModal: VguiPanelId | null = null
  private modalSubtree: VguiPanelId | null = null
  private modalRestrict = true
  private outsideClickListener: VguiPanelId | null = null
  private readonly rootPanel: VguiPanelId

  constructor(configuration: VguiRuntimeConfiguration) {
    if (!configuration || !RUNTIME_IDENTITY.test(configuration.runtimeIdentity)) throw new RuntimeFault("InvalidConfiguration", "runtimeIdentity")
    if (!configuration.root || typeof configuration.root.append !== "function" || !configuration.root.ownerDocument) throw new RuntimeFault("InvalidConfiguration", "root")
    if (!configuration.rootControl || !["Panel", "EditablePanel"].includes(configuration.rootControl.control) || !NAME.test(configuration.rootControl.name)) throw new RuntimeFault("InvalidConfiguration", "rootControl")
    if (!validViewport(configuration.viewport)) throw new RuntimeFault("InvalidViewport", "viewport")
    if (!validLimits(configuration.limits)) throw new RuntimeFault("InvalidConfiguration", "limits")
    if (!configuration.clock || typeof configuration.clock.nowSeconds !== "function" || !finite(configuration.clock.nowSeconds())) throw new RuntimeFault("InvalidConfiguration", "clock")
    if (!configuration.random || typeof configuration.random.nextUnit !== "function") throw new RuntimeFault("InvalidConfiguration", "random")
    if (typeof configuration.onRequest !== "function") throw new RuntimeFault("InvalidConfiguration", "onRequest")
    if (!Array.isArray(configuration.customControls)) throw new RuntimeFault("InvalidConfiguration", "customControls")

    this.runtimeIdentity = configuration.runtimeIdentity
    this.root = configuration.root
    this.document = configuration.root.ownerDocument
    this.limits = Object.freeze({ ...configuration.limits })
    this.clock = configuration.clock
    this.random = configuration.random
    this.onRequest = configuration.onRequest
    this.viewport = Object.freeze({ ...configuration.viewport })
    this.reducedMotion = configuration.reducedMotion
    this.timeSeconds = configuration.clock.nowSeconds()
    this.scheme = configuration.scheme
    this.localizationRecord = configuration.localization
    this.animationScripts = configuration.animationScripts

    for (const name of VGUI_GENERIC_CONTROL_NAMES) {
      const folded = asciiFold(name)
      if (!this.registrations.has(folded)) this.registrations.set(folded, genericRegistration(name))
    }
    for (const registration of configuration.customControls) {
      if (!validRegistration(registration, this.limits)) throw new RuntimeFault("InvalidConfiguration", `control:${registration?.name ?? ""}`)
      const folded = asciiFold(registration.name)
      if (this.registrations.has(folded)) throw new RuntimeFault("InvalidConfiguration", `control:${registration.name}`)
      this.registrations.set(folded, Object.freeze({
        ...registration,
        animationVariables: Object.freeze([...BASE_ANIMATION_VARIABLES, ...registration.animationVariables]),
        acceptedProperties: Object.freeze([...registration.acceptedProperties]),
      }))
    }

    this.validateScheme(configuration.scheme)
    this.validateLocalization(configuration.localization)
    this.validateAnimationScripts(configuration.animationScripts)
    this.installScheme(configuration.scheme)
    this.installLocalization(configuration.localization)
    this.installAnimationScripts(configuration.animationScripts)
    for (const registration of this.registrations.values()) {
      for (const definition of registration.animationVariables) this.convertAnimationScalar(definition.converter, definition.defaultValue, definition, null)
    }

    if (this.limits.maxDomNodes < 3) throw new RuntimeFault("DomLimit", "root")
    this.style = this.document.createElement("style")
    this.style.dataset.playsrcVgui = "source-runtime"
    this.style.textContent = VGUI_CSS
    this.host = this.document.createElement("div")
    this.host.className = "playsrc-vgui-root playsrc-vgui-runtime"
    this.host.dataset.vguiOwner = "playsrc"
    this.host.dataset.vguiRuntime = this.runtimeIdentity
    this.host.style.width = `${this.viewport.width}px`
    this.host.style.height = `${this.viewport.height}px`
    this.host.style.pointerEvents = "auto"

    try {
      this.root.append(this.style, this.host)
      this.rootPanel = this.createPanelInternal(null, configuration.rootControl.control, configuration.rootControl.name, null).id
      const rootPanel = this.requirePanel(this.rootPanel)
      rootPanel.bounds = { x: 0, y: 0, width: this.viewport.width, height: this.viewport.height }
      rootPanel.mouseInput = true
      rootPanel.keyboardInput = true
      this.solveGeometry()
      this.publishDom()
      this.installBrowserAdapter()
    } catch (error) {
      this.destroyDom()
      if (error instanceof RuntimeFault) throw error
      throw new RuntimeFault("DomFailure", "initialize")
    }
  }

  apply(operation: VguiOperation): VguiOperationResult {
    if (this.destroyed && operation.kind !== "destroy") return this.failure("Destroyed", operation.kind, operation.kind)
    try {
      let panel: VguiPanelId | undefined
      switch (operation.kind) {
        case "create-panel":
          panel = this.createPanel(operation)
          break
        case "delete-panel":
          this.deletePanel(operation.panel, operation.deferred)
          break
        case "reparent-panel":
          this.reparentPanel(operation.panel, operation.parent)
          break
        case "set-bounds":
          this.setBounds(operation.panel, operation.bounds)
          break
        case "set-minimum-size":
          this.setMinimumSize(operation.panel, operation.width, operation.height)
          break
        case "set-panel-state":
          this.setPanelState(operation)
          break
        case "move-to-front":
          this.movePanel(operation.panel, true)
          break
        case "move-to-back":
          this.movePanel(operation.panel, false)
          break
        case "replace-resource":
          this.replaceResource(operation.parent, operation.document, operation.selection)
          break
        case "replace-scheme":
          this.replaceScheme(operation.scheme)
          break
        case "replace-localization":
          this.replaceLocalization(operation.localization)
          break
        case "replace-animation-scripts":
          this.replaceAnimationScripts(operation.scripts)
          break
        case "set-dialog-variable":
          this.setDialogVariable(operation.panel, operation.name, operation.value)
          break
        case "mutate-control":
          this.mutateControl(operation.panel, operation.mutation)
          break
        case "request-focus":
          this.requestFocus(operation.panel)
          break
        case "set-default-button":
          this.setDefaultButton(operation.group, operation.panel)
          break
        case "set-pointer-capture":
          this.setPointerCapture(operation.panel, operation.initiatingButton, operation.pointerId)
          break
        case "set-application-modal":
          this.setApplicationModal(operation.panel)
          break
        case "set-modal-subtree":
          this.setModalSubtree(operation.panel, operation.restrictToSubtree, operation.outsideClickListener)
          break
        case "pointer-move":
          this.pointerMove(operation.x, operation.y, operation.pointerId)
          break
        case "pointer-press":
          this.pointerPress(operation.button, operation.x, operation.y, operation.pointerId, operation.clicks)
          break
        case "pointer-release":
          this.pointerRelease(operation.button, operation.x, operation.y, operation.pointerId)
          break
        case "pointer-wheel":
          this.pointerWheel(operation.delta, operation.x, operation.y)
          break
        case "key-press":
          this.keyPress(operation.key, operation.shift, operation.control, operation.alt, operation.meta, operation.repeat)
          break
        case "key-typed":
          this.keyTyped(operation.key, operation.shift, operation.control, operation.alt, operation.meta)
          break
        case "key-release":
          this.keyRelease(operation.key, operation.shift, operation.control, operation.alt, operation.meta)
          break
        case "text-input":
          this.textInput(operation.text)
          break
        case "composition-start":
          this.compositionStart()
          break
        case "composition-update":
          this.compositionUpdate(operation.text, operation.caret)
          break
        case "composition-end":
          this.compositionEnd(operation.text)
          break
        case "clipboard-result":
          this.clipboardResult(operation.requestId, operation.result, operation.text)
          break
        case "post-message":
          this.postMessage(operation.target, operation.source, operation.message, operation.delaySeconds)
          break
        case "start-animation-sequence":
          this.startAnimationSequence(operation.parent, operation.sequence, operation.cancelable)
          break
        case "stop-animation-sequence":
          this.stopAnimationSequence(operation.parent, operation.sequence)
          break
        case "set-viewport":
          this.setViewport(operation.viewport)
          break
        case "set-reduced-motion":
          this.reducedMotion = operation.reduced
          this.host.dataset.reducedMotion = operation.reduced ? "true" : "false"
          break
        case "frame":
          this.runFrame(operation.timeSeconds)
          break
        case "destroy":
          this.destroy()
          break
      }
      this.revision += 1
      return Object.freeze({ ok: true, revision: this.revision, ...(panel === undefined ? {} : { panel }) })
    } catch (error) {
      const fault = error instanceof RuntimeFault ? error : new RuntimeFault("InvalidOperation", operation.kind)
      return this.failure(fault.code, operation.kind, fault.subject)
    }
  }

  snapshot(): VguiRuntimeSnapshot {
    const panels = [...this.panels.values()]
      .sort((left, right) => left.id - right.id)
      .map((panel): VguiPanelSnapshot => Object.freeze({
        id: panel.id,
        control: panel.control,
        name: panel.name,
        parent: panel.parent,
        children: Object.freeze([...panel.children]),
        resourceOwner: panel.resourceOwner,
        bounds: Object.freeze({ ...panel.bounds }),
        absoluteBounds: Object.freeze({ ...panel.absoluteBounds }),
        clip: Object.freeze({ ...panel.clip }),
        inset: Object.freeze({ ...panel.inset }),
        minimumSize: Object.freeze([panel.minimumWidth, panel.minimumHeight]) as readonly [number, number],
        z: panel.z,
        popup: panel.popup,
        topmostPopup: panel.topmostPopup,
        visible: panel.visible,
        effectivelyVisible: panel.effectivelyVisible,
        enabled: panel.enabled,
        mouseInput: panel.mouseInput,
        keyboardInput: panel.keyboardInput,
        proportional: panel.proportional,
        tabPosition: panel.tabPosition,
        text: panel.text,
        accessibleName: panel.accessibleName,
        accessibleDescription: panel.accessibleDescription,
        role: panel.registration.role,
        state: Object.freeze({
          armed: panel.armed,
          depressed: panel.depressed,
          selected: panel.selected,
          checked: panel.checked,
          value: panel.value,
          minimum: panel.minimum,
          maximum: panel.maximum,
          rangeWindow: panel.rangeWindow,
          progress: panel.progress,
          activeIndex: panel.activeIndex,
          caret: panel.caret,
          selection: Object.freeze([panel.selectionStart, panel.selectionEnd]) as readonly [number, number],
          editable: panel.editable,
          multiline: panel.multiline,
          numericOnly: panel.numericOnly,
          textHidden: panel.textHidden,
          maximumCharacters: panel.maximumCharacters,
          frameInteraction: panel.frameInteraction,
          bodyText: panel.bodyText,
          items: cloneItems(panel.items),
          composition: Object.freeze({ active: panel.compositionActive, text: panel.compositionText, caret: panel.compositionCaret }),
        }),
        animationVariables: Object.freeze(Object.fromEntries(panel.animationValues)),
      }))

    return Object.freeze({
      runtimeIdentity: this.runtimeIdentity,
      lifecycle: this.destroyed ? "destroyed" : "live",
      revision: this.revision,
      frame: this.frame,
      timeSeconds: this.timeSeconds,
      viewport: Object.freeze({ ...this.viewport }),
      reducedMotion: this.reducedMotion,
      rootPanel: this.rootPanel,
      panels: Object.freeze(panels),
      popups: Object.freeze([...this.popups]),
      input: Object.freeze({
        pointer: Object.freeze([this.pointerX, this.pointerY]) as readonly [number, number],
        mouseOver: this.mouseOver,
        mouseFocus: this.mouseFocus,
        capture: this.capture,
        captureButton: this.captureButton,
        keyFocus: this.keyFocus,
        calculatedKeyFocus: this.calculatedKeyFocus,
        applicationModal: this.applicationModal,
        modalSubtree: this.modalSubtree,
        modalRestriction: this.modalSubtree === null ? null : this.modalRestrict ? "inclusive" : "exclusive",
        pressedKeys: Object.freeze([...this.pressedKeys]),
        downKeys: Object.freeze([...this.downKeys]),
        releasedKeys: Object.freeze([...this.releasedKeys]),
        pressedButtons: Object.freeze([...this.pressedButtons]),
        downButtons: Object.freeze([...this.downButtons]),
        releasedButtons: Object.freeze([...this.releasedButtons]),
      }),
      schemeIdentity: this.scheme.identity,
      localizationIdentity: this.localizationRecord.identity,
      animationScriptIdentity: this.animationScripts.identity,
      activeAnimations: this.activeAnimations.length,
      delayedCommands: this.delayedCommands.length,
      queuedMessages: this.queuedMessages.length,
      diagnostics: Object.freeze(this.diagnostics.map((diagnostic) => Object.freeze({ ...diagnostic }))),
      trace: Object.freeze(this.trace.map((entry) => Object.freeze({ ...entry }))),
      ownedResources: Object.freeze({
        nodes: this.destroyed ? 0 : this.panels.size + this.auxiliaryNodes.size + 2,
        listeners: this.listeners.length,
        observers: 0 as const,
        timers: 0 as const,
      }),
    })
  }

  private failure(code: VguiDiagnosticCode, operation: VguiDiagnostic["operation"], subject: string): VguiOperationResult {
    const diagnostic = this.recordDiagnostic(code, operation, subject)
    return Object.freeze({ ok: false, diagnostic })
  }

  private recordDiagnostic(code: VguiDiagnosticCode, operation: VguiDiagnostic["operation"], subject: string): VguiDiagnostic {
    const diagnostic = Object.freeze({ sequence: this.nextDiagnostic++, code, operation, subject })
    if (this.diagnostics.length < this.limits.maxDiagnostics) this.diagnostics.push(diagnostic)
    return diagnostic
  }

  private addTrace(phase: string, panel: VguiPanelId | null, detail: string): void {
    if (this.trace.length >= this.limits.maxDiagnostics) this.trace.shift()
    this.trace.push({ sequence: this.nextTrace++, phase, panel, detail })
  }

  private validateScheme(scheme: VguiScheme): void {
    if (!scheme || !IDENTITY.test(scheme.identity) || !validString(scheme.revision, 128, false) || !validString(scheme.tag, 64, false)) {
      throw new RuntimeFault("MalformedScheme", "identity")
    }
    if (!Array.isArray(scheme.colors) || scheme.colors.length > this.limits.maxSchemeColors) throw new RuntimeFault("RegistryLimit", "scheme.colors")
    if (!Array.isArray(scheme.settings) || scheme.settings.length > this.limits.maxSchemeSettings) throw new RuntimeFault("RegistryLimit", "scheme.settings")
    if (!Array.isArray(scheme.fonts) || scheme.fonts.length > this.limits.maxPropertiesPerPanel) throw new RuntimeFault("RegistryLimit", "scheme.fonts")
    if (!Array.isArray(scheme.borders) || scheme.borders.length > this.limits.maxSchemeBorders) throw new RuntimeFault("RegistryLimit", "scheme.borders")
    if (!Array.isArray(scheme.images) || scheme.images.length > this.limits.maxSchemeImages) throw new RuntimeFault("RegistryLimit", "scheme.images")

    const unique = (values: readonly Readonly<{ name: string }>[], subject: string): void => {
      const names = new Set<string>()
      for (const value of values) {
        if (!value || !validString(value.name, 255, false)) throw new RuntimeFault("MalformedScheme", subject)
        const folded = asciiFold(value.name)
        if (names.has(folded)) throw new RuntimeFault("MalformedScheme", `${subject}:${value.name}`)
        names.add(folded)
      }
    }
    unique(scheme.colors, "colors")
    unique(scheme.settings, "settings")
    unique(scheme.fonts, "fonts")
    unique(scheme.borders, "borders")
    unique(scheme.images, "images")

    for (const entry of [...scheme.colors, ...scheme.settings]) {
      if (!validString(entry.value, this.limits.maxStringCodeUnits)) throw new RuntimeFault("MalformedScheme", entry.name)
    }
    for (const font of scheme.fonts) {
      if (!validString(font.cssFamily, this.limits.maxStringCodeUnits, false)
        || !safeInteger(font.sizePx) || font.sizePx < 0 || font.sizePx > 32767
        || !safeInteger(font.lineHeightPx) || font.lineHeightPx < 0 || font.lineHeightPx > 32767
        || !safeInteger(font.weight) || font.weight < 0 || font.weight > 1000
        || !["normal", "italic"].includes(font.style)
        || typeof font.available !== "boolean"
        || (font.measure !== undefined && typeof font.measure !== "function")) throw new RuntimeFault("MalformedScheme", `font:${font.name}`)
    }
    for (const border of scheme.borders) this.validateBorder(border)
    for (const image of scheme.images) this.validateImage(image)

    const colorValues = new Map(scheme.colors.map((entry) => [asciiFold(entry.name), entry.value]))
    const settingValues = new Map(scheme.settings.map((entry) => [asciiFold(entry.name), entry.value]))
    for (const entry of scheme.colors) this.resolveColorFromMaps(entry.name, colorValues, settingValues, new Set())
    for (const lookup of GENERIC_SCHEME_COLOR_LOOKUPS) {
      if (colorValues.has(asciiFold(lookup)) || settingValues.has(asciiFold(lookup))) this.resolveColorFromMaps(lookup, colorValues, settingValues, new Set())
    }
    for (const border of scheme.borders) {
      if ((border.kind === "image" || border.kind === "scalable-image")
        && !scheme.images.some((image) => sameName(image.name, border.image))) {
        throw new RuntimeFault("MissingReference", `border:${border.name}:${border.image}`)
      }
      if (border.kind === "scalable-image" && border.color.some((channel: number) => channel !== 255)) {
        const image = scheme.images.find((candidate) => sameName(candidate.name, border.image))
        if (!image?.variants?.some((variant: NonNullable<VguiImagePresentation["variants"]>[number]) => variant.frame === 0 && variant.rotation === 0 && variant.tint.every((channel: number, index: number) => channel === border.color[index]))) {
          throw new RuntimeFault("MissingReference", `border:${border.name}:image-variant`)
        }
      }
    }
  }

  private validateBorder(border: VguiBorder): void {
    if (!border || !validString(border.name, 255, false)
      || ![0, 1, 2].includes(border.backgroundType)
      || typeof border.paintFirst !== "boolean"
      || !this.validInsets(border.inset)) throw new RuntimeFault("MalformedScheme", `border:${border?.name ?? ""}`)
    if (border.kind === "line") {
      for (const side of [border.sides.left, border.sides.top, border.sides.right, border.sides.bottom]) {
        if (!Array.isArray(side) || side.length > this.limits.maxPropertiesPerPanel) throw new RuntimeFault("RegistryLimit", `border:${border.name}`)
        for (const line of side) {
          if (!this.validColor(line.color) || !safeInteger(line.startOffset) || !safeInteger(line.endOffset)) {
            throw new RuntimeFault("MalformedScheme", `border:${border.name}`)
          }
        }
      }
    } else if (border.kind === "image") {
      if (!validString(border.image, this.limits.maxStringCodeUnits, false) || typeof border.tiled !== "boolean") {
        throw new RuntimeFault("MalformedScheme", `border:${border.name}`)
      }
    } else if (border.kind === "scalable-image") {
      if (!validString(border.image, this.limits.maxStringCodeUnits, false)
        || ![border.sourceCornerWidth, border.sourceCornerHeight, border.drawCornerWidth, border.drawCornerHeight].every((value) => safeInteger(value) && value >= 0 && value <= 32767)
        || !this.validColor(border.color)) throw new RuntimeFault("MalformedScheme", `border:${border.name}`)
    } else {
      throw new RuntimeFault("MalformedScheme", `border:${(border as { name?: string }).name ?? ""}`)
    }
  }

  private validateImage(image: VguiImagePresentation): void {
    if (!image || !validString(image.name, 255, false) || !IDENTITY.test(image.logicalIdentity)
      || !validString(image.revision, 128, false) || !validString(image.browserUrl, this.limits.maxStringCodeUnits, false)
      || !safeInteger(image.width) || image.width <= 0 || image.width > 32767
      || !safeInteger(image.height) || image.height <= 0 || image.height > 32767
      || !safeInteger(image.frames) || image.frames <= 0 || image.frames > 32767
      || typeof image.hardwareFiltered !== "boolean") throw new RuntimeFault("MalformedScheme", `image:${image?.name ?? ""}`)
    if (image.variants !== undefined) {
      if (!Array.isArray(image.variants) || image.variants.length > this.limits.maxPropertiesPerPanel) throw new RuntimeFault("RegistryLimit", `image:${image.name}:variants`)
      const keys = new Set<string>()
      for (const variant of image.variants) {
        if (!variant || !safeInteger(variant.frame) || variant.frame < 0 || variant.frame >= image.frames || !this.validColor(variant.tint)
          || ![0, 1, 2, 3].includes(variant.rotation)
          || !validString(variant.browserUrl, this.limits.maxStringCodeUnits, false)) throw new RuntimeFault("MalformedScheme", `image:${image.name}:variant`)
        const key = `${variant.frame}:${variant.tint.join(",")}:${variant.rotation}`
        if (keys.has(key)) throw new RuntimeFault("MalformedScheme", `image:${image.name}:variant:${key}`)
        keys.add(key)
      }
    }
    for (let frame = 1; frame < image.frames; frame += 1) {
      if (!image.variants?.some((variant: NonNullable<VguiImagePresentation["variants"]>[number]) => variant.frame === frame && variant.rotation === 0 && variant.tint.every((channel: number) => channel === 255))) {
        throw new RuntimeFault("MissingReference", `image:${image.name}:frame:${frame}`)
      }
    }
  }

  private validInsets(inset: VguiBorder["inset"]): boolean {
    return !!inset && [inset.left, inset.top, inset.right, inset.bottom].every((value) => safeInteger(value) && value >= -32767 && value <= 32767)
  }

  private validColor(color: Rgba): boolean {
    return Array.isArray(color) && color.length === 4 && color.every((channel) => safeInteger(channel) && channel >= 0 && channel <= 255)
  }

  private validateLocalization(localization: VguiLocalization): void {
    if (!localization || !IDENTITY.test(localization.identity) || !validString(localization.revision, 128, false)
      || !/^[a-z][a-z0-9_-]{0,63}$/u.test(localization.language)
      || !Array.isArray(localization.tokens)) throw new RuntimeFault("MalformedLocalization", "localization")
    if (localization.tokens.length > this.limits.maxLocalizationTokens) throw new RuntimeFault("RegistryLimit", "localization.tokens")
    for (const token of localization.tokens) {
      if (!token || !validString(token.name, 255, false) || !validString(token.value, this.limits.maxTextCodeUnits)) {
        throw new RuntimeFault("MalformedLocalization", token?.name ?? "token")
      }
    }
  }

  private validateAnimationScripts(scripts: VguiAnimationScriptSet): void {
    if (!scripts || !IDENTITY.test(scripts.identity) || !validString(scripts.revision, 128, false)
      || !Array.isArray(scripts.scripts) || !Array.isArray(scripts.activeConditions)) {
      throw new RuntimeFault("MalformedAnimation", "scripts")
    }
    if (scripts.scripts.length > this.limits.maxAnimationScripts) throw new RuntimeFault("AnimationLimit", "scripts")
    let sequenceCount = 0
    let commandCount = 0
    for (const script of scripts.scripts) {
      if (!script || !IDENTITY.test(script.logicalIdentity) || !validString(script.revision, 128, false) || !Array.isArray(script.sequences)) {
        throw new RuntimeFault("MalformedAnimation", script?.logicalIdentity ?? "script")
      }
      for (const sequence of script.sequences) {
        sequenceCount += 1
        if (sequenceCount > this.limits.maxAnimationSequences) throw new RuntimeFault("AnimationLimit", "sequences")
        if (!sequence || !validString(sequence.name, 255, false) || !Array.isArray(sequence.commands)
          || (sequence.condition !== null && !validString(sequence.condition, 255, false))) {
          throw new RuntimeFault("MalformedAnimation", sequence?.name ?? "sequence")
        }
        for (const command of sequence.commands) {
          commandCount += 1
          if (commandCount > this.limits.maxAnimationCommands) throw new RuntimeFault("AnimationLimit", "commands")
          this.validateAnimationCommand(command, sequence.name)
        }
      }
    }
  }

  private validateAnimationCommand(command: VguiAnimationCommand, sequence: string): void {
    if (!command || !validString(command.kind, 32, false) || (command.condition !== null && !validString(command.condition, 255, false))) {
      throw new RuntimeFault("MalformedAnimation", `${sequence}:command`)
    }
    const delay = command.delaySeconds
    if (!finite(delay) || delay < 0) throw new RuntimeFault("MalformedAnimation", `${sequence}:${command.kind}:delay`)
    if (command.kind === "Animate") {
      if (!validString(command.panel, 255, false) || !validString(command.variable, 255, false)
        || !validString(command.target, this.limits.maxStringCodeUnits, false)
        || !validString(command.interpolator, 32, false) || !finite(command.parameter)
        || !finite(command.durationSeconds) || command.durationSeconds < 0) {
        throw new RuntimeFault("MalformedAnimation", `${sequence}:Animate`)
      }
    }
  }

  private installScheme(scheme: VguiScheme): void {
    this.scheme = scheme
    this.colors.clear()
    this.settings.clear()
    this.fonts.clear()
    this.borders.clear()
    this.images.clear()
    for (const entry of scheme.colors) this.colors.set(asciiFold(entry.name), entry.value)
    for (const entry of scheme.settings) this.settings.set(asciiFold(entry.name), entry.value)
    for (const font of scheme.fonts) this.fonts.set(asciiFold(font.name), Object.freeze({ ...font }))
    for (const border of scheme.borders) this.borders.set(asciiFold(border.name), border)
    for (const image of scheme.images) this.images.set(asciiFold(image.name), Object.freeze({ ...image }))
  }

  private installLocalization(localization: VguiLocalization): void {
    this.localizationRecord = localization
    this.localization.clear()
    for (const token of localization.tokens) this.localization.set(asciiFold(token.name.replace(/^#/u, "")), token.value)
  }

  private installAnimationScripts(scripts: VguiAnimationScriptSet): void {
    this.animationScripts = scripts
    this.sequences.clear()
    const active = new Set(scripts.activeConditions.map(asciiFold))
    for (const script of scripts.scripts) {
      for (const sequence of script.sequences) {
        if (!conditionActive(sequence.condition, active)) continue
        const folded = asciiFold(sequence.name)
        if (this.sequences.has(folded)) continue
        const commands = sequence.commands.filter((command) => conditionActive(command.condition, active))
        let duration = 0
        for (const command of commands) {
          if (command.kind === "Animate") duration = Math.max(duration, command.delaySeconds + command.durationSeconds)
        }
        this.sequences.set(folded, { name: sequence.name, commands: Object.freeze([...commands]), duration })
      }
    }
  }

  private resolveColorFromMaps(
    name: string,
    colors: ReadonlyMap<string, string>,
    settings: ReadonlyMap<string, string>,
    seen: Set<string>,
  ): Rgba {
    const literal = parseColorLiteral(name, 0)
    if (literal) return literal
    const folded = asciiFold(name)
    if (seen.has(folded)) throw new RuntimeFault("MalformedScheme", `color-cycle:${name}`)
    seen.add(folded)
    const colorValue = colors.get(folded)
    if (colorValue !== undefined) {
      const result = parseColorLiteral(colorValue, 0)
      if (!result) throw new RuntimeFault("MalformedScheme", `color:${name}`)
      return result
    }
    const settingValue = settings.get(folded)
    if (settingValue !== undefined) return this.resolveColorFromMaps(settingValue, colors, settings, seen)
    throw new RuntimeFault("MissingSchemeValue", `color:${name}`)
  }

  private resolveColor(name: string, fallback: Rgba): Rgba {
    try {
      return this.resolveColorFromMaps(name, this.colors, this.settings, new Set())
    } catch (error) {
      if (error instanceof RuntimeFault && error.code === "MissingSchemeValue") return fallback
      throw error
    }
  }

  private replaceScheme(scheme: VguiScheme): void {
    this.validateScheme(scheme)
    for (const panel of this.panels.values()) {
      if (panel.font && !scheme.fonts.some((font) => sameName(font.name, panel.font!))) throw new RuntimeFault("MissingReference", `${panel.name}:font:${panel.font}`)
      if (panel.border && !scheme.borders.some((border) => sameName(border.name, panel.border!))) throw new RuntimeFault("MissingReference", `${panel.name}:border:${panel.border}`)
      if (panel.image && !scheme.images.some((image) => sameName(image.name, panel.image!))) throw new RuntimeFault("MissingReference", `${panel.name}:image:${panel.image}`)
    }
    this.installScheme(scheme)
    for (const panel of this.panels.values()) { this.updateAutoSize(panel); this.reapplyPanelPresentation(panel) }
    this.solveGeometry()
    this.publishDom()
  }

  private replaceLocalization(localization: VguiLocalization): void {
    this.validateLocalization(localization)
    this.installLocalization(localization)
    for (const panel of this.panels.values()) { this.refreshText(panel); this.refreshBodyText(panel); this.updateAutoSize(panel) }
    this.solveGeometry()
    this.publishDom()
  }

  private replaceAnimationScripts(scripts: VguiAnimationScriptSet): void {
    this.validateAnimationScripts(scripts)
    this.finishCancelableAnimations()
    this.activeAnimations.splice(0)
    this.delayedCommands.splice(0)
    this.installAnimationScripts(scripts)
  }

  private createPanel(operation: Extract<VguiOperation, { kind: "create-panel" }>): VguiPanelId {
    const parent = this.requirePanel(operation.parent)
    if (!NAME.test(operation.name)) throw new RuntimeFault("MalformedValue", `name:${operation.name}`)
    const properties = operation.properties ?? []
    if (!Array.isArray(properties) || properties.length > this.limits.maxPropertiesPerPanel) throw new RuntimeFault("ResourceLimit", operation.name)
    const registration = this.registration(operation.control)
    for (const property of properties) {
      if (!property || !validString(property.name, 255, false) || !validString(property.value, this.limits.maxStringCodeUnits)) {
        throw new RuntimeFault("MalformedValue", `${operation.name}:property`)
      }
      if (!propertyAllowed(operation.control, registration, property.name)) throw new RuntimeFault("UnknownProperty", `${operation.control}.${property.name}`)
    }
    const panel = this.createPanelInternal(parent.id, operation.control, operation.name, null)
    try {
      this.applyPanelProperties(panel, properties)
      this.solveGeometry()
      this.publishDom()
      return panel.id
    } catch (error) {
      this.deletePanelImmediate(panel.id)
      throw error
    }
  }

  private createPanelInternal(
    parentId: VguiPanelId | null,
    control: VguiControlName,
    name: string,
    resourceOwner: string | null,
  ): PanelState {
    if (this.panels.size >= this.limits.maxPanels) throw new RuntimeFault("PanelLimit", name)
    const registration = this.registration(control)
    const sourceIdentity = asSourceControl(control)
    const reservedAuxiliary = ["MessageBox", "QueryBox"].some((value) => sameName(sourceIdentity, value)) ? 6
      : sameName(sourceIdentity, "Frame") ? 3 : 0
    if (this.panels.size + this.auxiliaryNodes.size + reservedAuxiliary + 3 > this.limits.maxDomNodes) throw new RuntimeFault("DomLimit", name)
    if (parentId !== null) {
      const parent = this.requirePanel(parentId)
      if (parent.children.length >= this.limits.maxChildrenPerPanel) throw new RuntimeFault("ChildLimit", parent.name)
      if (this.depth(parent.id) + 1 > this.limits.maxHierarchyDepth) throw new RuntimeFault("HierarchyLimit", name)
    }
    const element = this.document.createElement(registration.element)
    element.className = "playsrc-vgui-control playsrc-vgui-source-control"
    element.dataset.vguiControl = control
    element.dataset.vguiName = name
    element.dataset.vguiPanel = String(this.nextPanelId)
    element.id = `${this.runtimeIdentity}-panel-${this.nextPanelId}`
    element.style.position = "absolute"
    element.style.boxSizing = "border-box"
    element.style.margin = "0"
    element.style.padding = "0"
    element.style.borderRadius = "0"
    element.style.overflow = "hidden"
    if (registration.role) element.setAttribute("role", registration.role)

    const id = this.nextPanelId++
    const animationDefinitions = new Map<string, VguiAnimationVariable>()
    const animationValues = new Map<string, string | number | boolean | Rgba>()
    for (const definition of registration.animationVariables) {
      const folded = asciiFold(definition.name)
      if (animationDefinitions.has(folded)) continue
      animationDefinitions.set(folded, definition)
      animationValues.set(definition.name, this.convertAnimationScalar(definition.converter, definition.defaultValue, definition, null))
    }
    const sourceControl = sourceIdentity
    const isFrame = ["Frame", "MessageBox", "QueryBox"].some((value) => sameName(sourceControl, value))
    const isEditableText = ["TextEntry", "ComboBox"].some((value) => sameName(sourceControl, value))
    const isRadio = sameName(sourceControl, "RadioButton")
    const hasText = ["Label", "Button", "CheckButton", "RadioButton", "TextEntry", "RichText", "Frame", "Menu", "MenuItem", "ComboBox", "PropertySheet", "PropertyPage", "ListPanel", "MessageBox", "QueryBox", "URLLabel"].some((value) => sameName(sourceControl, value))
    const defaultWidth = isFrame ? 128 : sameName(sourceControl, "ScrollBar") && sameName(control, "ScrollBar_Vertical") ? 19 : 64
    const defaultHeight = isFrame ? 66 : sameName(sourceControl, "ScrollBar") ? sameName(control, "ScrollBar_Vertical") ? 64 : 19 : 24
    const panel: PanelState = {
      id,
      control,
      registration,
      name,
      parent: parentId,
      children: [],
      tieOrder: this.nextOrder++,
      resourceOwner,
      properties: new Map(),
      bounds: { x: 0, y: 0, width: defaultWidth, height: defaultHeight },
      absoluteBounds: { x: 0, y: 0, width: defaultWidth, height: defaultHeight },
      clip: { x: 0, y: 0, width: defaultWidth, height: defaultHeight },
      inset: { ...EMPTY_INSETS },
      minimumWidth: isFrame ? 128 : 0,
      minimumHeight: isFrame ? 66 : 0,
      autoResize: 0,
      pinCorner: 0,
      pinOffsetX: 0,
      pinOffsetY: 0,
      resizeOffsetX: 0,
      resizeOffsetY: 0,
      z: 0,
      popup: isFrame,
      topmostPopup: false,
      visible: true,
      effectivelyVisible: true,
      enabled: true,
      mouseInput: true,
      keyboardInput: true,
      proportional: false,
      tabPosition: 0,
      subTabPosition: 0,
      nav: new Map(),
      actionTargets: parentId === null || (resourceOwner === null && !this.editableParent(parentId)) ? [] : [parentId],
      dialogVariables: new Map(),
      defaultButton: null,
      currentDefaultButton: null,
      textSource: "",
      text: "",
      bodyTextSource: "",
      bodyText: "",
      accessibleName: name,
      accessibleDescription: "",
      tooltip: "",
      command: null,
      url: null,
      border: null,
      font: hasText && this.fonts.has("default") ? this.fonts.get("default")!.name : null,
      image: null,
      drawColor: WHITE,
      fillColor: TRANSPARENT,
      armed: false,
      depressed: false,
      selected: false,
      checked: false,
      checkable: sameName(sourceControl, "CheckButton") || isRadio,
      staySelected: false,
      stayArmed: false,
      activation: isRadio || sameName(sourceControl, "CheckButton") ? 0 : sameName(sourceControl, "MenuItem") ? 1 : 2,
      value: 0,
      minimum: 0,
      maximum: 0,
      rangeWindow: 0,
      numTicks: 10,
      thumbWidth: 8,
      dragging: false,
      dragStartValue: 0,
      dragStartCoordinate: 0,
      frameInteraction: null,
      frameStartBounds: { x: 0, y: 0, width: 0, height: 0 },
      frameStartPointer: Object.freeze([0, 0]) as readonly [number, number],
      frameClosePressed: false,
      progress: 0,
      progressVariable: null,
      items: [],
      itemElements: new Map(),
      chromeElements: new Map(),
      pressedItem: null,
      activeIndex: null,
      caret: 0,
      selectionStart: -1,
      selectionEnd: 0,
      editable: isEditableText,
      multiline: sameName(sourceControl, "RichText"),
      numericOnly: false,
      allowUnicode: sameName(sourceControl, "RichText"),
      textHidden: false,
      maximumCharacters: -1,
      selectAllOnFirstFocus: false,
      firstFocus: true,
      compositionActive: false,
      compositionText: "",
      compositionCaret: 0,
      animationDefinitions,
      animationValues,
      element,
    }
    this.panels.set(id, panel)
    if (parentId !== null) {
      const parent = this.requirePanel(parentId)
      parent.children.push(id)
      this.sortChildren(parent)
    }
    if (panel.popup) this.addPopup(panel)
    this.addTrace("panel-create", id, `${control}:${name}`)
    return panel
  }

  private registration(control: VguiControlName): VguiControlRegistration {
    const registration = this.registrations.get(asciiFold(control))
    if (!registration) throw new RuntimeFault("UnknownControl", control)
    return registration
  }

  private editableParent(panelId: VguiPanelId): boolean {
    const control = asSourceControl(this.requirePanel(panelId).control)
    return ["EditablePanel", "Frame", "PropertySheet", "PropertyPage", "MessageBox", "QueryBox"].some((name) => sameName(control, name))
  }

  private requirePanel(id: VguiPanelId): PanelState {
    if (!safeInteger(id) || id <= 0) throw new RuntimeFault("InvalidPanel", String(id))
    const panel = this.panels.get(id)
    if (!panel) throw new RuntimeFault("InvalidPanel", String(id))
    return panel
  }

  private depth(id: VguiPanelId): number {
    let depth = 1
    let panel = this.requirePanel(id)
    const seen = new Set<VguiPanelId>([id])
    while (panel.parent !== null) {
      if (seen.has(panel.parent)) throw new RuntimeFault("HierarchyCycle", panel.name)
      seen.add(panel.parent)
      panel = this.requirePanel(panel.parent)
      depth += 1
    }
    return depth
  }

  private sortChildren(parent: PanelState): void {
    parent.children.sort((leftId, rightId) => {
      const left = this.requirePanel(leftId)
      const right = this.requirePanel(rightId)
      return left.z - right.z || left.tieOrder - right.tieOrder
    })
  }

  private convertAnimationScalar(
    converter: VguiAnimationConverter,
    value: string,
    definition: VguiAnimationVariable,
    panel: PanelState | null,
  ): string | number | boolean | Rgba {
    if (converter === "float") return parseFloatValue(value, definition.name)
    if (converter === "int") return parseInteger(value, definition.name)
    if (converter === "bool") return parseBoolean(value, definition.name)
    if (converter === "Color") return this.resolveColor(value, TRANSPARENT)
    if (converter === "char" || converter === "string") {
      const maximum = definition.maximumCodeUnits ?? this.limits.maxStringCodeUnits
      if (!validString(value, maximum)) throw new RuntimeFault("TextLimit", definition.name)
      return value
    }
    if (converter === "HFont" || converter === "vgui::HFont") {
      if (!this.fonts.has(asciiFold(value))) throw new RuntimeFault("MissingReference", `font:${value}`)
      return value
    }
    if (converter === "textureid") {
      return value
    }
    if (converter === "proportional_float") return this.proportional(parseFloatValue(value, definition.name), panel)
    if (converter === "proportional_int") return this.proportional(parseInteger(value, definition.name), panel)
    if (converter === "proportional_xpos") return this.computePosition(value, panel, true)
    if (converter === "proportional_ypos") return this.computePosition(value, panel, false)
    if (converter === "proportional_width") return this.computeDimension(value, panel, true)
    if (converter === "proportional_height") return this.computeDimension(value, panel, false)
    throw new RuntimeFault("MalformedAnimation", `converter:${converter}`)
  }

  private proportional(value: number, panel: PanelState | null): number {
    const height = panel?.parent === null || panel?.parent === undefined
      ? this.viewport.height
      : this.requirePanel(panel.parent).bounds.height || this.viewport.height
    return Math.trunc(value * height / 480)
  }

  private computeDimension(value: string, panel: PanelState | null, horizontal: boolean, computingOther = false, resourceSemantics = false): number {
    if (!panel) return this.proportional(parseFloatValue(value, "dimension"), null)
    const parent = panel.parent === null ? null : this.requirePanel(panel.parent)
    const useParent = resourceSemantics
      ? panel.properties.get("proportionalToParent") === "1"
      : panel.proportional && parent !== null
    const parentWidth = useParent ? parent?.bounds.width ?? this.viewport.width : this.viewport.width
    const parentHeight = useParent ? parent?.bounds.height ?? this.viewport.height : this.viewport.height
    const parentSize = horizontal ? parentWidth : parentHeight
    const current = horizontal ? panel.bounds.width : panel.bounds.height
    if (!validString(value, this.limits.maxStringCodeUnits, false)) throw new RuntimeFault("MalformedValue", `${panel.name}:dimension`)
    let text = value
    let mode: "normal" | "full" | "other" | "parent" | "self" = "normal"
    const prefix = text[0]?.toLowerCase()
    if (prefix === "f") mode = "full"
    else if (prefix === "o") mode = "other"
    else if (prefix === "p") mode = "parent"
    else if (prefix === "s") mode = "self"
    if (mode !== "normal") text = text.slice(1)
    const amount = parseFloatValue(text || "0", `${panel.name}:dimension`)
    const integer = Math.trunc(amount)
    if (mode === "other") {
      if (computingOther) throw new RuntimeFault("MalformedValue", `${panel.name}:recursive-dimension`)
      const other = this.computeDimension(
        panel.properties.get(horizontal ? "tall" : "wide") ?? "0",
        panel,
        !horizontal,
        true,
        resourceSemantics,
      )
      return Math.trunc(other * amount)
    }
    if (mode === "parent") {
      const margin = panel.proportional ? this.proportional(integer, panel) : integer
      return Math.trunc((parentSize - margin) * amount)
    }
    if (mode === "self") return Math.trunc(current * amount)
    let result = integer
    if (panel.proportional) result = this.proportional(result, panel)
    if (mode === "full") result = parentSize - result
    return result
  }

  private computePosition(value: string, panel: PanelState | null, horizontal: boolean, resourceSemantics = false): number {
    if (!panel) return this.proportional(parseFloatValue(value, "position"), null)
    const parent = panel.parent === null ? null : this.requirePanel(panel.parent)
    const useParent = resourceSemantics
      ? panel.properties.get("proportionalToParent") === "1"
      : panel.proportional && parent !== null
    const parentSize = horizontal
      ? useParent ? parent?.bounds.width ?? this.viewport.width : this.viewport.width
      : useParent ? parent?.bounds.height ?? this.viewport.height : this.viewport.height
    const ownSize = horizontal ? panel.bounds.width : panel.bounds.height
    let cursor = 0
    const parseTerm = (): number => {
      let alignment: "none" | "far" | "center" = "none"
      let proportion: "none" | "self" | "parent" = "none"
      const alignPrefix = value[cursor]?.toLowerCase()
      if (alignPrefix === "r") { alignment = "far"; cursor += 1 }
      else if (alignPrefix === "c") { alignment = "center"; cursor += 1 }
      const proportionPrefix = value[cursor]?.toLowerCase()
      if (proportionPrefix === "s") { proportion = "self"; cursor += 1 }
      else if (proportionPrefix === "p") { proportion = "parent"; cursor += 1 }
      const match = value.slice(cursor).match(/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)/u)
      if (!match) throw new RuntimeFault("MalformedValue", `${panel.name}:position:${value}`)
      cursor += match[0].length
      const amount = Number(match[0])
      let delta: number
      if (proportion === "self") delta = Math.trunc(ownSize * amount)
      else if (proportion === "parent") delta = Math.trunc(parentSize * amount)
      else delta = panel.proportional ? this.proportional(Math.trunc(amount), panel) : Math.trunc(amount)
      if (alignment === "far") return parentSize - delta
      if (alignment === "center") return Math.trunc(parentSize / 2) + delta
      return delta
    }
    let result = parseTerm()
    while (cursor < value.length) {
      const operation = value[cursor]
      if (operation !== "+" && operation !== "-") throw new RuntimeFault("MalformedValue", `${panel.name}:position:${value}`)
      cursor += 1
      const term = parseTerm()
      result = operation === "+" ? result + term : result - term
    }
    return result
  }

  private setBounds(panelId: VguiPanelId, bounds: VguiRect): void {
    const panel = this.requirePanel(panelId)
    if (!bounds || ![bounds.x, bounds.y, bounds.width, bounds.height].every(safeInteger)) throw new RuntimeFault("MalformedValue", panel.name)
    const changedSize = panel.bounds.width !== bounds.width || panel.bounds.height !== bounds.height
    panel.bounds = {
      x: bounds.x,
      y: bounds.y,
      width: Math.max(panel.minimumWidth, bounds.width),
      height: Math.max(panel.minimumHeight, bounds.height),
    }
    if (changedSize) this.resizeChildren(panel)
    this.solveGeometry()
    this.publishDom()
  }

  private setMinimumSize(panelId: VguiPanelId, width: number, height: number): void {
    const panel = this.requirePanel(panelId)
    if (![width, height].every((value) => safeInteger(value) && value >= 0 && value <= 32767)) throw new RuntimeFault("MalformedValue", panel.name)
    panel.minimumWidth = width
    panel.minimumHeight = height
    panel.bounds.width = Math.max(panel.bounds.width, width)
    panel.bounds.height = Math.max(panel.bounds.height, height)
    this.resizeChildren(panel)
    this.solveGeometry()
    this.publishDom()
  }

  private setPanelState(operation: Extract<VguiOperation, { kind: "set-panel-state" }>): void {
    const panel = this.requirePanel(operation.panel)
    if (operation.visible !== undefined) {
      panel.visible = operation.visible
      if (operation.visible && sameName(asSourceControl(panel.control), "Menu") && panel.popup) {
        this.removePopup(panel.id)
        this.popups.push(panel.id)
        this.sortPopups()
      }
    }
    if (operation.enabled !== undefined) panel.enabled = operation.enabled
    if (operation.mouseInput !== undefined) panel.mouseInput = operation.mouseInput
    if (operation.keyboardInput !== undefined) panel.keyboardInput = operation.keyboardInput
    if (operation.proportional !== undefined) panel.proportional = operation.proportional
    if (operation.z !== undefined) {
      if (!safeInteger(operation.z) || operation.z < -32768 || operation.z > 32767) throw new RuntimeFault("MalformedValue", `${panel.name}:z`)
      panel.z = operation.z
      if (panel.parent !== null) this.sortChildren(this.requirePanel(panel.parent))
    }
    if (operation.popup !== undefined && panel.popup !== operation.popup) {
      panel.popup = operation.popup
      if (panel.popup) this.addPopup(panel)
      else this.removePopup(panel.id)
    }
    if (operation.topmostPopup !== undefined) {
      panel.topmostPopup = operation.topmostPopup
      if (panel.popup) this.sortPopups()
    }
    if (this.keyFocus !== null && !this.inputEligible(this.keyFocus)) { this.requestedFocus = null; this.clearFocusRequested = true }
    if (!this.pointerEligible(this.capture)) this.setPointerCapture(null, null, null)
    this.solveGeometry()
    this.publishDom()
  }

  private reparentPanel(panelId: VguiPanelId, parentId: VguiPanelId): void {
    const panel = this.requirePanel(panelId)
    const parent = this.requirePanel(parentId)
    if (panel.id === this.rootPanel) throw new RuntimeFault("InvalidOperation", "root-reparent")
    if (panel.id === parent.id || this.hasAncestor(parent.id, panel.id)) throw new RuntimeFault("HierarchyCycle", panel.name)
    if (parent.children.length >= this.limits.maxChildrenPerPanel) throw new RuntimeFault("ChildLimit", parent.name)
    if (this.depth(parent.id) + this.subtreeDepth(panel.id) > this.limits.maxHierarchyDepth) throw new RuntimeFault("HierarchyLimit", panel.name)
    if (panel.parent !== null) {
      const oldParent = this.requirePanel(panel.parent)
      oldParent.children = oldParent.children.filter((id) => id !== panel.id)
    }
    panel.parent = parent.id
    if (this.editableParent(parent.id) && !panel.actionTargets.includes(parent.id)) panel.actionTargets.push(parent.id)
    parent.children.push(panel.id)
    this.sortChildren(parent)
    this.solveGeometry()
    this.publishDom()
    this.addTrace("panel-reparent", panel.id, String(parent.id))
  }

  private hasAncestor(panelId: VguiPanelId, ancestor: VguiPanelId): boolean {
    let panel: PanelState | undefined = this.panels.get(panelId)
    const seen = new Set<VguiPanelId>()
    while (panel) {
      if (panel.id === ancestor) return true
      if (seen.has(panel.id)) throw new RuntimeFault("HierarchyCycle", panel.name)
      seen.add(panel.id)
      panel = panel.parent === null ? undefined : this.panels.get(panel.parent)
    }
    return false
  }

  private subtreeDepth(panelId: VguiPanelId): number {
    const panel = this.requirePanel(panelId)
    let maximum = 1
    for (const child of panel.children) maximum = Math.max(maximum, 1 + this.subtreeDepth(child))
    return maximum
  }

  private deletePanel(panelId: VguiPanelId, deferred: boolean): void {
    const panel = this.requirePanel(panelId)
    if (panel.id === this.rootPanel) throw new RuntimeFault("InvalidOperation", "root-delete")
    if (deferred) {
      this.deferredDeletes.add(panel.id)
      this.addTrace("panel-mark-delete", panel.id, "deferred")
    } else {
      this.deletePanelImmediate(panel.id)
      this.solveGeometry()
      this.publishDom()
    }
  }

  private deletePanelImmediate(panelId: VguiPanelId): void {
    const panel = this.panels.get(panelId)
    if (!panel) return
    for (const child of [...panel.children].reverse()) this.deletePanelImmediate(child)
    if (panel.parent !== null) {
      const parent = this.panels.get(panel.parent)
      if (parent) parent.children = parent.children.filter((id) => id !== panel.id)
    }
    this.removePopup(panel.id)
    this.queuedMessages.splice(0, this.queuedMessages.length, ...this.queuedMessages.filter((message) => message.target !== panel.id && message.source !== panel.id))
    this.delayedCommands.splice(0, this.delayedCommands.length, ...this.delayedCommands.filter((command) => command.parent !== panel.id))
    this.activeAnimations.splice(0, this.activeAnimations.length, ...this.activeAnimations.filter((animation) => animation.panel !== panel.id && animation.parent !== panel.id))
    this.deferredDeletes.delete(panel.id)
    for (const [requestId, owner] of this.pendingClipboardReads) if (owner === panel.id) this.pendingClipboardReads.delete(requestId)
    if (this.mouseOver === panel.id) this.mouseOver = null
    if (this.mouseFocus === panel.id) this.mouseFocus = null
    if (this.capture === panel.id) this.setPointerCapture(null, null, null)
    if (this.keyFocus === panel.id) this.keyFocus = null
    if (this.calculatedKeyFocus === panel.id) this.calculatedKeyFocus = null
    if (this.requestedFocus === panel.id) this.requestedFocus = null
    if (this.applicationModal === panel.id) this.applicationModal = null
    if (this.modalSubtree === panel.id) this.modalSubtree = null
    if (this.outsideClickListener === panel.id) this.outsideClickListener = null
    panel.element.remove()
    for (const element of panel.itemElements.values()) {
      element.remove()
      this.auxiliaryNodes.delete(element)
    }
    panel.itemElements.clear()
    for (const element of panel.chromeElements.values()) {
      element.remove()
      this.auxiliaryNodes.delete(element)
    }
    panel.chromeElements.clear()
    this.panels.delete(panel.id)
    this.addTrace("panel-delete", panel.id, panel.name)
  }

  private movePanel(panelId: VguiPanelId, front: boolean): void {
    const panel = this.requirePanel(panelId)
    if (panel.popup) {
      this.removePopup(panel.id)
      if (front) this.popups.push(panel.id)
      else this.popups.unshift(panel.id)
      this.sortPopups()
    }
    if (panel.parent !== null) {
      const parent = this.requirePanel(panel.parent)
      parent.children = parent.children.filter((id) => id !== panel.id)
      const peers = parent.children.map((id) => this.requirePanel(id))
      if (front) {
        const firstHigher = peers.findIndex((peer) => peer.z > panel.z)
        const index = firstHigher < 0 ? peers.length : firstHigher
        parent.children.splice(index, 0, panel.id)
      } else {
        const firstEqualOrHigher = peers.findIndex((peer) => peer.z >= panel.z)
        const index = firstEqualOrHigher < 0 ? peers.length : firstEqualOrHigher
        parent.children.splice(index, 0, panel.id)
      }
      parent.children.forEach((id, index) => { this.requirePanel(id).tieOrder = this.nextOrder + index })
      this.nextOrder += parent.children.length
    }
    this.publishDom()
  }

  private replaceResource(parentId: VguiPanelId, document: VguiResourceDocument, selection: VguiResourceSelection): void {
    const parent = this.requirePanel(parentId)
    if (!document || !IDENTITY.test(document.logicalIdentity) || !validString(document.revision, 128, false)
      || !validResourceNode(document.root, this.limits, new Set(), 1, { value: 0 })) {
      throw new RuntimeFault("MalformedResource", document?.logicalIdentity ?? "document")
    }
    if (!selection || !Array.isArray(selection.activeConditions) || !Array.isArray(selection.resolutionSuffixes)
      || selection.activeConditions.some((value) => !validString(value, 255, false))
      || selection.resolutionSuffixes.some((value) => !validString(value, 64, false))) {
      throw new RuntimeFault("MalformedResource", `${document.logicalIdentity}:selection`)
    }
    const root = selectedResourceRoot(document, selection)
    const plans: ResourceControlPlan[] = []
    const known = new Map<string, { control: VguiControlName; panel: VguiPanelId | null }>()
    for (const childId of parent.children) {
      const child = this.requirePanel(childId)
      if (child.resourceOwner === null) known.set(asciiFold(child.name), { control: child.control, panel: child.id })
    }
    let newPanels = 0
    let newAuxiliary = 0
    for (const block of root.children) {
      if (block.value !== null) continue
      const foldedName = asciiFold(block.name)
      const existing = known.get(foldedName)
      const controlName = existing?.control ?? firstScalar(block, "ControlName") ?? null
      if (!controlName) throw new RuntimeFault("UnknownControl", `${document.logicalIdentity}:${block.name}`)
      const registration = this.registration(controlName)
      const properties: { name: string; value: string }[] = []
      for (const property of block.children) {
        if (sameName(property.name, "ControlName")) continue
        if (property.value === null) {
          if (![...OBJECT_PROPERTIES, ...selection.activeConditions].some((name) => sameName(name, property.name))) {
            throw new RuntimeFault("UnknownProperty", `${controlName}.${property.name}`)
          }
          continue
        }
        if (!propertyAllowed(controlName, registration, property.name)) throw new RuntimeFault("UnknownProperty", `${controlName}.${property.name}`)
        properties.push({ name: property.name, value: property.value })
      }
      this.validatePanelReferences(controlName, block.name, properties)
      this.validatePanelPropertyValues(parent, controlName, block.name, properties)
      if (!existing) {
        newPanels += 1
        if (["MessageBox", "QueryBox"].some((identity) => sameName(asSourceControl(controlName), identity))) newAuxiliary += 6
        else if (sameName(asSourceControl(controlName), "Frame")) newAuxiliary += 3
        known.set(foldedName, { control: controlName, panel: null })
      }
      plans.push(Object.freeze({ blockName: block.name, control: controlName, properties: Object.freeze(properties), existing: existing?.panel ?? null }))
    }
    const oldResourcePanels = parent.children.filter((id) => this.requirePanel(id).resourceOwner !== null)
    const oldAuxiliary = oldResourcePanels.reduce((count, id) => {
      const panel = this.requirePanel(id)
      return count + panel.itemElements.size + panel.chromeElements.size
    }, 0)
    if (this.panels.size - oldResourcePanels.length + newPanels > this.limits.maxPanels) throw new RuntimeFault("PanelLimit", document.logicalIdentity)
    if (parent.children.length - oldResourcePanels.length + newPanels > this.limits.maxChildrenPerPanel) throw new RuntimeFault("ChildLimit", parent.name)
    if (this.panels.size - oldResourcePanels.length + newPanels + this.auxiliaryNodes.size - oldAuxiliary + newAuxiliary + 2 > this.limits.maxDomNodes) throw new RuntimeFault("DomLimit", document.logicalIdentity)

    for (const panelId of [...oldResourcePanels].reverse()) this.deletePanelImmediate(panelId)
    const applied = new Map<string, PanelState>()
    for (const plan of plans) {
      let panel = plan.existing === null ? applied.get(asciiFold(plan.blockName)) : this.panels.get(plan.existing)
      if (!panel) {
        panel = this.createPanelInternal(parent.id, plan.control, plan.blockName, document.logicalIdentity)
        applied.set(asciiFold(plan.blockName), panel)
      }
      this.applyPanelProperties(panel, plan.properties)
      applied.delete(asciiFold(plan.blockName))
      applied.set(asciiFold(panel.name), panel)
    }
    this.solveGeometry()
    this.publishDom()
    this.addTrace("resource-replace", parent.id, `${document.logicalIdentity}:${plans.length}`)
  }

  private validatePanelReferences(control: VguiControlName, name: string, properties: readonly Readonly<{ name: string; value: string }>[]): void {
    const value = (propertyName: string): string | null => properties.find((property) => sameName(property.name, propertyName))?.value ?? null
    const font = value("font") ?? value("title_font")
    if (font && !this.fonts.has(asciiFold(font))) throw new RuntimeFault("MissingReference", `${name}:font:${font}`)
    const image = value("image")
    if (image && !this.images.has(asciiFold(image))) throw new RuntimeFault("MissingReference", `${name}:image:${image}`)
    const border = value("border") ?? value("border_override")
    if (border && !this.borders.has(asciiFold(border))) throw new RuntimeFault("MissingReference", `${name}:border:${border}`)
    const url = value("URLText")
    if (sameName(asSourceControl(control), "URLLabel") && url && !url.startsWith("#") && !URL.test(url)) {
      throw new RuntimeFault("MalformedValue", `${name}:URLText`)
    }
  }

  private validatePanelPropertyValues(parent: PanelState, control: VguiControlName, name: string, properties: readonly Readonly<{ name: string; value: string }>[]): void {
    const first = (propertyName: string): string | null => properties.find((property) => sameName(property.name, propertyName))?.value ?? null
    const probe = {
      parent: parent.id,
      name,
      proportional: false,
      bounds: { x: 0, y: 0, width: 0, height: 0 },
      properties: new Map(properties.map((property) => [property.name, property.value])),
    } as PanelState
    for (const dimension of ["wide", "tall"] as const) {
      const value = first(dimension)
      if (value !== null) this.computeDimension(value, probe, dimension === "wide", false, true)
    }
    for (const position of ["xpos", "ypos"] as const) {
      const value = first(position)
      if (value !== null) this.computePosition(value, probe, position === "xpos", true)
    }
    for (const property of ["zpos", "tabPosition", "TabPosition", "SubTabPosition", "maxchars", "rangeMin", "rangeMax", "rangeWindow", "value", "numTicks", "thumbwidth", "button_activation_type", "AutoResize", "PinCorner", "PinnedCornerOffsetX", "PinnedCornerOffsetY", "UnpinnedCornerOffsetX", "UnpinnedCornerOffsetY", "actionsignallevel", "rotation", "PaintBackgroundType", "RoundedCorners"]) {
      const value = first(property)
      if (value !== null) parseInteger(value, `${name}:${property}`)
    }
    for (const property of ["visible", "enabled", "mouseinputenabled", "keyboardinputenabled", "selected", "checked", "default", "stayselectedonclick", "stay_armed_on_click", "editable", "NumericInputOnly", "unicode", "textHidden", "selectallonfirstfocus", "multiline", "checkable", "moveable", "sizeable", "deleteSelfOnClose", "settitlebarvisible", "setclosebuttonvisible", "nobuttons"]) {
      const value = first(property)
      if (property === "visible" && ["Frame", "MessageBox", "QueryBox"].some((identity) => sameName(asSourceControl(control), identity))) continue
      if (value !== null) parseBoolean(value, `${name}:${property}`)
    }
    for (const property of ["allcaps", "dulltext", "brighttext", "wrap", "centerwrap", "auto_wide_tocontents", "auto_tall_tocontents", "use_proportional_insets", "scaleImage", "scaleProportional", "tileImage", "tileHorizontally", "tileVertically", "positionImage"]) {
      const value = first(property)
      if (value !== null) parseBoolean(value, `${name}:${property}`)
    }
    for (const property of ["progress", "scaleAmount"]) {
      const value = first(property)
      if (value !== null) parseFloatValue(value, `${name}:${property}`)
    }
    for (const property of ["fillcolor", "drawcolor", "fillcolor_override", "drawcolor_override", "fgcolor_override", "bgcolor_override", "defaultFgColor_override", "defaultBgColor_override", "armedFgColor_override", "armedBgColor_override", "depressedFgColor_override", "depressedBgColor_override", "selectedFgColor_override", "selectedBgColor_override", "keyboardFocusColor_override", "disabledfgcolor2_override", "infocus_bgcolor_override", "outoffocus_bgcolor_override", "titlebarbgcolor_override", "titlebardisabledbgcolor_override", "titlebarfgcolor_override", "titlebardisabledfgcolor_override"]) {
      const value = first(property)
      if (value !== null && !parseColorLiteral(value, 255)) this.resolveColor(value, property === "fillcolor" ? TRANSPARENT : WHITE)
    }
    if (sameName(asSourceControl(control), "ImagePanel")) {
      const imageName = first("image")
      const drawValue = first("drawcolor_override") ?? first("drawcolor")
      if (imageName) {
        const draw = drawValue ? parseColorLiteral(drawValue, 255) ?? this.resolveColor(drawValue, WHITE) : WHITE
        const white = draw[0] === 255 && draw[1] === 255 && draw[2] === 255 && draw[3] === 255
        const image = this.images.get(asciiFold(imageName))
        const rotation = parseInteger(first("rotation") ?? "0", `${name}:rotation`)
        if (rotation < 0 || rotation > 3) throw new RuntimeFault("MalformedValue", `${name}:rotation`)
        if ((!white || rotation !== 0) && !image?.variants?.some((variant) => variant.frame === 0 && variant.rotation === rotation && variant.tint.every((channel, index) => channel === draw[index]))) {
          throw new RuntimeFault("MissingReference", `${name}:image-variant:0:${draw.join(",")}`)
        }
      }
    }
    const fieldName = first("fieldName")
    if (fieldName !== null && !NAME.test(fieldName)) throw new RuntimeFault("MalformedValue", `${name}:fieldName`)
    const actionLevel = first("actionsignallevel")
    if (actionLevel !== null && parseInteger(actionLevel, `${name}:actionsignallevel`) > this.depth(parent.id)) {
      throw new RuntimeFault("MissingReference", `${name}:actionsignallevel`)
    }
    if ((first("auto_wide_tocontents") === "1" || first("auto_tall_tocontents") === "1")) {
      const fontName = first("font") ?? "Default"
      const font = this.fonts.get(asciiFold(fontName))
      if (!font?.measure) throw new RuntimeFault("MissingReference", `${name}:font-metrics:${fontName}`)
    }
    const backgroundType = parseInteger(first("PaintBackgroundType") ?? "0", `${name}:PaintBackgroundType`)
    if (backgroundType < 0 || backgroundType > 2) throw new RuntimeFault("MalformedValue", `${name}:PaintBackgroundType`)
    if (backgroundType > 0) {
      const color = first("bgcolor_override") ? parseColorLiteral(first("bgcolor_override")!, 0) ?? this.resolveColor(first("bgcolor_override")!, TRANSPARENT) : this.resolveColor("Panel.BgColor", TRANSPARENT)
      const textures = backgroundType === 1 ? [first("Texture1") ?? "vgui/hud/8x800corner1"] : [
        first("Texture1") ?? "vgui/hud/8x800corner1", first("Texture2") ?? "vgui/hud/8x800corner2",
        first("Texture3") ?? "vgui/hud/8x800corner3", first("Texture4") ?? "vgui/hud/8x800corner4",
      ]
      for (const texture of textures) {
        if (!this.imageUrl(texture, color, 0, 0)) throw new RuntimeFault("MissingReference", `${name}:background-texture:${texture}`)
      }
    }
    const sourceControl = asSourceControl(control)
    if (sameName(sourceControl, "URLLabel")) {
      const value = first("URLText")
      if (value?.startsWith("#")) {
        const localized = this.localization.get(asciiFold(value.slice(1)))
        if (!localized || !URL.test(localized)) throw new RuntimeFault("MalformedValue", `${name}:URLText`)
      }
    }
  }

  private applyPanelProperties(panel: PanelState, properties: readonly Readonly<{ name: string; value: string }>[]): void {
    const first = (name: string): string | null => properties.find((property) => sameName(property.name, name))?.value ?? null
    for (const property of properties) {
      if (![...panel.properties.keys()].some((name) => sameName(name, property.name))) panel.properties.set(property.name, property.value)
      const definition = panel.animationDefinitions.get(asciiFold(property.name))
      if (definition) panel.animationValues.set(definition.name, this.convertAnimationScalar(definition.converter, property.value, definition, panel))
    }

    const proportionalToParent = first("proportionalToParent") === "1"
    const referenceParent = proportionalToParent && panel.parent !== null ? this.requirePanel(panel.parent) : null
    const priorWidth = panel.bounds.width
    const priorHeight = panel.bounds.height
    const wide = first("wide")
    const tall = first("tall")
    if (wide !== null) panel.bounds.width = this.computeDimension(wide, panel, true, false, true)
    if (tall !== null) panel.bounds.height = this.computeDimension(tall, panel, false, false, true)
    panel.bounds.width = Math.max(panel.minimumWidth, panel.bounds.width)
    panel.bounds.height = Math.max(panel.minimumHeight, panel.bounds.height)
    const xpos = first("xpos")
    const ypos = first("ypos")
    if (xpos !== null) panel.bounds.x = this.computePosition(xpos, panel, true, true)
    if (ypos !== null) panel.bounds.y = this.computePosition(ypos, panel, false, true)
    if (referenceParent) {
      void priorWidth
      void priorHeight
    }

    const z = first("zpos")
    if (z !== null) {
      panel.z = parseInteger(z, `${panel.name}:zpos`)
      if (panel.parent !== null) this.sortChildren(this.requirePanel(panel.parent))
    }
    const isFrameControl = ["Frame", "MessageBox", "QueryBox"].some((identity) => sameName(asSourceControl(panel.control), identity))
    if (!isFrameControl) panel.visible = first("visible") === null ? true : parseInteger(first("visible")!, `${panel.name}:visible`) !== 0
    panel.enabled = first("enabled") === null ? true : parseBoolean(first("enabled")!, `${panel.name}:enabled`)
    const mouseInput = first("mouseinputenabled")
    if (mouseInput !== null && !parseBoolean(mouseInput, `${panel.name}:mouseinputenabled`)) panel.mouseInput = false
    const keyboardInput = first("keyboardinputenabled")
    if (keyboardInput !== null) panel.keyboardInput = parseBoolean(keyboardInput, `${panel.name}:keyboardinputenabled`)
    const tabPosition = first("tabPosition") ?? first("TabPosition")
    if (tabPosition !== null) panel.tabPosition = parseInteger(tabPosition, `${panel.name}:tabPosition`)
    const subTabPosition = first("SubTabPosition")
    if (subTabPosition !== null) panel.subTabPosition = parseInteger(subTabPosition, `${panel.name}:SubTabPosition`)
    const tooltip = first("tooltiptext")
    if (tooltip) panel.tooltip = this.resolveTextValue(tooltip, panel)
    for (const direction of ["navUp", "navDown", "navLeft", "navRight", "navToRelay", "navActivate", "navBack"]) {
      const target = first(direction)
      if (target !== null) panel.nav.set(asciiFold(direction), target)
    }
    const actionLevel = first("actionsignallevel")
    if (actionLevel !== null) {
      let target = panel
      let remaining = parseInteger(actionLevel, `${panel.name}:actionsignallevel`)
      if (remaining < 0) throw new RuntimeFault("MalformedValue", `${panel.name}:actionsignallevel`)
      while (remaining > 0) {
        if (target.parent === null) throw new RuntimeFault("MissingReference", `${panel.name}:actionsignallevel`)
        target = this.requirePanel(target.parent)
        remaining -= 1
      }
      if (!panel.actionTargets.includes(target.id)) panel.actionTargets.push(target.id)
    }
    const border = first("border") ?? first("border_override")
    if (border) this.applyBorder(panel, border)
    const font = first("font") ?? first("title_font")
    if (font) panel.font = font

    const text = first("labelText") ?? first("text")
    if (text !== null) {
      if (["MessageBox", "QueryBox"].some((identity) => sameName(asSourceControl(panel.control), identity))) {
        panel.bodyTextSource = text
        panel.bodyText = this.resolveTextValue(text, panel)
      } else {
        panel.textSource = text
        this.refreshText(panel)
      }
    }
    const title = first("title")
    if (title !== null) {
      panel.textSource = title
      this.refreshText(panel)
    }
    const accessibleDescription = first("description")
    if (accessibleDescription !== null) panel.accessibleDescription = this.resolveTextValue(accessibleDescription, panel)

    const command = first("command")
    if (command) panel.command = command
    const sourceControl = asSourceControl(panel.control)
    if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((identity) => sameName(sourceControl, identity))) {
      const selected = first("selected")
      if (selected !== null) this.setSelected(panel, parseBoolean(selected, `${panel.name}:selected`), false)
      panel.staySelected = first("stayselectedonclick") === null ? false : parseBoolean(first("stayselectedonclick")!, `${panel.name}:stayselectedonclick`)
      panel.stayArmed = first("stay_armed_on_click") === null ? false : parseBoolean(first("stay_armed_on_click")!, `${panel.name}:stay_armed_on_click`)
      const activation = first("button_activation_type")
      if (activation !== null) {
        const value = parseInteger(activation, `${panel.name}:button_activation_type`)
        if (value < 0 || value > 2) throw new RuntimeFault("MalformedValue", `${panel.name}:button_activation_type`)
        panel.activation = value as 0 | 1 | 2
      }
      const isDefault = first("default")
      if (isDefault !== null && parseBoolean(isDefault, `${panel.name}:default`) && panel.parent !== null) {
        const group = this.requirePanel(panel.parent)
        group.defaultButton = panel.id
        group.currentDefaultButton = panel.id
      }
      panel.checkable = panel.checkable || (first("checkable") !== null && parseBoolean(first("checkable")!, `${panel.name}:checkable`))
      const checked = first("checked")
      if (checked !== null) panel.checked = parseBoolean(checked, `${panel.name}:checked`)
    }
    if (["TextEntry", "RichText", "ComboBox"].some((identity) => sameName(sourceControl, identity))) {
      panel.editable = first("editable") === null ? panel.editable : parseBoolean(first("editable")!, `${panel.name}:editable`)
      panel.maximumCharacters = first("maxchars") === null ? panel.maximumCharacters : parseInteger(first("maxchars")!, `${panel.name}:maxchars`)
      if (panel.maximumCharacters < -1) throw new RuntimeFault("MalformedValue", `${panel.name}:maxchars`)
      panel.numericOnly = first("NumericInputOnly") === null ? panel.numericOnly : parseBoolean(first("NumericInputOnly")!, `${panel.name}:NumericInputOnly`)
      panel.allowUnicode = first("unicode") === null ? panel.allowUnicode : parseBoolean(first("unicode")!, `${panel.name}:unicode`)
      panel.textHidden = first("textHidden") === null ? panel.textHidden : parseBoolean(first("textHidden")!, `${panel.name}:textHidden`)
      panel.selectAllOnFirstFocus = first("selectallonfirstfocus") === null ? panel.selectAllOnFirstFocus : parseBoolean(first("selectallonfirstfocus")!, `${panel.name}:selectallonfirstfocus`)
      panel.multiline = first("multiline") === null ? panel.multiline : parseBoolean(first("multiline")!, `${panel.name}:multiline`)
    }
    if (sameName(sourceControl, "ImagePanel")) {
      const image = first("image")
      if (image) panel.image = image
      const fill = first("fillcolor_override") ?? first("fillcolor")
      if (fill) panel.fillColor = parseColorLiteral(fill, 255) ?? this.resolveColor(fill, TRANSPARENT)
      const draw = first("drawcolor_override") ?? first("drawcolor")
      if (draw) panel.drawColor = parseColorLiteral(draw, 255) ?? this.resolveColor(draw, WHITE)
    }
    if (sameName(sourceControl, "Slider") || sameName(sourceControl, "ScrollBar")) {
      const minimum = first("rangeMin")
      const maximum = first("rangeMax")
      if (minimum !== null) panel.minimum = parseInteger(minimum, `${panel.name}:rangeMin`)
      if (maximum !== null) panel.maximum = parseInteger(maximum, `${panel.name}:rangeMax`)
      if (sameName(sourceControl, "ScrollBar") && panel.maximum < panel.minimum) panel.maximum = panel.minimum
      const rangeWindow = first("rangeWindow")
      if (rangeWindow !== null) panel.rangeWindow = Math.max(0, parseInteger(rangeWindow, `${panel.name}:rangeWindow`))
      const value = first("value")
      if (value !== null) this.setControlValue(panel, parseInteger(value, `${panel.name}:value`), false)
      const ticks = first("numTicks")
      if (ticks !== null) panel.numTicks = Math.max(0, parseInteger(ticks, `${panel.name}:numTicks`))
      const thumb = first("thumbwidth")
      if (thumb !== null) panel.thumbWidth = this.proportional(parseInteger(thumb, `${panel.name}:thumbwidth`), panel)
    }
    if (sameName(sourceControl, "ProgressBar")) {
      const progress = first("progress")
      if (progress !== null) panel.progress = Math.max(0, Math.min(1, parseFloatValue(progress, `${panel.name}:progress`)))
      panel.progressVariable = first("variable") ?? first("analogValue") ?? panel.progressVariable
    }
    if (sameName(sourceControl, "URLLabel")) {
      const urlText = first("URLText")
      if (urlText !== null) {
        const resolved = urlText.startsWith("#") ? this.localization.get(asciiFold(urlText.slice(1))) : urlText
        if (!resolved || !URL.test(resolved)) throw new RuntimeFault("MalformedValue", `${panel.name}:URLText`)
        panel.url = resolved
      }
    }
    const fieldName = first("fieldName")
    if (fieldName !== null) {
      if (!NAME.test(fieldName)) throw new RuntimeFault("MalformedValue", `${panel.name}:fieldName`)
      panel.name = fieldName
      panel.element.dataset.vguiName = fieldName
      if (!panel.accessibleName || panel.accessibleName === panel.name) panel.accessibleName = fieldName
    }
    this.applyAutoResizeSettings(panel, first)
    this.updateAutoSize(panel)
    this.reapplyPanelPresentation(panel)
  }

  private applyAutoResizeSettings(panel: PanelState, first: (name: string) => string | null): void {
    const autoResize = first("AutoResize") === null ? 0 : parseInteger(first("AutoResize")!, `${panel.name}:AutoResize`)
    const pinCorner = first("PinCorner") === null ? 0 : parseInteger(first("PinCorner")!, `${panel.name}:PinCorner`)
    if (autoResize < 0 || autoResize > 3 || pinCorner < 0 || pinCorner > 3) throw new RuntimeFault("MalformedValue", `${panel.name}:auto-resize`)
    const parent = panel.parent === null ? null : this.requirePanel(panel.parent)
    const parentWidth = parent?.bounds.width ?? panel.bounds.width
    const parentHeight = parent?.bounds.height ?? panel.bounds.height
    const { x, y, width, height } = panel.bounds
    let pinX = 0
    let pinY = 0
    let resizeX = 0
    let resizeY = 0
    if (pinCorner === 0) {
      pinX = x; pinY = y; resizeX = x + width - parentWidth; resizeY = y + height - parentHeight
    } else if (pinCorner === 1) {
      pinX = x + width - parentWidth; pinY = y; resizeX = x; resizeY = y + height - parentHeight
    } else if (pinCorner === 2) {
      pinX = x; pinY = y + height - parentHeight; resizeX = x + width - parentWidth; resizeY = y
    } else {
      pinX = x + width - parentWidth; pinY = y + height - parentHeight; resizeX = x; resizeY = y
    }
    const override = (name: string, fallback: number): number => {
      const value = first(name)
      if (value === null) return fallback
      const parsed = parseInteger(value, `${panel.name}:${name}`)
      return panel.proportional ? this.proportional(parsed, panel) : parsed
    }
    panel.autoResize = autoResize as 0 | 1 | 2 | 3
    panel.pinCorner = pinCorner as 0 | 1 | 2 | 3
    panel.pinOffsetX = override("PinnedCornerOffsetX", pinX)
    panel.pinOffsetY = override("PinnedCornerOffsetY", pinY)
    panel.resizeOffsetX = autoResize === 0 ? 0 : override("UnpinnedCornerOffsetX", resizeX)
    panel.resizeOffsetY = autoResize === 0 ? 0 : override("UnpinnedCornerOffsetY", resizeY)
  }

  private resizeChildren(parent: PanelState): void {
    if (parent.properties.get("skip_autoresize") === "1") return
    for (const childId of parent.children) {
      const child = this.requirePanel(childId)
      const horizontal = child.autoResize === 1 || child.autoResize === 3
      const vertical = child.autoResize === 2 || child.autoResize === 3
      let x: number
      let y: number
      let right: number
      let bottom: number
      if (child.pinCorner === 1 || child.pinCorner === 3) {
        right = parent.bounds.width + child.pinOffsetX
        x = horizontal ? child.resizeOffsetX : right - child.bounds.width
      } else {
        x = child.pinOffsetX
        right = horizontal ? parent.bounds.width + child.resizeOffsetX : child.pinOffsetX + child.bounds.width
      }
      if (child.pinCorner === 2 || child.pinCorner === 3) {
        bottom = parent.bounds.height + child.pinOffsetY
        y = vertical ? child.resizeOffsetY : bottom - child.bounds.height
      } else {
        y = child.pinOffsetY
        bottom = vertical ? parent.bounds.height + child.resizeOffsetY : child.pinOffsetY + child.bounds.height
      }
      right = Math.max(right, x)
      bottom = Math.max(bottom, y)
      const changed = child.bounds.width !== right - x || child.bounds.height !== bottom - y
      child.bounds = { x, y, width: right - x, height: bottom - y }
      if (changed) this.resizeChildren(child)
    }
  }

  private applyBorder(panel: PanelState, name: string): void {
    const border = this.borders.get(asciiFold(name))
    if (!border) throw new RuntimeFault("MissingReference", `${panel.name}:border:${name}`)
    panel.border = border.name
    panel.inset = { ...border.inset }
    panel.animationValues.set("PaintBackgroundType", border.backgroundType)
  }

  private resolveTextValue(source: string, panel: PanelState): string {
    let value = source
    if (source.startsWith("#")) {
      const localized = this.localization.get(asciiFold(source.slice(1)))
      if (localized === undefined) this.recordDiagnostic("MissingLocalization", "replace-resource", source)
      else value = localized
    }
    const variables = this.dialogVariablesFor(panel)
    let output = ""
    for (let index = 0; index < value.length;) {
      if (value[index] !== "%") {
        output += value[index++]
        continue
      }
      if (value[index + 1] === "%") {
        output += "%"
        index += 2
        continue
      }
      if (value[index + 1] === "s" && /[0-9]/u.test(value[index + 2] ?? "")) {
        output += value[index++]
        continue
      }
      const end = value.indexOf("%", index + 1)
      if (end < 0) {
        output += value[index++]
        continue
      }
      const name = value.slice(index + 1, end).slice(0, 31)
      output += variables.get(asciiFold(name)) ?? "[unknown]"
      index = end + 1
    }
    if (output.length > this.limits.maxTextCodeUnits) throw new RuntimeFault("TextLimit", panel.name)
    return output
  }

  private dialogVariablesFor(panel: PanelState): ReadonlyMap<string, string> {
    if (panel.dialogVariables.size > 0) return panel.dialogVariables
    if (panel.parent !== null) return this.requirePanel(panel.parent).dialogVariables
    return panel.dialogVariables
  }

  private visibleText(text: string): string {
    let output = ""
    for (let index = 0; index < text.length; index += 1) {
      if (text[index] !== "&") output += text[index]
      else if (text[index + 1] === "&") { output += "&"; index += 1 }
    }
    return output
  }

  private hotkey(text: string): string | null {
    for (let index = 0; index < text.length - 1; index += 1) {
      if (text[index] !== "&") continue
      if (text[index + 1] === "&") { index += 1; continue }
      const candidate = text[index + 1]
      if (/^[\p{L}\p{N}]$/u.test(candidate)) return candidate.toLocaleLowerCase("en-US")
    }
    return null
  }

  private refreshText(panel: PanelState): void {
    const resolved = this.resolveTextValue(panel.textSource, panel)
    panel.text = panel.properties.get("allcaps") === "1"
      ? this.visibleText(resolved).toUpperCase()
      : this.visibleText(resolved)
    panel.accessibleName = panel.text || panel.name
    panel.caret = Math.min(panel.caret, panel.text.length)
    panel.selectionStart = panel.selectionStart < 0 ? -1 : Math.min(panel.selectionStart, panel.text.length)
    panel.selectionEnd = Math.min(panel.selectionEnd, panel.text.length)
  }

  private refreshBodyText(panel: PanelState): void {
    if (!panel.bodyTextSource) return
    panel.bodyText = this.resolveTextValue(panel.bodyTextSource, panel)
  }

  private reapplyPanelPresentation(panel: PanelState): void {
    const sourceControl = asSourceControl(panel.control)
    panel.element.style.backgroundImage = "none"
    panel.element.style.backgroundSize = "auto"
    panel.element.style.backgroundPosition = "0% 0%"
    panel.element.style.backgroundRepeat = "repeat"
    let foreground = this.resolveColor("Panel.FgColor", WHITE)
    let background = this.resolveColor("Panel.BgColor", TRANSPARENT)
    if (sameName(sourceControl, "Label")) {
      const labelColor = panel.properties.get("brighttext") === "1" ? "Label.TextBrightColor" : panel.properties.get("dulltext") === "1" ? "Label.TextDullColor" : "Label.TextColor"
      foreground = this.resolveColor(panel.enabled ? labelColor : "Label.DisabledFgColor1", foreground)
      background = this.resolveColor("Label.BgColor", background)
    } else if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(sourceControl, name))) {
      const prefix = sameName(sourceControl, "MenuItem") ? "Menu" : sameName(sourceControl, "CheckButton") ? "CheckButton" : sameName(sourceControl, "RadioButton") ? "RadioButton" : "Button"
      const state = panel.depressed ? "Depressed" : panel.armed ? "Armed" : panel.selected ? "Selected" : ""
      foreground = this.resolveColor(`${prefix}.${state ? `${state}TextColor` : "TextColor"}`, foreground)
      background = this.resolveColor(`${prefix}.${state ? `${state}BgColor` : "BgColor"}`, background)
    } else if (sameName(sourceControl, "TextEntry")) {
      foreground = this.resolveColor(panel.enabled ? "TextEntry.TextColor" : "TextEntry.DisabledTextColor", foreground)
      background = this.resolveColor(panel.enabled ? "TextEntry.BgColor" : "TextEntry.DisabledBgColor", background)
    } else if (sameName(sourceControl, "RichText")) {
      foreground = this.resolveColor("RichText.TextColor", foreground)
      background = this.resolveColor("RichText.BgColor", background)
    } else if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(sourceControl, name))) {
      const focused = this.keyFocus !== null && this.hasAncestor(this.keyFocus, panel.id)
      background = this.resolveColor(focused ? "Frame.BgColor" : "Frame.OutOfFocusBgColor", background)
      const override = panel.properties.get(focused ? "infocus_bgcolor_override" : "outoffocus_bgcolor_override")
      if (override) background = parseColorLiteral(override, 0) ?? this.resolveColor(override, background)
    } else if (sameName(sourceControl, "ProgressBar")) {
      foreground = this.resolveColor("ProgressBar.FgColor", foreground)
      background = this.resolveColor("ProgressBar.BgColor", background)
    } else if (sameName(sourceControl, "Slider")) {
      foreground = this.resolveColor("Slider.NobColor", foreground)
      background = this.resolveColor("Slider.TrackColor", background)
    } else if (sameName(sourceControl, "ListPanel")) {
      foreground = this.resolveColor("ListPanel.TextColor", foreground)
      background = this.resolveColor("ListPanel.BgColor", background)
    }
    const foregroundOverride = panel.properties.get(panel.depressed ? "depressedFgColor_override" : panel.armed ? "armedFgColor_override" : panel.selected ? "selectedFgColor_override" : "defaultFgColor_override")
      ?? panel.properties.get("fgcolor_override")
    const backgroundOverride = panel.properties.get(panel.depressed ? "depressedBgColor_override" : panel.armed ? "armedBgColor_override" : panel.selected ? "selectedBgColor_override" : "defaultBgColor_override")
      ?? panel.properties.get("bgcolor_override")
    if (foregroundOverride) foreground = parseColorLiteral(foregroundOverride, 0) ?? this.resolveColor(foregroundOverride, foreground)
    if (backgroundOverride) background = parseColorLiteral(backgroundOverride, 0) ?? this.resolveColor(backgroundOverride, background)
    const animatedForeground = panel.animationValues.get("FgColor")
    const animatedBackground = panel.animationValues.get("BgColor")
    if (Array.isArray(animatedForeground) && animatedForeground.length === 4) foreground = animatedForeground as unknown as Rgba
    if (Array.isArray(animatedBackground) && animatedBackground.length === 4) background = animatedBackground as unknown as Rgba
    panel.element.style.color = rgba(foreground)
    this.presentPanelBackground(panel, background)
    const alpha = panel.animationValues.get("alpha")
    panel.element.style.opacity = String(Math.max(0, Math.min(255, typeof alpha === "number" ? alpha : 255)) / 255)
    if (panel.font) {
      const font = this.fonts.get(asciiFold(panel.font))
      if (!font) throw new RuntimeFault("MissingReference", `${panel.name}:font:${panel.font}`)
      panel.element.style.fontFamily = font.cssFamily
      panel.element.style.fontSize = `${font.sizePx}px`
      panel.element.style.lineHeight = `${font.lineHeightPx}px`
      panel.element.style.fontWeight = String(font.weight)
      panel.element.style.fontStyle = font.style
      panel.element.style.fontSynthesis = "none"
      panel.element.dataset.fontAvailable = font.available ? "true" : "false"
      if (!font.available) panel.element.style.color = "transparent"
      if (sameName(sourceControl, "Label")) {
        const wrap = panel.properties.get("wrap") === "1" || panel.properties.get("centerwrap") === "1"
        panel.element.style.whiteSpace = wrap ? "normal" : "nowrap"
        panel.element.style.textAlign = panel.properties.get("centerwrap") === "1" ? "center" : this.textAlignment(panel.properties.get("textAlignment"))[0]
      }
    }
    if (!panel.font && (panel.text.length > 0 || panel.bodyText.length > 0 || panel.items.length > 0)) {
      panel.element.dataset.fontAvailable = "false"
      panel.element.style.color = "transparent"
    }
    if (sameName(sourceControl, "Label")) {
      const alignment = this.textAlignment(panel.properties.get("textAlignment"))
      panel.element.style.display = "flex"
      panel.element.style.justifyContent = alignment[0] === "left" ? "flex-start" : alignment[0] === "right" ? "flex-end" : "center"
      panel.element.style.alignItems = alignment[1] === "top" ? "flex-start" : alignment[1] === "bottom" ? "flex-end" : "center"
      let insetX = Number(panel.properties.get("textinsetx") ?? 0)
      let insetY = Number(panel.properties.get("textinsety") ?? 0)
      if (panel.properties.get("use_proportional_insets") === "1") {
        insetX = this.proportional(insetX, panel)
        insetY = Math.ceil(insetY * this.proportional(1000, panel) / 1000)
      }
      panel.element.style.padding = `${insetY}px ${insetX}px`
    }
    if (panel.border) this.presentBorder(panel, this.borders.get(asciiFold(panel.border))!)
    if (panel.image) this.presentImage(panel, this.images.get(asciiFold(panel.image))!)
  }

  private textAlignment(value: string | undefined): readonly ["left" | "center" | "right", "top" | "center" | "bottom"] {
    const folded = asciiFold(value ?? "west")
    const horizontal = folded.includes("west") ? "left" : folded.includes("east") ? "right" : "center"
    const vertical = folded.includes("north") ? "top" : folded.includes("south") ? "bottom" : "center"
    return Object.freeze([horizontal, vertical])
  }

  private updateAutoSize(panel: PanelState): void {
    if (panel.properties.get("auto_wide_tocontents") !== "1" && panel.properties.get("auto_tall_tocontents") !== "1") return
    const font = this.fonts.get(asciiFold(panel.font ?? "Default"))
    if (!font?.measure) throw new RuntimeFault("MissingReference", `${panel.name}:font-metrics:${panel.font ?? "Default"}`)
    const wrap = panel.properties.get("wrap") === "1" || panel.properties.get("centerwrap") === "1"
    const metrics = font.measure(panel.text, wrap ? Math.max(0, panel.bounds.width) : null)
    if (!metrics || !finite(metrics.width) || !finite(metrics.height) || metrics.width < 0 || metrics.height < 0) throw new RuntimeFault("MalformedValue", `${panel.name}:font-metrics`)
    if (panel.properties.get("auto_wide_tocontents") === "1") panel.bounds.width = Math.max(panel.minimumWidth, Math.ceil(metrics.width))
    if (panel.properties.get("auto_tall_tocontents") === "1") panel.bounds.height = Math.max(panel.minimumHeight, Math.ceil(metrics.height))
  }

  private presentBorder(panel: PanelState, border: VguiBorder): void {
    if (border.kind === "line") {
      const images: string[] = []
      const sizes: string[] = []
      const positions: string[] = []
      const append = (color: Rgba, size: string, position: string): void => {
        images.push(`linear-gradient(${rgba(color)}, ${rgba(color)})`)
        sizes.push(size)
        positions.push(position)
      }
      border.sides.left.forEach((line, index) => append(line.color, `1px calc(100% - ${line.startOffset + line.endOffset}px)`, `${index}px ${line.startOffset}px`))
      border.sides.top.forEach((line, index) => append(line.color, `calc(100% - ${line.startOffset + line.endOffset}px) 1px`, `${line.startOffset}px ${index}px`))
      border.sides.right.forEach((line, index) => append(line.color, `1px calc(100% - ${line.startOffset + line.endOffset}px)`, `calc(100% - ${index + 1}px) ${line.startOffset}px`))
      border.sides.bottom.forEach((line, index) => append(line.color, `calc(100% - ${line.startOffset + line.endOffset}px) 1px`, `${line.startOffset}px calc(100% - ${index + 1}px)`))
      images.reverse()
      sizes.reverse()
      positions.reverse()
      panel.element.style.border = "0"
      panel.element.style.borderImage = "none"
      const existingImages = panel.element.style.backgroundImage
      const existingSizes = panel.element.style.backgroundSize
      const existingPositions = panel.element.style.backgroundPosition
      const hasExisting = !!existingImages && existingImages !== "none"
      panel.element.style.backgroundImage = [...images, ...(hasExisting ? [existingImages] : [])].join(", ")
      panel.element.style.backgroundSize = [...sizes, ...(hasExisting ? [existingSizes] : [])].join(", ")
      panel.element.style.backgroundPosition = [...positions, ...(hasExisting ? [existingPositions] : [])].join(", ")
      panel.element.style.backgroundRepeat = "no-repeat"
      return
    }
    const image = this.images.get(asciiFold(border.image))
    if (!image) throw new RuntimeFault("MissingReference", `${panel.name}:border-image:${border.image}`)
    panel.element.style.borderStyle = "solid"
    const variant = border.kind === "scalable-image" && border.color.some((channel) => channel !== 255)
      ? image.variants?.find((candidate) => candidate.frame === 0 && candidate.rotation === 0 && candidate.tint.every((channel, index) => channel === border.color[index]))
      : null
    panel.element.style.borderImageSource = `url(${JSON.stringify(variant?.browserUrl ?? image.browserUrl)})`
    if (border.kind === "image") {
      panel.element.style.borderImageSlice = "1 fill"
      panel.element.style.borderImageRepeat = border.tiled ? "repeat" : "stretch"
    } else {
      panel.element.style.borderImageSlice = `${border.sourceCornerHeight} ${border.sourceCornerWidth} fill`
      panel.element.style.borderImageWidth = `${border.drawCornerHeight}px ${border.drawCornerWidth}px`
      panel.element.style.borderImageRepeat = "stretch"
    }
  }

  private imageUrl(name: string, tint: Rgba, frame: number, rotation: 0 | 1 | 2 | 3): string | null {
    const image = this.images.get(asciiFold(name))
    if (!image || frame < 0 || frame >= image.frames) return null
    const white = tint.every((channel) => channel === 255)
    if (frame === 0 && rotation === 0 && white) return image.browserUrl
    return image.variants?.find((variant) => variant.frame === frame && variant.rotation === rotation && variant.tint.every((channel, index) => channel === tint[index]))?.browserUrl ?? null
  }

  private presentPanelBackground(panel: PanelState, color: Rgba): void {
    const value = panel.animationValues.get("PaintBackgroundType")
    const type = typeof value === "number" ? Math.max(0, Math.min(2, Math.trunc(value))) : 0
    if (type === 0) {
      panel.element.style.backgroundColor = rgba(color)
      return
    }
    panel.element.style.backgroundColor = "transparent"
    if (type === 1) {
      const texture = String(panel.animationValues.get("Texture1") ?? "")
      const url = this.imageUrl(texture, color, 0, 0)
      if (!url) {
        this.recordDiagnostic("MissingReference", "frame", `${panel.name}:background-texture:${texture}`)
        return
      }
      panel.element.style.backgroundImage = `url(${JSON.stringify(url)})`
      panel.element.style.backgroundSize = "100% 100%"
      panel.element.style.backgroundPosition = "0 0"
      panel.element.style.backgroundRepeat = "no-repeat"
      return
    }
    const corner = panel.proportional ? Math.max(Math.trunc(this.proportional(8, panel) / 2), 8) : 8
    const mask = Math.max(0, Math.min(15, Number(panel.properties.get("RoundedCorners") ?? 15)))
    const textures = ["Texture1", "Texture2", "Texture3", "Texture4"].map((name) => String(panel.animationValues.get(name) ?? ""))
    const cornerPositions = ["0 0", "100% 0", "100% 100%", "0 100%"]
    const bits = [1, 2, 8, 4]
    const images: string[] = []
    const sizes: string[] = []
    const positions: string[] = []
    for (let index = 0; index < 4; index += 1) {
      const url = (mask & bits[index]) !== 0 ? this.imageUrl(textures[index], color, 0, 0) : null
      if ((mask & bits[index]) !== 0 && !url) {
        this.recordDiagnostic("MissingReference", "frame", `${panel.name}:background-texture:${textures[index]}`)
        continue
      }
      images.push(url ? `url(${JSON.stringify(url)})` : `linear-gradient(${rgba(color)}, ${rgba(color)})`)
      sizes.push(`${corner}px ${corner}px`)
      positions.push(cornerPositions[index])
    }
    images.push(`linear-gradient(${rgba(color)}, ${rgba(color)})`, `linear-gradient(${rgba(color)}, ${rgba(color)})`, `linear-gradient(${rgba(color)}, ${rgba(color)})`)
    sizes.push(`calc(100% - ${corner * 2}px) ${corner}px`, `100% calc(100% - ${corner * 2}px)`, `calc(100% - ${corner * 2}px) ${corner}px`)
    positions.push(`${corner}px 0`, `0 ${corner}px`, `${corner}px 100%`)
    panel.element.style.backgroundImage = images.join(", ")
    panel.element.style.backgroundSize = sizes.join(", ")
    panel.element.style.backgroundPosition = positions.join(", ")
    panel.element.style.backgroundRepeat = "no-repeat"
  }

  private presentImage(panel: PanelState, image: VguiImagePresentation): void {
    if (!image) throw new RuntimeFault("MissingReference", `${panel.name}:image`)
    const frame = Math.max(0, Math.min(image.frames - 1, Number(panel.properties.get("frame") ?? 0)))
    const rotation = Number(panel.properties.get("rotation") ?? 0)
    if (!safeInteger(rotation) || rotation < 0 || rotation > 3) throw new RuntimeFault("MalformedValue", `${panel.name}:rotation`)
    const whiteTint = panel.drawColor[0] === 255 && panel.drawColor[1] === 255 && panel.drawColor[2] === 255 && panel.drawColor[3] === 255
    const variant = image.variants?.find((candidate) => candidate.frame === frame && candidate.rotation === rotation && candidate.tint.every((channel, index) => channel === panel.drawColor[index]))
    if ((frame !== 0 || !whiteTint || rotation !== 0) && !variant) {
      panel.element.removeAttribute("src")
      panel.element.style.backgroundImage = "none"
      this.recordDiagnostic("MissingReference", "frame", `${panel.name}:image-variant:${frame}:${panel.drawColor.join(",")}`)
      return
    }
    const url = variant?.browserUrl ?? image.browserUrl
    const tiled = panel.properties.get("tileImage") === "1" || panel.properties.get("tileHorizontally") === "1" || panel.properties.get("tileVertically") === "1"
    panel.element.removeAttribute("src")
    panel.element.style.backgroundImage = `url(${JSON.stringify(url)})`
    if (tiled) {
      panel.element.style.backgroundRepeat = panel.properties.get("tileHorizontally") === "0" ? "repeat-y" : panel.properties.get("tileVertically") === "0" ? "repeat-x" : "repeat"
      panel.element.style.backgroundSize = `${image.width}px ${image.height}px`
    } else {
      panel.element.style.backgroundRepeat = "no-repeat"
      let scale = Number(panel.properties.get("scaleAmount") ?? 0)
      if (panel.properties.get("scaleProportional") === "1" && panel.properties.get("scaleImage") === "1" && panel.proportional) {
        scale *= this.proportional(1000, panel) * 0.001
      }
      panel.element.style.backgroundSize = panel.properties.get("scaleImage") === "1"
        ? scale > 0 ? `${Math.trunc(image.width * scale)}px ${Math.trunc(image.height * scale)}px` : "100% 100%"
        : `${image.width}px ${image.height}px`
      panel.element.style.backgroundPosition = "0 0"
    }
    panel.element.setAttribute("draggable", "false")
    panel.element.style.objectFit = panel.properties.get("scaleImage") === "1" ? "fill" : "none"
    panel.element.style.imageRendering = image.hardwareFiltered ? "auto" : "pixelated"
    panel.element.style.backgroundColor = rgba(panel.fillColor)
    panel.element.style.transform = "none"
  }

  private setDialogVariable(panelId: VguiPanelId, name: string, value: string | number): void {
    const panel = this.requirePanel(panelId)
    if (!validString(name, 31, false) || (typeof value !== "string" && !finite(value))) throw new RuntimeFault("MalformedValue", `${panel.name}:dialog-variable`)
    if (typeof value === "string" && !validString(value, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", name)
    const folded = asciiFold(name)
    if (!panel.dialogVariables.has(folded) && panel.dialogVariables.size >= this.limits.maxDialogVariables) throw new RuntimeFault("RegistryLimit", `${panel.name}:dialog-variables`)
    const previous = panel.dialogVariables.get(folded)
    panel.dialogVariables.set(folded, String(value))
    try {
      for (const target of [panel, ...panel.children.map((id) => this.requirePanel(id))]) {
        this.refreshText(target)
        this.refreshBodyText(target)
        this.updateAutoSize(target)
        if (target.progressVariable && sameName(target.progressVariable, name)) {
          const numeric = Number(value)
          if (Number.isFinite(numeric) && numeric >= 0) target.progress = Math.max(0, Math.min(1, Math.trunc(numeric) / 100))
        }
      }
    } catch (error) {
      if (previous === undefined) panel.dialogVariables.delete(folded)
      else panel.dialogVariables.set(folded, previous)
      for (const target of [panel, ...panel.children.map((id) => this.requirePanel(id))]) { this.refreshText(target); this.refreshBodyText(target) }
      throw error
    }
    this.publishDom()
    this.addTrace("dialog-variable", panel.id, name)
  }

  private mutateControl(panelId: VguiPanelId, mutation: VguiControlMutation): void {
    const panel = this.requirePanel(panelId)
    if (!mutation || typeof mutation !== "object") throw new RuntimeFault("InvalidOperation", panel.name)
    if (mutation.text !== undefined && !validString(mutation.text, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", panel.name)
    if (mutation.items !== undefined) {
      if (!Array.isArray(mutation.items) || mutation.items.length > this.limits.maxChildrenPerPanel) throw new RuntimeFault("ChildLimit", `${panel.name}:items`)
      const resultingNodes = this.panels.size + this.auxiliaryNodes.size - panel.itemElements.size + mutation.items.length + 2
      if (resultingNodes > this.limits.maxDomNodes) throw new RuntimeFault("DomLimit", `${panel.name}:items`)
      const ids = new Set<number>()
      for (const item of mutation.items) {
        if (!item || !safeInteger(item.id) || ids.has(item.id) || !validString(item.text, this.limits.maxTextCodeUnits)) throw new RuntimeFault("MalformedValue", `${panel.name}:items`)
        ids.add(item.id)
        if (item.command !== undefined && !validString(item.command, this.limits.maxStringCodeUnits)) throw new RuntimeFault("MalformedValue", `${panel.name}:item-command`)
        if (sameName(asSourceControl(panel.control), "PropertySheet")) {
          const page = this.panels.get(item.id)
          if (!page || page.parent !== panel.id || !sameName(asSourceControl(page.control), "PropertyPage")) throw new RuntimeFault("MissingReference", `${panel.name}:page:${item.id}`)
        }
      }
    }
    for (const number of [mutation.value, mutation.minimum, mutation.maximum, mutation.rangeWindow, mutation.progress, mutation.imageFrame]) {
      if (number !== undefined && !finite(number)) throw new RuntimeFault("MalformedValue", panel.name)
    }
    if (mutation.image !== undefined && (!validString(mutation.image, this.limits.maxStringCodeUnits, false) || !this.images.has(asciiFold(mutation.image)))) {
      throw new RuntimeFault("MissingReference", `${panel.name}:image:${mutation.image ?? ""}`)
    }
    if (mutation.imageFrame !== undefined) {
      const imageName = mutation.image ?? panel.image
      const image = imageName ? this.images.get(asciiFold(imageName)) : null
      if (!image || !safeInteger(mutation.imageFrame) || mutation.imageFrame < 0 || mutation.imageFrame >= image.frames) throw new RuntimeFault("MalformedValue", `${panel.name}:imageFrame`)
      const rotation = Number(panel.properties.get("rotation") ?? 0)
      const white = panel.drawColor[0] === 255 && panel.drawColor[1] === 255 && panel.drawColor[2] === 255 && panel.drawColor[3] === 255
      if ((mutation.imageFrame !== 0 || !white || rotation !== 0) && !image.variants?.some((variant) => variant.frame === mutation.imageFrame && variant.rotation === rotation && variant.tint.every((channel, index) => channel === panel.drawColor[index]))) {
        throw new RuntimeFault("MissingReference", `${panel.name}:image-variant:${mutation.imageFrame}`)
      }
    }
    if (mutation.activeIndex !== undefined && mutation.activeIndex !== null
      && (!safeInteger(mutation.activeIndex) || mutation.activeIndex < 0 || mutation.activeIndex >= (mutation.items ?? panel.items).length)) {
      throw new RuntimeFault("MalformedValue", `${panel.name}:activeIndex`)
    }
    if (mutation.url !== undefined && !URL.test(mutation.url)) throw new RuntimeFault("MalformedValue", `${panel.name}:url`)
    if (mutation.title !== undefined && !validString(mutation.title, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", panel.name)
    if (mutation.description !== undefined && !validString(mutation.description, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", panel.name)
    if (mutation.text !== undefined) {
      if (["MessageBox", "QueryBox"].some((identity) => sameName(asSourceControl(panel.control), identity))) {
        panel.bodyTextSource = mutation.text
        this.refreshBodyText(panel)
      } else {
        panel.textSource = mutation.text
        this.refreshText(panel)
        panel.caret = panel.text.length
        panel.selectionStart = -1
        panel.selectionEnd = panel.caret
      }
      this.updateAutoSize(panel)
    }
    if (mutation.minimum !== undefined) panel.minimum = Math.trunc(mutation.minimum)
    if (mutation.maximum !== undefined) panel.maximum = Math.trunc(mutation.maximum)
    if (mutation.rangeWindow !== undefined) panel.rangeWindow = Math.max(0, Math.trunc(mutation.rangeWindow))
    if (mutation.value !== undefined) this.setControlValue(panel, Math.trunc(mutation.value), true)
    if (mutation.selected !== undefined) this.setSelected(panel, mutation.selected, true)
    if (mutation.checked !== undefined) panel.checked = mutation.checked
    if (mutation.items !== undefined) {
      panel.items = mutation.items.map((item) => ({ id: item.id, text: item.text, command: item.command ?? null, enabled: item.enabled ?? true, checked: item.checked ?? false }))
      if (panel.activeIndex !== null && panel.activeIndex >= panel.items.length) panel.activeIndex = null
      if (sameName(asSourceControl(panel.control), "PropertySheet") && panel.activeIndex === null && panel.items.length > 0) panel.activeIndex = 0
    }
    if (mutation.activeIndex !== undefined) {
      panel.activeIndex = mutation.activeIndex
      if (panel.activeIndex !== null) {
        panel.textSource = panel.items[panel.activeIndex].text
        this.refreshText(panel)
        this.updateAutoSize(panel)
      }
    }
    if (sameName(asSourceControl(panel.control), "PropertySheet")) this.activatePropertyPage(panel)
    if (mutation.progress !== undefined) panel.progress = Math.max(0, Math.min(1, mutation.progress))
    if (mutation.image !== undefined) panel.image = mutation.image
    if (mutation.imageFrame !== undefined) panel.properties.set("frame", String(mutation.imageFrame))
    if (mutation.url !== undefined) {
      panel.url = mutation.url
    }
    if (mutation.title !== undefined) {
      panel.textSource = mutation.title
      this.refreshText(panel)
      this.updateAutoSize(panel)
    }
    if (mutation.description !== undefined) {
      panel.accessibleDescription = mutation.description
    }
    this.reapplyPanelPresentation(panel)
    this.publishDom()
  }

  private setControlValue(panel: PanelState, value: number, signal: boolean): void {
    const sourceControl = asSourceControl(panel.control)
    let clamped: number
    if (sameName(sourceControl, "ScrollBar")) clamped = Math.max(panel.minimum, Math.min(panel.maximum - panel.rangeWindow, value))
    else if (panel.minimum <= panel.maximum) clamped = Math.max(panel.minimum, Math.min(panel.maximum, value))
    else clamped = Math.max(panel.maximum, Math.min(panel.minimum, value))
    const changed = panel.value !== clamped
    panel.value = clamped
    if (changed && signal) this.postAction(panel, sameName(sourceControl, "Slider") ? "SliderMoved" : "ScrollBarSliderMoved", { position: clamped })
  }

  private setSelected(panel: PanelState, selected: boolean, signal: boolean): void {
    const sourceControl = asSourceControl(panel.control)
    if (sameName(sourceControl, "RadioButton")) {
      if (selected && !panel.enabled) return
      if (selected && panel.parent !== null) {
        const originalTab = panel.tabPosition || panel.subTabPosition
        const parent = this.requirePanel(panel.parent)
        for (const siblingId of parent.children) {
          const sibling = this.requirePanel(siblingId)
          const siblingTab = sibling.tabPosition || sibling.subTabPosition
          if (sibling.id !== panel.id && sameName(asSourceControl(sibling.control), "RadioButton") && siblingTab === originalTab) {
            sibling.selected = false
            sibling.checked = false
            if (sibling.tabPosition !== 0) sibling.subTabPosition = sibling.tabPosition
            sibling.tabPosition = 0
          }
        }
        panel.tabPosition = originalTab
        panel.checked = true
        if (signal) this.postAction(panel, "RadioButtonChecked", { tabposition: originalTab })
        this.requestedFocus = panel.id
      } else if (!selected) {
        panel.checked = false
        if (panel.tabPosition !== 0) panel.subTabPosition = panel.tabPosition
        panel.tabPosition = 0
      }
    } else if (sameName(sourceControl, "CheckButton")) {
      panel.checked = selected
      if (signal) this.postAction(panel, "CheckButtonChecked", { state: selected ? 1 : 0 })
    }
    panel.selected = selected
    panel.depressed = panel.enabled && !panel.staySelected && panel.armed && panel.selected
  }

  private postAction(panel: PanelState, name: string, fields: Readonly<Record<string, VguiMessageValue>> = {}): void {
    const message = freezeMessage({ name, fields: Object.freeze({ ...fields, panel: panel.id }) })
    for (let index = panel.actionTargets.length - 1; index >= 0; index -= 1) {
      const target = panel.actionTargets[index]
      if (this.panels.has(target)) this.postMessage(target, panel.id, message, 0)
    }
  }

  private requestFocus(panelId: VguiPanelId | null): void {
    if (panelId !== null) {
      const panel = this.requirePanel(panelId)
      if (!this.keyboardEligible(panel.id)) throw new RuntimeFault("InvalidOperation", `${panel.name}:focus`)
    }
    this.requestedFocus = panelId
    this.clearFocusRequested = panelId === null
    this.calculatedKeyFocus = this.calculateKeyFocus()
  }

  private setDefaultButton(groupId: VguiPanelId, panelId: VguiPanelId | null): void {
    const group = this.requirePanel(groupId)
    if (panelId !== null) {
      const panel = this.requirePanel(panelId)
      if (!this.hasAncestor(panel.id, group.id) || !["Button", "CheckButton", "RadioButton"].some((name) => sameName(asSourceControl(panel.control), name))) {
        throw new RuntimeFault("InvalidOperation", `${group.name}:default-button`)
      }
    }
    if (group.currentDefaultButton !== null && this.panels.has(group.currentDefaultButton)) {
      this.postMessage(group.currentDefaultButton, group.id, { name: "SetAsCurrentDefaultButton", fields: { state: 0 } }, 0)
    }
    group.defaultButton = panelId
    group.currentDefaultButton = panelId
    if (panelId !== null) this.postMessage(panelId, group.id, { name: "SetAsCurrentDefaultButton", fields: { state: 1 } }, 0)
  }

  private setPointerCapture(panelId: VguiPanelId | null, initiatingButton: VguiPointerButton | null, pointerId: number | null): void {
    if (panelId !== null) {
      const panel = this.requirePanel(panelId)
      if (!this.pointerEligible(panel.id)) throw new RuntimeFault("InvalidOperation", `${panel.name}:capture`)
      if (pointerId === null || !safeInteger(pointerId) || pointerId < 0) throw new RuntimeFault("MalformedValue", `${panel.name}:pointerId`)
    }
    if (this.capture !== null && this.capture !== panelId) {
      const old = this.panels.get(this.capture)
      if (old) {
        old.dragging = false
        old.frameInteraction = null
        this.postMessage(old.id, null, { name: "MouseCaptureLost", fields: {} }, 0)
        try {
          if (this.pointerId !== null && old.element.hasPointerCapture?.(this.pointerId)) old.element.releasePointerCapture(this.pointerId)
        } catch {}
      }
    }
    this.capture = panelId
    this.captureButton = panelId === null ? null : initiatingButton
    this.pointerId = panelId === null ? null : pointerId
    if (panelId !== null && pointerId !== null) {
      try { this.requirePanel(panelId).element.setPointerCapture?.(pointerId) } catch {}
    }
    this.mouseFocus = this.capture ?? this.mouseOver
    this.addTrace("pointer-capture", panelId, initiatingButton ?? "none")
  }

  private setApplicationModal(panelId: VguiPanelId | null): void {
    if (panelId !== null) this.requirePanel(panelId)
    this.applicationModal = panelId
    if (!this.modalEligible(this.capture)) this.setPointerCapture(null, null, null)
    if (this.keyFocus !== null && !this.modalEligible(this.keyFocus)) { this.requestedFocus = null; this.clearFocusRequested = true }
    this.calculatedKeyFocus = this.calculateKeyFocus()
  }

  private setModalSubtree(panelId: VguiPanelId | null, restrict: boolean, listener: VguiPanelId | null): void {
    if (panelId !== null) this.requirePanel(panelId)
    if (listener !== null) this.requirePanel(listener)
    this.modalSubtree = panelId
    this.modalRestrict = restrict
    this.outsideClickListener = panelId === null ? null : listener
    if (!this.modalEligible(this.capture)) this.setPointerCapture(null, null, null)
    this.updateMouseOver()
    this.calculatedKeyFocus = this.calculateKeyFocus()
  }

  private inputEligible(panelId: VguiPanelId | null): boolean {
    return panelId !== null && this.keyboardEligible(panelId)
  }

  private pointerEligible(panelId: VguiPanelId | null): boolean {
    if (panelId === null) return false
    const panel = this.panels.get(panelId)
    if (!panel || !panel.effectivelyVisible || !panel.enabled || !panel.mouseInput || !this.modalEligible(panel.id)) return false
    for (let parentId = panel.parent; parentId !== null;) {
      const parent = this.panels.get(parentId)
      if (!parent || !parent.visible || !parent.mouseInput) return false
      parentId = parent.parent
    }
    return true
  }

  private keyboardEligible(panelId: VguiPanelId): boolean {
    const panel = this.panels.get(panelId)
    if (!panel || !panel.registration.focusable || !panel.effectivelyVisible || !panel.enabled || !panel.keyboardInput || !this.modalEligible(panel.id)) return false
    for (let parentId = panel.parent; parentId !== null;) {
      const parent = this.panels.get(parentId)
      if (!parent || !parent.visible || !parent.keyboardInput) return false
      parentId = parent.parent
    }
    return true
  }

  private modalEligible(panelId: VguiPanelId | null): boolean {
    if (panelId === null) return true
    if (this.applicationModal !== null && !this.hasAncestor(panelId, this.applicationModal)) return false
    if (this.modalSubtree !== null) {
      const child = this.hasAncestor(panelId, this.modalSubtree)
      return this.modalRestrict ? child : !child
    }
    return true
  }

  private calculateKeyFocus(): VguiPanelId | null {
    if (this.clearFocusRequested) return null
    if (this.requestedFocus !== null && this.keyboardEligible(this.requestedFocus)) return this.requestedFocus
    if (this.keyFocus !== null && this.keyboardEligible(this.keyFocus)) return this.keyFocus
    for (let index = this.popups.length - 1; index >= 0; index -= 1) {
      const popup = this.panels.get(this.popups[index])
      if (!popup || !popup.effectivelyVisible || !popup.keyboardInput || !this.modalEligible(popup.id)) continue
      return this.firstFocusable(popup.id) ?? (popup.registration.focusable ? popup.id : null)
    }
    return this.firstFocusable(this.rootPanel)
  }

  private firstFocusable(panelId: VguiPanelId): VguiPanelId | null {
    const panel = this.requirePanel(panelId)
    const ordered = panel.children
      .map((id) => this.requirePanel(id))
      .filter((child) => child.tabPosition > 0 && this.keyboardEligible(child.id))
      .sort((left, right) => left.tabPosition - right.tabPosition || left.tieOrder - right.tieOrder)
    if (ordered.length > 0) return ordered[0].id
    for (const childId of panel.children) {
      const nested = this.firstFocusable(childId)
      if (nested !== null) return nested
    }
    return null
  }

  private updateMouseOver(): void {
    const next = this.hitTest(this.pointerX, this.pointerY)
    if (next === this.mouseOver) {
      this.mouseFocus = this.capture ?? next
      return
    }
    const previous = this.mouseOver
    this.mouseOver = next
    if (previous !== null && (this.capture === null || previous === this.capture)) this.postMessage(previous, null, { name: "CursorExited", fields: {} }, 0)
    if (next !== null && (this.capture === null || next === this.capture)) this.postMessage(next, null, { name: "CursorEntered", fields: {} }, 0)
    this.mouseFocus = this.capture ?? next
  }

  private hitTest(x: number, y: number, ignoreModal = false): VguiPanelId | null {
    for (let index = this.popups.length - 1; index >= 0; index -= 1) {
      const popup = this.panels.get(this.popups[index])
      if (!popup || (!ignoreModal && !this.modalEligible(popup.id))) continue
      const hit = this.hitPanel(popup.id, x, y, ignoreModal)
      if (hit !== null) return hit
    }
    return this.hitPanel(this.rootPanel, x, y, ignoreModal)
  }

  private hitPanel(panelId: VguiPanelId, x: number, y: number, ignoreModal: boolean): VguiPanelId | null {
    const panel = this.panels.get(panelId)
    if (!panel || !panel.effectivelyVisible || !panel.mouseInput || (!ignoreModal && !this.modalEligible(panel.id))) return null
    const comboExpanded = sameName(asSourceControl(panel.control), "ComboBox") && panel.properties.get("expanded") === "1"
    const hitRect = comboExpanded
      ? { ...panel.clip, height: panel.clip.height + panel.items.length * Math.max(1, Number(panel.properties.get("itemheight") ?? 20)) }
      : panel.clip
    if (!inside(hitRect, x, y)) return null
    for (let index = panel.children.length - 1; index >= 0; index -= 1) {
      const child = this.requirePanel(panel.children[index])
      if (child.popup) continue
      const hit = this.hitPanel(child.id, x, y, ignoreModal)
      if (hit !== null) return hit
    }
    return panel.id
  }

  private pointerMove(x: number, y: number, pointerId: number): void {
    if (![x, y].every(finite) || !safeInteger(pointerId) || pointerId < 0) throw new RuntimeFault("MalformedValue", "pointer-move")
    this.pointerX = Math.trunc(x)
    this.pointerY = Math.trunc(y)
    this.pointerId = pointerId
    this.updateMouseOver()
    const target = this.capture ?? this.mouseFocus
    if (target !== null) this.postMessage(target, null, { name: "CursorMoved", fields: { xpos: this.pointerX, ypos: this.pointerY } }, 0)
    if (this.capture !== null) this.handleCapturedMove(this.requirePanel(this.capture))
  }

  private pointerPress(button: VguiPointerButton, x: number, y: number, pointerId: number, clicks: 1 | 2 | 3): void {
    this.pointerMove(x, y, pointerId)
    if (!["left", "right", "middle", "button4", "button5"].includes(button) || ![1, 2, 3].includes(clicks)) throw new RuntimeFault("MalformedValue", "pointer-press")
    this.pendingPressedButtons.add(button)
    this.downButtons.add(button)
    let target = this.capture ?? this.mouseFocus
    if (target === null && this.modalSubtree !== null && this.outsideClickListener !== null) {
      const unrestricted = this.hitTest(this.pointerX, this.pointerY, true)
      if (unrestricted !== null) {
        this.postMessage(this.outsideClickListener, unrestricted, { name: "UnhandledMouseClick", fields: { code: button } }, 0)
        target = this.outsideClickListener
      }
    }
    if (target !== null) {
      const messageName = clicks === 1 ? "MousePressed" : clicks === 2 ? "MouseDoublePressed" : "MouseTriplePressed"
      this.postMessage(target, null, { name: messageName, fields: { code: button, pointerId } }, 0)
      this.requestedFocus = this.focusTargetForPointer(target)
      this.clearFocusRequested = false
    }
  }

  private pointerRelease(button: VguiPointerButton, x: number, y: number, pointerId: number): void {
    this.pointerMove(x, y, pointerId)
    this.pendingReleasedButtons.add(button)
    this.downButtons.delete(button)
    const target = this.capture ?? this.mouseFocus
    if (target !== null) this.postMessage(target, null, { name: "MouseReleased", fields: { code: button, pointerId } }, 0)
    if (this.capture !== null && (this.captureButton === null || this.captureButton === button)) this.setPointerCapture(null, null, null)
  }

  private pointerWheel(delta: number, x: number, y: number): void {
    if (!finite(delta) || !finite(x) || !finite(y)) throw new RuntimeFault("MalformedValue", "pointer-wheel")
    this.pointerX = Math.trunc(x)
    this.pointerY = Math.trunc(y)
    this.updateMouseOver()
    if (this.mouseFocus !== null) this.postMessage(this.mouseFocus, null, { name: "MouseWheeled", fields: { delta: Math.trunc(delta) } }, 0)
  }

  private focusTargetForPointer(panelId: VguiPanelId): VguiPanelId | null {
    let panel: PanelState | undefined = this.panels.get(panelId)
    while (panel) {
      if (this.keyboardEligible(panel.id)) return panel.id
      panel = panel.parent === null ? undefined : this.panels.get(panel.parent)
    }
    return null
  }

  private keyPress(key: string, shift: boolean, control: boolean, alt: boolean, meta: boolean, repeat: boolean): void {
    if (!validString(key, 64, false) || ![shift, control, alt, meta, repeat].every((value) => typeof value === "boolean")) throw new RuntimeFault("MalformedValue", "key-press")
    this.pendingPressedKeys.add(key)
    this.downKeys.add(key)
    const target = this.calculateKeyFocus()
    if (target !== null) this.postMessage(target, null, { name: "KeyCodePressed", fields: { code: key, shift, control, alt, meta, repeat } }, 0)
  }

  private keyTyped(key: string, shift: boolean, control: boolean, alt: boolean, meta: boolean): void {
    if (!validString(key, 64, false)) throw new RuntimeFault("MalformedValue", "key-typed")
    const target = this.calculateKeyFocus()
    if (target !== null) this.postMessage(target, null, { name: "KeyCodeTyped", fields: { code: key, shift, control, alt, meta } }, 0)
  }

  private keyRelease(key: string, shift: boolean, control: boolean, alt: boolean, meta: boolean): void {
    if (!validString(key, 64, false)) throw new RuntimeFault("MalformedValue", "key-release")
    this.pendingReleasedKeys.add(key)
    this.downKeys.delete(key)
    const target = this.calculateKeyFocus()
    if (target !== null) this.postMessage(target, null, { name: "KeyCodeReleased", fields: { code: key, shift, control, alt, meta } }, 0)
  }

  private textInput(text: string): void {
    if (!validString(text, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", "text-input")
    const target = this.calculateKeyFocus()
    if (target === null) return
    const panel = this.requirePanel(target)
    if (!["TextEntry", "ComboBox"].some((name) => sameName(asSourceControl(panel.control), name)) || !panel.editable) return
    this.insertText(panel, text)
    this.postAction(panel, "TextChanged", {})
    this.publishDom()
  }

  private compositionStart(): void {
    const panel = this.focusedTextPanel()
    if (!panel) return
    panel.compositionActive = true
    panel.compositionText = ""
    panel.compositionCaret = 0
  }

  private compositionUpdate(text: string, caret: number): void {
    const panel = this.focusedTextPanel()
    if (!panel || !panel.compositionActive || !validString(text, this.limits.maxTextCodeUnits) || !safeInteger(caret) || caret < 0 || caret > text.length) {
      throw new RuntimeFault("InvalidOperation", "composition-update")
    }
    panel.compositionText = text
    panel.compositionCaret = caret
  }

  private compositionEnd(text: string): void {
    const panel = this.focusedTextPanel()
    if (!panel || !panel.compositionActive) throw new RuntimeFault("InvalidOperation", "composition-end")
    panel.compositionActive = false
    panel.compositionText = ""
    panel.compositionCaret = 0
    this.insertText(panel, text)
    this.postAction(panel, "TextChanged", {})
    this.publishDom()
  }

  private focusedTextPanel(): PanelState | null {
    if (this.keyFocus === null) return null
    const panel = this.panels.get(this.keyFocus)
    if (!panel || !["TextEntry", "ComboBox"].some((name) => sameName(asSourceControl(panel.control), name))) return null
    return panel
  }

  private insertText(panel: PanelState, input: string): void {
    let admitted = ""
    for (const character of input) {
      if (character === "\r" || character === "\t" || (!panel.multiline && character === "\n")) continue
      if (panel.numericOnly && !/^\p{Nd}$|^\.$/u.test(character)) {
        this.pendingRequests.push(Object.freeze({ kind: "sound", panel: panel.id, logicalIdentity: "Resource/warning.wav" }))
        continue
      }
      if (!panel.allowUnicode && character.codePointAt(0)! > 127) continue
      admitted += character
    }
    const start = panel.selectionStart >= 0 ? Math.min(panel.selectionStart, panel.selectionEnd) : panel.caret
    const end = panel.selectionStart >= 0 ? Math.max(panel.selectionStart, panel.selectionEnd) : panel.caret
    const next = panel.text.slice(0, start) + admitted + panel.text.slice(end)
    const maximum = panel.maximumCharacters < 0 ? this.limits.maxTextCodeUnits : Math.min(panel.maximumCharacters, this.limits.maxTextCodeUnits)
    if (next.length > maximum) return
    panel.text = next
    panel.textSource = next
    panel.caret = start + admitted.length
    panel.selectionStart = -1
    panel.selectionEnd = panel.caret
  }

  private deleteSelection(panel: PanelState): boolean {
    if (panel.selectionStart < 0 || panel.selectionStart === panel.selectionEnd) return false
    const start = Math.min(panel.selectionStart, panel.selectionEnd)
    const end = Math.max(panel.selectionStart, panel.selectionEnd)
    panel.text = panel.text.slice(0, start) + panel.text.slice(end)
    panel.textSource = panel.text
    panel.caret = start
    panel.selectionStart = -1
    panel.selectionEnd = start
    return true
  }

  private selectionText(panel: PanelState): string {
    if (panel.selectionStart < 0) return ""
    const start = Math.min(panel.selectionStart, panel.selectionEnd)
    const end = Math.max(panel.selectionStart, panel.selectionEnd)
    return panel.text.slice(start, end)
  }

  private postMessage(target: VguiPanelId, source: VguiPanelId | null, message: VguiMessage, delaySeconds: number): void {
    this.requirePanel(target)
    if (source !== null) this.requirePanel(source)
    if (!message || !validString(message.name, 255, false) || !message.fields || typeof message.fields !== "object") throw new RuntimeFault("MalformedValue", "message")
    if (!finite(delaySeconds) || delaySeconds < 0) throw new RuntimeFault("MalformedValue", `${message.name}:delay`)
    for (const [name, value] of Object.entries(message.fields)) {
      if (!validString(name, 255, false) || !["string", "number", "boolean"].includes(typeof value) && value !== null) throw new RuntimeFault("MalformedValue", `${message.name}:field`)
      if (typeof value === "string" && !validString(value, this.limits.maxStringCodeUnits)) throw new RuntimeFault("TextLimit", `${message.name}:${name}`)
      if (typeof value === "number" && !finite(value)) throw new RuntimeFault("MalformedValue", `${message.name}:${name}`)
    }
    if (this.queuedMessages.length >= this.limits.maxQueuedMessages) throw new RuntimeFault("MessageLimit", message.name)
    this.queuedMessages.push({ order: this.nextOrder++, due: this.timeSeconds + delaySeconds, target, source, message: freezeMessage(message) })
  }

  private dispatchMessages(): void {
    for (let index = 0; index < this.queuedMessages.length;) {
      const queued = this.queuedMessages[index]
      if (queued.due > this.timeSeconds) { index += 1; continue }
      this.queuedMessages.splice(index, 1)
      const panel = this.panels.get(queued.target)
      if (!panel || (queued.source !== null && !this.panels.has(queued.source))) continue
      this.dispatchMessage(panel, queued.message, queued.source)
      index = 0
    }
  }

  private dispatchMessage(panel: PanelState, message: VguiMessage, source: VguiPanelId | null): void {
    this.addTrace("message", panel.id, message.name)
    const sourceControl = asSourceControl(panel.control)
    if (sameName(message.name, "CursorEntered")) {
      if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(sourceControl, name)) && panel.enabled && !panel.selected) {
        const changed = !panel.armed
        panel.armed = true
        const sound = panel.properties.get("sound_armed")
        if (changed && sound) this.pendingRequests.push(Object.freeze({ kind: "sound", panel: panel.id, logicalIdentity: sound }))
      }
      this.reapplyPanelPresentation(panel)
      return
    }
    if (sameName(message.name, "CursorExited")) {
      if (!panel.selected) panel.armed = false
      panel.depressed = false
      this.reapplyPanelPresentation(panel)
      return
    }
    if (sameName(message.name, "MousePressed") || sameName(message.name, "MouseDoublePressed") || sameName(message.name, "MouseTriplePressed")) {
      this.controlMousePressed(panel, String(message.fields.code ?? "left") as VguiPointerButton, Number(message.fields.pointerId ?? this.pointerId ?? 0))
      return
    }
    if (sameName(message.name, "MouseReleased")) {
      this.controlMouseReleased(panel, String(message.fields.code ?? "left") as VguiPointerButton)
      return
    }
    if (sameName(message.name, "MouseWheeled")) {
      const delta = Number(message.fields.delta ?? 0)
      if (["ScrollBar", "ListPanel", "Menu", "RichText"].some((name) => sameName(sourceControl, name))) {
        this.setControlValue(panel, panel.value - delta, true)
      } else if (panel.parent !== null) {
        this.postMessage(panel.parent, panel.id, message, 0)
      }
      return
    }
    if (sameName(message.name, "KeyCodePressed")) {
      this.controlKeyPressed(panel, String(message.fields.code ?? ""), Boolean(message.fields.shift), Boolean(message.fields.control || message.fields.meta))
      return
    }
    if (sameName(message.name, "KeyCodeTyped")) {
      this.controlKeyTyped(panel, String(message.fields.code ?? ""), Boolean(message.fields.shift), Boolean(message.fields.control || message.fields.meta), Boolean(message.fields.alt))
      return
    }
    if (sameName(message.name, "KeyCodeReleased")) {
      this.controlKeyReleased(panel, String(message.fields.code ?? ""))
      return
    }
    if (sameName(message.name, "Hotkey")) {
      if (sameName(sourceControl, "Label")) {
        const associate = panel.properties.get("associate")
        const target = associate ? this.findByName(this.rootPanel, associate) : null
        if (target && this.keyboardEligible(target.id)) this.requestedFocus = target.id
        return
      }
      this.clickControl(panel)
      return
    }
    if (sameName(message.name, "SetFocus")) {
      if (panel.selectAllOnFirstFocus && panel.firstFocus) {
        panel.selectionStart = panel.text.length > 0 ? 0 : -1
        panel.selectionEnd = panel.text.length
        panel.caret = panel.text.length
        panel.firstFocus = false
      }
      return
    }
    if (sameName(message.name, "KillFocus")) {
      panel.dragging = false
      panel.frameInteraction = null
      panel.frameClosePressed = false
      return
    }
    if (sameName(message.name, "Command")) {
      const command = String(message.fields.command ?? "")
      if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(sourceControl, name))) {
        if (sameName(command, "Close") || sameName(command, "CloseModal")) {
          this.closeFrame(panel)
          return
        }
        if (sameName(sourceControl, "QueryBox") && (sameName(command, "OK") || sameName(command, "Cancel"))) {
          this.closeQuery(panel, sameName(command, "OK"))
          return
        }
        if (sameName(sourceControl, "MessageBox") && (sameName(command, "OnOk") || sameName(command, "OnCancel"))) {
          this.closeQuery(panel, sameName(command, "OnOk"))
          return
        }
      }
      if (command) this.pendingRequests.push(Object.freeze({ kind: "command", panel: source ?? panel.id, command }))
      return
    }
    if (sameName(message.name, "ResetData") || sameName(message.name, "ApplyChanges") || sameName(message.name, "PageShow") || sameName(message.name, "PageHide")) return
    if (sameName(message.name, "SetAsCurrentDefaultButton")) {
      panel.properties.set("currentDefault", String(message.fields.state ?? 0))
      return
    }
    if (sameName(message.name, "MouseCaptureLost")) {
      panel.dragging = false
      panel.frameInteraction = null
      panel.frameClosePressed = false
      panel.selected = false
      panel.depressed = false
      return
    }
    this.pendingRequests.push(Object.freeze({ kind: "message", target: panel.id, source, message: freezeMessage(message) }))
  }

  private controlMousePressed(panel: PanelState, button: VguiPointerButton, pointerId: number): void {
    if (!panel.enabled || button !== "left") return
    const control = asSourceControl(panel.control)
    if (sameName(control, "URLLabel")) {
      if (panel.url) this.pendingRequests.push(Object.freeze({ kind: "external-open", panel: panel.id, url: panel.url }))
      return
    }
    if (sameName(control, "Label")) {
      const associate = panel.properties.get("associate")
      const target = associate ? this.findByName(this.rootPanel, associate) : null
      if (target && this.keyboardEligible(target.id)) this.requestedFocus = target.id
      return
    }
    if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(control, name))) {
      const dialogButton = this.frameDialogButton(panel)
      if (dialogButton !== null) {
        panel.pressedItem = dialogButton === "ok" ? -1 : -2
        this.setPointerCapture(panel.id, "left", pointerId)
        return
      }
      if (this.withinFrameClose(panel)) {
        panel.frameClosePressed = true
        this.setPointerCapture(panel.id, "left", pointerId)
        return
      }
      if (this.beginFrameInteraction(panel, pointerId)) return
    }
    if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(control, name))) {
      if (panel.activation === 0) {
        this.requestedFocus = panel.keyboardInput ? panel.id : this.requestedFocus
        this.clickControl(panel)
      } else if (panel.activation === 2) {
        this.requestedFocus = panel.keyboardInput ? panel.id : this.requestedFocus
        this.setSelected(panel, true, false)
        panel.depressed = true
        this.setPointerCapture(panel.id, button, pointerId)
        const sound = panel.properties.get("sound_depressed")
        if (sound) this.pendingRequests.push(Object.freeze({ kind: "sound", panel: panel.id, logicalIdentity: sound }))
      }
      this.reapplyPanelPresentation(panel)
      return
    }
    if (sameName(control, "Slider")) {
      this.sliderPress(panel, pointerId)
      return
    }
    if (sameName(control, "ScrollBar")) {
      this.scrollBarPress(panel, pointerId)
      return
    }
    if (sameName(control, "ListPanel")) {
      const rowHeight = Math.max(1, Number(panel.properties.get("linespacing") ?? 20))
      const index = Math.floor((this.pointerY - panel.absoluteBounds.y) / rowHeight)
      if (index >= 0 && index < panel.items.length && panel.items[index].enabled) {
        panel.activeIndex = index
        this.postAction(panel, "ItemSelected", { itemID: panel.items[index].id })
      }
    } else if (sameName(control, "Menu") || sameName(control, "ComboBox")) {
      const rowHeight = Math.max(1, Number(panel.properties.get("itemheight") ?? 20))
      const localY = this.pointerY - panel.absoluteBounds.y
      if (sameName(control, "ComboBox") && localY < panel.bounds.height) {
        panel.properties.set("expanded", panel.properties.get("expanded") === "1" ? "0" : "1")
        this.requestedFocus = panel.id
        return
      }
      const index = Math.floor((localY - (sameName(control, "ComboBox") ? panel.bounds.height : 0)) / rowHeight)
      if (index >= 0 && index < panel.items.length && panel.items[index].enabled) {
        panel.activeIndex = index
        panel.pressedItem = index
      }
    } else if (sameName(control, "PropertySheet")) {
      const tabWidth = Math.max(1, Number(panel.properties.get("tabwidth") ?? (panel.items.length > 0 ? panel.bounds.width / panel.items.length : panel.bounds.width)))
      const index = Math.floor((this.pointerX - panel.absoluteBounds.x) / tabWidth)
      if (index >= 0 && index < panel.items.length && panel.items[index].enabled) {
        panel.activeIndex = index
        this.activatePropertyPage(panel)
      }
    }
  }

  private controlMouseReleased(panel: PanelState, button: VguiPointerButton): void {
    if (button !== "left") return
    const control = asSourceControl(panel.control)
    if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(control, name))) {
      if (panel.activation !== 0) {
        const admitted = panel.enabled && (this.mouseOver === panel.id || this.hasAncestor(this.mouseOver ?? -1, panel.id))
        if ((panel.activation !== 2 || panel.selected) && admitted) this.clickControl(panel)
        else if (!panel.staySelected) this.setSelected(panel, false, false)
      }
      panel.depressed = false
      this.reapplyPanelPresentation(panel)
    } else if (sameName(control, "Slider")) {
      const wasDragging = panel.dragging
      panel.dragging = false
      if (panel.enabled) this.postAction(panel, "SliderDragEnd", { position: panel.value })
      if (wasDragging) this.addTrace("slider-drag-end", panel.id, String(panel.value))
    } else if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(control, name))) {
      if (panel.pressedItem === -1 || panel.pressedItem === -2) {
        const expected = panel.pressedItem === -1 ? "ok" : "cancel"
        const selected = this.frameDialogButton(panel)
        panel.pressedItem = null
        if (selected === expected) this.closeQuery(panel, expected === "ok")
      }
      if (panel.frameClosePressed) {
        const close = this.withinFrameClose(panel)
        panel.frameClosePressed = false
        if (close) this.closeFrame(panel)
      }
      panel.frameInteraction = null
    } else if ((sameName(control, "Menu") || sameName(control, "ComboBox")) && panel.pressedItem !== null) {
      const index = panel.pressedItem
      panel.pressedItem = null
      if (index === panel.activeIndex) this.activateItem(panel, index)
    }
  }

  private clickControl(panel: PanelState): void {
    const control = asSourceControl(panel.control)
    if (sameName(control, "RadioButton")) {
      this.setSelected(panel, true, true)
    } else if (sameName(control, "CheckButton")) {
      this.setSelected(panel, !panel.selected, true)
    } else if (sameName(control, "MenuItem")) {
      if (panel.checkable) panel.checked = !panel.checked
      this.postAction(panel, "MenuItemSelected", {})
    } else {
      this.setSelected(panel, true, false)
    }
    if (panel.command && !sameName(control, "RadioButton")) {
      this.postAction(panel, "Command", { command: panel.command })
    }
    if (sameName(control, "CheckButton")) this.postAction(panel, "ButtonToggled", { state: panel.selected ? 1 : 0 })
    const releaseSound = panel.properties.get("sound_released")
    if (releaseSound && !["CheckButton", "RadioButton"].some((name) => sameName(control, name))) {
      this.pendingRequests.push(Object.freeze({ kind: "sound", panel: panel.id, logicalIdentity: releaseSound }))
    }
    if (!panel.staySelected && !["CheckButton", "RadioButton"].some((name) => sameName(control, name))) this.setSelected(panel, false, false)
    if (!panel.stayArmed) panel.armed = false
    this.reapplyPanelPresentation(panel)
  }

  private controlKeyPressed(panel: PanelState, key: string, shift: boolean, control: boolean): void {
    const sourceControl = asSourceControl(panel.control)
    if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(sourceControl, name)) && (key === "Enter" || key === "Space")) {
      panel.armed = true
      panel.properties.set("keyDown", "1")
      if (panel.activation !== 1) this.clickControl(panel)
      return
    }
    if (["TextEntry", "ComboBox"].some((name) => sameName(sourceControl, name))) this.editKey(panel, key, shift, control)
  }

  private controlKeyReleased(panel: PanelState, key: string): void {
    const sourceControl = asSourceControl(panel.control)
    if (["Button", "CheckButton", "RadioButton", "MenuItem"].some((name) => sameName(sourceControl, name))
      && panel.properties.get("keyDown") === "1" && (key === "Enter" || key === "Space")) {
      if (panel.activation !== 0) this.clickControl(panel)
      panel.properties.delete("keyDown")
      panel.armed = false
    }
  }

  private controlKeyTyped(panel: PanelState, key: string, shift: boolean, control: boolean, alt: boolean): void {
    const sourceControl = asSourceControl(panel.control)
    if (key === "Tab") {
      this.navigateTab(panel, shift ? -1 : 1)
      return
    }
    if ((sameName(sourceControl, "Menu") || sameName(sourceControl, "ComboBox"))
      && ["Escape", "ArrowDown", "ArrowUp", "Home", "End", "Enter"].includes(key)) {
      this.menuKey(panel, key)
      return
    }
    if (key === "Enter") {
      if (sameName(sourceControl, "QueryBox") || sameName(sourceControl, "MessageBox")) {
        this.closeQuery(panel, true)
        return
      }
      const group = this.focusGroup(panel)
      const target = group.currentDefaultButton ?? group.defaultButton
      if (target !== null && this.keyboardEligible(target)) this.postMessage(target, panel.id, { name: "Hotkey", fields: {} }, 0)
      return
    }
    if (key === "Escape") {
      if (sameName(sourceControl, "QueryBox")) this.closeQuery(panel, false)
      else if (sameName(sourceControl, "Menu")) panel.visible = false
      else if (panel.popup && this.applicationModal === panel.id) {
        this.applicationModal = null
        panel.visible = false
      }
      return
    }
    if (alt && key.length === 1) {
      const group = this.focusGroup(panel)
      const hotkey = key.toLocaleLowerCase("en-US")
      const target = group.children.map((id) => this.requirePanel(id)).find((child) => this.hotkey(child.textSource) === hotkey && child.visible && child.enabled)
      if (target) this.postMessage(target.id, panel.id, { name: "Hotkey", fields: {} }, 0)
      return
    }
    if (sameName(sourceControl, "Slider")) {
      this.sliderKey(panel, key)
      return
    }
    if (sameName(sourceControl, "PropertySheet") && (key === "ArrowLeft" || key === "ArrowRight")) {
      this.changeActiveItem(panel, key === "ArrowRight" ? 1 : -1)
      return
    }
    if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(key)) this.navigateDirection(panel, key)
  }

  private beginFrameInteraction(panel: PanelState, pointerId: number): boolean {
    const moveable = panel.properties.get("moveable") !== "0"
    const sizeable = panel.properties.get("sizeable") !== "0"
    const localX = this.pointerX - panel.absoluteBounds.x
    const localY = this.pointerY - panel.absoluteBounds.y
    const edge = Math.max(1, panel.proportional ? this.proportional(5, panel) : 5)
    let horizontal = ""
    let vertical = ""
    if (sizeable) {
      if (localX < edge) horizontal = "w"
      else if (localX >= panel.bounds.width - edge) horizontal = "e"
      if (localY < edge) vertical = "n"
      else if (localY >= panel.bounds.height - edge) vertical = "s"
    }
    const resize = `${vertical}${horizontal}` as PanelState["frameInteraction"]
    if (resize) panel.frameInteraction = resize
    else if (moveable && localY < Math.max(edge, panel.proportional ? this.proportional(23, panel) : 23)) panel.frameInteraction = "move"
    else return false
    panel.frameStartBounds = { ...panel.bounds }
    panel.frameStartPointer = Object.freeze([this.pointerX, this.pointerY])
    this.setPointerCapture(panel.id, "left", pointerId)
    return true
  }

  private withinFrameClose(panel: PanelState): boolean {
    if (panel.properties.get("setclosebuttonvisible") === "0" || panel.properties.get("settitlebarvisible") === "0") return false
    const scale = panel.proportional ? this.viewport.height / 480 : 1
    const side = Math.trunc(15 * scale)
    const x = panel.bounds.width - Math.trunc(25 * scale)
    const y = Math.trunc(8 * scale)
    const localX = this.pointerX - panel.absoluteBounds.x
    const localY = this.pointerY - panel.absoluteBounds.y
    return localX >= x && localX < x + side && localY >= y && localY < y + side
  }

  private frameDialogButton(panel: PanelState): "ok" | "cancel" | null {
    const control = asSourceControl(panel.control)
    if (!["MessageBox", "QueryBox"].some((name) => sameName(control, name))) return null
    const scale = panel.proportional ? this.viewport.height / 480 : 1
    const width = Math.trunc(64 * scale)
    const height = Math.trunc(24 * scale)
    const y = panel.bounds.height - height - Math.trunc(15 * scale)
    const query = sameName(control, "QueryBox")
    const okX = query ? Math.trunc(panel.bounds.width / 2) - width - Math.trunc(scale) : Math.trunc((panel.bounds.width - width) / 2)
    const cancelX = Math.trunc(panel.bounds.width / 2) + Math.trunc(16 * scale)
    const localX = this.pointerX - panel.absoluteBounds.x
    const localY = this.pointerY - panel.absoluteBounds.y
    if (localY < y || localY >= y + height) return null
    if (localX >= okX && localX < okX + width) return "ok"
    if (query && localX >= cancelX && localX < cancelX + width) return "cancel"
    return null
  }

  private handleCapturedMove(panel: PanelState): void {
    if (panel.frameInteraction) {
      const dx = this.pointerX - panel.frameStartPointer[0]
      const dy = this.pointerY - panel.frameStartPointer[1]
      const initial = panel.frameStartBounds
      let x = initial.x
      let y = initial.y
      let width = initial.width
      let height = initial.height
      if (panel.frameInteraction === "move") {
        x += dx
        y += dy
      } else {
        if (panel.frameInteraction.includes("w")) { x += dx; width -= dx }
        if (panel.frameInteraction.includes("e")) width += dx
        if (panel.frameInteraction.includes("n")) { y += dy; height -= dy }
        if (panel.frameInteraction.includes("s")) height += dy
        if (width < panel.minimumWidth) {
          if (panel.frameInteraction.includes("w")) x -= panel.minimumWidth - width
          width = panel.minimumWidth
        }
        if (height < panel.minimumHeight) {
          if (panel.frameInteraction.includes("n")) y -= panel.minimumHeight - height
          height = panel.minimumHeight
        }
      }
      x = Math.max(0, Math.min(this.viewport.width - width, x))
      y = Math.max(0, Math.min(this.viewport.height - height, y))
      panel.bounds = { x, y, width, height }
      this.resizeChildren(panel)
      this.solveGeometry()
      return
    }
    if (panel.dragging) {
      const control = asSourceControl(panel.control)
      if (sameName(control, "Slider")) {
        const usable = Math.max(1, panel.bounds.width - panel.thumbWidth)
        const range = panel.maximum - panel.minimum
        const value = panel.minimum + Math.trunc((this.pointerX - panel.absoluteBounds.x) / usable * range)
        this.setControlValue(panel, value, true)
      } else if (sameName(control, "ScrollBar")) {
        const vertical = sameName(panel.control, "ScrollBar_Vertical")
        const length = vertical ? panel.bounds.height : panel.bounds.width
        const coordinate = vertical ? this.pointerY - panel.absoluteBounds.y : this.pointerX - panel.absoluteBounds.x
        const movable = Math.max(1, panel.maximum - panel.minimum - panel.rangeWindow)
        this.setControlValue(panel, panel.minimum + Math.round(coordinate / Math.max(1, length - 1) * movable), true)
      }
    }
  }

  private sliderPress(panel: PanelState, pointerId: number): void {
    const local = this.pointerX - panel.absoluteBounds.x
    const width = Math.max(1, panel.bounds.width)
    const range = panel.maximum - panel.minimum
    const fraction = Math.max(0, Math.min(1, local / Math.max(1, width - 1)))
    const value = panel.minimum + Math.round(fraction * range)
    const thumbStart = range === 0 ? 0 : Math.round((panel.value - panel.minimum) / range * Math.max(0, width - panel.thumbWidth))
    const onThumb = local >= thumbStart && local < thumbStart + panel.thumbWidth
    const dragOnReposition = panel.properties.get("drag_on_reposition") === "1"
    if (!onThumb) this.setControlValue(panel, value, true)
    if (onThumb || dragOnReposition) {
      panel.dragging = true
      panel.dragStartCoordinate = this.pointerX
      panel.dragStartValue = panel.value
      this.postAction(panel, "SliderDragStart", { position: panel.value })
      this.setPointerCapture(panel.id, "left", pointerId)
    }
    this.requestedFocus = panel.id
  }

  private scrollBarPress(panel: PanelState, pointerId: number): void {
    const vertical = sameName(panel.control, "ScrollBar_Vertical")
    const length = vertical ? panel.bounds.height : panel.bounds.width
    const coordinate = vertical ? this.pointerY - panel.absoluteBounds.y : this.pointerX - panel.absoluteBounds.x
    const itemRange = panel.maximum - panel.minimum
    if (itemRange <= panel.rangeWindow || itemRange <= 0) return
    const proportion = panel.rangeWindow / itemRange
    const thumbSize = Math.max(vertical ? panel.bounds.width : panel.bounds.height, (length - 1) * proportion)
    const free = Math.max(0, length - 1 - thumbSize)
    const denominator = itemRange - panel.rangeWindow
    const thumbStart = denominator === 0 ? 0 : free * (panel.value - panel.minimum) / denominator
    if (coordinate >= thumbStart && coordinate < thumbStart + thumbSize) {
      panel.dragging = true
      this.setPointerCapture(panel.id, "left", pointerId)
    } else if (coordinate < thumbStart) {
      this.setControlValue(panel, panel.value - panel.rangeWindow, true)
    } else {
      this.setControlValue(panel, panel.value + panel.rangeWindow, true)
    }
  }

  private sliderKey(panel: PanelState, key: string): void {
    if (key === "ArrowLeft" || key === "ArrowDown") this.setControlValue(panel, panel.value - 1, true)
    else if (key === "ArrowRight" || key === "ArrowUp") this.setControlValue(panel, panel.value + 1, true)
    else if (key === "PageDown") this.setControlValue(panel, panel.value - Math.trunc((panel.maximum - panel.minimum) / panel.numTicks), true)
    else if (key === "PageUp") this.setControlValue(panel, panel.value + Math.trunc((panel.maximum - panel.minimum) / panel.numTicks), true)
    else if (key === "Home") this.setControlValue(panel, panel.minimum, true)
    else if (key === "End") this.setControlValue(panel, panel.maximum, true)
  }

  private editKey(panel: PanelState, key: string, shift: boolean, control: boolean): void {
    const anchor = panel.selectionStart >= 0 ? panel.selectionStart : panel.caret
    const setCaret = (position: number): void => {
      const next = Math.max(0, Math.min(panel.text.length, position))
      if (shift) {
        if (panel.selectionStart < 0) panel.selectionStart = anchor
        panel.selectionEnd = next
      } else {
        panel.selectionStart = -1
        panel.selectionEnd = next
      }
      panel.caret = next
    }
    if (control && key.toLowerCase() === "a") {
      panel.selectionStart = panel.text.length > 0 ? 0 : -1
      panel.selectionEnd = panel.text.length
      panel.caret = panel.text.length
      return
    }
    if (control && ["c", "x"].includes(key.toLowerCase())) {
      const text = this.selectionText(panel)
      if (text) this.pendingRequests.push(Object.freeze({ kind: "clipboard-write", panel: panel.id, text }))
      if (key.toLowerCase() === "x" && panel.editable && this.deleteSelection(panel)) this.postAction(panel, "TextChanged", {})
      return
    }
    if (control && key.toLowerCase() === "v") {
      const requestId = this.nextRequestId++
      this.pendingClipboardReads.set(requestId, panel.id)
      this.pendingRequests.push(Object.freeze({ kind: "clipboard-read", panel: panel.id, requestId }))
      return
    }
    if (!panel.editable && !["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) return
    if (key === "ArrowLeft") setCaret(panel.caret - 1)
    else if (key === "ArrowRight") setCaret(panel.caret + 1)
    else if (key === "Home") setCaret(0)
    else if (key === "End") setCaret(panel.text.length)
    else if (key === "Backspace") {
      if (!this.deleteSelection(panel) && panel.caret > 0) {
        panel.text = panel.text.slice(0, panel.caret - 1) + panel.text.slice(panel.caret)
        panel.caret -= 1
        panel.textSource = panel.text
      }
      this.postAction(panel, "TextChanged", {})
    } else if (key === "Delete") {
      if (!this.deleteSelection(panel) && panel.caret < panel.text.length) {
        panel.text = panel.text.slice(0, panel.caret) + panel.text.slice(panel.caret + 1)
        panel.textSource = panel.text
      }
      this.postAction(panel, "TextChanged", {})
    }
  }

  private focusGroup(panel: PanelState): PanelState {
    let current = panel
    while (current.parent !== null) {
      const parent = this.requirePanel(current.parent)
      if (["EditablePanel", "Frame", "PropertySheet", "MessageBox", "QueryBox"].some((name) => sameName(asSourceControl(parent.control), name))) return parent
      current = parent
    }
    return this.requirePanel(this.rootPanel)
  }

  private navigateTab(panel: PanelState, direction: -1 | 1): void {
    const group = this.focusGroup(panel)
    const candidates = group.children
      .map((id) => this.requirePanel(id))
      .filter((candidate) => candidate.tabPosition > 0 && this.keyboardEligible(candidate.id))
      .sort((left, right) => left.tabPosition - right.tabPosition || left.tieOrder - right.tieOrder)
    if (candidates.length === 0) return
    const current = candidates.findIndex((candidate) => candidate.id === panel.id)
    const next = current < 0 ? direction > 0 ? 0 : candidates.length - 1 : (current + direction + candidates.length) % candidates.length
    this.requestedFocus = candidates[next].id
    if (["Button", "CheckButton", "RadioButton"].some((name) => sameName(asSourceControl(candidates[next].control), name))) group.currentDefaultButton = candidates[next].id
    else group.currentDefaultButton = group.defaultButton
  }

  private navigateDirection(panel: PanelState, key: string): void {
    const field = key === "ArrowUp" ? "navup" : key === "ArrowDown" ? "navdown" : key === "ArrowLeft" ? "navleft" : "navright"
    const seen = new Set<VguiPanelId>()
    let current = panel
    while (!seen.has(current.id)) {
      seen.add(current.id)
      const targetName = current.nav.get(field) ?? current.nav.get("navtorelay")
      if (!targetName) return
      const target = this.findByName(this.rootPanel, targetName)
      if (!target) return
      if (this.keyboardEligible(target.id)) {
        this.requestedFocus = target.id
        return
      }
      current = target
    }
  }

  private menuKey(panel: PanelState, key: string): void {
    const combo = sameName(asSourceControl(panel.control), "ComboBox")
    if (key === "Escape") {
      if (combo) panel.properties.set("expanded", "0")
      else panel.visible = false
      return
    }
    if (combo && (key === "ArrowDown" || key === "ArrowUp")) panel.properties.set("expanded", "1")
    if (key === "ArrowDown") this.changeActiveItem(panel, 1)
    else if (key === "ArrowUp") this.changeActiveItem(panel, -1)
    else if (key === "Home") panel.activeIndex = this.nextEnabledItem(panel, -1, 1)
    else if (key === "End") panel.activeIndex = this.nextEnabledItem(panel, panel.items.length, -1)
    else if (key === "Enter" && panel.activeIndex !== null) this.activateItem(panel, panel.activeIndex)
  }

  private changeActiveItem(panel: PanelState, direction: -1 | 1): void {
    const start = panel.activeIndex ?? (direction > 0 ? -1 : panel.items.length)
    panel.activeIndex = this.nextEnabledItem(panel, start, direction)
    if (panel.activeIndex !== null && sameName(asSourceControl(panel.control), "ComboBox")) {
      panel.textSource = panel.items[panel.activeIndex].text
      this.refreshText(panel)
    }
    if (sameName(asSourceControl(panel.control), "PropertySheet")) this.activatePropertyPage(panel)
  }

  private activatePropertyPage(sheet: PanelState): void {
    for (let index = 0; index < sheet.items.length; index += 1) {
      const page = this.panels.get(sheet.items[index].id)
      if (!page) continue
      const visible = index === sheet.activeIndex
      if (page.visible !== visible) this.postMessage(page.id, sheet.id, { name: visible ? "PageShow" : "PageHide", fields: {} }, 0)
      page.visible = visible
    }
  }

  private nextEnabledItem(panel: PanelState, start: number, direction: -1 | 1): number | null {
    if (panel.items.length === 0) return null
    for (let offset = 1; offset <= panel.items.length; offset += 1) {
      const index = (start + direction * offset + panel.items.length) % panel.items.length
      if (panel.items[index].enabled) return index
    }
    return null
  }

  private activateItem(panel: PanelState, index: number): void {
    const item = panel.items[index]
    if (!item?.enabled) return
    panel.activeIndex = index
    panel.textSource = item.text
    this.refreshText(panel)
    if (item.command) this.pendingRequests.push(Object.freeze({ kind: "command", panel: panel.id, command: item.command }))
    this.postAction(panel, "MenuItemSelected", { itemID: item.id })
    if (sameName(asSourceControl(panel.control), "Menu")) panel.visible = false
    if (sameName(asSourceControl(panel.control), "ComboBox")) panel.properties.set("expanded", "0")
  }

  private closeQuery(panel: PanelState, accepted: boolean): void {
    panel.visible = false
    if (this.applicationModal === panel.id) this.applicationModal = null
    const command = panel.properties.get(accepted ? "okcommand" : "cancelcommand")
    if (command) this.pendingRequests.push(Object.freeze({ kind: "command", panel: panel.id, command }))
    this.postAction(panel, accepted ? "OK" : "Cancel", {})
  }

  private closeFrame(panel: PanelState): void {
    panel.visible = false
    if (this.applicationModal === panel.id) this.applicationModal = null
    if (this.capture === panel.id) this.setPointerCapture(null, null, null)
    if (this.keyFocus !== null && this.hasAncestor(this.keyFocus, panel.id)) {
      this.requestedFocus = null
      this.clearFocusRequested = true
    }
    this.postAction(panel, "Close", {})
    if (panel.properties.get("deleteSelfOnClose") === "1") this.deferredDeletes.add(panel.id)
  }

  private clipboardResult(requestId: number, result: "success" | "denied" | "unavailable" | "failed", text?: string): void {
    if (!safeInteger(requestId) || requestId <= 0) throw new RuntimeFault("MalformedValue", "clipboard-result")
    const panelId = this.pendingClipboardReads.get(requestId)
    if (panelId === undefined) throw new RuntimeFault("InvalidOperation", `clipboard:${requestId}`)
    this.pendingClipboardReads.delete(requestId)
    if (result !== "success") return
    if (text === undefined || !validString(text, this.limits.maxTextCodeUnits)) throw new RuntimeFault("TextLimit", `clipboard:${requestId}`)
    const panel = this.panels.get(panelId)
    if (!panel || !panel.editable) return
    this.insertText(panel, text)
    this.postAction(panel, "TextChanged", {})
    this.publishDom()
  }

  private findByName(rootId: VguiPanelId, name: string): PanelState | null {
    const root = this.requirePanel(rootId)
    if (sameName(root.name, name)) return root
    for (const childId of root.children) {
      const found = this.findByName(childId, name)
      if (found) return found
    }
    return null
  }

  private startAnimationSequence(parentId: VguiPanelId, name: string, cancelable: boolean): void {
    this.startAnimationSequenceInternal(parentId, name, cancelable, new Set())
  }

  private startAnimationSequenceInternal(parentId: VguiPanelId, name: string, cancelable: boolean, ran: Set<string>): void {
    const parent = this.requirePanel(parentId)
    const sequence = this.sequences.get(asciiFold(name))
    if (!sequence) throw new RuntimeFault("MissingSequence", name)
    const runIdentity = `${parent.id}:${asciiFold(name)}`
    if (ran.has(runIdentity)) return
    ran.add(runIdentity)
    this.stopAnimationSequence(parent.id, sequence.name)
    let newActive = 0
    let newDelayed = 0
    for (const command of sequence.commands) {
      if (command.kind === "Animate") {
        const panel = this.findByName(parent.id, command.panel)
        if (!panel) throw new RuntimeFault("MissingReference", `${sequence.name}:${command.panel}`)
        this.animationTarget(panel, command.variable, command.target)
        newActive += 1
      } else {
        newDelayed += 1
      }
    }
    if (this.activeAnimations.length + newActive > this.limits.maxActiveAnimations) throw new RuntimeFault("AnimationLimit", `${sequence.name}:active`)
    if (this.delayedCommands.length + newDelayed > this.limits.maxDelayedCommands) throw new RuntimeFault("AnimationLimit", `${sequence.name}:delayed`)
    for (const command of sequence.commands) {
      if (command.kind === "Animate") {
        const panel = this.findByName(parent.id, command.panel)!
        this.activeAnimations.push({
          order: this.nextOrder++,
          sequence: sequence.name,
          parent: parent.id,
          panel: panel.id,
          variable: command.variable,
          cancelable,
          start: this.timeSeconds + command.delaySeconds,
          end: this.timeSeconds + command.delaySeconds + command.durationSeconds,
          interpolator: command.interpolator,
          parameter: command.parameter,
          relative: command.relative ? Object.freeze({ ...command.relative }) : null,
          started: false,
          startValue: blankAnimationValue(),
          endValue: this.animationTarget(panel, command.variable, command.target),
        })
      } else {
        this.delayedCommands.push({
          order: this.nextOrder++,
          due: this.timeSeconds + command.delaySeconds,
          sequence: sequence.name,
          parent: parent.id,
          cancelable,
          command,
        })
      }
    }
    this.addTrace("animation-start", parent.id, `${sequence.name}:${sequence.duration}`)
  }

  private stopAnimationSequence(parentId: VguiPanelId, name: string): void {
    this.requirePanel(parentId)
    this.activeAnimations.splice(0, this.activeAnimations.length, ...this.activeAnimations.filter((animation) => !(animation.parent === parentId && sameName(animation.sequence, name))))
    this.delayedCommands.splice(0, this.delayedCommands.length, ...this.delayedCommands.filter((command) => !(command.parent === parentId && sameName(command.sequence, name))))
  }

  private animationTarget(panel: PanelState, variable: string, target: string): AnimationValue {
    const folded = asciiFold(variable)
    const values = target.trim().split(/\s+/u)
    if (folded === "position") {
      if (values.length !== 2) throw new RuntimeFault("MalformedAnimation", `${panel.name}:Position`)
      return Object.freeze([this.animationPosition(values[0], true), this.animationPosition(values[1], false), 0, 0])
    }
    if (folded === "size") {
      if (values.length !== 2) throw new RuntimeFault("MalformedAnimation", `${panel.name}:Size`)
      return Object.freeze([this.proportional(parseFloatValue(values[0], "Size"), null), this.proportional(parseFloatValue(values[1], "Size"), null), 0, 0])
    }
    if (folded === "xpos") return Object.freeze([this.animationPosition(target, true), 0, 0, 0])
    if (folded === "ypos") return Object.freeze([this.animationPosition(target, false), 0, 0, 0])
    if (folded === "wide" || folded === "tall") return Object.freeze([this.proportional(parseFloatValue(target, variable), null), 0, 0, 0])
    if (folded === "fgcolor" || folded === "bgcolor") {
      const color = parseColorLiteral(target, 0) ?? this.resolveColor(target, TRANSPARENT)
      return Object.freeze([color[0], color[1], color[2], color[3]])
    }
    if (folded === "modelpos" || folded === "model_pos") {
      if (values.length !== 3) throw new RuntimeFault("MalformedAnimation", `${panel.name}:ModelPos`)
      return Object.freeze([parseFloatValue(values[0], variable), parseFloatValue(values[1], variable), parseFloatValue(values[2], variable), 0])
    }
    const definition = panel.animationDefinitions.get(folded)
    if (!definition) throw new RuntimeFault("MissingReference", `${panel.name}:animation-variable:${variable}`)
    if (definition.converter === "Color") {
      const color = parseColorLiteral(target, 0) ?? this.resolveColor(target, TRANSPARENT)
      return Object.freeze([color[0], color[1], color[2], color[3]])
    }
    if (["float", "int", "bool", "proportional_float", "proportional_int", "proportional_xpos", "proportional_ypos", "proportional_width", "proportional_height"].includes(definition.converter)) {
      return Object.freeze([parseFloatValue(target, variable), 0, 0, 0])
    }
    throw new RuntimeFault("MalformedAnimation", `${panel.name}:${variable}:non-interpolable`)
  }

  private animationPosition(value: string, horizontal: boolean): number {
    let text = value
    let alignment: "none" | "far" | "center" = "none"
    if (text[0]?.toLowerCase() === "r") { alignment = "far"; text = text.slice(1) }
    else if (text[0]?.toLowerCase() === "c") { alignment = "center"; text = text.slice(1) }
    const scaled = this.proportional(parseFloatValue(text, "animation-position"), null)
    const dimension = horizontal ? this.viewport.width : this.viewport.height
    if (alignment === "far") return dimension - scaled
    if (alignment === "center") return Math.trunc(dimension / 2) + scaled
    return scaled
  }

  private readAnimationValue(animation: ActiveAnimation, panel: PanelState): AnimationValue {
    const folded = asciiFold(animation.variable)
    const relative = this.relativeOffset(animation)
    if (folded === "position") return Object.freeze([panel.bounds.x - relative[0], panel.bounds.y - relative[1], 0, 0])
    if (folded === "size") return Object.freeze([panel.bounds.width, panel.bounds.height, 0, 0])
    if (folded === "xpos") return Object.freeze([panel.bounds.x - relative[0], 0, 0, 0])
    if (folded === "ypos") return Object.freeze([panel.bounds.y - relative[1], 0, 0, 0])
    if (folded === "wide") return Object.freeze([panel.bounds.width, 0, 0, 0])
    if (folded === "tall") return Object.freeze([panel.bounds.height, 0, 0, 0])
    const stored = panel.animationValues.get(animation.variable)
      ?? [...panel.animationValues.entries()].find(([name]) => sameName(name, animation.variable))?.[1]
    if (Array.isArray(stored)) return Object.freeze([stored[0], stored[1], stored[2], stored[3]])
    if (typeof stored === "number") {
      const definition = panel.animationDefinitions.get(folded)
      if (definition && ["proportional_float", "proportional_int"].includes(definition.converter)) {
        return Object.freeze([stored * 480 / this.viewport.height, 0, 0, 0])
      }
      return Object.freeze([stored, 0, 0, 0])
    }
    return blankAnimationValue()
  }

  private writeAnimationValue(animation: ActiveAnimation, panel: PanelState, value: AnimationValue): void {
    const folded = asciiFold(animation.variable)
    const relative = this.relativeOffset(animation)
    if (folded === "position") panel.bounds = { ...panel.bounds, x: Math.trunc(value[0]) + relative[0], y: Math.trunc(value[1]) + relative[1] }
    else if (folded === "size") {
      panel.bounds = { ...panel.bounds, width: Math.max(panel.minimumWidth, Math.trunc(value[0])), height: Math.max(panel.minimumHeight, Math.trunc(value[1])) }
      this.resizeChildren(panel)
    }
    else if (folded === "xpos") panel.bounds.x = Math.trunc(value[0]) + relative[0]
    else if (folded === "ypos") panel.bounds.y = Math.trunc(value[0]) + relative[1]
    else if (folded === "wide") { panel.bounds.width = Math.max(panel.minimumWidth, Math.trunc(value[0])); this.resizeChildren(panel) }
    else if (folded === "tall") { panel.bounds.height = Math.max(panel.minimumHeight, Math.trunc(value[0])); this.resizeChildren(panel) }
    else if (folded === "fgcolor" || folded === "bgcolor") panel.animationValues.set(folded === "fgcolor" ? "FgColor" : "BgColor", this.animationColor(value))
    else if (folded === "modelpos" || folded === "model_pos") panel.animationValues.set("ModelPos", Object.freeze([value[0], value[1], value[2], 0]) as Rgba)
    else {
      const definition = panel.animationDefinitions.get(folded)
      if (!definition) return
      let output: string | number | boolean | Rgba
      if (definition.converter === "Color") output = this.animationColor(value)
      else if (definition.converter === "int") output = Math.trunc(value[0])
      else if (definition.converter === "bool") output = Math.trunc(value[0]) !== 0
      else if (definition.converter === "proportional_float") output = this.proportional(value[0], panel)
      else if (definition.converter === "proportional_int") output = this.proportional(Math.trunc(value[0]), panel)
      else if (definition.converter === "proportional_xpos") output = this.computePosition(String(value[0]), panel, true)
      else if (definition.converter === "proportional_ypos") output = this.computePosition(String(value[0]), panel, false)
      else if (definition.converter === "proportional_width") output = this.computeDimension(String(value[0]), panel, true)
      else if (definition.converter === "proportional_height") output = this.computeDimension(String(value[0]), panel, false)
      else output = value[0]
      panel.animationValues.set(definition.name, output)
    }
    this.reapplyPanelPresentation(panel)
  }

  private animationColor(value: AnimationValue): Rgba {
    return Object.freeze(value.map((channel) => Math.max(0, Math.min(255, Math.trunc(channel)))) as unknown as Rgba)
  }

  private relativeOffset(animation: ActiveAnimation): readonly [number, number] {
    if (!animation.relative) return Object.freeze([0, 0])
    const panel = this.findByName(this.rootPanel, animation.relative.panel)
    if (!panel) return Object.freeze([0, 0])
    const token = asciiFold(animation.relative.alignment)
    const north = token === "north" || token === "n"
    const northeast = token === "northeast" || token === "ne"
    const west = token === "west" || token === "w"
    const center = token === "center" || token === "c"
    const east = token === "east" || token === "e"
    const southwest = token === "southwest" || token === "sw"
    const south = token === "south" || token === "s"
    const southeast = token === "southeast" || token === "se"
    const x = northeast || east || southeast
      ? panel.bounds.x + panel.bounds.width
      : north || center || south
        ? Math.trunc((panel.bounds.x + panel.bounds.width) / 2)
        : panel.bounds.x
    const y = southwest || south || southeast
      ? panel.bounds.y + panel.bounds.height
      : west || center || east
        ? Math.trunc((panel.bounds.y + panel.bounds.height) / 2)
        : panel.bounds.y
    return Object.freeze([x, y])
  }

  private sampleInterpolator(interpolator: string, parameter: number, position: number): number {
    if (sameName(interpolator, "Accel")) return position * position
    if (sameName(interpolator, "Deaccel")) return Math.sqrt(Math.max(0, position))
    if (sameName(interpolator, "Spline")) return position * position * (3 - 2 * position)
    if (sameName(interpolator, "Pulse")) return 0.5 + 0.5 * Math.cos(position * 2 * Math.PI * parameter)
    if (sameName(interpolator, "Flicker")) return this.randomUnit() < parameter ? 1 : 0
    if (sameName(interpolator, "Bias")) return this.bias(position, parameter)
    if (sameName(interpolator, "Gain")) return position < 0.5 ? this.bias(position * 2, 1 - parameter) / 2 : 1 - this.bias(2 - position * 2, 1 - parameter) / 2
    if (sameName(interpolator, "Bounce")) {
      if (position < 0.33) return 1 - Math.sin(Math.PI * position / 0.33)
      if (position < 0.67) return 0.5 + 0.5 * (1 - Math.sin(Math.PI * (position - 0.33) / 0.34))
      return 0.8 + 0.2 * (1 - Math.sin(Math.PI * (position - 0.67) / 0.33))
    }
    return position
  }

  private bias(position: number, amount: number): number {
    if (!finite(amount) || amount <= 0 || amount >= 1) throw new RuntimeFault("MalformedAnimation", "bias")
    return Math.pow(position, Math.log(amount) / Math.log(0.5))
  }

  private randomUnit(): number {
    const value = this.random.nextUnit()
    if (!finite(value) || value < 0 || value >= 1) throw new RuntimeFault("MalformedAnimation", "random")
    return value
  }

  private updateDelayedCommands(runCancelableToCompletion: boolean): void {
    const ran = new Set<string>()
    for (let index = 0; index < this.delayedCommands.length;) {
      const delayed = this.delayedCommands[index]
      if (runCancelableToCompletion ? !delayed.cancelable : delayed.due > this.timeSeconds) { index += 1; continue }
      this.delayedCommands.splice(index, 1)
      if (!this.panels.has(delayed.parent)) continue
      this.executeDelayedCommand(delayed, ran)
      index = 0
    }
  }

  private executeDelayedCommand(delayed: DelayedAnimationCommand, ran: Set<string>): void {
    const parent = this.requirePanel(delayed.parent)
    const command = delayed.command
    this.addTrace("animation-command", parent.id, `${delayed.sequence}:${command.kind}`)
    if (command.kind === "RunEvent") {
      this.startAnimationSequenceInternal(parent.id, command.sequence, delayed.cancelable, ran)
    } else if (command.kind === "RunEventChild") {
      const child = this.findByName(parent.id, command.child)
      if (!child) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.child}`)
      this.startAnimationSequenceInternal(child.id, command.sequence, delayed.cancelable, ran)
    } else if (command.kind === "StopEvent") {
      this.stopAnimationSequence(parent.id, command.sequence)
    } else if (command.kind === "StopAnimation") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      const index = this.activeAnimations.findIndex((animation) => animation.panel === panel.id && sameName(animation.variable, command.variable) && !sameName(animation.sequence, delayed.sequence))
      if (index >= 0) this.activeAnimations.splice(index, 1)
    } else if (command.kind === "StopPanelAnimations") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      this.activeAnimations.splice(0, this.activeAnimations.length, ...this.activeAnimations.filter((animation) => animation.panel !== panel.id || sameName(animation.sequence, delayed.sequence)))
    } else if (command.kind === "SetFont") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      const definition = panel.animationDefinitions.get(asciiFold(command.variable))
      if (!definition || !["HFont", "vgui::HFont"].includes(definition.converter)) throw new RuntimeFault("MissingReference", `${panel.name}:${command.variable}`)
      panel.animationValues.set(definition.name, this.convertAnimationScalar(definition.converter, command.font, definition, panel))
      panel.font = command.font
      this.reapplyPanelPresentation(panel)
    } else if (command.kind === "SetTexture") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      if (!this.images.has(asciiFold(command.texture))) throw new RuntimeFault("MissingReference", `${panel.name}:${command.texture}`)
      const definition = panel.animationDefinitions.get(asciiFold(command.variable))
      if (!definition || definition.converter !== "textureid") throw new RuntimeFault("MissingReference", `${panel.name}:${command.variable}`)
      panel.animationValues.set(definition.name, command.texture)
    } else if (command.kind === "SetString") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      const definition = panel.animationDefinitions.get(asciiFold(command.variable))
      if (!definition || !["char", "string"].includes(definition.converter)) throw new RuntimeFault("MissingReference", `${panel.name}:${command.variable}`)
      panel.animationValues.set(definition.name, this.convertAnimationScalar(definition.converter, command.value, definition, panel))
    } else if (command.kind === "FireCommand") {
      this.pendingRequests.push(Object.freeze({ kind: "command", panel: parent.id, command: command.command }))
    } else if (command.kind === "PlaySound") {
      this.pendingRequests.push(Object.freeze({ kind: "sound", panel: parent.id, logicalIdentity: command.sound }))
    } else if (command.kind === "SetVisible") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      panel.visible = command.visible
    } else if (command.kind === "SetInputEnabled") {
      const panel = this.findByName(parent.id, command.panel)
      if (!panel) throw new RuntimeFault("MissingReference", `${delayed.sequence}:${command.panel}`)
      panel.mouseInput = command.enabled
      panel.keyboardInput = command.enabled
    }
  }

  private updateActiveAnimations(runCancelableToCompletion: boolean): void {
    for (let index = 0; index < this.activeAnimations.length;) {
      const animation = this.activeAnimations[index]
      if (runCancelableToCompletion ? !animation.cancelable : this.timeSeconds < animation.start) { index += 1; continue }
      const panel = this.panels.get(animation.panel)
      if (!panel) { this.activeAnimations.splice(index, 1); continue }
      if (!animation.started && !runCancelableToCompletion) {
        animation.startValue = this.readAnimationValue(animation, panel)
        animation.started = true
      }
      const complete = runCancelableToCompletion || this.timeSeconds >= animation.end
      let value: AnimationValue
      if (complete || this.reducedMotion) {
        value = animation.endValue
      } else {
        const duration = animation.end - animation.start
        const normalized = duration <= 0 ? 1 : (this.timeSeconds - animation.start) / duration
        const sample = this.sampleInterpolator(animation.interpolator, animation.parameter, normalized)
        value = Object.freeze([
          animation.startValue[0] + (animation.endValue[0] - animation.startValue[0]) * sample,
          animation.startValue[1] + (animation.endValue[1] - animation.startValue[1]) * sample,
          animation.startValue[2] + (animation.endValue[2] - animation.startValue[2]) * sample,
          animation.startValue[3] + (animation.endValue[3] - animation.startValue[3]) * sample,
        ])
      }
      this.writeAnimationValue(animation, panel, value)
      if (complete) this.activeAnimations.splice(index, 1)
      else index += 1
    }
  }

  private finishCancelableAnimations(): void {
    this.updateDelayedCommands(true)
    this.updateActiveAnimations(true)
  }

  private runFrame(timeSeconds: number): void {
    if (!finite(timeSeconds) || timeSeconds < this.timeSeconds) throw new RuntimeFault("MalformedValue", "frame-time")
    if (this.inFrame) throw new RuntimeFault("ReentrantFrame", "frame")
    this.inFrame = true
    try {
      this.timeSeconds = timeSeconds
      this.addTrace("frame", null, `begin:${this.frame + 1}`)
      this.pressedKeys.clear()
      this.releasedKeys.clear()
      this.pressedButtons.clear()
      this.releasedButtons.clear()
      for (const key of this.pendingPressedKeys) this.pressedKeys.add(key)
      for (const key of this.pendingReleasedKeys) this.releasedKeys.add(key)
      for (const button of this.pendingPressedButtons) this.pressedButtons.add(button)
      for (const button of this.pendingReleasedButtons) this.releasedButtons.add(button)
      this.pendingPressedKeys.clear()
      this.pendingReleasedKeys.clear()
      this.pendingPressedButtons.clear()
      this.pendingReleasedButtons.clear()
      this.addTrace("input-rollover", null, "complete")

      this.calculatedKeyFocus = this.calculateKeyFocus()
      this.commitFocus(this.calculatedKeyFocus)
      this.addTrace("focus", this.keyFocus, "committed")
      this.dispatchMessages()
      this.addTrace("messages", null, "dispatched")
      this.addTrace("ticks", this.keyFocus, "focus-tick")
      this.solveGeometry()
      this.addTrace("layout", null, "solved")
      this.updateDelayedCommands(false)
      this.updateActiveAnimations(false)
      this.addTrace("animation", null, "advanced")
      for (const panelId of [...this.deferredDeletes]) this.deletePanelImmediate(panelId)
      this.solveGeometry()
      this.publishDom()
      this.addTrace("dom", null, "published")
      this.flushRequests()
      this.frame += 1
      this.addTrace("frame", null, `end:${this.frame}`)
    } finally {
      this.inFrame = false
    }
  }

  private commitFocus(next: VguiPanelId | null): void {
    if (next === this.keyFocus) {
      this.clearFocusRequested = false
      return
    }
    const previous = this.keyFocus
    if (previous !== null) {
      const oldPanel = this.panels.get(previous)
      if (oldPanel) {
        this.dispatchMessage(oldPanel, { name: "KillFocus", fields: { newPanel: next } }, null)
        try { oldPanel.element.blur() } catch {}
      }
    }
    if (next !== null) {
      const newPanel = this.requirePanel(next)
      this.dispatchMessage(newPanel, { name: "SetFocus", fields: {} }, null)
      try { newPanel.element.focus({ preventScroll: true }) } catch { try { newPanel.element.focus() } catch {} }
    }
    this.keyFocus = next
    this.requestedFocus = next
    this.clearFocusRequested = false
    if (next !== null) this.movePanel(next, true)
    for (const panel of this.panels.values()) {
      if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(asSourceControl(panel.control), name))) this.reapplyPanelPresentation(panel)
    }
  }

  private flushRequests(): void {
    for (const request of this.pendingRequests.splice(0)) {
      try {
        this.onRequest(request)
      } catch {
        this.recordDiagnostic("RequestSinkFailure", "frame", request.kind)
      }
    }
  }

  private setViewport(viewport: VguiViewport): void {
    if (!validViewport(viewport)) throw new RuntimeFault("InvalidViewport", "viewport")
    const changedSize = viewport.width !== this.viewport.width || viewport.height !== this.viewport.height
    this.viewport = Object.freeze({ ...viewport })
    this.host.style.width = `${viewport.width}px`
    this.host.style.height = `${viewport.height}px`
    const root = this.requirePanel(this.rootPanel)
    const sizeChanged = root.bounds.width !== viewport.width || root.bounds.height !== viewport.height
    root.bounds = { x: 0, y: 0, width: viewport.width, height: viewport.height }
    if (changedSize) {
      this.finishCancelableAnimations()
      this.installAnimationScripts(this.animationScripts)
      for (const panel of this.panels.values()) {
        if (panel.id !== this.rootPanel) this.reapplyStoredGeometry(panel)
      }
      if (sizeChanged) this.resizeChildren(root)
    }
    this.solveGeometry()
    this.publishDom()
  }

  private reapplyStoredGeometry(panel: PanelState): void {
    const property = (name: string): string | null => [...panel.properties.entries()].find(([key]) => sameName(key, name))?.[1] ?? null
    const wide = property("wide")
    const tall = property("tall")
    if (wide !== null) panel.bounds.width = Math.max(panel.minimumWidth, this.computeDimension(wide, panel, true, false, true))
    if (tall !== null) panel.bounds.height = Math.max(panel.minimumHeight, this.computeDimension(tall, panel, false, false, true))
    const xpos = property("xpos")
    const ypos = property("ypos")
    if (xpos !== null) panel.bounds.x = this.computePosition(xpos, panel, true, true)
    if (ypos !== null) panel.bounds.y = this.computePosition(ypos, panel, false, true)
  }

  private solveGeometry(): void {
    const workspace: VguiRect = { x: 0, y: 0, width: this.viewport.width, height: this.viewport.height }
    const visiting = new Set<VguiPanelId>()
    const solved = new Set<VguiPanelId>()
    const solve = (panelId: VguiPanelId): void => {
      if (solved.has(panelId)) return
      if (visiting.has(panelId)) throw new RuntimeFault("HierarchyCycle", `solve:${panelId}`)
      visiting.add(panelId)
      const panel = this.requirePanel(panelId)
      const parent = panel.parent === null ? null : this.requirePanel(panel.parent)
      if (parent && !visiting.has(parent.id)) solve(parent.id)
      let x = panel.bounds.x
      let y = panel.bounds.y
      if (parent && !panel.popup) {
        x += parent.absoluteBounds.x + parent.inset.left
        y += parent.absoluteBounds.y + parent.inset.top
      }
      const siblingName = panel.properties.get("pin_to_sibling")
      if (siblingName && parent) {
        const sibling = parent.children.map((id) => this.requirePanel(id)).find((candidate) => sameName(candidate.name, siblingName))
        if (!sibling) throw new RuntimeFault("MissingReference", `${panel.name}:pin:${siblingName}`)
        solve(sibling.id)
        const ownCorner = this.corner(panel.properties.get("pin_corner_to_sibling"))
        const siblingCorner = this.corner(panel.properties.get("pin_to_sibling_corner"))
        const parentOriginX = panel.popup ? 0 : parent.absoluteBounds.x + parent.inset.left
        const parentOriginY = panel.popup ? 0 : parent.absoluteBounds.y + parent.inset.top
        const signX = siblingCorner[0] === 0 ? -1 : 1
        const signY = siblingCorner[1] === 0 ? -1 : 1
        x = sibling.absoluteBounds.x - parentOriginX + sibling.absoluteBounds.width * siblingCorner[0] - panel.bounds.width * ownCorner[0] + panel.bounds.x * signX + parentOriginX
        y = sibling.absoluteBounds.y - parentOriginY + sibling.absoluteBounds.height * siblingCorner[1] - panel.bounds.height * ownCorner[1] + panel.bounds.y * signY + parentOriginY
      }
      panel.absoluteBounds = {
        x: Math.max(-32767, Math.min(32767, Math.trunc(x))),
        y: Math.max(-32767, Math.min(32767, Math.trunc(y))),
        width: Math.max(0, panel.bounds.width),
        height: Math.max(0, panel.bounds.height),
      }
      panel.effectivelyVisible = panel.visible && (parent?.effectivelyVisible ?? true)
      if (panel.popup || !parent) panel.clip = rectIntersection(panel.absoluteBounds, workspace)
      else {
        const parentClip = {
          x: parent.clip.x,
          y: parent.clip.y,
          width: Math.max(0, parent.clip.width - parent.inset.right),
          height: Math.max(0, parent.clip.height - parent.inset.bottom),
        }
        panel.clip = rectIntersection(panel.absoluteBounds, parentClip)
      }
      for (const childId of panel.children) solve(childId)
      visiting.delete(panelId)
      solved.add(panelId)
    }
    solve(this.rootPanel)
    for (const popupId of this.popups) if (this.panels.has(popupId)) solve(popupId)
  }

  private corner(value: string | undefined): readonly [number, number] {
    const folded = asciiFold(value ?? "0")
    if (folded === "1" || folded === "top-right") return Object.freeze([1, 0])
    if (folded === "2" || folded === "bottom-left") return Object.freeze([0, 1])
    if (folded === "3" || folded === "bottom-right") return Object.freeze([1, 1])
    if (folded === "4" || folded === "center-top") return Object.freeze([0.5, 0])
    if (folded === "5" || folded === "center-right") return Object.freeze([1, 0.5])
    if (folded === "6" || folded === "center-bottom") return Object.freeze([0.5, 1])
    if (folded === "7" || folded === "center-left") return Object.freeze([0, 0.5])
    return Object.freeze([0, 0])
  }

  private addPopup(panel: PanelState): void {
    if (!this.popups.includes(panel.id)) this.popups.push(panel.id)
    this.sortPopups()
  }

  private removePopup(panelId: VguiPanelId): void {
    const index = this.popups.indexOf(panelId)
    if (index >= 0) this.popups.splice(index, 1)
  }

  private sortPopups(): void {
    const prior = new Map(this.popups.map((id, index) => [id, index]))
    this.popups.sort((leftId, rightId) => {
      const left = this.requirePanel(leftId)
      const right = this.requirePanel(rightId)
      return Number(left.topmostPopup) - Number(right.topmostPopup) || prior.get(leftId)! - prior.get(rightId)!
    })
  }

  private publishDom(): void {
    if (this.destroyed) return
    try {
      const place = (panelId: VguiPanelId): void => {
        const panel = this.requirePanel(panelId)
        const parent = panel.popup || panel.parent === null ? this.host : this.requirePanel(panel.parent).element
        parent.append(panel.element)
        const relativeX = panel.popup || panel.parent === null ? panel.absoluteBounds.x : panel.bounds.x + this.requirePanel(panel.parent).inset.left
        const relativeY = panel.popup || panel.parent === null ? panel.absoluteBounds.y : panel.bounds.y + this.requirePanel(panel.parent).inset.top
        panel.element.style.left = `${relativeX}px`
        panel.element.style.top = `${relativeY}px`
        panel.element.style.width = `${panel.bounds.width}px`
        panel.element.style.height = `${panel.bounds.height}px`
        panel.element.style.zIndex = String(panel.z)
        panel.element.style.display = panel.effectivelyVisible ? "block" : "none"
        panel.element.style.visibility = panel.effectivelyVisible ? "visible" : "hidden"
        panel.element.style.pointerEvents = panel.effectivelyVisible && panel.mouseInput ? "auto" : "none"
        const clipTop = Math.max(0, panel.clip.y - panel.absoluteBounds.y)
        const clipLeft = Math.max(0, panel.clip.x - panel.absoluteBounds.x)
        const clipRight = Math.max(0, panel.absoluteBounds.x + panel.absoluteBounds.width - panel.clip.x - panel.clip.width)
        const clipBottom = Math.max(0, panel.absoluteBounds.y + panel.absoluteBounds.height - panel.clip.y - panel.clip.height)
        panel.element.style.clipPath = `inset(${clipTop}px ${clipRight}px ${clipBottom}px ${clipLeft}px)`
        panel.element.hidden = !panel.effectivelyVisible
        panel.element.setAttribute("aria-hidden", panel.effectivelyVisible ? "false" : "true")
        panel.element.setAttribute("aria-disabled", panel.enabled ? "false" : "true")
        panel.element.setAttribute("aria-label", panel.accessibleName)
        if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(asSourceControl(panel.control), name))) {
          panel.element.setAttribute("aria-modal", this.applicationModal === panel.id ? "true" : "false")
        }
        if (panel.accessibleDescription) panel.element.setAttribute("aria-description", panel.accessibleDescription)
        else panel.element.removeAttribute("aria-description")
        panel.element.tabIndex = panel.registration.focusable && panel.keyboardInput && panel.enabled ? panel.tabPosition > 0 ? panel.tabPosition : 0 : -1
        panel.element.dataset.focused = this.keyFocus === panel.id ? "true" : "false"
        panel.element.dataset.armed = panel.armed ? "true" : "false"
        panel.element.dataset.depressed = panel.depressed ? "true" : "false"
        panel.element.dataset.selected = panel.selected ? "true" : "false"
        if (panel.frameInteraction) panel.element.dataset.interaction = panel.frameInteraction
        else delete panel.element.dataset.interaction
        this.publishControlDom(panel)
        for (const childId of panel.children) if (!this.requirePanel(childId).popup) place(childId)
      }
      place(this.rootPanel)
      for (const popupId of this.popups) if (this.panels.has(popupId)) place(popupId)
    } catch (error) {
      if (error instanceof RuntimeFault) throw error
      throw new RuntimeFault("DomFailure", "publish")
    }
  }

  private publishControlDom(panel: PanelState): void {
    const control = asSourceControl(panel.control)
    if (sameName(control, "TextEntry") || sameName(control, "ComboBox")) {
      const input = panel.element as HTMLInputElement | HTMLTextAreaElement
      input.value = panel.text
      if (panel.element.tagName.toLowerCase() === "input") (input as HTMLInputElement).type = panel.textHidden ? "password" : "text"
      else panel.element.style.setProperty("-webkit-text-security", panel.textHidden ? "disc" : "none")
      input.readOnly = !panel.editable
      input.setAttribute("aria-multiline", panel.multiline ? "true" : "false")
      input.setAttribute("autocomplete", "off")
      panel.element.style.resize = "none"
      panel.element.style.whiteSpace = panel.multiline ? "pre-wrap" : "pre"
      try {
        const start = panel.selectionStart < 0 ? panel.caret : panel.selectionStart
        input.setSelectionRange(start, panel.selectionEnd)
      } catch {}
    } else if (!sameName(control, "ImagePanel") && !["Frame", "MessageBox", "QueryBox"].some((name) => sameName(control, name))) {
      panel.element.textContent = panel.text
    }
    if (sameName(control, "CheckButton") || sameName(control, "RadioButton")) panel.element.setAttribute("aria-checked", panel.checked ? "true" : "false")
    if (sameName(control, "Button") || sameName(control, "MenuItem")) panel.element.setAttribute("aria-pressed", panel.selected ? "true" : "false")
    if (sameName(control, "Slider") || sameName(control, "ScrollBar")) {
      panel.element.setAttribute("aria-valuemin", String(panel.minimum))
      panel.element.setAttribute("aria-valuemax", String(panel.maximum))
      panel.element.setAttribute("aria-valuenow", String(panel.value))
      panel.element.setAttribute("aria-orientation", sameName(panel.control, "ScrollBar_Vertical") ? "vertical" : "horizontal")
    }
    if (sameName(control, "ProgressBar")) {
      panel.element.setAttribute("aria-valuemin", "0")
      panel.element.setAttribute("aria-valuemax", "1")
      panel.element.setAttribute("aria-valuenow", String(panel.progress))
      panel.element.style.setProperty("--vgui-progress", String(panel.progress))
    }
    if (sameName(control, "ComboBox")) {
      panel.element.setAttribute("aria-expanded", panel.properties.get("expanded") === "1" ? "true" : "false")
      if (panel.activeIndex !== null) panel.element.setAttribute("aria-activedescendant", `${this.runtimeIdentity}-${panel.id}-item-${panel.items[panel.activeIndex].id}`)
    }
    if (sameName(control, "PropertyPage")) panel.element.setAttribute("aria-selected", panel.visible ? "true" : "false")
    if (sameName(control, "URLLabel") && panel.url) panel.element.setAttribute("href", panel.url)
    if (["Frame", "MessageBox", "QueryBox"].some((name) => sameName(control, name))) this.publishFrameChrome(panel)
    if (sameName(control, "Label")) {
      const associate = panel.properties.get("associate")
      const target = associate ? this.findByName(this.rootPanel, associate) : null
      if (target) panel.element.setAttribute("aria-controls", target.element.id)
      else panel.element.removeAttribute("aria-controls")
    }
    this.publishItemDom(panel)
    this.reapplyPanelPresentation(panel)
    this.presentControlGeometry(panel)
  }

  private publishFrameChrome(panel: PanelState): void {
    const get = (name: string, tag: keyof HTMLElementTagNameMap): HTMLElement => {
      let element = panel.chromeElements.get(name)
      if (element) return element
      if (this.panels.size + this.auxiliaryNodes.size + 3 > this.limits.maxDomNodes) throw new RuntimeFault("DomLimit", `${panel.name}:${name}`)
      element = this.document.createElement(tag)
      element.className = `playsrc-vgui-frame-${name}${tag === "button" ? " playsrc-vgui-source-control" : ""}`
      element.dataset.vguiChrome = name
      panel.chromeElements.set(name, element)
      this.auxiliaryNodes.add(element)
      return element
    }
    const background = get("title-background", "div")
    const title = get("title", "span")
    const close = get("close", "button")
    const scale = panel.proportional ? this.viewport.height / 480 : 1
    const inset = Math.trunc(5 * scale)
    const captionBottom = Math.trunc(28 * scale)
    const titleX = Math.trunc(Number(panel.animationValues.get("titletextinsetX") || 28) * scale)
    const titleY = Math.trunc(Number(panel.animationValues.get("titletextinsetY") || 9) * scale)
    const focused = this.keyFocus !== null && this.hasAncestor(this.keyFocus, panel.id)
    const backgroundOverride = panel.properties.get(focused ? "titlebarbgcolor_override" : "titlebardisabledbgcolor_override")
    const foregroundOverride = panel.properties.get(focused ? "titlebarfgcolor_override" : "titlebardisabledfgcolor_override")
    const backgroundColor = backgroundOverride
      ? parseColorLiteral(backgroundOverride, 0) ?? this.resolveColor(backgroundOverride, TRANSPARENT)
      : this.resolveColor(focused ? "FrameTitleBar.BgColor" : "FrameTitleBar.DisabledBgColor", TRANSPARENT)
    const foregroundColor = foregroundOverride
      ? parseColorLiteral(foregroundOverride, 0) ?? this.resolveColor(foregroundOverride, WHITE)
      : this.resolveColor(focused ? "FrameTitleBar.TextColor" : "FrameTitleBar.DisabledTextColor", WHITE)
    for (const element of [background, title, close]) {
      element.style.position = "absolute"
      panel.element.append(element)
    }
    const visible = panel.properties.get("settitlebarvisible") !== "0"
    background.style.display = visible ? "block" : "none"
    background.style.left = `${inset}px`
    background.style.top = `${inset}px`
    background.style.width = `${Math.max(0, panel.bounds.width - inset * 2)}px`
    background.style.height = `${Math.max(0, captionBottom - inset)}px`
    background.style.backgroundColor = rgba(backgroundColor)
    background.style.pointerEvents = "none"
    title.style.display = visible ? "block" : "none"
    title.style.left = `${titleX}px`
    title.style.top = `${titleY}px`
    title.style.width = `${Math.max(0, panel.bounds.width - Math.trunc(72 * scale))}px`
    title.style.height = `${panel.font ? this.fonts.get(asciiFold(panel.font))?.lineHeightPx ?? 16 : 16}px`
    title.style.color = rgba(foregroundColor)
    title.style.overflow = "hidden"
    title.style.whiteSpace = "nowrap"
    title.style.pointerEvents = "none"
    title.textContent = panel.text
    const closeVisible = visible && panel.properties.get("setclosebuttonvisible") !== "0"
    close.style.display = closeVisible ? "block" : "none"
    close.style.left = `${panel.bounds.width - Math.trunc(25 * scale)}px`
    close.style.top = `${Math.trunc(8 * scale)}px`
    close.style.width = `${Math.trunc(15 * scale)}px`
    close.style.height = `${Math.trunc(15 * scale)}px`
    close.style.background = "transparent"
    close.style.border = "0"
    close.style.color = "transparent"
    close.style.pointerEvents = "auto"
    close.setAttribute("aria-label", "Close")
    close.tabIndex = -1
    const control = asSourceControl(panel.control)
    if (["MessageBox", "QueryBox"].some((name) => sameName(control, name))) {
      const body = get("message", "div")
      const ok = get("ok", "button")
      const cancel = get("cancel", "button")
      for (const element of [body, ok, cancel]) {
        element.style.position = "absolute"
        panel.element.append(element)
      }
      const clientInset = Math.trunc(Number(panel.properties.get("clientinsetx_override") ?? 5) * scale)
      const buttonWidth = Math.trunc(64 * scale)
      const buttonHeight = Math.trunc(24 * scale)
      const buttonY = panel.bounds.height - buttonHeight - Math.trunc(15 * scale)
      body.style.left = `${clientInset}px`
      body.style.top = `${captionBottom + Math.trunc(5 * scale)}px`
      body.style.width = `${Math.max(0, panel.bounds.width - clientInset * 2)}px`
      body.style.height = `${Math.max(0, buttonY - captionBottom - Math.trunc(10 * scale))}px`
      body.style.display = "flex"
      body.style.alignItems = "flex-start"
      body.style.justifyContent = "center"
      body.style.pointerEvents = "none"
      body.textContent = panel.bodyText
      const query = sameName(control, "QueryBox")
      const okX = query ? Math.trunc(panel.bounds.width / 2) - buttonWidth - Math.trunc(scale) : Math.trunc((panel.bounds.width - buttonWidth) / 2)
      ok.style.left = `${okX}px`
      ok.style.top = `${buttonY}px`
      ok.style.width = `${buttonWidth}px`
      ok.style.height = `${buttonHeight}px`
      ok.textContent = this.localization.get("messagebox_ok") ?? "#MessageBox_OK"
      ok.setAttribute("aria-label", ok.textContent)
      ok.tabIndex = -1
      ok.style.color = rgba(this.resolveColor("Button.TextColor", WHITE))
      ok.style.backgroundColor = rgba(this.resolveColor("Button.BgColor", TRANSPARENT))
      const cancelVisible = query
      cancel.style.display = cancelVisible ? "block" : "none"
      cancel.style.left = `${Math.trunc(panel.bounds.width / 2) + Math.trunc(16 * scale)}px`
      cancel.style.top = `${buttonY}px`
      cancel.style.width = `${buttonWidth}px`
      cancel.style.height = `${buttonHeight}px`
      cancel.textContent = this.localization.get("querybox_cancel") ?? "#QueryBox_Cancel"
      cancel.setAttribute("aria-label", cancel.textContent)
      cancel.tabIndex = -1
      cancel.style.color = rgba(this.resolveColor("Button.TextColor", WHITE))
      cancel.style.backgroundColor = rgba(this.resolveColor("Button.BgColor", TRANSPARENT))
    }
  }

  private presentControlGeometry(panel: PanelState): void {
    const control = asSourceControl(panel.control)
    const images: string[] = []
    const sizes: string[] = []
    const positions: string[] = []
    const append = (color: Rgba, size: string, position: string, repeat = "no-repeat"): void => {
      images.push(`linear-gradient(${rgba(color)}, ${rgba(color)})`)
      sizes.push(size)
      positions.push(position)
      void repeat
    }
    if (sameName(control, "Slider")) {
      const scale = panel.proportional ? this.viewport.height / 480 : 1
      const trackY = Math.trunc(8 * scale)
      const trackHeight = Math.trunc(4 * scale)
      const trackWidth = Math.max(0, panel.bounds.width - panel.thumbWidth)
      const denominator = panel.maximum - panel.minimum
      const fraction = denominator === 0 ? 0 : (panel.value - panel.minimum) / denominator
      const thumbX = Math.trunc(Math.max(0, trackWidth - panel.thumbWidth) * fraction + 0.5)
      const thumbHeight = Math.trunc(16 * scale)
      const thumbY = trackY + Math.trunc(trackHeight / 2) - Math.trunc(thumbHeight / 2)
      append(this.resolveColor("Slider.TrackColor", TRANSPARENT), `${trackWidth}px ${trackHeight}px`, `0 ${trackY}px`)
      append(this.resolveColor("Slider.NobColor", WHITE), `${panel.thumbWidth}px ${thumbHeight}px`, `${thumbX}px ${thumbY}px`)
      if (panel.numTicks > 0) {
        const tickColor = this.resolveColor("Slider.TextColor", WHITE)
        const free = trackWidth - panel.thumbWidth
        for (let index = 0; index <= panel.numTicks; index += 1) {
          const x = Math.trunc(panel.thumbWidth / 2 + index * free / panel.numTicks)
          append(tickColor, `${Math.max(1, Math.trunc(scale))}px ${Math.trunc(5 * scale)}px`, `${x}px ${trackY + panel.thumbWidth}px`)
        }
      }
    } else if (sameName(control, "ScrollBar")) {
      const vertical = sameName(panel.control, "ScrollBar_Vertical")
      const cross = vertical ? panel.bounds.width : panel.bounds.height
      const noButtons = panel.properties.get("nobuttons") === "1"
      const trackStart = noButtons ? 0 : cross
      const length = (vertical ? panel.bounds.height : panel.bounds.width) - (noButtons ? 0 : cross * 2)
      const range = panel.maximum - panel.minimum
      if (range > panel.rangeWindow && range > 0 && length > 0) {
        const thumbLength = Math.max(cross, (length - 1) * panel.rangeWindow / range)
        const free = Math.max(0, length - 1 - thumbLength)
        const denominator = range - panel.rangeWindow
        const position = denominator === 0 ? 0 : free * (panel.value - panel.minimum) / denominator
        const color = this.resolveColor("ScrollBarSlider.FgColor", WHITE)
        append(color, vertical ? `${Math.max(0, cross - 1)}px ${Math.trunc(thumbLength)}px` : `${Math.trunc(thumbLength)}px ${Math.max(0, cross - 1)}px`, vertical ? `0 ${Math.trunc(trackStart + position)}px` : `${Math.trunc(trackStart + position)}px 1px`)
      }
    } else if (sameName(control, "ProgressBar")) {
      const scale = panel.proportional ? this.viewport.height / 480 : 1
      const gap = Math.max(0, Number(panel.properties.get("segment_gap") ?? Math.trunc(4 * scale)))
      const segment = Math.max(1, Number(panel.properties.get("segment_width") ?? Math.trunc(8 * scale)))
      const inset = Math.max(0, Number(panel.properties.get("bar_inset") ?? Math.trunc(4 * scale)))
      const margin = Math.max(0, Number(panel.properties.get("margin") ?? 0))
      const direction = asciiFold(panel.properties.get("direction") ?? "east")
      const horizontal = direction === "east" || direction === "west"
      const length = Math.max(0, (horizontal ? panel.bounds.width : panel.bounds.height) - margin * 2)
      const count = Math.trunc(length / (gap + segment) * panel.progress)
      const color = this.resolveColor("ProgressBar.FgColor", WHITE)
      for (let index = 0; index < count; index += 1) {
        const offset = gap + index * (gap + segment)
        if (direction === "west") append(color, `${segment}px ${Math.max(0, panel.bounds.height - inset * 2)}px`, `${panel.bounds.width - margin - offset - segment}px ${inset}px`)
        else if (direction === "north") append(color, `${Math.max(0, panel.bounds.width - inset * 2)}px ${segment}px`, `${inset}px ${panel.bounds.height - margin - offset - segment}px`)
        else if (direction === "south") append(color, `${Math.max(0, panel.bounds.width - inset * 2)}px ${segment}px`, `${inset}px ${margin + offset}px`)
        else append(color, `${segment}px ${Math.max(0, panel.bounds.height - inset * 2)}px`, `${margin + offset}px ${inset}px`)
      }
    }
    if (images.length === 0) return
    const existingImages = panel.element.style.backgroundImage
    const existingSizes = panel.element.style.backgroundSize
    const existingPositions = panel.element.style.backgroundPosition
    const hasExisting = !!existingImages && existingImages !== "none"
    panel.element.style.backgroundImage = [...images, ...(hasExisting ? [existingImages] : [])].join(", ")
    panel.element.style.backgroundSize = [...sizes, ...(hasExisting ? [existingSizes] : [])].join(", ")
    panel.element.style.backgroundPosition = [...positions, ...(hasExisting ? [existingPositions] : [])].join(", ")
    panel.element.style.backgroundRepeat = "no-repeat"
  }

  private publishItemDom(panel: PanelState): void {
    const control = asSourceControl(panel.control)
    if (!["Menu", "ComboBox", "ListPanel", "PropertySheet"].some((name) => sameName(control, name))) return
    const live = new Set(panel.items.map((item) => item.id))
    for (const [id, element] of panel.itemElements) {
      if (live.has(id)) continue
      element.remove()
      panel.itemElements.delete(id)
      this.auxiliaryNodes.delete(element)
    }
    for (let index = 0; index < panel.items.length; index += 1) {
      const item = panel.items[index]
      let element = panel.itemElements.get(item.id)
      if (!element) {
        if (this.panels.size + this.auxiliaryNodes.size + 3 > this.limits.maxDomNodes) throw new RuntimeFault("DomLimit", `${panel.name}:item:${item.id}`)
        element = this.document.createElement("div")
        element.className = "playsrc-vgui-item"
        element.dataset.vguiItem = String(item.id)
        element.id = `${this.runtimeIdentity}-${panel.id}-item-${item.id}`
        panel.itemElements.set(item.id, element)
        this.auxiliaryNodes.add(element)
      }
      element.textContent = item.text
      element.style.position = sameName(control, "ComboBox") ? "absolute" : "relative"
      element.style.height = `${Math.max(1, Number(panel.properties.get("itemheight") ?? panel.properties.get("linespacing") ?? 20))}px`
      if (sameName(control, "ComboBox")) {
        const rowHeight = Math.max(1, Number(panel.properties.get("itemheight") ?? 20))
        element.style.left = "0"
        element.style.top = `${panel.bounds.height + index * rowHeight}px`
        element.style.width = "100%"
        element.style.display = panel.properties.get("expanded") === "1" ? "block" : "none"
        panel.element.style.overflow = "visible"
      }
      element.style.pointerEvents = item.enabled ? "auto" : "none"
      element.setAttribute("aria-disabled", item.enabled ? "false" : "true")
      element.setAttribute("aria-selected", panel.activeIndex === index ? "true" : "false")
      const selected = panel.activeIndex === index
      if (sameName(control, "Menu") || sameName(control, "ComboBox")) {
        element.style.color = rgba(this.resolveColor(selected ? "Menu.ArmedTextColor" : "Menu.TextColor", WHITE))
        element.style.backgroundColor = rgba(this.resolveColor(selected ? "Menu.ArmedBgColor" : "Menu.BgColor", TRANSPARENT))
      } else if (sameName(control, "ListPanel")) {
        element.style.color = rgba(this.resolveColor(selected ? "ListPanel.SelectedTextColor" : "ListPanel.TextColor", WHITE))
        element.style.backgroundColor = rgba(this.resolveColor(selected ? "ListPanel.SelectedBgColor" : "ListPanel.BgColor", TRANSPARENT))
      }
      if (sameName(control, "Menu")) {
        element.setAttribute("role", "menuitem")
        if (item.checked) element.setAttribute("aria-checked", "true")
      } else if (sameName(control, "ComboBox")) element.setAttribute("role", "option")
      else if (sameName(control, "ListPanel")) element.setAttribute("role", "row")
      else {
        element.setAttribute("role", "tab")
        element.setAttribute("aria-controls", this.panels.get(item.id)?.element.id ?? String(item.id))
      }
      panel.element.append(element)
    }
  }

  private installBrowserAdapter(): void {
    const window = this.document.defaultView
    const listenerCount = 12 + (window ? 1 : 0)
    if (listenerCount > this.limits.maxListeners) throw new RuntimeFault("ListenerLimit", "browser-adapter")
    this.listen(this.host, "pointermove", (raw) => {
      const event = raw as PointerEvent
      event.preventDefault()
      this.browserApply({ kind: "pointer-move", x: event.clientX, y: event.clientY, pointerId: event.pointerId })
    })
    this.listen(this.host, "pointerdown", (raw) => {
      const event = raw as PointerEvent
      event.preventDefault()
      const clicks = Math.max(1, Math.min(3, (event as PointerEvent & { detail?: number }).detail ?? 1)) as 1 | 2 | 3
      this.browserApply({ kind: "pointer-press", button: this.browserButton(event.button), x: event.clientX, y: event.clientY, pointerId: event.pointerId, clicks })
    })
    this.listen(this.host, "pointerup", (raw) => {
      const event = raw as PointerEvent
      event.preventDefault()
      this.browserApply({ kind: "pointer-release", button: this.browserButton(event.button), x: event.clientX, y: event.clientY, pointerId: event.pointerId })
    })
    this.listen(this.host, "pointercancel", (raw) => {
      const event = raw as PointerEvent
      event.preventDefault()
      if (this.capture !== null) this.browserApply({ kind: "set-pointer-capture", panel: null, initiatingButton: null, pointerId: null })
    })
    this.listen(this.host, "wheel", (raw) => {
      const event = raw as WheelEvent
      event.preventDefault()
      this.browserApply({ kind: "pointer-wheel", delta: Math.sign(-event.deltaY), x: event.clientX, y: event.clientY })
    }, { passive: false })
    this.listen(this.document, "keydown", (raw) => {
      const event = raw as KeyboardEvent
      const key = this.browserKey(event.key)
      if (["Tab", "Enter", "Space", "Escape", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "PageUp", "PageDown", "Home", "End", "Backspace", "Delete"].includes(key)) event.preventDefault()
      this.apply({ kind: "key-press", key, shift: event.shiftKey, control: event.ctrlKey, alt: event.altKey, meta: event.metaKey, repeat: event.repeat })
      this.apply({ kind: "key-typed", key, shift: event.shiftKey, control: event.ctrlKey, alt: event.altKey, meta: event.metaKey })
      this.browserFrame()
    })
    this.listen(this.document, "keyup", (raw) => {
      const event = raw as KeyboardEvent
      this.apply({ kind: "key-release", key: this.browserKey(event.key), shift: event.shiftKey, control: event.ctrlKey, alt: event.altKey, meta: event.metaKey })
      this.browserFrame()
    })
    this.listen(this.document, "input", (raw) => {
      const target = raw.target as HTMLInputElement | null
      const panelId = Number(target?.dataset.vguiPanel)
      if (!target || !Number.isSafeInteger(panelId)) return
      this.browserInputValue(panelId, target.value)
      this.browserFrame()
    })
    this.listen(this.document, "compositionstart", () => this.browserApply({ kind: "composition-start" }))
    this.listen(this.document, "compositionupdate", (raw) => {
      const event = raw as CompositionEvent
      this.browserApply({ kind: "composition-update", text: event.data, caret: event.data.length })
    })
    this.listen(this.document, "compositionend", (raw) => {
      const event = raw as CompositionEvent
      this.browserApply({ kind: "composition-end", text: event.data })
    })
    this.listen(this.document, "visibilitychange", () => {
      if (!this.document.hidden) return
      if (this.capture !== null) this.apply({ kind: "set-pointer-capture", panel: null, initiatingButton: null, pointerId: null })
      this.apply({ kind: "request-focus", panel: null })
      this.browserFrame()
    })
    if (window) {
      this.listen(window, "blur", () => {
        if (this.capture !== null) this.apply({ kind: "set-pointer-capture", panel: null, initiatingButton: null, pointerId: null })
        this.apply({ kind: "request-focus", panel: null })
        this.browserFrame()
      })
    }
  }

  private listen(target: EventTarget, type: string, listener: EventListener, options?: AddEventListenerOptions | boolean): void {
    if (this.listeners.length >= this.limits.maxListeners) throw new RuntimeFault("ListenerLimit", type)
    target.addEventListener(type, listener, options)
    this.listeners.push(Object.freeze({ target, type, listener, options }))
  }

  private browserApply(operation: VguiOperation): void {
    this.apply(operation)
    this.browserFrame()
  }

  private browserInputValue(panelId: VguiPanelId, value: string): void {
    const panel = this.panels.get(panelId)
    if (!panel || !panel.editable || panel.compositionActive || !validString(value, this.limits.maxTextCodeUnits)) return
    panel.text = ""
    panel.textSource = ""
    panel.caret = 0
    panel.selectionStart = -1
    panel.selectionEnd = 0
    this.insertText(panel, value)
    this.postAction(panel, "TextChanged", {})
    this.publishDom()
  }

  private browserFrame(): void {
    const now = this.clock.nowSeconds()
    if (!finite(now)) {
      this.recordDiagnostic("InvalidOperation", "browser-event", "clock")
      return
    }
    this.apply({ kind: "frame", timeSeconds: Math.max(this.timeSeconds, now) })
  }

  private browserButton(button: number): VguiPointerButton {
    if (button === 0) return "left"
    if (button === 1) return "middle"
    if (button === 2) return "right"
    if (button === 3) return "button4"
    return "button5"
  }

  private browserKey(key: string): string {
    if (key === " ") return "Space"
    return key
  }

  private destroy(): void {
    if (this.destroyed) return
    this.destroyed = true
    for (const record of this.listeners.splice(0).reverse()) record.target.removeEventListener(record.type, record.listener, record.options)
    if (this.capture !== null) {
      const panel = this.panels.get(this.capture)
      try {
        if (panel && this.pointerId !== null && panel.element.hasPointerCapture?.(this.pointerId)) panel.element.releasePointerCapture(this.pointerId)
      } catch {}
    }
    this.capture = null
    this.captureButton = null
    this.keyFocus = null
    this.calculatedKeyFocus = null
    this.requestedFocus = null
    this.clearFocusRequested = false
    this.mouseFocus = null
    this.mouseOver = null
    this.applicationModal = null
    this.modalSubtree = null
    this.outsideClickListener = null
    this.queuedMessages.splice(0)
    this.delayedCommands.splice(0)
    this.activeAnimations.splice(0)
    this.deferredDeletes.clear()
    this.pendingRequests.splice(0)
    this.pendingClipboardReads.clear()
    for (const panel of [...this.panels.values()].sort((left, right) => right.id - left.id)) panel.element.remove()
    this.panels.clear()
    this.auxiliaryNodes.clear()
    this.popups.splice(0)
    this.destroyDom()
  }

  private destroyDom(): void {
    this.host?.remove()
    this.style?.remove()
  }
}
