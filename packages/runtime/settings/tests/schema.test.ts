import { describe, expect, test } from "bun:test"
import {
  DEFAULT_SETTINGS_LIMITS,
  SOURCE_CONVAR_FLAGS,
  SOURCE_COMMAND_LIMITS,
  SOURCE_KEY_MODIFIERS,
  SettingsCatalogError,
  defineSettingsCatalog,
  normalizeBindingCapture,
  validateSettingValue,
} from "../src"
import { fixtureCatalog } from "./fixture"

describe("immutable settings schemas", () => {
  test("retains the exact Source flag and modifier bit contracts", () => {
    expect(SOURCE_CONVAR_FLAGS.ARCHIVE).toBe(1 << 7)
    expect(SOURCE_CONVAR_FLAGS.USER_INFO).toBe(1 << 9)
    expect(SOURCE_CONVAR_FLAGS.REPLICATED).toBe(1 << 13)
    expect(SOURCE_CONVAR_FLAGS.CHEAT).toBe(1 << 14)
    expect(SOURCE_CONVAR_FLAGS.EXECUTE_DESPITE_DEFAULT).toBe(0x8000_0000)
    expect(SOURCE_COMMAND_LIMITS).toEqual({ maximumArguments: 64, maximumUtf8Bytes: 511 })
    expect(SOURCE_KEY_MODIFIERS).toMatchObject({ SHIFT: 1, CONTROL: 2, ALT: 4, ALL: 7 })
  })

  test("clones and freezes caller-owned catalog records", () => {
    const aliases = ["spacebar"]
    const settings = [{
      kind: "binding" as const,
      id: "input.jump",
      page: "keyboard",
      owner: "input" as const,
      defaultValue: { code: "space", modifiers: 0 },
      action: "+jump",
      flags: 0,
      visibility: "visible" as const,
      restart: "live" as const,
      persistence: "persistent" as const,
      consoleNames: [] as string[],
    }]
    const catalog = defineSettingsCatalog({
      identity: "immutable.catalog",
      bindingProfile: {
        identity: "immutable.profile",
        inputs: [{ code: "space", aliases }],
        modifierMask: 7,
        conflictPolicy: "replace",
        reserved: [],
      },
      settings,
    })
    aliases[0] = "changed"
    settings[0].id = "changed"
    settings[0].defaultValue.code = "changed"
    expect(catalog.settings[0]).toMatchObject({ id: "input.jump", defaultValue: { code: "space" } })
    expect(catalog.bindingProfile?.inputs[0].aliases).toEqual(["spacebar"])
    expect(Object.isFrozen(catalog.settings)).toBe(true)
    expect(Object.isFrozen(catalog.settings[0])).toBe(true)
  })

  test("validates every value domain without coercion or clamping", () => {
    const catalog = fixtureCatalog()
    const byId = (id: string) => catalog.settings.find((setting) => setting.id === id)!
    expect(validateSettingValue(catalog, byId("audio.master-muted"), true).ok).toBe(true)
    expect(validateSettingValue(catalog, byId("audio.master-muted"), 1).ok).toBe(false)
    expect(validateSettingValue(catalog, byId("video.texture-detail"), -1).ok).toBe(true)
    expect(validateSettingValue(catalog, byId("video.texture-detail"), 0.5).ok).toBe(false)
    expect(validateSettingValue(catalog, byId("audio.master-volume"), Number.NaN).ok).toBe(false)
    expect(validateSettingValue(catalog, byId("video.vsync"), "1").ok).toBe(false)
    expect(validateSettingValue(catalog, byId("application.player-name"), "é".repeat(6)).ok).toBe(true)
    expect(validateSettingValue(catalog, byId("application.player-name"), "é".repeat(7)).ok).toBe(false)
    expect(validateSettingValue(catalog, byId("input.jump"), { code: "spacebar", modifiers: 0 }).ok).toBe(true)
    expect(validateSettingValue(catalog, byId("input.jump"), { code: "unknown", modifiers: 0 }).ok).toBe(false)
  })

  test("retains finite owner state outside an Options edit range without admitting that edit", () => {
    const catalog = defineSettingsCatalog({
      identity: "owner.range",
      settings: [{
        kind: "float",
        id: "color.green",
        page: "advanced",
        owner: "game",
        defaultValue: 0,
        minimum: 1,
        maximum: 255,
        flags: 0,
        visibility: "visible",
        restart: "live",
        persistence: "persistent",
        consoleNames: [],
      }],
    })
    expect(catalog.settings[0]).toMatchObject({ defaultValue: 0, minimum: 1 })
    expect(validateSettingValue(catalog, catalog.settings[0], 0).ok).toBe(false)
  })

  test("normalizes physical aliases and left/right modifier state into Source bits", () => {
    const profile = fixtureCatalog().bindingProfile!
    expect(normalizeBindingCapture(profile, {
      code: "spacebar",
      shift: true,
      control: true,
      alt: true,
    })).toEqual({ ok: true, binding: { code: "space", modifiers: 7 } })
    expect(normalizeBindingCapture(profile, { code: "esc" })).toMatchObject({ ok: false, code: "ReservedBinding" })
    expect(normalizeBindingCapture(profile, { code: "missing" })).toMatchObject({ ok: false, code: "UnknownPhysicalInput" })
  })

  test("rejects collisions, invalid defaults, unknown targets, and raised limits", () => {
    const base = fixtureCatalog()
    expect(() => defineSettingsCatalog({
      identity: "collision.catalog",
      convars: [
        { name: "volume", defaultValue: "1", help: "", flags: 0, visibility: "visible" },
      ],
      commands: [
        { name: "VOLUME", help: "", flags: 0, visibility: "visible", completion: "none" },
      ],
      settings: [],
    })).toThrow(SettingsCatalogError)
    expect(() => defineSettingsCatalog({
      identity: "unknown.target",
      settings: [{
        kind: "boolean",
        id: "test.value",
        page: "test",
        owner: "game",
        defaultValue: false,
        flags: 0,
        visibility: "visible",
        restart: "live",
        persistence: "persistent",
        consoleNames: ["missing"],
      }],
    })).toThrow(/unknown console name/)
    expect(() => defineSettingsCatalog({
      identity: "bad.default",
      settings: [{
        kind: "integer",
        id: "test.value",
        page: "test",
        owner: "game",
        defaultValue: 0.5,
        minimum: 0,
        maximum: 1,
        flags: 0,
        visibility: "visible",
        restart: "live",
        persistence: "persistent",
        consoleNames: [],
      }],
    })).toThrow(/integer setting/)
    expect(() => defineSettingsCatalog({
      identity: "raised.limit",
      limits: { maximumSettings: DEFAULT_SETTINGS_LIMITS.maximumSettings + 1 },
      settings: [],
    })).toThrow(/maximumSettings/)
    expect(base.settings).toHaveLength(8)
  })
})
