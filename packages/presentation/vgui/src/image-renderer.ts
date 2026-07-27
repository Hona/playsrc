import type { Rgba } from "./contract"
import type {
  VguiImageMaterialPresentation,
  VguiImageMaterialTexture,
} from "./runtime-contract"

export type VguiImageRasterTexturePixels = Readonly<{
  width: number
  height: number
  rgba: Uint8ClampedArray
  filtered: boolean
  colorRead: "srgb" | "linear"
}>

export type VguiImageRasterGeometry =
  | Readonly<{ kind: "stretch"; rotation: 0 | 1 | 2 | 3 }>
  | Readonly<{ kind: "tile"; rotation: 0 | 1 | 2 | 3 }>
  | Readonly<{ kind: "crop"; u0: number; v0: number; u1: number; v1: number }>
  | Readonly<{
      kind: "nine-slice"
      sourceCornerWidth: number
      sourceCornerHeight: number
      drawCornerWidth: number
      drawCornerHeight: number
    }>

export type VguiImageRasterRequest = Readonly<{
  width: number
  height: number
  viewportWidth: number
  viewportHeight: number
  tint: Rgba
  geometry: VguiImageRasterGeometry
  material: VguiImageMaterialPresentation
}>

const clamp = (value: number, minimum = 0, maximum = 1): number => Math.max(minimum, Math.min(maximum, value))
const mix = (left: number, right: number, amount: number): number => left + (right - left) * amount
const smooth = (edge0: number, edge1: number, value: number): number => {
  if (edge0 === edge1) return value < edge0 ? 0 : 1
  const amount = clamp((value - edge0) / (edge1 - edge0))
  return amount * amount * (3 - 2 * amount)
}
const srgbToLinear = (value: number): number => value <= 0.04045
  ? value / 12.92
  : ((value + 0.055) / 1.055) ** 2.4
const linearToSrgb = (value: number): number => value <= 0.0031308
  ? value * 12.92
  : 1.055 * clamp(value) ** (1 / 2.4) - 0.055

function wrap(value: number): number { return value - Math.floor(value) }

function texel(texture: VguiImageRasterTexturePixels, x: number, y: number): readonly [number, number, number, number] {
  const wrappedX = ((x % texture.width) + texture.width) % texture.width
  const wrappedY = ((y % texture.height) + texture.height) % texture.height
  const offset = (wrappedY * texture.width + wrappedX) * 4
  const red = texture.rgba[offset]! / 255
  const green = texture.rgba[offset + 1]! / 255
  const blue = texture.rgba[offset + 2]! / 255
  return [
    texture.colorRead === "srgb" ? srgbToLinear(red) : red,
    texture.colorRead === "srgb" ? srgbToLinear(green) : green,
    texture.colorRead === "srgb" ? srgbToLinear(blue) : blue,
    texture.rgba[offset + 3]! / 255,
  ]
}

function sample(texture: VguiImageRasterTexturePixels, u: number, v: number): readonly [number, number, number, number] {
  const wrappedU = wrap(u)
  const wrappedV = wrap(v)
  if (!texture.filtered) return texel(texture, Math.floor(wrappedU * texture.width), Math.floor(wrappedV * texture.height))
  const x = wrappedU * texture.width - 0.5
  const y = wrappedV * texture.height - 0.5
  const x0 = Math.floor(x)
  const y0 = Math.floor(y)
  const fx = x - x0
  const fy = y - y0
  const topLeft = texel(texture, x0, y0)
  const topRight = texel(texture, x0 + 1, y0)
  const bottomLeft = texel(texture, x0, y0 + 1)
  const bottomRight = texel(texture, x0 + 1, y0 + 1)
  return [
    mix(mix(topLeft[0], topRight[0], fx), mix(bottomLeft[0], bottomRight[0], fx), fy),
    mix(mix(topLeft[1], topRight[1], fx), mix(bottomLeft[1], bottomRight[1], fx), fy),
    mix(mix(topLeft[2], topRight[2], fx), mix(bottomLeft[2], bottomRight[2], fx), fy),
    mix(mix(topLeft[3], topRight[3], fx), mix(bottomLeft[3], bottomRight[3], fx), fy),
  ]
}

function rotated(u: number, v: number, rotation: 0 | 1 | 2 | 3): readonly [number, number] {
  if (rotation === 1) return [v, 1 - u]
  if (rotation === 2) return [1 - u, 1 - v]
  if (rotation === 3) return [1 - v, u]
  return [u, v]
}

function slicedCoordinate(position: number, destination: number, source: number, sourceCorner: number, drawCorner: number): number {
  if (destination <= 0 || source <= 0) return 0
  const farDraw = destination - drawCorner
  if (position < drawCorner && drawCorner > 0) return position / drawCorner * sourceCorner / source
  if (position >= farDraw && drawCorner > 0) return (source - sourceCorner + (position - farDraw) / drawCorner * sourceCorner) / source
  const centerDraw = Math.max(0, destination - drawCorner * 2)
  const centerSource = Math.max(0, source - sourceCorner * 2)
  return (sourceCorner + (centerDraw > 0 ? (position - drawCorner) / centerDraw * centerSource : 0)) / source
}

