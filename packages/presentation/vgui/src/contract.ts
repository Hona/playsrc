export const SOURCE_CONSOLE_CEILINGS = Object.freeze({
  maxInputUtf8Bytes: 255,
  maxHistoryItems: 100,
  maxCompletionItems: 64,
  maxCompletionItemUtf8Bytes: 63,
  maxVisibleCompletionItems: 10,
  maxExecutionUtf8Bytes: 510,
  maxExecutionArguments: 64,
})

export type ConsoleLimits = Readonly<{
  maxInputUtf8Bytes: number
  maxHistoryItems: number
  maxCatalogItems: number
  maxCatalogItemUtf8Bytes: number
  maxCompletionItems: number
  maxCompletionItemUtf8Bytes: number
  maxVisibleCompletionItems: number
  maxOutputBatchSegments: number
  maxOutputBatchUtf8Bytes: number
  maxOutputSegments: number
  maxOutputUtf8Bytes: number
  maxDiagnostics: number
  maxDomNodes: number
  maxListeners: number
}>

export type Rgba = readonly [red: number, green: number, blue: number, alpha: number]

export type ConsoleFontResource = Readonly<{
  logicalIdentity: string
  family: string
  sizePxAt480: number
  lineHeightPxAt480: number
  weight: number
  style: "normal" | "italic"
}>

export type ConsoleBorderResource = Readonly<{
  logicalName: string
  color: Rgba
  widthPxAt480: number
  style: "solid" | "inset" | "outset"
}>

export type ConsoleResources = Readonly<{
  identity: string
  scheme: Readonly<{
    logicalIdentity: string
    tag: string
    revision: string
  }>
  localization: Readonly<{
    logicalIdentity: string
    language: string
    title: string
    submit: string
    entryAccessibleName: string
    historyAccessibleName: string
    completionAccessibleName: string
  }>
  colors: Readonly<{
    frameBackground: Rgba
    titleText: Rgba
    historyBackground: Rgba
    inputBackground: Rgba
    inputText: Rgba
    completionBackground: Rgba
    completionText: Rgba
    completionSelected: Rgba
    focus: Rgba
    normalOutput: Rgba
    developerOutput: Rgba
  }>
  fonts: Readonly<{
    title: ConsoleFontResource
    console: ConsoleFontResource
    completion: ConsoleFontResource
  }>
  border: ConsoleBorderResource
  frameTitleHeightPxAt480: number
}>

export type ConsoleResourceResolution =
  | Readonly<{ kind: "resolved"; resources: ConsoleResources }>
  | Readonly<{ kind: "missing"; logicalIdentity: string }>
  | Readonly<{
      kind: "malformed"
      logicalIdentity: string
      reason:
        | "InvalidIdentity"
        | "MissingColor"
        | "InvalidColor"
        | "MissingFont"
        | "InvalidFont"
        | "MissingBorder"
        | "InvalidBorder"
        | "MissingLocalization"
        | "InvalidLocalization"
        | "InvalidMetric"
    }>

export type ConsoleCatalogDisposition = "visible" | "hidden" | "development"

export type ConsoleCatalogItem =
  | Readonly<{
      kind: "command"
      name: string
      disposition: ConsoleCatalogDisposition
      acceptsSuggestions: boolean
    }>
  | Readonly<{
      kind: "convar"
      name: string
      disposition: ConsoleCatalogDisposition
      displayValue: string
    }>

export type ConsoleCatalog = Readonly<{
  revision: string
  items: readonly ConsoleCatalogItem[]
}>

export type ConsoleOutputSegment =
  | Readonly<{ kind: "normal"; text: string }>
  | Readonly<{ kind: "developer"; text: string }>
  | Readonly<{ kind: "color"; text: string; color: Rgba }>

export type ConsoleCompletionSuggestion = Readonly<{
  text: string
  disposition: ConsoleCatalogDisposition
}>

export type ConsoleCompletionResult = Readonly<{
  requestId: number
  catalogRevision: string
  suggestions: readonly ConsoleCompletionSuggestion[]
}>

export type ConsoleViewport = Readonly<{
  width: number
  height: number
  devicePixelRatio: number
}>

export type ConsoleCompletionCancellationReason =
  | "input-changed"
  | "catalog-replaced"
  | "hidden"
  | "root-replaced"
  | "cancelled"
  | "destroyed"
  | "submitted"

