import {
  mountVguiFontSet,
  isVguiGenericResourcePropertySupported,
  parseVguiAnimationScript,
  resolveVguiSchemeFonts,
  type VguiAnimationScriptSet,
  type VguiBorder,
  type VguiControlRegistration,
  type VguiDesktopPlatform,
  type VguiFontByteSupply,
  type VguiFontFileIdentity,
  type VguiFontPresentation,
  type VguiFontSetMount,
  type VguiGenericControlName,
  type VguiImagePresentation,
  type VguiImageMaterialPresentation,
  type VguiImageMaterialTexture,
  type VguiLocalization,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiScheme,
  type VguiSchemeDocument,
  type VguiSchemeNode,
} from "@playsrc/vgui"
import {
  tf2UiResources,
  type Tf2UiPanelDocument,
  type Tf2UiResourceDescriptor,
  type Tf2UiResourceNode,
  type Tf2UiSchemeDescriptor,
} from "../ui-resources"

export type Tf2UiIntegrationDiagnostic = Readonly<{
  code:
    | "MissingBundleObject"
    | "ChangedBundleObject"
    | "UnsupportedControl"
    | "UnsupportedProperty"
    | "UnsupportedImageMaterial"
    | "UnsupportedImageVariant"
    | "UnsupportedBorder"
    | "FontUnavailable"
    | "AnimationMalformed"
  subject: string
}>

export type Tf2VguiResources = Readonly<{
  identity: string
  descriptor: Tf2UiResourceDescriptor
  clientScheme: VguiScheme
  sourceScheme: VguiScheme
  localization: VguiLocalization
  animations: VguiAnimationScriptSet
  activeConditions: readonly string[]
  customControls: readonly VguiControlRegistration[]
  gameUiBackground: Tf2GameUiBackgroundDescriptor
  diagnostics: readonly Tf2UiIntegrationDiagnostic[]
  document(logicalPath: string): VguiResourceDocument
  panelDocument(logicalPath: string): Tf2UiPanelDocument
  destroy(): void
}>

export type Tf2GameUiBackgroundDescriptor = Readonly<{
  identity: string
  contentBuild: string
  source: Readonly<{ logicalPath: string; byteLength: number; sha256: string }>
  defaultChapter: 1
  backgroundName: string
  variants: readonly Readonly<{
    aspect: "standard" | "widescreen"
    image: string
    material: string
    materialSha256: string
    texture: string
    textureSha256: string
    width: number
    height: number
  }>[]
}>

export type Tf2VguiResourceRequest = Readonly<{
  dependencies: ReadonlyMap<string, Uint8Array>
  viewportHeight: number
  platform: VguiDesktopPlatform
  createObjectUrl?: (bytes: Uint8Array, mediaType: string) => string
  revokeObjectUrl?: (url: string) => void
}>

type RawUiMaterial = Readonly<{
  configuredValue: string
  material: string
  materialSha256: string
  shader: string
  baseTexture: string
  baseColorRead: string
  secondTexture: string | null
  secondColorRead: string | null
  detailTexture: string | null
  detailColorRead: string | null
  detailScale: readonly [number, number]
  detailBlendMode: number
  detailBlendFactor: number
  detailTint: [number, number, number]
  distanceAlpha: boolean
  distanceAlphaFromDetail: boolean
  softEdges: boolean
  scaleSoftEdges: boolean
  edgeSoftnessStart: number
  edgeSoftnessEnd: number
  outline: boolean
  outlineColor: [number, number, number]
  outlineAlpha: number
  outlineStart0: number
  outlineStart1: number
  outlineEnd0: number
  outlineEnd1: number
  scaleOutline: boolean
  glow: boolean
  glowColor: [number, number, number]
  glowAlpha: number
  glowStart: number
  glowEnd: number
  glowX: number
  glowY: number
}>
type RawUiTexture = Readonly<{
  logicalPath: string
  sha256: string
  width: number
  height: number
  frames: number
  rawFlags: number
}>
type RawGameUiBackground = Readonly<{
  schema: string
  contentBuild: string
  chapterSource: Readonly<{ logicalPath: string; byteLength: number; sha256: string }>
  defaultChapter: number
  backgroundName: string
  variants: readonly Readonly<{
    aspect: string
    configuredValue: string
    material: string
    materialSha256: string
    texture: string
    textureSha256: string
    width: number
    height: number
  }>[]
}>

const SHA256 = /^[0-9a-f]{64}$/u
const GENERIC_CONTROLS = new Set([
  "Panel", "EditablePanel", "Label", "ImagePanel", "Button", "TextEntry", "RichText", "Frame",
  "ScrollBar", "Slider", "ComboBox", "Menu", "MenuItem", "PropertySheet", "PropertyPage",
  "CheckButton", "RadioButton", "ProgressBar", "ListPanel", "MessageBox", "QueryBox", "URLLabel",
  "ContinuousProgressBar", "Divider", "FrameSystemButton", "HTML", "ScalableImagePanel",
  "ScrollableEditablePanel", "SectionedListPanel",
])
const UNSUPPORTED_GENERIC_CONTROLS = new Set<string>()
const CUSTOM_BASES: Readonly<Record<string, VguiControlRegistration["baseControl"]>> = Object.freeze({
  BuildModeDialog: "EditablePanel",
  CAutoFittingLabel: "Label",
  CAvatarImagePanel: "ImagePanel",
  CCompetitiveAccessInfoPanel: "EditablePanel",
  CCurrencyStatusPanel: "EditablePanel",
  CCvarNegateCheckButton: "CheckButton",
  CCvarSlider: "Slider",
  CCvarToggleCheckButton: "CheckButton",
  CCyclingAdContainerPanel: "EditablePanel",
  CDashboardPartyMember: "EditablePanel",
  CEmbeddedItemModelPanel: "Panel",
  CEventPlayListEntry: "EditablePanel",
  CExButton: "Button",
  CExImageButton: "Button",
  CExLabel: "Label",
  CExplanationPopup: "EditablePanel",
  CGammaDialog: "Frame",
  CIconPanel: "Panel",
  CItemModelPanel: "Panel",
  CLabeledCommandComboBox: "ComboBox",
  CLoadingDialog: "Frame",
  CMainMenuNotificationsControl: "EditablePanel",
  CModelPanel: "Panel",
  COptionsSubMultiplayer: "Frame",
  COptionsSubVideoAdvancedDlg: "Frame",
  CPanelListPanel: "ScrollableEditablePanel",
  CPlayListEntry: "EditablePanel",
  CPvPRankPanel: "EditablePanel",
  CSteamFriendsListPanel: "EditablePanel",
  CTeamMenu: "Frame",
  CTFClassImage: "ImagePanel",
  CTFClassTipsItemPanel: "EditablePanel",
  CTFClassTipsPanel: "EditablePanel",
  CTFFooter: "EditablePanel",
  CTFImagePanel: "ImagePanel",
  CTFLogoPanel: "Panel",
  CTFPlayerModelPanel: "Panel",
  PanelListPanel: "ScrollableEditablePanel",
  CTFTeamButton: "Button",
  URLButton: "Button",
})

