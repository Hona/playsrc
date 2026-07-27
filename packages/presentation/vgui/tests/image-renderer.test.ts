import { describe, expect, test } from "bun:test"
import { shadeVguiImage, type VguiImageRasterRequest, type VguiImageRasterTexturePixels } from "../src/image-renderer"
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
