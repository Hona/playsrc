export type BrowserPlatform = "windows" | "macos" | "linux" | "other"

export type LocalPlatformFontFaceRequest = Readonly<{
  identity: string
  localName: string
  browserFamily: string
  weight: number
  style: "normal" | "italic"
  unicodeRange: readonly [minimum: number, maximum: number]
}>

export type LocalPlatformFontTarget = Readonly<{
  identity: string
  requiredPlatform: BrowserPlatform
  faces: readonly LocalPlatformFontFaceRequest[]
}>

export type LocalPlatformFontFace = Readonly<{
  identity: string
  localName: string
  browserFamily: string
  weight: number
  style: "normal" | "italic"
  unicodeRange: readonly [minimum: number, maximum: number]
}>

export type LocalPlatformFontCapability =
  | Readonly<{
      kind: "supported"
      targetIdentity: string
      platform: BrowserPlatform
      faces: readonly LocalPlatformFontFace[]
    }>
  | Readonly<{
      kind: "unsupported"
      targetIdentity: string
      platform: BrowserPlatform
      reason:
        | "unsupported-platform"
        | "font-loading-api-unavailable"
        | "local-face-unavailable"
        | "font-set-publication-failed"
        | "invalid-target"
      unadmittedFaceIdentities: readonly string[]
    }>

export type LocalPlatformFontAdapter = Readonly<{
  reportedPlatform: string | null
  loadLocalFace(request: LocalPlatformFontFaceRequest): Promise<unknown>
  addLoadedFace(face: unknown): void
  removeLoadedFace(face: unknown): void
}>

type NavigatorWithUserAgentData = Navigator & Readonly<{
  userAgentData?: Readonly<{ platform?: string }>
}>

function frozenUnsupported(
  targetIdentity: string,
  platform: BrowserPlatform,
  reason: Extract<LocalPlatformFontCapability, { kind: "unsupported" }>["reason"],
  unadmittedFaceIdentities: readonly string[] = [],
): LocalPlatformFontCapability {
  return Object.freeze({
    kind: "unsupported" as const,
    targetIdentity,
    platform,
    reason,
    unadmittedFaceIdentities: Object.freeze([...unadmittedFaceIdentities]),
  })
}

function validTarget(target: LocalPlatformFontTarget): boolean {
  if (
    !target
    || !/^[a-z0-9][a-z0-9./_-]{0,255}$/u.test(target.identity)
    || !["windows", "macos", "linux", "other"].includes(target.requiredPlatform)
    || !Array.isArray(target.faces)
    || target.faces.length === 0
    || target.faces.length > 64
  ) return false
  const identities = new Set<string>()
  const selections = new Set<string>()
  const ranges = new Map<string, Array<readonly [number, number]>>()
  for (const face of target.faces) {
    if (
      !face
      || !/^[a-z0-9][a-z0-9./_-]{0,255}$/u.test(face.identity)
      || identities.has(face.identity)
      || typeof face.localName !== "string"
      || face.localName.length === 0
      || face.localName.length > 255
      || /[\u0000-\u001f\u007f]/u.test(face.localName)
      || !/^[a-z][a-z0-9-]{0,127}$/u.test(face.browserFamily)
      || !Number.isSafeInteger(face.weight)
      || face.weight < 0
      || face.weight > 1000
      || (face.style !== "normal" && face.style !== "italic")
      || !Array.isArray(face.unicodeRange)
      || face.unicodeRange.length !== 2
      || !Number.isSafeInteger(face.unicodeRange[0])
      || !Number.isSafeInteger(face.unicodeRange[1])
      || face.unicodeRange[0] < 0
      || face.unicodeRange[0] > face.unicodeRange[1]
      || face.unicodeRange[1] > 0xffff
    ) return false
    const selection = `${face.browserFamily}\u0000${face.weight}\u0000${face.style}`
    const selectionRanges = ranges.get(selection) ?? []
    if (selectionRanges.some(([minimum, maximum]) => face.unicodeRange[0] <= maximum && face.unicodeRange[1] >= minimum)) {
      return false
    }
    const exactSelection = `${selection}\u0000${face.unicodeRange[0]}\u0000${face.unicodeRange[1]}`
    if (selections.has(exactSelection)) return false
    identities.add(face.identity)
    selections.add(exactSelection)
    selectionRanges.push(face.unicodeRange)
    ranges.set(selection, selectionRanges)
  }
  return true
}

