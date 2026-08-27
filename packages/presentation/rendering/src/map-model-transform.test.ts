import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { applyMapModelTransform, applyMapModelRenderBounds, type MapModelTransform } from "./map-model-transform"
import { RetainedStaticSceneGroup } from "./static-scene-group"

test("an admitted fixed-matrix occurrence follows authoritative closed/mid/open/close/retrigger transforms without replacing GPU resources", () => {
  const instance = new THREE.Group()
  const geometry = new THREE.BoxGeometry(8, 8, 8), material = new THREE.MeshBasicMaterial()
  const mesh = new THREE.Mesh(geometry, material)
  instance.add(mesh)
  instance.matrix.makeTranslation(128, 260, 784)
  instance.matrixAutoUpdate = false
  const root = new RetainedStaticSceneGroup(); root.add(instance)
  root.updateMatrixWorld(true)
  for (const z of [784, 820, 908, 856, 784, 832, 832, 808, 784]) {
    const state: MapModelTransform = { sourceIndex: 38, worldPosition: [128, 260, z], worldAngles: [0, 0, 0], draw: true }
    applyMapModelTransform(instance, state)
    root.updateMatrixWorld()
    // No evidence helper may force the matrices current after the draw.
    expect(new THREE.Vector3().setFromMatrixPosition(mesh.matrixWorld).toArray()).toEqual([128, 260, z])
    expect(root.children).toHaveLength(1)
    expect(mesh.geometry).toBe(geometry)
    expect(mesh.material).toBe(material)
    expect(mesh.matrixWorld.elements[14]).toBe(z)
  }
  geometry.dispose(); material.dispose()
})

test("authoritative world angles retain authored model scale and visibility changes", () => {
  const instance = new THREE.Group()
  instance.matrix.makeScale(2, 2, 2); instance.matrixAutoUpdate = false
  const state: MapModelTransform = { sourceIndex: 1, worldPosition: [10, 20, 30], worldAngles: [0, 90, 0], draw: false }
  applyMapModelTransform(instance, state); instance.updateMatrixWorld(true)
  const point = new THREE.Vector3(1, 0, 0).applyMatrix4(instance.matrixWorld)
  expect(point.x).toBeCloseTo(10); expect(point.y).toBeCloseTo(22); expect(point.z).toBeCloseTo(30)
  expect(instance.visible).toBe(false)
  applyMapModelTransform(instance, { ...state, draw: true })
  expect(instance.visible).toBe(true)
})

test("animated map occurrences keep frustum admission using authored sequence bounds, not the closed pose", () => {
  const geometry = new THREE.BufferGeometry(), material = new THREE.MeshBasicMaterial()
  const mesh = new THREE.SkinnedMesh(geometry, material)
  mesh.frustumCulled = false
  applyMapModelRenderBounds(mesh, [[-128, -4, -64], [128, 0, 192]])
  const sphere = mesh.boundingSphere!
  expect(mesh.frustumCulled).toBe(true)
  expect(sphere.center.toArray()).toEqual([0, -2, 64])
  for (const x of [-128, 128]) for (const y of [-4, 0]) for (const z of [-64, 192]) {
    expect(sphere.center.distanceTo(new THREE.Vector3(x, y, z))).toBeLessThanOrEqual(sphere.radius + 1e-6)
  }
  applyMapModelRenderBounds(mesh, [[-128, -4, -64], [128, 0, 64]])
  expect(mesh.boundingSphere).toBe(sphere)
  expect(mesh.geometry).toBe(geometry)
  expect(mesh.material).toBe(material)
  expect(() => applyMapModelRenderBounds(mesh, [[0, 0, 0], [-1, 0, 0]])).toThrow("render bounds")
  geometry.dispose(); material.dispose()
})
