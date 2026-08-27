import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import { installRenderObjectLifetime } from "../src/render-object-lifetime"

test("retiring old draws removes resource listeners but preserves transferred geometry/material and new draws", () => {
  const deleted: object[] = []
  const renderer = { backend: { isWebGPUBackend: true }, contextNode: { id: 1, version: 0 }, _currentSourceMaterial: null }
  const nodes = { getCacheKey: () => 0, delete(object: object) { deleted.push(object) } }
  const manager = new RenderObjects(renderer, nodes, {}, { delete() {} }, { deleteForRender() {} }, {})
  const original = manager.createRenderObject
  const lifetime = installRenderObjectLifetime(manager)
  const geometry = new THREE.BoxGeometry(), material = new THREE.MeshBasicMaterial()
  const old = new THREE.Group(), current = new THREE.Group(), oldMesh = new THREE.Mesh(geometry, material), currentMesh = new THREE.Mesh(geometry, material)
  old.add(oldMesh); current.add(currentMesh)
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(), lights = {}
  const a = manager.get(oldMesh, material, scene, camera, lights, { id: 1 }, null)
  const b = manager.get(oldMesh, material, scene, camera, lights, { id: 2 }, null)
  const c = manager.get(currentMesh, material, scene, camera, lights, { id: 1 }, null)
  let resourceDisposals = 0
  material.addEventListener("dispose", () => { resourceDisposals++ })
  geometry.addEventListener("dispose", () => { resourceDisposals++ })
  // Detaching a scene alone does not remove these native subscriptions.
  old.removeFromParent()
  expect((material as any)._listeners.dispose).toHaveLength(4)
  lifetime.release(old)
  lifetime.release(old)
  expect(deleted).toEqual([a, b])
  expect((material as any)._listeners.dispose).toHaveLength(2)
  expect((geometry as any)._listeners.dispose).toHaveLength(2)
  expect(resourceDisposals).toBe(0)
  material.dispose(); geometry.dispose()
  expect(deleted).toEqual([a, b, c])
  expect(resourceDisposals).toBe(2)
  lifetime.restore()
  expect(manager.createRenderObject).toBe(original)
})

test("a flex geometry replacement moves the draw subscription off its immutable bind geometry", () => {
  const manager = new RenderObjects({ backend: {}, contextNode: { id: 1, version: 0 }, _currentSourceMaterial: null },
    { getCacheKey: () => 0, delete() {} }, {}, { delete() {} }, { deleteForRender() {} }, {})
  const lifetime = installRenderObjectLifetime(manager)
  const bind = new THREE.BoxGeometry(), flex = bind.clone(), material = new THREE.MeshBasicMaterial()
  const mesh = new THREE.Mesh(bind, material), root = new THREE.Group()
  root.add(mesh)
  const draw = manager.get(mesh, material, new THREE.Scene(), new THREE.PerspectiveCamera(), {}, { id: 1 }, null)
  expect((bind as any)._listeners.dispose).toHaveLength(1)
  mesh.geometry = flex
  draw.setGeometry(flex)
  expect((bind as any)._listeners.dispose).toHaveLength(0)
  expect((flex as any)._listeners.dispose).toHaveLength(1)
  lifetime.release(root)
  expect((bind as any)._listeners.dispose).toHaveLength(0)
  expect((flex as any)._listeners.dispose).toHaveLength(0)
  lifetime.restore()
})
