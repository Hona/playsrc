import { describe, expect, test } from "bun:test"
import { packRuntimeLightmapLayout, parseRuntimeMap, sourceLdrLightmapIrradiance } from "../src/runtime-map"

function fixture(extendedLdr = false): Uint8Array {
  const bytes: number[] = [...new TextEncoder().encode("PSMP")]
  const u32 = (value: number) => bytes.push(...new Uint8Array(new Uint32Array([value]).buffer))
  const i32 = (value: number) => bytes.push(...new Uint8Array(new Int32Array([value]).buffer))
  const f32 = (value: number) => bytes.push(...new Uint8Array(new Float32Array([value]).buffer))
  const sized = (value: Uint8Array) => { u32(value.byteLength); bytes.push(...value) }
  u32(extendedLdr ? 9 : 6)
  u32(20)
  u32(731)
  bytes.push(0)
  u32(1)
  u32(1)
  u32(0)
  u32(1)
  sized(new TextEncoder().encode("materials/test.vmt"))
  i32(64)
  i32(64)
  u32(7)
  u32(0)
  u32(0)
  i32(0)
  bytes.push(1)
  u32(3)
  u32(1)
  for (const value of [0, 0, 0, 1, 0, 0, 0, 1, 0]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1, 0, 0, 1]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1]) f32(value)
  if (extendedLdr) for (const value of [0, 127, 255]) f32(value)
  u32(0)
  u32(1)
  u32(2)
  i32(-1)
  bytes.push(0, 255, 255, 255)
  i32(0)
  i32(0)
  if (extendedLdr) bytes.push(0, 0, 0, 0)
  sized(new TextEncoder().encode("{}\0"))
  u32(1)
  bytes.push(1, 0, 0, 0)
  if (extendedLdr) bytes.push(0, 0, 0, 0, 0, 0, 0, 0)
  u32(0)
  u32(0)
  return new Uint8Array(bytes)
}

describe("runtime map rendering input", () => {
  test("decodes direct world batches without GLB or per-face calls", () => {
    const map = parseRuntimeMap(fixture())
    expect(map).toMatchObject({
      bspVersion: 20,
      mapRevision: 731,
      lightingProfile: 0,
      drawableSurfaces: 1,
      entityCount: 1,
    })
    expect(map.materials[0]?.logicalPath).toBe("materials/test.vmt")
    expect([...map.batches[0]!.positions]).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0])
    expect([...map.batches[0]!.indices]).toEqual([0, 1, 2])
    expect([...map.batches[0]!.faces]).toEqual([7])
  })

  test("admits the authored UnlitTwoTexture model shader without weakening unknown-shader rejection", () => {
    const bytes = fixture()
    bytes[bytes.length - 12] = 10
    expect(parseRuntimeMap(bytes).materials[0]?.shader).toBe(10)
    bytes[bytes.length - 12] = 11
    expect(parseRuntimeMap(bytes).materials[0]?.shader).toBe(11)
    bytes[bytes.length - 12] = 12
    expect(parseRuntimeMap(bytes).materials[0]?.shader).toBe(12)
    bytes[bytes.length - 12] = 13
    expect(() => parseRuntimeMap(bytes)).toThrow("runtime material payload is invalid")
  })

  test("retains displacement alpha and extended material records in Source LDR maps", () => {
    const map = parseRuntimeMap(fixture(true))
    expect(map.schema).toBe(9)
    expect(map.lighting.profile).toBe("ldr")
    expect([...map.batches[0]!.displacementAlpha]).toEqual([0, 127, 255])
    expect(map.lightmapLayout.gutter).toBe(1)
  })

  test("rejects truncation and trailing bytes", () => {
    const bytes = fixture()
    expect(() => parseRuntimeMap(bytes.subarray(0, bytes.length - 1))).toThrow()
    const trailing = new Uint8Array(bytes.length + 1)
    trailing.set(bytes)
    expect(() => parseRuntimeMap(trailing)).toThrow()
  })

  test("round-trips authored LDR lightmaps through Source vertex gamma, sRGB, and overbright", () => {
    expect(sourceLdrLightmapIrradiance([0, 0, 0])).toEqual([0, 0, 0])
    expect(sourceLdrLightmapIrradiance([1, 1, 1])[0]).toBeCloseTo(0.9918344056, 7)
    expect(sourceLdrLightmapIrradiance([4, 0.25, 0])[0]).toBeGreaterThan(3.8)
    expect(() => sourceLdrLightmapIrradiance([-1, 0, 0])).toThrow(/radiance/i)
  })

  test("packs exact authored lightmap rectangles into the smallest deterministic bounded atlas", () => {
    const surfaces = Object.freeze([
      Object.freeze({ face: 8, width: 4, height: 2 }),
      Object.freeze({ face: 3, width: 2, height: 8 }),
      Object.freeze({ face: 6, width: 4, height: 2 }),
      Object.freeze({ face: 1, width: 2, height: 8 }),
    ])
    const forward = packRuntimeLightmapLayout(surfaces, 1)
    const reversed = packRuntimeLightmapLayout([...surfaces].reverse(), 1)
    expect(forward.width).toBeLessThan(4096)
    expect(forward.width * forward.height).toBeLessThan(4096)
    expect([...forward.placements]).toEqual([...reversed.placements])
    for (const placement of forward.placements.values()) {
      expect(placement.x).toBeGreaterThanOrEqual(1)
      expect(placement.y).toBeGreaterThanOrEqual(1)
      expect(placement.x + placement.width).toBeLessThan(forward.width)
      expect(placement.y + placement.height).toBeLessThan(forward.height)
    }
    expect(() => packRuntimeLightmapLayout([...surfaces, surfaces[0]!], 1)).toThrow("duplicate lightmap")
  })
})
