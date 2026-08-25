import { describe, expect, test } from "bun:test"
import { parseRuntimeMap } from "../src/runtime-map"

function fixture(): Uint8Array {
  const bytes: number[] = [...new TextEncoder().encode("PSMP")]
  const u32 = (value: number) => bytes.push(...new Uint8Array(new Uint32Array([value]).buffer))
  const i32 = (value: number) => bytes.push(...new Uint8Array(new Int32Array([value]).buffer))
  const f32 = (value: number) => bytes.push(...new Uint8Array(new Float32Array([value]).buffer))
  const sized = (value: Uint8Array) => { u32(value.byteLength); bytes.push(...value) }
  u32(6)
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
  u32(0)
  u32(1)
  u32(2)
  i32(-1)
  bytes.push(0, 255, 255, 255)
  i32(0)
  i32(0)
  sized(new TextEncoder().encode("{}\0"))
  u32(1)
  bytes.push(1, 0, 0, 0)
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

  test("rejects truncation and trailing bytes", () => {
    const bytes = fixture()
    expect(() => parseRuntimeMap(bytes.subarray(0, bytes.length - 1))).toThrow()
    const trailing = new Uint8Array(bytes.length + 1)
    trailing.set(bytes)
    expect(() => parseRuntimeMap(trailing)).toThrow()
  })
})
