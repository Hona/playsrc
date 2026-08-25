// LightmappedGeneric behavior is adapted from Valve Source SDK 2013;
// see LICENSE.source-sdk-2013.
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"

type Vector3 = readonly [number, number, number]
type Vector4 = readonly [number, number, number, number]

export class SourceLightmappedError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SourceLightmappedError"
  }
}

export type SourceLightmappedEnvironment = Readonly<{
  tint: Vector3
  contrast: number
  saturation: number
  fresnelReflection: number
  environmentScale: number
}>

export type SourceLightmappedPixelRequest = Readonly<{
  base: Vector4
  irradiance: Vector3
  normalSample: Vector4
  tangentS: Vector3
  tangentT: Vector3
  surfaceNormal: Vector3
  eyeVector: Vector3
  cubemap: Vector3
  environment: SourceLightmappedEnvironment
  exposure: number
}>

function validateEnvironment(state: SourceLightmappedEnvironment): void {
  if (![...state.tint, state.contrast, state.saturation, state.fresnelReflection, state.environmentScale].every(Number.isFinite)
    || state.environmentScale < 0) {
    throw new SourceLightmappedError("LightmappedGeneric environment state is invalid")
  }
}

export function evaluateSourceLightmappedPixel(request: SourceLightmappedPixelRequest): Vector4 {
  validateEnvironment(request.environment)
  if (![...request.base, ...request.irradiance, ...request.normalSample, ...request.tangentS,
    ...request.tangentT, ...request.surfaceNormal, ...request.eyeVector, ...request.cubemap,
    request.exposure].every(Number.isFinite) || request.exposure < 0) {
    throw new SourceLightmappedError("LightmappedGeneric pixel input is invalid")
  }
  const eyeLength = Math.hypot(...request.eyeVector)
  if (eyeLength === 0) throw new SourceLightmappedError("LightmappedGeneric eye vector is invalid")
  const decoded = request.normalSample.slice(0, 3).map((value) => value * 2 - 1)
  const normal = request.surfaceNormal.map((value, index) =>
    request.tangentS[index]! * decoded[0]! + request.tangentT[index]! * decoded[1]! + value * decoded[2]!,
  ) as unknown as Vector3
  const dot = normal.reduce((value, component, index) => value + component * request.eyeVector[index]! / eyeLength, 0)
  const fresnel = (1 - dot) ** 5 * (1 - request.environment.fresnelReflection)
    + request.environment.fresnelReflection
  let specular = request.cubemap.map((value, index) =>
    value * request.environment.environmentScale * request.environment.tint[index]!,
  ) as unknown as Vector3
  specular = specular.map((value) =>
    value * (1 - request.environment.contrast) + value * value * request.environment.contrast,
  ) as unknown as Vector3
  const grey = specular[0] * 0.299 + specular[1] * 0.587 + specular[2] * 0.114
  return Object.freeze([
    (request.base[0] * request.irradiance[0]
      + (grey * (1 - request.environment.saturation) + specular[0] * request.environment.saturation) * fresnel) * request.exposure,
    (request.base[1] * request.irradiance[1]
      + (grey * (1 - request.environment.saturation) + specular[1] * request.environment.saturation) * fresnel) * request.exposure,
    (request.base[2] * request.irradiance[2]
      + (grey * (1 - request.environment.saturation) + specular[2] * request.environment.saturation) * fresnel) * request.exposure,
    request.base[3],
  ])
}

export type SourceLightmappedEnvironmentNode = Readonly<{
  normalNode: ReturnType<typeof TSL.texture>
  specular: any
}>

export function createSourceLightmappedEnvironmentNode(input: Readonly<{
  geometry: THREE.BufferGeometry
  normal: THREE.Texture
  cubemap: THREE.CubeTexture
  state: SourceLightmappedEnvironment
}>): SourceLightmappedEnvironmentNode {
  validateEnvironment(input.state)
  const position = input.geometry.getAttribute("position")
  const tangentS = input.geometry.getAttribute("sourceTangentS")
  const tangentT = input.geometry.getAttribute("sourceTangentT")
  const normal = input.geometry.getAttribute("normal")
  if (!position || !normal || !tangentS || !tangentT || normal.itemSize !== 3
    || tangentS.itemSize !== 3 || tangentT.itemSize !== 3
    || normal.count !== position.count || tangentS.count !== position.count || tangentT.count !== position.count) {
    throw new SourceLightmappedError("LightmappedGeneric environment requires authored tangent-S, tangent-T, and normal attributes")
  }

  const normalNode = TSL.texture(input.normal)
  const decoded = normalNode.rgb.mul(2).sub(1)
  const worldNormal = TSL.attribute("sourceTangentS", "vec3").mul(decoded.x)
    .add(TSL.attribute("sourceTangentT", "vec3").mul(decoded.y))
    .add(TSL.normalWorldGeometry.mul(decoded.z))
  const eye = TSL.cameraPosition.sub(TSL.positionWorld)
  const reflection = worldNormal.mul(worldNormal.dot(eye).mul(2))
    .sub(eye.mul(worldNormal.dot(worldNormal)))
  const fresnel = TSL.float(1).sub(worldNormal.dot(eye.normalize())).pow(5)
    .mul(1 - input.state.fresnelReflection).add(input.state.fresnelReflection)
  let specular: any = TSL.cubeTexture(input.cubemap, reflection).rgb
    .mul(input.state.environmentScale)
    .mul(TSL.vec3(...input.state.tint))
  specular = TSL.mix(specular, specular.mul(specular), input.state.contrast)
  const grey = specular.dot(TSL.vec3(0.299, 0.587, 0.114))
  specular = TSL.mix(TSL.vec3(grey), specular, input.state.saturation).mul(fresnel)
  return Object.freeze({ normalNode, specular })
}
