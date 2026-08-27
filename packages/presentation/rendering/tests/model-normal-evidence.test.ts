import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { modelNormalEvidence, visibleModelIntersection } from "../src/model-normal-evidence"
import { bindSourceModelMesh, createSourceModelSkeleton } from "../src/source-model-skinning"

test("normal evidence uses the posed triangle and authored normals, not Raycaster face forwarding", () => {
  const geometry = new THREE.BoxGeometry()
  const normals = geometry.getAttribute("normal")
  for (let i = 0; i < normals.count; i++) normals.setXYZ(i, 1, 0, 0)
  geometry.setAttribute("skinIndex", new THREE.BufferAttribute(new Uint16Array(normals.count * 4), 4))
  geometry.setAttribute("skinWeight", new THREE.BufferAttribute(Float32Array.from({ length: normals.count * 4 }, (_, i) => Number(i % 4 === 0)), 4))
  const skeleton = createSourceModelSkeleton(Float32Array.from([0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0]))
  const material = new THREE.MeshBasicNodeMaterial()
  const mesh = bindSourceModelMesh(geometry, material, skeleton)
  mesh.updateMatrixWorld(true)
  const hit = new THREE.Raycaster(new THREE.Vector3(0, 0, 10), new THREE.Vector3(0, 0, -1)).intersectObject(mesh)[0]!
  expect(modelNormalEvidence(hit).worldNormal).toEqual([0, 0, 1])
  expect(modelNormalEvidence(hit).worldPosition[2]).toBeCloseTo(0.5)
  expect(visibleModelIntersection(hit)).toBe(true)
  const parent = new THREE.Group()
  parent.add(mesh)
  parent.visible = false
  expect(visibleModelIntersection(hit)).toBe(false)
  skeleton.dispose(); geometry.dispose(); material.dispose()
})

test("normal evidence preserves reflected and nonuniform object inverse transpose", () => {
  const geometry = new THREE.BoxGeometry()
  const normals = geometry.getAttribute("normal")
  for (let i = 0; i < normals.count; i++) normals.setXYZ(i, 1, 1, 1)
  const material = new THREE.MeshBasicNodeMaterial()
  const mesh = new THREE.Mesh(geometry, material)
  mesh.scale.set(2, -1, 0.5)
  mesh.updateMatrixWorld(true)
  const hit = new THREE.Raycaster(new THREE.Vector3(0, 0, 10), new THREE.Vector3(0, 0, -1)).intersectObject(mesh)[0]!
  const normal = modelNormalEvidence(hit).worldNormal
  const expected = [0.5, -1, 2].map(value => value / Math.hypot(0.5, 1, 2))
  for (let i = 0; i < 3; i++) expect(normal[i]).toBeCloseTo(expected[i]!, 12)
  geometry.dispose(); material.dispose()
})
