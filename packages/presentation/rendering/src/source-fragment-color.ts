import * as TSL from "three/tsl"
import type { MaterialStateInput } from "./index"
import { sourceFragmentUsesAlpha } from "./material-state"
import { sourceWaterFogFragment, type SourceWaterFogUniforms } from "./source-water"

export function sourceFragmentColor(sample: any, state?: MaterialStateInput, waterFogUniforms?: SourceWaterFogUniforms, dynamicFade = false): any {
  const alpha = sample.a.mul(state?.alphaModulation ?? 1)
  const authored = TSL.vec4(sample.rgb, sourceFragmentUsesAlpha(state, dynamicFade) ? alpha : 1)
  const color = waterFogUniforms && !state?.blendEnabled ? sourceWaterFogFragment(authored, waterFogUniforms) : authored
  if (state?.fragmentDiscard.kind !== "alpha") return color
  return TSL.Fn(() => {
    const rejected = state.fragmentDiscard.pass === "greater" ? alpha.lessThanEqual(state.fragmentDiscard.reference) : alpha.lessThan(state.fragmentDiscard.reference)
    rejected.discard()
    return color
  })()
}
