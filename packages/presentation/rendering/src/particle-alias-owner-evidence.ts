import type * as THREE from "three/webgpu"

/** Opt-in receipt, not a renderer/cache. Hashes are read only after sampling. */
export function installParticleAliasOwnerReceipt() {
  const entries = new Map<string, { texture: WeakRef<THREE.Texture>; material: WeakRef<THREE.Material> }>()
  return {
    register(name: string, material: THREE.Material, texture: THREE.Texture) {
      entries.set(name, { texture: new WeakRef(texture), material: new WeakRef(material) })
    },
    async snapshot() {
      const images = new Set<THREE.Texture>(), records = []
      for (const [name, references] of entries) {
        const texture = references.texture.deref(), material = references.material.deref()
        if (!texture || !material) throw new Error("Particle owner retired before receipt")
        images.add(texture)
        const mips = []
        for (const mip of texture.mipmaps as THREE.CompressedTexture["mipmaps"]) {
          const bytes = new Uint8Array(mip.data.buffer, mip.data.byteOffset, mip.data.byteLength)
          const hash = await crypto.subtle.digest("SHA-256", bytes)
          mips.push({ width: mip.width, height: mip.height, bytes: bytes.length, sha256: Array.from(new Uint8Array(hash), value => value.toString(16).padStart(2, "0")).join("") })
        }
        records.push({ name, image: texture.name, mips, width: texture.image.width, height: texture.image.height,
          format: texture.format, type: texture.type, colorSpace: texture.colorSpace, minFilter: texture.minFilter, magFilter: texture.magFilter,
          wrapS: texture.wrapS, wrapT: texture.wrapT, anisotropy: texture.anisotropy, generateMipmaps: texture.generateMipmaps,
          flipY: texture.flipY, premultiplyAlpha: texture.premultiplyAlpha, unpackAlignment: texture.unpackAlignment,
          side: material.side, depthTest: material.depthTest, depthWrite: material.depthWrite, depthFunc: material.depthFunc,
          blending: material.blending, blendSrc: material.blendSrc, blendDst: material.blendDst, blendEquation: material.blendEquation,
          blendSrcAlpha: material.blendSrcAlpha, blendDstAlpha: material.blendDstAlpha, blendEquationAlpha: material.blendEquationAlpha })
      }
      return { uniqueImages: images.size, records }
    },
  }
}
