import { describe, expect, test } from "bun:test"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { BrowserConfigurationError, parseBrowserConfiguration } from "../src/config"

const object = (kind: "source-object" | "derived-object", value: string) => ({ kind, mediaType: "application/octet-stream", byteLength: "1", sha256: value.repeat(64) })
const valid = Object.freeze({
  application: "tf2",
  applicationBuild: "a".repeat(64),
  target: "jump_beef",
  renderLevel: 2,
  assetOrigin: "http://127.0.0.1:4173",
  allowedExternalOrigins: ["https://allowed-host"],
  bsp: object("source-object", "b"),
  wasm: object("derived-object", "c"),
  dependencies: object("derived-object", "d"),
  ui: object("derived-object", "e"),
  startup: TF2_CONFIGURED_STARTUP,
  loading: { mapPhotoLocations: TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, stampBackground: TF2_STAMP_BACKGROUND },
  presentation: { randomSeed: 0, activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false },
})

describe("TF2 browser startup and loading configuration", () => {
  test("accepts only the exact configured presentation descriptors", () => {
    expect(parseBrowserConfiguration(valid, valid.assetOrigin)).toEqual(valid)
    const deployed = { ...valid, assetOrigin: "https://assets.playsrc.online" }
    expect(parseBrowserConfiguration(deployed, "https://playsrc.online")).toEqual(deployed)
  })

  test("rejects changed media, loading sources, origins, and extra fields", () => {
    const invalid = [
      { ...valid, startup: { ...TF2_CONFIGURED_STARTUP, source: { ...TF2_CONFIGURED_STARTUP.source, sha256: "0".repeat(64) } } },
      { ...valid, loading: { ...valid.loading, mapPhotoLocations: valid.loading.mapPhotoLocations.slice(1) } },
      { ...valid, loading: { ...valid.loading, stampBackground: { ...TF2_STAMP_BACKGROUND, texture: { ...TF2_STAMP_BACKGROUND.texture, byteLength: 1 } } } },
      { ...valid, assetOrigin: "https://other.invalid" },
      { ...valid, extra: true },
    ]
    for (const value of invalid) expect(() => parseBrowserConfiguration(value, valid.assetOrigin)).toThrow(BrowserConfigurationError)
    expect(() => parseBrowserConfiguration(
      { ...valid, assetOrigin: "https://assets.playsrc.online" },
      "https://other.invalid",
    )).toThrow(BrowserConfigurationError)
  })
})
