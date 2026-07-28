import { describe, expect, test } from "bun:test"
import { readFile } from "node:fs/promises"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { createDeployedBrowserConfiguration, parseTf2Release, TF2_ASSET_ORIGIN, TF2_RELEASE_SCHEMA } from "../src/deployment"
import checkedRelease from "../releases/current.json"

const object = (kind: "source-object" | "derived-object" | "source-root" | "catalog", hash: string, mediaType = "application/octet-stream", byteLength = "1") => ({ kind, mediaType, byteLength, sha256: hash.length === 64 ? hash : hash.repeat(64) })
const release = {
  schema: TF2_RELEASE_SCHEMA,
  defaultTarget: "jump_beef",
  objects: { wasm: object("derived-object", "a"), catalog: object("catalog", "b", "application/vnd.playsrc.asset-catalog+json") },
  targets: [
    { target: "jump_beef", contentBuild: TF2_CONTENT_BUILD.contentBuild, objects: { bsp: object("source-object", "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959", "application/octet-stream", "33379388"), resources: object("source-root", "c", "application/vnd.playsrc.resource-graph+json"), dependencyLedger: object("derived-object", "d", "application/vnd.playsrc.source-dependency-ledger+json") } },
    { target: "pl_upward", contentBuild: TF2_CONTENT_BUILD.contentBuild, objects: { bsp: object("source-object", "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709", "application/octet-stream", "25446018"), resources: object("source-root", "e", "application/vnd.playsrc.resource-graph+json"), dependencyLedger: object("derived-object", "f", "application/vnd.playsrc.source-dependency-ledger+json") } },
  ],
}

describe("TF2 production release", () => {
  test("keeps the HL2 landing-page teaser disabled", async () => {
    const landing = await readFile(new URL("../../index.html", import.meta.url), "utf8")
    expect(landing).toContain("<button type=\"button\" disabled>Half-Life 2</button>")
  })

  test("admits the checked dual-map descriptor", () => {
    const parsed = parseTf2Release(checkedRelease)
    expect(parsed.defaultTarget).toBe("jump_beef")
    expect(parsed.targets.map((target) => target.target)).toEqual(["jump_beef", "pl_upward"])
    expect(parsed.targets[1].objects.bsp.sha256).toBe("15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709")
  })

  test("builds one exact dual-map browser configuration", () => {
    const configuration = createDeployedBrowserConfiguration(parseTf2Release(release), "f".repeat(64))
    expect(configuration.assetOrigin).toBe(TF2_ASSET_ORIGIN)
    expect(configuration.defaultTarget).toBe("jump_beef")
    expect(configuration.targets.map((target) => target.target)).toEqual(["jump_beef", "pl_upward"])
  })

  test("rejects old shape, missing, duplicate, swapped, malformed and changed descriptors", () => {
    const malformed = [
      { schema: "playsrc-tf2-release-v1", target: "jump_beef", contentBuild: TF2_CONTENT_BUILD.contentBuild, objects: {} },
      { ...release, targets: release.targets.slice(0, 1) },
      { ...release, targets: [release.targets[0], release.targets[0]] },
      { ...release, targets: [release.targets[1], release.targets[0]] },
      { ...release, targets: release.targets.map((target) => ({ ...target, objects: { ...target.objects, resources: release.targets[0].objects.resources } })) },
      { ...release, objects: { ...release.objects, wasm: { ...release.objects.wasm, byteLength: "536870913" } } },
      { ...release, objects: { ...release.objects, wasm: { ...release.objects.wasm, mediaType: "application/wasm" } } },
    ]
    for (const value of malformed) expect(() => parseTf2Release(value)).toThrow()
  })
})
