// Refract shader behavior is adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.

import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { sourceShaderGammaToLinear } from "./color-output"

type Vector2 = readonly [number, number]
type Vector3 = readonly [number, number, number]
type Vector4 = readonly [number, number, number, number]

export type SourceRefractShaderState = Readonly<{
  refractAmount: number
  refractTint: Vector3
  blurAmount: 0 | 1
  ignoreDepth: boolean
}>

export type SourceRefractPixelRequest = Readonly<{
  state: SourceRefractShaderState
  coordinate: Vector2
  normal: Vector4
  tintTexture?: Vector3
  sample: (coordinate: Vector2) => Vector4
}>

export type SourceRefractPixelResult = Readonly<{
  rgba: Vector4
  warpedCoordinate: Vector2
  framebufferSamples: number
}>

export class SourceRefractError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SourceRefractError"
  }
}

function validate(state: SourceRefractShaderState): void {
  if (![state.refractAmount, ...state.refractTint].every(Number.isFinite)
    || (state.blurAmount !== 0 && state.blurAmount !== 1)) {
    throw new SourceRefractError("Refract shader state is invalid")
  }
}

export function evaluateSourceRefractPixel(input: SourceRefractPixelRequest): SourceRefractPixelResult {
  validate(input.state)
  if (![...input.coordinate, ...input.normal, ...(input.tintTexture ?? [])].every(Number.isFinite)) {
    throw new SourceRefractError("Refract pixel input is invalid")
  }
  const scale = input.normal[3] * input.state.refractAmount
  const warpedCoordinate = Object.freeze([
    input.coordinate[0] + (input.normal[0] * 2 - 1) * scale,
    input.coordinate[1] + (input.normal[1] * 2 - 1) * scale,
  ]) as Vector2
  const offsets = input.state.blurAmount === 0
    ? [[0, 0, 1] as const]
    : [
        [-0.5 / 512, -0.5 / 512, 0.4444444] as const,
        [1 / 512, -0.5 / 512, 0.2222222] as const,
        [-0.5 / 512, 1 / 512, 0.2222222] as const,
        [1 / 512, 1 / 512, 0.1111111] as const,
      ]
  const color = [0, 0, 0]
  for (const [offsetX, offsetY, weight] of offsets) {
    const sample = input.sample([warpedCoordinate[0] + offsetX, warpedCoordinate[1] + offsetY])
    if (sample.length !== 4 || !sample.every(Number.isFinite)) {
      throw new SourceRefractError("Refract framebuffer sample is invalid")
    }
    for (let index = 0; index < 3; index++) color[index]! += sample[index]! * weight
  }
  return Object.freeze({
    rgba: Object.freeze([
      color[0]! * sourceShaderGammaToLinear(input.state.refractTint[0]) * (input.tintTexture ? 2 * input.tintTexture[0] : 1),
      color[1]! * sourceShaderGammaToLinear(input.state.refractTint[1]) * (input.tintTexture ? 2 * input.tintTexture[1] : 1),
      color[2]! * sourceShaderGammaToLinear(input.state.refractTint[2]) * (input.tintTexture ? 2 * input.tintTexture[2] : 1),
      input.normal[3],
    ]) as Vector4,
    warpedCoordinate,
    framebufferSamples: offsets.length,
  })
}

export function createSourceRefractMaterial(input: Readonly<{
  state: SourceRefractShaderState
  normal: THREE.Texture
  tint?: THREE.Texture
  framebuffer?: THREE.Texture
}>): Readonly<{ material: THREE.MeshBasicNodeMaterial; normalNode: ReturnType<typeof TSL.texture> }> {
  validate(input.state)
  const normalNode = TSL.texture(input.normal, TSL.uv())
  const warped = TSL.screenUV.add(normalNode.xy.mul(2).sub(1).mul(normalNode.a).mul(input.state.refractAmount))
  const coordinate = input.state.blurAmount === 1 ? warped.add(TSL.vec2(-0.5 / 512, -0.5 / 512)) : warped
  const first = input.framebuffer ? TSL.texture(input.framebuffer, coordinate) : TSL.viewportSharedTexture(coordinate)
  first.value.minFilter = THREE.LinearFilter
  first.value.magFilter = THREE.LinearFilter
  let color: any = first.rgb
  if (input.state.blurAmount === 1) {
    const framebuffer = first.value
    color = color.mul(0.4444444)
      .add(TSL.texture(framebuffer, warped.add(TSL.vec2(1 / 512, -0.5 / 512))).rgb.mul(0.2222222))
      .add(TSL.texture(framebuffer, warped.add(TSL.vec2(-0.5 / 512, 1 / 512))).rgb.mul(0.2222222))
      .add(TSL.texture(framebuffer, warped.add(TSL.vec2(1 / 512, 1 / 512))).rgb.mul(0.1111111))
  }
  const material = new THREE.MeshBasicNodeMaterial({
    transparent: true,
    depthTest: !input.state.ignoreDepth,
    depthWrite: false,
    blending: THREE.NormalBlending,
  })
  if (input.tint) color = color.mul(TSL.texture(input.tint, TSL.uv()).rgb).mul(2)
  material.colorNode = TSL.vec4(
    color.mul(TSL.vec3(...input.state.refractTint.map(sourceShaderGammaToLinear) as unknown as Vector3)),
    normalNode.a,
  )
  material.toneMapped = false
  return Object.freeze({ material, normalNode })
}