const lower = (value: string): string => value.toLowerCase()
const scalar = (node: Tf2UiResourceNode, name: string): string | null =>
  node.children.find((child) => lower(child.name) === lower(name) && child.value !== null)?.value ?? null
const object = (node: Tf2UiResourceNode, name: string): Tf2UiResourceNode | null =>
  node.children.find((child) => lower(child.name) === lower(name) && child.value === null) ?? null
const integerList = (value: string | null, count: number): number[] | null => {
  if (value === null) return null
  const values = value.trim().split(/\s+/u).map(Number)
  return values.length === count && values.every(Number.isSafeInteger) ? values : null
}
const literalColor = (value: string): readonly [number, number, number, number] | null => {
  const channels = value.trim().split(/\s+/u).map(Number)
  if ((channels.length !== 3 && channels.length !== 4) || channels.some((item) => !Number.isSafeInteger(item) || item < 0 || item > 255)) return null
  return Object.freeze([channels[0]!, channels[1]!, channels[2]!, channels[3] ?? 255])
}

function resourceNode(node: Tf2UiResourceNode, children: readonly VguiResourceNode[]): VguiResourceNode {
  return Object.freeze({
    name: node.name,
    value: node.value,
    condition: node.condition?.token ?? null,
    children: Object.freeze(children),
  })
}

function schemeNode(node: Tf2UiResourceNode): VguiSchemeNode {
  return Object.freeze({
    name: node.name,
    value: node.value,
    condition: node.condition?.token ?? null,
    children: Object.freeze(node.children.map(schemeNode)),
  })
}

function sourceBytes(
  dependencies: ReadonlyMap<string, Uint8Array>,
  logicalPath: string,
  byteLength: number | null,
  sha256: string | null,
  diagnostics: Tf2UiIntegrationDiagnostic[],
): Uint8Array | null {
  const bytes = dependencies.get(logicalPath)
  if (!bytes) {
    diagnostics.push(Object.freeze({ code: "MissingBundleObject", subject: logicalPath }))
    return null
  }
  if (bytes.byteLength !== byteLength || !sha256 || !SHA256.test(sha256)) {
    diagnostics.push(Object.freeze({ code: "ChangedBundleObject", subject: logicalPath }))
    return null
  }
  return bytes
}

function sfnt(bytes: Uint8Array): Readonly<{ version: string; families: readonly string[] }> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const u16 = (offset: number) => view.getUint16(offset, false)
  const u32 = (offset: number) => view.getUint32(offset, false)
  const tables = u16(4)
  let base = 0
  for (let index = 0; index < tables; index += 1) {
    const offset = 12 + index * 16
    if (new TextDecoder().decode(bytes.subarray(offset, offset + 4)) === "name") base = u32(offset + 8)
  }
  if (!base) throw new Error("SFNT name table missing")
  const count = u16(base + 2)
  const strings = u16(base + 4)
  const families = new Set<string>()
  for (let index = 0; index < count; index += 1) {
    const offset = base + 6 + index * 12
    const platform = u16(offset)
    if (u16(offset + 6) !== 1) continue
    const length = u16(offset + 8)
    const start = base + strings + u16(offset + 10)
    const raw = bytes.subarray(start, start + length)
    let value = ""
    if (platform === 0 || platform === 3) {
      for (let position = 0; position < raw.length; position += 2) value += String.fromCharCode((raw[position]! << 8) | raw[position + 1]!)
    } else value = String.fromCharCode(...raw)
    if (value) families.add(value)
  }
  return Object.freeze({ version: `sfnt-16.16:${u32(0)}`, families: Object.freeze([...families]) })
}

function schemeDocument(value: Tf2UiSchemeDescriptor): VguiSchemeDocument {
  const root = value.source.document?.find((node) => lower(node.name) === "scheme")
  if (!root || !value.source.sha256) throw new Error(`Malformed configured scheme ${value.source.logicalPath}`)
  return Object.freeze({
    logicalIdentity: value.source.logicalPath,
    sha256: value.source.sha256,
    baseLogicalIdentities: value.baseLogicalPaths,
    root: schemeNode(root),
  })
}

