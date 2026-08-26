import { describe, expect, test } from "bun:test"
import { decodeSettingsPersistence, TF2_SELECTED_OPTIONS } from "@playsrc/settings"
import { initializeTf2BrowserSettings } from "../../src/settings-integration"

const owners = Object.freeze({
  renderer: "available",
  audio: "available",
  input: "available",
  game: "available",
  application: "available",
} as const)

describe("TF2 browser settings integration", () => {
  test("applies owner batches and emits one canonical persistence document", async () => {
    const requests: string[] = []
    const settings = initializeTf2BrowserSettings({
      persistence: null,
      owners,
      async apply(request) {
        requests.push(`${request.owner}:${request.changes.map((change) => change.settingId).join(",")}`)
        return Object.freeze({ requestId: request.requestId, status: "applied" as const })
      },
    })
    settings.begin()
    settings.set("audio.effect-volume", 0.65)
    settings.set("audio.music-volume", 0.35)
    settings.set("audio.master-muted", true)
    const applied = await settings.apply()
    expect(applied.lastApply?.complete).toBeTrue()
    expect(requests).toEqual(["audio:audio.effect-volume,audio.music-volume,audio.master-muted"])
    const decoded = decodeSettingsPersistence(TF2_SELECTED_OPTIONS, settings.persistence())
    expect(decoded.ok).toBeTrue()
    if (decoded.ok) {
      expect(decoded.decoded.values["audio.effect-volume"]).toBe(0.65)
      expect(decoded.decoded.values["audio.music-volume"]).toBe(0.35)
      expect(decoded.decoded.values["audio.master-muted"]).toBeTrue()
    }
  })

  test("retains decode and owner rejection as observable state without fallback", async () => {
    const settings = initializeTf2BrowserSettings({
      persistence: new TextEncoder().encode("not-json"),
      owners,
      async apply(request) {
        return Object.freeze({ requestId: request.requestId, status: "rejected" as const, reason: "owner effect unavailable" })
      },
    })
    expect(settings.snapshot().persistenceDiagnostic?.code).toBe("MalformedPersistence")
    settings.set("video.hdr", 1)
    const result = await settings.apply()
    expect(result.lastApply?.complete).toBeFalse()
    expect(result.lastApply?.rejectedOwners).toEqual(["renderer"])
    expect(result.settings.pending?.["video.hdr"]).toBe(1)
    expect(result.settings.current["video.hdr"]).toBe(0)
  })
})
