import type { HdrProfile, Rgb, RuntimeWorldLight } from "./runtime-map"

export type DirectionalLightingInput =
  | Readonly<{ kind: "normal"; value: Rgb }>
  | Readonly<{ kind: "ssbump"; value: Rgb }>

export const SOURCE_BUMP_BASIS = Object.freeze([
  Object.freeze([0.8164966106414795, 0, 0.5773502588272095]),
  Object.freeze([-0.40824833512306213, 0.7071067690849304, 0.5773502588272095]),
  Object.freeze([-0.4082482159137726, -0.7071068286895752, 0.5773502588272095]),
]) as readonly [Rgb, Rgb, Rgb]

export class LightingInputError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "LightingInputError"
  }
}

function finiteRgb(value: Rgb): boolean {
  return value.length === 3 && value.every(Number.isFinite)
}

function dot(left: Rgb, right: Rgb): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

export function directionalWeights(input: DirectionalLightingInput): Rgb {
  if (!finiteRgb(input.value)) throw new LightingInputError("directional lighting input is non-finite")
  if (input.kind === "ssbump") {
    if (input.value.some((value) => value < 0)) {
      throw new LightingInputError("SSBump coefficients must be non-negative")
    }
    return Object.freeze([...input.value]) as unknown as Rgb
  }
  const weights = SOURCE_BUMP_BASIS.map((basis) => {
    const projected = Math.max(0, Math.min(1, dot(input.value, basis)))
    return projected * projected
  }) as [number, number, number]
  const sum = weights[0] + weights[1] + weights[2]
  if (sum <= 1e-12) throw new LightingInputError("normal has no accepted Source basis contribution")
  return Object.freeze(weights.map((value) => value / sum)) as unknown as Rgb
}

export function evaluateDirectionalLightmap(
  flat: Rgb,
  directional: readonly [Rgb, Rgb, Rgb] | undefined,
  input: DirectionalLightingInput | undefined,
): Rgb {
  if (!finiteRgb(flat)) throw new LightingInputError("flat lightmap sample is non-finite")
  if (!directional || !input) {
    throw new LightingInputError("directional lightmap data and its normal or SSBump input are required")
  }
  if (directional.some((value) => !finiteRgb(value))) {
    throw new LightingInputError("directional lightmap sample is non-finite")
  }
  const weights = directionalWeights(input)
  return Object.freeze([0, 1, 2].map((channel) =>
    directional[0][channel]! * weights[0]
    + directional[1][channel]! * weights[1]
    + directional[2][channel]! * weights[2],
  )) as unknown as Rgb
}

export type AmbientCubeRequest = Readonly<{
  leaf: number
  samples: readonly Readonly<{ sample: number; weight: number }>[]
}>

export function sampleAmbientCube(profile: HdrProfile, request: AmbientCubeRequest): readonly [Rgb, Rgb, Rgb, Rgb, Rgb, Rgb] {
  if (!Number.isSafeInteger(request.leaf) || request.leaf < 0 || request.leaf >= profile.ambientIndexes.length) {
    throw new LightingInputError("ambient leaf identity is invalid")
  }
  const range = profile.ambientIndexes[request.leaf]!
  if (request.samples.length < 1 || request.samples.length > range.sampleCount) {
    throw new LightingInputError("ambient sample selection is empty or exceeds its leaf range")
  }
  let weightSum = 0
  const identities = new Set<number>()
  const cube = Array.from({ length: 6 }, () => [0, 0, 0] as [number, number, number])
  for (const selected of request.samples) {
    if (
      !Number.isSafeInteger(selected.sample)
      || selected.sample < range.firstSample
      || selected.sample >= range.firstSample + range.sampleCount
      || !Number.isFinite(selected.weight)
      || selected.weight < 0
      || !identities.add(selected.sample)
    ) {
      throw new LightingInputError("ambient sample identity or weight is invalid")
    }
    weightSum += selected.weight
    const source = profile.ambientSamples[selected.sample]!
    for (let side = 0; side < 6; side += 1) {
      for (let channel = 0; channel < 3; channel += 1) {
        cube[side]![channel] += source.cube[side]![channel]! * selected.weight
      }
    }
  }
  if (Math.abs(weightSum - 1) > 1e-6) throw new LightingInputError("ambient sample weights must sum to one")
  return Object.freeze(cube.map((side) => Object.freeze(side))) as unknown as readonly [Rgb, Rgb, Rgb, Rgb, Rgb, Rgb]
}

export type WorldLightRequest = Readonly<{
  candidates: readonly number[]
  styleScalars: readonly Readonly<{ style: number; scalar: number }>[]
}>

export type PreparedWorldLight = Readonly<{
  sourceIndex: number
  source: RuntimeWorldLight
  intensity: Rgb
}>

export function prepareWorldLights(profile: HdrProfile, request: WorldLightRequest): readonly PreparedWorldLight[] {
  const styles = new Map<number, number>()
  for (const value of request.styleScalars) {
    if (
      !Number.isSafeInteger(value.style) || value.style < 0 || value.style > 63
      || !Number.isFinite(value.scalar) || value.scalar < 0 || styles.has(value.style)
    ) {
      throw new LightingInputError("world-light style scalar is invalid")
    }
    styles.set(value.style, value.scalar)
  }
  const candidates = new Set<number>()
  return Object.freeze(request.candidates.map((sourceIndex) => {
    if (
      !Number.isSafeInteger(sourceIndex) || sourceIndex < 0 || sourceIndex >= profile.worldLights.length
      || !candidates.add(sourceIndex)
    ) {
      throw new LightingInputError("world-light candidate identity is invalid")
    }
    const source = profile.worldLights[sourceIndex]!
    const scalar = styles.get(source.style)
    if (scalar === undefined) throw new LightingInputError("a world-light style scalar is missing")
    const intensity = Object.freeze(source.intensity.map((value) => value * scalar)) as unknown as Rgb
    return Object.freeze({ sourceIndex, source, intensity })
  }))
}
