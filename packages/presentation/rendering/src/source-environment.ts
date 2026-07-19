// Sky and Water behavior in this file is adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.

import type { SourceVector3 } from "./model-lighting"

export type SkyFace = "right" | "left" | "back" | "front" | "up" | "down"

export type SkyVertex = Readonly<{ position: SourceVector3; uv: readonly [number, number] }>
export type SkyFaceGeometry = Readonly<{
  face: SkyFace
  normal: SourceVector3
  vertices: readonly [SkyVertex, SkyVertex, SkyVertex, SkyVertex]
  indices: readonly [0, 1, 2, 0, 2, 3]
}>

export type AuthoredCubePlane = Readonly<{
  mip: number
  face: number
  width: number
  height: number
  rgba: Uint8Array | Uint16Array
}>

export type AuthoredCubemap = Readonly<{
  mipCount: number
  scalarEncoding: "u8" | "f16"
  planes: readonly AuthoredCubePlane[]
}>

export type CubemapSample = Readonly<{ index: number; origin: SourceVector3 }>

export class SourceEnvironmentError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SourceEnvironmentError"
  }
}

const SKY_SCALE = 0.57735
const SKY_SEAM_MINIMUM = 1 / 512
const SKY_SEAM_MAXIMUM = 511 / 512
const SKY_FACES = Object.freeze([
  Object.freeze({ face: "right" as const, normal: [1, 0, 0] as SourceVector3 }),
  Object.freeze({ face: "left" as const, normal: [-1, 0, 0] as SourceVector3 }),
  Object.freeze({ face: "back" as const, normal: [0, 1, 0] as SourceVector3 }),
  Object.freeze({ face: "front" as const, normal: [0, -1, 0] as SourceVector3 }),
  Object.freeze({ face: "up" as const, normal: [0, 0, 1] as SourceVector3 }),
  Object.freeze({ face: "down" as const, normal: [0, 0, -1] as SourceVector3 }),
])

function finiteVector(value: readonly number[], length: number): boolean {
  return value.length === length && value.every(Number.isFinite)
}

function skyPosition(face: number, s: number, t: number, extent: number, origin: SourceVector3): SourceVector3 {
  const positions: SourceVector3[] = [
    [extent, -s * extent, t * extent],
    [-extent, s * extent, t * extent],
    [s * extent, extent, t * extent],
    [-s * extent, -extent, t * extent],
    [-t * extent, -s * extent, extent],
    [t * extent, -s * extent, -extent],
  ]
  const value = positions[face]!
  return [value[0] + origin[0], value[1] + origin[1], value[2] + origin[2]]
}

export function buildSourceSkyGeometry(origin: SourceVector3, farZ: number): readonly SkyFaceGeometry[] {
  if (!finiteVector(origin, 3) || !Number.isFinite(farZ) || farZ <= 0) {
    throw new SourceEnvironmentError("2D sky geometry input is invalid")
  }
  const extent = farZ * SKY_SCALE
  const corners = [[-1, -1], [-1, 1], [1, 1], [1, -1]] as const
  return Object.freeze(SKY_FACES.map((semantic, face): SkyFaceGeometry => Object.freeze({
    ...semantic,
    vertices: Object.freeze(corners.map(([s, t]): SkyVertex => Object.freeze({
      position: skyPosition(face, s, t, extent, origin),
      uv: Object.freeze([
        Math.max(SKY_SEAM_MINIMUM, Math.min(SKY_SEAM_MAXIMUM, (s + 1) * 0.5)),
        1 - Math.max(SKY_SEAM_MINIMUM, Math.min(SKY_SEAM_MAXIMUM, (t + 1) * 0.5)),
      ]) as readonly [number, number],
    }))) as unknown as SkyFaceGeometry["vertices"],
    indices: Object.freeze([0, 1, 2, 0, 2, 3]) as SkyFaceGeometry["indices"],
  })))
}

export function sourceSkyFaceVisible(face: SkyFace, viewForward: SourceVector3): boolean {
  if (!finiteVector(viewForward, 3)) throw new SourceEnvironmentError("2D sky view direction is invalid")
  const semantic = SKY_FACES.find((value) => value.face === face)
  if (!semantic) throw new SourceEnvironmentError("2D sky face is invalid")
  const forward = normalized(viewForward)
  return semantic.normal[0] * forward[0] + semantic.normal[1] * forward[1] + semantic.normal[2] * forward[2] >= -0.29289
}

