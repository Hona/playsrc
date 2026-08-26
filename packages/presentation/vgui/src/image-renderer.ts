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

export function isDirectVguiImageMaterial(material: VguiImageMaterialPresentation): boolean {
  return material.shader === "unlit-generic"
    && material.base.colorRead === "srgb"
    && material.second === null
    && material.detail === null
    && !material.distanceAlpha
    && !material.distanceAlphaFromDetail
    && !material.softEdges
    && !material.outline
    && !material.glow
}

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
const SRGB_TEXTURE_SAMPLES = Float64Array.from({ length: 256 }, (_, value) => srgbToLinear(value / 255))
const LINEAR_TEXTURE_SAMPLES = Float64Array.from({ length: 256 }, (_, value) => value / 255)
const linearToSrgb = (value: number): number => value <= 0.0031308
  ? value * 12.92
  : 1.055 * clamp(value) ** (1 / 2.4) - 0.055

function wrap(value: number): number { return value - Math.floor(value) }

function sample(texture: VguiImageRasterTexturePixels, u: number, v: number, output: Float64Array): void {
  const wrappedU = wrap(u)
  const wrappedV = wrap(v)
  if (!texture.filtered) {
    const x = Math.floor(wrappedU * texture.width)
    const y = Math.floor(wrappedV * texture.height)
    const offset = (y * texture.width + x) * 4
    const colors = texture.colorRead === "srgb" ? SRGB_TEXTURE_SAMPLES : LINEAR_TEXTURE_SAMPLES
    output[0] = colors[texture.rgba[offset]!]!
    output[1] = colors[texture.rgba[offset + 1]!]!
    output[2] = colors[texture.rgba[offset + 2]!]!
    output[3] = LINEAR_TEXTURE_SAMPLES[texture.rgba[offset + 3]!]!
    return
  }
  const x = wrappedU * texture.width - 0.5
  const y = wrappedV * texture.height - 0.5
  const x0 = Math.floor(x)
  const y0 = Math.floor(y)
  const fx = x - x0
  const fy = y - y0
  const firstX = x0 < 0 ? x0 + texture.width : x0
  const nextX = x0 + 1 >= texture.width ? x0 + 1 - texture.width : x0 + 1
  const firstY = y0 < 0 ? y0 + texture.height : y0
  const nextY = y0 + 1 >= texture.height ? y0 + 1 - texture.height : y0 + 1
  const firstRow = firstY * texture.width * 4
  const nextRow = nextY * texture.width * 4
  for (let channel = 0; channel < 4; channel += 1) {
    const values = channel !== 3 && texture.colorRead === "srgb" ? SRGB_TEXTURE_SAMPLES : LINEAR_TEXTURE_SAMPLES
    output[channel] = mix(
      mix(values[texture.rgba[firstRow + firstX * 4 + channel]!]!, values[texture.rgba[firstRow + nextX * 4 + channel]!]!, fx),
      mix(values[texture.rgba[nextRow + firstX * 4 + channel]!]!, values[texture.rgba[nextRow + nextX * 4 + channel]!]!, fx),
      fy,
    )
  }
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

function shadeVguiImageRows(
  request: VguiImageRasterRequest,
  textures: ReadonlyMap<string, VguiImageRasterTexturePixels>,
  output: Uint8ClampedArray,
  firstRow: number,
  lastRow: number,
): Uint8ClampedArray {
  const baseTexture = textures.get(request.material.base.logicalIdentity)
  if (!baseTexture) throw new Error(`VGUI base texture ${request.material.base.logicalIdentity} is unavailable`)
  const secondTexture = request.material.second ? textures.get(request.material.second.logicalIdentity) : null
  const detailTexture = request.material.detail ? textures.get(request.material.detail.logicalIdentity) : null
  if (request.material.second && !secondTexture) throw new Error(`VGUI second texture ${request.material.second.logicalIdentity} is unavailable`)
  if (request.material.detail && !detailTexture) throw new Error(`VGUI detail texture ${request.material.detail.logicalIdentity} is unavailable`)
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
  const tintRed = request.tint[0] / 255
  const tintGreen = request.tint[1] / 255
  const tintBlue = request.tint[2] / 255
  const tintAlpha = request.tint[3] / 255
  const color = new Float64Array(4)
  const secondary = new Float64Array(4)
  for (let y = firstRow; y < lastRow; y += 1) {
    for (let x = 0; x < request.width; x += 1) {
      let u: number
      let v: number
      if (request.geometry.kind === "stretch" || request.geometry.kind === "tile") {
        u = (x + 0.5) / (request.geometry.kind === "stretch" ? request.width : request.material.base.width)
        v = (y + 0.5) / (request.geometry.kind === "stretch" ? request.height : request.material.base.height)
        const rotation = request.geometry.rotation
        if (rotation === 1) { const previous = u; u = v; v = 1 - previous }
        else if (rotation === 2) { u = 1 - u; v = 1 - v }
        else if (rotation === 3) { const previous = u; u = 1 - v; v = previous }
      } else if (request.geometry.kind === "crop") {
        u = mix(request.geometry.u0, request.geometry.u1, (x + 0.5) / request.width)
        v = mix(request.geometry.v0, request.geometry.v1, (y + 0.5) / request.height)
      } else {
        u = slicedCoordinate(x + 0.5, request.width, request.material.base.width, request.geometry.sourceCornerWidth, request.geometry.drawCornerWidth)
        v = slicedCoordinate(y + 0.5, request.height, request.material.base.height, request.geometry.sourceCornerHeight, request.geometry.drawCornerHeight)
      }
      sample(baseTexture, u, v, color)
      if (request.material.shader === "unlit-two-texture") {
        sample(secondTexture!, u, v, secondary)
        color[0] *= secondary[0]!
        color[1] *= secondary[1]!
        color[2] *= secondary[2]!
        color[3] = 1
      } else {
        let distance = color[3]!
        if (detailTexture) {
          sample(detailTexture, u * request.material.detailScale[0], v * request.material.detailScale[1], secondary)
          secondary[0] *= request.material.detailTint[0]
          secondary[1] *= request.material.detailTint[1]
          secondary[2] *= request.material.detailTint[2]
          if (request.material.distanceAlpha && request.material.distanceAlphaFromDetail) {
            distance = secondary[3]!
            secondary[3] = 1
          }
          if (request.material.detailBlendMode === 8) {
            for (let channel = 0; channel < 4; channel += 1) {
              color[channel] = mix(color[channel]!, color[channel]! * secondary[channel]!, request.material.detailBlendFactor)
            }
          } else if (request.material.detailBlendMode === 9) {
            color[3] = mix(color[3]!, color[3]! * secondary[3]!, request.material.detailBlendFactor)
          }
        }
        if (request.material.distanceAlpha) {
          if (request.material.outline) {
            const factor = smooth(outlineStart0, outlineStart1, distance) * smooth(outlineEnd1, outlineEnd0, distance)
            color[0] = mix(color[0]!, request.material.outlineColor[0], factor)
            color[1] = mix(color[1]!, request.material.outlineColor[1], factor)
            color[2] = mix(color[2]!, request.material.outlineColor[2], factor)
            color[3] = mix(color[3]!, request.material.outlineAlpha, factor)
          }
          const mask = request.material.softEdges ? smooth(softEnd, softStart, distance) : distance >= 0.5 ? 1 : 0
          if (request.material.softEdges || detailTexture) color[3] *= mask
          else color[3] = mask
          if (request.material.glow) {
            const glowSource = detailTexture ?? baseTexture
            sample(glowSource, (u + request.material.glowX) * (detailTexture ? request.material.detailScale[0] : 1), (v + request.material.glowY) * (detailTexture ? request.material.detailScale[1] : 1), secondary)
            const amount = smooth(request.material.glowStart, request.material.glowEnd, secondary[3]!)
            color[0] = mix(request.material.glowColor[0] * amount, color[0]!, mask)
            color[1] = mix(request.material.glowColor[1] * amount, color[1]!, mask)
            color[2] = mix(request.material.glowColor[2] * amount, color[2]!, mask)
            color[3] = mix(request.material.glowAlpha * amount, color[3]!, mask)
          }
        }
      }
      color[0] *= tintRed
      color[1] *= tintGreen
      color[2] *= tintBlue
      color[3] *= tintAlpha
      const offset = (y * request.width + x) * 4
      output[offset] = Math.round(clamp(linearToSrgb(color[0]!)) * 255)
      output[offset + 1] = Math.round(clamp(linearToSrgb(color[1]!)) * 255)
      output[offset + 2] = Math.round(clamp(linearToSrgb(color[2]!)) * 255)
      output[offset + 3] = Math.round(clamp(color[3]!) * 255)
    }
  }
  return output
}

export function shadeVguiImage(
  request: VguiImageRasterRequest,
  textures: ReadonlyMap<string, VguiImageRasterTexturePixels>,
): Uint8ClampedArray {
  return shadeVguiImageRows(request, textures, new Uint8ClampedArray(request.width * request.height * 4), 0, request.height)
}

export async function shadeVguiImageIncrementally(
  request: VguiImageRasterRequest,
  textures: ReadonlyMap<string, VguiImageRasterTexturePixels>,
  yieldWork: () => Promise<void>,
  maximumPixels = 4096,
): Promise<Uint8ClampedArray> {
  if (!Number.isSafeInteger(maximumPixels) || maximumPixels < 1) throw new Error("VGUI incremental raster pixel budget is invalid")
  const output = new Uint8ClampedArray(request.width * request.height * 4)
  const rows = Math.max(1, Math.floor(maximumPixels / request.width))
  for (let first = 0; first < request.height; first += rows) {
    if (first > 0) await yieldWork()
    shadeVguiImageRows(request, textures, output, first, Math.min(request.height, first + rows))
  }
  return output
}

export class VguiRasterCache<Value> {
  readonly #entries = new Map<string, Readonly<{ value: Value; bytes: number }>>()
  readonly #maximumBytes: number
  readonly #maximumEntries: number
  #bytes = 0

  constructor(maximumBytes = 64 * 1024 * 1024, maximumEntries = 512) {
    if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || !Number.isSafeInteger(maximumEntries) || maximumEntries < 1) {
      throw new Error("VGUI raster cache bounds are invalid")
    }
    this.#maximumBytes = maximumBytes
    this.#maximumEntries = maximumEntries
  }

  get(identity: string): Value | undefined {
    const entry = this.#entries.get(identity)
    if (!entry) return undefined
    this.#entries.delete(identity)
    this.#entries.set(identity, entry)
    return entry.value
  }

  set(identity: string, value: Value, bytes: number): void {
    if (!identity || !Number.isSafeInteger(bytes) || bytes < 0) throw new Error("VGUI raster cache entry is invalid")
    this.delete(identity)
    if (bytes > this.#maximumBytes) return
    this.#entries.set(identity, Object.freeze({ value, bytes }))
    this.#bytes += bytes
    while (this.#bytes > this.#maximumBytes || this.#entries.size > this.#maximumEntries) {
      this.delete(this.#entries.keys().next().value!)
    }
  }

  delete(identity: string): void {
    const entry = this.#entries.get(identity)
    if (!entry) return
    this.#entries.delete(identity)
    this.#bytes -= entry.bytes
  }

  clear(): void {
    this.#entries.clear()
    this.#bytes = 0
  }

  snapshot(): Readonly<{ entries: number; bytes: number }> {
    return Object.freeze({ entries: this.#entries.size, bytes: this.#bytes })
  }
}

export class VguiImageRasterizer {
  readonly #document: Document
  readonly #textures = new VguiRasterCache<Promise<VguiImageRasterTexturePixels>>()
  readonly #renders = new VguiRasterCache<Promise<string>>()
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
      const canvas = this.#document.createElement("canvas")
      canvas.width = texture.width
      canvas.height = texture.height
      const context = canvas.getContext("2d", { willReadFrequently: true })
      if (!context) { bitmap.close(); throw new Error("VGUI image decode canvas is unavailable") }
      try {
        context.clearRect(0, 0, texture.width, texture.height)
        context.drawImage(bitmap, 0, 0)
      } finally {
        bitmap.close()
      }
      return Object.freeze({
        width: texture.width,
        height: texture.height,
        rgba: context.getImageData(0, 0, texture.width, texture.height).data,
        filtered: texture.hardwareFiltered,
        colorRead: texture.colorRead,
      })
    })()
    this.#textures.set(texture.logicalIdentity, loading, texture.width * texture.height * 4)
    void loading.catch(() => this.#textures.delete(texture.logicalIdentity))
    return loading
  }

  async render(image: HTMLImageElement, request: VguiImageRasterRequest): Promise<void> {
    const signature = JSON.stringify(request)
    let rendering = this.#renders.get(signature)
    if (!rendering) {
      rendering = (async () => {
        const sources = [request.material.base, request.material.second, request.material.detail].filter((value): value is VguiImageMaterialTexture => value !== null)
        const loaded = await Promise.all(sources.map((texture) => this.#load(texture)))
        const pixels = await shadeVguiImageIncrementally(
          request,
          new Map(sources.map((texture, index) => [texture.logicalIdentity, loaded[index]!])),
          () => new Promise<void>(resolve => setTimeout(resolve, 0)),
        )
        const canvas = this.#document.createElement("canvas")
        canvas.width = request.width
        canvas.height = request.height
        const context = retainedVguiRasterContext(canvas)
        if (!context) throw new Error("VGUI presentation canvas is unavailable")
        context.putImageData(new ImageData(pixels, request.width, request.height), 0, 0)
        return canvas.toDataURL("image/png")
      })()
      this.#renders.set(signature, rendering, request.width * request.height * 4)
      void rendering.catch(() => this.#renders.delete(signature))
    }
    const source = await rendering
    if (this.#destroyed || !image.isConnected) return
    if (image.width !== request.width) image.width = request.width
    if (image.height !== request.height) image.height = request.height
    if (image.src !== source) image.src = source
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.#textures.clear()
    this.#renders.clear()
  }
}

export function retainedVguiRasterContext(canvas: Pick<HTMLCanvasElement, "getContext">): CanvasRenderingContext2D | null {
  return canvas.getContext("2d", { willReadFrequently: true })
}
