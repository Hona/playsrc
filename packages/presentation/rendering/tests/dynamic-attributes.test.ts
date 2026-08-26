import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { synchronizeDynamicAttribute } from "../src/dynamic-attributes"

test("retains stable model attributes and uploads only the changed authored span", () => {
  const attribute = new THREE.BufferAttribute(new Float32Array([1, 2, 3, 4, 5, 6]), 3)
  const version = attribute.version
  expect(synchronizeDynamicAttribute(attribute, new Float32Array([1, 2, 8, 9, 5, 6])))
    .toEqual({ changed: true, offset: 2, count: 2, bytes: 8 })
  expect([...attribute.array]).toEqual([1, 2, 8, 9, 5, 6])
  expect(attribute.updateRanges).toEqual([{ start: 2, count: 2 }])
  expect(attribute.version).toBe(version + 1)

  expect(synchronizeDynamicAttribute(attribute, new Float32Array([1, 2, 8, 9, 5, 6])))
    .toEqual({ changed: false, offset: 0, count: 0, bytes: 0 })
  expect(attribute.version).toBe(version + 1)
})

test("compares exact binary32 identities, including signed zero and NaN payloads", () => {
  const existing = new Float32Array(3)
  new Uint32Array(existing.buffer).set([0x00000000, 0x7fc00001, 0x3f800000])
  const source = new Float32Array(3)
  new Uint32Array(source.buffer).set([0x80000000, 0x7fc00002, 0x3f800000])
  const attribute = new THREE.BufferAttribute(existing, 3)
  expect(synchronizeDynamicAttribute(attribute, source)).toMatchObject({ changed: true, offset: 0, count: 2 })
  expect([...new Uint32Array(existing.buffer)]).toEqual([0x80000000, 0x7fc00002, 0x3f800000])
  expect(synchronizeDynamicAttribute(attribute, source).changed).toBe(false)
})

test("rejects incompatible dynamic attribute layouts without mutating their GPU state", () => {
  const attribute = new THREE.BufferAttribute(new Float32Array([1, 2, 3]), 3)
  expect(() => synchronizeDynamicAttribute(attribute, new Float32Array([1, 2]))).toThrow("authored pose")
  expect(attribute.version).toBe(0)
})
