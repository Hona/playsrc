import { describe, expect, test } from "bun:test"
import { readFile } from "node:fs/promises"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import {
  createDeployedBrowserConfiguration,
  parseTf2Release,
  TF2_ASSET_ORIGIN,
  TF2_RELEASE_SCHEMA,
} from "../src/deployment"
import checkedRelease from "../releases/jump_beef.json"

const object = (kind: "source-object" | "derived-object", hash: string, mediaType = "application/octet-stream") => ({
  kind,
  mediaType,
  byteLength: "1",
  sha256: hash.repeat(64),
})

const release = {
  schema: TF2_RELEASE_SCHEMA,
  target: "jump_beef",
  contentBuild: TF2_CONTENT_BUILD.contentBuild,
  objects: {
    bsp: object("source-object", "a"),
    wasm: object("derived-object", "b"),
    dependencies: object("derived-object", "c"),
    ui: object("derived-object", "d"),
    dependencyLedger: object("derived-object", "e", "application/vnd.playsrc.source-dependency-ledger+json"),
  },
}

describe("TF2 production release", () => {
  test("admits Blob-backed VGUI images and the configured analytics beacon", async () => {
    const headers = await readFile(new URL("../../_headers", import.meta.url), "utf8")
    expect(headers).toContain("connect-src 'self' blob: https://assets.playsrc.online https://cloudflareinsights.com")
    expect(headers).toContain("script-src 'self' 'wasm-unsafe-eval' https://static.cloudflareinsights.com")
  })

  test("accepts the checked jump_beef release descriptor", () => {
    const parsed = parseTf2Release(checkedRelease)
    expect(parsed.objects.dependencies.byteLength).toBe("256997450")
    expect(parsed.objects.ui.byteLength).toBe("36564904")
  })

  test("builds one exact cross-origin browser configuration", () => {
    const parsed = parseTf2Release(release)
    const configuration = createDeployedBrowserConfiguration(parsed, "f".repeat(64))
    expect(configuration.assetOrigin).toBe(TF2_ASSET_ORIGIN)
    expect(configuration.allowedExternalOrigins).toEqual([])
    expect(configuration.bsp).toEqual(release.objects.bsp)
  })

  test("rejects changed release shape and descriptor media", () => {
    expect(() => parseTf2Release({ ...release, extra: true })).toThrow()
    expect(() => parseTf2Release({
      ...release,
      objects: { ...release.objects, wasm: { ...release.objects.wasm, mediaType: "application/wasm" } },
    })).toThrow()
  })
})
