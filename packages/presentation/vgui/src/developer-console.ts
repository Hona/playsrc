import {
  SOURCE_CONSOLE_CEILINGS,
  type ConsoleCatalog,
  type ConsoleCatalogItem,
  type ConsoleCompletionCancellationReason,
  type ConsoleCompletionResult,
  type ConsoleDiagnostic,
  type ConsoleDiagnosticCode,
  type ConsoleLimits,
  type ConsoleOperation,
  type ConsoleOperationResult,
  type ConsoleOutputSegment,
  type ConsoleRequest,
  type ConsoleResourceResolution,
  type ConsoleResources,
  type ConsoleSnapshot,
  type ConsoleViewport,
  type DeveloperConsole,
  type DeveloperConsoleConfiguration,
  type DeveloperConsoleInitialization,
  type Rgba,
} from "./contract"
import { VguiControl, VguiDomRuntime } from "./panel-runtime"

type CompletionCandidate = Readonly<{
  label: string
  insertion: string
}>

type PendingCompletion = Readonly<{
  requestId: number
  catalogRevision: string
  commandName: string
  partialText: string
}>

type FrameBounds = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

type FrameInteraction = NonNullable<ConsoleSnapshot["frame"]["interaction"]>

type ConsoleModel = {
  runtimeIdentity: string
  lifecycle: "initialized" | "mounted" | "destroyed"
  revision: number
  visible: boolean
  foregroundRevision: number
  viewport: ConsoleViewport
  frameBounds: FrameBounds
  frameInteraction: FrameInteraction | null
  capturedPointerId: number | null
  reducedMotion: boolean
  resources: ConsoleResources
  catalog: ConsoleCatalog
  entryText: string
  output: ConsoleOutputSegment[]
  outputUtf8Bytes: number
  history: string[]
  historyCursor: number | null
  historyDraft: string
  completionSource: "none" | "catalog" | "owner"
  completion: CompletionCandidate[]
  completionSelected: number | null
  pendingCompletion: PendingCompletion | null
  diagnostics: ConsoleDiagnostic[]
}

type ViewCallbacks = Readonly<{
  input(value: string): void
  keydown(event: KeyboardEvent): void
  submit(): void
  focusout(event: FocusEvent): void
  completion(index: number): void
  visibility(): void
  foreground(): void
  frameBounds(bounds: FrameBounds): void
  frameInteraction(interaction: FrameInteraction | null, pointerId: number | null): void
}>

const encoder = new TextEncoder()
const STATIC_OWNED_NODES = 20
const STATIC_LISTENERS = 17

function utf8Bytes(value: string): number {
  return encoder.encode(value).byteLength
}

function hasValidUnicode(value: string): boolean {
  if (typeof value !== "string") return false
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index)
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false
      index += 1
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false
    }
  }
  return true
}

function asciiFold(value: string): string {
  let folded = ""
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    folded += code >= 65 && code <= 90 ? String.fromCharCode(code + 32) : value[index]
  }
  return folded
}

function finiteInteger(value: number, minimum: number, maximum = Number.MAX_SAFE_INTEGER): boolean {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum
}

function validLogicalIdentity(value: string): boolean {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 1024 &&
    value === value.toLowerCase() &&
    /^[\x20-\x7e]+$/.test(value) &&
    !value.startsWith("/") &&
    !value.includes("\\") &&
    value.split("/").every((part) => part !== "" && part !== "." && part !== "..")
  )
}

function validColor(value: Rgba): boolean {
  return Array.isArray(value) && value.length === 4 && value.every((channel) => finiteInteger(channel, 0, 255))
}

function cloneColor(value: Rgba): Rgba {
  return Object.freeze([value[0], value[1], value[2], value[3]]) as Rgba
}

function cloneResources(resources: ConsoleResources): ConsoleResources {
  const cloneFont = (font: ConsoleResources["fonts"]["console"]) =>
    Object.freeze({
      logicalIdentity: font.logicalIdentity,
      family: font.family,
      browserFamily: font.browserFamily,
      sizePxAt480: font.sizePxAt480,
      lineHeightPxAt480: font.lineHeightPxAt480,
      weight: font.weight,
      style: font.style,
      proportional: font.proportional,
      outlinePxAt480: font.outlinePxAt480,
    })
  const cloneBorder = (border: ConsoleResources["borders"]["frame"]) =>
    Object.freeze({
      logicalName: border.logicalName,
      colors: Object.freeze({
        left: cloneColor(border.colors.left),
        top: cloneColor(border.colors.top),
        right: cloneColor(border.colors.right),
        bottom: cloneColor(border.colors.bottom),
      }),
      widthsPxAt480: Object.freeze([...border.widthsPxAt480]) as typeof border.widthsPxAt480,
      insetPxAt480: Object.freeze([...border.insetPxAt480]) as typeof border.insetPxAt480,
      proportional: border.proportional,
    })
  return Object.freeze({
    identity: resources.identity,
    scheme: Object.freeze({ ...resources.scheme }),
    localization: Object.freeze({ ...resources.localization }),
    colors: Object.freeze({
      frameBackground: cloneColor(resources.colors.frameBackground),
      frameBackgroundUnfocused: cloneColor(resources.colors.frameBackgroundUnfocused),
      titleBackground: cloneColor(resources.colors.titleBackground),
      titleBackgroundUnfocused: cloneColor(resources.colors.titleBackgroundUnfocused),
      titleText: cloneColor(resources.colors.titleText),
      titleTextUnfocused: cloneColor(resources.colors.titleTextUnfocused),
      historyBackground: cloneColor(resources.colors.historyBackground),
      inputBackground: cloneColor(resources.colors.inputBackground),
      inputText: cloneColor(resources.colors.inputText),
      inputSelectionBackground: cloneColor(resources.colors.inputSelectionBackground),
      inputSelectionText: cloneColor(resources.colors.inputSelectionText),
      inputCursor: cloneColor(resources.colors.inputCursor),
      completionBackground: cloneColor(resources.colors.completionBackground),
      completionText: cloneColor(resources.colors.completionText),
      completionArmedBackground: cloneColor(resources.colors.completionArmedBackground),
      completionArmedText: cloneColor(resources.colors.completionArmedText),
      submitBackground: cloneColor(resources.colors.submitBackground),
      submitText: cloneColor(resources.colors.submitText),
      submitArmedBackground: cloneColor(resources.colors.submitArmedBackground),
      submitArmedText: cloneColor(resources.colors.submitArmedText),
      submitDepressedBackground: cloneColor(resources.colors.submitDepressedBackground),
      submitDepressedText: cloneColor(resources.colors.submitDepressedText),
      closeButton: cloneColor(resources.colors.closeButton),
      closeButtonUnfocused: cloneColor(resources.colors.closeButtonUnfocused),
      focus: cloneColor(resources.colors.focus),
      normalOutput: cloneColor(resources.colors.normalOutput),
      developerOutput: cloneColor(resources.colors.developerOutput),
    }),
    fonts: Object.freeze({
      title: cloneFont(resources.fonts.title),
      console: cloneFont(resources.fonts.console),
      entry: cloneFont(resources.fonts.entry),
      completion: cloneFont(resources.fonts.completion),
      submit: cloneFont(resources.fonts.submit),
    }),
    borders: Object.freeze({
      frame: cloneBorder(resources.borders.frame),
      history: cloneBorder(resources.borders.history),
      entry: cloneBorder(resources.borders.entry),
      submit: cloneBorder(resources.borders.submit),
      submitDepressed: cloneBorder(resources.borders.submitDepressed),
      completion: cloneBorder(resources.borders.completion),
    }),
    layout: Object.freeze({ ...resources.layout }),
  })
}

