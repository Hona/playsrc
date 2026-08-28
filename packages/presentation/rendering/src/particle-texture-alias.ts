import type * as THREE from "three/webgpu"

const uploadState = ["format", "type", "colorSpace", "channel", "internalFormat", "wrapS", "wrapT", "magFilter", "minFilter",
  "anisotropy", "generateMipmaps", "flipY", "premultiplyAlpha", "unpackAlignment", "matrixAutoUpdate", "rotation", "mapping", "onUpdate"] as const

/** Compare the actual factory result, not a path/hash heuristic. Particle
 * materials retain their own nodes, UVs, blend and animation attributes. Only
 * a complete, identical compressed image/sampler can use an existing owner. */
export function particleTextureAlias(candidate: THREE.Texture, owned: Iterable<THREE.Texture>): THREE.Texture | undefined {
  if (!(candidate as THREE.CompressedTexture).isCompressedTexture || candidate.isRenderTargetTexture || !candidate.source.dataReady || (candidate.image.depth ?? 1) !== 1) return
  const mips = candidate.mipmaps as THREE.CompressedTexture["mipmaps"]
  if (!mips.length) return
  for (const texture of owned) {
    if (!(texture as THREE.CompressedTexture).isCompressedTexture || texture.isRenderTargetTexture || !texture.source.dataReady || (texture.image.depth ?? 1) !== 1) continue
    const prior = texture.mipmaps as THREE.CompressedTexture["mipmaps"]
    // Identity of the private canonical views is stronger than equal contents:
    // independently mutable/decompressed/replaced input spans cannot alias.
    if (prior.length !== mips.length || prior[0]?.data !== mips[0]?.data) continue
    if (uploadState.some(key => texture[key] !== candidate[key]) || texture.image.width !== candidate.image.width || texture.image.height !== candidate.image.height
      || !texture.offset.equals(candidate.offset) || !texture.repeat.equals(candidate.repeat) || !texture.center.equals(candidate.center) || !texture.matrix.equals(candidate.matrix)) continue
    if (mips.some((mip, index) => mip.data !== prior[index]!.data || mip.width !== prior[index]!.width || mip.height !== prior[index]!.height)) continue
    return texture
  }
}
