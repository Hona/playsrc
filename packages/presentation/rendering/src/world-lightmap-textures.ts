import * as THREE from "three/webgpu"
import { configureWorldLightmap } from "./material-state"
import type { RuntimeLightmap } from "./runtime-map"
import { OwnedResourceGeneration } from "./resource-generation"

export type WorldLightmapTextures = readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?]

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
