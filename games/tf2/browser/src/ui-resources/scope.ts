import { TF2_CONTENT_BUILD } from "../content-build"
import { configuredTf2AuthoredScopeInput } from "./scope.generated"

const SHA256 = /^[0-9a-f]{64}$/u

export type Tf2ScopeSource = Readonly<{ logicalPath: string; byteLength: number; sha256: string }>
export type Tf2ScopeTexture = Readonly<{
  source: Tf2ScopeSource
  width: number
  height: number
  clampS: boolean
  clampT: boolean
  noLod: boolean
  encoding: "png" | "rgba-deflate"
  mips: readonly Readonly<{ sha256: string; data: string }>[]
}>
export type Tf2AuthoredScope = Readonly<{
  schema: "playsrc-tf2-authored-sniper-scope-v2"
  contentBuild: string
  quadrants: readonly Tf2ScopeSource[]
  chargeMaterial: Tf2ScopeSource
  tint: Tf2ScopeTexture
  normal: Tf2ScopeTexture
  chargeBase: Tf2ScopeTexture
  chargeMask: Tf2ScopeTexture
}>

function source(input: unknown, path: string): Tf2ScopeSource {
  const value = input as Record<string, unknown>
  if (!value || value.logicalPath !== path || !Number.isSafeInteger(value.byteLength)
    || (value.byteLength as number) <= 0 || typeof value.sha256 !== "string" || !SHA256.test(value.sha256)) {
    throw new Error(`TF2 authored Sniper scope source differs: ${path}`)
  }
  return Object.freeze({ logicalPath: path, byteLength: value.byteLength as number, sha256: value.sha256 })
}

function texture(input: unknown, path: string): Tf2ScopeTexture {
  const value = input as Record<string, unknown>
  if (!value || !Number.isSafeInteger(value.width) || !Number.isSafeInteger(value.height)
    || (value.width as number) <= 0 || (value.height as number) <= 0 || (value.width as number) > 1024
    || (value.height as number) > 1024 || typeof value.clampS !== "boolean" || typeof value.clampT !== "boolean" || typeof value.noLod !== "boolean"
    || value.encoding !== (path === "materials/hud/scope_normal_ul.vtf" ? "rgba-deflate" : "png")
    || !Array.isArray(value.mips) || value.mips.length < 1 || value.mips.length > 11) {
    throw new Error(`TF2 authored Sniper scope texture differs: ${path}`)
  }
  return Object.freeze({
    source: source(value.source, path),
    width: value.width as number,
    height: value.height as number,
    clampS: value.clampS,
    clampT: value.clampT,
    noLod: value.noLod,
    encoding: value.encoding as "png" | "rgba-deflate",
    mips: Object.freeze(value.mips.map((frame: Record<string, unknown>) => {
      const pattern = value.encoding === "png" ? /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/u : /^[A-Za-z0-9+/]+={0,2}$/u
      if (typeof frame.sha256 !== "string" || !SHA256.test(frame.sha256)
        || typeof frame.data !== "string" || !pattern.test(frame.data)) throw new Error(`TF2 scope mip differs: ${path}`)
      return Object.freeze({ sha256: frame.sha256, data: frame.data })
    })),
  })
}

export function createTf2AuthoredScope(input: unknown): Tf2AuthoredScope {
  const value = input as Record<string, unknown>
  if (!value || value.schema !== "playsrc-tf2-authored-sniper-scope-v2"
    || value.contentBuild !== TF2_CONTENT_BUILD.contentBuild || !Array.isArray(value.quadrants)
    || value.quadrants.length !== 4) throw new Error("TF2 authored Sniper scope descriptor is malformed")
  return Object.freeze({
    schema: "playsrc-tf2-authored-sniper-scope-v2",
    contentBuild: value.contentBuild,
    quadrants: Object.freeze((["ul", "ur", "lr", "ll"] as const)
      .map((suffix, index) => source(value.quadrants[index], `materials/hud/scope_sniper_${suffix}.vmt`))),
    chargeMaterial: source(value.chargeMaterial, "materials/hud/sniperscope_numbers.vmt"),
    tint: texture(value.tint, "materials/hud/scope_sniper_ul.vtf"),
    normal: texture(value.normal, "materials/hud/scope_normal_ul.vtf"),
    chargeBase: texture(value.chargeBase, "materials/hud/sniperscope_numbers.vtf"),
    chargeMask: texture(value.chargeMask, "materials/hud/sniperscope_numbers2.vtf"),
  })
}

export const tf2AuthoredScope = createTf2AuthoredScope(configuredTf2AuthoredScopeInput)
