import { describe, expect, test } from "bun:test"
import {
  SOURCE_KEY_MODIFIERS,
  createSettingsState,
  type AdapterRequestResult,
  type SettingsState,
} from "../src"
import { availableOwners, fixtureCatalog } from "./fixture"

function begin(state: SettingsState): number {
  const result = state.beginTransaction()
  if (!result.ok) throw new Error(result.diagnostic.message)
  return result.transactionId
}

describe("settings transactions", () => {
  test("keeps current, pending, and applied distinct across cancel and reset", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    expect(state.setValue(transaction, "audio.master-volume", 0.625).ok).toBe(true)
    expect(state.setValue(transaction, "application.player-name", "Scout").ok).toBe(true)
    expect(state.snapshot()).toMatchObject({
      current: { "audio.master-volume": 1, "application.player-name": "Player" },
      applied: { "audio.master-volume": 1, "application.player-name": "Player" },
      pending: { "audio.master-volume": 0.625, "application.player-name": "Scout" },
      dirtySettingIds: ["audio.master-volume", "application.player-name"],
    })
    expect(state.cancel(transaction).ok).toBe(true)
    expect(state.snapshot()).toMatchObject({ pending: null, dirtySettingIds: [] })

    const resetTransaction = begin(state)
    state.setValue(resetTransaction, "audio.master-volume", 0.25)
    state.setValue(resetTransaction, "application.player-name", "Medic")
    expect(state.reset(resetTransaction, ["audio.master-volume"]).ok).toBe(true)
    expect(state.snapshot()).toMatchObject({
      pending: { "audio.master-volume": 1, "application.player-name": "Medic" },
      dirtySettingIds: ["application.player-name"],
    })
  })

  test("produces owner-batched requests in fixed owner and schema order", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    state.setValue(transaction, "audio.master-volume", 0.5)
    state.setValue(transaction, "audio.master-muted", true)
    state.setValue(transaction, "video.texture-detail", 2)
    state.setValue(transaction, "application.player-name", "Heavy")
    const prepared = state.prepareApply(transaction)
    if (!prepared.ok) throw new Error(prepared.diagnostic.message)
    expect(prepared.plan.requests.map((request) => request.owner)).toEqual(["renderer", "audio", "application"])
    expect(prepared.plan.requests[1].changes.map((change) => change.settingId)).toEqual([
      "audio.master-volume",
      "audio.master-muted",
    ])
    expect(prepared.plan.requests[1].changes).toMatchObject([
      { previousValue: 1, nextValue: 0.5, consoleNames: ["volume"] },
      { previousValue: false, nextValue: true, consoleNames: [] },
    ])
    expect(state.setValue(transaction, "video.vsync", 1)).toMatchObject({
      ok: false,
      diagnostic: { code: "ApplyInFlight" },
    })

    const results = prepared.plan.requests.map((request): AdapterRequestResult => ({
      requestId: request.requestId,
      status: "applied",
    }))
    const settled = state.settleApply(prepared.plan.planId, results)
    expect(settled).toMatchObject({
      ok: true,
      complete: true,
      rejectedOwners: [],
      restart: ["owner-restart", "application-restart"],
    })
    expect(state.snapshot()).toMatchObject({
      current: {
        "audio.master-volume": 0.5,
        "audio.master-muted": true,
        "video.texture-detail": 2,
        "application.player-name": "Heavy",
      },
      applied: {
        "audio.master-volume": 0.5,
        "audio.master-muted": true,
      },
      pending: null,
    })
  })

  test("retains rejected owner changes while publishing successful batches", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    state.setValue(transaction, "video.vsync", 1)
    state.setValue(transaction, "audio.master-volume", 0.75)
    const prepared = state.prepareApply(transaction)
    if (!prepared.ok) throw new Error(prepared.diagnostic.message)
    const results = prepared.plan.requests.map((request): AdapterRequestResult => request.owner === "renderer"
      ? { requestId: request.requestId, status: "rejected", reason: "device lost" }
      : { requestId: request.requestId, status: "applied" })
    expect(state.settleApply(prepared.plan.planId, results)).toMatchObject({
      ok: true,
      complete: false,
      rejectedOwners: ["renderer"],
      rejections: [{ owner: "renderer", reason: "device lost" }],
    })
    expect(state.snapshot()).toMatchObject({
      current: { "video.vsync": 0, "audio.master-volume": 0.75 },
      applied: { "video.vsync": 0, "audio.master-volume": 0.75 },
      pending: { "video.vsync": 1, "audio.master-volume": 0.75 },
      dirtySettingIds: ["video.vsync"],
      activeTransactionId: transaction,
    })
    const retry = state.prepareApply(transaction)
    if (!retry.ok) throw new Error(retry.diagnostic.message)
    expect(retry.plan.requests.map((request) => request.owner)).toEqual(["renderer"])
  })

  test("blocks unknown and unavailable owners before emitting any plan", () => {
    const state = createSettingsState({
      catalog: fixtureCatalog(),
      owners: availableOwners({ audio: "unknown" }),
    })
    const transaction = begin(state)
    state.setValue(transaction, "audio.master-volume", 0.5)
    expect(state.prepareApply(transaction)).toMatchObject({ ok: false, diagnostic: { code: "OwnerUnknown", owner: "audio" } })
    expect(state.snapshot().inFlightPlanId).toBeNull()
    state.setOwnerAvailability("audio", "unavailable")
    expect(state.prepareApply(transaction)).toMatchObject({ ok: false, diagnostic: { code: "OwnerUnavailable" } })
    state.setOwnerAvailability("audio", "available")
    expect(state.prepareApply(transaction).ok).toBe(true)
  })

  test("rejects stale identities and malformed settlement without mutation", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    expect(state.setValue(transaction + 1, "audio.master-volume", 0.5)).toMatchObject({ ok: false, diagnostic: { code: "StaleTransaction" } })
    expect(state.setValue(transaction, "missing", 0.5)).toMatchObject({ ok: false, diagnostic: { code: "UnknownSetting" } })
    expect(state.setValue(transaction, "audio.master-volume", 2)).toMatchObject({ ok: false, diagnostic: { code: "InvalidValue" } })
    state.setValue(transaction, "audio.master-volume", 0.5)
    const prepared = state.prepareApply(transaction)
    if (!prepared.ok) throw new Error(prepared.diagnostic.message)
    expect(state.settleApply(prepared.plan.planId, [])).toMatchObject({ ok: false, diagnostic: { code: "MalformedApplyResults" } })
    expect(state.snapshot()).toMatchObject({ current: { "audio.master-volume": 1 }, inFlightPlanId: prepared.plan.planId })
    const request = prepared.plan.requests[0]
    expect(state.settleApply(prepared.plan.planId, [
      { requestId: request.requestId, status: "applied" },
    ]).ok).toBe(true)
    expect(state.settleApply(prepared.plan.planId, [])).toMatchObject({ ok: false, diagnostic: { code: "StaleApplyPlan" } })
  })

  test("preserves retained unmuted volume and explicit mute independently", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    state.setValue(transaction, "audio.master-volume", 0.8)
    state.setValue(transaction, "audio.master-muted", true)
    const prepared = state.prepareApply(transaction)
    if (!prepared.ok) throw new Error(prepared.diagnostic.message)
    state.settleApply(prepared.plan.planId, prepared.plan.requests.map((request) => ({ requestId: request.requestId, status: "applied" })))
    expect(state.snapshot().current).toMatchObject({ "audio.master-volume": 0.8, "audio.master-muted": true })
    const unmute = begin(state)
    state.setValue(unmute, "audio.master-muted", false)
    expect(state.snapshot().pending).toMatchObject({ "audio.master-volume": 0.8, "audio.master-muted": false })
  })

  test("bounds the journal and supports explicit current synchronization only while idle", () => {
    const state = createSettingsState({
      catalog: fixtureCatalog("replace", { maximumJournalEntries: 3 }),
      owners: availableOwners(),
    })
    expect(state.synchronize({ "audio.master-volume": 0.9 }).ok).toBe(true)
    const transaction = begin(state)
    state.setValue(transaction, "audio.master-volume", 0.8)
    expect(state.synchronize({ "audio.master-volume": 0.7 })).toMatchObject({ ok: false, diagnostic: { code: "TransactionActive" } })
    state.setValue(transaction, "audio.master-muted", true)
    state.cancel(transaction)
    const snapshot = state.snapshot()
    expect(snapshot.journal).toHaveLength(3)
    expect(snapshot.journalStartSequence).toBe(snapshot.journal[0].sequence)
    expect(snapshot.current["audio.master-volume"]).toBe(0.9)
    expect(snapshot.applied["audio.master-volume"]).toBe(1)
  })

  test("commits a no-change transaction through an empty explicit plan", () => {
    const state = createSettingsState({ catalog: fixtureCatalog(), owners: availableOwners() })
    const transaction = begin(state)
    const prepared = state.prepareApply(transaction)
    if (!prepared.ok) throw new Error(prepared.diagnostic.message)
    expect(prepared.plan.requests).toEqual([])
    expect(state.settleApply(prepared.plan.planId, [])).toMatchObject({ ok: true, complete: true })
    expect(state.snapshot().activeTransactionId).toBeNull()
  })
})

