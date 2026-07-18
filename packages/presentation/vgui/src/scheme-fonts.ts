export type VguiDesktopPlatform = "windows" | "macos" | "linux"

export type VguiSchemeNode = Readonly<{
  name: string
  value: string | null
  condition: string | null
  children: readonly VguiSchemeNode[]
}>

export type VguiSchemeDocument = Readonly<{
  logicalIdentity: string
  sha256: string
  baseLogicalIdentities: readonly string[]
  root: VguiSchemeNode
}>

export type VguiFontFileIdentity = Readonly<{
  kind: "content" | "external" | "bitmap"
  logicalIdentity: string
  sha256: string
  byteLength: number
  version: string
  families: readonly string[]
}>

export type VguiLocalFontIdentity = Readonly<{
  platform: VguiDesktopPlatform
  faceName: string
  version: string | null
}>

export type VguiSchemeSurfaceFeatures = Readonly<{
  antialias: boolean
  dropShadow: boolean
  outline: boolean
}>

export type VguiSchemeFontContext = Readonly<{
  platform: VguiDesktopPlatform
  viewportHeight: number
  language: string
  minMode: boolean
  steamDeck: boolean
  surfaceFeatures: VguiSchemeSurfaceFeatures
}>

export type VguiSchemeFontLookup = Readonly<{
  identity: string
  name: string
  proportional: boolean
}>

export type VguiFontSourceRequest =
  | Readonly<{
      kind: "local"
      identity: string
      platform: VguiDesktopPlatform
      faceName: string
      version: string | null
    }>
  | Readonly<{
      kind: "content" | "external" | "bitmap"
      identity: string
      logicalIdentity: string
      sha256: string
      byteLength: number
      version: string
      family: string
    }>

export type VguiFontFaceRequest = Readonly<{
  identity: string
  unicodeRange: readonly [minimum: number, maximum: number]
  sources: readonly VguiFontSourceRequest[]
}>

export type VguiFontEffects = Readonly<{
  italic: boolean
  underline: boolean
  strikeout: boolean
  symbol: boolean
  antialias: boolean
  blur: number
  scanlines: number
  dropShadow: boolean
  outline: boolean
  additive: boolean
  custom: boolean
  bitmap: boolean
  rotary: boolean
}>

export type VguiResolvedFontRequest = Readonly<{
  identity: string
  schemeFontName: string
  proportional: boolean
  family: string
  candidateName: string
  requestedHeight: number
  weight: number
  bitmapScale: readonly [x: number, y: number]
  effects: VguiFontEffects
  faces: readonly VguiFontFaceRequest[]
}>

export type VguiSchemeFontDiagnosticCode =
  | "InvalidInput"
  | "DuplicateDocument"
  | "MissingDocument"
  | "CyclicBase"
  | "MalformedDocument"
  | "MissingFont"
  | "MissingCandidate"
  | "MalformedCandidate"
  | "MissingFontFileIdentity"
  | "MissingBitmapFile"

export type VguiSchemeFontDiagnostic = Readonly<{
  code: VguiSchemeFontDiagnosticCode
  subject: string
}>

export type VguiSchemeFontResolution =
  | Readonly<{
      ok: true
      schemeLogicalIdentity: string
      schemeSha256: string
      documentLogicalIdentities: readonly string[]
      fonts: readonly VguiResolvedFontRequest[]
    }>
  | Readonly<{ ok: false; diagnostic: VguiSchemeFontDiagnostic }>

export type VguiSchemeFontResolutionRequest = Readonly<{
  schemeLogicalIdentity: string
  documents: readonly VguiSchemeDocument[]
  fontFiles: readonly VguiFontFileIdentity[]
  localFonts: readonly VguiLocalFontIdentity[]
  context: VguiSchemeFontContext
  lookups: readonly VguiSchemeFontLookup[]
}>

type MutableNode = {
  name: string
  value: string | null
  children: MutableNode[]
}

type CustomFont = Readonly<{
  family: string
  logicalIdentity: string
  file: VguiFontFileIdentity | null
  range: readonly [number, number] | null
}>

