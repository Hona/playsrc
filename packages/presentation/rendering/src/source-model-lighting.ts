// Source model, eye, and static-vertex lighting behavior is adapted from Valve Source SDK 2013;
// see LICENSE.source-sdk-2013.
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { ModelEyeState, ModelLightingInput } from "./model-lighting"

type VectorUniform = ReturnType<typeof TSL.uniform>

type LocalLightUniform = Readonly<{
  enabled: VectorUniform
  kind: VectorUniform
  color: VectorUniform
  position: VectorUniform
  direction: VectorUniform
  attenuation: VectorUniform
  falloff: VectorUniform
  theta: VectorUniform
  phi: VectorUniform
}>

export type SourceModelLightingUniforms = Readonly<{
  ambientEnabled: VectorUniform
  cameraPosition: VectorUniform
  ambient: readonly [VectorUniform, VectorUniform, VectorUniform, VectorUniform, VectorUniform, VectorUniform]
  local: readonly [LocalLightUniform, LocalLightUniform, LocalLightUniform, LocalLightUniform]
}>

export type SourceModelEyeUniforms = Readonly<{
  irisU: VectorUniform
  irisV: VectorUniform
  glintU: VectorUniform
  glintV: VectorUniform
  origin: VectorUniform
}>

function vector(): VectorUniform {
  return TSL.uniform(new THREE.Vector3(), "vec3")
}

function scalar(value = 0): VectorUniform {
  return TSL.uniform(value, "float")
}

export function createSourceModelEyeUniforms(): SourceModelEyeUniforms {
  const row = () => TSL.uniform(new THREE.Vector4(), "vec4")
  return Object.freeze({ irisU: row(), irisV: row(), glintU: row(), glintV: row(), origin: vector() })
}

export function updateSourceModelEyeUniforms(uniforms: SourceModelEyeUniforms, eye: ModelEyeState): void {
  const row = (target: VectorUniform, value: readonly [number, number, number, number]) => {
    ;(target.value as THREE.Vector4).set(value[0], value[1], value[2], value[3])
  }
  row(uniforms.irisU, eye.irisU)
  row(uniforms.irisV, eye.irisV)
  row(uniforms.glintU, eye.glintU)
  row(uniforms.glintV, eye.glintV)
  assign(uniforms.origin, eye.worldOrigin)
}

export function sourceEyeIrisNode(
  iris: THREE.Texture,
  eye: SourceModelEyeUniforms,
  dilation: number,
  refract: boolean,
): any {
  const position = TSL.vec4(TSL.positionWorld, 1)
  const projected = TSL.vec2(eye.irisU.dot(position), eye.irisV.dot(position))
  const initial = refract ? projected.mul(0.5).add(0.25) : projected
  const centered = initial.sub(0.5)
  const radius = centered.length().div(0.2).clamp(0, 1)
  const amount = Math.max(0, Math.min(1, dilation)) * 2.5 - 1.25
  const uv = centered.mul(radius.sub(1).mul(amount).add(1)).add(0.5)
  return TSL.texture(iris, uv)
}

export function createSourceModelLightingUniforms(): SourceModelLightingUniforms {
  const ambient = Array.from({ length: 6 }, vector) as unknown as SourceModelLightingUniforms["ambient"]
  const local = Array.from({ length: 4 }, (): LocalLightUniform => Object.freeze({
    enabled: scalar(),
    kind: scalar(),
    color: vector(),
    position: vector(),
    direction: vector(),
    attenuation: vector(),
    falloff: scalar(),
    theta: scalar(),
    phi: scalar(),
  })) as unknown as SourceModelLightingUniforms["local"]
  for (const light of local) {
    assign(light.direction, [0, 0, -1])
    assign(light.attenuation, [1, 0, 0])
    light.falloff.value = 1
    light.phi.value = Math.PI
  }
  return Object.freeze({ ambientEnabled: scalar(), cameraPosition: vector(), ambient: Object.freeze(ambient), local: Object.freeze(local) })
}

function assign(target: VectorUniform, value: readonly [number, number, number]): void {
  ;(target.value as THREE.Vector3).set(value[0], value[1], value[2])
}