function cssString(value: string): string {
  return `"${value.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`
}

function cssUnicodeRange([minimum, maximum]: readonly [number, number]): string {
  return `U+${minimum.toString(16).padStart(4, "0")}-${maximum.toString(16).padStart(4, "0")}`
}

function reportedBrowserPlatform(): string | null {
  if (typeof navigator === "undefined") return null
  return (navigator as NavigatorWithUserAgentData).userAgentData?.platform ?? navigator.platform ?? null
}

function browserAdapter(reportedPlatform: string | null): LocalPlatformFontAdapter | null {
  if (typeof FontFace !== "function" || typeof document === "undefined" || !document.fonts) return null
  return Object.freeze({
    reportedPlatform,
    async loadLocalFace(request: LocalPlatformFontFaceRequest): Promise<FontFace> {
      const face = new FontFace(
        request.browserFamily,
        `local(${cssString(request.localName)})`,
        {
          style: request.style,
          weight: request.weight === 0 ? "normal" : String(request.weight),
          unicodeRange: cssUnicodeRange(request.unicodeRange),
        },
      )
      const loaded = await face.load()
      if (loaded.status !== "loaded") throw new Error("local face did not load")
      return loaded
    },
    addLoadedFace(face: unknown) {
      document.fonts.add(face as FontFace)
    },
    removeLoadedFace(face: unknown) {
      document.fonts.delete(face as FontFace)
    },
  })
}

export function classifyBrowserPlatform(reportedPlatform: string | null): BrowserPlatform {
  const value = reportedPlatform?.trim().toLowerCase() ?? ""
  if (value.startsWith("win")) return "windows"
  if (value.startsWith("mac")) return "macos"
  if (value.startsWith("linux") && !value.includes("android")) return "linux"
  return "other"
}

export async function admitLocalPlatformFonts(
  target: LocalPlatformFontTarget,
  injectedAdapter?: LocalPlatformFontAdapter,
): Promise<LocalPlatformFontCapability> {
  const reportedPlatform = injectedAdapter?.reportedPlatform ?? reportedBrowserPlatform()
  const adapter = injectedAdapter ?? browserAdapter(reportedPlatform)
  const platform = classifyBrowserPlatform(reportedPlatform)
  if (!validTarget(target)) return frozenUnsupported(target?.identity ?? "invalid", platform, "invalid-target")
  if (platform !== target.requiredPlatform) {
    return frozenUnsupported(target.identity, platform, "unsupported-platform", target.faces.map((face) => face.identity))
  }
  if (!adapter) {
    return frozenUnsupported(target.identity, platform, "font-loading-api-unavailable", target.faces.map((face) => face.identity))
  }

  const loaded: Array<Readonly<{ request: LocalPlatformFontFaceRequest; face: unknown }>> = []
  const missing: string[] = []
  for (const request of target.faces) {
    try {
      loaded.push(Object.freeze({ request, face: await adapter.loadLocalFace(request) }))
    } catch {
      missing.push(request.identity)
    }
  }
  if (missing.length > 0) return frozenUnsupported(target.identity, platform, "local-face-unavailable", missing)

  const published: unknown[] = []
  try {
    for (const loadedFace of loaded) {
      adapter.addLoadedFace(loadedFace.face)
      published.push(loadedFace.face)
    }
  } catch {
    for (const face of published.reverse()) {
      try { adapter.removeLoadedFace(face) } catch {}
    }
    return frozenUnsupported(target.identity, platform, "font-set-publication-failed", target.faces.map((face) => face.identity))
  }

  return Object.freeze({
    kind: "supported" as const,
    targetIdentity: target.identity,
    platform,
    faces: Object.freeze(target.faces.map((face) => Object.freeze({
      ...face,
      unicodeRange: Object.freeze([...face.unicodeRange]) as typeof face.unicodeRange,
    }))),
  })
}
