import { describe, expect, test } from "bun:test"
import {
  applyVguiGlyphEffects,
  getVguiCharacterMetrics,
  measureVguiText,
  mountVguiFontSet,
  type VguiExactFontMetrics,
  type VguiFontByteSupply,
  type VguiFontEffects,
  type VguiFontMountAdapter,
  type VguiFontSourceRequest,
  type VguiResolvedFontRequest,
  type VguiSuppliedFontProfile,
} from "../src"

const effects: VguiFontEffects = Object.freeze({
  italic: false,
  underline: false,
  strikeout: false,
  symbol: false,
  antialias: false,
  blur: 0,
  scanlines: 0,
  dropShadow: false,
  outline: false,
  additive: false,
  custom: false,
  bitmap: false,
  rotary: false,
})

function local(faceName = "Tahoma"): VguiFontSourceRequest {
  return Object.freeze({
    kind: "local",
    identity: `local:windows:${faceName.toLowerCase().replaceAll(" ", "-")}`,
    platform: "windows",
    faceName,
    version: null,
  })
}

function resolvedFont(
  identity: string,
  sources: readonly VguiFontSourceRequest[],
  overrides: Partial<VguiResolvedFontRequest> = {},
): VguiResolvedFontRequest {
  return Object.freeze({
    identity,
    schemeFontName: identity,
    proportional: false,
    family: sources[0]?.kind === "local" ? sources[0].faceName : "Supplied Test",
    candidateName: "1",
    requestedHeight: 10,
    weight: 500,
    bitmapScale: Object.freeze([1, 1]),
    effects,
    faces: Object.freeze([Object.freeze({
      identity: `${identity}:full`,
      unicodeRange: Object.freeze([0, 0xffff] as const),
      sources: Object.freeze([...sources]),
    })]),
    ...overrides,
  })
}

function adapter(events: string[] = [], overrides: Partial<VguiFontMountAdapter> = {}): VguiFontMountAdapter {
  return Object.freeze({
    async loadFace(request) {
      events.push(`load:${request.font.identity}:${request.source.identity}:${request.bytes?.byteLength ?? "local"}`)
      return Object.freeze({ identity: request.face.identity, source: request.source.identity })
    },
    publishFace(face) { events.push(`publish:${(face as { identity: string }).identity}`) },
    removeFace(face) { events.push(`remove:${(face as { identity: string }).identity}`) },
    ...overrides,
  })
}

async function hash(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return [...digest].map((value) => value.toString(16).padStart(2, "0")).join("")
}

async function byteSource(kind: "content" | "external", identity: string, text: string) {
  const bytes = new TextEncoder().encode(text)
  const sha256 = await hash(bytes)
  const source: VguiFontSourceRequest = Object.freeze({
    kind,
    identity: `${kind}:${identity}:${sha256}`,
    logicalIdentity: identity,
    sha256,
    byteLength: bytes.byteLength,
    version: "test-version-1",
    family: "Supplied Test",
  })
  const supply: VguiFontByteSupply = Object.freeze({
    kind,
    logicalIdentity: identity,
    sha256,
    byteLength: bytes.byteLength,
    version: "test-version-1",
    bytes,
  })
  return { source, supply }
}

function exactProfile(font: VguiResolvedFontRequest): VguiSuppliedFontProfile {
  return Object.freeze({
    fontIdentity: font.identity,
    metrics: Object.freeze({
      identity: `${font.identity}-metrics`,
      sha256: "3".repeat(64),
      version: "metrics-1",
      requestedHeight: 10,
      height: 12,
      ascent: 9,
      maxCharWidth: 9,
      glyphs: Object.freeze([
        Object.freeze({ codeUnit: 0x41, faceIdentity: `${font.identity}:full`, abc: Object.freeze([-1, 6, 1] as const) }),
        Object.freeze({ codeUnit: 0x56, faceIdentity: `${font.identity}:full`, abc: Object.freeze([0, 6, 0] as const) }),
        Object.freeze({ codeUnit: 0x26, faceIdentity: `${font.identity}:full`, abc: Object.freeze([0, 4, 0] as const) }),
      ]),
      kerning: Object.freeze([
        Object.freeze({ codeUnit: 0x41, before: 0, after: 0x56, width: 3, abcA: -1 }),
      ]),
    }),
    raster: Object.freeze({
      identity: `${font.identity}-raster`,
      sha256: "4".repeat(64),
      version: "raster-1",
      metricsIdentity: `${font.identity}-metrics`,
      pixelFormat: "rgba8",
      antialiasMode: "none",
    }),
  })
}