function coordinates(request: VguiImageRasterRequest, x: number, y: number): readonly [number, number] {
  if (request.geometry.kind === "stretch") {
    return rotated((x + 0.5) / request.width, (y + 0.5) / request.height, request.geometry.rotation)
  }
  if (request.geometry.kind === "tile") {
    return rotated((x + 0.5) / request.material.base.width, (y + 0.5) / request.material.base.height, request.geometry.rotation)
  }
  if (request.geometry.kind === "crop") {
    return [
      mix(request.geometry.u0, request.geometry.u1, (x + 0.5) / request.width),
      mix(request.geometry.v0, request.geometry.v1, (y + 0.5) / request.height),
    ]
  }
  return [
    slicedCoordinate(x + 0.5, request.width, request.material.base.width, request.geometry.sourceCornerWidth, request.geometry.drawCornerWidth),
    slicedCoordinate(y + 0.5, request.height, request.material.base.height, request.geometry.sourceCornerHeight, request.geometry.drawCornerHeight),
  ]
}

export function shadeVguiImage(
  request: VguiImageRasterRequest,
  textures: ReadonlyMap<string, VguiImageRasterTexturePixels>,
): Uint8ClampedArray {
  const baseTexture = textures.get(request.material.base.logicalIdentity)
  if (!baseTexture) throw new Error(`VGUI base texture ${request.material.base.logicalIdentity} is unavailable`)
  const secondTexture = request.material.second ? textures.get(request.material.second.logicalIdentity) : null
  const detailTexture = request.material.detail ? textures.get(request.material.detail.logicalIdentity) : null
  if (request.material.second && !secondTexture) throw new Error(`VGUI second texture ${request.material.second.logicalIdentity} is unavailable`)
  if (request.material.detail && !detailTexture) throw new Error(`VGUI detail texture ${request.material.detail.logicalIdentity} is unavailable`)
  const output = new Uint8ClampedArray(request.width * request.height * 4)
  let softStart = request.material.edgeSoftnessStart
  let softEnd = request.material.edgeSoftnessEnd
  let outlineStart0 = request.material.outlineStart0
  let outlineStart1 = request.material.outlineStart1
  let outlineEnd0 = request.material.outlineEnd0
  let outlineEnd1 = request.material.outlineEnd1
  if (request.material.scaleSoftEdges || request.material.scaleOutline) {
    const resolutionScale = Math.max(0.5, 1024 / request.viewportWidth, 768 / request.viewportHeight)
    if (request.material.scaleSoftEdges) {
      const middle = 0.5 * (softStart + softEnd)
      softStart = clamp(middle + resolutionScale * (softStart - middle), 0.05, 0.99)
      softEnd = clamp(middle + resolutionScale * (softEnd - middle), 0.05, 0.99)
    }
    if (request.material.scaleOutline) {
      const startMiddle = 0.5 * (outlineStart1 + outlineStart0)
      outlineStart1 = clamp(startMiddle + resolutionScale * (outlineStart1 - startMiddle), 0.05, 0.99)
      const endMiddle = 0.5 * (outlineEnd1 + outlineEnd0)
      outlineEnd1 = clamp(endMiddle + resolutionScale * (outlineEnd1 - endMiddle), 0.05, 0.99)
    }
  }
  const tint = request.tint.map((value) => value / 255) as [number, number, number, number]
  for (let y = 0; y < request.height; y += 1) {
    for (let x = 0; x < request.width; x += 1) {
      const [u, v] = coordinates(request, x, y)
      let color = [...sample(baseTexture, u, v)] as [number, number, number, number]
      if (request.material.shader === "unlit-two-texture") {
        const second = sample(secondTexture!, u, v)
        color = [color[0] * second[0], color[1] * second[1], color[2] * second[2], 1]
      } else {
        let distance = color[3]
        if (detailTexture) {
          const detail = [...sample(detailTexture, u * request.material.detailScale[0], v * request.material.detailScale[1])] as [number, number, number, number]
          detail[0] *= request.material.detailTint[0]
          detail[1] *= request.material.detailTint[1]
          detail[2] *= request.material.detailTint[2]
          if (request.material.distanceAlpha && request.material.distanceAlphaFromDetail) {
            distance = detail[3]
            detail[3] = 1
          }
          if (request.material.detailBlendMode === 8) {
            for (let channel = 0; channel < 4; channel += 1) {
              color[channel] = mix(color[channel]!, color[channel]! * detail[channel]!, request.material.detailBlendFactor)
            }
          }
        }
        if (request.material.distanceAlpha) {
          if (request.material.outline) {
            const factor = smooth(outlineStart0, outlineStart1, distance) * smooth(outlineEnd1, outlineEnd0, distance)
            color = [
              mix(color[0], request.material.outlineColor[0], factor),
              mix(color[1], request.material.outlineColor[1], factor),
              mix(color[2], request.material.outlineColor[2], factor),
              mix(color[3], request.material.outlineAlpha, factor),
            ]
          }
          const mask = request.material.softEdges ? smooth(softEnd, softStart, distance) : distance >= 0.5 ? 1 : 0
          if (request.material.softEdges || detailTexture) color[3] *= mask
          else color[3] = mask
          if (request.material.glow) {
            const glowSource = detailTexture ?? baseTexture
            const glowSample = sample(glowSource, (u + request.material.glowX) * (detailTexture ? request.material.detailScale[0] : 1), (v + request.material.glowY) * (detailTexture ? request.material.detailScale[1] : 1))
            const amount = smooth(request.material.glowStart, request.material.glowEnd, glowSample[3])
            const glow = [
              request.material.glowColor[0] * amount,
              request.material.glowColor[1] * amount,
              request.material.glowColor[2] * amount,
              request.material.glowAlpha * amount,
            ]
            color = [0, 1, 2, 3].map((channel) => mix(glow[channel]!, color[channel]!, mask)) as [number, number, number, number]
          }
        }
      }
      color[0] *= tint[0]
      color[1] *= tint[1]
      color[2] *= tint[2]
      color[3] *= tint[3]
      const offset = (y * request.width + x) * 4
      output[offset] = Math.round(clamp(linearToSrgb(color[0])) * 255)
      output[offset + 1] = Math.round(clamp(linearToSrgb(color[1])) * 255)
      output[offset + 2] = Math.round(clamp(linearToSrgb(color[2])) * 255)
      output[offset + 3] = Math.round(clamp(color[3]) * 255)
    }
  }
  return output
}

