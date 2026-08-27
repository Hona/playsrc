import type { ObjectDescriptor } from "@playsrc/asset-store"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { tf2MapLoading, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import type { BrowserConfiguration } from "./config"
import { TF2_TARGET_NAMES, tf2MapBsp, type Tf2TargetName } from "@playsrc/game-tf2-browser/maps"

const HASH = /^[0-9a-f]{64}$/

export const TF2_RELEASE_SCHEMA = "playsrc-tf2-release-v2" as const
export const TF2_APPLICATION_ORIGIN = "https://playsrc.online"
export const TF2_ASSET_ORIGIN = "https://assets.playsrc.online"

export type Tf2ReleaseTarget = Readonly<{
  target: Tf2TargetName
  contentBuild: typeof TF2_CONTENT_BUILD.contentBuild
  objects: Readonly<{
    bsp: ObjectDescriptor
    resources: ObjectDescriptor
    dependencyLedger: ObjectDescriptor
  }>
}>

export type Tf2Release = Readonly<{
  schema: typeof TF2_RELEASE_SCHEMA
  defaultTarget: Tf2TargetName
  objects: Readonly<{ wasm: ObjectDescriptor; catalog: ObjectDescriptor }>
  targets: readonly Tf2ReleaseTarget[]
}>

export function parseTf2Release(value: unknown): Tf2Release {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "defaultTarget\0objects\0schema\0targets"
    || value.schema !== TF2_RELEASE_SCHEMA
    || !TF2_TARGET_NAMES.includes(value.defaultTarget as Tf2TargetName)
    || !record(value.objects)
    || Object.keys(value.objects).sort().join("\0") !== "catalog\0wasm"
    || !Array.isArray(value.targets)
    || value.targets.length !== TF2_TARGET_NAMES.length
  ) throw new Error("TF2 release descriptor is malformed")

  const objects = Object.freeze({
    wasm: objectDescriptor(value.objects.wasm, "derived-object", "application/octet-stream"),
    catalog: objectDescriptor(value.objects.catalog, "catalog", "application/vnd.playsrc.asset-catalog+json"),
  })
  const targets = value.targets.map((candidate, index): Tf2ReleaseTarget => {
    const target = TF2_TARGET_NAMES[index]
    if (
      !record(candidate)
      || Object.keys(candidate).sort().join("\0") !== "contentBuild\0objects\0target"
      || candidate.target !== target
      || candidate.contentBuild !== TF2_CONTENT_BUILD.contentBuild
      || !record(candidate.objects)
      || Object.keys(candidate.objects).sort().join("\0") !== "bsp\0dependencyLedger\0resources"
    ) throw new Error("TF2 release target descriptor is malformed")
    const bsp = objectDescriptor(candidate.objects.bsp, "source-object", "application/octet-stream")
    const expectedBsp = tf2MapBsp(target)
    if (bsp.byteLength !== expectedBsp.byteLength || bsp.sha256 !== expectedBsp.sha256) {
      throw new Error("TF2 release BSP identity differs")
    }
    return Object.freeze({
      target,
      contentBuild: TF2_CONTENT_BUILD.contentBuild,
      objects: Object.freeze({
        bsp,
        resources: objectDescriptor(candidate.objects.resources, "source-root", "application/vnd.playsrc.resource-graph+json"),
        dependencyLedger: objectDescriptor(candidate.objects.dependencyLedger, "derived-object", "application/vnd.playsrc.source-dependency-ledger+json"),
      }),
    })
  })
  for (const field of ["bsp", "resources", "dependencyLedger"] as const) {
    if (new Set(targets.map((target) => target.objects[field].sha256)).size !== targets.length) {
      throw new Error(`TF2 release ${field} identities are duplicated`)
    }
  }
  return Object.freeze({
    schema: TF2_RELEASE_SCHEMA,
    defaultTarget: value.defaultTarget as Tf2TargetName,
    objects,
    targets: Object.freeze(targets),
  })
}

export function createDeployedBrowserConfiguration(release: Tf2Release, applicationBuild: string): BrowserConfiguration {
  if (!HASH.test(applicationBuild)) throw new Error("TF2 application build identity is malformed")
  return Object.freeze({
    application: "tf2",
    applicationBuild,
    defaultTarget: release.defaultTarget,
    renderLevel: 2,
    assetOrigin: TF2_ASSET_ORIGIN,
    allowedExternalOrigins: Object.freeze([]),
    wasm: release.objects.wasm,
    catalog: release.objects.catalog,
    targets: Object.freeze(release.targets.map((target) => Object.freeze({
      ...target,
      loading: loadingDescriptor(target.target),
    }))),
    startup: TF2_CONFIGURED_STARTUP,
    presentation: Object.freeze({ randomSeed: 0, activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false }),
  })
}

function loadingDescriptor(target: Tf2TargetName): BrowserConfiguration["targets"][number]["loading"] {
  return Object.freeze({
    mapPhotoLocations: tf2MapLoading(target).photoLocations,
    mapPhoto: tf2MapLoading(target).photo,
    stampBackground: TF2_STAMP_BACKGROUND,
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
  if (!Number.isSafeInteger(byteLength) || byteLength < 1 || byteLength > 536_870_912) throw new Error("TF2 release object byte length is outside its bound")
  return Object.freeze({ kind, mediaType, byteLength: value.byteLength, sha256: value.sha256 as string })
}
