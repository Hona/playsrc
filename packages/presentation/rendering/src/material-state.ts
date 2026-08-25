import * as THREE from "three/webgpu"

export function worldMaterialSide(features: number): THREE.Side {
  return (features & 8) !== 0 ? THREE.DoubleSide : THREE.FrontSide
}

export function sourceDepthBias(category: "none" | "decal"): Readonly<{ enabled: boolean; slopeScale: number; units: number }> {
  return category === "decal"
    ? Object.freeze({ enabled: true, slopeScale: -0.5, units: -262_144 })
    : Object.freeze({ enabled: false, slopeScale: 0, units: 0 })
}

export type ParticleDepthState = Readonly<{
  depthTest: boolean
  depthWrite: boolean
  depthFunction: number
  blendEnabled: boolean
}>

export function applyParticleDepthState(material: THREE.Material, state: ParticleDepthState): void {
  if ((state.depthWrite && (!state.depthTest || state.blendEnabled)) || (state.depthFunction !== 0 && state.depthFunction !== 1)) {
    throw new Error("Source Particle material depth state is invalid")
  }
  material.depthTest = state.depthTest
  material.depthWrite = state.depthWrite
  material.depthFunc = state.depthFunction === 0 ? THREE.LessDepth : THREE.LessEqualDepth
}

export function configureWorldLightmap(texture: THREE.Texture, profile: "ldr" | "hdr" = "ldr"): void {
  texture.colorSpace = THREE.NoColorSpace
  texture.channel = 1
  texture.flipY = false
  texture.minFilter = profile === "hdr" ? THREE.LinearFilter : THREE.NearestFilter
  texture.magFilter = profile === "hdr" ? THREE.LinearFilter : THREE.NearestFilter
  texture.generateMipmaps = false
  texture.wrapS = THREE.ClampToEdgeWrapping
  texture.wrapT = THREE.ClampToEdgeWrapping
  texture.needsUpdate = true
}
