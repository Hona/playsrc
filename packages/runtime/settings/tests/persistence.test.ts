import { describe, expect, test } from "bun:test"
import {
  SettingsPersistenceError,
  createSettingsState,
  decodeSettingsPersistence,
  encodeSettingsPersistence,
} from "../src"
import { availableOwners, fixtureCatalog } from "./fixture"

const encoder = new TextEncoder()

function replaceJson(bytes: Uint8Array, mutate: (value: any) => void): Uint8Array {
  const value = JSON.parse(new TextDecoder().decode(bytes))
  mutate(value)
  return encoder.encode(JSON.stringify(value))
}

describe("persistence-neutral settings bytes", () => {
  test("encodes schema-ordered canonical bytes and round-trips all persistent kinds", () => {
    const catalog = fixtureCatalog()
    const values = {
      "audio.master-volume": 0.8,
      "audio.master-muted": true,
      "video.texture-detail": -1,
      "video.vsync": 1,
      "application.player-name": "Démo",
      "input.jump": null,
      "input.voice": { code: "key-m", modifiers: 5 },
      "session.preview": true,
    } as const
    const bytes = encodeSettingsPersistence(catalog, values)
    expect(new TextDecoder().decode(bytes)).toBe(
      '{"format":"playsrc-settings","revision":1,"catalog":"test.settings.replace","values":' +
      '[["audio.master-volume","float",0.8],["audio.master-muted","boolean",true],' +
      '["video.texture-detail","integer",-1],["video.vsync","enum",1],' +
      '["application.player-name","string","Démo"],["input.jump","binding",null],' +
      '["input.voice","binding",{"code":"key-m","modifiers":5}]]}',
    )
    const decoded = decodeSettingsPersistence(catalog, bytes)
    expect(decoded).toMatchObject({ ok: true, decoded: { values: {
      "audio.master-volume": 0.8,
      "audio.master-muted": true,
      "application.player-name": "Démo",
      "input.jump": null,
      "input.voice": { code: "key-m", modifiers: 5 },
    } } })
    if (!decoded.ok) throw new Error(decoded.diagnostic.message)
    expect(decoded.decoded.values).not.toHaveProperty("session.preview")
    expect(Object.isFrozen(decoded.decoded.values)).toBe(true)
  })

  test("stages a decoded document atomically without changing current values", () => {
    const catalog = fixtureCatalog()
    const source = createSettingsState({ catalog, owners: availableOwners() })
    const transaction = source.beginTransaction()
    if (!transaction.ok) throw new Error(transaction.diagnostic.message)
    source.setValue(transaction.transactionId, "audio.master-volume", 0.7)
    source.setValue(transaction.transactionId, "audio.master-muted", true)
    const bytes = encodeSettingsPersistence(catalog, source.snapshot().pending!)

    const target = createSettingsState({ catalog, owners: availableOwners() })
    const targetTransaction = target.beginTransaction()
    if (!targetTransaction.ok) throw new Error(targetTransaction.diagnostic.message)
    const decoded = decodeSettingsPersistence(catalog, bytes)
    if (!decoded.ok) throw new Error(decoded.diagnostic.message)
    expect(target.stagePersistence(targetTransaction.transactionId, decoded.decoded.values).ok).toBe(true)
    expect(target.snapshot()).toMatchObject({
      current: { "audio.master-volume": 1, "audio.master-muted": false },
      pending: { "audio.master-volume": 0.7, "audio.master-muted": true },
    })
  })

  test("rejects wrong, unknown, duplicate, missing, malformed, and over-bound documents", () => {
    const catalog = fixtureCatalog()
    const state = createSettingsState({ catalog, owners: availableOwners() })
    const bytes = encodeSettingsPersistence(catalog, state.snapshot().current)
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.catalog = "other.catalog" })))
      .toMatchObject({ ok: false, diagnostic: { code: "WrongCatalog" } })
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.values[0][0] = "unknown.setting" })))
      .toMatchObject({ ok: false, diagnostic: { code: "UnknownPersistedSetting", unknownIds: ["unknown.setting"] } })
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.values.push(value.values[0]) })))
      .toMatchObject({ ok: false, diagnostic: { code: "DuplicatePersistedSetting" } })
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.values.pop() })))
      .toMatchObject({ ok: false, diagnostic: { code: "MissingPersistedSetting" } })
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.values[0][2] = Number.NaN })))
      .toMatchObject({ ok: false, diagnostic: { code: "MalformedPersistence" } })
    expect(decodeSettingsPersistence(catalog, replaceJson(bytes, (value) => { value.values = {} })))
      .toMatchObject({ ok: false, diagnostic: { code: "MalformedPersistence" } })
    expect(decodeSettingsPersistence(catalog, new Uint8Array([0xff])))
      .toMatchObject({ ok: false, diagnostic: { code: "MalformedPersistence" } })

    const bounded = fixtureCatalog("replace", { maximumPersistenceBytes: 64 })
    expect(decodeSettingsPersistence(bounded, new Uint8Array(65)))
      .toMatchObject({ ok: false, diagnostic: { code: "LimitExceeded" } })
    expect(() => encodeSettingsPersistence(bounded, state.snapshot().current)).toThrow(SettingsPersistenceError)
  })

  test("refuses missing and invalid values during encoding", () => {
    const catalog = fixtureCatalog()
    expect(() => encodeSettingsPersistence(catalog, {})).toThrow(/missing/)
    expect(() => encodeSettingsPersistence(catalog, {
      ...createSettingsState({ catalog }).snapshot().current,
      "application.player-name": "x".repeat(13),
    })).toThrow(/UTF-8/)
    expect(() => encodeSettingsPersistence(catalog, {
      ...createSettingsState({ catalog }).snapshot().current,
      unknown: true,
    })).toThrow(/unknown/)
  })
})