export function updateSourceModelLightingUniforms(
  uniforms: SourceModelLightingUniforms,
  input: ModelLightingInput,
): void {
  uniforms.ambientEnabled.value = Number(input.ambientLight)
  assign(uniforms.cameraPosition, input.cameraPosition)
  for (let side = 0; side < 6; side += 1) assign(uniforms.ambient[side]!, input.ambientCube[side]!)
  for (let index = 0; index < 4; index += 1) {
    const target = uniforms.local[index]!
    const light = input.localLights[index]
    target.enabled.value = Number(light !== undefined)
    if (!light) continue
    target.kind.value = light.kind === "point" ? 0 : light.kind === "directional" ? 1 : 2
    assign(target.color, light.color)
    assign(target.position, light.position)
    assign(target.direction, light.direction)
    assign(target.attenuation, light.attenuation)
    target.falloff.value = light.falloff
    target.theta.value = light.theta
    target.phi.value = light.phi
  }
}

function ambientCubeNode(uniforms: SourceModelLightingUniforms, normal: any): any {
  const cube = uniforms.ambient
  return normal.x.lessThan(0).select(cube[1], cube[0]).mul(normal.x.mul(normal.x))
    .add(normal.y.lessThan(0).select(cube[3], cube[2]).mul(normal.y.mul(normal.y)))
    .add(normal.z.lessThan(0).select(cube[5], cube[4]).mul(normal.z.mul(normal.z)))
    .mul(uniforms.ambientEnabled)
}

export type SourceModelPhongState = Readonly<{
  maskSource: number
  invertMask: boolean
  albedoTint: boolean
  exponent: number
  exponentFactor: number
  tint: readonly [number, number, number]
  boost: number
  packedFresnel: readonly [number, number, number]
  rim: null | Readonly<{ exponent: number; boost: number; exponentTextureAlphaMask: boolean }>
}>

export type SourceModelSurface = Readonly<{
  halfLambert: boolean
  diffuseWarp?: THREE.Texture
  exponentTexture?: THREE.Texture
  phong?: SourceModelPhongState | null
  environment?: Readonly<{
    texture: THREE.CubeTexture
    tint: readonly [number, number, number]
    scale: number
  }>
}>

export function sourceModelLightingNode(
  uniforms: SourceModelLightingUniforms,
  halfLambert: boolean,
  diffuseWarp?: THREE.Texture,
): any {
  const normal = TSL.normalWorld.normalize()
  return TSL.Fn(() => {
    const result = ambientCubeNode(uniforms, normal).toVar()
    for (const light of uniforms.local) {
      TSL.If(light.enabled.greaterThan(0.5), () => {
        const directional = light.kind.greaterThan(0.5).and(light.kind.lessThan(1.5))
        const spot = light.kind.greaterThan(1.5)
        const delta = light.position.sub(TSL.positionWorld)
        const distance = delta.length().max(0.0001)
        const toward = delta.div(distance)
        const direction = directional.select(light.direction.negate().normalize(), toward)
        const denominator = light.attenuation.x
          .add(light.attenuation.y.mul(distance))
          .add(light.attenuation.z.mul(distance.mul(distance)))
          .max(0.0001)
        let attenuation = directional.select(TSL.float(1), denominator.reciprocal())
        const cone = light.direction.normalize().dot(toward.negate())
        const inner = light.theta.mul(0.5).cos()
        const outer = light.phi.mul(0.5).cos()
        const spread = inner.sub(outer)
        const reciprocal = spread.greaterThan(1e-10).select(spread.reciprocal(), TSL.float(1))
        const factor = cone.sub(outer).mul(reciprocal).max(0.0001).pow(light.falloff).clamp(0, 1)
        attenuation = spot.select(cone.lessThanEqual(outer).select(TSL.float(0), attenuation.mul(factor)), attenuation)
        const rawDiffuse = normal.dot(direction)
        let diffuse = halfLambert
          ? rawDiffuse.mul(0.5).add(0.5).clamp(0, 1)
          : rawDiffuse.clamp(0, 1)
        if (diffuseWarp) {
          diffuse = TSL.texture(diffuseWarp, TSL.vec2(diffuse, 0.5)).rgb.mul(2)
        } else if (halfLambert) {
          diffuse = diffuse.pow(2)
        }
        result.addAssign(light.color.mul(attenuation).mul(diffuse))
      })
    }
    return result
  })()
}

