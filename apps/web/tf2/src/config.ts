import type { ObjectDescriptor } from "@playsrc/asset-store"
import { TF2_CONFIGURED_STARTUP, validateTf2StartupDescriptor, type Tf2StartupDescriptor } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND, type Tf2LoadingAsset } from "@playsrc/game-tf2-browser/loading-presentation"

const HASH = /^[0-9a-f]{64}$/
const PRODUCTION_APPLICATION_ORIGIN = "https://playsrc.online"
const PRODUCTION_ASSET_ORIGIN = "https://assets.playsrc.online"

export type BrowserConfiguration = Readonly<{
  application: "tf2"
  applicationBuild: string
  target: "jump_beef"
  renderLevel: 0 | 1 | 2
  assetOrigin: string
  allowedExternalOrigins: readonly string[]
  bsp: ObjectDescriptor
  wasm: ObjectDescriptor
  catalog: ObjectDescriptor
  startup: Tf2StartupDescriptor
  loading: Readonly<{
    mapPhotoLocations: typeof TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS
    stampBackground: Readonly<{ material: Tf2LoadingAsset; texture: Tf2LoadingAsset }>
  }>
  presentation: Readonly<{
    randomSeed: number
    activeHoliday: "none" | "summer" | "halloween" | "fullmoon" | "christmas"
    activeWar: string | null
    activeOperation: boolean
    freeTrial: boolean
  }>
}>

export class BrowserConfigurationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "BrowserConfigurationError"
  }
}

export function parseBrowserConfiguration(value: unknown, applicationOrigin: string): BrowserConfiguration {
  if (
    !record(value) ||
    Object.keys(value).sort().join("\0") !==
      "allowedExternalOrigins\0application\0applicationBuild\0assetOrigin\0bsp\0catalog\0loading\0presentation\0renderLevel\0startup\0target\0wasm" ||
    value.application !== "tf2" ||
    typeof value.applicationBuild !== "string" ||
    !HASH.test(value.applicationBuild) ||
    value.target !== "jump_beef" ||
    (value.renderLevel !== 0 && value.renderLevel !== 1 && value.renderLevel !== 2) ||
    typeof value.assetOrigin !== "string" ||
    !acceptedAssetOrigin(applicationOrigin, value.assetOrigin) ||
    !Array.isArray(value.allowedExternalOrigins) ||
    value.allowedExternalOrigins.length > 16 ||
    value.allowedExternalOrigins.some((origin) => {
      if (typeof origin !== "string") return true
      try {
        const url = new URL(origin)
        return (
          url.protocol !== "https:" ||
          url.origin !== origin ||
          Boolean(url.username || url.password || url.pathname !== "/" || url.search || url.hash)
        )
      } catch {
        return true
      }
    }) ||
    new Set(value.allowedExternalOrigins).size !== value.allowedExternalOrigins.length ||
    !descriptor(value.bsp, "source-object") ||
    !descriptor(value.wasm, "derived-object") ||
    !descriptor(value.catalog, "catalog", "application/vnd.playsrc.asset-catalog+json") ||
    !validateTf2StartupDescriptor(value.startup).ok ||
    JSON.stringify(value.startup) !== JSON.stringify(TF2_CONFIGURED_STARTUP) ||
    !record(value.loading) ||
    Object.keys(value.loading).sort().join("\0") !== "mapPhotoLocations\0stampBackground" ||
    JSON.stringify(value.loading.mapPhotoLocations) !== JSON.stringify(TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS) ||
    JSON.stringify(value.loading.stampBackground) !== JSON.stringify(TF2_STAMP_BACKGROUND) ||
    !record(value.presentation) ||
    Object.keys(value.presentation).sort().join("\0") !== "activeHoliday\0activeOperation\0activeWar\0freeTrial\0randomSeed" ||
    !Number.isSafeInteger(value.presentation.randomSeed) ||
    (value.presentation.randomSeed as number) < -0x7fff_ffff ||
    (value.presentation.randomSeed as number) > 0x7fff_ffff ||
    !["none", "summer", "halloween", "fullmoon", "christmas"].includes(value.presentation.activeHoliday as string) ||
    !(value.presentation.activeWar === null || (typeof value.presentation.activeWar === "string" && /^[a-z0-9_]{1,63}$/u.test(value.presentation.activeWar))) ||
    typeof value.presentation.activeOperation !== "boolean" ||
    typeof value.presentation.freeTrial !== "boolean"
  ) throw new BrowserConfigurationError("Browser configuration fields are invalid")
  return Object.freeze(value as BrowserConfiguration)
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function descriptor(value: unknown, kind: "source-object" | "derived-object" | "source-root" | "catalog", mediaType = "application/octet-stream"): value is ObjectDescriptor {
  return (
    record(value) &&
    Object.keys(value).sort().join("\0") === "byteLength\0kind\0mediaType\0sha256" &&
    value.kind === kind &&
    value.mediaType === mediaType &&
    typeof value.byteLength === "string" &&
    /^(0|[1-9]\d*)$/.test(value.byteLength) &&
    HASH.test(value.sha256 as string)
  )
}

export async function loadBrowserConfiguration(): Promise<BrowserConfiguration> {
  let response: Response
  try {
    response = await fetch(`${import.meta.env.BASE_URL}playsrc-config.json`, {
      cache: "no-store",
      credentials: "same-origin",
      redirect: "error",
    })
  } catch {
    throw new BrowserConfigurationError("Browser configuration request failed")
  }
  if (response.status !== 200 || response.redirected) {
    throw new BrowserConfigurationError("Browser configuration response failed")
  }
  let value: unknown
  try {
    value = await response.json()
  } catch {
    throw new BrowserConfigurationError("Browser configuration is not JSON")
  }
  return parseBrowserConfiguration(value, window.location.origin)
}

function acceptedAssetOrigin(applicationOrigin: string, assetOrigin: string): boolean {
  return assetOrigin === applicationOrigin
    || (applicationOrigin === PRODUCTION_APPLICATION_ORIGIN && assetOrigin === PRODUCTION_ASSET_ORIGIN)
}
