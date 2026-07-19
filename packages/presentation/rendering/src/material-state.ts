import * as THREE from "three/webgpu"

export function worldMaterialSide(features: number): THREE.Side {
  return (features & 8) !== 0 ? THREE.DoubleSide : THREE.FrontSide
}

export function sourceDepthBias(category: "none" | "decal"): Readonly<{ enabled: boolean; slopeScale: number; units: number }> {
  return category === "decal"
    ? Object.freeze({ enabled: true, slopeScale: -0.5, units: -262_144 })
    : Object.freeze({ enabled: false, slopeScale: 0, units: 0 })
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
