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

const object = (kind: "source-object" | "derived-object" | "source-root", hash: string, mediaType = "application/octet-stream") => ({
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
    catalog: object("catalog", "c", "application/vnd.playsrc.asset-catalog+json"),
    dependencyLedger: object("derived-object", "e", "application/vnd.playsrc.source-dependency-ledger+json"),
  },
}

describe("TF2 production release", () => {
  test("keeps the HL2 landing-page teaser disabled", async () => {
    const landing = await readFile(new URL("../../index.html", import.meta.url), "utf8")
    expect(landing).toContain("<button type=\"button\" disabled>Half-Life 2</button>")
    expect(landing).not.toContain("href=\"/hl2\"")
  })

  test("admits threaded WASM, Blob-backed VGUI images, and the configured analytics beacon", async () => {
    const [headers, ignore, workflow, applicationPackage] = await Promise.all([
      readFile(new URL("../../_headers", import.meta.url), "utf8"),
      readFile(new URL("../../../../.gitignore", import.meta.url), "utf8"),
      readFile(new URL("../../../../.github/workflows/checks.yml", import.meta.url), "utf8"),
      readFile(new URL("../package.json", import.meta.url), "utf8"),
    ])
    expect(headers).toContain("Cross-Origin-Opener-Policy: same-origin")
    expect(headers).toContain("Cross-Origin-Embedder-Policy: require-corp")
    expect(headers).toContain("connect-src 'self' blob: https://assets.playsrc.online https://cloudflareinsights.com")
    expect(headers).toContain("script-src 'self' 'wasm-unsafe-eval' https://static.cloudflareinsights.com")
    expect(ignore).toContain("games/tf2/browser/src/wasm-generated/")
    expect(workflow).toContain("toolchain: nightly-2025-12-10")
    expect(workflow).toContain("cargo +1.97.1 install wasm-bindgen-cli --version 0.2.126 --locked")
    expect(workflow).toContain("bun run build:tf2-wasm-bindings")
    expect(applicationPackage).toContain("build-tf2-wasm-bindings.ts && vite build")
  })

  test("accepts the checked jump_beef release descriptor", () => {
    const parsed = parseTf2Release(checkedRelease)
    expect(parsed.objects.catalog.byteLength).toBe("286")
    expect(parsed.objects.catalog.sha256).toBe("165f0604e52e086dc52612dd205b15f6a685132bd01494a6ecaaae670478fb1b")
    expect(parsed.objects.dependencyLedger).toMatchObject({
      byteLength: "664374",
      sha256: "fe01f384c4e31bb8d6480a09abafeffbf29b8449cf1a772532c0e1e80776d2aa",
    })
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
