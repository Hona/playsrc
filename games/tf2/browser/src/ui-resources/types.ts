export type Tf2UiResourceDomain =
  | "main-menu"
  | "loading"
  | "scheme"
  | "scheme-base"
  | "hud"
  | "class-selection"
  | "animation-manifest"
  | "animation-script"
  | "options"
  | "localization"

export type Tf2UiOwner =
  | "vgui"
  | "tf2"
  | "gameui"
  | "settings"
  | "application"
  | "service"
  | "external"
  | "unsupported"

export type Tf2UiCommandCategory = "gameplay" | "application" | "service" | "external" | "unsupported"

export type Tf2UiCondition = Readonly<{
  token: string
  symbol: string
  negated: boolean
  placement: "BeforeValue" | "AfterScalar"
}>

export type Tf2UiResourceNode = Readonly<{
  name: string
  value: string | null
  scalarKind: string | null
  condition: Tf2UiCondition | null
  children: readonly Tf2UiResourceNode[]
}>

export type Tf2UiProvider = Readonly<{
  order: number
  identity: string
  kind: "vpk" | "directory"
  revision: string
  pathIds: readonly string[]
  configuredLocation: string
}>

export type Tf2UiResourceSource = Readonly<{
  domain: Tf2UiResourceDomain
  logicalPath: string
  outcome: "found" | "missing"
  byteLength: number | null
  sha256: string | null
  providerIdentity: string | null
  providerKind: "Vpk" | "Directory" | null
  providerRevision: string | null
  encoding: "SourceBytes" | "Utf16LittleEndian" | "opaque-producer-input" | null
  roots: number
  nodes: number
  directives: readonly string[]
  document: readonly Tf2UiResourceNode[] | null
  checkedLocations: readonly string[]
}>

export type Tf2UiPanelDocument = Readonly<{
  identity: string
  domain: "main-menu" | "loading" | "hud" | "class-selection" | "options"
  source: Tf2UiResourceSource
  roots: readonly Tf2UiResourceNode[]
}>

export type Tf2UiSchemeEntry = Readonly<{
  identity: string
  name: string
  node: Tf2UiResourceNode
}>

export type Tf2UiSchemeDescriptor = Readonly<{
  identity: string
  source: Tf2UiResourceSource
  baseLogicalPaths: readonly string[]
  colors: readonly Tf2UiSchemeEntry[]
  fontDefinitions: readonly Tf2UiSchemeEntry[]
  borders: readonly Tf2UiSchemeEntry[]
}>

export type Tf2UiBorderDescriptor = Readonly<{
  identity: string
  sourceLogicalPath: string
  name: string
  node: Tf2UiResourceNode
  owner: "vgui"
}>

export type Tf2UiControlDescriptor = Readonly<{
  identity: string
  name: string
  owner: Tf2UiOwner
  sourceOccurrences: number
}>

export type Tf2UiPropertyDescriptor = Readonly<{
  identity: string
  sourceLogicalPath: string
  nodePath: string
  name: string
  kind: "object" | "scalar"
  owner: Tf2UiOwner
  condition: Tf2UiCondition | null
}>

export type Tf2UiCommandDescriptor = Readonly<{
  identity: string
  sourceLogicalPath: string
  nodePath: string
  command: string
  category: Tf2UiCommandCategory
  capabilityOwner: Tf2UiOwner
  executable: false
}>

export type Tf2UiLocalizationToken = Readonly<{
  identity: string
  name: string
  status: "resolved" | "missing"
  occurrences: number
  definitions: readonly Readonly<{
    sourceLogicalPath: string
    value: string
    condition: Tf2UiCondition | null
  }>[]
  owner: "vgui"
}>

export type Tf2UiDependency = Readonly<{
  logicalPath: string
  outcome: "found" | "missing"
  byteLength: number | null
  sha256: string | null
  providerIdentity: string | null
  providerKind: "Vpk" | "Directory" | null
  providerRevision: string | null
  checkedLocations: readonly string[]
}>