export function decodeRgbsBilinear(
  samples: readonly [readonly [number, number, number, number], readonly [number, number, number, number], readonly [number, number, number, number], readonly [number, number, number, number]],
  fraction: readonly [number, number],
): SourceVector3 {
  if (samples.some((sample) => !finiteVector(sample, 4)) || !finiteVector(fraction, 2)) {
    throw new SourceEnvironmentError("RGBS sky sample input is invalid")
  }
  const x = Math.max(0, Math.min(1, fraction[0]))
  const y = Math.max(0, Math.min(1, fraction[1]))
  const decoded = samples.map((sample) => [sample[0] * sample[3], sample[1] * sample[3], sample[2] * sample[3]] as SourceVector3)
  return [0, 1, 2].map((channel) => {
    const top = decoded[0]![channel]! * (1 - x) + decoded[1]![channel]! * x
    const bottom = decoded[2]![channel]! * (1 - x) + decoded[3]![channel]! * x
    return (top * (1 - y) + bottom * y) * 8
  }) as unknown as SourceVector3
}

export function selectNearestCubemap(samples: readonly CubemapSample[], position: SourceVector3): number {
  if (samples.length < 1 || !finiteVector(position, 3)) throw new SourceEnvironmentError("cubemap selection input is invalid")
  let selected = 0
  let best = Number.POSITIVE_INFINITY
  const identities = new Set<number>()
  for (let source = 0; source < samples.length; source += 1) {
    const sample = samples[source]!
    if (!Number.isSafeInteger(sample.index) || sample.index < 0 || !identities.add(sample.index) || !finiteVector(sample.origin, 3)) {
      throw new SourceEnvironmentError("cubemap sample is invalid")
    }
    const x = position[0] - sample.origin[0]
    const y = position[1] - sample.origin[1]
    const z = position[2] - sample.origin[2]
    const distance = x * x + y * y + z * z
    if (distance < best) {
      best = distance
      selected = sample.index
    }
  }
  return selected
}

type CubeCoordinate = Readonly<{ face: number; u: number; v: number }>

export function sourceCubemapCoordinate(direction: SourceVector3): CubeCoordinate {
  if (!finiteVector(direction, 3) || direction.every((value) => value === 0)) {
    throw new SourceEnvironmentError("cubemap direction is invalid")
  }
  const [x, y, z] = direction
  const ax = Math.abs(x), ay = Math.abs(y), az = Math.abs(z)
  if (ax >= ay && ax >= az) {
    return x >= 0
      ? Object.freeze({ face: 0, u: (-z / ax + 1) * 0.5, v: (-y / ax + 1) * 0.5 })
      : Object.freeze({ face: 1, u: (z / ax + 1) * 0.5, v: (-y / ax + 1) * 0.5 })
  }
  if (ay >= az) {
    return y >= 0
      ? Object.freeze({ face: 2, u: (x / ay + 1) * 0.5, v: (z / ay + 1) * 0.5 })
      : Object.freeze({ face: 3, u: (x / ay + 1) * 0.5, v: (-z / ay + 1) * 0.5 })
  }
  return z >= 0
    ? Object.freeze({ face: 4, u: (x / az + 1) * 0.5, v: (-y / az + 1) * 0.5 })
    : Object.freeze({ face: 5, u: (-x / az + 1) * 0.5, v: (-y / az + 1) * 0.5 })
}

function directionFromFace(face: number, u: number, v: number): SourceVector3 {
  const s = u * 2 - 1
  const t = v * 2 - 1
  const directions: SourceVector3[] = [
    [1, -t, -s], [-1, -t, s], [s, 1, t], [s, -1, -t], [s, -t, 1], [-s, -t, -1],
  ]
  const value = directions[face]
  if (!value) throw new SourceEnvironmentError("cubemap face is invalid")
  return value
}

function halfToFloat(value: number): number {
  const sign = value >> 15 & 1
  const exponent = value >> 10 & 0x1f
  const fraction = value & 0x3ff
  if (exponent === 0) return (sign ? -1 : 1) * 2 ** -14 * (fraction / 1024)
  if (exponent === 31) return fraction === 0 ? (sign ? -Infinity : Infinity) : Number.NaN
  return (sign ? -1 : 1) * 2 ** (exponent - 15) * (1 + fraction / 1024)
}

function validateCubemap(input: AuthoredCubemap, mip: number): Map<number, AuthoredCubePlane> {
  if (!Number.isSafeInteger(input.mipCount) || input.mipCount < 1 || !Number.isSafeInteger(mip) || mip < 0 || mip >= input.mipCount) {
    throw new SourceEnvironmentError("cubemap mip input is invalid")
  }
  const selected = new Map<number, AuthoredCubePlane>()
  for (const plane of input.planes) {
    if (plane.mip !== mip || plane.face < 0 || plane.face > 5) continue
    if (
      selected.has(plane.face)
      || !Number.isSafeInteger(plane.width) || plane.width < 1
      || !Number.isSafeInteger(plane.height) || plane.height !== plane.width
      || plane.rgba.length !== plane.width * plane.height * 4
    ) throw new SourceEnvironmentError("cubemap plane is invalid")
    selected.set(plane.face, plane)
  }
  if (selected.size !== 6) throw new SourceEnvironmentError("cubemap has an incomplete six-face mip")
  return selected
}