const IDENTITY = /^[a-z0-9][a-z0-9./_-]{0,511}$/u
const SHA256 = /^[a-f0-9]{64}$/u

function asciiFold(value: string): string {
  return value.replace(/[A-Z]/gu, (character) => character.toLowerCase())
}

function sameName(left: string, right: string): boolean {
  return asciiFold(left) === asciiFold(right)
}

function validNode(node: VguiSchemeNode, seen: Set<VguiSchemeNode>): boolean {
  if (
    !node
    || typeof node.name !== "string"
    || node.name.length === 0
    || node.name.length > 255
    || (node.value !== null && typeof node.value !== "string")
    || (node.condition !== null && typeof node.condition !== "string")
    || !Array.isArray(node.children)
    || seen.has(node)
  ) return false
  seen.add(node)
  const valid = node.children.every((child) => validNode(child, seen))
  seen.delete(node)
  return valid
}

function validContext(context: VguiSchemeFontContext): boolean {
  return !!context
    && ["windows", "macos", "linux"].includes(context.platform)
    && Number.isSafeInteger(context.viewportHeight)
    && context.viewportHeight > 0
    && context.viewportHeight <= 32767
    && /^[a-z][a-z0-9_-]{0,63}$/u.test(context.language)
    && typeof context.minMode === "boolean"
    && typeof context.steamDeck === "boolean"
    && !!context.surfaceFeatures
    && Object.values(context.surfaceFeatures).every((value) => typeof value === "boolean")
}

export function evaluateVguiSchemeCondition(
  condition: string | null,
  context: Pick<VguiSchemeFontContext, "platform" | "steamDeck">,
): boolean {
  if (condition === null) return true
  if (typeof condition !== "string") return false
  let value = condition
  if (value.startsWith("[")) value = value.slice(1)
  const negate = value.startsWith("!")
  const folded = asciiFold(value)
  let selected: boolean
  if (folded.includes("$deck")) selected = context.steamDeck
  else if (folded.includes("$x360")) selected = false
  else if (folded.includes("$win32")) selected = true
  else if (folded.includes("$windows")) selected = context.platform === "windows"
  else if (folded.includes("$osx")) selected = context.platform === "macos"
  else if (folded.includes("$linux")) selected = context.platform === "linux"
  else if (folded.includes("$posix")) selected = context.platform !== "windows"
  else return false
  return selected !== negate
}

function filterNode(node: VguiSchemeNode, context: VguiSchemeFontContext): MutableNode | null {
  if (!evaluateVguiSchemeCondition(node.condition, context)) return null
  return {
    name: node.name,
    value: node.value,
    children: node.children
      .map((child) => filterNode(child, context))
      .filter((child): child is MutableNode => child !== null),
  }
}

function cloneNode(node: MutableNode): MutableNode {
  return { name: node.name, value: node.value, children: node.children.map(cloneNode) }
}

function mergeBase(target: MutableNode, base: MutableNode): void {
  for (const baseChild of base.children) {
    const existing = target.children.find((child) => child.name === baseChild.name)
    if (existing) mergeBase(existing, baseChild)
    else target.children.push(cloneNode(baseChild))
  }
}

function applyResolutionSuffix(node: MutableNode, suffix: string): void {
  for (const child of node.children) applyResolutionSuffix(child, suffix)
  for (let index = 0; index < node.children.length; index += 1) {
    const child = node.children[index]
    const foldedName = asciiFold(child.name)
    const foldedSuffix = asciiFold(suffix)
    if (!foldedName.endsWith(foldedSuffix)) continue
    const plainName = child.name.slice(0, child.name.length - suffix.length)
    const originalIndex = node.children.findIndex((candidate) => candidate !== child && sameName(candidate.name, plainName))
    if (originalIndex >= 0) {
      node.children.splice(originalIndex, 1)
      if (originalIndex < index) index -= 1
    }
    child.name = plainName
  }
}

function firstChild(node: MutableNode, name: string): MutableNode | null {
  return node.children.find((child) => sameName(child.name, name)) ?? null
}

function scalar(node: MutableNode, name: string): string | null {
  return firstChild(node, name)?.value ?? null
}

