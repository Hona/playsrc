import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { AdditiveSpriteInput, MaterialStateInput } from "./index"
import { spriteCardNodes, type SpriteCardInput } from "./sprite-card"
import { sourceFragmentColor } from "./source-fragment-color"
import { sourceFragmentUsesAlpha } from "./material-state"
import { modelBaseTextureShape } from "./model-lighting-graphs"
import type { SourceWaterFogUniforms } from "./source-water"

export type ParticleGraphInput = Readonly<{
  texture: THREE.Texture; state: MaterialStateInput; spriteCard?: SpriteCardInput | null; additive?: AdditiveSpriteInput | null
  waterFog: SourceWaterFogUniforms; depth: any; exposure: any; hdr: boolean
  fog: Readonly<{ start: any; end: any; maximumDensity: any; enabled: any }>
}>

export function particleGraphKey(input: ParticleGraphInput): string {
  const { state } = input
  return JSON.stringify([modelBaseTextureShape(input.texture, 0), input.spriteCard, input.additive, input.hdr,
    state.alphaModulation, sourceFragmentUsesAlpha(state), state.blendEnabled, state.fragmentDiscard, state.fog])
}

/** The dedicated and shared owners use this exact shader construction. Only
 * the texture draw binding differs; authored arithmetic remains untouched. */
export function particleMaterialNodes(input: ParticleGraphInput, bind?: (nodes: readonly any[], color: any) => any) {
  const { texture, state, spriteCard, additive, waterFog, depth, exposure, hdr, fog } = input
  const current = TSL.texture(texture, TSL.uv()), next = TSL.texture(texture, TSL.attribute("particleUvNext", "vec2"))
  const blend = TSL.attribute("particleSheetBlend", "float"), color = TSL.attribute("particleColor", "vec4")
  const sampled = spriteCard?.blendFrames === false ? current : current.mul(TSL.float(1).sub(blend)).add(next.mul(blend))
  const sprite = spriteCard ? spriteCardNodes(spriteCard, sampled, color, depth) : null
  let output = sourceFragmentColor(sprite?.color ?? TSL.vec4(sampled.rgb.mul(color.rgb), sampled.a.mul(color.a)), state, waterFog)
  if (additive) {
    const tint = TSL.vec3(...additive.color.map(value => additive.srgb ? Math.pow(value, 2.2) : value) as [number, number, number])
    let rgb = current.rgb.mul(tint), alpha = current.a.mul(state.alphaModulation)
    if (additive.vertexColor) { rgb = rgb.mul(additive.srgb ? color.rgb.pow(2.2) : color.rgb); alpha = alpha.mul(color.a) }
    if (hdr) rgb = rgb.mul(additive.srgb ? Math.pow(additive.hdrColorScale, 2.2) : additive.hdrColorScale)
    rgb = rgb.mul(additive.srgb ? exposure : exposure.pow(1 / 2.2))
    const fogFactor = TSL.positionView.z.negate().sub(fog.start).div(fog.end.sub(fog.start)).clamp(0, fog.maximumDensity)
    if (state.fog !== 2) {
      const waterFraction = waterFog.waterHeight.sub(TSL.positionWorld.z).div(waterFog.eyeHeight.sub(TSL.positionWorld.z)).clamp(0, 1)
      const waterDepth = waterFraction.mul(TSL.clipSpace.z).mul(waterFog.inverseFogRange).clamp(0, 1)
      const factor = waterFog.enabled.greaterThan(0).select(waterDepth, fog.enabled.greaterThan(0).select(fogFactor, 0))
      rgb = rgb.mul(TSL.float(1).sub(factor))
    }
    output = TSL.vec4(additive.srgb ? rgb : rgb.max(0).pow(2.2), alpha)
  }
  return { color: bind ? bind([current, next], output) : output, position: sprite?.position }
}

export function bindParticleTexture(material: THREE.Material, texture: THREE.Texture): void {
  Object.defineProperty(material.userData, "sourceParticleTexture", { value: texture, configurable: true })
}

/** One invocation belongs to one particle material/resource generation. No
 * graph or texture can be borrowed from another scene, fog or device owner. */
export class ParticleMaterialGraphs {
  readonly #graphs = new Map<string, ReturnType<typeof particleMaterialNodes>>()
  get(material: THREE.Material, input: ParticleGraphInput) {
    bindParticleTexture(material, input.texture)
    const key = particleGraphKey(input)
    let graph = this.#graphs.get(key)
    if (!graph) {
      graph = particleMaterialNodes(input, (nodes, color) => TSL.Fn(() => {
        TSL.OnObjectUpdate(({ material }: { material: THREE.Material }) => {
          for (const node of nodes) node.value = material.userData.sourceParticleTexture
        })
        return color
      })())
      this.#graphs.set(key, graph)
    }
    return graph
  }
  get size() { return this.#graphs.size }
}
