import { describe, expect, test } from "bun:test"
import {
  evaluateVguiSchemeCondition,
  resolveVguiSchemeFonts,
  type VguiDesktopPlatform,
  type VguiFontFileIdentity,
  type VguiSchemeDocument,
  type VguiSchemeFontContext,
  type VguiSchemeFontLookup,
  type VguiSchemeNode,
} from "../src"

function node(
  name: string,
  value: string | readonly VguiSchemeNode[] | null = null,
  condition: string | null = null,
): VguiSchemeNode {
  return Object.freeze({
    name,
    value: typeof value === "string" ? value : null,
    condition,
    children: Object.freeze(Array.isArray(value) ? [...value] : []),
  })
}

function candidate(properties: readonly VguiSchemeNode[], condition: string | null = null): VguiSchemeNode {
  return node("1", properties, condition)
}

function font(name: string, properties: readonly VguiSchemeNode[], condition: string | null = null): VguiSchemeNode {
  return node(name, [candidate(properties)], condition)
}

const base: VguiSchemeDocument = Object.freeze({
  logicalIdentity: "resource/sourceschemebase.res",
  sha256: "1".repeat(64),
  baseLogicalIdentities: Object.freeze([]),
  root: node("Scheme", [
    node("BaseSettings", [
      node("FrameTitleBar.Font", "UiBold", "[$WIN32]"),
      node("FrameTitleBar.Font", "DefaultLarge", "[$WIN32]"),
    ]),
    node("Fonts", [
      font("DefaultFixedOutline", [
        node("name", "Lucida Console", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "14", "[$LINUX]"),
        node("tall", "11", "[$OSX]"),
        node("tall", "10"),
        node("tall_lodef", "15"),
        node("tall_hidef", "20"),
        node("weight", "0"),
        node("outline", "1"),
      ]),
      font("Default", [
        node("name", "Tahoma", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "16", "[!$LINUX]"),
        node("tall", "18", "[$LINUX]"),
        node("weight", "500"),
      ]),
      font("DefaultSmall", [
        node("name", "Tahoma", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "12", "[!$POSIX]"),
        node("tall", "15", "[$POSIX]"),
        node("weight", "0"),
      ]),
      font("DefaultLarge", [
        node("name", "Tahoma", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "18"),
        node("weight", "0"),
      ]),
      font("UiBold", [
        node("name", "Tahoma", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "12", "[!$LINUX]"),
        node("tall", "15", "[$LINUX]"),
        node("weight", "1000"),
      ], "[$WIN32]"),
      font("ConsoleText", [
        node("name", "Lucida Console", "[!$OSX]"),
        node("name", "Verdana", "[$OSX]"),
        node("tall", "11", "[$OSX]"),
        node("tall", "14", "[$LINUX]"),
        node("tall", "10"),
        node("weight", "500"),
      ]),
      node("ResolutionFont", [
        node("1", [node("name", "Tahoma"), node("tall", "10"), node("weight", "0"), node("yres", "480 719")]),
        node("2", [node("name", "Tahoma"), node("tall", "20"), node("weight", "0"), node("yres", "720 1080")]),
      ]),
      font("MinModeFont", [
        node("name", "Tahoma"),
        node("tall", "10"),
        node("tall_minmode", "7"),
        node("weight", "0"),
      ]),
      font("AppleSymbols", [node("name", "Apple Symbols"), node("tall", "12"), node("weight", "0")]),
      font("Marlett", [node("name", "Marlett"), node("tall", "14"), node("weight", "0"), node("symbol", "1")]),
    ]),
  ]),
})

const derived: VguiSchemeDocument = Object.freeze({
  logicalIdentity: "resource/sourcescheme.res",
  sha256: "2".repeat(64),
  baseLogicalIdentities: Object.freeze([base.logicalIdentity]),
  root: node("Scheme", [
    node("BaseSettings", [node("FrameTitleBar.Font", "DefaultLarge", "[!$OSX]")]),
    node("Fonts", [
      font("MainMenuFont", [
        node("name", "TF2 Build"),
        node("tall", "18"),
        node("weight", "500"),
        node("antialias", "1"),
      ], "[!$OSX]"),
    ]),
    node("CustomFontFiles", [
      node("6", [
        node("font", "resource/tf2build.ttf"),
        node("name", "TF2 Build"),
        node("german", [node("range", "0x0000 0x00FC")]),
      ]),
    ]),
  ]),
})

const tf2Build: VguiFontFileIdentity = Object.freeze({
  kind: "content",
  logicalIdentity: "resource/tf2build.ttf",
  sha256: "23faa58a08c929c0b6638f581488e49399cd7a390c70cb9debdaf8371a95e0c6",
  byteLength: 61_696,
  version: "sfnt-16.16:65536",
  families: Object.freeze(["TF2 Build"]),
})

function context(platform: VguiDesktopPlatform, overrides: Partial<VguiSchemeFontContext> = {}): VguiSchemeFontContext {
  return Object.freeze({
    platform,
    viewportHeight: 480,
    language: "english",
    minMode: false,
    steamDeck: false,
    surfaceFeatures: Object.freeze({ antialias: true, dropShadow: true, outline: true }),
    ...overrides,
  })
}

