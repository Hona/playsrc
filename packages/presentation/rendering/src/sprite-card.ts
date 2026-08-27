import * as TSL from "three/tsl"

export type SpriteCardInput = Readonly<{
  depthBlend: boolean; blendFrames: boolean; addSelf: number; overbright: number; depthBlendScale: number
  minimumSize: number; startFadeSize: number; endFadeSize: number; maximumSize: number
  maximumDistance: number; farFadeInterval: number
}>

/** SpriteCard's authored size, tint, premultiplication and depth feathering. */
export function spriteCardNodes(state: SpriteCardInput, sampled: any, tint: any, depthTexture: any): { position: any; color: any } {
  const center = TSL.attribute("particleCenter", "vec3")
  const offset = TSL.positionLocal.sub(center)
  const distance = center.sub(TSL.cameraPosition).length().max(0.000001)
  const radius = offset.length().mul(Math.SQRT1_2)
  const minimumRadius = radius.max(distance.mul(state.minimumSize))
  const sizeFade = TSL.float(1).sub(minimumRadius.sub(distance.mul(state.startFadeSize))
    .div(distance.mul(state.endFadeSize - state.startFadeSize)).clamp(0, 1))
  const farStart = Math.max(1, state.maximumDistance - state.farFadeInterval)
  const farFade = TSL.float(1).sub(distance.sub(farStart).div(state.maximumDistance - farStart).clamp(0, 1))
  const faded = tint.mul(TSL.varying(sizeFade.mul(farFade)))
  const finalRadius = minimumRadius.min(distance.mul(state.maximumSize)).mul(sizeFade.greaterThan(0).and(farFade.greaterThan(0)).select(1, 0))
  const position = center.add(offset.mul(finalRadius.div(radius.max(0.000001))))
  let alpha = faded.a
  if (state.depthBlend) {
    const sceneCompressed = depthTexture.a
    const spriteDepth = TSL.viewZToPerspectiveDepth(TSL.positionView.z, TSL.cameraNear, TSL.cameraFar).mul(TSL.positionView.z.negate()).div(192)
    const feather = sceneCompressed.sub(spriteDepth).abs().mul(192 / state.depthBlendScale)
      .max(TSL.smoothstep(0.75, 1, sceneCompressed)).clamp(0, 1)
    alpha = alpha.mul(feather)
  }
  const outputAlpha = sampled.a.mul(alpha)
  let rgb = sampled.rgb.mul(state.overbright)
  if (state.addSelf !== 0) {
    rgb = rgb.mul(outputAlpha)
    rgb = rgb.add(rgb.mul(state.overbright * state.addSelf).mul(alpha))
  }
  return { position, color: TSL.vec4(rgb.mul(faded.rgb), outputAlpha) }
}