function validateResources(resolution: ConsoleResourceResolution): ConsoleDiagnosticCode | null {
  if (!resolution || typeof resolution !== "object") return "MalformedResource"
  if (resolution.kind === "missing") {
    return validLogicalIdentity(resolution.logicalIdentity) ? "MissingResource" : "MalformedResource"
  }
  if (resolution.kind === "malformed") return "MalformedResource"
  if (resolution.kind !== "resolved") return "MalformedResource"
  try {
    const resources = resolution.resources
    if (
      !resources ||
      !validLogicalIdentity(resources.identity) ||
      !validLogicalIdentity(resources.scheme.logicalIdentity) ||
      !validLogicalIdentity(resources.localization.logicalIdentity) ||
      resources.scheme.tag.length === 0 ||
      resources.scheme.revision.length === 0
    ) {
      return "MalformedResource"
    }
    const labels = Object.values(resources.localization)
    if (
      labels.some((value) => typeof value !== "string" || !hasValidUnicode(value)) ||
      resources.localization.title === ""
    ) {
      return "MalformedResource"
    }
    if (Object.values(resources.colors).some((color) => !validColor(color))) return "MalformedResource"
    for (const font of Object.values(resources.fonts)) {
      if (
        !validLogicalIdentity(font.logicalIdentity) ||
        font.family.length === 0 ||
        !hasValidUnicode(font.family) ||
        (font.browserFamily !== null && !/^[a-z][a-z0-9-]{0,127}$/u.test(font.browserFamily)) ||
        !Number.isFinite(font.sizePxAt480) ||
        font.sizePxAt480 <= 0 ||
        font.sizePxAt480 > 512 ||
        !Number.isFinite(font.lineHeightPxAt480) ||
        font.lineHeightPxAt480 < font.sizePxAt480 ||
        font.lineHeightPxAt480 > 1024 ||
        !finiteInteger(font.weight, 0, 1000) ||
        !["normal", "italic"].includes(font.style) ||
        typeof font.proportional !== "boolean" ||
        !Number.isFinite(font.outlinePxAt480) ||
        font.outlinePxAt480 < 0 ||
        font.outlinePxAt480 > 16
      ) {
        return "MalformedResource"
      }
    }
    const admittedFonts = Object.values(resources.fonts).filter((font) => font.browserFamily !== null).length
    if (admittedFonts !== 0 && admittedFonts !== Object.keys(resources.fonts).length) return "MalformedResource"
    for (const border of Object.values(resources.borders)) {
      if (
        border.logicalName.length === 0 ||
        !hasValidUnicode(border.logicalName) ||
        Object.values(border.colors).some((color) => !validColor(color)) ||
        !Array.isArray(border.widthsPxAt480) ||
        border.widthsPxAt480.length !== 4 ||
        border.widthsPxAt480.some((value) => !Number.isFinite(value) || value < 0 || value > 32) ||
        !Array.isArray(border.insetPxAt480) ||
        border.insetPxAt480.length !== 4 ||
        border.insetPxAt480.some((value) => !Number.isFinite(value) || value < -32 || value > 32) ||
        typeof border.proportional !== "boolean"
      ) return "MalformedResource"
    }
    const metrics = Object.values(resources.layout)
    if (
      metrics.some((value) => !Number.isFinite(value) || value < 0 || value > 1024) ||
      resources.layout.frameMinimumWidthPx < 1 ||
      resources.layout.frameMinimumHeightPx < 1 ||
      resources.layout.clientMinimumWidthPx < 1 ||
      resources.layout.clientMinimumHeightPx < 1 ||
      resources.layout.entryHeightPxAt480 < 1 ||
      resources.layout.closeButtonSizePxAt480 < 1 ||
      resources.layout.captionHeightPxAt480 < 1 ||
      resources.layout.titleBackgroundBottomPxAt480 <= resources.layout.titleBackgroundInsetPxAt480
    ) return "MalformedResource"
  } catch {
    return "MalformedResource"
  }
  return null
}

function cloneCatalog(catalog: ConsoleCatalog): ConsoleCatalog {
  return Object.freeze({
    revision: catalog.revision,
    items: Object.freeze(catalog.items.map((item) => Object.freeze({ ...item }))),
  })
}

function validateCatalog(catalog: ConsoleCatalog, limits: ConsoleLimits): boolean {
  if (
    !catalog ||
    typeof catalog.revision !== "string" ||
    catalog.revision.length === 0 ||
    !hasValidUnicode(catalog.revision) ||
    utf8Bytes(catalog.revision) > limits.maxCatalogItemUtf8Bytes
  ) {
    return false
  }
  if (!Array.isArray(catalog.items) || catalog.items.length > limits.maxCatalogItems) return false
  const names = new Set<string>()
  for (const item of catalog.items) {
    if (
      !item ||
      !["command", "convar"].includes(item.kind) ||
      !["visible", "hidden", "development"].includes(item.disposition) ||
      typeof item.name !== "string" ||
      item.name.length === 0 ||
      !hasValidUnicode(item.name) ||
      utf8Bytes(item.name) > limits.maxCatalogItemUtf8Bytes ||
      /[\s\u0000-\u001f\u007f]/u.test(item.name)
    ) {
      return false
    }
    const folded = asciiFold(item.name)
    if (names.has(folded)) return false
    names.add(folded)
    if (item.kind === "command" && typeof item.acceptsSuggestions !== "boolean") return false
    if (
      item.kind === "convar" &&
      (typeof item.displayValue !== "string" ||
        !hasValidUnicode(item.displayValue) ||
        utf8Bytes(`${item.name} ${item.displayValue}`) > limits.maxCatalogItemUtf8Bytes)
    ) {
      return false
    }
  }
  return true
}

function validateViewport(viewport: ConsoleViewport): boolean {
  return (
    !!viewport &&
    finiteInteger(viewport.width, 1, 32767) &&
    finiteInteger(viewport.height, 1, 32767) &&
    Number.isFinite(viewport.devicePixelRatio) &&
    viewport.devicePixelRatio >= 0.5 &&
    viewport.devicePixelRatio <= 8
  )
}

function validateLimits(limits: ConsoleLimits): boolean {
  if (!limits || typeof limits !== "object") return false
  const positive = Object.values(limits).every((value) => finiteInteger(value, 1))
  return (
    positive &&
    limits.maxInputUtf8Bytes <= SOURCE_CONSOLE_CEILINGS.maxInputUtf8Bytes &&
    limits.maxHistoryItems <= SOURCE_CONSOLE_CEILINGS.maxHistoryItems &&
    limits.maxCatalogItemUtf8Bytes <= SOURCE_CONSOLE_CEILINGS.maxInputUtf8Bytes &&
    limits.maxCompletionItems <= SOURCE_CONSOLE_CEILINGS.maxCompletionItems &&
    limits.maxCompletionItemUtf8Bytes <= SOURCE_CONSOLE_CEILINGS.maxCompletionItemUtf8Bytes &&
    limits.maxVisibleCompletionItems >= 2 &&
    limits.maxVisibleCompletionItems <= SOURCE_CONSOLE_CEILINGS.maxVisibleCompletionItems &&
    limits.maxOutputBatchSegments >= 3 &&
    limits.maxOutputSegments >= 3 &&
    limits.maxOutputBatchUtf8Bytes >= limits.maxInputUtf8Bytes + 3 &&
    limits.maxOutputUtf8Bytes >= limits.maxInputUtf8Bytes + 3 &&
    limits.maxDomNodes >= STATIC_OWNED_NODES + limits.maxOutputSegments + limits.maxVisibleCompletionItems &&
    limits.maxListeners >= STATIC_LISTENERS
  )
}

