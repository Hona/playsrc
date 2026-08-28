import * as THREE from "three/webgpu"

/** A fade-capable prop is still opaque at full alpha. Keep pass state immutable:
 * nearby and distant occurrences can share the shader but not a mutable queue. */
export function createStaticPropFadeVariant(authored:THREE.MeshBasicNodeMaterial):THREE.MeshBasicNodeMaterial {
  const faded=authored.clone()
  faded.transparent=true
  faded.depthWrite=false
  if(faded.blending===THREE.NoBlending)faded.blending=THREE.NormalBlending
  return faded
}

export type StaticPropFadeBinding=Readonly<{mesh:THREE.Mesh;authored:THREE.MeshBasicNodeMaterial;faded:THREE.MeshBasicNodeMaterial}>
export function selectStaticPropFadePass(bindings:readonly StaticPropFadeBinding[],opacity:number):void {
  for(const binding of bindings)binding.mesh.material=opacity===1?binding.authored:binding.faded
}

export function distanceFadeOpacity(distanceSquared: number, minimumDistance: number, maximumDistance: number): number {
  const minimum = minimumDistance * minimumDistance
  const maximum = maximumDistance * maximumDistance
  if (distanceSquared >= maximum) return 0
  if (minimum >= 0 && distanceSquared > minimum) {
    return Math.max(0, Math.min(1, (maximum - distanceSquared) / (maximum - minimum)))
  }
  return 1
}

export function screenFadeOpacity(pixelWidth: number, minimumWidth: number, maximumWidth: number): number {
  if (pixelWidth <= minimumWidth) return 0
  if (maximumWidth >= 0 && pixelWidth < maximumWidth) {
    return Math.max(0, Math.min(1, (pixelWidth - minimumWidth) / (maximumWidth - minimumWidth)))
  }
  return 1
}

export function quantizeStaticPropOpacity(opacity: number): number {
  return Math.trunc(Math.max(0, Math.min(1, opacity)) * 255) / 255
}
