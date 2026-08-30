import { describe, expect, test } from "bun:test"
import { isDirectVguiImageMaterial, shadeVguiImage, shadeVguiImageIncrementally, VguiImageRasterizer, VguiRasterCache, type VguiImageRasterRequest, type VguiImageRasterTexturePixels } from "../src/image-renderer"
import type { VguiImageMaterialPresentation, VguiImageMaterialTexture } from "../src"

const texture = (identity: string): VguiImageMaterialTexture => Object.freeze({
  clampS: false, clampT: false,
  logicalIdentity: identity,
  revision: `${identity}-1`,
  browserUrl: `memory:${identity}`,
  width: 1,
  height: 1,
  hardwareFiltered: false,
  colorRead: "linear",
})

describe("bounded authored VGUI raster ownership", () => {
  test("resizing a retained resource does not dispose it, while eviction and clear do", () => {
    const disposed: object[] = [], value = {}
    const cache = new VguiRasterCache<object>(12, 2, entry => disposed.push(entry))
    cache.set("same", value, 4); cache.set("same", value, 8)
    expect(disposed).toEqual([])
    cache.set("next", {}, 8)
    expect(disposed).toEqual([value])
    cache.clear(); cache.clear()
    expect(disposed).toHaveLength(2)
    expect(cache.snapshot()).toEqual({ entries: 0, bytes: 0 })
  })
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
  vertexColorGamma: false,
  alphaTestReference: null,
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
  clampS: false, clampT: false,
  width: 1,
  height: 1,
  rgba: new Uint8ClampedArray(rgba),
  filtered: false,
  colorRead: "linear",
})

const request = (material: VguiImageMaterialPresentation): VguiImageRasterRequest => Object.freeze({
  pixelRatio: 1,
  width: 1,
  height: 1,
  viewportWidth: 1024,
  viewportHeight: 768,
  tint: Object.freeze([255, 255, 255, 255]),
  geometry: Object.freeze({ kind: "stretch", rotation: 0 }),
  material,
})

test("matching raster consumers share one encoded image and release its URL after pending decode", async () => {
  // Storage/lifecycle unit only; the headed parity fixture checks real PNG pixels.
  const names = ["fetch", "createImageBitmap", "ImageData"] as const
  const descriptors = names.map(name => Object.getOwnPropertyDescriptor(globalThis, name))
  const createUrl = URL.createObjectURL, revokeUrl = URL.revokeObjectURL
  let encodes = 0, uploads = 0
  const revoked: string[] = []
  Object.assign(globalThis, {
    fetch: async () => new Response(new Uint8Array([0])),
    createImageBitmap: async () => ({ close() {} }),
    ImageData: class { constructor(readonly data: Uint8ClampedArray, readonly width: number, readonly height: number) {} },
  })
  URL.createObjectURL = () => `blob:raster-${encodes}`
  URL.revokeObjectURL = value => { revoked.push(value) }
  const document = { createElement: () => ({ width: 0, height: 0,
    getContext: () => ({ clearRect() {}, drawImage() {}, getImageData: () => ({ data: new Uint8ClampedArray([255, 128, 0, 127]) }), putImageData() { uploads++ } }),
    toBlob(callback: (blob: Blob) => void) { encodes++; callback(new Blob(["encoded fixture"])) },
  }) } as unknown as Document
  const rasterizer = new VguiImageRasterizer(document)
  const image = () => ({ width: 0, height: 0, naturalWidth: 1, naturalHeight: 1, isConnected: true, src: "",
    getAttribute(name: string) { return String((this as any)[name]) }, async decode() {} }) as unknown as HTMLImageElement
  try {
    const targets = Array.from({ length: 50 }, image), input = request(baseMaterial())
    await Promise.all(targets.map(target => rasterizer.render(target, input)))
    expect(encodes).toBe(1); expect(uploads).toBe(1)
    expect(new Set(targets.map(target => target.src)).size).toBe(1)
    await rasterizer.render(targets[0]!, input)
    expect(encodes).toBe(1)
    await Promise.all([rasterizer.render(targets[0]!, { ...input, tint: [100, 100, 100, 100] }), rasterizer.render(targets[0]!, input)])
    expect(targets[0]!.src).toBe("blob:raster-1")
    const pending = Promise.withResolvers<void>(), entered = Promise.withResolvers<void>(), target = image()
    target.decode = async () => { entered.resolve(); await pending.promise }
    const rendering = rasterizer.render(target, input)
    await entered.promise
    rasterizer.destroy()
    expect(revoked).toEqual(["blob:raster-2"])
    pending.resolve(); await rendering
    expect(revoked).toEqual(["blob:raster-2", "blob:raster-1"])
  } finally {
    rasterizer.destroy()
    names.forEach((name, index) => { const descriptor = descriptors[index]; if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete (globalThis as any)[name] })
    URL.createObjectURL = createUrl; URL.revokeObjectURL = revokeUrl
  }
})

