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
  proportional: boolean
  outlinePxAt480: number
}>

export type ConsoleBorderResource = Readonly<{
  logicalName: string
  colors: Readonly<{
    left: Rgba
    top: Rgba
    right: Rgba
    bottom: Rgba
  }>
  widthsPxAt480: readonly [left: number, top: number, right: number, bottom: number]
  insetPxAt480: readonly [left: number, top: number, right: number, bottom: number]
  proportional: boolean
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
    closeAccessibleName: string
    entryAccessibleName: string
    historyAccessibleName: string
    completionAccessibleName: string
  }>
  colors: Readonly<{
    frameBackground: Rgba
    frameBackgroundUnfocused: Rgba
    titleBackground: Rgba
    titleBackgroundUnfocused: Rgba
    titleText: Rgba
    titleTextUnfocused: Rgba
    historyBackground: Rgba
    inputBackground: Rgba
    inputText: Rgba
    inputSelectionBackground: Rgba
    inputSelectionText: Rgba
    inputCursor: Rgba
    completionBackground: Rgba
    completionText: Rgba
    completionArmedBackground: Rgba
    completionArmedText: Rgba
    submitBackground: Rgba
    submitText: Rgba
    submitArmedBackground: Rgba
    submitArmedText: Rgba
    submitDepressedBackground: Rgba
    submitDepressedText: Rgba
    closeButton: Rgba
    closeButtonUnfocused: Rgba
    focus: Rgba
    normalOutput: Rgba
    developerOutput: Rgba
  }>
  fonts: Readonly<{
    title: ConsoleFontResource
    console: ConsoleFontResource
    entry: ConsoleFontResource
    completion: ConsoleFontResource
    submit: ConsoleFontResource
  }>
  borders: Readonly<{
    frame: ConsoleBorderResource
    history: ConsoleBorderResource
    entry: ConsoleBorderResource
    submit: ConsoleBorderResource
    submitDepressed: ConsoleBorderResource
    completion: ConsoleBorderResource
  }>
  layout: Readonly<{
    frameMinimumWidthPx: number
    frameMinimumHeightPx: number
    frameFocusTransitionSeconds: number
    clientMinimumWidthPx: number
    clientMinimumHeightPx: number
    clientInsetXPx: number
    clientInsetYPxAt480: number
    titleTextInsetXPxAt480: number
    titleTextInsetYPxAt480: number
    titleBackgroundInsetPxAt480: number
    titleBackgroundBottomPxAt480: number
    captionHeightPxAt480: number
    captionTitleBorderPxAt480: number
    clientTitleGapPxAt480: number
    closeButtonInsetRightPxAt480: number
    closeButtonInsetTopPxAt480: number
    closeButtonOffsetPxAt480: number
    closeButtonSizePxAt480: number
    closeGlyphSizePx: number
    resizeGripPxAt480: number
    resizeCornerPxAt480: number
    resizeBottomRightPxAt480: number
    consoleInsetPxAt480: number
    historyTopOffsetPxAt480: number
    historyDrawOffsetXPx: number
    historyDrawOffsetYPx: number
    entryHeightPxAt480: number
    entryInsetPxAt480: number
    entryDrawOffsetXPxAt480: number
    entryDrawOffsetYPxAt480: number
    submitWidthPxAt480: number
    submitInsetPxAt480: number
    completionTextInsetPxAt480: number
    completionRowPaddingPxAt480: number
  }>
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
      reason: "entry-backquote" | "frame-close"
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
  frame: Readonly<{
    x: number
    y: number
    width: number
    height: number
    interaction: "move" | "resize-n" | "resize-ne" | "resize-e" | "resize-se" | "resize-s" | "resize-sw" | "resize-w" | "resize-nw" | null
    capturedPointerId: number | null
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

export type ClientDiagnosticMode = 0 | 1 | 2

export type ClientDiagnosticVector = readonly [x: number, y: number, z: number]

export type ClientDiagnosticResources = Readonly<{
  identity: string
  scheme: Readonly<{
    logicalIdentity: string
    tag: string
    revision: string
  }>
  font: ConsoleFontResource
  colors: Readonly<{
    goodFps: Rgba
    warningFps: Rgba
    badFps: Rgba
    position: Rgba
  }>
  panelWidthPx: number
  panelPaddingPx: number
  panelHeightPaddingPx: number
  lineGapPx: number
  maximumLines: 4
}>

export type ClientDiagnosticFrame = Readonly<{
  realTimeMilliseconds: number
  fpsMode: ClientDiagnosticMode
  positionMode: ClientDiagnosticMode
  mapIdentity: string
  view: Readonly<{
    position: ClientDiagnosticVector
    angles: ClientDiagnosticVector
  }>
  player: Readonly<{
    position: ClientDiagnosticVector
    angles: ClientDiagnosticVector | null
    velocity: ClientDiagnosticVector
  }>
}>

export type ClientDiagnosticOperation =
  | Readonly<{ kind: "mount"; root: HTMLElement }>
  | Readonly<{ kind: "replace-root"; root: HTMLElement }>
  | Readonly<{ kind: "set-viewport"; viewport: ConsoleViewport }>
  | Readonly<{ kind: "present"; frame: ClientDiagnosticFrame }>
  | Readonly<{ kind: "destroy" }>

export type ClientDiagnosticOperationResult =
  | Readonly<{ ok: true; revision: number }>
  | Readonly<{ ok: false; code: "AlreadyMounted" | "NotMounted" | "Destroyed" | "InvalidViewport" | "InvalidFrame" | "DomFailure" }>

export type ClientDiagnosticSnapshot = Readonly<{
  runtimeIdentity: string
  lifecycle: "initialized" | "mounted" | "destroyed"
  revision: number
  visible: boolean
  viewport: ConsoleViewport
  fps: Readonly<{
    average: number | null
    low: number | null
    high: number | null
  }>
  lines: readonly Readonly<{ kind: "fps" | "position" | "unsupported"; text: string; color: Rgba }>[]
  ownedResources: Readonly<{
    nodes: number
    listeners: number
    observers: 0
    timers: 0
  }>
}>

export type ClientDiagnostics = Readonly<{
  apply(operation: ClientDiagnosticOperation): ClientDiagnosticOperationResult
  snapshot(): ClientDiagnosticSnapshot
}>

export type ClientDiagnosticsConfiguration = Readonly<{
  runtimeIdentity: string
  resources: ClientDiagnosticResources
  viewport: ConsoleViewport
}>

export type ClientDiagnosticsInitialization =
  | Readonly<{ ok: true; diagnostics: ClientDiagnostics }>
  | Readonly<{ ok: false; code: "InvalidConfiguration" | "InvalidViewport" }>
