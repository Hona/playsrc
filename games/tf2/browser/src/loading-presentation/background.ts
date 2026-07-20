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
  material: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vmt", byteLength: 105, sha256: "3850088d15a9147bc593cab2bbda5bc12eff053ccaa8cec6579bf18513c695d1", providerIdentity: "tf2_misc_dir.vpk", providerRevision: "63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9" }),
  texture: Object.freeze({ logicalPath: "materials/vgui/stamp_background_map.vtf", byteLength: 1_398_360, sha256: "2f00d21971c788a51bd254ec5b69ad79af52caad35f0cde2a1ec9f4dbaf4a955", providerIdentity: "tf2_textures_dir.vpk", providerRevision: "291719bce05f0d82e6fb20961e631c0dd3967a7fe5b11cb374ed56c25312337e" }),
})

export const TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS = Object.freeze([
  "jump-beef-pak!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-00-workshop!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-01-tf2_textures_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-02-tf2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-03-tf2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-04-tf2_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-05-hl2_textures_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-06-hl2_sound_vo_english_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-07-hl2_sound_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-08-hl2_misc_dir.vpk!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-09-tf!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-10-hl2!materials/vgui/maps/menu_photos_jump_beef.vmt",
  "game-11-download!materials/vgui/maps/menu_photos_jump_beef.vmt",
] as const)

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
  const wide = input.viewport.width / input.viewport.height >= 1.5999
  return Object.freeze({
    ok: true,
    generation: input.generation,
    mapIdentity: input.mapIdentity,
    mapPhoto: found,
    backingMaterial: input.backingMaterial,
    backingTexture: input.backingTexture,
    checkedLocations: checked,
    backgroundWidth: wide ? input.viewport.width : input.viewport.height * (4 / 3),
    disposition: found ? "map-photo" : "configured-generic",
  })
}