export function sourceModelSurfaceNode(
  base: any,
  uniforms: SourceModelLightingUniforms,
  state: SourceModelSurface,
  exposure: any,
): any {
  const normal = TSL.normalWorld.normalize()
  const eye = uniforms.cameraPosition.sub(TSL.positionWorld).normalize()
  let result = base.rgb.mul(sourceModelLightingNode(uniforms, state.halfLambert, state.diffuseWarp))
  const phong = state.phong
  if (phong) {
    const exponentSample = state.exponentTexture ? TSL.texture(state.exponentTexture, TSL.uv()) : undefined
    const exponent = exponentSample
      ? phong.exponentFactor !== 0
        ? exponentSample.r.mul(phong.exponentFactor).add(1)
        : phong.exponent >= 0 ? TSL.float(phong.exponent) : exponentSample.r.mul(149).add(1)
      : TSL.float(Math.max(phong.exponent, 0))
    const reflection = normal.mul(normal.dot(eye).mul(2)).sub(eye)
    const fresnelAmount = TSL.float(1).sub(normal.dot(eye)).clamp(0, 1)
    const fresnelOffset = fresnelAmount.mul(fresnelAmount).sub(0.5)
    const fresnel = TSL.float(phong.packedFresnel[1]).add(fresnelOffset.greaterThanEqual(0)
      .select(TSL.float(phong.packedFresnel[2]), TSL.float(phong.packedFresnel[0]))
      .mul(fresnelOffset))
    const specular = TSL.Fn(() => {
      const highlights = TSL.vec3(0).toVar()
      const rim = TSL.vec3(0).toVar()
      for (const light of uniforms.local) {
        TSL.If(light.enabled.greaterThan(0.5), () => {
          const directional = light.kind.greaterThan(0.5).and(light.kind.lessThan(1.5))
          const delta = light.position.sub(TSL.positionWorld)
          const distance = delta.length().max(0.0001)
          const direction = directional.select(light.direction.negate().normalize(), delta.div(distance))
          const denominator = light.attenuation.x
            .add(light.attenuation.y.mul(distance))
            .add(light.attenuation.z.mul(distance.mul(distance)))
            .max(0.0001)
          const attenuation = directional.select(TSL.float(1), denominator.reciprocal())
          const facing = normal.dot(direction).clamp(0, 1)
          const reflected = reflection.dot(direction).clamp(0, 1)
          const color = light.color.mul(attenuation).mul(facing)
          highlights.addAssign(color.mul(reflected.pow(exponent)))
          if (phong.rim) rim.addAssign(color.mul(reflected.pow(phong.rim.exponent)))
        })
      }
      let mask = phong.maskSource === 0 ? base.a : TSL.float(1)
      if (phong.invertMask) mask = TSL.float(1).sub(mask)
      highlights.mulAssign(mask.mul(fresnel).mul(phong.boost))
      if (phong.rim) {
        const rimMask = phong.rim.exponentTextureAlphaMask && exponentSample ? exponentSample.a : TSL.float(1)
        const rimFresnel = fresnelAmount.pow(4).mul(rimMask)
        rim.mulAssign(rimFresnel)
        highlights.assign(TSL.max(highlights, rim))
        highlights.addAssign(ambientCubeNode(uniforms, eye)
          .mul(phong.rim.boost)
          .mul(rimFresnel.mul(normal.z).clamp(0, 1)))
      }
      return highlights
    })()
    const tint = phong.albedoTint && exponentSample
      ? TSL.mix(TSL.vec3(1), base.rgb, exponentSample.g)
      : TSL.vec3(...phong.tint)
    result = result.add(specular.mul(tint))
  }
  if (state.environment) {
    const reflection = normal.mul(normal.dot(eye).mul(2)).sub(eye)
    result = result.add(TSL.cubeTexture(state.environment.texture, reflection).rgb
      .mul(TSL.vec3(...state.environment.tint))
      .mul(base.a)
      .mul(state.environment.scale))
  }
  return TSL.vec4(result.mul(exposure), base.a)
}

export function sourceStaticVertexLightingNode(): any {
  return TSL.attribute("staticLighting", "vec4").bgra.rgb.mul(2).max(0).pow(2.2)
}

export function sourceStaticVertexLighting(
  encoded: readonly [number, number, number],
): readonly [number, number, number] {
  return Object.freeze(encoded.map((channel) => Math.max(channel * 2, 0) ** 2.2)) as unknown as readonly [number, number, number]
}