function sourceInteger(value: string | null): number {
  if (value === null) return 0
  const match = value.match(/^\s*([+-]?\d+)/u)
  return match ? Number.parseInt(match[1], 10) : 0
}

function sourceFloat(value: string | null, fallback: number): number {
  if (value === null) return fallback
  const match = value.match(/^\s*([+-]?(?:\d+(?:\.\d*)?|\.\d+))/u)
  return match ? Number.parseFloat(match[1]) : fallback
}

function parseYResolution(value: string | null): readonly [number, number] {
  if (value === null) return Object.freeze([0, 0])
  const match = value.match(/^\s*([+-]?\d+)(?:\s+([+-]?\d+))?/u)
  const minimum = match ? Number.parseInt(match[1], 10) : 0
  const maximum = match?.[2] === undefined ? (minimum === 0 ? 0 : minimum) : Number.parseInt(match[2], 10)
  return Object.freeze([minimum, maximum])
}

function parseHexRange(value: string | null): readonly [number, number] | null {
  if (value === null) return null
  const match = value.match(/^\s*(?:0x)?([a-f0-9]+)\s+(?:0x)?([a-f0-9]+)/iu)
  if (!match) return null
  let minimum = Number.parseInt(match[1], 16)
  let maximum = Number.parseInt(match[2], 16)
  if (!Number.isSafeInteger(minimum) || !Number.isSafeInteger(maximum)) return null
  if (minimum > maximum) [minimum, maximum] = [maximum, minimum]
  if (minimum < 0 || maximum > 0xffff) return null
  return Object.freeze([minimum, maximum])
}

function languageMinimum(language: string): number {
  if (["korean", "tchinese", "schinese", "japanese"].includes(language)) return 13
  if (language === "thai") return 18
  return 0
}

function scaled(value: number, viewportHeight: number): number {
  return Math.trunc(value * viewportHeight / 480)
}

function localSource(
  faceName: string,
  context: VguiSchemeFontContext,
  localFonts: readonly VguiLocalFontIdentity[],
): VguiFontSourceRequest {
  const identity = localFonts.find((font) => font.platform === context.platform && sameName(font.faceName, faceName))
  return Object.freeze({
    kind: "local" as const,
    identity: `local:${context.platform}:${asciiFold(faceName)}`,
    platform: context.platform,
    faceName,
    version: identity?.version ?? null,
  })
}

function fileSource(file: VguiFontFileIdentity, family: string): VguiFontSourceRequest {
  return Object.freeze({
    kind: file.kind,
    identity: `${file.kind}:${file.logicalIdentity}:${file.sha256}`,
    logicalIdentity: file.logicalIdentity,
    sha256: file.sha256,
    byteLength: file.byteLength,
    version: file.version,
    family,
  })
}

function uniqueSources(sources: readonly VguiFontSourceRequest[]): readonly VguiFontSourceRequest[] {
  const identities = new Set<string>()
  return Object.freeze(sources.filter((source) => {
    if (identities.has(source.identity)) return false
    identities.add(source.identity)
    return true
  }))
}

function platformFontPolicy(platform: VguiDesktopPlatform): Readonly<{
  foreign: string
  completeFamilies: readonly string[]
  ultimateFallback: string
}> {
  if (platform === "windows") {
    return Object.freeze({ foreign: "Tahoma", completeFamilies: Object.freeze(["Marlett"]), ultimateFallback: "Tahoma" })
  }
  if (platform === "macos") {
    return Object.freeze({ foreign: "Helvetica", completeFamilies: Object.freeze(["Apple Symbols"]), ultimateFallback: "Monaco" })
  }
  return Object.freeze({
    foreign: "WenQuanYi Zen Hei",
    completeFamilies: Object.freeze(["Marlett", "WenQuanYi Zen Hei", "unifont"]),
    ultimateFallback: "DejaVu Sans",
  })
}