export type ConsoleRequest =
  | Readonly<{
      kind: "submission"
      requestId: number
      text: string
      catalogRevision: string
      maxExecutionUtf8Bytes: 510
      maxExecutionArguments: 64
    }>
  | Readonly<{
      kind: "completion"
      requestId: number
      catalogRevision: string
      commandName: string
      partialText: string
      maxItems: number
      maxItemUtf8Bytes: number
    }>
  | Readonly<{
      kind: "completion-cancelled"
      requestId: number
      reason: ConsoleCompletionCancellationReason
    }>
  | Readonly<{
      kind: "visibility"
      requestId: number
      operation: "hide"
      reason: "entry-backquote"
    }>

export type ConsoleDiagnosticCode =
  | "AlreadyMounted"
  | "NotMounted"
  | "NotVisible"
  | "Destroyed"
  | "InvalidLimits"
  | "InvalidViewport"
  | "MissingResource"
  | "MalformedResource"
  | "MalformedCatalog"
  | "MalformedText"
  | "InputLimit"
  | "OutputLimit"
  | "InvalidCompletion"
  | "StaleCompletion"
  | "DomLimit"
  | "ListenerLimit"
  | "DomFailure"
  | "RequestSinkFailure"

export type ConsoleDiagnostic = Readonly<{
  sequence: number
  code: ConsoleDiagnosticCode
  operation: ConsoleOperation["kind"] | "initialize" | "input" | "request"
  subject?: string
}>

export type ConsoleOperation =
  | Readonly<{ kind: "mount"; root: HTMLElement }>
  | Readonly<{ kind: "replace-root"; root: HTMLElement }>
  | Readonly<{ kind: "activate" }>
  | Readonly<{ kind: "foreground" }>
  | Readonly<{ kind: "focus-entry" }>
  | Readonly<{ kind: "hide" }>
  | Readonly<{ kind: "clear-output" }>
  | Readonly<{ kind: "append-output"; segments: readonly ConsoleOutputSegment[] }>
  | Readonly<{ kind: "replace-catalog"; catalog: ConsoleCatalog }>
  | Readonly<{ kind: "apply-completion"; result: ConsoleCompletionResult }>
  | Readonly<{ kind: "replace-resources"; resolution: ConsoleResourceResolution }>
  | Readonly<{ kind: "set-viewport"; viewport: ConsoleViewport }>
  | Readonly<{ kind: "set-reduced-motion"; reduced: boolean }>
  | Readonly<{ kind: "cancel" }>
  | Readonly<{ kind: "destroy" }>

export type ConsoleOperationResult =
  | Readonly<{ ok: true; revision: number }>
  | Readonly<{ ok: false; diagnostic: ConsoleDiagnostic }>

export type ConsoleSnapshot = Readonly<{
  runtimeIdentity: string
  lifecycle: "initialized" | "mounted" | "destroyed"
  revision: number
  visible: boolean
  focused: boolean
  foregroundRevision: number
  viewport: ConsoleViewport
  reducedMotion: boolean
  resourceIdentity: string
  catalogRevision: string
  entryText: string
  output: readonly ConsoleOutputSegment[]
  outputUtf8Bytes: number
  history: readonly string[]
  historyCursor: number | null
  completion: Readonly<{
    source: "none" | "catalog" | "owner"
    labels: readonly string[]
    selectedIndex: number | null
    visible: boolean
    pendingRequestId: number | null
  }>
  diagnostics: readonly ConsoleDiagnostic[]
  ownedResources: Readonly<{
    nodes: number
    listeners: number
    observers: 0
    timers: 0
  }>
}>

export type DeveloperConsole = Readonly<{
  apply(operation: ConsoleOperation): ConsoleOperationResult
  snapshot(): ConsoleSnapshot
}>

export type DeveloperConsoleConfiguration = Readonly<{
  runtimeIdentity: string
  limits: ConsoleLimits
  resources: ConsoleResourceResolution
  catalog: ConsoleCatalog
  viewport: ConsoleViewport
  reducedMotion: boolean
  onRequest(request: ConsoleRequest): void
}>

export type DeveloperConsoleInitialization =
  | Readonly<{ ok: true; console: DeveloperConsole }>
  | Readonly<{ ok: false; diagnostic: ConsoleDiagnostic }>
