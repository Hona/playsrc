import type { ObjectDescriptor } from "@playsrc/asset-store"

const HASH = /^[0-9a-f]{64}$/

export type BrowserConfiguration = Readonly<{
  application: "tf2"
  applicationBuild: string
  target: "jump_beef"
  assetOrigin: string
  allowedExternalOrigins: readonly string[]
  bsp: ObjectDescriptor
  wasm: ObjectDescriptor
}>

export class BrowserConfigurationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "BrowserConfigurationError"
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function descriptor(value: unknown, kind: "source-object" | "derived-object"): value is ObjectDescriptor {
  return record(value)
    && Object.keys(value).sort().join("\0") === "byteLength\0kind\0mediaType\0sha256"
    && value.kind === kind
    && value.mediaType === "application/octet-stream"
    && typeof value.byteLength === "string"
    && /^(0|[1-9]\d*)$/.test(value.byteLength)
    && HASH.test(value.sha256 as string)
}

export async function loadBrowserConfiguration(): Promise<BrowserConfiguration> {
  let response: Response
  try {
    response = await fetch("/playsrc-config.json", {
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
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "allowedExternalOrigins\0application\0applicationBuild\0assetOrigin\0bsp\0target\0wasm"
    || value.application !== "tf2"
    || typeof value.applicationBuild !== "string"
    || !HASH.test(value.applicationBuild)
    || value.target !== "jump_beef"
    || typeof value.assetOrigin !== "string"
    || value.assetOrigin !== window.location.origin
    || !Array.isArray(value.allowedExternalOrigins)
    || value.allowedExternalOrigins.length > 16
    || value.allowedExternalOrigins.some((origin) => {
      if (typeof origin !== "string") return true
      try {
        const url = new URL(origin)
        return url.protocol !== "https:"
          || url.origin !== origin
          || Boolean(url.username || url.password || url.pathname !== "/" || url.search || url.hash)
      } catch {
        return true
      }
    })
    || new Set(value.allowedExternalOrigins).size !== value.allowedExternalOrigins.length
    || !descriptor(value.bsp, "source-object")
    || !descriptor(value.wasm, "derived-object")
  ) {
    throw new BrowserConfigurationError("Browser configuration fields are invalid")
  }
  return Object.freeze(value as BrowserConfiguration)
}
