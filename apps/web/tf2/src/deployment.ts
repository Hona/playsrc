import type { ObjectDescriptor } from "@playsrc/asset-store"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import type { BrowserConfiguration } from "./config"

const HASH = /^[0-9a-f]{64}$/

export const TF2_RELEASE_SCHEMA = "playsrc-tf2-release-v1" as const
export const TF2_APPLICATION_ORIGIN = "https://playsrc.online"
export const TF2_ASSET_ORIGIN = "https://assets.playsrc.online"

export type Tf2Release = Readonly<{
  schema: typeof TF2_RELEASE_SCHEMA
  target: "jump_beef"
  contentBuild: typeof TF2_CONTENT_BUILD.contentBuild
  objects: Readonly<{
    bsp: ObjectDescriptor
    wasm: ObjectDescriptor
    dependencies: ObjectDescriptor
    ui: ObjectDescriptor
    dependencyLedger: ObjectDescriptor
  }>
}>

export function parseTf2Release(value: unknown): Tf2Release {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "contentBuild\0objects\0schema\0target"
    || value.schema !== TF2_RELEASE_SCHEMA
    || value.target !== "jump_beef"
    || value.contentBuild !== TF2_CONTENT_BUILD.contentBuild
    || !record(value.objects)
    || Object.keys(value.objects).sort().join("\0") !== "bsp\0dependencies\0dependencyLedger\0ui\0wasm"
  ) throw new Error("TF2 release descriptor is malformed")

  const objects = Object.freeze({
    bsp: objectDescriptor(value.objects.bsp, "source-object", "application/octet-stream"),
    wasm: objectDescriptor(value.objects.wasm, "derived-object", "application/octet-stream"),
    dependencies: objectDescriptor(value.objects.dependencies, "derived-object", "application/octet-stream"),
    ui: objectDescriptor(value.objects.ui, "derived-object", "application/octet-stream"),
    dependencyLedger: objectDescriptor(
      value.objects.dependencyLedger,
      "derived-object",
      "application/vnd.playsrc.source-dependency-ledger+json",
    ),
  })

  return Object.freeze({
    schema: TF2_RELEASE_SCHEMA,
    target: "jump_beef",
    contentBuild: TF2_CONTENT_BUILD.contentBuild,
    objects,
  })
}

export function createDeployedBrowserConfiguration(
  release: Tf2Release,
  applicationBuild: string,
): BrowserConfiguration {
  if (!HASH.test(applicationBuild)) throw new Error("TF2 application build identity is malformed")
  return Object.freeze({
    application: "tf2",
    applicationBuild,
    target: "jump_beef",
    renderLevel: 2,
    assetOrigin: TF2_ASSET_ORIGIN,
    allowedExternalOrigins: Object.freeze([]),
    bsp: release.objects.bsp,
    wasm: release.objects.wasm,
    dependencies: release.objects.dependencies,
    ui: release.objects.ui,
    startup: TF2_CONFIGURED_STARTUP,
    loading: Object.freeze({
      mapPhotoLocations: TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS,
      stampBackground: TF2_STAMP_BACKGROUND,
    }),
    presentation: Object.freeze({
      randomSeed: 0,
      activeHoliday: "none",
      activeWar: null,
      activeOperation: false,
      freeTrial: false,
    }),
  })
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function objectDescriptor(value: unknown, kind: ObjectDescriptor["kind"], mediaType: string): ObjectDescriptor {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "byteLength\0kind\0mediaType\0sha256"
    || value.kind !== kind
    || value.mediaType !== mediaType
    || typeof value.byteLength !== "string"
    || !/^(0|[1-9]\d*)$/.test(value.byteLength)
    || !HASH.test(value.sha256 as string)
  ) throw new Error("TF2 release object descriptor is malformed")
  const byteLength = Number(value.byteLength)
  if (!Number.isSafeInteger(byteLength) || byteLength < 1 || byteLength > 536_870_912) {
    throw new Error("TF2 release object byte length is outside its bound")
  }
  return Object.freeze({ kind, mediaType, byteLength: value.byteLength, sha256: value.sha256 as string })
}