describe("binding state", () => {
  test("replaces the prior action under the Source game-options policy", () => {
    const state = createSettingsState({ catalog: fixtureCatalog("replace"), owners: availableOwners() })
    const transaction = begin(state)
    const captured = state.captureBinding(transaction, "input.voice", { code: "spacebar" })
    expect(captured).toMatchObject({ ok: true, binding: { code: "space", modifiers: 0 }, displacedSettingId: "input.jump" })
    expect(state.snapshot().pending).toMatchObject({ "input.jump": null, "input.voice": { code: "space", modifiers: 0 } })
    expect(state.unbind(transaction, "input.voice").ok).toBe(true)
    expect(state.snapshot().pending?.["input.voice"]).toBeNull()
    expect(state.reset(transaction).ok).toBe(true)
    expect(state.snapshot().pending).toMatchObject({
      "input.jump": { code: "space", modifiers: 0 },
      "input.voice": { code: "key-k", modifiers: SOURCE_KEY_MODIFIERS.CONTROL },
    })
  })

  test("rejects exact chord conflicts under the VGUI editor policy", () => {
    const state = createSettingsState({ catalog: fixtureCatalog("reject"), owners: availableOwners() })
    const transaction = begin(state)
    expect(state.captureBinding(transaction, "input.voice", { code: "space" })).toMatchObject({
      ok: false,
      diagnostic: { code: "BindingConflict", conflictingSettingId: "input.jump" },
    })
    expect(state.captureBinding(transaction, "input.voice", { code: "space", shift: true }).ok).toBe(true)
    expect(state.captureBinding(transaction, "input.voice", { code: "escape" })).toMatchObject({ ok: false, diagnostic: { code: "ReservedBinding" } })
  })
})