function customFonts(
  root: MutableNode,
  context: VguiSchemeFontContext,
  files: readonly VguiFontFileIdentity[],
): readonly CustomFont[] | VguiSchemeFontDiagnostic {
  const section = firstChild(root, "CustomFontFiles")
  if (!section) return Object.freeze([])
  const result: CustomFont[] = []
  for (const entry of section.children) {
    const path = entry.value && entry.value.length > 0 ? entry.value : scalar(entry, "font")
    if (!path) continue
    const file = files.find((candidate) => candidate.kind !== "bitmap" && sameName(candidate.logicalIdentity, path))
    if (!file) return Object.freeze({ code: "MissingFontFileIdentity" as const, subject: path })
    const declaredFamily = scalar(entry, "name")
    const language = firstChild(entry, context.language)
    const rangeValue = language ? scalar(language, "range") : null
    const range = parseHexRange(rangeValue)
    if (rangeValue !== null && range === null) return Object.freeze({ code: "MalformedDocument" as const, subject: path })
    const families = declaredFamily ? [declaredFamily] : file.families
    if (families.length === 0) return Object.freeze({ code: "MalformedDocument" as const, subject: path })
    for (const family of families) result.push(Object.freeze({ family, logicalIdentity: path, file, range }))
  }
  return Object.freeze(result)
}

function bitmapFiles(root: MutableNode, files: readonly VguiFontFileIdentity[]): ReadonlyMap<string, VguiFontFileIdentity> {
  const result = new Map<string, VguiFontFileIdentity>()
  const section = firstChild(root, "BitmapFontFiles")
  if (!section) return result
  for (const entry of section.children) {
    if (!entry.value) continue
    const file = files.find((candidate) => candidate.kind === "bitmap" && sameName(candidate.logicalIdentity, entry.value!))
    if (file) result.set(asciiFold(entry.name), file)
  }
  return result
}

function sourceForFamily(
  family: string,
  custom: readonly CustomFont[],
  context: VguiSchemeFontContext,
  localFonts: readonly VguiLocalFontIdentity[],
): VguiFontSourceRequest | VguiSchemeFontDiagnostic {
  const matched = custom.find((font) => sameName(font.family, family))
  if (matched && !matched.file) return Object.freeze({ code: "MissingFontFileIdentity" as const, subject: matched.logicalIdentity })
  return matched ? fileSource(matched.file!, family) : localSource(family, context, localFonts)
}

function face(
  identity: string,
  minimum: number,
  maximum: number,
  sources: readonly VguiFontSourceRequest[],
): VguiFontFaceRequest {
  return Object.freeze({
    identity,
    unicodeRange: Object.freeze([minimum, maximum]) as readonly [number, number],
    sources: uniqueSources(sources),
  })
}

function resolveFaces(
  identity: string,
  family: string,
  bitmap: boolean,
  context: VguiSchemeFontContext,
  custom: readonly CustomFont[],
  bitmaps: ReadonlyMap<string, VguiFontFileIdentity>,
  localFonts: readonly VguiLocalFontIdentity[],
): readonly VguiFontFaceRequest[] | VguiSchemeFontDiagnostic {
  if (bitmap) {
    const file = bitmaps.get(asciiFold(family))
    if (!file) return Object.freeze({ code: "MissingBitmapFile" as const, subject: family })
    return Object.freeze([face(`${identity}:bitmap`, 0, 0x00ff, [fileSource(file, family)])])
  }

  const selectedCustom = custom.find((font) => sameName(font.family, family))
  const primary = sourceForFamily(family, custom, context, localFonts)
  if ("code" in primary) return primary
  const policy = platformFontPolicy(context.platform)
  const foreign = sourceForFamily(policy.foreign, custom, context, localFonts)
  if ("code" in foreign) return foreign
  if (sameName(family, policy.foreign)) {
    return Object.freeze([face(`${identity}:bmp`, 0, 0xffff, [primary])])
  }
  if (policy.completeFamilies.some((name) => sameName(name, family))) {
    const fallback = sourceForFamily(policy.ultimateFallback, custom, context, localFonts)
    if ("code" in fallback) return fallback
    if (sameName(policy.ultimateFallback, policy.foreign)) {
      return Object.freeze([face(`${identity}:bmp`, 0, 0xffff, [primary, fallback])])
    }
    return Object.freeze([
      face(`${identity}:primary`, 0, 0x00ff, [primary, fallback, foreign]),
      face(`${identity}:after`, 0x0100, 0xffff, [primary, foreign]),
    ])
  }

  const [minimum, maximum] = selectedCustom?.range ?? [0, 0x00ff]
  const faces: VguiFontFaceRequest[] = []
  if (minimum > 0) faces.push(face(`${identity}:before`, 0, minimum - 1, [foreign]))
  faces.push(face(`${identity}:primary`, minimum, maximum, [primary, foreign]))
  if (maximum < 0xffff) faces.push(face(`${identity}:after`, maximum + 1, 0xffff, [foreign]))
  return Object.freeze(faces)
}