function composedSections(
  descriptor: Tf2UiResourceDescriptor,
  identity: string,
  section: string,
  stack = new Set<string>(),
): Tf2UiResourceNode[] {
  if (stack.has(identity)) throw new Error(`Cyclic configured scheme ${identity}`)
  const value = descriptor.schemes.find((scheme) => scheme.source.logicalPath === identity)
  const root = value?.source.document?.find((node) => lower(node.name) === "scheme")
  if (!value || !root) throw new Error(`Missing configured scheme ${identity}`)
  const output: Tf2UiResourceNode[] = []
  for (const base of value.baseLogicalPaths) output.push(...composedSections(descriptor, base, section, new Set([...stack, identity])))
  for (const selected of root.children.filter((node) => lower(node.name) === lower(section))) {
    for (const entry of selected.children) {
      const prior = output.findIndex((candidate) => lower(candidate.name) === lower(entry.name))
      if (prior >= 0) output.splice(prior, 1)
      output.push(entry)
    }
  }
  return output
}

function colorResolver(descriptor: Tf2UiResourceDescriptor, schemeIdentity: string) {
  const values = new Map<string, string>()
  for (const entry of [...composedSections(descriptor, schemeIdentity, "BaseSettings"), ...composedSections(descriptor, schemeIdentity, "Colors")]) {
    if (entry.value !== null) values.set(lower(entry.name), entry.value)
  }
  const resolve = (value: string, seen = new Set<string>()): readonly [number, number, number, number] | null => {
    const literal = literalColor(value)
    if (literal) return literal
    const key = lower(value)
    if (seen.has(key)) return null
    const next = values.get(key)
    return next === undefined ? null : resolve(next, new Set([...seen, key]))
  }
  return Object.freeze({ values, resolve })
}

function concreteBorder(
  name: string,
  node: Tf2UiResourceNode,
  resolveColor: (value: string) => readonly [number, number, number, number] | null,
): VguiBorder | null {
  if (node.value !== null) return null
  const borderType = scalar(node, "bordertype")
  const insets = integerList(scalar(node, "inset"), 4) ?? [0, 0, 0, 0]
  const background = Number(scalar(node, "backgroundtype") ?? 0)
  if (![0, 1, 2].includes(background)) return null
  const common = {
    name,
    inset: Object.freeze({ left: insets[0]!, top: insets[1]!, right: insets[2]!, bottom: insets[3]! }),
    backgroundType: background as 0 | 1 | 2,
    paintFirst: borderType ? scalar(node, "paintfirst") !== "0" : scalar(node, "paintfirst") === "1",
  }
  if (borderType?.toLowerCase() === "image") {
    const image = scalar(node, "image")
    if (!image) return null
    return Object.freeze({ ...common, kind: "image", image, tiled: scalar(node, "tiled") === "1" })
  }
  if (borderType?.toLowerCase() === "scalable_image") {
    const image = scalar(node, "image")
    const values = ["src_corner_width", "src_corner_height", "draw_corner_width", "draw_corner_height"].map((key) => Number(scalar(node, key) ?? 0))
    const colorValue = scalar(node, "color")
    const color = colorValue ? resolveColor(colorValue) : Object.freeze([255, 255, 255, 255] as const)
    if (!image || !color || values.some((value) => !Number.isSafeInteger(value) || value < 0)) return null
    return Object.freeze({
      ...common,
      kind: "scalable-image",
      image,
      sourceCornerWidth: values[0]!,
      sourceCornerHeight: values[1]!,
      drawCornerWidth: values[2]!,
      drawCornerHeight: values[3]!,
      color,
    })
  }
  if (borderType !== null) return null
  const sides = (side: string) => {
    const value = object(node, side)
    if (!value) return Object.freeze([])
    return Object.freeze(value.children.flatMap((record) => {
      const color = scalar(record, "color")
      const offset = integerList(scalar(record, "offset"), 2) ?? [0, 0]
      const resolved = color ? resolveColor(color) : null
      return resolved ? [Object.freeze({ color: resolved, startOffset: offset[0]!, endOffset: offset[1]! })] : []
    }))
  }
  return Object.freeze({
    ...common,
    kind: "line",
    sides: Object.freeze({ left: sides("Left"), top: sides("Top"), right: sides("Right"), bottom: sides("Bottom") }),
  })
}

function borders(
  descriptor: Tf2UiResourceDescriptor,
  schemeIdentity: string,
  diagnostics: Tf2UiIntegrationDiagnostic[],
): readonly VguiBorder[] {
  const resolve = colorResolver(descriptor, schemeIdentity).resolve
  const entries = composedSections(descriptor, schemeIdentity, "Borders")
  const result = new Map<string, VguiBorder>()
  for (const entry of entries) {
    const value = concreteBorder(entry.name, entry, resolve)
    if (value) result.set(lower(entry.name), value)
    else if (entry.value === null) diagnostics.push(Object.freeze({ code: "UnsupportedBorder", subject: `${schemeIdentity}:${entry.name}` }))
  }
  for (const entry of entries.filter((candidate) => candidate.value !== null)) {
    const target = result.get(lower(entry.value!))
    if (target) result.set(lower(entry.name), Object.freeze({ ...target, name: entry.name }))
    else diagnostics.push(Object.freeze({ code: "UnsupportedBorder", subject: `${schemeIdentity}:${entry.name}->${entry.value}` }))
  }
  return Object.freeze([...result.values()])
}

