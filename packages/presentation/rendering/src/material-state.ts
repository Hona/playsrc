import * as THREE from "three/webgpu"

export function worldMaterialSide(features: number): THREE.Side {
  return (features & 8) !== 0 ? THREE.DoubleSide : THREE.FrontSide
}

export function configureWorldLightmap(texture: THREE.Texture): void {
  texture.colorSpace = THREE.NoColorSpace
  texture.channel = 1
  texture.flipY = false
  texture.minFilter = THREE.NearestFilter
  texture.magFilter = THREE.NearestFilter
  texture.generateMipmaps = false
  texture.wrapS = THREE.ClampToEdgeWrapping
  texture.wrapT = THREE.ClampToEdgeWrapping
  texture.needsUpdate = true
}