function lookup(identity: string, name: string, proportional = false): VguiSchemeFontLookup {
  return Object.freeze({ identity, name, proportional })
}

function resolve(platform: VguiDesktopPlatform, lookups: readonly VguiSchemeFontLookup[], overrides: Partial<VguiSchemeFontContext> = {}) {
  return resolveVguiSchemeFonts({
    schemeLogicalIdentity: derived.logicalIdentity,
    documents: [derived, base],
    fontFiles: [tf2Build],
    localFonts: [],
    context: context(platform, overrides),
    lookups,
  })
}

describe("Source desktop scheme font selection", () => {
  test("evaluates the official desktop condition vocabulary and WIN32 PC meaning", () => {
    expect(evaluateVguiSchemeCondition("[$WIN32]", context("windows"))).toBe(true)
    expect(evaluateVguiSchemeCondition("[$WIN32]", context("macos"))).toBe(true)
    expect(evaluateVguiSchemeCondition("[$WIN32]", context("linux"))).toBe(true)
    expect(evaluateVguiSchemeCondition("[$WINDOWS]", context("macos"))).toBe(false)
    expect(evaluateVguiSchemeCondition("[!$OSX]", context("macos"))).toBe(false)
    expect(evaluateVguiSchemeCondition("[$POSIX]", context("linux"))).toBe(true)
    expect(evaluateVguiSchemeCondition("[$LINUX]", context("macos"))).toBe(false)
    expect(evaluateVguiSchemeCondition("[$UNKNOWN]", context("windows"))).toBe(false)
  })

  test("composes the configured Windows console roles without applying console resolution suffixes", () => {
    const result = resolve("windows", [
      lookup("title", "DefaultLarge"),
      lookup("history", "ConsoleText"),
      lookup("entry", "Default"),
      lookup("completion", "DefaultSmall"),
      lookup("diagnostic", "DefaultFixedOutline"),
    ])
    if (!result.ok) throw new Error(result.diagnostic.code)
    expect(result.documentLogicalIdentities).toEqual([derived.logicalIdentity, base.logicalIdentity])
    expect(result.fonts.map((font) => [font.identity, font.family, font.requestedHeight, font.weight])).toEqual([
      ["title", "Tahoma", 18, 0],
      ["history", "Lucida Console", 10, 500],
      ["entry", "Tahoma", 16, 500],
      ["completion", "Tahoma", 12, 0],
      ["diagnostic", "Lucida Console", 10, 0],
    ])
    expect(result.fonts[4].effects.outline).toBe(true)
    expect(result.fonts[4].faces.map((face) => [face.unicodeRange, face.sources.map((source) => source.kind === "local" ? source.faceName : source.logicalIdentity)])).toEqual([
      [[0, 255], ["Lucida Console", "Tahoma"]],
      [[256, 65535], ["Tahoma"]],
    ])
  })

  test("selects the macOS and Linux property variants rather than a Windows target", () => {
    const macos = resolve("macos", [
      lookup("title", "UiBold"),
      lookup("history", "ConsoleText"),
      lookup("entry", "Default"),
      lookup("completion", "DefaultSmall"),
      lookup("diagnostic", "DefaultFixedOutline"),
    ])
    if (!macos.ok) throw new Error(macos.diagnostic.code)
    expect(macos.fonts.map((font) => [font.family, font.requestedHeight, font.weight])).toEqual([
      ["Verdana", 12, 1000],
      ["Verdana", 11, 500],
      ["Verdana", 16, 500],
      ["Verdana", 15, 0],
      ["Verdana", 11, 0],
    ])
    expect(macos.fonts[1].faces.map((face) => face.sources.map((source) => source.kind === "local" ? source.faceName : source.logicalIdentity))).toEqual([
      ["Verdana", "Helvetica"],
      ["Helvetica"],
    ])

    const linux = resolve("linux", [
      lookup("title", "DefaultLarge"),
      lookup("history", "ConsoleText"),
      lookup("entry", "Default"),
      lookup("completion", "DefaultSmall"),
      lookup("diagnostic", "DefaultFixedOutline"),
    ])
    if (!linux.ok) throw new Error(linux.diagnostic.code)
    expect(linux.fonts.map((font) => [font.family, font.requestedHeight, font.weight])).toEqual([
      ["Tahoma", 18, 0],
      ["Lucida Console", 14, 500],
      ["Tahoma", 18, 500],
      ["Tahoma", 15, 0],
      ["Lucida Console", 14, 0],
    ])
  })

  test("selects yres before proportional scaling and applies desktop minmode separately", () => {
    const selected = resolve("windows", [lookup("resolution", "ResolutionFont", true)], { viewportHeight: 720 })
    if (!selected.ok) throw new Error(selected.diagnostic.code)
    expect(selected.fonts[0].candidateName).toBe("2")
    expect(selected.fonts[0].requestedHeight).toBe(20)

    const proportional = resolve("windows", [lookup("default", "Default", true)], { viewportHeight: 720 })
    if (!proportional.ok) throw new Error(proportional.diagnostic.code)
    expect(proportional.fonts[0].requestedHeight).toBe(24)

    const minmode = resolve("windows", [lookup("minmode", "MinModeFont")], { minMode: true })
    if (!minmode.ok) throw new Error(minmode.diagnostic.code)
    expect(minmode.fonts[0].requestedHeight).toBe(7)

    const korean = resolve("windows", [lookup("small", "DefaultSmall")], { language: "korean" })
    if (!korean.ok) throw new Error(korean.diagnostic.code)
    expect(korean.fonts[0].requestedHeight).toBe(13)
    const thai = resolve("windows", [lookup("small", "DefaultSmall")], { language: "thai" })
    if (!thai.ok) throw new Error(thai.diagnostic.code)
    expect(thai.fonts[0].requestedHeight).toBe(18)
  })

  test("binds exact game font bytes and selected-language range without substituting them for system roles", () => {
    const result = resolve("windows", [lookup("main-menu", "MainMenuFont"), lookup("entry", "Default")], { language: "german" })
    if (!result.ok) throw new Error(result.diagnostic.code)
    const main = result.fonts[0]
    expect(main.faces[0].unicodeRange).toEqual([0, 0xfc])
    expect(main.faces[0].sources[0]).toMatchObject({
      kind: "content",
      logicalIdentity: tf2Build.logicalIdentity,
      sha256: tf2Build.sha256,
      byteLength: tf2Build.byteLength,
      version: tf2Build.version,
      family: "TF2 Build",
    })
    expect(result.fonts[1].faces[0].sources).toEqual([
      expect.objectContaining({ kind: "local", faceName: "Tahoma" }),
    ])
  })

  test("retains complete-family surface fallback ranges instead of a browser generic", () => {
    const windows = resolve("windows", [lookup("marlett", "Marlett")])
    if (!windows.ok) throw new Error(windows.diagnostic.code)
    expect(windows.fonts[0].faces).toEqual([
      expect.objectContaining({
        unicodeRange: [0, 0xffff],
        sources: [
          expect.objectContaining({ kind: "local", faceName: "Marlett" }),
          expect.objectContaining({ kind: "local", faceName: "Tahoma" }),
        ],
      }),
    ])

    const macos = resolve("macos", [lookup("symbols", "AppleSymbols")])
    if (!macos.ok) throw new Error(macos.diagnostic.code)
    expect(macos.fonts[0].faces.map((face) => [face.unicodeRange, face.sources.map((source) => source.kind === "local" ? source.faceName : source.family)])).toEqual([
      [[0, 0x00ff], ["Apple Symbols", "Monaco", "Helvetica"]],
      [[0x0100, 0xffff], ["Apple Symbols", "Helvetica"]],
    ])
  })

  test("reports missing and cyclic composition without partial fonts", () => {
    expect(resolveVguiSchemeFonts({
      schemeLogicalIdentity: derived.logicalIdentity,
      documents: [derived],
      fontFiles: [tf2Build],
      localFonts: [],
      context: context("windows"),
      lookups: [lookup("default", "Default")],
    })).toEqual({ ok: false, diagnostic: { code: "MissingDocument", subject: base.logicalIdentity } })

    const cyclic = Object.freeze({ ...base, baseLogicalIdentities: Object.freeze([derived.logicalIdentity]) })
    expect(resolveVguiSchemeFonts({
      schemeLogicalIdentity: derived.logicalIdentity,
      documents: [derived, cyclic],
      fontFiles: [tf2Build],
      localFonts: [],
      context: context("windows"),
      lookups: [lookup("default", "Default")],
    })).toEqual({ ok: false, diagnostic: { code: "CyclicBase", subject: derived.logicalIdentity } })
  })

  test("rejects unresolved custom bytes and malformed selected-language ranges", () => {
    expect(resolveVguiSchemeFonts({
      schemeLogicalIdentity: derived.logicalIdentity,
      documents: [derived, base],
      fontFiles: [],
      localFonts: [],
      context: context("windows"),
      lookups: [lookup("main-menu", "MainMenuFont")],
    })).toEqual({
      ok: false,
      diagnostic: { code: "MissingFontFileIdentity", subject: tf2Build.logicalIdentity },
    })

    const malformed = Object.freeze({
      ...derived,
      root: node("Scheme", [
        ...derived.root.children.filter((child) => child.name !== "CustomFontFiles"),
        node("CustomFontFiles", [node("6", [
          node("font", tf2Build.logicalIdentity),
          node("name", "TF2 Build"),
          node("german", [node("range", "not-a-range")]),
        ])]),
      ]),
    })
    expect(resolveVguiSchemeFonts({
      schemeLogicalIdentity: malformed.logicalIdentity,
      documents: [malformed, base],
      fontFiles: [tf2Build],
      localFonts: [],
      context: context("windows", { language: "german" }),
      lookups: [lookup("main-menu", "MainMenuFont")],
    })).toEqual({
      ok: false,
      diagnostic: { code: "MalformedDocument", subject: tf2Build.logicalIdentity },
    })
  })
})