function diagnostic(code: VguiSchemeFontDiagnosticCode, subject: string): VguiSchemeFontResolution {
  return Object.freeze({ ok: false as const, diagnostic: Object.freeze({ code, subject }) })
}

export function resolveVguiSchemeFonts(request: VguiSchemeFontResolutionRequest): VguiSchemeFontResolution {
  if (
    !request
    || !IDENTITY.test(request.schemeLogicalIdentity)
    || !Array.isArray(request.documents)
    || request.documents.length === 0
    || !Array.isArray(request.fontFiles)
    || !Array.isArray(request.localFonts)
    || !Array.isArray(request.lookups)
    || request.lookups.length === 0
    || !validContext(request.context)
  ) return diagnostic("InvalidInput", request?.schemeLogicalIdentity ?? "invalid")

  const documents = new Map<string, VguiSchemeDocument>()
  for (const document of request.documents) {
    if (
      !document
      || !IDENTITY.test(document.logicalIdentity)
      || !SHA256.test(document.sha256)
      || !Array.isArray(document.baseLogicalIdentities)
      || document.baseLogicalIdentities.some((identity: string) => !IDENTITY.test(identity))
      || !validNode(document.root, new Set())
    ) return diagnostic("MalformedDocument", document?.logicalIdentity ?? "invalid")
    const key = asciiFold(document.logicalIdentity)
    if (documents.has(key)) return diagnostic("DuplicateDocument", document.logicalIdentity)
    documents.set(key, document)
  }
  for (const file of request.fontFiles) {
    if (
      !file
      || !["content", "external", "bitmap"].includes(file.kind)
      || !IDENTITY.test(file.logicalIdentity)
      || !SHA256.test(file.sha256)
      || !Number.isSafeInteger(file.byteLength)
      || file.byteLength <= 0
      || typeof file.version !== "string"
      || file.version.length === 0
      || !Array.isArray(file.families)
      || file.families.some((family: string) => typeof family !== "string" || family.length === 0)
    ) return diagnostic("InvalidInput", file?.logicalIdentity ?? "font-file")
  }
  const lookupIdentities = new Set<string>()
  for (const lookup of request.lookups) {
    if (
      !lookup
      || !IDENTITY.test(lookup.identity)
      || lookupIdentities.has(lookup.identity)
      || typeof lookup.name !== "string"
      || lookup.name.length === 0
      || typeof lookup.proportional !== "boolean"
    ) return diagnostic("InvalidInput", lookup?.identity ?? "font-lookup")
    lookupIdentities.add(lookup.identity)
  }

  const usedDocuments: string[] = []
  const compose = (identity: string, stack: readonly string[]): MutableNode | VguiSchemeFontDiagnostic => {
    const key = asciiFold(identity)
    if (stack.includes(key)) return Object.freeze({ code: "CyclicBase" as const, subject: identity })
    const document = documents.get(key)
    if (!document) return Object.freeze({ code: "MissingDocument" as const, subject: identity })
    const filtered = filterNode(document.root, request.context)
    if (!filtered) return Object.freeze({ code: "MalformedDocument" as const, subject: identity })
    usedDocuments.push(document.logicalIdentity)
    for (const baseIdentity of document.baseLogicalIdentities) {
      const base = compose(baseIdentity, [...stack, key])
      if ("code" in base) return base
      mergeBase(filtered, base)
    }
    return filtered
  }

  const composed = compose(request.schemeLogicalIdentity, [])
  if ("code" in composed) return diagnostic(composed.code, composed.subject)
  if (!sameName(composed.name, "Scheme")) return diagnostic("MalformedDocument", request.schemeLogicalIdentity)
  if (request.context.minMode) applyResolutionSuffix(composed, "_minmode")

  const fontsSection = firstChild(composed, "Fonts")
  if (!fontsSection) return diagnostic("MissingFont", "Fonts")
  const custom = customFonts(composed, request.context, request.fontFiles)
  if ("code" in custom) return diagnostic(custom.code, custom.subject)
  const bitmaps = bitmapFiles(composed, request.fontFiles)
  const resolved: VguiResolvedFontRequest[] = []
  for (const lookup of request.lookups) {
    const font = firstChild(fontsSection, lookup.name)
    if (!font) return diagnostic("MissingFont", lookup.name)
    let selected: MutableNode | null = null
    let selectedYRes: readonly [number, number] = [0, 0]
    for (const candidate of font.children) {
      const yres = parseYResolution(scalar(candidate, "yres"))
      if (yres[0] !== 0 && (request.context.viewportHeight < yres[0] || request.context.viewportHeight > yres[1])) continue
      selected = candidate
      selectedYRes = yres
      break
    }
    if (!selected) return diagnostic("MissingCandidate", lookup.name)
    const family = scalar(selected, "name")
    let height = sourceInteger(scalar(selected, "tall"))
    let blur = sourceInteger(scalar(selected, "blur"))
    let scanlines = sourceInteger(scalar(selected, "scanlines"))
    let scaleX = sourceFloat(scalar(selected, "scalex"), 1)
    let scaleY = sourceFloat(scalar(selected, "scaley"), 1)
    if (selectedYRes[0] === 0 && selectedYRes[1] === 0 && lookup.proportional) {
      height = scaled(height, request.context.viewportHeight)
      blur = scaled(blur, request.context.viewportHeight)
      scanlines = scaled(scanlines, request.context.viewportHeight)
      scaleX = scaled(scaleX * 10_000, request.context.viewportHeight) * 0.0001
      scaleY = scaled(scaleY * 10_000, request.context.viewportHeight) * 0.0001
    }
    height = Math.max(languageMinimum(request.context.language), Math.min(127, height))
    const weight = sourceInteger(scalar(selected, "weight"))
    if (
      !family
      || height <= 0
      || !Number.isSafeInteger(weight)
      || blur < 0
      || scanlines < 0
      || !Number.isFinite(scaleX)
      || !Number.isFinite(scaleY)
      || scaleX <= 0
      || scaleY <= 0
    ) return diagnostic("MalformedCandidate", lookup.name)

    const requested = (name: string) => sourceInteger(scalar(selected!, name)) !== 0
    const effects: VguiFontEffects = Object.freeze({
      italic: requested("italic"),
      underline: requested("underline"),
      strikeout: requested("strikeout"),
      symbol: requested("symbol"),
      antialias: requested("antialias") && request.context.surfaceFeatures.antialias,
      blur,
      scanlines,
      dropShadow: requested("dropshadow") && request.context.surfaceFeatures.dropShadow,
      outline: requested("outline") && request.context.surfaceFeatures.outline,
      additive: requested("additive"),
      custom: requested("custom"),
      bitmap: requested("bitmap"),
      rotary: requested("rotary"),
    })
    const faces = resolveFaces(
      lookup.identity,
      family,
      effects.bitmap,
      request.context,
      custom,
      bitmaps,
      request.localFonts,
    )
    if ("code" in faces) return diagnostic(faces.code, faces.subject)
    resolved.push(Object.freeze({
      identity: lookup.identity,
      schemeFontName: lookup.name,
      proportional: lookup.proportional,
      family,
      candidateName: selected.name,
      requestedHeight: height,
      weight,
      bitmapScale: Object.freeze([scaleX, scaleY]) as readonly [number, number],
      effects,
      faces,
    }))
  }

  const scheme = documents.get(asciiFold(request.schemeLogicalIdentity))!
  return Object.freeze({
    ok: true as const,
    schemeLogicalIdentity: scheme.logicalIdentity,
    schemeSha256: scheme.sha256,
    documentLogicalIdentities: Object.freeze([...usedDocuments]),
    fonts: Object.freeze(resolved),
  })
}
