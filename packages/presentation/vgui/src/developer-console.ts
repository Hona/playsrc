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

type ConsoleModel = {
  runtimeIdentity: string
  lifecycle: "initialized" | "mounted" | "destroyed"
  revision: number
  visible: boolean
  foregroundRevision: number
  viewport: ConsoleViewport
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
}>

const encoder = new TextEncoder()
const STATIC_OWNED_NODES = 10
const STATIC_LISTENERS = 6

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
      sizePxAt480: font.sizePxAt480,
      lineHeightPxAt480: font.lineHeightPxAt480,
      weight: font.weight,
      style: font.style,
    })
  return Object.freeze({
    identity: resources.identity,
    scheme: Object.freeze({ ...resources.scheme }),
    localization: Object.freeze({ ...resources.localization }),
    colors: Object.freeze({
      frameBackground: cloneColor(resources.colors.frameBackground),
      titleText: cloneColor(resources.colors.titleText),
      historyBackground: cloneColor(resources.colors.historyBackground),
      inputBackground: cloneColor(resources.colors.inputBackground),
      inputText: cloneColor(resources.colors.inputText),
      completionBackground: cloneColor(resources.colors.completionBackground),
      completionText: cloneColor(resources.colors.completionText),
      completionSelected: cloneColor(resources.colors.completionSelected),
      focus: cloneColor(resources.colors.focus),
      normalOutput: cloneColor(resources.colors.normalOutput),
      developerOutput: cloneColor(resources.colors.developerOutput),
    }),
    fonts: Object.freeze({
      title: cloneFont(resources.fonts.title),
      console: cloneFont(resources.fonts.console),
      completion: cloneFont(resources.fonts.completion),
    }),
    border: Object.freeze({
      logicalName: resources.border.logicalName,
      color: cloneColor(resources.border.color),
      widthPxAt480: resources.border.widthPxAt480,
      style: resources.border.style,
    }),
    frameTitleHeightPxAt480: resources.frameTitleHeightPxAt480,
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
        !Number.isFinite(font.sizePxAt480) ||
        font.sizePxAt480 <= 0 ||
        font.sizePxAt480 > 512 ||
        !Number.isFinite(font.lineHeightPxAt480) ||
        font.lineHeightPxAt480 < font.sizePxAt480 ||
        font.lineHeightPxAt480 > 1024 ||
        !finiteInteger(font.weight, 1, 1000) ||
        !["normal", "italic"].includes(font.style)
      ) {
        return "MalformedResource"
      }
    }
    if (
      resources.border.logicalName.length === 0 ||
      !hasValidUnicode(resources.border.logicalName) ||
      !validColor(resources.border.color) ||
      !Number.isFinite(resources.border.widthPxAt480) ||
      resources.border.widthPxAt480 < 0 ||
      resources.border.widthPxAt480 > 32 ||
      !["solid", "inset", "outset"].includes(resources.border.style) ||
      !Number.isFinite(resources.frameTitleHeightPxAt480) ||
      resources.frameTitleHeightPxAt480 <= 0 ||
      resources.frameTitleHeightPxAt480 > 256
    ) {
      return "MalformedResource"
    }
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

function initialFailure(code: ConsoleDiagnosticCode, subject?: string): DeveloperConsoleInitialization {
  return Object.freeze({
    ok: false,
    diagnostic: Object.freeze({ sequence: 1, code, operation: "initialize", ...(subject ? { subject } : {}) }),
  })
}

class ConsoleView {
  readonly runtime: VguiDomRuntime
  readonly frame: VguiControl
  readonly title: VguiControl
  readonly client: VguiControl
  readonly history: VguiControl
  readonly entry: VguiControl
  readonly submit: VguiControl
  readonly completion: VguiControl
  private outputNodes: HTMLElement[] = []
  private completionItems: VguiControl[] = []

