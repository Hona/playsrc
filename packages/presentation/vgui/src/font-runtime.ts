import type {
  VguiFontEffects,
  VguiFontFaceRequest,
  VguiFontSourceRequest,
  VguiResolvedFontRequest,
} from "./scheme-fonts"

export type VguiFontByteSupply = Readonly<{
  kind: "content" | "external" | "bitmap"
  logicalIdentity: string
  sha256: string
  byteLength: number
  version: string
  bytes: Uint8Array
}>

export type VguiGlyphMetric = Readonly<{
  codeUnit: number
  faceIdentity: string
  abc: readonly [a: number, b: number, c: number]
}>

export type VguiKerningMetric = Readonly<{
  codeUnit: number
  before: number
  after: number
  width: number
  abcA: number
}>

export type VguiExactFontMetrics = Readonly<{
  identity: string
  sha256: string
  version: string
  requestedHeight: number
  height: number
  ascent: number
  maxCharWidth: number
  glyphs: readonly VguiGlyphMetric[]
  kerning: readonly VguiKerningMetric[]
}>

export type VguiExactRasterProfile = Readonly<{
  identity: string
  sha256: string
  version: string
  metricsIdentity: string
  pixelFormat: "rgba8"
  antialiasMode: "none" | "grayscale" | "platform"
}>

export type VguiSuppliedFontProfile = Readonly<{
  fontIdentity: string
  metrics: VguiExactFontMetrics
  raster: VguiExactRasterProfile
}>

export type VguiFontRequirement = Readonly<{
  kind: "required"
  reasons: readonly (
    | "exact-metrics-unavailable"
    | "native-raster-profile-unavailable"
    | "browser-shaping-differs"
    | "local-version-unverifiable"
    | "non-antialiased-mode-unavailable"
    | "bitmap-adapter-required"
  )[]
}>

export type VguiMountedFontCapability = Readonly<{
  identity: string
  schemeFontName: string
  browserFamily: string
  requestedHeight: number
  weight: number
  effects: VguiFontEffects
  faces: readonly Readonly<{
    identity: string
    unicodeRange: readonly [number, number]
    selectedSource: VguiFontSourceRequest
  }>[]
  metrics: Readonly<{ kind: "exact"; profile: VguiExactFontMetrics }> | VguiFontRequirement
  raster: Readonly<{ kind: "exact"; profile: VguiExactRasterProfile }> | VguiFontRequirement
  shaping: "source-glyph-stream" | "browser-native-unverified"
}>

export type VguiFontSetCapability =
  | Readonly<{
      kind: "supported"
      targetIdentity: string
      fonts: readonly VguiMountedFontCapability[]
    }>
  | Readonly<{
      kind: "unsupported"
      targetIdentity: string
      reason:
        | "invalid-target"
        | "digest-api-unavailable"
        | "font-loading-api-unavailable"
        | "font-source-unavailable"
        | "font-set-publication-failed"
      fontIdentity: string | null
      faceIdentity: string | null
      attemptedSourceIdentities: readonly string[]
    }>

export type VguiFontSetMountSnapshot = Readonly<{
  lifecycle: "mounted" | "destroyed"
  publishedFaces: number
  capability: VguiFontSetCapability
}>

export type VguiFontSetMount = Readonly<{
  snapshot(): VguiFontSetMountSnapshot
  destroy(): void
}>

export type VguiFontSetMountResult =
  | Readonly<{ ok: true; fontSet: VguiFontSetMount }>
  | Readonly<{ ok: false; capability: Extract<VguiFontSetCapability, { kind: "unsupported" }> }>

export type VguiFontFaceLoad = Readonly<{
  targetIdentity: string
  font: VguiResolvedFontRequest
  face: VguiFontFaceRequest
  source: VguiFontSourceRequest
  browserFamily: string
  bytes: Uint8Array | null
}>