describe("supplied VGUI font mounting", () => {
  test("verifies and mounts exact game/content bytes, exact external bytes, and local faces atomically", async () => {
    const content = await byteSource("content", "resource/test-content.ttf", "content-font-bytes")
    const external = await byteSource("external", "caller/font/external.ttf", "external-font-bytes")
    const fonts = [
      resolvedFont("content-font", [content.source]),
      resolvedFont("external-font", [external.source]),
      resolvedFont("local-font", [local("Tahoma")]),
    ]
    const events: string[] = []
    const result = await mountVguiFontSet({
      identity: "test/complete-supplied-set",
      fonts,
      byteSupplies: [content.supply, external.supply],
      profiles: [exactProfile(fonts[0])],
    }, adapter(events))
    if (!result.ok) throw new Error(result.capability.reason)
    const snapshot = result.fontSet.snapshot()
    expect(snapshot.lifecycle).toBe("mounted")
    expect(snapshot.publishedFaces).toBe(3)
    if (snapshot.capability.kind !== "supported") throw new Error(snapshot.capability.reason)
    expect(snapshot.capability.fonts[0].metrics.kind).toBe("exact")
    expect(snapshot.capability.fonts[0].raster.kind).toBe("exact")
    expect(snapshot.capability.fonts[0].shaping).toBe("source-glyph-stream")
    expect(snapshot.capability.fonts[1].metrics).toEqual({
      kind: "required",
      reasons: ["exact-metrics-unavailable", "native-raster-profile-unavailable", "browser-shaping-differs", "non-antialiased-mode-unavailable"],
    })
    expect(snapshot.capability.fonts[2].raster).toEqual({
      kind: "required",
      reasons: [
        "exact-metrics-unavailable",
        "native-raster-profile-unavailable",
        "browser-shaping-differs",
        "non-antialiased-mode-unavailable",
        "local-version-unverifiable",
      ],
    })
    expect(events.filter((event) => event.startsWith("publish:"))).toHaveLength(3)

    result.fontSet.destroy()
    result.fontSet.destroy()
    expect(result.fontSet.snapshot()).toMatchObject({ lifecycle: "destroyed", publishedFaces: 0 })
    expect(events.filter((event) => event.startsWith("remove:"))).toHaveLength(3)
  })

  test("rejects changed supplied bytes before calling the font adapter", async () => {
    const content = await byteSource("content", "resource/test-content.ttf", "content-font-bytes")
    const loaded: string[] = []
    const result = await mountVguiFontSet({
      identity: "test/changed-content",
      fonts: [resolvedFont("content-font", [content.source])],
      byteSupplies: [Object.freeze({ ...content.supply, bytes: new TextEncoder().encode("changed-font-bytes!") })],
      profiles: [],
    }, adapter(loaded))
    expect(result).toEqual({
      ok: false,
      capability: {
        kind: "unsupported",
        targetIdentity: "test/changed-content",
        reason: "invalid-target",
        fontIdentity: null,
        faceIdentity: null,
        attemptedSourceIdentities: [],
      },
    })
    expect(loaded).toEqual([])
  })

  test("uses declared range fallback sources and reports every failed source without browser fallback", async () => {
    const lucida = local("Lucida Console")
    const tahoma = local("Tahoma")
    const font = resolvedFont("console", [lucida, tahoma])
    const events: string[] = []
    const admitted = await mountVguiFontSet({
      identity: "test/range-fallback",
      fonts: [font],
      byteSupplies: [],
      profiles: [],
    }, adapter(events, {
      async loadFace(request) {
        events.push(`try:${request.source.identity}`)
        if (request.source.identity === lucida.identity) throw new Error("missing")
        return Object.freeze({ identity: request.face.identity, source: request.source.identity })
      },
    }))
    if (!admitted.ok) throw new Error(admitted.capability.reason)
    const capability = admitted.fontSet.snapshot().capability
    if (capability.kind !== "supported") throw new Error(capability.reason)
    expect(capability.fonts[0].faces[0].selectedSource).toEqual(tahoma)
    admitted.fontSet.destroy()

    const failed = await mountVguiFontSet({
      identity: "test/no-declared-source",
      fonts: [font],
      byteSupplies: [],
      profiles: [],
    }, adapter([], { async loadFace() { throw new Error("missing") } }))
    expect(failed).toEqual({
      ok: false,
      capability: {
        kind: "unsupported",
        targetIdentity: "test/no-declared-source",
        reason: "font-source-unavailable",
        fontIdentity: "console",
        faceIdentity: "console:full",
        attemptedSourceIdentities: [lucida.identity, tahoma.identity],
      },
    })
  })

  test("rolls back the complete publication set including the face whose publication throws", async () => {
    const events: string[] = []
    const fonts = [resolvedFont("one", [local()]), resolvedFont("two", [local()])]
    const result = await mountVguiFontSet({
      identity: "test/publication-rollback",
      fonts,
      byteSupplies: [],
      profiles: [],
    }, adapter(events, {
      publishFace(face) {
        const identity = (face as { identity: string }).identity
        events.push(`publish:${identity}`)
        if (identity === "two:full") throw new Error("publish failed")
      },
    }))
    expect(result).toMatchObject({ ok: false, capability: { reason: "font-set-publication-failed" } })
    expect(events).toEqual([
      "load:one:local:windows:tahoma:local",
      "load:two:local:windows:tahoma:local",
      "publish:one:full",
      "publish:two:full",
      "remove:two:full",
      "remove:one:full",
    ])
  })

  test("admits supplied bitmap bytes only through an explicit bitmap adapter", async () => {
    const bytes = new TextEncoder().encode("bitmap-font-table")
    const sha256 = await hash(bytes)
    const source: VguiFontSourceRequest = Object.freeze({
      kind: "bitmap",
      identity: `bitmap:materials/vgui/fonts/buttons.vbf:${sha256}`,
      logicalIdentity: "materials/vgui/fonts/buttons.vbf",
      sha256,
      byteLength: bytes.byteLength,
      version: "vbf-test-1",
      family: "Buttons",
    })
    const font = resolvedFont("bitmap", [source], { effects: Object.freeze({ ...effects, bitmap: true }) })
    const result = await mountVguiFontSet({
      identity: "test/bitmap-adapter",
      fonts: [font],
      byteSupplies: [Object.freeze({
        kind: "bitmap",
        logicalIdentity: source.logicalIdentity,
        sha256,
        byteLength: bytes.byteLength,
        version: source.version,
        bytes,
      })],
      profiles: [],
    }, adapter())
    if (!result.ok) throw new Error(result.capability.reason)
    const capability = result.fontSet.snapshot().capability
    if (capability.kind !== "supported") throw new Error(capability.reason)
    expect(capability.fonts[0].raster).toEqual({
      kind: "required",
      reasons: [
        "exact-metrics-unavailable",
        "native-raster-profile-unavailable",
        "browser-shaping-differs",
        "non-antialiased-mode-unavailable",
        "bitmap-adapter-required",
      ],
    })
    result.fontSet.destroy()
  })

  test("releases earlier loaded faces when a later face has no admitted source", async () => {
    const events: string[] = []
    const first = resolvedFont("first", [local()])
    const second = resolvedFont("second", [local("Missing")])
    const result = await mountVguiFontSet({
      identity: "test/partial-load-cleanup",
      fonts: [first, second],
      byteSupplies: [],
      profiles: [],
    }, adapter(events, {
      async loadFace(request) {
        events.push(`load:${request.face.identity}`)
        if (request.font.identity === "second") throw new Error("missing")
        return Object.freeze({ identity: request.face.identity })
      },
    }))
    expect(result).toMatchObject({ ok: false, capability: { reason: "font-source-unavailable", fontIdentity: "second" } })
    expect(events).toEqual(["load:first:full", "load:second:full", "remove:first:full"])
  })

  test("repeats mount and idempotent destroy without retaining a published face", async () => {
    const events: string[] = []
    for (let cycle = 0; cycle < 8; cycle += 1) {
      const result = await mountVguiFontSet({
        identity: `test/repeat-${cycle}`,
        fonts: [resolvedFont("font", [local()])],
        byteSupplies: [],
        profiles: [],
      }, adapter(events))
      if (!result.ok) throw new Error(result.capability.reason)
      result.fontSet.destroy()
      result.fontSet.destroy()
      expect(result.fontSet.snapshot().publishedFaces).toBe(0)
    }
    expect(events.filter((event) => event.startsWith("publish:"))).toHaveLength(8)
    expect(events.filter((event) => event.startsWith("remove:"))).toHaveLength(8)
  })
})

