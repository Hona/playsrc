import type { ObjectDescriptor } from "@playsrc/asset-store"
import { TF2_CONFIGURED_STARTUP, validateTf2StartupDescriptor, type Tf2StartupDescriptor } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_MAP_LOADING, TF2_STAMP_BACKGROUND, type Tf2LoadingAsset } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_TARGET_NAMES, type Tf2TargetName } from "./deployment"

const HASH = /^[0-9a-f]{64}$/
const PRODUCTION_APPLICATION_ORIGIN = "https://playsrc.online"
const PRODUCTION_ASSET_ORIGIN = "https://assets.playsrc.online"

export type BrowserTargetConfiguration = Readonly<{
  target: Tf2TargetName
  contentBuild: string
  objects: Readonly<{ bsp: ObjectDescriptor; resources: ObjectDescriptor; dependencyLedger: ObjectDescriptor }>
  loading: Readonly<{
    mapPhotoLocations: (typeof TF2_MAP_LOADING)[Tf2TargetName]["photoLocations"]
    mapPhoto: (typeof TF2_MAP_LOADING)[Tf2TargetName]["photo"]
    stampBackground: Readonly<{ material: Tf2LoadingAsset; texture: Tf2LoadingAsset }>
  }>
}>

export type BrowserConfiguration = Readonly<{
  application: "tf2"
  applicationBuild: string
  defaultTarget: Tf2TargetName
  renderLevel: 0 | 1 | 2
  assetOrigin: string
  allowedExternalOrigins: readonly string[]
  wasm: ObjectDescriptor
  catalog: ObjectDescriptor
  targets: readonly BrowserTargetConfiguration[]
  startup: Tf2StartupDescriptor
  presentation: Readonly<{ randomSeed: number; activeHoliday: "none" | "summer" | "halloween" | "fullmoon" | "christmas"; activeWar: string | null; activeOperation: boolean; freeTrial: boolean }>
}>

export class BrowserConfigurationError extends Error {
  constructor(message: string) { super(message); this.name = "BrowserConfigurationError" }
}

export function parseBrowserConfiguration(value: unknown, applicationOrigin: string): BrowserConfiguration {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "allowedExternalOrigins\0application\0applicationBuild\0assetOrigin\0catalog\0defaultTarget\0presentation\0renderLevel\0startup\0targets\0wasm"
    || value.application !== "tf2"
    || typeof value.applicationBuild !== "string" || !HASH.test(value.applicationBuild)
    || !TF2_TARGET_NAMES.includes(value.defaultTarget as Tf2TargetName)
    || (value.renderLevel !== 0 && value.renderLevel !== 1 && value.renderLevel !== 2)
    || typeof value.assetOrigin !== "string" || !acceptedAssetOrigin(applicationOrigin, value.assetOrigin)
    || !Array.isArray(value.allowedExternalOrigins) || value.allowedExternalOrigins.length > 16
    || value.allowedExternalOrigins.some(invalidExternalOrigin)
    || new Set(value.allowedExternalOrigins).size !== value.allowedExternalOrigins.length
    || !descriptor(value.wasm, "derived-object")
    || !descriptor(value.catalog, "catalog", "application/vnd.playsrc.asset-catalog+json")
    || !Array.isArray(value.targets) || value.targets.length < 1 || value.targets.length > TF2_TARGET_NAMES.length
    || (applicationOrigin === PRODUCTION_APPLICATION_ORIGIN && value.targets.length !== TF2_TARGET_NAMES.length)
    || !validateTf2StartupDescriptor(value.startup).ok || JSON.stringify(value.startup) !== JSON.stringify(TF2_CONFIGURED_STARTUP)
    || !validPresentation(value.presentation)
  ) throw new BrowserConfigurationError("Browser configuration fields are invalid")

  const targets = value.targets.map((candidate) => {
    if (!record(candidate) || !TF2_TARGET_NAMES.includes(candidate.target as Tf2TargetName)) {
      throw new BrowserConfigurationError("Browser target configuration is invalid")
    }
    return parseTarget(candidate, candidate.target as Tf2TargetName)
  })
  if (targets.some((target, index) => index > 0
    && TF2_TARGET_NAMES.indexOf(targets[index - 1]!.target) >= TF2_TARGET_NAMES.indexOf(target.target))) {
    throw new BrowserConfigurationError("Browser target preparation order is invalid")
  }
  if (!targets.some((target) => target.target === value.defaultTarget)) {
    throw new BrowserConfigurationError("Browser default target has not been prepared")
  }
  for (const field of ["bsp", "resources", "dependencyLedger"] as const) {
    if (new Set(targets.map((target) => target.objects[field].sha256)).size !== targets.length) throw new BrowserConfigurationError(`Browser target ${field} identities are duplicated`)
  }
  return Object.freeze({ ...value, targets: Object.freeze(targets) } as BrowserConfiguration)
}

