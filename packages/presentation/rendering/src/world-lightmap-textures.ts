import * as THREE from "three/webgpu"
import { configureWorldLightmap } from "./material-state"
import type { RuntimeLightmap } from "./runtime-map"
import { OwnedResourceGeneration } from "./resource-generation"

export type WorldLightmapTextures = readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?]

/** Borrow only the private, immutable LDR plane of an admitted exact source.
 * Style-addressable planes always get independent staging textures: a live
 * style update must not change a candidate while its pipelines are preparing.
 * No hashing, sample copying, cross-map cache, or early ownership transfer. */
export function borrowWorldLightmapTextures(lightmap: RuntimeLightmap, source: WorldLightmapTextures | undefined): WorldLightmapTextures | undefined {
  if (lightmap.profile !== "ldr" || lightmap.directional || !source || source.length !== 1) return
  const texture = source[0], image = texture.image
  if (image.data !== lightmap.flat || image.width !== lightmap.width || image.height !== lightmap.height
    || texture.type !== THREE.FloatType || texture.format !== THREE.RGBAFormat || texture.colorSpace !== THREE.NoColorSpace
    || texture.channel !== 1 || texture.flipY || texture.generateMipmaps || texture.mipmaps.length
    || texture.anisotropy !== 1 || texture.premultiplyAlpha || texture.unpackAlignment !== 1 || texture.internalFormat !== null
    || texture.minFilter !== THREE.NearestFilter || texture.magFilter !== THREE.NearestFilter
    || texture.wrapS !== THREE.ClampToEdgeWrapping || texture.wrapT !== THREE.ClampToEdgeWrapping) return
  return source
}

export function createWorldLightmapTextures(lightmap: RuntimeLightmap, generation: OwnedResourceGeneration): WorldLightmapTextures {
  return [lightmap.flat, ...(lightmap.directional ?? [])].map(plane => {
    const texture = new THREE.DataTexture(plane, lightmap.width, lightmap.height, THREE.RGBAFormat, THREE.FloatType)
    configureWorldLightmap(texture, lightmap.profile)
    return generation.add(texture)
  }) as unknown as WorldLightmapTextures
}

export function replaceWorldLightmapData(textures: WorldLightmapTextures, lightmap: RuntimeLightmap): void {
  const planes = [lightmap.flat, ...(lightmap.directional ?? [])]
  if (planes.length !== textures.filter(Boolean).length) throw new Error("lightmap plane count changed")
  for (let index = 0; index < planes.length; index++) {
    const texture = textures[index]!
    const image = texture.image as { data: Float32Array; width: number; height: number }
    image.data = planes[index]!
    image.width = lightmap.width
    image.height = lightmap.height
    texture.needsUpdate = true
  }
}