export class VguiImageRasterizer {
  readonly #document: Document
  readonly #textures = new Map<string, Promise<VguiImageRasterTexturePixels>>()
  readonly #renders = new Map<string, Promise<Uint8ClampedArray>>()
  readonly #bitmaps: ImageBitmap[] = []
  #destroyed = false

  constructor(document: Document) { this.#document = document }

  #load(texture: VguiImageMaterialTexture): Promise<VguiImageRasterTexturePixels> {
    const prior = this.#textures.get(texture.logicalIdentity)
    if (prior) return prior
    const loading = (async () => {
      const response = await fetch(texture.browserUrl)
      if (!response.ok) throw new Error(`VGUI image ${texture.logicalIdentity} request failed`)
      const bitmap = await createImageBitmap(await response.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" })
      if (this.#destroyed) { bitmap.close(); throw new Error("VGUI image rasterizer is destroyed") }
      this.#bitmaps.push(bitmap)
      const canvas = this.#document.createElement("canvas")
      canvas.width = texture.width
      canvas.height = texture.height
      const context = canvas.getContext("2d", { willReadFrequently: true })
      if (!context) throw new Error("VGUI image decode canvas is unavailable")
      context.clearRect(0, 0, texture.width, texture.height)
      context.drawImage(bitmap, 0, 0)
      return Object.freeze({
        width: texture.width,
        height: texture.height,
        rgba: context.getImageData(0, 0, texture.width, texture.height).data,
        filtered: texture.hardwareFiltered,
        colorRead: texture.colorRead,
      })
    })()
    this.#textures.set(texture.logicalIdentity, loading)
    return loading
  }

  async render(canvas: HTMLCanvasElement, request: VguiImageRasterRequest): Promise<void> {
    const signature = JSON.stringify(request)
    let rendering = this.#renders.get(signature)
    if (!rendering) {
      if (this.#renders.size >= 8_192) throw new Error("VGUI raster cache reached its explicit limit")
      rendering = (async () => {
        const sources = [request.material.base, request.material.second, request.material.detail].filter((value): value is VguiImageMaterialTexture => value !== null)
        const loaded = await Promise.all(sources.map((texture) => this.#load(texture)))
        return shadeVguiImage(request, new Map(sources.map((texture, index) => [texture.logicalIdentity, loaded[index]!])))
      })()
      this.#renders.set(signature, rendering)
      void rendering.catch(() => this.#renders.delete(signature))
    }
    const pixels = await rendering
    if (this.#destroyed || !canvas.isConnected) return
    canvas.width = request.width
    canvas.height = request.height
    const context = canvas.getContext("2d")
    if (!context) throw new Error("VGUI presentation canvas is unavailable")
    const copied = new Uint8ClampedArray(pixels.length)
    copied.set(pixels)
    context.putImageData(new ImageData(copied, request.width, request.height), 0, 0)
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    for (const bitmap of this.#bitmaps) bitmap.close()
    this.#bitmaps.splice(0)
    this.#textures.clear()
    this.#renders.clear()
  }
}
