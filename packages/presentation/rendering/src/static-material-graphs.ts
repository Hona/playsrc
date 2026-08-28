import * as TSL from "three/tsl"
import type * as THREE from "three/webgpu"
import type { MaterialStateInput } from "./index"
import { sourceFragmentUsesAlpha } from "./material-state"
import { sourceFragmentColor } from "./source-fragment-color"
import { sourceStaticVertexLightingNode } from "./source-model-lighting"
import type { SourceWaterFogUniforms } from "./source-water"

function fragmentKey(state?: MaterialStateInput) {
  return [state?.alphaModulation ?? 1, sourceFragmentUsesAlpha(state), Boolean(state?.blendEnabled), state?.fragmentDiscard]
}

/** Exact plane/node identity is retained: this does not collapse different
 * textures or move any first-use upload past pipeline readiness. One owner is
 * used only while constructing one scene's immutable template/VHV materials. */
export class StaticMaterialGraphs {
  static releasePreparationIdentity(material: THREE.Material) { delete material.userData.sourcePreparationIdentity }
  readonly #templates = new Map<string, any>()
  readonly #static = new Map<string, any>()
  get size() { return this.#static.size }
  constructor(readonly waterFog: SourceWaterFogUniforms, readonly exposure: any, readonly staticFade: any) {}

  template(base: any, state?: MaterialStateInput, created?: any) {
    const key=JSON.stringify([base.uuid,fragmentKey(state)])
    let color=this.#templates.get(key)
    if(!color){color=created??sourceFragmentColor(base,state,this.waterFog);this.#templates.set(key,color)}
    return color
  }

  vertex(base: any, state: MaterialStateInput | undefined, unlit: boolean, fading: boolean, fade: any) {
    if(!fading&&fade.value!==1) throw new Error("Non-fading static materials require their fixed alpha-one uniform")
    const key=JSON.stringify([base.uuid,fragmentKey(state),state?.alphaOwnership.opacity??false,unlit,fading])
    let color=this.#static.get(key)
    if(!color){
      const rgb=unlit?base.rgb:base.rgb.mul(sourceStaticVertexLightingNode()).mul(this.exposure)
      const opacity=state?.alphaOwnership.opacity?base.a:TSL.float(1)
      // Non-fading Source occurrences always publish alpha1. Keep the original
      // uniform and shader arithmetic; fading occurrences still bind per draw.
      color=sourceFragmentColor(TSL.vec4(rgb,opacity.mul(fading?this.staticFade:fade)),state,this.waterFog,fading)
      this.#static.set(key,color)
    }
    return color
  }
}