function customControls(descriptor: Tf2UiResourceDescriptor): readonly VguiControlRegistration[] {
  const properties = new Map<string, Set<string>>()
  const walk = (node: Tf2UiResourceNode) => {
    const control = scalar(node, "ControlName")
    if (control && CUSTOM_BASES[control]) {
      const selected = properties.get(control) ?? new Set<string>()
      for (const child of node.children) if (child.value !== null && lower(child.name) !== "controlname") selected.add(child.name)
      properties.set(control, selected)
    }
    for (const child of node.children) if (child.value === null) walk(child)
  }
  for (const panel of descriptor.panels) for (const root of panel.roots) walk(root)
  const configured = descriptor.controls.flatMap((control) => {
    const baseControl = CUSTOM_BASES[control.name]
    if (!baseControl) return []
    const element = baseControl === "Button" || baseControl === "CheckButton" ? "button"
      : baseControl === "ImagePanel" ? "div"
        : baseControl === "TextEntry" ? "textarea" : "div"
    const role = baseControl === "Button" ? "button" : baseControl === "CheckButton" ? "checkbox"
      : baseControl === "ImagePanel" ? "img" : baseControl === "Slider" ? "slider"
        : baseControl === "ComboBox" ? "combobox" : baseControl === "Frame" ? "dialog" : null
    return [Object.freeze({
      name: control.name,
      baseControl,
      element,
      role,
      focusable: ["Button", "CheckButton", "Slider", "ComboBox", "Frame", "ListPanel"].includes(baseControl),
      animationVariables: Object.freeze([]),
      acceptedProperties: Object.freeze([...(properties.get(control.name) ?? [])]),
    })]
  })
  return Object.freeze([
    ...configured,
    Object.freeze({
      name: "CHudMainMenuOverride", baseControl: "EditablePanel" as const, element: "div" as const, role: null,
      focusable: false, animationVariables: Object.freeze([]),
      acceptedProperties: Object.freeze(["update_url", "blog_url", "button_x_offset", "button_y", "button_y_delta"]),
    }),
    Object.freeze({
      name: "CTFMatchmakingDashboard", baseControl: "EditablePanel" as const, element: "div" as const, role: null,
      focusable: false, animationVariables: Object.freeze([]), acceptedProperties: Object.freeze(["collapsed_height", "expanded_height", "resize_time"]),
    }),
    Object.freeze({
      name: "CTFPlaylistPanel", baseControl: "EditablePanel" as const, element: "div" as const, role: null,
      focusable: false, animationVariables: Object.freeze([]), acceptedProperties: Object.freeze([]),
    }),
    Object.freeze({
      name: "CTFHudElement", baseControl: "EditablePanel" as const, element: "div" as const, role: null,
      focusable: false, animationVariables: Object.freeze([]), acceptedProperties: Object.freeze([
        "RightMargin", "SmallBoxWide", "SmallBoxTall", "PlusStyleBoxWide", "PlusStyleBoxTall",
        "PlusStyleExpandSelected", "LargeBoxWide", "LargeBoxTall", "BoxGap", "SelectionNumberXPos",
        "SelectionNumberYPos", "IconXPos", "IconYPos", "TextYPos", "ErrorYPos", "TextColor", "MaxSlots",
        "PlaySelectSounds", "SelectionAlpha", "BoxColor", "SelectedBoxClor", "SelectionNumberFg", "NumberFont",
        "HealthBonusPosAdj", "HealthDeathWarning", "HealthDeathWarningColor", "MeterFG", "MeterBG",
      ]),
    }),
    Object.freeze({
      name: "CTFHealthPanel", baseControl: "ImagePanel" as const, element: "div" as const, role: "img",
      focusable: false, animationVariables: Object.freeze([]), acceptedProperties: Object.freeze([]),
    }),
    Object.freeze({
      name: "CTFAdvancedOptionsDialog", baseControl: "EditablePanel" as const, element: "div" as const, role: "dialog",
      focusable: true,
      animationVariables: Object.freeze([
        { name: "control_w", converter: "proportional_int" as const, defaultValue: "0" },
        { name: "control_h", converter: "proportional_int" as const, defaultValue: "0" },
        { name: "slider_w", converter: "proportional_int" as const, defaultValue: "0" },
        { name: "slider_h", converter: "proportional_int" as const, defaultValue: "0" },
      ]),
      acceptedProperties: Object.freeze([]),
    }),
  ])
}