function texel(input: AuthoredCubemap, planes: Map<number, AuthoredCubePlane>, face: number, x: number, y: number): readonly [number, number, number, number] {
  let plane = planes.get(face)!
  if (x < 0 || y < 0 || x >= plane.width || y >= plane.height) {
    const remapped = sourceCubemapCoordinate(directionFromFace(face, (x + 0.5) / plane.width, (y + 0.5) / plane.height))
    plane = planes.get(remapped.face)!
    x = Math.max(0, Math.min(plane.width - 1, Math.floor(remapped.u * plane.width)))
    y = Math.max(0, Math.min(plane.height - 1, Math.floor(remapped.v * plane.height)))
  }
  const at = (y * plane.width + x) * 4
  const scalar = (index: number) => input.scalarEncoding === "u8"
    ? (plane.rgba[index]! / 255)
    : halfToFloat(plane.rgba[index]!)
  const result = [scalar(at), scalar(at + 1), scalar(at + 2), scalar(at + 3)] as const
  if (!result.every(Number.isFinite)) throw new SourceEnvironmentError("cubemap texel is non-finite")
  return result
}

export function sampleAuthoredCubemap(
  input: AuthoredCubemap,
  direction: SourceVector3,
  mip: number,
  filter: "nearest" | "linear",
): readonly [number, number, number, number] {
  const planes = validateCubemap(input, mip)
  const coordinate = sourceCubemapCoordinate(direction)
  const plane = planes.get(coordinate.face)!
  const x = coordinate.u * plane.width - 0.5
  const y = coordinate.v * plane.height - 0.5
  if (filter === "nearest") return texel(input, planes, coordinate.face, Math.round(x), Math.round(y))
  if (filter !== "linear") throw new SourceEnvironmentError("cubemap filter is invalid")
  const x0 = Math.floor(x), y0 = Math.floor(y), fx = x - x0, fy = y - y0
  const samples = [
    texel(input, planes, coordinate.face, x0, y0),
    texel(input, planes, coordinate.face, x0 + 1, y0),
    texel(input, planes, coordinate.face, x0, y0 + 1),
    texel(input, planes, coordinate.face, x0 + 1, y0 + 1),
  ]
  return [0, 1, 2, 3].map((channel) => {
    const top = samples[0]![channel]! * (1 - fx) + samples[1]![channel]! * fx
    const bottom = samples[2]![channel]! * (1 - fx) + samples[3]![channel]! * fx
    return top * (1 - fy) + bottom * fy
  }) as unknown as readonly [number, number, number, number]
}