function rgba(color: Rgba): string {
  return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${color[3] / 255})`
}

function result(revision: number): ConsoleOperationResult {
  return Object.freeze({ ok: true, revision })
}

function initialFrameBounds(viewport: ConsoleViewport, resources: ConsoleResources): FrameBounds {
  const scale = viewport.height / 480
  const proportional = (value: number) => Math.trunc(value * scale)
  const offsetX = proportional(16)
  const offsetY = proportional(64)
  const minimumWidth = Math.min(viewport.width, resources.layout.frameMinimumWidthPx)
  const minimumHeight = Math.min(viewport.height, resources.layout.frameMinimumHeightPx)
  const width = Math.min(viewport.width, Math.max(minimumWidth, Math.trunc(viewport.width / 2)))
  const height = Math.min(viewport.height, Math.max(minimumHeight, viewport.height - offsetY * 2))
  return Object.freeze({
    x: Math.max(0, Math.min(viewport.width - width, Math.trunc(viewport.width / 2) - offsetX)),
    y: Math.max(0, Math.min(viewport.height - height, offsetY)),
    width,
    height,
  })
}

function boundedFrameBounds(
  bounds: FrameBounds,
  viewport: ConsoleViewport,
  resources?: ConsoleResources,
): FrameBounds {
  const minimumWidth = Math.min(viewport.width, resources?.layout.frameMinimumWidthPx ?? 1)
  const minimumHeight = Math.min(viewport.height, resources?.layout.frameMinimumHeightPx ?? 1)
  const width = Math.max(minimumWidth, Math.min(viewport.width, bounds.width))
  const height = Math.max(minimumHeight, Math.min(viewport.height, bounds.height))
  return Object.freeze({
    x: Math.max(0, Math.min(viewport.width - width, bounds.x)),
    y: Math.max(0, Math.min(viewport.height - height, bounds.y)),
    width,
    height,
  })
}

function initialFailure(code: ConsoleDiagnosticCode, subject?: string): DeveloperConsoleInitialization {
  return Object.freeze({
    ok: false,
    diagnostic: Object.freeze({ sequence: 1, code, operation: "initialize", ...(subject ? { subject } : {}) }),
  })
}

class ConsoleView {
  readonly runtime: VguiDomRuntime
  readonly frame: VguiControl
  readonly titleBackground: VguiControl
  readonly titleBar: VguiControl
  readonly title: VguiControl
  readonly close: VguiControl
  readonly grips: readonly VguiControl[]
  readonly client: VguiControl
  readonly history: VguiControl
  readonly entry: VguiControl
  readonly submit: VguiControl
  readonly completion: VguiControl
  private outputNodes: HTMLElement[] = []
  private completionItems: VguiControl[] = []
  private interaction: Readonly<{
    kind: FrameInteraction
    pointerId: number
    startX: number
    startY: number
    bounds: FrameBounds
  }> | null = null
  private viewport: ConsoleViewport = Object.freeze({ width: 1, height: 1, devicePixelRatio: 1 })
  private resources: ConsoleResources | null = null
  private windowActive = true

  constructor(root: HTMLElement, runtimeIdentity: string, limits: ConsoleLimits, callbacks: ViewCallbacks) {
    this.runtime = new VguiDomRuntime(root, limits)
    this.runtime.host.dataset.runtimeIdentity = runtimeIdentity
    this.runtime.host.dataset.vguiService = "developer-console"
    try {
      this.frame = this.runtime.createControl("Frame", "GameConsole", "section", null)
      this.frame.element.classList.add("playsrc-vgui-frame")
      this.frame.element.setAttribute("role", "dialog")
      this.frame.element.setAttribute("aria-modal", "false")

      this.titleBackground = this.runtime.createControl("Panel", "ConsoleTitleBackground", "div", this.frame)
      this.titleBackground.element.classList.add("playsrc-vgui-title-background")

      this.titleBar = this.runtime.createControl("Panel", "ConsoleTitleBar", "div", this.frame)
      this.titleBar.element.classList.add("playsrc-vgui-titlebar")

      this.title = this.runtime.createControl("Label", "ConsoleTitle", "div", this.frame)
      this.title.element.classList.add("playsrc-vgui-title")
      this.title.element.setAttribute("aria-hidden", "true")

      this.close = this.runtime.createControl("Button", "ConsoleClose", "button", this.frame)
      this.close.element.classList.add("playsrc-vgui-close")
      this.close.element.setAttribute("type", "button")
      this.close.element.setAttribute("aria-label", "Close console")

      this.grips = Object.freeze([
        ["n", "FrameGripNorth"],
        ["ne", "FrameGripNorthEast"],
        ["e", "FrameGripEast"],
        ["se", "FrameGripSouthEast"],
        ["s", "FrameGripSouth"],
        ["sw", "FrameGripSouthWest"],
        ["w", "FrameGripWest"],
        ["nw", "FrameGripNorthWest"],
      ].map(([edge, name]) => {
        const grip = this.runtime.createControl("Panel", name, "div", this.frame)
        grip.element.classList.add("playsrc-vgui-grip", `playsrc-vgui-grip-${edge}`)
        grip.element.dataset.resizeEdge = edge
        grip.element.setAttribute("aria-hidden", "true")
        return grip
      }))

      this.client = this.runtime.createControl("Panel", "ConsolePage", "div", this.frame)
      this.client.element.classList.add("playsrc-vgui-client")

      this.history = this.runtime.createControl("RichText", "ConsoleHistory", "div", this.client)
      this.history.element.classList.add("playsrc-vgui-history")
      this.history.element.setAttribute("role", "log")
      this.history.element.setAttribute("aria-live", "polite")
      this.history.element.setAttribute("aria-relevant", "additions text")
      this.history.element.setAttribute("aria-atomic", "false")
      this.history.element.tabIndex = -1

      this.entry = this.runtime.createControl("TextEntry", "ConsoleEntry", "input", this.client)
      this.entry.element.classList.add("playsrc-vgui-entry")
      this.entry.element.setAttribute("type", "text")
      this.entry.element.setAttribute("autocomplete", "off")
      this.entry.element.setAttribute("autocapitalize", "off")
      this.entry.element.setAttribute("spellcheck", "false")
      this.entry.element.setAttribute("aria-autocomplete", "list")

      this.submit = this.runtime.createControl("Button", "ConsoleSubmit", "button", this.client)
      this.submit.element.classList.add("playsrc-vgui-submit")
      this.submit.element.setAttribute("type", "button")

      this.completion = this.runtime.createControl("Menu", "CompletionList", "ul", this.client)
      this.completion.element.classList.add("playsrc-vgui-completion")
      this.completion.element.setAttribute("role", "listbox")
      const completionId = `${runtimeIdentity}-completion`
      this.completion.element.id = completionId
      this.entry.element.setAttribute("aria-controls", completionId)

      this.runtime.listen(this.entry.element, "input", (() => {
        callbacks.input((this.entry.element as HTMLInputElement).value)
      }) as EventListener)
      this.runtime.listen(
        this.entry.element,
        "keydown",
        ((event: KeyboardEvent) => callbacks.keydown(event)) as EventListener,
      )
      this.runtime.listen(
        this.entry.element,
        "focusout",
        ((event: FocusEvent) => callbacks.focusout(event)) as EventListener,
      )
      this.runtime.listen(this.submit.element, "click", (() => callbacks.submit()) as EventListener)
      this.runtime.listen(this.close.element, "click", (() => callbacks.visibility()) as EventListener)
      this.runtime.listen(
        this.completion.element,
        "pointerdown",
        ((event: Event) => event.preventDefault()) as EventListener,
      )
      this.runtime.listen(this.completion.element, "click", ((event: Event) => {
        let target = event.target as HTMLElement | null
        while (target && target !== this.completion.element) {
          const value = target.dataset.completionIndex
          if (value !== undefined) {
            callbacks.completion(Number(value))
            return
          }
          target = target.parentElement
        }
      }) as EventListener)
      this.runtime.listen(this.frame.element, "pointerdown", ((event: PointerEvent) => {
        this.beginFrameInteraction(event, callbacks)
      }) as EventListener)
      this.runtime.listen(this.frame.element, "pointermove", ((event: PointerEvent) => {
        this.moveFrameInteraction(event, callbacks)
      }) as EventListener)
      this.runtime.listen(this.frame.element, "pointerup", ((event: PointerEvent) => {
        this.endFrameInteraction(event.pointerId, callbacks)
      }) as EventListener)
      this.runtime.listen(this.frame.element, "pointercancel", ((event: PointerEvent) => {
        this.endFrameInteraction(event.pointerId, callbacks)
      }) as EventListener)
      this.runtime.listen(this.frame.element, "lostpointercapture", ((event: PointerEvent) => {
        this.loseFrameInteraction(event.pointerId, callbacks)
      }) as EventListener)
      this.runtime.listen(this.frame.element, "focusin", (() => {
        this.frame.element.dataset.focused = "true"
      }) as EventListener)
      this.runtime.listen(this.frame.element, "focusout", ((event: FocusEvent) => {
        const next = event.relatedTarget
        this.frame.element.dataset.focused = String(!!next && this.frame.element.contains(next as Node))
      }) as EventListener)
      const view = this.runtime.document.defaultView
      if (view) {
        this.runtime.listen(view, "blur", (() => {
          this.windowActive = false
          this.frame.element.dataset.focused = "false"
          this.cancelFrameInteraction(callbacks)
        }) as EventListener)
        this.runtime.listen(view, "focus", (() => {
          this.windowActive = true
          const active = this.runtime.document.activeElement
          this.frame.element.dataset.focused = String(!!active && this.frame.element.contains(active))
        }) as EventListener)
      }
      this.runtime.listen(this.runtime.document, "visibilitychange", (() => {
        if (this.runtime.document.hidden) {
          this.windowActive = false
          this.frame.element.dataset.focused = "false"
          this.cancelFrameInteraction(callbacks)
        } else {
          this.windowActive = true
          const active = this.runtime.document.activeElement
          this.frame.element.dataset.focused = String(!!active && this.frame.element.contains(active))
        }
      }) as EventListener)
    } catch (error) {
      this.runtime.destroy()
      throw error
    }
  }

  sync(model: ConsoleModel, limits: ConsoleLimits): void {
    this.syncResources(model.resources, model.viewport)
    this.syncOutput(model.output, model.resources)
    this.syncCompletion(model, limits)
    this.syncViewport(model, limits)
    this.setEntry(model.entryText)
    this.frame.element.setAttribute("aria-label", model.resources.localization.title)
    this.history.element.setAttribute("aria-label", model.resources.localization.historyAccessibleName)
    this.entry.element.setAttribute("aria-label", model.resources.localization.entryAccessibleName)
    this.completion.element.setAttribute("aria-label", model.resources.localization.completionAccessibleName)
    this.close.element.setAttribute("aria-label", model.resources.localization.closeAccessibleName)
    this.title.element.textContent = model.resources.localization.title
    this.submit.element.textContent = model.resources.localization.submit
    this.frame.setVisible(model.visible)
    if (model.frameInteraction) this.frame.element.dataset.interaction = model.frameInteraction
    else delete this.frame.element.dataset.interaction
    this.runtime.host.dataset.reducedMotion = String(model.reducedMotion)
    this.runtime.host.dataset.devicePixelRatio = String(model.viewport.devicePixelRatio)
  }

  syncResources(resources: ConsoleResources, viewport: ConsoleViewport): void {
    this.resources = resources
    const style = this.runtime.host.style
    const colorVariables: ReadonlyArray<readonly [string, Rgba]> = [
      ["--vgui-frame-background", resources.colors.frameBackground],
      ["--vgui-frame-background-unfocused", resources.colors.frameBackgroundUnfocused],
      ["--vgui-title-background", resources.colors.titleBackground],
      ["--vgui-title-background-unfocused", resources.colors.titleBackgroundUnfocused],
      ["--vgui-title-text", resources.colors.titleText],
      ["--vgui-title-text-unfocused", resources.colors.titleTextUnfocused],
      ["--vgui-history-background", resources.colors.historyBackground],
      ["--vgui-input-background", resources.colors.inputBackground],
      ["--vgui-input-text", resources.colors.inputText],
      ["--vgui-input-selection-background", resources.colors.inputSelectionBackground],
      ["--vgui-input-selection-text", resources.colors.inputSelectionText],
      ["--vgui-input-cursor", resources.colors.inputCursor],
      ["--vgui-completion-background", resources.colors.completionBackground],
      ["--vgui-completion-text", resources.colors.completionText],
      ["--vgui-completion-armed-background", resources.colors.completionArmedBackground],
      ["--vgui-completion-armed-text", resources.colors.completionArmedText],
      ["--vgui-submit-background", resources.colors.submitBackground],
      ["--vgui-submit-text", resources.colors.submitText],
      ["--vgui-submit-armed-background", resources.colors.submitArmedBackground],
      ["--vgui-submit-armed-text", resources.colors.submitArmedText],
      ["--vgui-submit-depressed-background", resources.colors.submitDepressedBackground],
      ["--vgui-submit-depressed-text", resources.colors.submitDepressedText],
      ["--vgui-close", resources.colors.closeButton],
      ["--vgui-close-unfocused", resources.colors.closeButtonUnfocused],
      ["--vgui-focus", resources.colors.focus],
      ["--vgui-normal-output", resources.colors.normalOutput],
      ["--vgui-developer-output", resources.colors.developerOutput],
    ]
    for (const [name, color] of colorVariables) style.setProperty(name, rgba(color))
    const scale = viewport.height / 480
    const platformFontsReady = Object.values(resources.fonts).every((font) => font.browserFamily !== null)
    this.runtime.host.dataset.platformFontCapability = platformFontsReady ? "supported" : "unsupported"
    if (platformFontsReady) {
      this.runtime.host.removeAttribute("aria-hidden")
      this.runtime.host.removeAttribute("inert")
    } else {
      this.runtime.host.setAttribute("aria-hidden", "true")
      this.runtime.host.setAttribute("inert", "")
    }
    for (const [role, font] of Object.entries(resources.fonts)) {
      const fontScale = font.proportional ? scale : 1
      style.setProperty(`--vgui-${role}-font`, font.browserFamily ?? "playsrc-vgui-platform-font-unavailable")
      style.setProperty(`--vgui-${role}-size`, `${Math.max(1, Math.trunc(font.sizePxAt480 * fontScale))}px`)
      style.setProperty(`--vgui-${role}-line-height`, `${Math.max(1, Math.trunc(font.lineHeightPxAt480 * fontScale))}px`)
      style.setProperty(`--vgui-${role}-weight`, font.weight === 0 ? "normal" : String(font.weight))
      style.setProperty(`--vgui-${role}-style`, font.style)
    }
    for (const [role, border] of Object.entries(resources.borders)) this.syncBorder(role, border, scale)
    style.setProperty("--vgui-completion-text-inset", `${Math.trunc(resources.layout.completionTextInsetPxAt480 * scale)}px`)
    style.setProperty("--vgui-close-glyph-size", `${resources.layout.closeGlyphSizePx}px`)
    style.setProperty("--vgui-history-draw-x", `${resources.layout.historyDrawOffsetXPx}px`)
    style.setProperty("--vgui-history-draw-y", `${resources.layout.historyDrawOffsetYPx}px`)
    style.setProperty("--vgui-entry-draw-x", `${Math.trunc(resources.layout.entryDrawOffsetXPxAt480 * scale)}px`)
    style.setProperty("--vgui-entry-draw-y", `${Math.trunc(resources.layout.entryDrawOffsetYPxAt480 * scale)}px`)
    style.setProperty("--vgui-frame-focus-transition", `${resources.layout.frameFocusTransitionSeconds}s`)
    this.runtime.host.dataset.resourceIdentity = resources.identity
    this.runtime.host.dataset.schemeIdentity = resources.scheme.logicalIdentity
    this.runtime.host.dataset.schemeTag = resources.scheme.tag
    this.runtime.host.dataset.schemeRevision = resources.scheme.revision
    this.runtime.host.dataset.localizationIdentity = resources.localization.logicalIdentity
    this.runtime.host.dataset.frameBorderIdentity = resources.borders.frame.logicalName
    this.runtime.host.dataset.historyBorderIdentity = resources.borders.history.logicalName
    this.runtime.host.dataset.entryBorderIdentity = resources.borders.entry.logicalName
    this.runtime.host.dataset.submitBorderIdentity = resources.borders.submit.logicalName
    this.runtime.host.dataset.completionBorderIdentity = resources.borders.completion.logicalName
  }

  syncViewport(model: ConsoleModel, limits: ConsoleLimits): void {
    this.viewport = model.viewport
    const { width: viewportWidth, height: viewportHeight } = model.viewport
    const scale = viewportHeight / 480
    const proportional = (value: number) => Math.trunc(value * scale)
    const { x: frameX, y: frameY, width: frameWidth, height: frameHeight } = model.frameBounds
    this.frame.setBounds(frameX, frameY, frameWidth, frameHeight)

    const layout = model.resources.layout
    const titleBackgroundInset = proportional(layout.titleBackgroundInsetPxAt480)
    const titleBackgroundBottom = proportional(layout.titleBackgroundBottomPxAt480)
    const titleBarHeight = Math.max(1, proportional(layout.captionHeightPxAt480))
    this.titleBar.setBounds(
      0,
      0,
      Math.max(0, frameWidth - proportional(10)),
      Math.min(frameHeight, titleBarHeight),
    )
    this.titleBackground.setBounds(
      titleBackgroundInset,
      titleBackgroundInset,
      Math.max(0, frameWidth - titleBackgroundInset * 2),
      Math.min(frameHeight, Math.max(1, titleBackgroundBottom - titleBackgroundInset)),
    )
    const titleFontScale = model.resources.fonts.title.proportional ? scale : 1
    const titleFontHeight = Math.max(1, Math.trunc(model.resources.fonts.title.lineHeightPxAt480 * titleFontScale))
    this.title.setBounds(
      Math.max(0, proportional(layout.titleTextInsetXPxAt480)),
      Math.max(0, proportional(layout.titleTextInsetYPxAt480)),
      Math.max(0, frameWidth - proportional(72)),
      titleFontHeight,
    )
    const closeSize = Math.max(1, proportional(layout.closeButtonSizePxAt480))
    const closeX = frameWidth - proportional(layout.closeButtonInsetRightPxAt480 + layout.closeButtonOffsetPxAt480)
    this.close.setBounds(
      Math.max(0, closeX),
      Math.max(0, proportional(layout.closeButtonInsetTopPxAt480)),
      Math.min(closeSize, frameWidth),
      Math.min(closeSize, frameHeight),
    )
    const grip = Math.max(1, proportional(layout.resizeGripPxAt480))
    const corner = Math.max(1, proportional(layout.resizeCornerPxAt480))
    const bottomRight = Math.max(1, proportional(layout.resizeBottomRightPxAt480))
    const [north, northEast, east, southEast, south, southWest, west, northWest] = this.grips
    north.setBounds(corner, 0, Math.max(0, frameWidth - corner * 2), grip)
    northEast.setBounds(Math.max(0, frameWidth - corner), 0, corner, corner)
    east.setBounds(Math.max(0, frameWidth - grip), corner, grip, Math.max(0, frameHeight - corner - bottomRight))
    southEast.setBounds(Math.max(0, frameWidth - bottomRight), Math.max(0, frameHeight - bottomRight), bottomRight, bottomRight)
    south.setBounds(corner, Math.max(0, frameHeight - grip), Math.max(0, frameWidth - corner - bottomRight), grip)
    southWest.setBounds(0, Math.max(0, frameHeight - corner), corner, corner)
    west.setBounds(0, corner, grip, Math.max(0, frameHeight - corner * 2))
    northWest.setBounds(0, 0, corner, corner)

    const clientX = Math.min(frameWidth, layout.clientInsetXPx)
    const clientYInset = proportional(layout.clientInsetYPxAt480)
    const clientY = Math.min(
      frameHeight,
      clientYInset
        + titleFontHeight
        + proportional(layout.captionTitleBorderPxAt480)
        + proportional(layout.clientTitleGapPxAt480),
    )
    const clientWidth = Math.max(layout.clientMinimumWidthPx, frameWidth - clientX * 2)
    const clientHeight = Math.max(layout.clientMinimumHeightPx, frameHeight - clientYInset - clientY)
    this.client.setBounds(clientX, clientY, clientWidth, clientHeight)

    const inset = Math.max(0, proportional(layout.consoleInsetPxAt480))
    const topHeight = Math.max(0, proportional(layout.historyTopOffsetPxAt480))
    const entryHeight = Math.max(1, proportional(layout.entryHeightPxAt480))
    const entryInset = Math.max(0, proportional(layout.entryInsetPxAt480))
    const submitWidth = Math.max(1, proportional(layout.submitWidthPxAt480))
    const submitInset = Math.max(0, proportional(layout.submitInsetPxAt480))
    const historyWidth = Math.max(0, clientWidth - inset * 2)
    const historyHeight = Math.max(0, clientHeight - (entryInset * 2 + inset * 2 + topHeight + entryHeight))
    this.history.setBounds(inset, inset + topHeight, historyWidth, historyHeight)
    const submitX = Math.max(inset, clientWidth - (inset + submitWidth + submitInset))
    const entryY = Math.max(0, clientHeight - (entryInset * 2 + entryHeight))
    this.submit.setBounds(submitX, entryY, Math.min(submitWidth, Math.max(0, clientWidth - submitX)), entryHeight)
    const entryWidth = Math.max(0, submitX - entryInset - inset * 2)
    this.entry.setBounds(inset, entryY, entryWidth, entryHeight)

    const completionScale = model.resources.fonts.completion.proportional ? scale : 1
    const rowHeight = Math.max(
      1,
      Math.trunc(model.resources.fonts.completion.lineHeightPxAt480 * completionScale)
        + proportional(layout.completionRowPaddingPxAt480),
    )
    const visibleRows = Math.min(limits.maxVisibleCompletionItems, model.completion.length)
    const completionBorder = model.resources.borders.completion
    const completionBorderScale = completionBorder.proportional ? scale : 1
    const completionBorderHeight = Math.trunc(completionBorder.widthsPxAt480[1] * completionBorderScale)
      + Math.trunc(completionBorder.widthsPxAt480[3] * completionBorderScale)
    const desiredPopupHeight = rowHeight * Math.max(1, visibleRows) + completionBorderHeight
    const globalBelow = frameY + clientY + entryY + entryHeight
    const below = Math.max(0, viewportHeight - globalBelow)
    const popupHeight = Math.min(desiredPopupHeight, Math.max(rowHeight, below || desiredPopupHeight))
    const popupTop = below >= rowHeight ? entryY + entryHeight : Math.max(0, entryY - popupHeight)
    this.completion.setBounds(inset, popupTop, entryWidth, popupHeight)
    for (const item of this.completionItems) item.element.style.height = `${rowHeight}px`
  }

  private syncBorder(role: string, border: ConsoleResources["borders"]["frame"], viewportScale: number): void {
    const name = role.replace(/[A-Z]/gu, (letter) => `-${letter.toLowerCase()}`)
    const scale = border.proportional ? viewportScale : 1
    const sides = ["left", "top", "right", "bottom"] as const
    for (const [index, side] of sides.entries()) {
      this.runtime.host.style.setProperty(`--vgui-${name}-border-${side}`, rgba(border.colors[side]))
      this.runtime.host.style.setProperty(
        `--vgui-${name}-border-width-${side}`,
        `${Math.max(0, Math.trunc(border.widthsPxAt480[index] * scale))}px`,
      )
      this.runtime.host.style.setProperty(
        `--vgui-${name}-inset-${side}`,
        `${Math.trunc(border.insetPxAt480[index] * scale)}px`,
      )
    }
  }

  syncOutput(output: readonly ConsoleOutputSegment[], resources: ConsoleResources): void {
    this.history.element.replaceChildren()
    for (const node of this.outputNodes) this.runtime.releaseOwnedNode(node)
    this.outputNodes = []
    for (const segment of output) {
      const node = this.runtime.createOwnedNode("span")
      node.className = "playsrc-vgui-output-segment"
      node.dataset.outputKind = segment.kind
      node.textContent = segment.text
      if (segment.kind === "normal") node.style.color = rgba(resources.colors.normalOutput)
      else if (segment.kind === "developer") node.style.color = rgba(resources.colors.developerOutput)
      else node.style.color = rgba(segment.color)
      this.history.element.append(node)
      this.outputNodes.push(node)
    }
    this.history.element.scrollTop = this.history.element.scrollHeight
  }

  syncCompletion(model: ConsoleModel, limits: ConsoleLimits): void {
    for (const item of [...this.completionItems]) item.destroy()
    this.completionItems = []
    this.completion.element.replaceChildren()
    const visible = model.completion.length > 0
    const selectableCount =
      model.completion.length > limits.maxVisibleCompletionItems
        ? limits.maxVisibleCompletionItems - 1
        : Math.min(model.completion.length, limits.maxVisibleCompletionItems)
    for (let index = 0; index < selectableCount; index += 1) {
      const candidate = model.completion[index]
      const item = this.runtime.createControl("MenuItem", `Completion${index}`, "li", this.completion)
      item.element.classList.add("playsrc-vgui-menu-item")
      item.element.setAttribute("role", "option")
      item.element.dataset.completionIndex = String(index)
      item.element.id = `${this.completion.element.id}-item-${index}`
      item.element.textContent = candidate.label
      item.element.setAttribute("aria-selected", String(model.completionSelected === index))
      item.element.setAttribute("aria-posinset", String(index + 1))
      item.element.setAttribute("aria-setsize", String(model.completion.length))
      this.completionItems.push(item)
    }
    if (model.completion.length > limits.maxVisibleCompletionItems) {
      const overflow = this.runtime.createControl("MenuItem", "CompletionOverflow", "li", this.completion)
      overflow.element.classList.add("playsrc-vgui-menu-item", "playsrc-vgui-menu-overflow")
      overflow.element.setAttribute("role", "presentation")
      overflow.element.textContent = "…"
      this.completionItems.push(overflow)
    }
    this.completion.setVisible(visible)
    this.entry.element.setAttribute("aria-expanded", String(visible))
    const selected = model.completionSelected
    if (selected !== null && selected < selectableCount) {
      this.entry.element.setAttribute("aria-activedescendant", `${this.completion.element.id}-item-${selected}`)
    } else {
      this.entry.element.removeAttribute("aria-activedescendant")
    }
  }

  private beginFrameInteraction(event: PointerEvent, callbacks: ViewCallbacks): void {
    if (event.button !== 0 || this.interaction || !this.resources) return
    const target = event.target
    if (target && this.close.element.contains(target as Node)) return
    const frameRect = this.frame.element.getBoundingClientRect()
    const localX = event.clientX - frameRect.left
    const localY = event.clientY - frameRect.top
    const resizeEdge = target && "dataset" in (target as HTMLElement)
      ? (target as HTMLElement).dataset.resizeEdge
      : undefined
    let kind: FrameInteraction | null = null
    if (resizeEdge) kind = `resize-${resizeEdge}` as FrameInteraction
    else if (target === this.frame.element) kind = this.resizeInteraction(localX, localY)
    else if (target && this.titleBar.element.contains(target as Node)) kind = "move"
    if (!kind) return
    event.preventDefault()
    callbacks.foreground()
    try {
      this.frame.element.setPointerCapture(event.pointerId)
      if (!this.frame.element.hasPointerCapture(event.pointerId)) return
    } catch {
      return
    }
    this.interaction = Object.freeze({
      kind,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      bounds: Object.freeze({
        x: Number.parseInt(this.frame.element.style.left, 10),
        y: Number.parseInt(this.frame.element.style.top, 10),
        width: Number.parseInt(this.frame.element.style.width, 10),
        height: Number.parseInt(this.frame.element.style.height, 10),
      }),
    })
    callbacks.frameInteraction(kind, event.pointerId)
  }

  private resizeInteraction(x: number, y: number): FrameInteraction | null {
    if (!this.resources) return null
    const scale = this.viewport.height / 480
    const proportional = (value: number) => Math.max(1, Math.trunc(value * scale))
    const width = Number.parseInt(this.frame.element.style.width, 10)
    const height = Number.parseInt(this.frame.element.style.height, 10)
    const corner = proportional(this.resources.layout.resizeCornerPxAt480)
    const grip = proportional(this.resources.layout.resizeGripPxAt480)
    const bottomRight = proportional(this.resources.layout.resizeBottomRightPxAt480)
    const west = x >= 0 && x < (y >= height - corner ? corner : grip)
    const east = x <= width && x > width - (y >= height - bottomRight ? bottomRight : grip)
    const north = y >= 0 && y < (x < corner || x > width - corner ? corner : grip)
    const south = y <= height && y > height - (x > width - bottomRight ? bottomRight : x < corner ? corner : grip)
    if (north && west) return "resize-nw"
    if (north && east) return "resize-ne"
    if (south && west) return "resize-sw"
    if (south && east) return "resize-se"
    if (north) return "resize-n"
    if (east) return "resize-e"
    if (south) return "resize-s"
    if (west) return "resize-w"
    return null
  }

  private moveFrameInteraction(event: PointerEvent, callbacks: ViewCallbacks): void {
    const interaction = this.interaction
    if (!interaction || interaction.pointerId !== event.pointerId || !this.resources) return
    if (event.buttons === 0) {
      this.endFrameInteraction(event.pointerId, callbacks)
      return
    }
    event.preventDefault()
    const dx = event.clientX - interaction.startX
    const dy = event.clientY - interaction.startY
    const source = interaction.bounds
    if (interaction.kind === "move") {
      callbacks.frameBounds(Object.freeze({
        ...source,
        x: Math.max(0, Math.min(this.viewport.width - source.width, source.x + dx)),
        y: Math.max(0, Math.min(this.viewport.height - source.height, source.y + dy)),
      }))
      return
    }
    const minimumWidth = Math.min(this.viewport.width, this.resources.layout.frameMinimumWidthPx)
    const minimumHeight = Math.min(this.viewport.height, this.resources.layout.frameMinimumHeightPx)
    let left = source.x
    let top = source.y
    let right = source.x + source.width
    let bottom = source.y + source.height
    const direction = interaction.kind.slice("resize-".length)
    if (direction.includes("w")) left = Math.max(0, Math.min(right - minimumWidth, source.x + dx))
    if (direction.includes("e")) right = Math.min(this.viewport.width, Math.max(left + minimumWidth, source.x + source.width + dx))
    if (direction.includes("n")) top = Math.max(0, Math.min(bottom - minimumHeight, source.y + dy))
    if (direction.includes("s")) bottom = Math.min(this.viewport.height, Math.max(top + minimumHeight, source.y + source.height + dy))
    callbacks.frameBounds(Object.freeze({ x: left, y: top, width: right - left, height: bottom - top }))
  }

  private endFrameInteraction(pointerId: number, callbacks: ViewCallbacks): void {
    if (!this.interaction || this.interaction.pointerId !== pointerId) return
    this.interaction = null
    try {
      if (this.frame.element.hasPointerCapture(pointerId)) this.frame.element.releasePointerCapture(pointerId)
    } catch {}
    callbacks.frameInteraction(null, null)
  }

  private loseFrameInteraction(pointerId: number, callbacks: ViewCallbacks): void {
    if (!this.interaction || this.interaction.pointerId !== pointerId) return
    this.interaction = null
    callbacks.frameInteraction(null, null)
  }

  cancelFrameInteraction(callbacks?: ViewCallbacks): void {
    const interaction = this.interaction
    if (!interaction) return
    this.interaction = null
    try {
      if (this.frame.element.hasPointerCapture(interaction.pointerId)) {
        this.frame.element.releasePointerCapture(interaction.pointerId)
      }
    } catch {}
    callbacks?.frameInteraction(null, null)
  }

  setEntry(value: string): void {
    const input = this.entry.element as HTMLInputElement
    if (input.value !== value) input.value = value
  }

  focusEntry(): void {
    ;(this.entry.element as HTMLInputElement).focus({ preventScroll: true })
  }

  blurOwnedFocus(): void {
    const active = this.runtime.document.activeElement
    if (active && this.frame.element.contains(active) && typeof (active as HTMLElement).blur === "function") {
      ;(active as HTMLElement).blur()
    }
  }

  entryFocused(): boolean {
    return this.windowActive && this.runtime.document.activeElement === this.entry.element
  }

  moveToFront(): void {
    this.frame.moveToFront()
  }

  counts(): Readonly<{ nodes: number; listeners: number }> {
    return this.runtime.counts()
  }

  destroy(): void {
    this.cancelFrameInteraction()
    this.runtime.destroy()
    this.outputNodes = []
    this.completionItems = []
  }
}

class DeveloperConsoleImplementation implements DeveloperConsole {
  private readonly limits: ConsoleLimits
  private readonly onRequest: (request: ConsoleRequest) => void
  private readonly model: ConsoleModel
  private view: ConsoleView | null = null
  private nextRequestId = 1
  private nextDiagnosticSequence = 1
  private readonly requestQueue: ConsoleRequest[] = []

  constructor(configuration: DeveloperConsoleConfiguration, resources: ConsoleResources, catalog: ConsoleCatalog) {
    this.limits = Object.freeze({ ...configuration.limits })
    this.onRequest = configuration.onRequest
    this.model = {
      runtimeIdentity: configuration.runtimeIdentity,
      lifecycle: "initialized",
      revision: 0,
      visible: false,
      foregroundRevision: 0,
      viewport: Object.freeze({ ...configuration.viewport }),
      frameBounds: initialFrameBounds(configuration.viewport, resources),
      frameInteraction: null,
      capturedPointerId: null,
      reducedMotion: configuration.reducedMotion,
      resources,
      catalog,
      entryText: "",
      output: [],
      outputUtf8Bytes: 0,
      history: [],
      historyCursor: null,
      historyDraft: "",
      completionSource: "none",
      completion: [],
      completionSelected: null,
      pendingCompletion: null,
      diagnostics: [],
    }
  }

  apply(operation: ConsoleOperation): ConsoleOperationResult {
    if (this.model.lifecycle === "destroyed") {
      if (operation.kind === "destroy") return result(this.model.revision)
      return this.fail("Destroyed", operation.kind)
    }
    try {
      switch (operation.kind) {
        case "mount":
          return this.mount(operation.root)
        case "replace-root":
          return this.replaceRoot(operation.root)
        case "activate":
          if (!this.requireMounted(operation.kind)) return this.fail("NotMounted", operation.kind)
          this.model.visible = true
          this.model.foregroundRevision += 1
          this.commit()
          this.view?.moveToFront()
          this.view?.focusEntry()
          return result(this.model.revision)
        case "foreground":
          if (!this.requireMounted(operation.kind)) return this.fail("NotMounted", operation.kind)
          this.model.foregroundRevision += 1
          this.commit()
          this.view?.moveToFront()
          return result(this.model.revision)
        case "focus-entry":
          if (!this.requireMounted(operation.kind)) return this.fail("NotMounted", operation.kind)
          if (!this.model.visible) return this.fail("NotVisible", operation.kind)
          this.view?.focusEntry()
          this.commit(false)
          return result(this.model.revision)
        case "hide":
          if (!this.requireMounted(operation.kind)) return this.fail("NotMounted", operation.kind)
          this.cancelPending("hidden")
          this.closeCompletion()
          this.model.visible = false
          this.view?.cancelFrameInteraction()
          this.model.frameInteraction = null
          this.model.capturedPointerId = null
          this.view?.blurOwnedFocus()
          this.commit()
          return result(this.model.revision)
        case "clear-output":
          this.model.output = []
          this.model.outputUtf8Bytes = 0
          this.commit()
          return result(this.model.revision)
        case "append-output":
          if (!this.appendOutput(operation.segments)) return this.fail("OutputLimit", operation.kind)
          this.commit()
          return result(this.model.revision)
        case "replace-catalog":
          if (!validateCatalog(operation.catalog, this.limits)) return this.fail("MalformedCatalog", operation.kind)
          this.cancelPending("catalog-replaced")
          this.model.catalog = cloneCatalog(operation.catalog)
          this.rebuildCompletion()
          this.commit()
          return result(this.model.revision)
        case "apply-completion":
          return this.applyCompletion(operation.result)
        case "replace-resources": {
          const code = validateResources(operation.resolution)
          if (code) {
            const candidate =
              operation.resolution && operation.resolution.kind !== "resolved"
                ? operation.resolution.logicalIdentity
                : undefined
            const subject = candidate && validLogicalIdentity(candidate) ? candidate : undefined
            return this.fail(code, operation.kind, subject)
          }
          this.model.resources = cloneResources((operation.resolution as Extract<ConsoleResourceResolution, { kind: "resolved" }>).resources)
          this.model.frameBounds = boundedFrameBounds(this.model.frameBounds, this.model.viewport, this.model.resources)
          this.commit()
          return result(this.model.revision)
        }
        case "set-viewport":
          if (!validateViewport(operation.viewport)) return this.fail("InvalidViewport", operation.kind)
          this.model.viewport = Object.freeze({ ...operation.viewport })
          this.model.frameBounds = boundedFrameBounds(this.model.frameBounds, this.model.viewport, this.model.resources)
          this.commit()
          return result(this.model.revision)
        case "set-reduced-motion":
          this.model.reducedMotion = operation.reduced
          this.commit()
          return result(this.model.revision)
        case "cancel":
          this.cancelPending("cancelled")
          this.closeCompletion()
          this.commit()
          return result(this.model.revision)
        case "destroy":
          this.cancelPending("destroyed")
          this.view?.cancelFrameInteraction()
          this.view?.destroy()
          this.view = null
          this.model.visible = false
          this.model.lifecycle = "destroyed"
          this.model.completion = []
          this.model.completionSource = "none"
          this.model.completionSelected = null
          this.model.pendingCompletion = null
          this.model.frameInteraction = null
          this.model.capturedPointerId = null
          this.model.revision += 1
          this.flushRequests()
          return result(this.model.revision)
      }
    } catch (error) {
      const code = error instanceof Error && error.message === "DomLimit" ? "DomLimit" : error instanceof Error && error.message === "ListenerLimit" ? "ListenerLimit" : "DomFailure"
      return this.fail(code, operation.kind)
    }
  }

  snapshot(): ConsoleSnapshot {
    const counts = this.view?.counts() ?? { nodes: 0, listeners: 0 }
    return Object.freeze({
      runtimeIdentity: this.model.runtimeIdentity,
      lifecycle: this.model.lifecycle,
      revision: this.model.revision,
      visible: this.model.visible,
      focused: this.view?.entryFocused() ?? false,
      foregroundRevision: this.model.foregroundRevision,
      viewport: Object.freeze({ ...this.model.viewport }),
      reducedMotion: this.model.reducedMotion,
      resourceIdentity: this.model.resources.identity,
      catalogRevision: this.model.catalog.revision,
      entryText: this.model.entryText,
      output: Object.freeze(this.model.output.map(cloneOutputSegment)),
      outputUtf8Bytes: this.model.outputUtf8Bytes,
      history: Object.freeze([...this.model.history]),
      historyCursor: this.model.historyCursor,
      completion: Object.freeze({
        source: this.model.completionSource,
        labels: Object.freeze(this.model.completion.map((candidate) => candidate.label)),
        selectedIndex: this.model.completionSelected,
        visible: this.model.completion.length > 0,
        pendingRequestId: this.model.pendingCompletion?.requestId ?? null,
      }),
      frame: Object.freeze({
        ...this.model.frameBounds,
        interaction: this.model.frameInteraction,
        capturedPointerId: this.model.capturedPointerId,
      }),
      diagnostics: Object.freeze([...this.model.diagnostics]),
      ownedResources: Object.freeze({
        nodes: counts.nodes,
        listeners: counts.listeners,
        observers: 0 as const,
        timers: 0 as const,
      }),
    })
  }

  private mount(root: HTMLElement): ConsoleOperationResult {
    if (this.model.lifecycle === "mounted") return this.fail("AlreadyMounted", "mount")
    const view = this.createView(root)
    this.view = view
    this.model.lifecycle = "mounted"
    this.commit()
    return result(this.model.revision)
  }

  private replaceRoot(root: HTMLElement): ConsoleOperationResult {
    if (!this.requireMounted("replace-root")) return this.fail("NotMounted", "replace-root")
    if (this.view?.runtime.root === root) return result(this.model.revision)
    const wasFocused = this.view?.entryFocused() ?? false
    const replacement = this.createView(root)
    this.cancelPending("root-replaced")
    this.closeCompletion()
    this.model.frameInteraction = null
    this.model.capturedPointerId = null
    const previous = this.view
    this.view = replacement
    replacement.sync(this.model, this.limits)
    previous?.destroy()
    this.commit(false)
    if (wasFocused && this.model.visible) replacement.focusEntry()
    return result(this.model.revision)
  }

  private createView(root: HTMLElement): ConsoleView {
    let view: ConsoleView | null = null
    try {
      view = new ConsoleView(root, this.model.runtimeIdentity, this.limits, {
        input: (value) => this.onInput(value),
        keydown: (event) => this.onKeydown(event),
        submit: () => this.submit(),
        focusout: (event) => this.onFocusout(event),
        completion: (index) => this.selectCompletion(index),
        visibility: () => this.requestVisibility("frame-close"),
        foreground: () => this.foregroundFromFrame(),
        frameBounds: (bounds) => this.updateFrameBounds(bounds),
        frameInteraction: (interaction, pointerId) => this.updateFrameInteraction(interaction, pointerId),
      })
      view.sync(this.model, this.limits)
      return view
    } catch (error) {
      view?.destroy()
      throw error
    }
  }

  private requireMounted(_operation: ConsoleOperation["kind"]): boolean {
    return this.model.lifecycle === "mounted" && this.view !== null
  }

  private commit(sync = true): void {
    this.model.revision += 1
    if (sync) this.view?.sync(this.model, this.limits)
    this.flushRequests()
  }

  private fail(
    code: ConsoleDiagnosticCode,
    operation: ConsoleDiagnostic["operation"],
    subject?: string,
  ): ConsoleOperationResult {
    const diagnostic = Object.freeze({
      sequence: this.nextDiagnosticSequence++,
      code,
      operation,
      ...(subject ? { subject } : {}),
    })
    this.model.diagnostics.push(diagnostic)
    while (this.model.diagnostics.length > this.limits.maxDiagnostics) this.model.diagnostics.shift()
    this.model.revision += 1
    return Object.freeze({ ok: false, diagnostic })
  }

  private onInput(value: string): void {
    if (!hasValidUnicode(value)) {
      this.fail("MalformedText", "input")
      this.view?.setEntry(this.model.entryText)
      return
    }
    if (utf8Bytes(value) > this.limits.maxInputUtf8Bytes) {
      this.fail("InputLimit", "input")
      this.view?.setEntry(this.model.entryText)
      return
    }
    this.cancelPending("input-changed")
    this.model.entryText = value
    this.model.historyCursor = null
    this.model.historyDraft = ""
    this.rebuildCompletion()
    this.commit()
  }

  private onKeydown(event: KeyboardEvent): void {
    if (
      (event.code === "Backquote" || event.key === "`" || event.key === "~") &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey
    ) {
      event.preventDefault()
      this.requestVisibility("entry-backquote")
      return
    }
    if (event.key === "Enter") {
      event.preventDefault()
      this.submit()
      return
    }
    if (event.key === "Tab" && this.model.completion.length > 0) {
      event.preventDefault()
      this.cycleCompletion(event.shiftKey ? -1 : 1, true)
      return
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      if (this.model.completion.length > 0) this.cycleCompletion(event.key === "ArrowDown" ? 1 : -1, false)
      else this.traverseHistory(event.key === "ArrowUp" ? -1 : 1)
      return
    }
    if (event.key === "Escape" && this.model.completion.length > 0) {
      event.preventDefault()
      this.cancelPending("cancelled")
      this.closeCompletion()
      this.commit()
    }
  }

  private onFocusout(event: FocusEvent): void {
    const next = event.relatedTarget
    if (next && this.view?.completion.element.contains(next as Node)) return
    this.cancelPending("cancelled")
    this.closeCompletion()
    this.commit()
  }

  private requestVisibility(reason: "entry-backquote" | "frame-close"): void {
    this.cancelPending("hidden")
    this.closeCompletion()
    this.emitRequest(Object.freeze({
      kind: "visibility",
      requestId: this.nextRequestId++,
      operation: "hide",
      reason,
    }))
    this.commit()
  }

  private foregroundFromFrame(): void {
    if (this.model.lifecycle !== "mounted" || !this.model.visible) return
    this.model.foregroundRevision += 1
    this.commit()
    this.view?.moveToFront()
    this.view?.focusEntry()
  }

  private updateFrameBounds(bounds: FrameBounds): void {
    if (this.model.lifecycle !== "mounted") return
    if (![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)) return
    this.model.frameBounds = boundedFrameBounds(bounds, this.model.viewport, this.model.resources)
    this.model.revision += 1
    this.view?.syncViewport(this.model, this.limits)
    this.flushRequests()
  }

  private updateFrameInteraction(interaction: FrameInteraction | null, pointerId: number | null): void {
    if (this.model.lifecycle !== "mounted") return
    this.model.frameInteraction = interaction
    this.model.capturedPointerId = interaction ? pointerId : null
    this.commit()
  }

  private submit(): void {
    const text = this.model.entryText
    this.cancelPending("submitted")
    this.appendOutput([
      Object.freeze({ kind: "normal", text: "] " }),
      Object.freeze({ kind: "normal", text }),
      Object.freeze({ kind: "normal", text: "\n" }),
    ])
    const historyValue = text.replace(/ +$/u, "")
    if (historyValue.length > 0 && historyValue[0] !== " ") {
      const folded = asciiFold(historyValue)
      this.model.history = this.model.history.filter((item) => asciiFold(item) !== folded)
      this.model.history.push(historyValue)
      while (this.model.history.length > this.limits.maxHistoryItems) this.model.history.shift()
    }
    this.model.entryText = ""
    this.model.historyCursor = null
    this.model.historyDraft = ""
    this.closeCompletion()
    this.emitRequest(
      Object.freeze({
        kind: "submission",
        requestId: this.nextRequestId++,
        text,
        catalogRevision: this.model.catalog.revision,
        maxExecutionUtf8Bytes: SOURCE_CONSOLE_CEILINGS.maxExecutionUtf8Bytes,
        maxExecutionArguments: SOURCE_CONSOLE_CEILINGS.maxExecutionArguments,
      }),
    )
    this.commit()
    this.view?.focusEntry()
  }

  private appendOutput(segments: readonly ConsoleOutputSegment[]): boolean {
    if (!Array.isArray(segments) || segments.length > this.limits.maxOutputBatchSegments) return false
    let batchBytes = 0
    const cloned: ConsoleOutputSegment[] = []
    for (const segment of segments) {
      if (
        !segment ||
        !["normal", "developer", "color"].includes(segment.kind) ||
        typeof segment.text !== "string" ||
        !hasValidUnicode(segment.text)
      ) {
        return false
      }
      if (segment.kind === "color" && !validColor(segment.color)) return false
      const bytes = utf8Bytes(segment.text)
      batchBytes += bytes
      if (bytes > this.limits.maxOutputUtf8Bytes || batchBytes > this.limits.maxOutputBatchUtf8Bytes) return false
      if (segment.text.length > 0) cloned.push(cloneOutputSegment(segment))
    }
    this.model.output.push(...cloned)
    this.model.outputUtf8Bytes += cloned.reduce((total, segment) => total + utf8Bytes(segment.text), 0)
    while (
      this.model.output.length > this.limits.maxOutputSegments ||
      this.model.outputUtf8Bytes > this.limits.maxOutputUtf8Bytes
    ) {
      const removed = this.model.output.shift()
      if (removed) this.model.outputUtf8Bytes -= utf8Bytes(removed.text)
    }
    return true
  }

  private rebuildCompletion(): void {
    this.closeCompletion()
    const text = this.model.entryText
    if (text.length === 0) return
    const space = text.indexOf(" ")
    if (space >= 0) {
      const commandName = text.slice(0, space)
      const descriptor = this.model.catalog.items.find(
        (item): item is Extract<ConsoleCatalogItem, { kind: "command" }> =>
          item.kind === "command" &&
          item.disposition === "visible" &&
          item.acceptsSuggestions &&
          asciiFold(item.name) === asciiFold(commandName),
      )
      if (!descriptor) return
      const pending = Object.freeze({
        requestId: this.nextRequestId++,
        catalogRevision: this.model.catalog.revision,
        commandName: descriptor.name,
        partialText: text,
      })
      this.model.pendingCompletion = pending
      this.emitRequest(
        Object.freeze({
          kind: "completion",
          requestId: pending.requestId,
          catalogRevision: pending.catalogRevision,
          commandName: pending.commandName,
          partialText: pending.partialText,
          maxItems: this.limits.maxCompletionItems,
          maxItemUtf8Bytes: this.limits.maxCompletionItemUtf8Bytes,
        }),
      )
      return
    }
    const prefix = asciiFold(text)
    this.model.completion = this.model.catalog.items
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.disposition === "visible" && asciiFold(item.name).startsWith(prefix))
      .sort((left, right) => {
        const leftName = asciiFold(left.item.name)
        const rightName = asciiFold(right.item.name)
        const nameOrder = leftName < rightName ? -1 : leftName > rightName ? 1 : 0
        return nameOrder || left.index - right.index
      })
      .slice(0, this.limits.maxCompletionItems)
      .map(({ item }) =>
        Object.freeze({
          label: item.kind === "convar" ? `${item.name} ${item.displayValue}` : item.name,
          insertion: item.name,
        }),
      )
    this.model.completionSource = this.model.completion.length > 0 ? "catalog" : "none"
  }

  private applyCompletion(completion: ConsoleCompletionResult): ConsoleOperationResult {
    const pending = this.model.pendingCompletion
    if (
      !completion ||
      !pending ||
      completion.requestId !== pending.requestId ||
      completion.catalogRevision !== pending.catalogRevision ||
      completion.catalogRevision !== this.model.catalog.revision
    ) {
      return this.fail("StaleCompletion", "apply-completion")
    }
    if (!Array.isArray(completion.suggestions) || completion.suggestions.length > this.limits.maxCompletionItems) {
      return this.fail("InvalidCompletion", "apply-completion")
    }
    const candidates: CompletionCandidate[] = []
    for (const suggestion of completion.suggestions) {
      if (
        !suggestion ||
        !["visible", "hidden", "development"].includes(suggestion.disposition) ||
        typeof suggestion.text !== "string" ||
        suggestion.text.length === 0 ||
        !hasValidUnicode(suggestion.text) ||
        utf8Bytes(suggestion.text) > this.limits.maxCompletionItemUtf8Bytes
      ) {
        return this.fail("InvalidCompletion", "apply-completion")
      }
      if (suggestion.disposition === "visible") {
        candidates.push(Object.freeze({ label: suggestion.text, insertion: suggestion.text }))
      }
    }
    this.model.pendingCompletion = null
    this.model.completion = candidates
    this.model.completionSource = candidates.length > 0 ? "owner" : "none"
    this.model.completionSelected = null
    this.commit()
    return result(this.model.revision)
  }

  private cycleCompletion(direction: -1 | 1, apply: boolean): void {
    const count = this.model.completion.length
    if (count === 0) return
    const current = this.model.completionSelected
    const next = current === null ? (direction === 1 ? 0 : count - 1) : (current + direction + count) % count
    this.model.completionSelected = next
    if (apply) this.applyCandidate(next, false)
    else this.commit()
  }

  private selectCompletion(index: number): void {
    if (!finiteInteger(index, 0) || index >= this.model.completion.length) return
    this.model.completionSelected = index
    this.applyCandidate(index, true, true)
  }

  private applyCandidate(index: number, close: boolean, forceTrailingSpace = false): void {
    const candidate = this.model.completion[index]
    if (!candidate) return
    this.model.entryText = candidate.insertion
    if ((forceTrailingSpace || !candidate.insertion.includes(" ")) && !this.model.entryText.endsWith(" ")) {
      this.model.entryText += " "
    }
    this.model.historyCursor = null
    this.model.historyDraft = ""
    if (close) this.closeCompletion()
    this.commit()
    this.view?.focusEntry()
  }

  private traverseHistory(direction: -1 | 1): void {
    if (this.model.history.length === 0) return
    if (direction === -1) {
      if (this.model.historyCursor === null) {
        this.model.historyDraft = this.model.entryText
        this.model.historyCursor = this.model.history.length - 1
      } else if (this.model.historyCursor > 0) {
        this.model.historyCursor -= 1
      }
      this.model.entryText = this.model.history[this.model.historyCursor]
    } else if (this.model.historyCursor !== null) {
      if (this.model.historyCursor < this.model.history.length - 1) {
        this.model.historyCursor += 1
        this.model.entryText = this.model.history[this.model.historyCursor]
      } else {
        this.model.historyCursor = null
        this.model.entryText = this.model.historyDraft
        this.model.historyDraft = ""
      }
    }
    this.closeCompletion()
    this.commit()
    this.view?.focusEntry()
  }

  private closeCompletion(): void {
    this.model.completion = []
    this.model.completionSource = "none"
    this.model.completionSelected = null
  }

  private cancelPending(reason: ConsoleCompletionCancellationReason): void {
    const pending = this.model.pendingCompletion
    if (!pending) return
    this.model.pendingCompletion = null
    this.emitRequest(Object.freeze({ kind: "completion-cancelled", requestId: pending.requestId, reason }))
  }

  private emitRequest(request: ConsoleRequest): void {
    this.requestQueue.push(request)
  }

  private flushRequests(): void {
    while (this.requestQueue.length > 0) {
      const request = this.requestQueue.shift()
      if (!request) continue
      try {
        this.onRequest(request)
      } catch {
        this.fail("RequestSinkFailure", "request")
      }
    }
  }
}