async function fontPresentations(
  descriptor: Tf2UiResourceDescriptor,
  schemeIdentity: string,
  dependencies: ReadonlyMap<string, Uint8Array>,
  viewportHeight: number,
  platform: VguiDesktopPlatform,
  diagnostics: Tf2UiIntegrationDiagnostic[],
  mounts: VguiFontSetMount[],
): Promise<readonly VguiFontPresentation[]> {
  const documents = descriptor.schemes.map(schemeDocument)
  const files: VguiFontFileIdentity[] = []
  const supplies: VguiFontByteSupply[] = []
  for (const font of descriptor.fonts) {
    if (!font.source || font.source.outcome !== "found" || !font.source.sha256 || font.source.byteLength === null) continue
    const bytes = sourceBytes(dependencies, font.source.logicalPath, font.source.byteLength, font.source.sha256, diagnostics)
    if (!bytes) continue
    const bitmap = font.classification === "content-bitmap"
    const metadata = bitmap
      ? Object.freeze({ version: `vbf-sha256:${font.source.sha256}`, families: Object.freeze([font.configuredValue.includes("buttons_sc") ? "ButtonsSC" : "Buttons"]) })
      : sfnt(bytes)
    const file = Object.freeze({
      kind: bitmap ? "bitmap" as const : "content" as const,
      logicalIdentity: font.source.logicalPath,
      sha256: font.source.sha256,
      byteLength: font.source.byteLength,
      version: metadata.version,
      families: metadata.families,
    })
    files.push(file)
    supplies.push(Object.freeze({ ...file, bytes }))
  }
  const names = composedSections(descriptor, schemeIdentity, "Fonts").map((entry) => entry.name)
  const output: VguiFontPresentation[] = []
  for (const [index, name] of names.entries()) {
    const identity = `font-${String(index + 1).padStart(4, "0")}`
    const resolved = resolveVguiSchemeFonts({
      schemeLogicalIdentity: schemeIdentity,
      documents,
      fontFiles: files,
      localFonts: [],
      context: {
        platform,
        viewportHeight,
        language: "english",
        minMode: false,
        steamDeck: false,
        surfaceFeatures: { antialias: true, dropShadow: true, outline: true },
      },
      lookups: [{ identity, name, proportional: schemeIdentity === "resource/clientscheme.res" }],
    })
    if (!resolved.ok) {
      diagnostics.push(Object.freeze({ code: "FontUnavailable", subject: `${schemeIdentity}:${name}:${resolved.diagnostic.code}` }))
      continue
    }
    const request = resolved.fonts[0]!
    const requiredSupplies = new Set(request.faces.flatMap((face) => face.sources.flatMap((source) => source.kind === "local" ? [] : [source.logicalIdentity])))
    const mounted = await mountVguiFontSet({
      identity: `${descriptor.identity}/${schemeIdentity}/${identity}`,
      fonts: [request],
      byteSupplies: supplies.filter((supply) => requiredSupplies.has(supply.logicalIdentity)),
      profiles: [],
    })
    const capability = mounted.ok ? mounted.fontSet.snapshot().capability : mounted.capability
    if (mounted.ok) mounts.push(mounted.fontSet)
    const selected = capability.kind === "supported" ? capability.fonts[0] : null
    if (capability.kind === "unsupported") diagnostics.push(Object.freeze({ code: "FontUnavailable", subject: `${schemeIdentity}:${name}:${capability.reason}` }))
    const browserFamily = selected?.browserFamily ?? `playsrc-unavailable-${identity}`
    const measure = selected && typeof document !== "undefined" ? ((text: string, wrapWidth: number | null) => {
      const canvas = document.createElement("canvas")
      const context = canvas.getContext("2d")
      if (!context) return Object.freeze({ width: 0, height: request.requestedHeight })
      context.font = `${request.effects.italic ? "italic " : ""}${request.weight || 400} ${request.requestedHeight}px ${JSON.stringify(browserFamily)}`
      if (wrapWidth === null || wrapWidth <= 0) return Object.freeze({ width: context.measureText(text).width, height: request.requestedHeight })
      const words = text.split(/\s+/u)
      let lines = 1
      let width = 0
      let current = ""
      for (const word of words) {
        const candidate = current ? `${current} ${word}` : word
        const candidateWidth = context.measureText(candidate).width
        if (current && candidateWidth > wrapWidth) { lines += 1; width = Math.max(width, context.measureText(current).width); current = word }
        else current = candidate
      }
      width = Math.max(width, context.measureText(current).width)
      return Object.freeze({ width: Math.min(wrapWidth, width), height: lines * request.requestedHeight })
    }) : undefined
    output.push(Object.freeze({
      name,
      cssFamily: browserFamily,
      sizePx: request.requestedHeight,
      lineHeightPx: request.requestedHeight + (request.effects.outline ? 2 : 0),
      weight: request.weight,
      style: request.effects.italic ? "italic" : "normal",
      available: selected !== null,
      ...(measure ? { measure } : {}),
    }))
  }
  return Object.freeze(output)
}

function makeScheme(
  descriptor: Tf2UiResourceDescriptor,
  identity: string,
  fonts: readonly VguiFontPresentation[],
  images: readonly VguiImagePresentation[],
  diagnostics: Tf2UiIntegrationDiagnostic[],
): VguiScheme {
  const selected = descriptor.schemes.find((scheme) => scheme.source.logicalPath === identity)
  if (!selected?.source.sha256) throw new Error(`Configured scheme ${identity} is missing`)
  const colors = composedSections(descriptor, identity, "Colors").flatMap((entry) => entry.value === null ? [] : [Object.freeze({ name: entry.name, value: entry.value })])
  const settings = composedSections(descriptor, identity, "BaseSettings").flatMap((entry) => entry.value === null ? [] : [Object.freeze({ name: entry.name, value: entry.value })])
  return Object.freeze({
    identity,
    revision: selected.source.sha256,
    tag: identity === "resource/clientscheme.res" ? "ClientScheme" : "SourceScheme",
    colors: Object.freeze(colors),
    settings: Object.freeze(settings),
    fonts,
    borders: borders(descriptor, identity, diagnostics),
    images,
  })
}

