import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { modelIntersectsViewFrustum } from "../src/model-visibility"

test("matches authored primitive bounding spheres at frustum edges and preserves uncullable meshes", () => {
  const camera = new THREE.PerspectiveCamera(90, 1, 1, 100)
  camera.updateMatrixWorld()
  const projection = new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse)
  const frustum = new THREE.Frustum().setFromProjectionMatrix(projection)
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute("position", new THREE.Float32BufferAttribute([-1, 0, 0, 1, 0, 0], 3))
  geometry.computeBoundingSphere()
  const mesh = new THREE.Mesh(geometry, new THREE.MeshBasicMaterial())
  const root = new THREE.Group()
  root.add(mesh)
  root.updateMatrixWorld(true)
  const sphere = new THREE.Sphere(), scratch = new THREE.Matrix4()

  expect(modelIntersectsViewFrustum(frustum, root, new THREE.Matrix4().makeTranslation(0, 0, -5), sphere, scratch)).toBe(true)
  expect(modelIntersectsViewFrustum(frustum, root, new THREE.Matrix4().makeTranslation(20, 0, -5), sphere, scratch)).toBe(false)
  expect(modelIntersectsViewFrustum(frustum, root, new THREE.Matrix4().makeTranslation(5.5, 0, -5), sphere, scratch)).toBe(true)
  mesh.frustumCulled = false
  expect(modelIntersectsViewFrustum(frustum, root, new THREE.Matrix4().makeTranslation(20, 0, -5), sphere, scratch)).toBe(true)
})