describe("VGUI supplied metrics and raster effects", () => {
  const font = resolvedFont("metric-font", [local()])
  const profile: VguiExactFontMetrics = exactProfile(font).metrics

  test("returns supplied ABC metrics, missing-range maximum width, kerning, ampersand, and multiline size", () => {
    expect(getVguiCharacterMetrics(profile, 0x41)).toEqual({ faceIdentity: "metric-font:full", a: -1, b: 6, c: 1, width: 6 })
    expect(getVguiCharacterMetrics(profile, 0x5a)).toEqual({ faceIdentity: null, a: 0, b: 9, c: 0, width: 9 })
    expect(measureVguiText(profile, "AV")).toEqual({ width: 9, height: 12, lineCount: 1 })
    expect(measureVguiText(profile, "A&V\nZ")).toEqual({ width: 12, height: 24, lineCount: 2 })
    expect(measureVguiText(profile, "")).toEqual({ width: 0, height: 12, lineCount: 1 })

    const crossFace = Object.freeze({
      ...profile,
      glyphs: Object.freeze(profile.glyphs.map((glyph) => glyph.codeUnit === 0x56
        ? Object.freeze({ ...glyph, faceIdentity: "other-face" })
        : glyph)),
    })
    expect(measureVguiText(crossFace, "AV")).toEqual({ width: 12, height: 12, lineCount: 1 })
  })

  test("applies square outline, one-pixel shadow, scanline, and rotary effects without mutating input", () => {
    const rgba = new Uint8Array(3 * 3 * 4)
    rgba.set([255, 255, 255, 255], (1 * 3 + 1) * 4)
    const outlined = applyVguiGlyphEffects({ width: 3, height: 3, rgba }, Object.freeze({ ...effects, outline: true }))
    expect([...outlined.rgba.filter((_, index) => index % 4 === 3)]).toEqual(Array(9).fill(255))
    expect([...outlined.rgba.slice(0, 4)]).toEqual([0, 0, 0, 255])
    expect([...outlined.rgba.slice((1 * 3 + 1) * 4, (1 * 3 + 1) * 4 + 4)]).toEqual([255, 255, 255, 255])
    expect([...rgba.filter((_, index) => index % 4 === 3)]).toEqual([0, 0, 0, 0, 255, 0, 0, 0, 0])

    const shadowInput = new Uint8Array(3 * 3 * 4)
    shadowInput.set([255, 255, 255, 128], 0)
    const shadowed = applyVguiGlyphEffects({ width: 3, height: 3, rgba: shadowInput }, Object.freeze({ ...effects, dropShadow: true }))
    expect([...shadowed.rgba.slice((1 * 3 + 1) * 4, (1 * 3 + 1) * 4 + 4)]).toEqual([0, 0, 0, 128])

    const blurred = applyVguiGlyphEffects({ width: 3, height: 3, rgba }, Object.freeze({ ...effects, blur: 1 }))
    expect(blurred.rgba[(1 * 3 + 1) * 4 + 3]).toBeGreaterThan(0)
    expect(blurred.rgba[3]).toBeGreaterThan(0)
    const effected = applyVguiGlyphEffects({ width: 3, height: 3, rgba: shadowInput }, Object.freeze({
      ...effects,
      dropShadow: true,
      scanlines: 2,
      rotary: true,
    }))
    expect([...effected.rgba.slice((1 * 3 + 1) * 4, (1 * 3 + 1) * 4 + 4)]).toEqual([127, 127, 127, 255])
    expect([...effected.rgba.slice((2 * 3 + 1) * 4, (2 * 3 + 1) * 4 + 4)]).toEqual([0, 0, 0, 0])
  })
})