export type VguiFontMountAdapter = Readonly<{
  loadFace(request: VguiFontFaceLoad): Promise<unknown>
  publishFace(face: unknown): void
  removeFace(face: unknown): void
}>

export type VguiFontSetMountRequest = Readonly<{
  identity: string
  fonts: readonly VguiResolvedFontRequest[]
  byteSupplies: readonly VguiFontByteSupply[]
  profiles: readonly VguiSuppliedFontProfile[]
}>

export type VguiCharacterMetrics = Readonly<{
  faceIdentity: string | null
  a: number
  b: number
  c: number
  width: number
}>

export type VguiTextMetrics = Readonly<{
  width: number
  height: number
  lineCount: number
}>

export type VguiGlyphBitmap = Readonly<{
  width: number
  height: number
  rgba: Uint8Array
}>

const IDENTITY = /^[a-z0-9][a-z0-9./_-]{0,511}$/u
const SHA256 = /^[a-f0-9]{64}$/u

function unsupported(
  targetIdentity: string,
  reason: Extract<VguiFontSetCapability, { kind: "unsupported" }>['reason'],
  fontIdentity: string | null = null,
  faceIdentity: string | null = null,
  attemptedSourceIdentities: readonly string[] = [],
): Extract<VguiFontSetCapability, { kind: "unsupported" }> {
  return Object.freeze({
    kind: "unsupported" as const,
    targetIdentity,
    reason,
    fontIdentity,
    faceIdentity,
    attemptedSourceIdentities: Object.freeze([...attemptedSourceIdentities]),
  })
}

