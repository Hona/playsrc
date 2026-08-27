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

const SHA256 = /^[0-9a-f]{64}$/u
const MAP = /^[a-z0-9][a-z0-9_-]{0,94}$/u
const asset = (value: Tf2LoadingAsset | undefined): value is Tf2LoadingAsset => Boolean(value
  && value.byteLength > 0 && Number.isSafeInteger(value.byteLength) && SHA256.test(value.sha256)
  && value.logicalPath.length > 0 && value.providerIdentity.length > 0 && value.providerRevision.length > 0)

export const TF2_STAMP_BACKGROUND = Object.freeze({
  material: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vmt", byteLength: 105, sha256: "3850088d15a9147bc593cab2bbda5bc12eff053ccaa8cec6579bf18513c695d1", providerIdentity: "tf2_misc_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Misc }),
  texture: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vtf", byteLength: 1_398_360, sha256: "2f00d21971c788a51bd254ec5b69ad79af52caad35f0cde2a1ec9f4dbaf4a955", providerIdentity: "tf2_textures_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Textures }),
})

export const TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS = Object.freeze([
  "jump-beef-pak:maps/jump_beef.bsp!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-00-workshop:materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-01-tf2_textures_dir.vpk:tf2_textures_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-02-tf2_sound_vo_english_dir.vpk:tf2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-03-tf2_sound_misc_dir.vpk:tf2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-04-tf2_misc_dir.vpk:tf2_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-05-hl2_textures_dir.vpk:hl2_textures_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-06-hl2_sound_vo_english_dir.vpk:hl2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-07-hl2_sound_misc_dir.vpk:hl2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-08-hl2_misc_dir.vpk:hl2_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-09-tf:materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-10-hl2:materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-11-download:materials/vgui/maps/menu_photos_jump_beef.vmt",
] as const)

export const TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS = Object.freeze([
  "pl_upward-pak:maps/pl_upward.bsp!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-00-workshop:materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-01-tf2_textures_dir.vpk:tf2_textures_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-02-tf2_sound_vo_english_dir.vpk:tf2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-03-tf2_sound_misc_dir.vpk:tf2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-04-tf2_misc_dir.vpk:tf2_misc_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-05-hl2_textures_dir.vpk:hl2_textures_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-06-hl2_sound_vo_english_dir.vpk:hl2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-07-hl2_sound_misc_dir.vpk:hl2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-08-hl2_misc_dir.vpk:hl2_misc_dir.vpk!materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-09-tf:materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-10-hl2:materials/vgui/maps/menu_photos_pl_upward.vmt",
  "game-11-download:materials/vgui/maps/menu_photos_pl_upward.vmt",
] as const)

export const TF2_CTF_2FORT_MAP_PHOTO_LOCATIONS = Object.freeze([
  "ctf_2fort-pak:maps/ctf_2fort.bsp!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-00-workshop:materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-01-tf2_textures_dir.vpk:tf2_textures_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-02-tf2_sound_vo_english_dir.vpk:tf2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-03-tf2_sound_misc_dir.vpk:tf2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-04-tf2_misc_dir.vpk:tf2_misc_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-05-hl2_textures_dir.vpk:hl2_textures_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-06-hl2_sound_vo_english_dir.vpk:hl2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-07-hl2_sound_misc_dir.vpk:hl2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-08-hl2_misc_dir.vpk:hl2_misc_dir.vpk!materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-09-tf:materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-10-hl2:materials/vgui/maps/menu_photos_ctf_2fort.vmt",
  "game-11-download:materials/vgui/maps/menu_photos_ctf_2fort.vmt",
] as const)

export const TF2_CTF_2FORT_MAP_PHOTO = Object.freeze({
  material: Object.freeze({
    logicalPath: "materials/vgui/maps/menu_photos_ctf_2fort.vmt",
    byteLength: 126,
    sha256: "6c1228fd96a0f6029a924ea19d7801c9681db084742db8583e8f3d425056aac4",
    providerIdentity: "tf2_misc_dir.vpk",
    providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Misc,
  }),
  texture: Object.freeze({
    logicalPath: "materials/vgui/maps/menu_photos_ctf_2fort.vtf",
    byteLength: 349_784,
    sha256: "1ec1d0a675522d3245e72817d83f9292ea9c60bcfde8d40bfe1b38eff2c889ad",
    providerIdentity: "tf2_textures_dir.vpk",
    providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Textures,
  }),
})

export const TF2_MAP_LOADING = Object.freeze({
  jump_beef: Object.freeze({ photoLocations: TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, photo: null }),
  pl_upward: Object.freeze({ photoLocations: TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS, photo: null }),
  ctf_2fort: Object.freeze({ photoLocations: TF2_CTF_2FORT_MAP_PHOTO_LOCATIONS, photo: TF2_CTF_2FORT_MAP_PHOTO }),
  koth_viaduct: Object.freeze({
    photoLocations: Object.freeze([
      "koth_viaduct-pak:maps/koth_viaduct.bsp!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-00-workshop:materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-01-tf2_textures_dir.vpk:tf2_textures_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-02-tf2_sound_vo_english_dir.vpk:tf2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-03-tf2_sound_misc_dir.vpk:tf2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-04-tf2_misc_dir.vpk:tf2_misc_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-05-hl2_textures_dir.vpk:hl2_textures_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-06-hl2_sound_vo_english_dir.vpk:hl2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-07-hl2_sound_misc_dir.vpk:hl2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-08-hl2_misc_dir.vpk:hl2_misc_dir.vpk!materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-09-tf:materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-10-hl2:materials/vgui/maps/menu_photos_koth_viaduct.vmt",
      "game-11-download:materials/vgui/maps/menu_photos_koth_viaduct.vmt",
    ]),
    photo: Object.freeze({
      material: Object.freeze({ logicalPath: "materials/vgui/maps/menu_photos_koth_viaduct.vmt", byteLength: 129, sha256: "2f8dddeef0cff874e22ed4b58909fc5bf44b67fa0da9d55f1cd4956d22751ba4", providerIdentity: "tf2_misc_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Misc }),
      texture: Object.freeze({ logicalPath: "materials/vgui/maps/menu_photos_koth_viaduct.vtf", byteLength: 349784, sha256: "af246c72096fc065b5b5a2c0cd617638f31e783067104315ae7ffde691b1e70a", providerIdentity: "tf2_textures_dir.vpk", providerRevision: TF2_CONTENT_BUILD.archiveIndexes.tf2Textures }),
    }),
  }),
})

export function tf2MapLoading(target: string): Readonly<{
  photoLocations: readonly string[]
  photo: Readonly<{ material: Tf2LoadingAsset; texture: Tf2LoadingAsset }> | null
}> {
  const descriptor = Object.hasOwn(TF2_MAP_LOADING, target) ? TF2_MAP_LOADING[target as keyof typeof TF2_MAP_LOADING] : undefined
  if (!descriptor) throw new Error(`Loading presentation is not admitted for ${target}`)
  return descriptor
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
import { TF2_CONTENT_BUILD } from "../content-build"
