// Water shader behavior is adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.

import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { sourceShaderGammaToLinear } from "./color-output"
import { waterBlurOffsets } from "./source-environment"

type Vector2 = readonly [number, number]
type Vector3 = readonly [number, number, number]
export type SourceWaterPixel = readonly [number, number, number, number]

export class SourceWaterError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SourceWaterError"
  }
}

export type SourceWaterSampler = Readonly<{
  sample: (coordinate: Vector2) => SourceWaterPixel
}>

export type SourceWaterShaderState = Readonly<{
  profile: "ldr" | "hdr"
  mode: "expensive" | "cheap"
  aboveWater: boolean
  reflectAmount: number
  refractAmount: number
  reflectTint: Vector3
  refractTint: Vector3
  fogColor: Vector3
  fogStart: number
  fogEnd: number
  blurRefraction: boolean
  hasBaseTexture: boolean
  cheapBlend: boolean
  cheapStart: number
  cheapEnd: number
  reflectionBlendFactor: number
  fresnelEnabled: boolean
  linearLightScale: number
  environmentScale: number
}>

export type SourceWaterPixelRequest = Readonly<{
  state: SourceWaterShaderState
  clipPosition: readonly [number, number, number, number]
  normalSample: SourceWaterPixel
  tangentEyeVector: Vector3
  reflection: SourceWaterSampler | null
  refraction: SourceWaterSampler | null
}>

export type SourceWaterPixelResult = Readonly<{
  rgba: SourceWaterPixel
  reflectionUv: Vector2
  refractionUv: Vector2
  waterFogDepth: number
  fresnel: number
  refractionSamples: number
}>

export type SourceWaterFogAlphaRequest = Readonly<{
  waterHeight: number
  eyeHeight: number
  fragmentHeight: number
  projectedDepth: number
  inverseFogRange: number
}>

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