function parseTarget(value: unknown, target: Tf2TargetName): BrowserTargetConfiguration {
  const loading = TF2_MAP_LOADING[target]
  if (
    !record(value) || Object.keys(value).sort().join("\0") !== "contentBuild\0loading\0objects\0target"
    || value.target !== target || value.contentBuild !== "24245096"
    || !record(value.objects) || Object.keys(value.objects).sort().join("\0") !== "bsp\0dependencyLedger\0resources"
    || !descriptor(value.objects.bsp, "source-object")
    || !descriptor(value.objects.resources, "source-root", "application/vnd.playsrc.resource-graph+json")
    || !descriptor(value.objects.dependencyLedger, "derived-object", "application/vnd.playsrc.source-dependency-ledger+json")
    || !record(value.loading) || Object.keys(value.loading).sort().join("\0") !== "mapPhoto\0mapPhotoLocations\0stampBackground"
    || JSON.stringify(value.loading.mapPhotoLocations) !== JSON.stringify(loading.photoLocations)
    || JSON.stringify(value.loading.mapPhoto) !== JSON.stringify(loading.photo)
    || JSON.stringify(value.loading.stampBackground) !== JSON.stringify(TF2_STAMP_BACKGROUND)
  ) throw new BrowserConfigurationError("Browser target configuration is invalid")
  return Object.freeze(value as BrowserTargetConfiguration)
}

function invalidExternalOrigin(origin: unknown): boolean {
  if (typeof origin !== "string") return true
  try {
    const url = new URL(origin)
    return url.protocol !== "https:" || url.origin !== origin || Boolean(url.username || url.password || url.pathname !== "/" || url.search || url.hash)
  } catch { return true }
}

function validPresentation(value: unknown): boolean {
  return record(value)
    && Object.keys(value).sort().join("\0") === "activeHoliday\0activeOperation\0activeWar\0freeTrial\0randomSeed"
    && Number.isSafeInteger(value.randomSeed) && (value.randomSeed as number) >= -0x7fff_ffff && (value.randomSeed as number) <= 0x7fff_ffff
    && ["none", "summer", "halloween", "fullmoon", "christmas"].includes(value.activeHoliday as string)
    && (value.activeWar === null || (typeof value.activeWar === "string" && /^[a-z0-9_]{1,63}$/u.test(value.activeWar)))
    && typeof value.activeOperation === "boolean" && typeof value.freeTrial === "boolean"
}

function record(value: unknown): value is Record<string, unknown> { return typeof value === "object" && value !== null && !Array.isArray(value) }

function descriptor(value: unknown, kind: ObjectDescriptor["kind"], mediaType = "application/octet-stream"): value is ObjectDescriptor {
  if (!(record(value) && Object.keys(value).sort().join("\0") === "byteLength\0kind\0mediaType\0sha256" && value.kind === kind && value.mediaType === mediaType && typeof value.byteLength === "string" && /^(0|[1-9]\d*)$/.test(value.byteLength) && HASH.test(value.sha256 as string))) return false
  const length = Number(value.byteLength)
  return Number.isSafeInteger(length) && length >= 1 && length <= 536_870_912
}

export async function loadBrowserConfiguration(): Promise<BrowserConfiguration> {
  let response: Response
  try { response = await fetch(`${import.meta.env.BASE_URL}playsrc-config.json`, { cache: "no-store", credentials: "same-origin", redirect: "error" }) }
  catch { throw new BrowserConfigurationError("Browser configuration request failed") }
  if (response.status !== 200 || response.redirected) throw new BrowserConfigurationError("Browser configuration response failed")
  let value: unknown
  try { value = await response.json() } catch { throw new BrowserConfigurationError("Browser configuration is not JSON") }
  return parseBrowserConfiguration(value, window.location.origin)
}

function acceptedAssetOrigin(applicationOrigin: string, assetOrigin: string): boolean {
  return assetOrigin === applicationOrigin || (applicationOrigin === PRODUCTION_APPLICATION_ORIGIN && assetOrigin === PRODUCTION_ASSET_ORIGIN)
}
