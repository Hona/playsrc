import { describe, expect, spyOn, test } from "bun:test"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_MAP_LOADING, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { BrowserConfigurationError, loadBrowserConfiguration, parseBrowserConfiguration, tf2SelectableMapNames } from "../src/config"
import { tf2MapBsp } from "@playsrc/game-tf2-browser/maps"

const object = (kind: "source-object" | "derived-object" | "source-root" | "catalog", value: string, mediaType = "application/octet-stream") => ({ kind, mediaType, byteLength: "1", sha256: value.repeat(64) })
const target = (name: keyof typeof TF2_MAP_LOADING, offset: number) => ({
  target: name,
  contentBuild: "24245096",
  objects: { bsp: { ...object("source-object", String(offset)), ...tf2MapBsp(name) }, resources: object("source-root", String(offset + 1), "application/vnd.playsrc.resource-graph+json"), dependencyLedger: object("derived-object", String(offset + 2), "application/vnd.playsrc.source-dependency-ledger+json") },
  loading: { mapPhotoLocations: TF2_MAP_LOADING[name].photoLocations, mapPhoto: TF2_MAP_LOADING[name].photo, stampBackground: TF2_STAMP_BACKGROUND },
})
const valid = Object.freeze({
  application: "tf2", applicationBuild: "a".repeat(64), defaultTarget: "jump_beef", renderLevel: 2,
  assetOrigin: "http://127.0.0.1:4173", allowedExternalOrigins: ["https://allowed-host"],
  wasm: object("derived-object", "b"), catalog: object("catalog", "c", "application/vnd.playsrc.asset-catalog+json"),
  targets: [target("jump_beef", 1), target("pl_upward", 4), target("ctf_2fort", 7)], startup: TF2_CONFIGURED_STARTUP,
  presentation: { randomSeed: 0, activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false },
})

describe("TF2 browser multi-map configuration", () => {
  test("cache-busts the single configuration endpoint with the bundled release identity", async () => {
    const base = process.env.BASE_URL
    const windowDescriptor = Object.getOwnPropertyDescriptor(globalThis, "window")
    process.env.BASE_URL = "/tf2/"
    Object.defineProperty(globalThis, "window", { configurable: true, value: { location: { origin: valid.assetOrigin } } })
    const request = spyOn(globalThis, "fetch").mockImplementation(async () => Response.json(valid))
    try {
      await loadBrowserConfiguration("a".repeat(64))
      await loadBrowserConfiguration("b".repeat(64))
      expect(request.mock.calls).toEqual(["a", "b"].map(build => [
        `/tf2/playsrc-config.json?v=${build.repeat(64)}`,
        { cache: "no-store", credentials: "same-origin", redirect: "error" },
      ]))
      await expect(loadBrowserConfiguration("invalid")).rejects.toThrow(BrowserConfigurationError)
      expect(request).toHaveBeenCalledTimes(2)
    } finally {
      request.mockRestore()
      if (base === undefined) delete process.env.BASE_URL
      else process.env.BASE_URL = base
      if (windowDescriptor) Object.defineProperty(globalThis, "window", windowDescriptor)
      else Reflect.deleteProperty(globalThis, "window")
    }
  })

  test("admits explicitly prepared Viaduct only for local integration", () => {
    const integration = { ...valid, defaultTarget: "koth_viaduct", targets: [target("koth_viaduct", 1)] }
    const configuration = parseBrowserConfiguration(integration, valid.assetOrigin)
    expect(configuration.targets[0]!.target).toBe("koth_viaduct")
    expect(tf2SelectableMapNames(configuration, valid.assetOrigin)).toContain("koth_viaduct")
    expect(tf2SelectableMapNames(configuration, valid.assetOrigin)).not.toContain("cp_badlands")
    expect(() => parseBrowserConfiguration({ ...integration, assetOrigin: "https://assets.playsrc.online" }, "https://playsrc.online")).toThrow(BrowserConfigurationError)
  })

  test("admits explicitly prepared Badlands only for local integration", () => {
    const integration = { ...valid, defaultTarget: "cp_badlands", targets: [target("cp_badlands", 1)] }
    expect(parseBrowserConfiguration(integration, valid.assetOrigin).targets[0]!.target).toBe("cp_badlands")
    expect(() => parseBrowserConfiguration({ ...integration, assetOrigin: "https://assets.playsrc.online" }, "https://playsrc.online")).toThrow(BrowserConfigurationError)
  })

  test("accepts only the complete bounded target table", () => {
    const configuration = parseBrowserConfiguration(valid, valid.assetOrigin)
    expect(configuration).toEqual(valid)
    expect(configuration.targets[2]!.loading.mapPhoto?.material.logicalPath).toBe("materials/vgui/maps/menu_photos_ctf_2fort.vmt")
    expect(parseBrowserConfiguration({ ...valid, assetOrigin: "https://assets.playsrc.online" }, "https://playsrc.online").targets).toHaveLength(3)
    expect(parseBrowserConfiguration({ ...valid, targets: valid.targets.slice(0, 1) }, valid.assetOrigin).targets).toHaveLength(1)
    const published = parseBrowserConfiguration({ ...valid, assetOrigin: "https://assets.playsrc.online", targets: valid.targets.slice(0, 1) }, "https://playsrc.online")
    expect(tf2SelectableMapNames(published, "https://playsrc.online")).toEqual(["jump_beef"])
    expect(parseBrowserConfiguration({
      ...valid, assetOrigin: "https://assets.playsrc.online", targets: valid.targets.slice(0, 1),
    }, "https://playsrc.online").targets).toHaveLength(1)
  })

  test("rejects old shape, target table mutations, bounds, origins and presentation changes", () => {
    const invalid = [
      { ...valid, target: "jump_beef", bsp: valid.targets[0].objects.bsp },
      { ...valid, targets: [] },
      { ...valid, defaultTarget: "pl_upward", targets: valid.targets.slice(0, 1) },
      { ...valid, targets: [valid.targets[0], valid.targets[0]] },
      { ...valid, targets: [valid.targets[1], valid.targets[0]] },
      { ...valid, targets: valid.targets.map((entry) => ({ ...entry, objects: { ...entry.objects, resources: valid.targets[0].objects.resources } })) },
      { ...valid, targets: [{ ...valid.targets[0], loading: valid.targets[1].loading }, valid.targets[1], valid.targets[2]] },
      { ...valid, wasm: { ...valid.wasm, byteLength: "536870913" } },
      { ...valid, targets: [{ ...valid.targets[0], objects: { ...valid.targets[0].objects, bsp: object("source-object", "f") } }] },
      { ...valid, assetOrigin: "https://other.invalid" },
      { ...valid, extra: true },
    ]
    for (const value of invalid) expect(() => parseBrowserConfiguration(value, valid.assetOrigin)).toThrow(BrowserConfigurationError)
  })
})