function saturate(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function dot(left: SourceVector3, right: SourceVector3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

function normalized(value: SourceVector3): SourceVector3 {
  const magnitude = Math.hypot(...value)
  if (!Number.isFinite(magnitude) || magnitude <= 0) throw new SourceEnvironmentError("Water vector is not normalizable")
  return [value[0] / magnitude, value[1] / magnitude, value[2] / magnitude]
}

function mix(left: SourceVector3, right: SourceVector3, amount: number): SourceVector3 {
  return [0, 1, 2].map((channel) => left[channel]! * (1 - amount) + right[channel]! * amount) as unknown as SourceVector3
}

export type CheapWaterRequest = Readonly<{
  cubemap: SourceVector3
  environmentScale: number
  fogColor: SourceVector3
  reflectTint: SourceVector3
  reflectionBlendFactor: number
  worldNormal: SourceVector3
  worldEyeVector: SourceVector3
  distance: number
  cheapStart: number
  cheapEnd: number
  blend: boolean
  refractionDepth: number | null
  fresnel: boolean
  hdr: boolean
}>

export function evaluateCheapWater(request: CheapWaterRequest): readonly [number, number, number, number] {
  if (
    !finiteVector(request.cubemap, 3) || !finiteVector(request.fogColor, 3) || !finiteVector(request.reflectTint, 3)
    || !finiteVector(request.worldNormal, 3) || !finiteVector(request.worldEyeVector, 3)
    || ![request.environmentScale, request.reflectionBlendFactor, request.distance, request.cheapStart, request.cheapEnd, request.refractionDepth ?? 0].every(Number.isFinite)
    || request.cheapEnd === request.cheapStart
  ) throw new SourceEnvironmentError("cheap Water input is invalid")
  const normal = normalized(request.worldNormal)
  const eye = normalized(request.worldEyeVector)
  const dotValue = 1 - Math.max(0, dot(eye, normal))
  const fresnel = request.fresnel ? dotValue ** 5 : request.reflectionBlendFactor
  const reflected = [0, 1, 2].map((channel) => request.cubemap[channel]! * request.environmentScale * request.reflectTint[channel]!) as unknown as SourceVector3
  if (!request.blend) {
    const fog = request.hdr ? request.fogColor.map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4) as unknown as SourceVector3 : request.fogColor
    return [...mix(fog, reflected, fresnel), 1]
  }
  const distanceBlend = saturate(request.distance / (request.cheapEnd - request.cheapStart) - request.cheapStart / (request.cheapEnd - request.cheapStart))
  let alpha = saturate(fresnel + distanceBlend)
  if (request.refractionDepth !== null) alpha *= saturate((request.refractionDepth - 0.05) * 20)
  return [...reflected, alpha]
}

export function waterBlurOffsets(): readonly (readonly [number, number])[] {
  return Object.freeze(Array.from({ length: 25 }, (_, index) => Object.freeze([
    (index % 5 - 2) * 0.005,
    (Math.floor(index / 5) - 2) * 0.005,
  ]) as readonly [number, number]))
}

export type ExpensiveWaterRequest = Readonly<{
  reflection: SourceVector3 | null
  refraction: SourceVector3 | null
  normal: SourceVector3
  tangentEyeVector: SourceVector3
  reflectionTint: SourceVector3
  refractionTint: SourceVector3
  reflectionOverbright: number
  linearLightScale: number
  aboveWater: boolean
  waterFogDepth: number
  projectedDepth: number
  fogColor: SourceVector3
  fogStart: number
  fogEnd: number
  hasBaseTexture: boolean
}>

export function evaluateExpensiveWater(request: ExpensiveWaterRequest): SourceVector3 {
  if (
    (!request.reflection && !request.refraction)
    || !finiteVector(request.normal, 3) || !finiteVector(request.tangentEyeVector, 3)
    || !finiteVector(request.reflectionTint, 3) || !finiteVector(request.refractionTint, 3)
    || !finiteVector(request.fogColor, 3)
    || ![request.reflectionOverbright, request.linearLightScale, request.waterFogDepth, request.projectedDepth, request.fogStart, request.fogEnd].every(Number.isFinite)
    || (!request.aboveWater && request.fogEnd === request.fogStart)
  ) throw new SourceEnvironmentError("expensive Water input is invalid")
  const normal = normalized(request.normal)
  const eye = normalized(request.tangentEyeVector)
  let fresnel = (1 - saturate(dot(eye, normal))) ** 5
  if (!request.hasBaseTexture) fresnel *= saturate((request.waterFogDepth - 0.05) * 20)
  const reflection = request.reflection && [0, 1, 2].map((channel) => request.reflection![channel]! * request.reflectionTint[channel]! * request.reflectionOverbright) as unknown as SourceVector3
  let refraction = request.refraction && [0, 1, 2].map((channel) => request.refraction![channel]! * request.refractionTint[channel]!) as unknown as SourceVector3
  if (refraction) {
    const fog = request.aboveWater
      ? saturate(request.waterFogDepth - 0.05)
      : saturate((request.projectedDepth - request.fogStart) / (request.fogEnd - request.fogStart))
    refraction = mix(refraction, request.fogColor.map((value) => value * request.linearLightScale) as unknown as SourceVector3, fog)
  }
  if (reflection && refraction) return mix(refraction, reflection, fresnel)
  return reflection ?? refraction!
}

export type FogState = Readonly<{
  enabled: boolean
  blend: boolean
  radial: boolean
  direction: SourceVector3
  primary: readonly [number, number, number, number]
  secondary: readonly [number, number, number, number]
  start: number
  end: number
  maximumDensity: number
}>

export function evaluateFogColor(state: FogState, viewForward: SourceVector3): SourceVector3 {
  if (
    !finiteVector(state.direction, 3) || !finiteVector(state.primary, 4) || !finiteVector(state.secondary, 4)
    || !finiteVector(viewForward, 3) || ![state.start, state.end, state.maximumDensity].every(Number.isFinite)
  ) throw new SourceEnvironmentError("fog state is invalid")
  const primary = state.primary.slice(0, 3).map((value) => value / 255) as unknown as SourceVector3
  if (!state.blend) return primary
  const direction = normalized(state.direction)
  const forward = normalized(viewForward)
  const amount = dot(forward, direction) * 0.5 + 0.5
  const secondary = state.secondary.slice(0, 3).map((value) => value / 255) as unknown as SourceVector3
  return mix(secondary, primary, amount)
}

export function linearFogFactor(distance: number, start: number, end: number, maximumDensity: number): number {
  if (![distance, start, end, maximumDensity].every(Number.isFinite) || end === start) {
    throw new SourceEnvironmentError("linear fog input is invalid")
  }
  return Math.min(saturate((distance - start) / (end - start)), maximumDensity)
}