describe("configured VGUI image material raster", () => {
  test("clamped masks never bleed the opposite texture edge when proportionally enlarged", () => {
    const material = baseMaterial({ base: { ...texture("base"), width: 2, clampS: true, clampT: true } })
    const input = { ...pixels([0,0,0,0,255,255,255,255]), width: 2, filtered: true, clampS: true, clampT: true }
    const result = shadeVguiImage({ ...request(material), width: 8 }, new Map([["base", input]]))
    expect(result[3]).toBe(0)
    expect(result[result.length - 1]).toBe(255)
  })
  test("uses Source vertex RGB gamma conversion and authored alpha testing for panel corners", () => {
    const material = baseMaterial({ vertexColorGamma: true, alphaTestReference: 0.1 })
    const image = { ...request(material), tint: [117, 107, 94, 255] as const }
    expect([...shadeVguiImage(image, new Map([["base", pixels([255, 255, 255, 128])]]))]).toEqual([118, 107, 94, 128])
    expect(shadeVguiImage(image, new Map([["base", pixels([255, 255, 255, 20])]]))[3]).toBe(0)
    expect(shadeVguiImage(image, new Map([["base", pixels([255, 255, 255, 26])]]))[3]).toBe(26)
    expect(isDirectVguiImageMaterial({ ...material, base: { ...material.base, colorRead: "srgb" } })).toBe(false)
  })
  test("yields bounded raster rows without changing one authored filtered pixel", async () => {
    const source = Object.freeze({ ...texture("base"), width: 2, height: 2 })
    const rgba = new Uint8ClampedArray([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255])
    const image = Object.freeze({ ...request(baseMaterial({ base: source })), width: 7, height: 9 })
    const sources = new Map([["base", Object.freeze({ width: 2, height: 2, rgba, filtered: true, colorRead: "linear" as const })]])
    let yields = 0
    const incremental = await shadeVguiImageIncrementally(image, sources, async () => { yields += 1 }, 14)
    expect([...incremental]).toEqual([...shadeVguiImage(image, sources)])
    expect(yields).toBe(4)
  })

  test("never yields an already bounded raster and rejects invalid incremental budgets", async () => {
    const image = request(baseMaterial())
    let yields = 0
    expect([...await shadeVguiImageIncrementally(image, new Map([["base", pixels([255, 128, 64, 255])]]), async () => { yields += 1 }, 1)]).toEqual([255, 188, 137, 255])
    expect(yields).toBe(0)
    await expect(shadeVguiImageIncrementally(image, new Map([["base", pixels([1, 2, 3, 4])]]), async () => {}, 0)).rejects.toThrow("budget")
  })

  test("preserves every exact authored sRGB byte without repeated transfer-function evaluation", () => {
    const source = Object.freeze({ ...texture("base"), width: 256, colorRead: "srgb" as const })
    const rgba = new Uint8ClampedArray(256 * 4)
    for (let value = 0; value < 256; value += 1) rgba.set([value, value, value, value], value * 4)
    const output = shadeVguiImage(Object.freeze({ ...request(baseMaterial({ base: source })), width: 256 }), new Map([
      ["base", Object.freeze({ width: 256, height: 1, rgba, filtered: false, colorRead: "srgb" as const })],
    ]))
    expect([...output]).toEqual([...rgba])
  })

  test("retains exact bilinear wrapped texture edges for authored filtered materials", () => {
    const source = Object.freeze({ ...texture("base"), width: 2, height: 2 })
    const rgba = new Uint8ClampedArray([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255])
    const output = shadeVguiImage(Object.freeze({ ...request(baseMaterial({ base: source })), width: 4, height: 4 }), new Map([
      ["base", Object.freeze({ width: 2, height: 2, rgba, filtered: true, colorRead: "linear" as const })],
    ]))
    expect([...output.slice(0, 4)]).toEqual([207, 137, 137, 255])
    expect([...output.slice(-4)]).toEqual([207, 225, 225, 255])
  })

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