function cssString(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`
}

function cssUnicodeRange([minimum, maximum]: readonly [number, number]): string {
  return `U+${minimum.toString(16).padStart(4, "0")}-${maximum.toString(16).padStart(4, "0")}`
}

function copiedArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength)
  new Uint8Array(buffer).set(bytes)
  return buffer
}

function browserPlatform(): "windows" | "macos" | "linux" | "other" {
  if (typeof navigator === "undefined") return "other"
  const reported = ((navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform
    ?? navigator.platform
    ?? "").trim().toLowerCase()
  if (reported.startsWith("win")) return "windows"
  if (reported.startsWith("mac")) return "macos"
  if (reported.startsWith("linux") && !reported.includes("android")) return "linux"
  return "other"
}

function defaultBrowserAdapter(): VguiFontMountAdapter | null {
  if (typeof FontFace !== "function" || typeof document === "undefined" || !document.fonts) return null
  return Object.freeze({
    async loadFace(request: VguiFontFaceLoad): Promise<FontFace> {
      if (request.source.kind === "bitmap") throw new Error("bitmap adapter required")
      if (request.source.kind === "local" && request.source.platform !== browserPlatform()) throw new Error("local face platform mismatch")
      if (request.font.weight < 0 || request.font.weight > 1000) throw new Error("browser weight unsupported")
      const source = request.source.kind === "local"
        ? `local(${cssString(request.source.faceName)})`
        : copiedArrayBuffer(request.bytes!)
      const face = new FontFace(request.browserFamily, source, {
        style: request.font.effects.italic ? "italic" : "normal",
        weight: request.font.weight === 0 ? "normal" : String(request.font.weight),
        unicodeRange: cssUnicodeRange(request.face.unicodeRange),
      })
      const loaded = await face.load()
      if (loaded.status !== "loaded") throw new Error("font face did not load")
      return loaded
    },
    publishFace(face: unknown) { document.fonts.add(face as FontFace) },
    removeFace(face: unknown) { document.fonts.delete(face as FontFace) },
  })
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const copy = bytes.slice()
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", copy))
  return [...digest].map((value) => value.toString(16).padStart(2, "0")).join("")
}

async function browserFamily(targetIdentity: string, fontIdentity: string): Promise<string> {
  const digest = await sha256(new TextEncoder().encode(`${targetIdentity}\u0000${fontIdentity}`))
  return `playsrc-vgui-${digest.slice(0, 24)}`
}

function validSource(source: VguiFontSourceRequest): boolean {
  if (!source || typeof source.identity !== "string" || source.identity.length === 0) return false
  if (source.kind === "local") {
    return ["windows", "macos", "linux"].includes(source.platform)
      && typeof source.faceName === "string"
      && source.faceName.length > 0
      && (source.version === null || (typeof source.version === "string" && source.version.length > 0))
  }
  return ["content", "external", "bitmap"].includes(source.kind)
    && IDENTITY.test(source.logicalIdentity)
    && SHA256.test(source.sha256)
    && Number.isSafeInteger(source.byteLength)
    && source.byteLength > 0
    && typeof source.version === "string"
    && source.version.length > 0
    && typeof source.family === "string"
    && source.family.length > 0
}

function validFont(font: VguiResolvedFontRequest): boolean {
  if (
    !font
    || !IDENTITY.test(font.identity)
    || typeof font.schemeFontName !== "string"
    || font.schemeFontName.length === 0
    || !Number.isSafeInteger(font.requestedHeight)
    || font.requestedHeight <= 0
    || font.requestedHeight > 127
    || !Number.isSafeInteger(font.weight)
    || typeof font.family !== "string"
    || font.family.length === 0
    || typeof font.candidateName !== "string"
    || font.candidateName.length === 0
    || typeof font.proportional !== "boolean"
    || !Array.isArray(font.bitmapScale)
    || font.bitmapScale.length !== 2
    || font.bitmapScale.some((value: number) => !Number.isFinite(value) || value <= 0)
    || !font.effects
    || !Number.isSafeInteger(font.effects.blur)
    || font.effects.blur < 0
    || !Number.isSafeInteger(font.effects.scanlines)
    || font.effects.scanlines < 0
    || [
      font.effects.italic,
      font.effects.underline,
      font.effects.strikeout,
      font.effects.symbol,
      font.effects.antialias,
      font.effects.dropShadow,
      font.effects.outline,
      font.effects.additive,
      font.effects.custom,
      font.effects.bitmap,
      font.effects.rotary,
    ].some((value) => typeof value !== "boolean")
    || !Array.isArray(font.faces)
    || font.faces.length === 0
  ) return false
  const faceIdentities = new Set<string>()
  const ranges: Array<readonly [number, number]> = []
  for (const face of font.faces) {
    if (
      !face
      || typeof face.identity !== "string"
      || face.identity.length === 0
      || faceIdentities.has(face.identity)
      || !Array.isArray(face.unicodeRange)
      || face.unicodeRange.length !== 2
      || !Number.isSafeInteger(face.unicodeRange[0])
      || !Number.isSafeInteger(face.unicodeRange[1])
      || face.unicodeRange[0] < 0
      || face.unicodeRange[0] > face.unicodeRange[1]
      || face.unicodeRange[1] > 0xffff
      || ranges.some(([minimum, maximum]) => face.unicodeRange[0] <= maximum && face.unicodeRange[1] >= minimum)
      || !Array.isArray(face.sources)
      || face.sources.length === 0
      || face.sources.some((source: VguiFontSourceRequest) => !validSource(source))
    ) return false
    faceIdentities.add(face.identity)
    ranges.push(face.unicodeRange)
  }
  return true
}

function validMetrics(profile: VguiExactFontMetrics, font: VguiResolvedFontRequest): boolean {
  if (
    !profile
    || !IDENTITY.test(profile.identity)
    || !SHA256.test(profile.sha256)
    || typeof profile.version !== "string"
    || profile.version.length === 0
    || profile.requestedHeight !== font.requestedHeight
    || !Number.isFinite(profile.height)
    || profile.height <= 0
    || !Number.isFinite(profile.ascent)
    || profile.ascent < 0
    || profile.ascent > profile.height
    || !Number.isFinite(profile.maxCharWidth)
    || profile.maxCharWidth < 0
    || !Array.isArray(profile.glyphs)
    || !Array.isArray(profile.kerning)
  ) return false
  const faceByIdentity = new Map(font.faces.map((face) => [face.identity, face] as const))
  const glyphs = new Set<number>()
  for (const glyph of profile.glyphs) {
    const glyphFace = glyph ? faceByIdentity.get(glyph.faceIdentity) : null
    if (
      !glyph
      || !Number.isSafeInteger(glyph.codeUnit)
      || glyph.codeUnit < 0
      || glyph.codeUnit > 0xffff
      || glyphs.has(glyph.codeUnit)
      || !glyphFace
      || glyph.codeUnit < glyphFace.unicodeRange[0]
      || glyph.codeUnit > glyphFace.unicodeRange[1]
      || !Array.isArray(glyph.abc)
      || glyph.abc.length !== 3
      || glyph.abc.some((value: number) => !Number.isFinite(value))
    ) return false
    glyphs.add(glyph.codeUnit)
  }
  const kernings = new Set<string>()
  for (const kern of profile.kerning) {
    if (
      !kern
      || [kern.codeUnit, kern.before, kern.after].some((value: number) => !Number.isSafeInteger(value) || value < 0 || value > 0xffff)
      || !Number.isFinite(kern.width)
      || !Number.isFinite(kern.abcA)
      || !glyphs.has(kern.codeUnit)
    ) return false
    const key = `${kern.codeUnit}:${kern.before}:${kern.after}`
    if (kernings.has(key)) return false
    kernings.add(key)
  }
  return true
}

function validProfile(profile: VguiSuppliedFontProfile, font: VguiResolvedFontRequest): boolean {
  return !!profile
    && profile.fontIdentity === font.identity
    && validMetrics(profile.metrics, font)
    && !!profile.raster
    && IDENTITY.test(profile.raster.identity)
    && SHA256.test(profile.raster.sha256)
    && typeof profile.raster.version === "string"
    && profile.raster.version.length > 0
    && profile.raster.metricsIdentity === profile.metrics.identity
    && profile.raster.pixelFormat === "rgba8"
    && ["none", "grayscale", "platform"].includes(profile.raster.antialiasMode)
    && (font.effects.antialias || profile.raster.antialiasMode === "none")
}

function sourceRequirement(font: VguiResolvedFontRequest, sources: readonly VguiFontSourceRequest[]): VguiFontRequirement {
  const reasons: VguiFontRequirement["reasons"][number][] = [
    "exact-metrics-unavailable",
    "native-raster-profile-unavailable",
    "browser-shaping-differs",
  ]
  if (!font.effects.antialias) reasons.push("non-antialiased-mode-unavailable")
  if (sources.some((source) => source.kind === "local" && source.version === null)) reasons.push("local-version-unverifiable")
  if (sources.some((source) => source.kind === "bitmap")) reasons.push("bitmap-adapter-required")
  return Object.freeze({ kind: "required" as const, reasons: Object.freeze(reasons) })
}

function findSupply(source: Exclude<VguiFontSourceRequest, { kind: "local" }>, supplies: readonly VguiFontByteSupply[]): VguiFontByteSupply | null {
  return supplies.find((supply) =>
    supply.kind === source.kind
    && supply.logicalIdentity === source.logicalIdentity
    && supply.sha256 === source.sha256
    && supply.byteLength === source.byteLength
    && supply.version === source.version
  ) ?? null
}

function cloneFont(font: VguiResolvedFontRequest): VguiResolvedFontRequest {
  return Object.freeze({
    ...font,
    bitmapScale: Object.freeze([...font.bitmapScale]) as readonly [number, number],
    effects: Object.freeze({ ...font.effects }),
    faces: Object.freeze(font.faces.map((face) => Object.freeze({
      ...face,
      unicodeRange: Object.freeze([...face.unicodeRange]) as readonly [number, number],
      sources: Object.freeze(face.sources.map((source) => Object.freeze({ ...source }))),
    }))),
  })
}

function cloneProfile(profile: VguiSuppliedFontProfile): VguiSuppliedFontProfile {
  return Object.freeze({
    fontIdentity: profile.fontIdentity,
    metrics: Object.freeze({
      ...profile.metrics,
      glyphs: Object.freeze(profile.metrics.glyphs.map((glyph: VguiGlyphMetric) => Object.freeze({
        ...glyph,
        abc: Object.freeze([...glyph.abc]) as readonly [number, number, number],
      }))),
      kerning: Object.freeze(profile.metrics.kerning.map((kern: VguiKerningMetric) => Object.freeze({ ...kern }))),
    }),
    raster: Object.freeze({ ...profile.raster }),
  })
}

export async function mountVguiFontSet(
  request: VguiFontSetMountRequest,
  injectedAdapter?: VguiFontMountAdapter,
): Promise<VguiFontSetMountResult> {
  if (
    !request
    || !IDENTITY.test(request.identity)
    || !Array.isArray(request.fonts)
    || request.fonts.length === 0
    || request.fonts.some((font) => !validFont(font))
    || new Set(request.fonts.map((font) => font.identity)).size !== request.fonts.length
    || !Array.isArray(request.byteSupplies)
    || !Array.isArray(request.profiles)
  ) return Object.freeze({ ok: false as const, capability: unsupported(request?.identity ?? "invalid", "invalid-target") })

  if (typeof crypto === "undefined" || !crypto.subtle) {
    return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "digest-api-unavailable") })
  }

  const fonts = request.fonts.map(cloneFont)
  const profileByFont = new Map<string, VguiSuppliedFontProfile>()
  for (const profile of request.profiles) {
    const font = fonts.find((candidate) => candidate.identity === profile.fontIdentity)
    if (!font || profileByFont.has(profile.fontIdentity) || !validProfile(profile, font)) {
      return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "invalid-target") })
    }
    profileByFont.set(profile.fontIdentity, cloneProfile(profile))
  }
  if (request.byteSupplies.some((supply) => !supply || !(supply.bytes instanceof Uint8Array))) {
    return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "invalid-target") })
  }
  const supplies = request.byteSupplies.map((supply) => Object.freeze({ ...supply, bytes: supply.bytes.slice() }))
  for (const supply of supplies) {
    if (
      !["content", "external", "bitmap"].includes(supply.kind)
      || !IDENTITY.test(supply.logicalIdentity)
      || !SHA256.test(supply.sha256)
      || !Number.isSafeInteger(supply.byteLength)
      || supply.byteLength <= 0
      || supply.bytes.byteLength !== supply.byteLength
      || typeof supply.version !== "string"
      || supply.version.length === 0
      || await sha256(supply.bytes) !== supply.sha256
    ) return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "invalid-target") })
  }
  if (new Set(supplies.map((supply) => `${supply.kind}:${supply.logicalIdentity}`)).size !== supplies.length) {
    return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "invalid-target") })
  }
  const adapter = injectedAdapter ?? defaultBrowserAdapter()
  if (!adapter) {
    return Object.freeze({ ok: false as const, capability: unsupported(request.identity, "font-loading-api-unavailable") })
  }

  const loaded: Array<Readonly<{
    font: VguiResolvedFontRequest
    face: VguiFontFaceRequest
    source: VguiFontSourceRequest
    browserFamily: string
    handle: unknown
  }>> = []
  for (const font of fonts) {
    const family = await browserFamily(request.identity, font.identity)
    for (const face of font.faces) {
      const attempted: string[] = []
      let admitted: typeof loaded[number] | null = null
      for (const source of face.sources) {
        attempted.push(source.identity)
        let bytes: Uint8Array | null = null
        if (source.kind !== "local") {
          const supply = findSupply(source, supplies)
          if (!supply) continue
          bytes = supply.bytes.slice()
        }
        try {
          const handle = await adapter.loadFace(Object.freeze({
            targetIdentity: request.identity,
            font,
            face,
            source,
            browserFamily: family,
            bytes,
          }))
          admitted = Object.freeze({ font, face, source, browserFamily: family, handle })
          break
        } catch {}
      }
      if (!admitted) {
        for (const entry of [...loaded].reverse()) {
          try { adapter.removeFace(entry.handle) } catch {}
        }
        return Object.freeze({
          ok: false as const,
          capability: unsupported(request.identity, "font-source-unavailable", font.identity, face.identity, attempted),
        })
      }
      loaded.push(admitted)
    }
  }

  const published: typeof loaded = []
  try {
    for (const entry of loaded) {
      adapter.publishFace(entry.handle)
      published.push(entry)
    }
  } catch {
    for (const entry of [...loaded].reverse()) {
      try { adapter.removeFace(entry.handle) } catch {}
    }
    return Object.freeze({
      ok: false as const,
      capability: unsupported(request.identity, "font-set-publication-failed"),
    })
  }

  const capabilities = fonts.map((font): VguiMountedFontCapability => {
    const fontFaces = loaded.filter((entry) => entry.font.identity === font.identity)
    const profile = profileByFont.get(font.identity)
    const requirement = sourceRequirement(font, fontFaces.map((face) => face.source))
    return Object.freeze({
      identity: font.identity,
      schemeFontName: font.schemeFontName,
      browserFamily: fontFaces[0].browserFamily,
      requestedHeight: font.requestedHeight,
      weight: font.weight,
      effects: font.effects,
      faces: Object.freeze(fontFaces.map((entry) => Object.freeze({
        identity: entry.face.identity,
        unicodeRange: Object.freeze([...entry.face.unicodeRange]) as readonly [number, number],
        selectedSource: entry.source,
      }))),
      metrics: profile
        ? Object.freeze({ kind: "exact" as const, profile: profile.metrics })
        : requirement,
      raster: profile
        ? Object.freeze({ kind: "exact" as const, profile: profile.raster })
        : requirement,
      shaping: profile ? "source-glyph-stream" as const : "browser-native-unverified" as const,
    })
  })
  const capability: VguiFontSetCapability = Object.freeze({
    kind: "supported" as const,
    targetIdentity: request.identity,
    fonts: Object.freeze(capabilities),
  })
  let destroyed = false
  const fontSet: VguiFontSetMount = Object.freeze({
    snapshot(): VguiFontSetMountSnapshot {
      return Object.freeze({
        lifecycle: destroyed ? "destroyed" : "mounted",
        publishedFaces: destroyed ? 0 : published.length,
        capability,
      })
    },
    destroy(): void {
      if (destroyed) return
      destroyed = true
      for (const entry of [...published].reverse()) {
        try { adapter.removeFace(entry.handle) } catch {}
      }
      published.length = 0
    },
  })
  return Object.freeze({ ok: true as const, fontSet })
}

function glyphFor(profile: VguiExactFontMetrics, codeUnit: number): VguiGlyphMetric | null {
  return profile.glyphs.find((glyph) => glyph.codeUnit === codeUnit) ?? null
}

export function getVguiCharacterMetrics(profile: VguiExactFontMetrics, codeUnit: number): VguiCharacterMetrics {
  if (!Number.isSafeInteger(codeUnit) || codeUnit < 0 || codeUnit > 0xffff) {
    return Object.freeze({ faceIdentity: null, a: 0, b: 0, c: 0, width: 0 })
  }
  const glyph = glyphFor(profile, codeUnit)
  if (!glyph) {
    return Object.freeze({ faceIdentity: null, a: 0, b: profile.maxCharWidth, c: 0, width: profile.maxCharWidth })
  }
  const [a, b, c] = glyph.abc
  return Object.freeze({ faceIdentity: glyph.faceIdentity, a, b, c, width: a + b + c })
}

export function measureVguiText(profile: VguiExactFontMetrics, text: string): VguiTextMetrics {
  if (typeof text !== "string") return Object.freeze({ width: 0, height: 0, lineCount: 0 })
  let maximumWidth = 0
  let currentWidth = 0
  let lineCount = 1
  let before = 0
  for (let index = 0; index < text.length; index += 1) {
    const codeUnit = text.charCodeAt(index)
    const after = index + 1 < text.length ? text.charCodeAt(index + 1) : 0
    if (codeUnit === 0x0a) {
      lineCount += 1
      currentWidth = 0
    } else if (codeUnit !== 0x26) {
      const glyph = glyphFor(profile, codeUnit)
      const beforeGlyph = glyphFor(profile, before)
      const afterGlyph = glyphFor(profile, after)
      const kernBefore = glyph && beforeGlyph?.faceIdentity === glyph.faceIdentity ? before : 0
      const kernAfter = glyph && afterGlyph?.faceIdentity === glyph.faceIdentity ? after : 0
      const kern = profile.kerning.find((entry) =>
        entry.codeUnit === codeUnit && entry.before === kernBefore && entry.after === kernAfter
      )
      currentWidth += kern?.width ?? getVguiCharacterMetrics(profile, codeUnit).width
      maximumWidth = Math.max(maximumWidth, currentWidth)
    }
    before = codeUnit
  }
  return Object.freeze({ width: Math.ceil(maximumWidth), height: lineCount * profile.height, lineCount })
}

function pixelOffset(width: number, x: number, y: number): number {
  return (y * width + x) * 4
}

function applyShadow(pixels: Uint8Array, width: number, height: number): void {
  const source = pixels.slice()
  for (let y = 1; y < height; y += 1) {
    for (let x = 1; x < width; x += 1) {
      const destinationOffset = pixelOffset(width, x, y)
      if (source[destinationOffset + 3] !== 0) continue
      const sourceOffset = pixelOffset(width, x - 1, y - 1)
      pixels[destinationOffset] = 0
      pixels[destinationOffset + 1] = 0
      pixels[destinationOffset + 2] = 0
      pixels[destinationOffset + 3] = source[sourceOffset + 3]
    }
  }
}

function applyOutline(pixels: Uint8Array, width: number, height: number): void {
  const source = pixels.slice()
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = pixelOffset(width, x, y)
      if (source[offset + 3] !== 0) continue
      let bordered = false
      for (let dy = -1; dy <= 1 && !bordered; dy += 1) {
        for (let dx = -1; dx <= 1; dx += 1) {
          if (dx === 0 && dy === 0) continue
          const testX = x + dx
          const testY = y + dy
          if (testX < 0 || testX >= width || testY < 0 || testY >= height) continue
          const test = pixelOffset(width, testX, testY)
          if (source[test] !== 0 && source[test + 1] !== 0 && source[test + 2] !== 0 && source[test + 3] !== 0) {
            bordered = true
            break
          }
        }
      }
      if (bordered) {
        pixels[offset] = 0
        pixels[offset + 1] = 0
        pixels[offset + 2] = 0
        pixels[offset + 3] = 255
      }
    }
  }
}

function blurKernel(radius: number): readonly number[] {
  const sigma = 0.683 * radius
  return Object.freeze(Array.from({ length: radius * 2 + 1 }, (_, index) => {
    const distance = index - radius
    return 1 / Math.sqrt(2 * 3.14 * sigma * sigma) * Math.pow(2.7, -(distance * distance) / (2 * sigma * sigma))
  }))
}

function applyBlur(pixels: Uint8Array, width: number, height: number, radius: number): void {
  if (radius === 0) return
  const kernel = blurKernel(radius)
  const horizontal = pixels.slice()
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = Math.max(0, radius - x)
      const sourceStart = x + offset - radius
      let count = radius * 2 + 1 - offset
      if (x >= width - radius) count = width - (x - offset)
      let alpha = 0
      for (let sample = 0; sample < count; sample += 1) {
        alpha += horizontal[pixelOffset(width, sourceStart + sample, y) + 3] * kernel[offset + sample]
      }
      const pixel = pixelOffset(width, x, y)
      const channel = Math.trunc(alpha)
      pixels[pixel] = channel > 0 ? 255 : 0
      pixels[pixel + 1] = channel > 0 ? 255 : 0
      pixels[pixel + 2] = channel > 0 ? 255 : 0
      pixels[pixel + 3] = channel
    }
  }
  const vertical = pixels.slice()
  for (let y = 0; y < height; y += 1) {
    const offset = Math.max(0, radius - y)
    const sourceStart = y + offset - radius
    let count = radius * 2 + 1 - offset
    if (y >= height - radius) count = height - (y - offset)
    for (let x = 0; x < width; x += 1) {
      let alpha = 0
      for (let sample = 0; sample < count; sample += 1) {
        alpha += vertical[pixelOffset(width, x, sourceStart + sample) + 3] * kernel[offset + sample]
      }
      const pixel = pixelOffset(width, x, y)
      const channel = Math.trunc(alpha)
      pixels[pixel] = channel > 0 ? 255 : 0
      pixels[pixel + 1] = channel > 0 ? 255 : 0
      pixels[pixel + 2] = channel > 0 ? 255 : 0
      pixels[pixel + 3] = channel
    }
  }
}

export function applyVguiGlyphEffects(bitmap: VguiGlyphBitmap, effects: VguiFontEffects): VguiGlyphBitmap {
  if (
    !bitmap
    || !Number.isSafeInteger(bitmap.width)
    || !Number.isSafeInteger(bitmap.height)
    || bitmap.width <= 0
    || bitmap.height <= 0
    || !(bitmap.rgba instanceof Uint8Array)
    || bitmap.rgba.byteLength !== bitmap.width * bitmap.height * 4
    || !effects
    || !Number.isSafeInteger(effects.blur)
    || effects.blur < 0
    || effects.blur >= bitmap.width
    || effects.blur >= bitmap.height
    || !Number.isSafeInteger(effects.scanlines)
    || effects.scanlines < 0
  ) return Object.freeze({ width: 0, height: 0, rgba: new Uint8Array() })
  const pixels = bitmap.rgba.slice()
  if (effects.dropShadow) applyShadow(pixels, bitmap.width, bitmap.height)
  if (effects.outline) applyOutline(pixels, bitmap.width, bitmap.height)
  applyBlur(pixels, bitmap.width, bitmap.height, effects.blur)
  if (effects.scanlines >= 2) {
    for (let y = 0; y < bitmap.height; y += 1) {
      if (y % effects.scanlines === 0) continue
      for (let x = 0; x < bitmap.width; x += 1) {
        const offset = pixelOffset(bitmap.width, x, y)
        pixels[offset] = Math.trunc(pixels[offset] * 0.7)
        pixels[offset + 1] = Math.trunc(pixels[offset + 1] * 0.7)
        pixels[offset + 2] = Math.trunc(pixels[offset + 2] * 0.7)
      }
    }
  }
  if (effects.rotary) {
    const y = Math.trunc(bitmap.height * 0.5)
    for (let x = 0; x < bitmap.width; x += 1) {
      const offset = pixelOffset(bitmap.width, x, y)
      pixels[offset] = 127
      pixels[offset + 1] = 127
      pixels[offset + 2] = 127
      pixels[offset + 3] = 255
    }
  }
  return Object.freeze({ width: bitmap.width, height: bitmap.height, rgba: pixels })
}
