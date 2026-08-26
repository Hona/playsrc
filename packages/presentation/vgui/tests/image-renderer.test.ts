import { describe, expect, test } from "bun:test"
import { isDirectVguiImageMaterial, shadeVguiImage, VguiRasterCache, type VguiImageRasterRequest, type VguiImageRasterTexturePixels } from "../src/image-renderer"
import type { VguiImageMaterialPresentation, VguiImageMaterialTexture } from "../src"

const texture = (identity: string): VguiImageMaterialTexture => Object.freeze({
  logicalIdentity: identity,
  revision: `${identity}-1`,
  browserUrl: `memory:${identity}`,
  width: 1,
  height: 1,
  hardwareFiltered: false,
  colorRead: "linear",
})

describe("bounded authored VGUI raster ownership", () => {
  test("evicts least-recently-used decoded buffers by exact retained bytes", () => {
    const cache = new VguiRasterCache<Uint8Array>(12, 3)
    const first = new Uint8Array(4)
    const second = new Uint8Array(4)
    const third = new Uint8Array(4)
    cache.set("first", first, first.byteLength)
    cache.set("second", second, second.byteLength)
    cache.set("third", third, third.byteLength)
    expect(cache.get("first")).toBe(first)
    cache.set("fourth", new Uint8Array(4), 4)
    expect(cache.get("second")).toBeUndefined()
    expect(cache.get("first")).toBe(first)
    expect(cache.snapshot()).toEqual({ entries: 3, bytes: 12 })
    cache.set("oversized", new Uint8Array(16), 16)
    expect(cache.get("oversized")).toBeUndefined()
    expect(cache.snapshot()).toEqual({ entries: 3, bytes: 12 })
    cache.clear()
    expect(cache.snapshot()).toEqual({ entries: 0, bytes: 0 })
  })

  test("bounds identities independently of decoded byte budget", () => {
    const cache = new VguiRasterCache<number>(100, 2)
    cache.set("first", 1, 1)
    cache.set("second", 2, 1)
    cache.set("third", 3, 1)
    expect(cache.get("first")).toBeUndefined()
    expect(cache.snapshot()).toEqual({ entries: 2, bytes: 2 })
  })
})

const baseMaterial = (overrides: Partial<VguiImageMaterialPresentation> = {}): VguiImageMaterialPresentation => Object.freeze({
  shader: "unlit-generic",
  base: texture("base"),
  second: null,
  detail: null,
  detailScale: [1, 1],
  detailBlendMode: 0,
  detailBlendFactor: 1,
  detailTint: Object.freeze([1, 1, 1]),
  distanceAlpha: false,
  distanceAlphaFromDetail: false,
  softEdges: false,
  scaleSoftEdges: false,
  edgeSoftnessStart: 0.6,
  edgeSoftnessEnd: 0.5,
  outline: false,
  outlineColor: Object.freeze([1, 1, 1]),
  outlineAlpha: 0,
  outlineStart0: 0,
  outlineStart1: 0,
  outlineEnd0: 0,
  outlineEnd1: 0,
  scaleOutline: false,
  glow: false,
  glowColor: Object.freeze([1, 1, 1]),
  glowAlpha: 1,
  glowStart: 0.7,
  glowEnd: 0.5,
  glowX: 0,
  glowY: 0,
  ...overrides,
})

const pixels = (rgba: readonly number[]): VguiImageRasterTexturePixels => Object.freeze({
  width: 1,
  height: 1,
  rgba: new Uint8ClampedArray(rgba),
  filtered: false,
  colorRead: "linear",
})

const request = (material: VguiImageMaterialPresentation): VguiImageRasterRequest => Object.freeze({
  width: 1,
  height: 1,
  viewportWidth: 1024,
  viewportHeight: 768,
  tint: Object.freeze([255, 255, 255, 255]),
  geometry: Object.freeze({ kind: "stretch", rotation: 0 }),
  material,
})

describe("configured VGUI image material raster", () => {
  test("uses authored sRGB textures directly only when the material has no shader effects", () => {
    const srgb = Object.freeze({ ...texture("base"), colorRead: "srgb" as const })
    const direct = baseMaterial({ base: srgb })
    expect(isDirectVguiImageMaterial(direct)).toBeTrue()
    expect(isDirectVguiImageMaterial(baseMaterial())).toBeFalse()
    for (const override of [
      { shader: "unlit-two-texture" as const },
      { second: texture("second") },
      { detail: texture("detail") },
      { distanceAlpha: true },
      { distanceAlphaFromDetail: true },
      { softEdges: true },
      { outline: true },
      { glow: true },
    ]) expect(isDirectVguiImageMaterial(baseMaterial({ base: srgb, ...override }))).toBeFalse()
  })

  test("applies detail blend mode 8 and distance-alpha soft mask in target order", () => {
    const detailTexture = texture("detail")
    const material = baseMaterial({
      detail: detailTexture,
      detailBlendMode: 8,
      distanceAlpha: true,
      distanceAlphaFromDetail: true,
      softEdges: true,
      edgeSoftnessStart: 0.6,
      edgeSoftnessEnd: 0.4,
    })
    const result = shadeVguiImage(request(material), new Map([
      ["base", pixels([255, 128, 64, 255])],
      ["detail", pixels([128, 255, 128, 128])],
    ]))
    expect([...result]).toEqual([188, 188, 99, 131])
  })

  test("masks only base alpha for authored Source detail blend mode 9", () => {
    const material=baseMaterial({detail:texture("detail"),detailBlendMode:9})
    const result=shadeVguiImage(request(material),new Map([["base",pixels([255,128,64,200])],["detail",pixels([0,255,0,128])]]))
    expect([...result]).toEqual([255,188,137,100])
  })

  test("uses actual viewport dimensions for configured outline scaling", () => {
    const detailTexture = texture("detail")
    const material = baseMaterial({
      detail: detailTexture,
      detailBlendMode: 8,
      distanceAlpha: true,
      distanceAlphaFromDetail: true,
      outline: true,
      outlineColor: Object.freeze([1, 0, 0]),
      outlineAlpha: 1,
      outlineStart0: 0.3,
      outlineStart1: 0.49,
      outlineEnd0: 0.57,
      outlineEnd1: 0.73,
      scaleOutline: true,
    })
    const nominal = shadeVguiImage(request(material), new Map([
      ["base", pixels([0, 0, 255, 255])],
      ["detail", pixels([255, 255, 255, 125])],
    ]))
    const smaller = shadeVguiImage(Object.freeze({ ...request(material), viewportWidth: 512, viewportHeight: 384 }), new Map([
      ["base", pixels([0, 0, 255, 255])],
      ["detail", pixels([255, 255, 255, 125])],
    ]))
    expect([...nominal]).not.toEqual([...smaller])
  })
})
