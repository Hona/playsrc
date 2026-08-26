import { expect, test } from "bun:test"
import { inflateSync } from "node:zlib"
import { encodeVguiRasterPng } from "../src/raster-png"

test("encodes every straight-alpha channel without canvas premultiplication loss", async () => {
  const pixels = new Uint8ClampedArray(256 * 2 * 4)
  for (let index = 0; index < 512; index += 1) pixels.set([index & 255, 255 - (index & 255), 43, index & 255], index * 4)
  const png = await encodeVguiRasterPng(256, 2, pixels)
  expect([...png.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10])
  const data = new DataView(png.buffer)
  let offset = 8
  let decoded: Buffer | undefined
  while (offset < png.length) {
    const length = data.getUint32(offset)
    const name = String.fromCharCode(...png.subarray(offset + 4, offset + 8))
    if (name === "IDAT") decoded = inflateSync(png.subarray(offset + 8, offset + 8 + length))
    offset += length + 12
  }
  expect(decoded).toBeDefined()
  expect(decoded![0]).toBe(0)
  expect([...decoded!.subarray(1, 1025)]).toEqual([...pixels.subarray(0, 1024)])
  expect(decoded![1025]).toBe(0)
  expect([...decoded!.subarray(1026)]).toEqual([...pixels.subarray(1024)])
})

test("rejects malformed retained raster extents", async () => {
  await expect(encodeVguiRasterPng(0, 2, new Uint8ClampedArray())).rejects.toThrow("dimensions")
  await expect(encodeVguiRasterPng(1, 2, new Uint8ClampedArray(4))).rejects.toThrow("dimensions")
})
