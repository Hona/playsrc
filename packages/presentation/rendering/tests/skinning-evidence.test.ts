import { expect, test } from "bun:test"
import { unpackGpuRgbaRows } from "../src/skinning-evidence"

test("keeps aligned GPU pixels and strips only exact WebGPU row padding at native window widths", () => {
  const aligned = new Uint8Array(64 * 4 * 2)
  expect(unpackGpuRgbaRows(aligned, 64, 2)).toBe(aligned)
  const bytes = new Uint8Array(512 + 260).fill(99)
  bytes.fill(1, 0, 260)
  bytes.fill(2, 512)
  expect([...unpackGpuRgbaRows(bytes, 65, 2)]).toEqual([...new Array(260).fill(1), ...new Array(260).fill(2)])
  const floats = new Float32Array(128 + 68).fill(Number.NaN)
  floats.fill(0.25, 0, 68)
  floats.fill(0.75, 128)
  expect([...unpackGpuRgbaRows(floats, 17, 2)]).toEqual([...new Array(68).fill(0.25), ...new Array(68).fill(0.75)])
  const halves = new Uint16Array(128 + 68).fill(65535)
  halves.fill(1, 0, 68); halves.fill(2, 128)
  expect(unpackGpuRgbaRows(halves, 17, 2)).toBeInstanceOf(Uint16Array)
  expect([...unpackGpuRgbaRows(halves, 17, 2)]).toEqual([...new Array(68).fill(1), ...new Array(68).fill(2)])
  expect(() => unpackGpuRgbaRows(new Uint8Array(10), 65, 2)).toThrow("row layout differs")
  expect(() => unpackGpuRgbaRows(bytes, 0, 2)).toThrow("dimensions")
})
