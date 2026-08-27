import { describe, expect, test } from "bun:test"
import { resolveTf2LoadingBackground, TF2_MAP_LOADING, TF2_STAMP_BACKGROUND, type Tf2LoadingAsset } from "../../src/loading-presentation"

const missing = TF2_MAP_LOADING.jump_beef.photoLocations.map((location) => ({ location, outcome: "missing" as const }))
const input = (width = 1_280, height = 720) => ({ generation: 1, mapIdentity: "jump_beef", viewport: { width, height }, mapPhotoLookups: missing, backingMaterial: TF2_STAMP_BACKGROUND.material, backingTexture: TF2_STAMP_BACKGROUND.texture })

describe("TF2 loading background resolution", () => {
  test("records jump_beef's exact map-photo absence and configured generic background", () => {
    const result = resolveTf2LoadingBackground(input())
    expect(result).toEqual({
      ok: true,
      generation: 1,
      mapIdentity: "jump_beef",
      mapPhoto: null,
      backingMaterial: TF2_STAMP_BACKGROUND.material,
      backingTexture: TF2_STAMP_BACKGROUND.texture,
      checkedLocations: TF2_MAP_LOADING.jump_beef.photoLocations,
      backgroundWidth: 960,
      disposition: "configured-generic",
    })
  })

  test("selects the exact configured ctf_2fort map photo from its indexed provider", () => {
    const result = resolveTf2LoadingBackground({
      ...input(),
      mapIdentity: "ctf_2fort",
      mapPhotoLookups: TF2_MAP_LOADING.ctf_2fort.photoLocations.map((location) =>
        location.startsWith("game-04-tf2_misc_dir.vpk:")
          ? { location, outcome: "found" as const, asset: TF2_MAP_LOADING.ctf_2fort.photo!.material }
          : { location, outcome: "missing" as const }),
    })
    expect(result).toMatchObject({
      ok: true,
      mapIdentity: "ctf_2fort",
      mapPhoto: TF2_MAP_LOADING.ctf_2fort.photo!.material,
      disposition: "map-photo",
      checkedLocations: TF2_MAP_LOADING.ctf_2fort.photoLocations,
    })
    expect(TF2_MAP_LOADING.ctf_2fort.photo!.texture.sha256).toBe("1ec1d0a675522d3245e72817d83f9292ea9c60bcfde8d40bfe1b38eff2c889ad")
  })

  test("uses the first resolved mounted candidate and retains all checked locations", () => {
    const photo: Tf2LoadingAsset = { logicalPath: "materials/vgui/maps/menu_photos_cp_test.vmt", byteLength: 90, sha256: "1".repeat(64), providerIdentity: "map-pak", providerRevision: "map-hash" }
    const result = resolveTf2LoadingBackground({ ...input(1_024, 768), mapIdentity: "cp_test", mapPhotoLookups: [{ location: "map-pak!photo", outcome: "found", asset: photo }, { location: "game!photo", outcome: "missing" }] })
    expect(result).toMatchObject({ ok: true, mapPhoto: photo, disposition: "map-photo", checkedLocations: ["map-pak!photo", "game!photo"], backgroundWidth: 1_024 })
  })

  test("rejects malformed results and derives 4:3 width for tall viewports", () => {
    expect(resolveTf2LoadingBackground({ ...input(), mapPhotoLookups: [{ location: "pak!photo", outcome: "malformed" }] })).toEqual({ ok: false, generation: 1, code: "MalformedMapPhoto", subject: "pak!photo", checkedLocations: ["pak!photo"] })
    expect(resolveTf2LoadingBackground(input(390, 844))).toMatchObject({ ok: true, backgroundWidth: 1_125 })
  })
})