function cloneOutputSegment(segment: ConsoleOutputSegment): ConsoleOutputSegment {
  if (segment.kind === "color") {
    return Object.freeze({ kind: "color", text: segment.text, color: cloneColor(segment.color) })
  }
  return Object.freeze({ kind: segment.kind, text: segment.text })
}

export function initializeDeveloperConsole(
  configuration: DeveloperConsoleConfiguration,
): DeveloperConsoleInitialization {
  if (
    !configuration ||
    !/^[a-z][a-z0-9_-]{0,63}$/.test(configuration.runtimeIdentity) ||
    !validateLimits(configuration.limits) ||
    typeof configuration.onRequest !== "function"
  ) {
    return initialFailure("InvalidLimits")
  }
  if (!validateViewport(configuration.viewport)) return initialFailure("InvalidViewport")
  const resourceCode = validateResources(configuration.resources)
  if (resourceCode) {
    const candidate =
      configuration.resources && configuration.resources.kind !== "resolved"
        ? configuration.resources.logicalIdentity
        : undefined
    const subject = candidate && validLogicalIdentity(candidate) ? candidate : undefined
    return initialFailure(resourceCode, subject)
  }
  if (!validateCatalog(configuration.catalog, configuration.limits)) return initialFailure("MalformedCatalog")
  const resources = cloneResources(
    (configuration.resources as Extract<ConsoleResourceResolution, { kind: "resolved" }>).resources,
  )
  const catalog = cloneCatalog(configuration.catalog)
  return Object.freeze({
    ok: true,
    console: new DeveloperConsoleImplementation(configuration, resources, catalog),
  })
}