export type Tf2UiImageDescriptor = Readonly<{
  identity: string
  configuredValue: string
  classification: "content-vtf" | "procedural-material" | "missing-material" | "missing-texture" | "unsupported-pic"
  material: Tf2UiDependency | null
  textures: readonly Readonly<{
    source: Tf2UiDependency
    version: string
    width: number
    height: number
    depth: number
    frames: number
    faces: number
    mipCount: number
    highFormatCode: number
    lowFormatCode: number
    rawFlags: number
  }>[]
  owner: "vgui"
}>

export type Tf2UiFontDescriptor = Readonly<{
  identity: string
  configuredValue: string
  classification: "content-sfnt" | "content-bitmap" | "scheme-reference" | "missing-font"
  source: Tf2UiDependency | null
  owner: "vgui"
}>

export type Tf2UiAdvancedOptionChoice = Readonly<{ label: string; value: string }>
export type Tf2UiAdvancedOption = Readonly<{
  identity: string
  category: string
  prompt: string
  tooltip: string | null
  kind: "BOOL" | "NUMBER" | "STRING" | "LIST" | "SLIDER"
  minimum: number | null
  maximum: number | null
  choices: readonly Tf2UiAdvancedOptionChoice[]
  contentDefault: string
}>
export type Tf2UiKeyboardAction = Readonly<{
  section: number
  sectionName: string
  binding: string
  description: string
}>

export type Tf2UiResourceDescriptor = Readonly<{
  schema: "playsrc-tf2-ui-resources-v1"
  identity: string
  game: "tf2"
  contentBuild: string
  sourceLedger: string
  providers: readonly Tf2UiProvider[]
  sources: readonly Tf2UiResourceSource[]
  panels: readonly Tf2UiPanelDocument[]
  schemes: readonly Tf2UiSchemeDescriptor[]
  borders: readonly Tf2UiBorderDescriptor[]
  localization: Readonly<{
    language: "english"
    sources: readonly Tf2UiResourceSource[]
    tokens: readonly Tf2UiLocalizationToken[]
  }>
  images: readonly Tf2UiImageDescriptor[]
  fonts: readonly Tf2UiFontDescriptor[]
  advancedOptions: readonly Tf2UiAdvancedOption[]
  keyboardActions: readonly Tf2UiKeyboardAction[]
  animation: Readonly<{
    manifest: Tf2UiResourceSource
    scripts: readonly Tf2UiResourceSource[]
    compositionOrder: readonly string[]
  }>
  controls: readonly Tf2UiControlDescriptor[]
  properties: readonly Tf2UiPropertyDescriptor[]
  commands: readonly Tf2UiCommandDescriptor[]
  missingDependencies: readonly string[]
  conditions: Readonly<{
    platforms: readonly ["windows", "macos", "linux"]
    languages: readonly ["english", "%language%"]
    resolutions: readonly ["default", "minmode", "lodef", "hidef"]
    aspect: readonly ["if_taller", "if_wider"]
    session: readonly ["menu", "in-game", "replay"]
  }>
  bounds: typeof tf2UiResourceBounds
}>

export type Tf2UiResourceDiagnosticCode =
  | "InvalidInput"
  | "BoundExceeded"
  | "DuplicateIdentity"
  | "ChangedSource"
  | "MalformedResource"
  | "MissingRequiredResource"
  | "UnclassifiedControl"
  | "UnclassifiedCommand"
  | "UnclassifiedCapability"

export type Tf2UiResourceResolution =
  | Readonly<{ ok: true; descriptor: Tf2UiResourceDescriptor }>
  | Readonly<{
      ok: false
      diagnostic: Readonly<{
        code: Tf2UiResourceDiagnosticCode
        subject: string
      }>
    }>

export const tf2UiResourceBounds = Object.freeze({
  maximumProviders: 64,
  maximumSources: 128,
  maximumNodes: 32_768,
  maximumDepth: 101,
  maximumStringBytes: 4_095,
  maximumControls: 256,
  maximumProperties: 32_768,
  maximumCommands: 512,
  maximumLocalizationTokens: 4_096,
  maximumImages: 2_048,
  maximumFonts: 512,
  maximumRetainedSourceBytes: 64 * 1024 * 1024,
} as const)
