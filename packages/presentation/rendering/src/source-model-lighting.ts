import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { ModelLightingInput } from "./model-lighting"

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
  ambient: readonly [VectorUniform, VectorUniform, VectorUniform, VectorUniform, VectorUniform, VectorUniform]
  local: readonly [LocalLightUniform, LocalLightUniform, LocalLightUniform, LocalLightUniform]
}>

function vector(): VectorUniform {
  return TSL.uniform(new THREE.Vector3(), "vec3")
}

function scalar(value = 0): VectorUniform {
  return TSL.uniform(value, "float")
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
  return Object.freeze({ ambientEnabled: scalar(), ambient: Object.freeze(ambient), local: Object.freeze(local) })
}

function assign(target: VectorUniform, value: readonly [number, number, number]): void {
  ;(target.value as THREE.Vector3).set(value[0], value[1], value[2])
}

export function updateSourceModelLightingUniforms(
  uniforms: SourceModelLightingUniforms,
  input: ModelLightingInput,
): void {
  uniforms.ambientEnabled.value = Number(input.ambientLight)
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

export function sourceModelLightingNode(uniforms: SourceModelLightingUniforms, halfLambert: boolean): any {
  const normal = TSL.normalWorld.normalize()
  const cube = uniforms.ambient
  let result = normal.x.lessThan(0).select(cube[1], cube[0]).mul(normal.x.mul(normal.x))
    .add(normal.y.lessThan(0).select(cube[3], cube[2]).mul(normal.y.mul(normal.y)))
    .add(normal.z.lessThan(0).select(cube[5], cube[4]).mul(normal.z.mul(normal.z)))
    .mul(uniforms.ambientEnabled)

  for (const light of uniforms.local) {
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
    const diffuse = halfLambert
      ? rawDiffuse.mul(0.5).add(0.5).clamp(0, 1).pow(2)
      : rawDiffuse.clamp(0, 1)
    result = result.add(light.color.mul(attenuation).mul(diffuse).mul(light.enabled))
  }
  return result
}

export function sourceStaticVertexLightingNode(): any {
  return TSL.attribute("staticLighting", "vec4").bgra.rgb.mul(2).max(0).pow(2.2)
}

export function sourceStaticVertexLighting(
  encoded: readonly [number, number, number],
): readonly [number, number, number] {
  return Object.freeze(encoded.map((channel) => Math.max(channel * 2, 0) ** 2.2)) as unknown as readonly [number, number, number]
}
