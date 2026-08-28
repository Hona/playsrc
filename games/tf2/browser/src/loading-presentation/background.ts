import { TF2_CONTENT_BUILD } from "../content-build"
import { TF2_MAPS, type Tf2TargetName } from "../maps"

export type Tf2LoadingAsset = Readonly<{
  logicalPath: string
  byteLength: number
  sha256: string
  providerIdentity: string
  providerRevision: string
}>

export type Tf2LoadingLookup = Readonly<{
  location: string
  outcome: "found" | "missing" | "malformed"
  asset?: Tf2LoadingAsset
}>

export type Tf2LoadingBackgroundInput = Readonly<{
  generation: number
  mapIdentity: string
  viewport: Readonly<{ width: number; height: number }>
  mapPhotoLookups: readonly Tf2LoadingLookup[]
  backingMaterial: Tf2LoadingAsset
  backingTexture: Tf2LoadingAsset
}>

export type Tf2LoadingBackgroundResult =
  | Readonly<{
      ok: true
      generation: number
      mapIdentity: string
      mapPhoto: Tf2LoadingAsset | null
      backingMaterial: Tf2LoadingAsset
      backingTexture: Tf2LoadingAsset
      checkedLocations: readonly string[]
      backgroundWidth: number
      disposition: "map-photo" | "configured-generic"
    }>
  | Readonly<{ ok: false; generation: number; code: "InvalidInput" | "MalformedMapPhoto"; subject: string; checkedLocations: readonly string[] }>

type MapLoading = Readonly<{
  photoLocations: readonly string[]
  photo: Readonly<{ material: Tf2LoadingAsset; texture: Tf2LoadingAsset }> | null
}>

const SHA256 = /^[0-9a-f]{64}$/u
const MAP = /^[a-z0-9][a-z0-9_-]{0,94}$/u
const asset = (value: Tf2LoadingAsset | undefined): value is Tf2LoadingAsset => Boolean(value
  && value.byteLength > 0 && Number.isSafeInteger(value.byteLength) && SHA256.test(value.sha256)
  && value.logicalPath.length > 0 && value.providerIdentity.length > 0 && value.providerRevision.length > 0)

export const TF2_STAMP_BACKGROUND = Object.freeze({
  material: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vmt", byteLength: 105, sha256: "3850088d15a9147bc593cab2bbda5bc12eff053ccaa8cec6579bf18513c695d1", providerIdentity: "tf2_misc_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Misc }),
  texture: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vtf", byteLength: 1_398_360, sha256: "2f00d21971c788a51bd254ec5b69ad79af52caad35f0cde2a1ec9f4dbaf4a955", providerIdentity: "tf2_textures_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Textures }),
})

function mapLoading(target: Tf2TargetName): MapLoading {
  const source = TF2_MAPS[target]
  const material = `materials/vgui/maps/menu_photos_${target}.vmt`
  const archives = ["tf2_textures", "tf2_sound_vo_english", "tf2_sound_misc", "tf2_misc", "hl2_textures", "hl2_sound_vo_english", "hl2_sound_misc", "hl2_misc"]
  const photoLocations = Object.freeze([
    `${source.pakProvider}:${source.logicalPath}!${material}`,
    `game-00-workshop:${material}`,
    ...archives.map((archive, index) => `game-0${index + 1}-${archive}_dir.vpk:${archive}_dir.vpk!${material}`),
    `game-09-tf:${material}`, `game-10-hl2:${material}`, `game-11-download:${material}`,
  ])
  const photo = source.loadingPhoto
  return Object.freeze({
    photoLocations,
    photo: photo ? Object.freeze({
      // Keep the deployed descriptor's canonical field order. Existing clients
      // validate these exact configured identities before generation recovery.
      material: Object.freeze({ logicalPath: material, byteLength: photo.material.byteLength, sha256: photo.material.sha256, providerIdentity: "tf2_misc_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Misc }),
      texture: Object.freeze({ logicalPath: `materials/vgui/maps/menu_photos_${target}.vtf`, byteLength: photo.texture.byteLength, sha256: photo.texture.sha256, providerIdentity: "tf2_textures_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Textures }),
    }) : null,
  })
}

export const TF2_MAP_LOADING = Object.freeze(Object.fromEntries(
  (Object.keys(TF2_MAPS) as Tf2TargetName[]).map((target) => [target, mapLoading(target)]),
)) as Readonly<Record<Tf2TargetName, MapLoading>>

export function tf2MapLoading(target: string): MapLoading {
  if (!Object.hasOwn(TF2_MAP_LOADING, target)) throw new Error(`Loading presentation is not declared for ${target}`)
  return TF2_MAP_LOADING[target as Tf2TargetName]
}

export function resolveTf2LoadingBackground(input: Tf2LoadingBackgroundInput): Tf2LoadingBackgroundResult {
  const checked = Object.freeze(input.mapPhotoLookups.map((lookup) => lookup.location))
  if (!Number.isSafeInteger(input.generation) || input.generation < 1 || !MAP.test(input.mapIdentity)
    || !Number.isSafeInteger(input.viewport.width) || input.viewport.width < 1
    || !Number.isSafeInteger(input.viewport.height) || input.viewport.height < 1
    || !asset(input.backingMaterial) || !asset(input.backingTexture) || input.mapPhotoLookups.length < 1) {
    return Object.freeze({ ok: false, generation: input.generation, code: "InvalidInput", subject: "background", checkedLocations: checked })
  }
  const malformed = input.mapPhotoLookups.find((lookup) => lookup.outcome === "malformed" || (lookup.outcome === "found" && !asset(lookup.asset)))
  if (malformed) return Object.freeze({ ok: false, generation: input.generation, code: "MalformedMapPhoto", subject: malformed.location, checkedLocations: checked })
  const found = input.mapPhotoLookups.find((lookup) => lookup.outcome === "found")?.asset ?? null
  return Object.freeze({
    ok: true,
    generation: input.generation,
    mapIdentity: input.mapIdentity,
    mapPhoto: found,
    backingMaterial: input.backingMaterial,
    backingTexture: input.backingTexture,
    checkedLocations: checked,
    backgroundWidth: Math.trunc(input.viewport.height * (4 / 3)),
    disposition: found ? "map-photo" : "configured-generic",
  })
}
