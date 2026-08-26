import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { createStaticPropBatch, MAX_STATIC_PROPS_PER_BATCH } from "../src/static-prop-batches"

function occurrence(source: number, x: number, lighting: readonly number[]) {
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute("position", new THREE.Float32BufferAttribute([0, 0, 0, 1, 0, 0, 0, 1, 0], 3))
  geometry.setAttribute("normal", new THREE.Float32BufferAttribute([0, 0, 1, 0, 0, 1, 0, 0, 1], 3))
  geometry.setAttribute("uv", new THREE.Float32BufferAttribute([0, 0, 1, 0, 0, 1], 2))
  geometry.setAttribute("staticLighting", new THREE.Uint8BufferAttribute(lighting, 4, true))
  geometry.setIndex([0, 1, 2])
  return { source, geometry, matrix: new THREE.Matrix4().makeTranslation(x, 0, 0) }
}

test("static-prop batches preserve authored transforms, distinct VHV colors and independent PVS visibility", () => {
  const first = occurrence(17, 4, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
  const second = occurrence(23, 9, [21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32])
  const batch = createStaticPropBatch([first, second], new THREE.MeshBasicMaterial())
  expect(batch.mesh.geometry.getIndex()!.usage).toBe(THREE.StaticDrawUsage)
  expect(batch.mesh.geometry.getAttribute("position").array).toEqual(new Float32Array([4, 0, 0, 5, 0, 0, 4, 1, 0, 9, 0, 0, 10, 0, 0, 9, 1, 0]))
  expect(batch.mesh.geometry.getAttribute("staticLighting").array).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]))
  expect(batch.mesh.visible).toBe(false)

  expect(batch.setVisible(1, true)).toBe(true)
  batch.update()
  expect(batch.mesh.geometry.getIndex()!.array.slice(0, 3)).toEqual(new Uint32Array([3, 4, 5]))
  expect(batch.sourceAtFace(0)).toBe(23)

  batch.setVisible(0, true)
  batch.update()
  expect(batch.mesh.geometry.getIndex()!.array.slice(0, 6)).toEqual(new Uint32Array([0, 1, 2, 3, 4, 5]))
  expect([batch.sourceAtFace(0), batch.sourceAtFace(1)]).toEqual([17, 23])

  batch.setVisible(1, false)
  batch.update()
  expect(batch.mesh.geometry.drawRange.count).toBe(3)
  expect(batch.sourceAtFace(0)).toBe(17)
})

test("static-prop batches enforce Source's 512-occurrence submission bound", () => {
  const input = occurrence(1, 0, Array(12).fill(0))
  expect(() => createStaticPropBatch([input], new THREE.MeshBasicMaterial())).toThrow("occurrence count")
  expect(() => createStaticPropBatch(Array(MAX_STATIC_PROPS_PER_BATCH + 1).fill(input), new THREE.MeshBasicMaterial())).toThrow("occurrence count")
})
