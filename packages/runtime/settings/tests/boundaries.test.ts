import { describe, expect, test } from "bun:test"
import {
  DEFAULT_SETTINGS_LIMITS,
  SettingsCatalogError,
  SettingsStateError,
  createSettingsState,
  defineSettingsCatalog,
  type SettingSchema,
} from "../src"
import { availableOwners, fixtureCatalog } from "./fixture"

function booleanSchema(index: number): SettingSchema {
  return {
    kind: "boolean",
    id: `test.value-${index}`,
    page: "test",
    owner: "game",
    defaultValue: false,
    flags: 0,
    visibility: "visible",
    restart: "live",
    persistence: "persistent",
    consoleNames: [],
  }
}

describe("catalog and transaction boundaries", () => {
  test("admits exact setting/profile limits and rejects limit plus one", () => {
    const limit = 3
    const exact = defineSettingsCatalog({
      identity: "boundary.exact",
      limits: { maximumSettings: limit, maximumPhysicalInputs: limit, maximumTransactionChanges: limit },
      bindingProfile: {
        identity: "boundary.profile",
        inputs: Array.from({ length: limit }, (_, index) => ({ code: `key-${index}` })),
        modifierMask: 0,
        conflictPolicy: "reject",
        reserved: [],
      },
      settings: Array.from({ length: limit }, (_, index) => booleanSchema(index)),
    })
    expect(exact.settings).toHaveLength(limit)
    expect(exact.bindingProfile?.inputs).toHaveLength(limit)
    expect(() => defineSettingsCatalog({
      identity: "boundary.settings-over",
      limits: { maximumSettings: limit, maximumTransactionChanges: limit },
      settings: Array.from({ length: limit + 1 }, (_, index) => booleanSchema(index)),
    })).toThrow(SettingsCatalogError)
    expect(() => defineSettingsCatalog({
      identity: "boundary.inputs-over",
      limits: { maximumPhysicalInputs: limit },
      bindingProfile: {
        identity: "boundary.profile-over",
        inputs: Array.from({ length: limit + 1 }, (_, index) => ({ code: `key-${index}` })),
        modifierMask: 0,
        conflictPolicy: "reject",
        reserved: [],
      },
      settings: [],
    })).toThrow(SettingsCatalogError)
  })

  test("rolls back a limit-plus-one staged change", () => {
    const state = createSettingsState({
      catalog: fixtureCatalog("replace", { maximumTransactionChanges: 1 }),
      owners: availableOwners(),
    })
    const begun = state.beginTransaction()
    if (!begun.ok) throw new Error(begun.diagnostic.message)
    expect(state.setValue(begun.transactionId, "audio.master-volume", 0.5).ok).toBe(true)
    expect(state.setValue(begun.transactionId, "audio.master-muted", true)).toMatchObject({
      ok: false,
      diagnostic: { code: "LimitExceeded" },
    })
    expect(state.snapshot()).toMatchObject({
      pending: { "audio.master-volume": 0.5, "audio.master-muted": false },
      dirtySettingIds: ["audio.master-volume"],
    })
    expect(state.stagePersistence(begun.transactionId, {
      "audio.master-volume": 0.75,
      "audio.master-muted": true,
    })).toMatchObject({ ok: false, diagnostic: { code: "LimitExceeded" } })
    expect(state.snapshot()).toMatchObject({
      pending: { "audio.master-volume": 0.5, "audio.master-muted": false },
    })
  })

  test("rejects conflicting persisted bindings atomically", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const begun = state.beginTransaction()
    if (!begun.ok) throw new Error(begun.diagnostic.message)
    expect(state.stagePersistence(begun.transactionId, {
      "audio.master-volume": 0.25,
      "input.jump": { code: "key-m", modifiers: 0 },
      "input.voice": { code: "key-m", modifiers: 0 },
    })).toMatchObject({ ok: false, diagnostic: { code: "BindingConflict" } })
    expect(state.snapshot().pending).toMatchObject({
      "audio.master-volume": 1,
      "input.jump": { code: "space", modifiers: 0 },
      "input.voice": { code: "key-k", modifiers: 2 },
    })
  })

  test("rejects malformed initial and owner state before publication", () => {
    const catalog = fixtureCatalog()
    expect(() => createSettingsState({ catalog, initial: { unknown: true } })).toThrow(SettingsStateError)
    expect(() => createSettingsState({
      catalog,
      owners: { audio: "broken" as "available" },
    })).toThrow(SettingsStateError)
    expect(() => createSettingsState({
      catalog: { ...catalog, settings: [...catalog.settings] },
    })).toThrow(/defineSettingsCatalog/)
  })

  test("keeps package ceilings internally consistent", () => {
    expect(DEFAULT_SETTINGS_LIMITS.maximumTransactionChanges).toBeLessThanOrEqual(DEFAULT_SETTINGS_LIMITS.maximumSettings)
    expect(DEFAULT_SETTINGS_LIMITS.maximumPersistenceBytes).toBe(256 * 1_024)
    expect(DEFAULT_SETTINGS_LIMITS.maximumJournalEntries).toBe(512)
  })
})
