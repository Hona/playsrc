import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import Textures from "three/src/renderers/common/Textures.js"
import { createWorldLightmapTextures, replaceWorldLightmapData } from "../src/world-lightmap-textures"
import { OwnedResourceGeneration } from "../src/resource-generation"
import { parseRuntimeMap, buildRuntimeLightmap, type RuntimeLightmap } from "../src/runtime-map"
import { hdrFixture } from "./hdr-fixture"

// Real Three texture/version/disposal owner; only the native device boundary is
// recorded. This proves API work and source aliasing, NOT physical residency.
function ownerLoop() {
  const state = { liveBytes: 0, peakBytes: 0, createdBytes: 0, uploadBytes: 0, destroyedBytes: 0 }
  const backend = {
    createTexture(texture: THREE.DataTexture, options: any) {
      expect(options.levels).toBe(1); expect(options.needsMipmaps).toBe(false)
      const bytes = texture.image.data.byteLength
      state.liveBytes += bytes; state.createdBytes += bytes; state.peakBytes = Math.max(state.peakBytes, state.liveBytes)
    },
    updateTexture(texture: THREE.DataTexture, options: any) {
      expect(options.image.data).toBe(texture.image.data)
      state.uploadBytes += options.image.data.byteLength
    },
    destroyTexture(texture: THREE.DataTexture) { const bytes = texture.image.data.byteLength; state.liveBytes -= bytes; state.destroyedBytes += bytes },
  }
  const textures = new Textures({}, backend, { createTexture() {}, destroyTexture() {} })
  return { state, upload: (values: readonly (THREE.DataTexture | undefined)[]) => values.forEach(texture => { if (texture) textures.updateTexture(texture) }) }
}

test("actual lightmap owner replacement has no CPU sample copy but duplicates allocation/upload", async () => {
  const loop = ownerLoop(), width = 2048, height = 1053
  const lightmap: RuntimeLightmap = { width, height, profile: "ldr", flat: new Float32Array(width * height * 4), styleScalars: new Map([[0, 1]]) }
  let generation = new OwnedResourceGeneration(1, 1)
  let textures = createWorldLightmapTextures(lightmap, generation)
  generation.activate(); loop.upload(textures)
  for (let index = 2; index <= 4; index++) {
    const next = new OwnedResourceGeneration(1, index), staged = createWorldLightmapTextures(lightmap, next)
    expect(staged[0].image.data).toBe(textures[0].image.data)
    expect(staged[0]).not.toBe(textures[0])
    next.activate(); loop.upload(staged)
    expect(loop.state.liveBytes).toBe(lightmap.flat.byteLength * 2)
    await generation.retire(Promise.resolve())
    generation = next; textures = staged
    expect(loop.state.liveBytes).toBe(lightmap.flat.byteLength)
  }
  expect(loop.state.peakBytes).toBe(lightmap.flat.byteLength * 2)
  expect(loop.state.uploadBytes).toBe(lightmap.flat.byteLength * 4)
  generation.dispose(); generation.dispose()
  expect(loop.state.liveBytes).toBe(0)
  expect(loop.state.createdBytes).toBe(loop.state.destroyedBytes)
  console.log(JSON.stringify({ ownerLoop: loop.state, cpuCopiedSampleBytes: 0 }))
})

test("HDR styles replace every plane without changing format, samples, filtering or mip semantics", () => {
  const map = parseRuntimeMap(hdrFixture().bytes), lightmap = buildRuntimeLightmap(map, [{ style: 0, scalar: 1 }])
  const generation = new OwnedResourceGeneration(1, 1), textures = createWorldLightmapTextures(lightmap, generation), loop = ownerLoop()
  generation.activate(); loop.upload(textures)
  const changed = buildRuntimeLightmap(map, [{ style: 0, scalar: 0.5 }])
  const versions = textures.map(texture => texture!.version)
  replaceWorldLightmapData(textures, changed); loop.upload(textures)
  for (const [index, texture] of textures.entries()) {
    expect(texture!.image.data).toBe([changed.flat, ...changed.directional!][index])
    expect(texture!.version).toBe(versions[index]! + 1)
    expect(texture).toMatchObject({ type: THREE.FloatType, format: THREE.RGBAFormat, colorSpace: THREE.NoColorSpace, channel: 1,
      flipY: false, generateMipmaps: false, minFilter: THREE.LinearFilter, magFilter: THREE.LinearFilter,
      wrapS: THREE.ClampToEdgeWrapping, wrapT: THREE.ClampToEdgeWrapping })
    expect(texture!.mipmaps).toHaveLength(0)
  }
  expect(loop.state.uploadBytes).toBe(changed.flat.byteLength * 8)
  generation.dispose(); expect(loop.state.liveBytes).toBe(0)
})
