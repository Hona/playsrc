import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { applyMapModelTransform, type MapModelTransform } from "./map-model-transform"
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