  constructor(root: HTMLElement, runtimeIdentity: string, limits: ConsoleLimits, callbacks: ViewCallbacks) {
    this.runtime = new VguiDomRuntime(root, limits)
    this.runtime.host.dataset.runtimeIdentity = runtimeIdentity
    try {
      this.frame = this.runtime.createControl("Frame", "GameConsole", "section", null)
      this.frame.element.classList.add("playsrc-vgui-frame")
      this.frame.element.setAttribute("role", "dialog")
      this.frame.element.setAttribute("aria-modal", "false")

      this.title = this.runtime.createControl("Label", "ConsoleTitle", "div", this.frame)
      this.title.element.classList.add("playsrc-vgui-title")
      this.title.element.setAttribute("aria-hidden", "true")

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
    this.title.element.textContent = model.resources.localization.title
    this.submit.element.textContent = model.resources.localization.submit
    this.frame.setVisible(model.visible)
    this.runtime.host.dataset.reducedMotion = String(model.reducedMotion)
    this.runtime.host.dataset.devicePixelRatio = String(model.viewport.devicePixelRatio)
  }

  syncResources(resources: ConsoleResources, viewport: ConsoleViewport): void {
    const style = this.runtime.host.style
    const colorVariables: ReadonlyArray<readonly [string, Rgba]> = [
      ["--vgui-frame-background", resources.colors.frameBackground],
      ["--vgui-title-text", resources.colors.titleText],
      ["--vgui-history-background", resources.colors.historyBackground],
      ["--vgui-input-background", resources.colors.inputBackground],
      ["--vgui-input-text", resources.colors.inputText],
      ["--vgui-completion-background", resources.colors.completionBackground],
      ["--vgui-completion-text", resources.colors.completionText],
      ["--vgui-completion-selected", resources.colors.completionSelected],
      ["--vgui-focus", resources.colors.focus],
      ["--vgui-normal-output", resources.colors.normalOutput],
      ["--vgui-developer-output", resources.colors.developerOutput],
      ["--vgui-border-color", resources.border.color],
    ]
    for (const [name, color] of colorVariables) style.setProperty(name, rgba(color))
    const scale = viewport.height / 480
    for (const [role, font] of Object.entries(resources.fonts)) {
      style.setProperty(`--vgui-${role}-font`, font.family)
      style.setProperty(`--vgui-${role}-size`, `${Math.max(1, Math.trunc(font.sizePxAt480 * scale))}px`)
      style.setProperty(`--vgui-${role}-line-height`, `${Math.max(1, Math.trunc(font.lineHeightPxAt480 * scale))}px`)
      style.setProperty(`--vgui-${role}-weight`, String(font.weight))
      style.setProperty(`--vgui-${role}-style`, font.style)
    }
    style.setProperty("--vgui-border-width", `${Math.max(0, Math.trunc(resources.border.widthPxAt480 * scale))}px`)
    style.setProperty("--vgui-border-style", resources.border.style)
    this.runtime.host.dataset.resourceIdentity = resources.identity
    this.runtime.host.dataset.schemeIdentity = resources.scheme.logicalIdentity
    this.runtime.host.dataset.schemeTag = resources.scheme.tag
    this.runtime.host.dataset.schemeRevision = resources.scheme.revision
    this.runtime.host.dataset.localizationIdentity = resources.localization.logicalIdentity
    this.runtime.host.dataset.borderIdentity = resources.border.logicalName
  }

  syncViewport(model: ConsoleModel, limits: ConsoleLimits): void {
    const { width: viewportWidth, height: viewportHeight } = model.viewport
    const scale = viewportHeight / 480
    const proportional = (value: number) => Math.trunc(value * scale)
    const offsetX = proportional(16)
    const offsetY = proportional(64)
    const minimumWidth = Math.min(viewportWidth, Math.max(1, proportional(100)))
    const minimumHeight = Math.min(viewportHeight, Math.max(1, proportional(100)))
    const frameWidth = Math.min(viewportWidth, Math.max(minimumWidth, Math.trunc(viewportWidth / 2)))
    const frameHeight = Math.min(viewportHeight, Math.max(minimumHeight, viewportHeight - offsetY * 2))
    const frameX = Math.max(0, Math.min(viewportWidth - frameWidth, Math.trunc(viewportWidth / 2) - offsetX))
    const frameY = Math.max(0, Math.min(viewportHeight - frameHeight, offsetY))
    this.frame.setBounds(frameX, frameY, frameWidth, frameHeight)

    const titleHeight = Math.min(frameHeight, Math.max(1, proportional(model.resources.frameTitleHeightPxAt480)))
    this.title.setBounds(0, 0, frameWidth, titleHeight)
    const clientHeight = Math.max(0, frameHeight - titleHeight)
    this.client.setBounds(0, titleHeight, frameWidth, clientHeight)

    const inset = Math.max(0, proportional(8))
    const topHeight = Math.max(0, proportional(4))
    const entryHeight = Math.max(1, proportional(24))
    const entryInset = Math.max(0, proportional(4))
    const submitWidth = Math.max(1, proportional(64))
    const submitInset = Math.max(0, proportional(7))
    const historyWidth = Math.max(0, frameWidth - inset * 2)
    const historyHeight = Math.max(0, clientHeight - (entryInset * 2 + inset * 2 + topHeight + entryHeight))
    this.history.setBounds(inset, inset + topHeight, historyWidth, historyHeight)
    const submitX = Math.max(inset, frameWidth - (inset + submitWidth + submitInset))
    const entryY = Math.max(0, clientHeight - (entryInset * 2 + entryHeight))
    this.submit.setBounds(submitX, entryY, Math.min(submitWidth, Math.max(0, frameWidth - submitX)), entryHeight)
    const entryWidth = Math.max(0, submitX - entryInset - inset * 2)
    this.entry.setBounds(inset, entryY, entryWidth, entryHeight)

    const rowHeight = Math.max(1, Math.trunc(model.resources.fonts.completion.lineHeightPxAt480 * scale))
    const visibleRows = Math.min(limits.maxVisibleCompletionItems, model.completion.length)
    const desiredPopupHeight = rowHeight * Math.max(1, visibleRows)
    const globalBelow = frameY + titleHeight + entryY + entryHeight
    const below = Math.max(0, viewportHeight - globalBelow)
    const popupHeight = Math.min(desiredPopupHeight, Math.max(rowHeight, below || desiredPopupHeight))
    const popupTop = below >= rowHeight ? entryY + entryHeight : Math.max(0, entryY - popupHeight)
    this.completion.setBounds(inset, popupTop, entryWidth, popupHeight)
    for (const item of this.completionItems) item.element.style.height = `${rowHeight}px`
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
    return this.runtime.document.activeElement === this.entry.element
  }

  moveToFront(): void {
    this.frame.moveToFront()
  }

  counts(): Readonly<{ nodes: number; listeners: number }> {
    return this.runtime.counts()
  }

  destroy(): void {
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
          this.commit()
          return result(this.model.revision)
        }
        case "set-viewport":
          if (!validateViewport(operation.viewport)) return this.fail("InvalidViewport", operation.kind)
          this.model.viewport = Object.freeze({ ...operation.viewport })
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
          this.view?.destroy()
          this.view = null
          this.model.visible = false
          this.model.lifecycle = "destroyed"
          this.model.completion = []
          this.model.completionSource = "none"
          this.model.completionSelected = null
          this.model.pendingCompletion = null
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
      this.cancelPending("hidden")
      this.closeCompletion()
      this.emitRequest(
        Object.freeze({
          kind: "visibility",
          requestId: this.nextRequestId++,
          operation: "hide",
          reason: "entry-backquote",
        }),
      )
      this.commit()
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