function saturate(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function validateState(state: SourceWaterShaderState): void {
  if (
    (state.profile !== "ldr" && state.profile !== "hdr")
    || (state.mode !== "expensive" && state.mode !== "cheap")
    || !finite(state.reflectTint)
    || !finite(state.refractTint)
    || !finite(state.fogColor)
    || !finite([
      state.reflectAmount,
      state.refractAmount,
      state.fogStart,
      state.fogEnd,
      state.cheapStart,
      state.cheapEnd,
      state.reflectionBlendFactor,
      state.linearLightScale,
      state.environmentScale,
    ])
    || state.linearLightScale < 0
    || state.environmentScale < 0
    || (!state.aboveWater && state.fogStart === state.fogEnd)
    || (state.mode === "cheap" && state.cheapBlend && state.cheapStart === state.cheapEnd)
  ) {
    throw new SourceWaterError("Water shader state is invalid")
  }
}

function normalize(value: Vector3): Vector3 {
  const length = Math.hypot(...value)
  if (!Number.isFinite(length) || length === 0) throw new SourceWaterError("Water eye vector is invalid")
  return [value[0] / length, value[1] / length, value[2] / length]
}

function sample(sampler: SourceWaterSampler, coordinate: Vector2): SourceWaterPixel {
  const value = sampler.sample(coordinate)
  if (value.length !== 4 || !finite(value)) throw new SourceWaterError("Water texture sample is invalid")
  return value
}

function mix(left: Vector3, right: Vector3, amount: number): Vector3 {
  return [
    left[0] + (right[0] - left[0]) * amount,
    left[1] + (right[1] - left[1]) * amount,
    left[2] + (right[2] - left[2]) * amount,
  ]
}

export function sourceWaterProjectiveCoordinates(
  clipPosition: readonly [number, number, number, number],
): Readonly<{ reflection: Vector2; refraction: Vector2 }> {
  if (clipPosition.length !== 4 || !finite(clipPosition) || clipPosition[3] === 0) {
    throw new SourceWaterError("Water projected position is invalid")
  }
  const [x, y, , w] = clipPosition
  return Object.freeze({
    reflection: Object.freeze([(x + w) / (2 * w), (y + w) / (2 * w)]) as Vector2,
    refraction: Object.freeze([(x + w) / (2 * w), (-y + w) / (2 * w)]) as Vector2,
  })
}

export function sourceWaterFogAlpha(request: SourceWaterFogAlphaRequest): number {
  if (!finite([
    request.waterHeight,
    request.eyeHeight,
    request.fragmentHeight,
    request.projectedDepth,
    request.inverseFogRange,
  ])) {
    throw new SourceWaterError("Water fog-depth input is invalid")
  }
  const eyeDepth = request.eyeHeight - request.fragmentHeight
  if (eyeDepth === 0) throw new SourceWaterError("Water fog-depth eye and fragment heights coincide")
  const waterFraction = saturate((request.waterHeight - request.fragmentHeight) / eyeDepth)
  return saturate(waterFraction * request.projectedDepth * request.inverseFogRange)
}

export function evaluateSourceWaterPixel(request: SourceWaterPixelRequest): SourceWaterPixelResult {
  const { state } = request
  validateState(state)
  if (state.mode !== "expensive") throw new SourceWaterError("Water pixel evaluator requires the expensive shader")
  if (!request.reflection && !request.refraction) throw new SourceWaterError("Water requires reflection or refraction")
  if (!finite(request.normalSample) || !finite(request.tangentEyeVector)) {
    throw new SourceWaterError("Water normal or eye input is invalid")
  }
  if (state.aboveWater && !request.refraction) {
    throw new SourceWaterError("Above-water reflection requires authored refraction-depth alpha")
  }
  if (state.blurRefraction && !request.refraction) {
    throw new SourceWaterError("Blurred Water requires an authored refraction target")
  }

  const projected = sourceWaterProjectiveCoordinates(request.clipPosition)
  const normal: Vector3 = [
    request.normalSample[0] * 2 - 1,
    request.normalSample[1] * 2 - 1,
    request.normalSample[2] * 2 - 1,
  ]
  let refractionSamples = 0
  let waterFogDepth = state.aboveWater
    ? sample(request.refraction!, projected.refraction)[3]
    : 1
  if (state.aboveWater) refractionSamples += 1

  const depthScale = !state.hasBaseTexture && !state.blurRefraction ? waterFogDepth : 1
  const normalScale = request.normalSample[3] * depthScale
  const reflectionUv = Object.freeze([
    projected.reflection[0] + normal[0] * normalScale * state.reflectAmount,
    projected.reflection[1] + normal[1] * normalScale * state.reflectAmount,
  ]) as Vector2
  const refractionUv = Object.freeze([
    projected.refraction[0] + normal[0] * normalScale * state.refractAmount,
    projected.refraction[1] + normal[1] * normalScale * state.refractAmount,
  ]) as Vector2

  const reflectionTint = state.reflectTint.map((value) =>
    sourceShaderGammaToLinear(value) * (state.profile === "hdr" ? 4 : 1),
  ) as unknown as Vector3
  const refractionTint = state.refractTint.map(sourceShaderGammaToLinear) as unknown as Vector3
  const fogColor = state.fogColor.map((value) =>
    sourceShaderGammaToLinear(value) * state.linearLightScale,
  ) as unknown as Vector3

  let reflected: Vector3 | null = null
  if (request.reflection) {
    const value = sample(request.reflection, reflectionUv)
    const overbright = state.blurRefraction && state.profile === "hdr" ? 4 : 1
    reflected = [
      value[0] * reflectionTint[0] * overbright,
      value[1] * reflectionTint[1] * overbright,
      value[2] * reflectionTint[2] * overbright,
    ]
  }

  let refracted: Vector3 | null = null
  if (request.refraction) {
    if (state.blurRefraction) {
      const sum = [0, 0, 0, 0]
      for (const offset of waterBlurOffsets()) {
        const value = sample(request.refraction, [
          refractionUv[0] + offset[0],
          refractionUv[1] + offset[1],
        ])
        for (let channel = 0; channel < 4; channel += 1) sum[channel]! += value[channel]!
        refractionSamples += 1
      }
      const blurred = sum.map((value) => value / 25) as unknown as SourceWaterPixel
      refracted = [
        blurred[0] * refractionTint[0],
        blurred[1] * refractionTint[1],
        blurred[2] * refractionTint[2],
      ]
      if (state.aboveWater) waterFogDepth = blurred[3]
    } else {
      const value = sample(request.refraction, refractionUv)
      refractionSamples += 1
      refracted = [value[0], value[1], value[2]]
      if (state.aboveWater) waterFogDepth = value[3]
    }

    const fog = state.aboveWater
      ? saturate(waterFogDepth - 0.05)
      : saturate((request.clipPosition[2] - state.fogStart) / (state.fogEnd - state.fogStart))
    refracted = mix(refracted, fogColor, fog)
  }

  const eye = normalize(request.tangentEyeVector)
  const normalDotEye = saturate(eye[0] * normal[0] + eye[1] * normal[1] + eye[2] * normal[2])
  let fresnel = (1 - normalDotEye) ** 5
  if (!state.hasBaseTexture) fresnel *= saturate((waterFogDepth - 0.05) * 20)

  const color = reflected && refracted ? mix(refracted, reflected, fresnel) : reflected ?? refracted!
  return Object.freeze({
    rgba: Object.freeze([...color, 1]) as SourceWaterPixel,
    reflectionUv,
    refractionUv,
    waterFogDepth,
    fresnel,
    refractionSamples,
  })
}

export type SourceViewFogUniforms = Readonly<{
  enabled: ReturnType<typeof TSL.uniform>
  start: ReturnType<typeof TSL.uniform>
  end: ReturnType<typeof TSL.uniform>
  maximumDensity: ReturnType<typeof TSL.uniform>
  color: ReturnType<typeof TSL.uniform>
}>

export function createSourceViewFogUniforms(): SourceViewFogUniforms {
  return Object.freeze({
    enabled: TSL.uniform(0, "float"),
    start: TSL.uniform(0, "float"),
    end: TSL.uniform(1, "float"),
    maximumDensity: TSL.uniform(1, "float"),
    color: TSL.uniform(new THREE.Vector3(), "vec3"),
  })
}

export function sourceViewFogNode(uniforms: SourceViewFogUniforms): any {
  const distance = TSL.positionView.z.negate()
  const factor = distance.sub(uniforms.start)
    .div(uniforms.end.sub(uniforms.start))
    .clamp(0, uniforms.maximumDensity)
  return TSL.fog(uniforms.color, uniforms.enabled.greaterThan(0).select(factor, 0))
}

export type SourceWaterFogUniforms = Readonly<{
  enabled: ReturnType<typeof TSL.uniform>
  waterHeight: ReturnType<typeof TSL.uniform>
  eyeHeight: ReturnType<typeof TSL.uniform>
  inverseFogRange: ReturnType<typeof TSL.uniform>
  fogColor: ReturnType<typeof TSL.uniform>
}>

export function createSourceWaterFogUniforms(): SourceWaterFogUniforms {
  return Object.freeze({
    enabled: TSL.uniform(0, "float"),
    waterHeight: TSL.uniform(0, "float"),
    eyeHeight: TSL.uniform(0, "float"),
    inverseFogRange: TSL.uniform(0, "float"),
    fogColor: TSL.uniform(new THREE.Vector3(), "vec3"),
  })
}

export function sourceWaterFogFragment(color: any, uniforms: SourceWaterFogUniforms): any {
  const fragmentHeight = TSL.positionWorld.z
  const waterFraction = uniforms.waterHeight.sub(fragmentHeight)
    .div(uniforms.eyeHeight.sub(fragmentHeight))
    .clamp(0, 1)
  const waterDepth = waterFraction.mul(TSL.clipSpace.z).mul(uniforms.inverseFogRange).clamp(0, 1)
  const enabled = uniforms.enabled.greaterThan(0)
  const rgb = enabled.select(TSL.mix(color.rgb, uniforms.fogColor, waterDepth), color.rgb)
  return TSL.vec4(rgb, enabled.select(waterDepth, color.a))
}

export type SourceWaterGpuInput = Readonly<{
  geometry: THREE.BufferGeometry
  state: SourceWaterShaderState
  normal: THREE.Texture
  reflection: THREE.Texture | null
  refraction: THREE.Texture | null
  cubemap: THREE.CubeTexture | null
  refractionDepthEncoding: "source-water-fog-alpha" | null
  linearLightScale?: ReturnType<typeof TSL.uniform>
}>

export type SourceWaterGpuMaterial = Readonly<{
  material: THREE.MeshBasicNodeMaterial
  normalNode: ReturnType<typeof TSL.texture>
}>

export function createSourceWaterMaterial(input: SourceWaterGpuInput): SourceWaterGpuMaterial {
  const { state } = input
  validateState(state)
  const position = input.geometry.getAttribute("position")
  const tangentS = input.geometry.getAttribute("sourceTangentS")
  const tangentT = input.geometry.getAttribute("sourceTangentT")
  const normal = input.geometry.getAttribute("normal")
  if (
    !position || !normal || !tangentS || !tangentT
    || normal.itemSize !== 3 || tangentS.itemSize !== 3 || tangentT.itemSize !== 3
    || normal.count !== position.count || tangentS.count !== position.count || tangentT.count !== position.count
  ) {
    throw new SourceWaterError("Water surface requires authored normal, tangent-S, and tangent-T attributes")
  }
  if (state.mode === "expensive" && !input.reflection && !input.refraction) {
    throw new SourceWaterError("Expensive Water requires an authored reflection or refraction target")
  }
  if (state.mode === "cheap" && !input.cubemap) {
    throw new SourceWaterError("Cheap Water requires its selected authored cubemap")
  }
  if (state.aboveWater && input.refraction && input.refractionDepthEncoding !== "source-water-fog-alpha") {
    throw new SourceWaterError("Above-water refraction requires the Source water-fog depth-alpha target")
  }

  const normalNode = TSL.texture(input.normal)
  const decodedNormal = normalNode.rgb.mul(2).sub(1)
  const linearLightScale = input.linearLightScale ?? TSL.float(state.linearLightScale)
  const tangentSNode = TSL.attribute("sourceTangentS", "vec3")
  const tangentTNode = TSL.attribute("sourceTangentT", "vec3")
  const worldNormal = TSL.normalWorldGeometry
  const worldEye = TSL.cameraPosition.sub(TSL.positionWorld)
  const tangentEye = TSL.vec3(
    worldEye.dot(tangentSNode),
    worldEye.dot(tangentTNode),
    worldEye.dot(worldNormal),
  ).normalize()
  const clip = TSL.clipSpace
  const reflectionUv = TSL.vec2(clip.x.add(clip.w), clip.y.add(clip.w)).mul(0.5).div(clip.w)
  const refractionUv = TSL.vec2(clip.x.add(clip.w), clip.y.negate().add(clip.w)).mul(0.5).div(clip.w)
  const transparent = state.mode === "cheap" && state.cheapBlend
  const material = new THREE.MeshBasicNodeMaterial({
    transparent,
    depthTest: true,
    depthWrite: !transparent,
    side: THREE.FrontSide,
    blending: transparent ? THREE.CustomBlending : THREE.NoBlending,
    ...(transparent ? {
      blendSrc: THREE.SrcAlphaFactor,
      blendDst: THREE.OneMinusSrcAlphaFactor,
    } : {}),
  })
  material.toneMapped = false

  if (state.mode === "cheap") {
    const waterNormal = tangentSNode.mul(decodedNormal.x)
      .add(tangentTNode.mul(decodedNormal.y))
      .add(worldNormal.mul(decodedNormal.z))
    const eyeDistance = worldEye.length()
    const eyeDirection = worldEye.normalize()
    const reflectionDirection = waterNormal.mul(waterNormal.dot(eyeDirection).mul(2))
      .sub(eyeDirection.mul(waterNormal.dot(waterNormal)))
    const cubemap = TSL.cubeTexture(input.cubemap!, reflectionDirection).rgb
    const reflected = cubemap.mul(state.environmentScale).mul(TSL.vec3(...state.reflectTint))
    const fresnel = state.fresnelEnabled
      ? TSL.float(1).sub(eyeDirection.dot(waterNormal).max(0)).pow(5)
      : TSL.float(state.reflectionBlendFactor)

    if (state.cheapBlend) {
      const distanceBlend = eyeDistance.div(state.cheapEnd - state.cheapStart)
        .sub(state.cheapStart / (state.cheapEnd - state.cheapStart)).clamp(0, 1)
      let alpha = fresnel.add(distanceBlend).clamp(0, 1)
      if (input.refraction) {
        alpha = alpha.mul(TSL.texture(input.refraction, refractionUv).a.sub(0.05).mul(20).clamp(0, 1))
      }
      material.colorNode = TSL.vec4(reflected.mul(linearLightScale), alpha)
    } else {
      const fog = state.profile === "hdr"
        ? TSL.vec3(...state.fogColor.map((value) => value ** 2.2) as Vector3)
        : TSL.vec3(...state.fogColor)
      material.colorNode = TSL.vec4(TSL.mix(fog, reflected, fresnel).mul(linearLightScale), 1)
    }
    return Object.freeze({ material, normalNode })
  }

  const depthSample = state.aboveWater
    ? TSL.texture(input.refraction!, refractionUv).a
    : TSL.float(1)
  const distortionDepth = !state.hasBaseTexture && !state.blurRefraction ? depthSample : TSL.float(1)
  const distortion = decodedNormal.xy.mul(normalNode.a).mul(distortionDepth)
  const warpedReflectionUv = reflectionUv.add(distortion.mul(state.reflectAmount))
  const warpedRefractionUv = refractionUv.add(distortion.mul(state.refractAmount))
  const reflectionTint = TSL.vec3(...state.reflectTint.map((value) =>
    sourceShaderGammaToLinear(value) * (state.profile === "hdr" ? 4 : 1),
  ) as Vector3)
  const refractionTint = TSL.vec3(...state.refractTint.map(sourceShaderGammaToLinear) as Vector3)
  const fogColor = TSL.vec3(...state.fogColor.map(sourceShaderGammaToLinear) as Vector3)
    .mul(linearLightScale)

  let reflectionColor: any = null
  if (input.reflection) {
    reflectionColor = TSL.texture(input.reflection, warpedReflectionUv).rgb.mul(reflectionTint)
    if (state.blurRefraction && state.profile === "hdr") reflectionColor = reflectionColor.mul(4)
  }

  let refractionColor: any = null
  let waterDepth: any = depthSample
  if (input.refraction) {
    if (state.blurRefraction) {
      let blurred: any = TSL.vec4(0)
      for (const offset of waterBlurOffsets()) {
        blurred = blurred.add(TSL.texture(input.refraction, warpedRefractionUv.add(TSL.vec2(...offset))))
      }
      blurred = blurred.div(25)
      refractionColor = blurred.rgb.mul(refractionTint)
      if (state.aboveWater) waterDepth = blurred.a
    } else {
      const value = TSL.texture(input.refraction, warpedRefractionUv)
      refractionColor = value.rgb
      if (state.aboveWater) waterDepth = value.a
    }
    const fog = state.aboveWater
      ? waterDepth.sub(0.05).clamp(0, 1)
      : clip.z.sub(state.fogStart).div(state.fogEnd - state.fogStart).clamp(0, 1)
    refractionColor = TSL.mix(refractionColor, fogColor, fog)
  }

  let fresnel: any = TSL.float(1).sub(tangentEye.dot(decodedNormal).clamp(0, 1)).pow(5)
  if (!state.hasBaseTexture) fresnel = fresnel.mul(waterDepth.sub(0.05).mul(20).clamp(0, 1))
  const output = reflectionColor && refractionColor
    ? TSL.mix(refractionColor, reflectionColor, fresnel)
    : reflectionColor ?? refractionColor
  material.colorNode = TSL.vec4(output, 1)
  return Object.freeze({ material, normalNode })
}