export async function initializeTf2VguiResources(request: Tf2VguiResourceRequest): Promise<Tf2VguiResources> {
  if (!request || !Number.isSafeInteger(request.viewportHeight) || request.viewportHeight <= 0 || request.viewportHeight > 32_767) {
    throw new Error("TF2 VGUI viewport is invalid")
  }
  const descriptor = tf2UiResources
  const diagnostics: Tf2UiIntegrationDiagnostic[] = []
  const urls: string[] = []
  const mounts: VguiFontSetMount[] = []
  const createUrl = request.createObjectUrl ?? ((bytes: Uint8Array, mediaType: string) => URL.createObjectURL(new Blob([bytes.slice()], { type: mediaType })))
  const revokeUrl = request.revokeObjectUrl ?? ((url: string) => URL.revokeObjectURL(url))
  const materialBytes = request.dependencies.get("playsrc/tf2-ui/materials.json")
  if (!materialBytes) throw new Error("TF2 UI material descriptor is missing")
  let materialInput: unknown
  try { materialInput = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(materialBytes)) }
  catch { throw new Error("TF2 UI material descriptor is malformed") }
  if (!materialInput || typeof materialInput !== "object" || Array.isArray(materialInput)
    || (materialInput as Record<string, unknown>).schema !== "playsrc-tf2-ui-materials-v1"
    || (materialInput as Record<string, unknown>).descriptor !== descriptor.identity
    || !Array.isArray((materialInput as Record<string, unknown>).images)
    || !Array.isArray((materialInput as Record<string, unknown>).textures)) {
    throw new Error("TF2 UI material descriptor identity differs")
  }
  const materialRecords = new Map<string, RawUiMaterial>()
  for (const value of (materialInput as { images: unknown[] }).images) {
    if (!value || typeof value !== "object" || Array.isArray(value) || typeof (value as RawUiMaterial).configuredValue !== "string"
      || !SHA256.test((value as RawUiMaterial).materialSha256)) {
      throw new Error("TF2 UI material record is malformed")
    }
    const record = value as RawUiMaterial
    if (materialRecords.has(lower(record.configuredValue))) throw new Error(`Duplicate TF2 UI material ${record.configuredValue}`)
    materialRecords.set(lower(record.configuredValue), record)
  }
  const textureDescriptors = new Map<string, RawUiTexture>()
  for (const value of (materialInput as { textures: unknown[] }).textures) {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("TF2 UI texture record is malformed")
    const texture = value as RawUiTexture
    if (typeof texture.logicalPath !== "string" || !SHA256.test(texture.sha256) || !Number.isSafeInteger(texture.width) || !Number.isSafeInteger(texture.height) || !Number.isSafeInteger(texture.frames) || !Number.isSafeInteger(texture.rawFlags)) throw new Error("TF2 UI texture record is malformed")
    textureDescriptors.set(texture.logicalPath, texture)
  }
  const textureUrls = new Map<string, string>()
  const texturePresentation = (logicalPath: string, colorRead: string | null): VguiImageMaterialTexture => {
    const texture = textureDescriptors.get(logicalPath)
    if (!texture) throw new Error(`TF2 UI texture ${logicalPath} is unavailable`)
    const path = `playsrc/tf2-ui/png/${texture.sha256}/0.png`
    const bytes = request.dependencies.get(path)
    if (!bytes) throw new Error(`TF2 UI browser texture ${path} is unavailable`)
    let browserUrl = textureUrls.get(path)
    if (!browserUrl) {
      browserUrl = createUrl(bytes, "image/png")
      textureUrls.set(path, browserUrl)
      urls.push(browserUrl)
    }
    const resolvedColorRead = colorRead === "srgb" || (colorRead === "format-dependent" && (texture.rawFlags & 0x40) !== 0) ? "srgb" : "linear"
    return Object.freeze({
      logicalIdentity: logicalPath,
      revision: texture.sha256,
      browserUrl,
      width: texture.width,
      height: texture.height,
      hardwareFiltered: (texture.rawFlags & 1) === 0,
      colorRead: resolvedColorRead,
    })
  }
  const images: VguiImagePresentation[] = []
  for (const image of descriptor.images) {
    if (image.classification !== "content-vtf") {
      diagnostics.push(Object.freeze({ code: "UnsupportedImageMaterial", subject: image.configuredValue }))
      continue
    }
    const raw = materialRecords.get(lower(image.configuredValue))
    if (!raw) throw new Error(`TF2 UI material record ${image.configuredValue} is missing`)
    if (raw.materialSha256 !== image.material!.sha256) throw new Error(`TF2 UI material record ${image.configuredValue} changed`)
    const base = texturePresentation(raw.baseTexture, raw.baseColorRead)
    const second = raw.secondTexture ? texturePresentation(raw.secondTexture, raw.secondColorRead) : null
    const detail = raw.detailTexture ? texturePresentation(raw.detailTexture, raw.detailColorRead) : null
    const material: VguiImageMaterialPresentation = Object.freeze({
      shader: raw.shader.toLowerCase() === "unlittwotexture" ? "unlit-two-texture" : "unlit-generic",
      base,
      second,
      detail,
      detailScale: raw.detailScale,
      detailBlendMode: raw.detailBlendMode as 0 | 8,
      detailBlendFactor: raw.detailBlendFactor,
      detailTint: Object.freeze([...raw.detailTint]) as readonly [number, number, number],
      distanceAlpha: raw.distanceAlpha,
      distanceAlphaFromDetail: raw.distanceAlphaFromDetail,
      softEdges: raw.softEdges,
      scaleSoftEdges: raw.scaleSoftEdges,
      edgeSoftnessStart: raw.edgeSoftnessStart,
      edgeSoftnessEnd: raw.edgeSoftnessEnd,
      outline: raw.outline,
      outlineColor: Object.freeze([...raw.outlineColor]) as readonly [number, number, number],
      outlineAlpha: raw.outlineAlpha,
      outlineStart0: raw.outlineStart0,
      outlineStart1: raw.outlineStart1,
      outlineEnd0: raw.outlineEnd0,
      outlineEnd1: raw.outlineEnd1,
      scaleOutline: raw.scaleOutline,
      glow: raw.glow,
      glowColor: Object.freeze([...raw.glowColor]) as readonly [number, number, number],
      glowAlpha: raw.glowAlpha,
      glowStart: raw.glowStart,
      glowEnd: raw.glowEnd,
      glowX: raw.glowX,
      glowY: raw.glowY,
    })
    images.push(Object.freeze({
      name: image.configuredValue,
      logicalIdentity: raw.material,
      revision: image.material!.sha256!,
      browserUrl: base.browserUrl,
      width: base.width,
      height: base.height,
      frames: 1,
      hardwareFiltered: base.hardwareFiltered,
      material,
    }))
  }
  const existingImages = new Set(images.map((image) => lower(image.name)))
  for (const raw of materialRecords.values()) {
    if (existingImages.has(lower(raw.configuredValue))) continue
    const base = texturePresentation(raw.baseTexture, raw.baseColorRead)
    const second = raw.secondTexture ? texturePresentation(raw.secondTexture, raw.secondColorRead) : null
    const detail = raw.detailTexture ? texturePresentation(raw.detailTexture, raw.detailColorRead) : null
    const material: VguiImageMaterialPresentation = Object.freeze({
      shader: raw.shader.toLowerCase() === "unlittwotexture" ? "unlit-two-texture" : "unlit-generic",
      base, second, detail,
      detailScale: raw.detailScale,
      detailBlendMode: raw.detailBlendMode as 0 | 8,
      detailBlendFactor: raw.detailBlendFactor,
      detailTint: Object.freeze([...raw.detailTint]) as readonly [number, number, number],
      distanceAlpha: raw.distanceAlpha,
      distanceAlphaFromDetail: raw.distanceAlphaFromDetail,
      softEdges: raw.softEdges,
      scaleSoftEdges: raw.scaleSoftEdges,
      edgeSoftnessStart: raw.edgeSoftnessStart,
      edgeSoftnessEnd: raw.edgeSoftnessEnd,
      outline: raw.outline,
      outlineColor: Object.freeze([...raw.outlineColor]) as readonly [number, number, number],
      outlineAlpha: raw.outlineAlpha,
      outlineStart0: raw.outlineStart0,
      outlineStart1: raw.outlineStart1,
      outlineEnd0: raw.outlineEnd0,
      outlineEnd1: raw.outlineEnd1,
      scaleOutline: raw.scaleOutline,
      glow: raw.glow,
      glowColor: Object.freeze([...raw.glowColor]) as readonly [number, number, number],
      glowAlpha: raw.glowAlpha,
      glowStart: raw.glowStart,
      glowEnd: raw.glowEnd,
      glowX: raw.glowX,
      glowY: raw.glowY,
    })
    images.push(Object.freeze({
      name: raw.configuredValue,
      logicalIdentity: raw.material,
      revision: raw.materialSha256,
      browserUrl: base.browserUrl,
      width: base.width,
      height: base.height,
      frames: 1,
      hardwareFiltered: base.hardwareFiltered,
      material,
    }))
  }
  const backgroundBytes = request.dependencies.get("playsrc/tf2-gameui-background.json")
  if (!backgroundBytes) throw new Error("TF2 GameUI base-background descriptor is missing")
  let backgroundInput: unknown
  try { backgroundInput = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(backgroundBytes)) }
  catch { throw new Error("TF2 GameUI base-background descriptor is malformed") }
  if (!backgroundInput || typeof backgroundInput !== "object" || Array.isArray(backgroundInput)) throw new Error("TF2 GameUI base-background descriptor is malformed")
  const rawBackground = backgroundInput as RawGameUiBackground
  if (rawBackground.schema !== "playsrc-tf2-gameui-background-v1" || rawBackground.contentBuild !== descriptor.contentBuild
    || rawBackground.defaultChapter !== 1 || !rawBackground.backgroundName
    || !rawBackground.chapterSource || rawBackground.chapterSource.logicalPath !== "scripts/chapterbackgrounds.txt"
    || !Number.isSafeInteger(rawBackground.chapterSource.byteLength) || rawBackground.chapterSource.byteLength < 1
    || !SHA256.test(rawBackground.chapterSource.sha256) || !Array.isArray(rawBackground.variants) || rawBackground.variants.length !== 2) {
    throw new Error("TF2 GameUI base-background descriptor identity differs")
  }
  const backgroundVariants = rawBackground.variants.map((variant, index) => {
    const aspect = index === 0 ? "standard" : "widescreen"
    const image = images.find((candidate) => lower(candidate.name) === lower(variant.configuredValue))
    const material = image?.material
    if (variant.aspect !== aspect || !image || !material || image.logicalIdentity !== variant.material
      || image.revision !== variant.materialSha256 || material.base.logicalIdentity !== variant.texture
      || material.base.revision !== variant.textureSha256 || image.width !== variant.width || image.height !== variant.height
      || !SHA256.test(variant.materialSha256) || !SHA256.test(variant.textureSha256)
      || !Number.isSafeInteger(variant.width) || !Number.isSafeInteger(variant.height) || variant.width < 1 || variant.height < 1) {
      throw new Error(`TF2 GameUI base-background ${aspect} variant differs`)
    }
    return Object.freeze({
      aspect,
      image: variant.configuredValue,
      material: variant.material,
      materialSha256: variant.materialSha256,
      texture: variant.texture,
      textureSha256: variant.textureSha256,
      width: variant.width,
      height: variant.height,
    })
  })
  const gameUiBackground: Tf2GameUiBackgroundDescriptor = Object.freeze({
    identity: `tf2-gameui-background-${descriptor.contentBuild}-${rawBackground.chapterSource.sha256.slice(0, 16)}`,
    contentBuild: rawBackground.contentBuild,
    source: Object.freeze({ ...rawBackground.chapterSource }),
    defaultChapter: 1,
    backgroundName: rawBackground.backgroundName,
    variants: Object.freeze(backgroundVariants),
  })
  const [clientFonts, sourceFonts] = await Promise.all([
    fontPresentations(descriptor, "resource/clientscheme.res", request.dependencies, request.viewportHeight, request.platform, diagnostics, mounts),
    fontPresentations(descriptor, "resource/sourcescheme.res", request.dependencies, request.viewportHeight, request.platform, diagnostics, mounts),
  ])
  const localization: VguiLocalization = Object.freeze({
    identity: `${descriptor.identity}/localization/english`,
    revision: `${descriptor.identity}-localization-english`,
    language: "english",
    tokens: Object.freeze(descriptor.localization.tokens.flatMap((token) => {
      const definition = token.definitions.at(-1)
      return definition ? [Object.freeze({ name: token.name.replace(/^#/u, ""), value: definition.value })] : []
    })),
  })
  const parsedScripts = []
  for (const source of descriptor.animation.scripts) {
    const bytes = sourceBytes(request.dependencies, source.logicalPath, source.byteLength, source.sha256, diagnostics)
    if (!bytes || !source.sha256) continue
    const parsed = parseVguiAnimationScript(source.logicalPath, source.sha256, bytes, {
      maximumSourceBytes: 1_048_576,
      maximumTokenCodeUnits: 511,
      maximumSequences: 1_024,
      maximumCommands: 8_192,
    })
    if (!parsed.ok) diagnostics.push(Object.freeze({ code: "AnimationMalformed", subject: `${source.logicalPath}:${parsed.diagnostic.subject}` }))
    else parsedScripts.push(parsed.script)
  }
  const activeConditions = Object.freeze([
    "WIN32",
    ...(request.platform === "windows" ? ["WINDOWS"] : request.platform === "macos" ? ["OSX", "POSIX"] : ["LINUX", "POSIX"]),
  ])
  const animations: VguiAnimationScriptSet = Object.freeze({
    identity: `${descriptor.identity}/hudanimations`,
    revision: `${descriptor.identity}-hudanimations`,
    scripts: Object.freeze(parsedScripts),
    activeConditions,
  })
  const panels = new Map(descriptor.panels.map((panel) => [panel.source.logicalPath, panel]))
  const supportedImages = new Set(images.map((image) => lower(image.name)))
  const borderNames = new Map([
    ["resource/clientscheme.res", new Set(borders(descriptor, "resource/clientscheme.res", []).map((border) => lower(border.name)))],
    ["resource/sourcescheme.res", new Set(borders(descriptor, "resource/sourcescheme.res", []).map((border) => lower(border.name)))],
  ])
  const documents = new Map<string, VguiResourceDocument>()
  const convertDocument = (panel: Tf2UiPanelDocument): VguiResourceDocument => {
    const root = panel.roots[0]
    if (!root || !panel.source.sha256) throw new Error(`Configured panel document ${panel.source.logicalPath} is malformed`)
    const convert = (node: Tf2UiResourceNode): VguiResourceNode => {
      const control = scalar(node, "ControlName")
      const unsupported = control && UNSUPPORTED_GENERIC_CONTROLS.has(control)
      if (unsupported) diagnostics.push(Object.freeze({ code: "UnsupportedControl", subject: `${panel.source.logicalPath}:${control}:${node.name}` }))
      const children = node.children.filter((child) => {
        if (child.value === null) {
          const nestedControl = scalar(child, "ControlName")
          if (nestedControl && UNSUPPORTED_GENERIC_CONTROLS.has(nestedControl)) {
            diagnostics.push(Object.freeze({ code: "UnsupportedControl", subject: `${panel.source.logicalPath}:${nestedControl}:${child.name}` }))
            return false
          }
          return true
        }
        if (lower(child.name) === "image" && !supportedImages.has(lower(child.value))) {
          diagnostics.push(Object.freeze({ code: "UnsupportedImageMaterial", subject: `${panel.source.logicalPath}:${node.name}:${child.value}` }))
          return false
        }
        if (/(?:_lodef|_minmode)$/iu.test(child.name)) {
          diagnostics.push(Object.freeze({ code: "UnsupportedProperty", subject: `${panel.source.logicalPath}:${node.name}:inactive-resolution:${child.name}` }))
          return false
        }
        if ((lower(child.name) === "border" || lower(child.name) === "border_override")
          && !borderNames.get(panel.domain === "hud" || panel.domain === "main-menu" || panel.domain === "team-selection" ? "resource/clientscheme.res" : "resource/sourcescheme.res")?.has(lower(child.value))) {
          diagnostics.push(Object.freeze({ code: "UnsupportedBorder", subject: `${panel.source.logicalPath}:${node.name}:${child.value}` }))
          return false
        }
        if (lower(child.name) === "scaleimage" && control !== "ImagePanel" && control !== "CTFImagePanel" && control !== "CTFClassImage") {
          diagnostics.push(Object.freeze({ code: "UnsupportedProperty", subject: `${panel.source.logicalPath}:${node.name}:${control ?? "code-created"}.${child.name}` }))
          return false
        }
        if (control && GENERIC_CONTROLS.has(control) && !isVguiGenericResourcePropertySupported(control as VguiGenericControlName, child.name)) {
          diagnostics.push(Object.freeze({ code: "UnsupportedProperty", subject: `${panel.source.logicalPath}:${node.name}:${control}.${child.name}` }))
          return false
        }
        return true
      })
      return resourceNode(node, children.map(convert))
    }
    return Object.freeze({
      logicalIdentity: panel.source.logicalPath,
      revision: panel.source.sha256,
      root: convert(root),
    })
  }
  for (const panel of descriptor.panels) documents.set(panel.source.logicalPath, convertDocument(panel))
  let destroyed = false
  return Object.freeze({
    identity: descriptor.identity,
    descriptor,
    clientScheme: makeScheme(descriptor, "resource/clientscheme.res", clientFonts, images, diagnostics),
    sourceScheme: makeScheme(descriptor, "resource/sourcescheme.res", sourceFonts, images, diagnostics),
    localization,
    animations,
    activeConditions,
    customControls: customControls(descriptor),
    gameUiBackground,
    diagnostics: Object.freeze(diagnostics),
    document(logicalPath: string) {
      const document = documents.get(logicalPath)
      if (!document) throw new Error(`TF2 VGUI document ${logicalPath} is unavailable`)
      return document
    },
    panelDocument(logicalPath: string) {
      const panel = panels.get(logicalPath)
      if (!panel) throw new Error(`TF2 panel document ${logicalPath} is unavailable`)
      return panel
    },
    destroy() {
      if (destroyed) return
      destroyed = true
      for (const mount of mounts) mount.destroy()
      for (const url of urls) revokeUrl(url)
    },
  })
}
