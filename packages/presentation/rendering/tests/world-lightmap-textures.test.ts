import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import Textures from "three/src/renderers/common/Textures.js"
import { borrowWorldLightmapTextures, createWorldLightmapTextures, replaceWorldLightmapData } from "../src/world-lightmap-textures"
import { retainedSceneSource } from "../src/scene-resource-handoff"
import { SharedTextureResidency } from "../src/texture-residency"
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

const staticLightmap = (): RuntimeLightmap => ({ width: 8, height: 4, profile: "ldr", flat: new Float32Array(8 * 4 * 4), styleScalars: new Map([[0, 1]]) })
const mapHash = "a".repeat(64), resourceHash = "b".repeat(64)
function source(lightmap = staticLightmap(), generation = new OwnedResourceGeneration(1, 1)) {
  const textures = createWorldLightmapTextures(lightmap, generation)
  generation.activate()
  return { lightmap, textures, disposables: generation, payloadSha256: mapHash, loadRequest: { resourceIdentity: resourceHash }, residency: new SharedTextureResidency<THREE.Texture>(generation) }
}

test("same-map exact private plane handoff removes all repeated native work and retains one owner", async () => {
  let active = source(), loop = ownerLoop()
  const bytes = active.lightmap.flat.byteLength, first = active.textures[0], version = first.version
  loop.upload(active.textures)
  for (let scene = 2; scene <= 16; scene++) {
    const retained = retainedSceneSource(active, 1, mapHash, resourceHash)!
    const next = new OwnedResourceGeneration(1, scene), textures = borrowWorldLightmapTextures(active.lightmap, retained.textures)!
    const residency = new SharedTextureResidency<THREE.Texture>(next, 4, undefined, retained.residency)
    loop.upload(textures) // Three's actual uploaded version gate, not a fake skip.
    expect(textures[0]).toBe(first); expect(textures[0].version).toBe(version)
    expect(loop.state.liveBytes).toBe(bytes)
    expect(next.snapshot().resources).toBe(0)
    next.activate(); residency.commitTransfers([textures[0]])
    await active.disposables.retire(Promise.resolve()); active.residency.clear()
    active = { ...active, textures, disposables: next, residency }
    expect(next.snapshot().resources).toBe(1)
  }
  expect(loop.state).toEqual({ liveBytes: bytes, peakBytes: bytes, createdBytes: bytes, uploadBytes: bytes, destroyedBytes: 0 })
  active.disposables.dispose(); expect(loop.state.liveBytes).toBe(0)
})

test("different map, source closure, device, or terminal generation cannot lend any texture", () => {
  const active = source()
  for (const [device, map, resource] of [[2, mapHash, resourceHash], [1, "c".repeat(64), resourceHash], [1, mapHash, "d".repeat(64)], [1, mapHash, undefined], [1, mapHash, "invalid"]] as const) {
    expect(retainedSceneSource(active, device, map, resource)).toBeUndefined()
  }
  active.disposables.dispose()
  expect(retainedSceneSource(active, 1, mapHash, resourceHash)).toBeUndefined()
})

test("source plane/view/topology or sampler mismatch always allocates independently", () => {
  const active = source(), lightmap = active.lightmap
  expect(borrowWorldLightmapTextures({ ...lightmap, flat: new Float32Array(lightmap.flat.buffer) }, active.textures)).toBeUndefined()
  expect(borrowWorldLightmapTextures({ ...lightmap, flat: lightmap.flat.slice() }, active.textures)).toBeUndefined()
  expect(borrowWorldLightmapTextures({ ...lightmap, width: 4, height: 8 }, active.textures)).toBeUndefined()
  expect(borrowWorldLightmapTextures({ ...lightmap, profile: "hdr" }, active.textures)).toBeUndefined()
  for (const [key, value] of Object.entries({ type: THREE.HalfFloatType, format: THREE.RedFormat, colorSpace: THREE.SRGBColorSpace,
    channel: 0, flipY: true, generateMipmaps: true, anisotropy: 4, premultiplyAlpha: true, unpackAlignment: 4, internalFormat: "RGBA16F", minFilter: THREE.LinearFilter, magFilter: THREE.LinearFilter,
    wrapS: THREE.RepeatWrapping, wrapT: THREE.RepeatWrapping })) {
    const texture = active.textures[0] as any, original = texture[key]; texture[key] = value
    expect(borrowWorldLightmapTextures(lightmap, active.textures)).toBeUndefined(); texture[key] = original
  }
  active.disposables.dispose()
})

test("candidate cancellation/rollback never disposes the borrowed active texture; destruction and loss are terminal", async () => {
  const active = source(), loop = ownerLoop(); loop.upload(active.textures)
  const staged = new OwnedResourceGeneration(1, 2), residency = new SharedTextureResidency<THREE.Texture>(staged, 4, undefined, active.residency)
  const borrowed = borrowWorldLightmapTextures(active.lightmap, active.textures)!
  loop.upload(borrowed); staged.dispose(); residency.clear()
  expect(active.disposables.snapshot().resources).toBe(1); expect(loop.state.destroyedBytes).toBe(0)
  const invalidDevice = new OwnedResourceGeneration(2, 3)
  expect(() => active.disposables.transferTo(invalidDevice, [borrowed[0]])).toThrow("transfer is invalid")
  expect(invalidDevice.snapshot().resources).toBe(0)
  await active.disposables.retire(Promise.reject(new Error("device lost")))
  active.disposables.dispose(); expect(loop.state.destroyedBytes).toBe(active.lightmap.flat.byteLength)
  expect(retainedSceneSource(active, 1, mapHash, resourceHash)).toBeUndefined()
})

test("a live HDR style change cannot mutate an independently staged candidate", () => {
  const map = parseRuntimeMap(hdrFixture().bytes), lightmap = buildRuntimeLightmap(map, [{ style: 0, scalar: 1 }])
  const active = source(lightmap), staged = new OwnedResourceGeneration(1, 2)
  expect(borrowWorldLightmapTextures(lightmap, active.textures)).toBeUndefined()
  const next = createWorldLightmapTextures(lightmap, staged), bytes = new Uint8Array(next[0].image.data.buffer).slice()
  const changed = buildRuntimeLightmap(map, [{ style: 0, scalar: 0.25 }])
  replaceWorldLightmapData(active.textures, changed)
  expect(new Uint8Array(next[0].image.data.buffer)).toEqual(bytes)
  expect(active.textures[0].image.data).not.toEqual(next[0].image.data)
  staged.dispose(); active.disposables.dispose()
})
