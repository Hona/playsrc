import { describe, expect, test } from "bun:test"
import {
  buildRuntimeLightmap,
  parseRuntimeMap,
  validateRuntimeMapHashes,
} from "../src/runtime-map"
import { hdrFixture } from "./hdr-fixture"

describe("schema-7 HDR runtime map", () => {
  test("validates the complete descriptor and preserves raw radiance above one", async () => {
    const map = parseRuntimeMap(hdrFixture().bytes)
    await validateRuntimeMapHashes(map)
    expect(map).toMatchObject({ schema: 7, lightingProfile: 1, lightingSampleCount: 4 })
    expect(map.lighting.profile).toBe("hdr")
    if (map.lighting.profile !== "hdr") throw new Error("expected HDR")
    expect([...map.lighting.samples.slice(0, 3)]).toEqual([20, 1, 0.5])
    expect(map.lighting.descriptor).toMatchObject({
      outputRole: "map-runtime-hdr",
      compilerIdentity: "playsrc-map-runtime-hdr-1",
      lightmappedFaces: 1,
      directionalFaces: 1,
    })
    expect(map.lighting.descriptor.members).toHaveLength(10)
    expect(map.lighting.descriptor.worldLights).toHaveLength(1)
    expect(map.lighting.descriptor.ambientSamples).toHaveLength(1)
    expect(map.lighting.descriptor.profileMaterials).toHaveLength(1)
    expect(map.lighting.descriptor.consumedInputs).toHaveLength(1)
    expect(map.lighting.descriptor.requirements.map((value) => [value.family, value.disposition])).toEqual([
      ["sky", "Unsupported"],
      ["water", "Missing"],
      ["environment", "Missing"],
    ])
  })

  test("retains authored negative world-light attenuation coefficients", () => {
    const map = parseRuntimeMap(hdrFixture([0], -0.00020752029377035797).bytes)
    if (map.lighting.profile !== "hdr") throw new Error("expected HDR")
    expect(map.lighting.descriptor.worldLights[0]!.linearAttenuation).toBe(Math.fround(-0.00020752029377035797))
  })

  test("retains the signed radius computed from linear attenuation", () => {
    const map = parseRuntimeMap(hdrFixture([0], -2, -1).bytes)
    if (map.lighting.profile !== "hdr") throw new Error("expected HDR")
    expect(map.lighting.descriptor.worldLights[0]!.radius).toBe(-1)
    expect(map.lighting.descriptor.worldLights[0]!.linearAttenuation).toBe(-2)
  })

  test("constructs four float planes and applies integer-HDR quantization without RGBA8", () => {
    const map = parseRuntimeMap(hdrFixture().bytes)
    const lightmap = map.lightmap!
    const placement = map.lightmapLayout.placements.get(0)!
    const at = (placement.y * lightmap.width + placement.x) * 4
    expect(lightmap.profile).toBe("hdr")
    expect(lightmap.directional).toHaveLength(3)
    expect(lightmap.flat).toBeInstanceOf(Float32Array)
    expect(lightmap.flat[at]).toBe(65_535 / 4_096)
    expect(lightmap.flat[at + 1]).toBe(1)
    expect(lightmap.flat[at + 2]).toBe(0.5)
    expect(lightmap.directional![0]![at]).toBeCloseTo(10, 8)
    expect(lightmap.directional![1]![at]).toBe(65_535 / 4_096)
    expect(lightmap.directional![2]![at]).toBe(65_535 / 4_096)
    expect(lightmap.flat[(placement.y * lightmap.width + placement.x - 1) * 4]).toBe(lightmap.flat[at])
  })

  test("requires explicit exact style selection for nonzero styles", () => {
    const map = parseRuntimeMap(hdrFixture([0, 1]).bytes)
    expect(map.lightmap).toBeUndefined()
    expect(() => buildRuntimeLightmap(map)).toThrow(/light-style scalars/i)
    const style0 = buildRuntimeLightmap(map, [{ style: 0, scalar: 1 }, { style: 1, scalar: 0 }])
    const style1 = buildRuntimeLightmap(map, [{ style: 0, scalar: 0 }, { style: 1, scalar: 1 }])
    const placement = map.lightmapLayout.placements.get(0)!
    const at = (placement.y * style0.width + placement.x) * 4
    expect(style0.flat[at]).toBe(65_535 / 4_096)
    expect(style0.flat[at + 1]).toBe(1)
    expect(style1.flat[at + 1]).toBe(2)
    expect(() => buildRuntimeLightmap(map, [{ style: 0, scalar: 1 }])).toThrow(/missing/i)
    expect(() => buildRuntimeLightmap(map, [{ style: 0, scalar: 1 }, { style: 0, scalar: 1 }, { style: 1, scalar: 1 }])).toThrow(/invalid/i)
  })

  test("rejects closure, retained-resource hash, reserved-byte, and profile mutations", async () => {
    const closure = hdrFixture()
    closure.bytes[closure.closureOffset] ^= 1
    const closureMap = parseRuntimeMap(closure.bytes)
    await expect(validateRuntimeMapHashes(closureMap)).rejects.toThrow(/closure/i)

    const texture = hdrFixture()
    texture.bytes[texture.profileTextureHashOffset] ^= 1
    const textureMap = parseRuntimeMap(texture.bytes)
    await expect(validateRuntimeMapHashes(textureMap)).rejects.toThrow(/texture SHA-256/i)

    const reserved = hdrFixture()
    reserved.bytes[reserved.profileReservedOffset] = 1
    expect(() => parseRuntimeMap(reserved.bytes)).toThrow(/reserved/i)

    const profile = hdrFixture()
    profile.bytes[16] = 0
    expect(() => parseRuntimeMap(profile.bytes)).toThrow(/profile differs/i)
  })
})
