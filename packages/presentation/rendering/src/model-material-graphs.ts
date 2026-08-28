import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { MaterialStateInput } from "./index"
import { ModelLightingGraphs, bindModelTexture, modelBaseTextureShape, modelEnvironmentShape, perObjectModelTextures, type ModelTextureBindingName } from "./model-lighting-graphs"
import { sourceEyeIrisNode, sourceModelSurfaceNode, type SourceModelPhongState } from "./source-model-lighting"
import { sourceFragmentColor } from "./source-fragment-color"
import type { SourceWaterFogUniforms } from "./source-water"

export type ModelMaterialGraphInput = Readonly<{
  shader: string
  state: Readonly<{ halfLambert: boolean; phong?: SourceModelPhongState | null; dilation?: number; ambientOcclusionColor?: readonly [number, number, number]; glossiness?: number }>
  fragment?: MaterialStateInput
  base: any
  baseTexture?: Readonly<{ texture: THREE.Texture; sourceFormat: number | null }>
  textures?: Readonly<{ warp?: THREE.Texture; exponent?: THREE.Texture; iris?: THREE.Texture; ambientOcclusion?: THREE.Texture }>
  environment?: Readonly<{ texture: THREE.CubeTexture; tint: readonly [number, number, number]; scale: number }>
  exposure: any
  waterFog?: SourceWaterFogUniforms
}>

export function swizzleModelTexture(sample: any, sourceFormat: number | null): any {
  return sourceFormat === 1 ? sample.abgr : sourceFormat === 11 ? sample.gbar : sourceFormat === 12 ? sample.bgra : sourceFormat === 16 ? TSL.vec4(sample.bgr, 1) : sample
}

/** Exact graph structure, not material/plane identity or current draw values.
 * Exposure/fog/device lifetime belongs to the supplied scene graph owner. */
export function modelMaterialGraphKey(input: ModelMaterialGraphInput): string {
  const { state, textures, environment, baseTexture } = input
  // Three's TextureNode.getUniformHash uses texture UUID: equal roles can
  // collapse into one binding. Preserve that alias partition without keying
  // the graph on the particular texture objects used by the first draw.
  const roles = [input.shader === "eyes" || input.shader === "eye-refract" ? undefined : baseTexture?.texture,
    textures?.warp, textures?.exponent, textures?.iris, textures?.ambientOcclusion, environment?.texture]
  const aliases = roles.map(texture => texture ? roles.indexOf(texture) : -1)
  return JSON.stringify([input.shader, { ...state, phong: state.phong ? {
    maskSource: state.phong.maskSource, invertMask: state.phong.invertMask, albedoTint: state.phong.albedoTint,
    textureExponent: state.phong.exponent < 0, factorExponent: state.phong.exponentFactor !== 0,
    rim: state.phong.rim ? { exponentTextureAlphaMask: state.phong.rim.exponentTextureAlphaMask } : null,
  } : null }, input.fragment,
  input.shader === "eyes" || input.shader === "eye-refract" ? "iris" : baseTexture ? modelBaseTextureShape(baseTexture.texture, baseTexture.sourceFormat) : input.base.uuid,
  ...[textures?.warp, textures?.exponent, textures?.iris, textures?.ambientOcclusion].map(texture => texture ? modelBaseTextureShape(texture, 0) : "none"),
  modelEnvironmentShape(environment?.texture), environment?.tint, environment?.scale, aliases])
}

/** Used by real model admission/drawing and deterministic compiler acceptance. */
export function modelMaterialGraph(mesh: THREE.Mesh, graphs: ModelLightingGraphs, input: ModelMaterialGraphInput, created?: () => void): any {
  const { state, baseTexture, textures, environment } = input
  if (state.phong) graphs.bindPhong(mesh, state.phong)
  if (baseTexture) bindModelTexture(mesh, "sourceBaseTexture", baseTexture.texture)
  for (const [name, texture] of [["sourceWarpTexture", textures?.warp], ["sourceExponentTexture", textures?.exponent],
    ["sourceIrisTexture", textures?.iris], ["sourceAmbientOcclusionTexture", textures?.ambientOcclusion], ["sourceEnvironment", environment?.texture]] as const) {
    if (texture) bindModelTexture(mesh, name, texture)
  }
  return graphs.get(modelMaterialGraphKey(input), () => {
    created?.()
    const bindings: { name: ModelTextureBindingName; node: any }[] = []
    const sampler = (name: ModelTextureBindingName, texture?: THREE.Texture) => {
      if (!texture) return undefined
      const node = TSL.texture(texture, TSL.uv()); bindings.push({ name, node }); return node
    }
    const eye = input.shader === "eyes" || input.shader === "eye-refract"
    const base = eye ? sourceEyeIrisNode(sampler("sourceIrisTexture", textures?.iris)!, graphs.eyes, state.dilation!, input.shader === "eye-refract")
      : baseTexture ? swizzleModelTexture(sampler("sourceBaseTexture", baseTexture.texture), baseTexture.sourceFormat) : input.base
    const shaded = sourceModelSurfaceNode(base, graphs.lighting, {
      halfLambert: state.phong ? true : state.halfLambert, phong: state.phong, phongUniforms: graphs.phong,
      diffuseWarp: sampler("sourceWarpTexture", textures?.warp), exponentTexture: sampler("sourceExponentTexture", textures?.exponent), environment,
      ...(input.shader === "eye-refract" ? { eye: { ambientOcclusion: sampler("sourceAmbientOcclusionTexture", textures?.ambientOcclusion),
        ambientOcclusionColor: state.ambientOcclusionColor!, glossiness: state.glossiness! } } : {}),
    }, input.exposure)
    if (shaded.environmentNode) bindings.push({ name: "sourceEnvironment", node: shaded.environmentNode })
    const color = bindings.length ? perObjectModelTextures(shaded.color, bindings) : shaded.color
    return sourceFragmentColor(color, input.fragment, input.waterFog)
  })
}
