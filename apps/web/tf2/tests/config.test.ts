import { describe, expect, test } from "bun:test"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { BrowserConfigurationError, parseBrowserConfiguration } from "../src/config"

const object = (kind: "source-object" | "derived-object" | "source-root" | "catalog", value: string, mediaType = "application/octet-stream") => ({ kind, mediaType, byteLength: "1", sha256: value.repeat(64) })
const target = (name: "jump_beef" | "pl_upward", offset: number) => ({
  target: name,
  contentBuild: "24245096",
  objects: { bsp: object("source-object", String(offset)), resources: object("source-root", String(offset + 1), "application/vnd.playsrc.resource-graph+json"), dependencyLedger: object("derived-object", String(offset + 2), "application/vnd.playsrc.source-dependency-ledger+json") },
  loading: { mapPhotoLocations: name === "jump_beef" ? TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS : TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS, stampBackground: TF2_STAMP_BACKGROUND },
})
const valid = Object.freeze({
  application: "tf2", applicationBuild: "a".repeat(64), defaultTarget: "jump_beef", renderLevel: 2,
  assetOrigin: "http://127.0.0.1:4173", allowedExternalOrigins: ["https://allowed-host"],
  wasm: object("derived-object", "b"), catalog: object("catalog", "c", "application/vnd.playsrc.asset-catalog+json"),
  targets: [target("jump_beef", 1), target("pl_upward", 4)], startup: TF2_CONFIGURED_STARTUP,
  presentation: { randomSeed: 0, activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false },
})

describe("TF2 browser multi-map configuration", () => {
  test("accepts only the complete bounded target table", () => {
    expect(parseBrowserConfiguration(valid, valid.assetOrigin)).toEqual(valid)
    expect(parseBrowserConfiguration({ ...valid, assetOrigin: "https://assets.playsrc.online" }, "https://playsrc.online").targets).toHaveLength(2)
  })

  test("rejects old shape, target table mutations, bounds, origins and presentation changes", () => {
    const invalid = [
      { ...valid, target: "jump_beef", bsp: valid.targets[0].objects.bsp },
      { ...valid, targets: valid.targets.slice(0, 1) },
      { ...valid, targets: [valid.targets[0], valid.targets[0]] },
      { ...valid, targets: [valid.targets[1], valid.targets[0]] },
      { ...valid, targets: valid.targets.map((entry) => ({ ...entry, objects: { ...entry.objects, resources: valid.targets[0].objects.resources } })) },
      { ...valid, targets: [{ ...valid.targets[0], loading: valid.targets[1].loading }, valid.targets[1]] },
      { ...valid, wasm: { ...valid.wasm, byteLength: "536870913" } },
      { ...valid, assetOrigin: "https://other.invalid" },
      { ...valid, extra: true },
    ]
    for (const value of invalid) expect(() => parseBrowserConfiguration(value, valid.assetOrigin)).toThrow(BrowserConfigurationError)
  })
})
