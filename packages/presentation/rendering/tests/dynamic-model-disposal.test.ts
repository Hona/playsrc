import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { disposeDynamicModel } from "../src/dynamic-model-disposal"
import { RetainedModelCache } from "../src/retained-model-cache"
import { bindSourceModelMesh, createSourceModelSkeleton, updateSourceModelSkeleton } from "../src/source-model-skinning"

test("parking retains per-actor palette revisions; eviction and generation retirement dispose owned resources once", () => {
  const template = new THREE.BufferGeometry(), templateMaterial = new THREE.MeshBasicNodeMaterial()
  let sharedDisposals = 0
  template.addEventListener("dispose", () => sharedDisposals++)
  templateMaterial.addEventListener("dispose", () => sharedDisposals++)
  const matrix = (offset: number) => Float32Array.from([1, 0, 0, offset, 0, 1, 0, 0, 0, 0, 1, 0])
  const make = () => {
    const instance = new THREE.Group(), skeleton = createSourceModelSkeleton(matrix(0))
    const material = templateMaterial.clone(), flex = template.clone()
    const base = bindSourceModelMesh(flex, material, skeleton), overlay = bindSourceModelMesh(flex, material, skeleton)
    base.userData.dynamicMaterial = overlay.userData.dynamicMaterial = true
    base.userData.dynamicGeometry = overlay.userData.dynamicGeometry = true
    instance.add(base, overlay, new THREE.Mesh(template, templateMaterial))
    const disposed = { skeleton: 0, material: 0, geometry: 0 }
    const original = skeleton.dispose.bind(skeleton)
    skeleton.dispose = () => { disposed.skeleton++; original() }
    material.addEventListener("dispose", () => disposed.material++)
    flex.addEventListener("dispose", () => disposed.geometry++)
    return { instance, skeleton, disposed }
  }
  const scene = new THREE.Group(), a = make(), b = make()
  scene.add(a.instance, b.instance)
  const cache = new RetainedModelCache<ReturnType<typeof make>>(1, value => disposeDynamicModel(value.instance))
  updateSourceModelSkeleton(a.skeleton, matrix(5))
  a.instance.removeFromParent(); cache.retain("world:1:skin0", a)
  expect(a.disposed).toEqual({ skeleton: 0, material: 0, geometry: 0 })
  const restored = cache.take("world:1:skin0")!
  scene.add(restored.instance)
  expect(updateSourceModelSkeleton(restored.skeleton, matrix(9))).toBe(64)
  expect(updateSourceModelSkeleton(restored.skeleton, matrix(9))).toBe(0)
  expect(restored.skeleton.boneMatrices[12]).toBe(9)
  expect(b.skeleton.boneMatrices[12]).toBe(0)
  restored.instance.removeFromParent(); cache.retain("world:1:skin0", restored)
  b.instance.removeFromParent(); cache.retain("world:2:skin0", b)
  expect(a.disposed).toEqual({ skeleton: 1, material: 1, geometry: 1 })
  cache.clear(); cache.clear()
  expect(b.disposed).toEqual({ skeleton: 1, material: 1, geometry: 1 })
  expect(scene.children).toEqual([])
  expect(sharedDisposals).toBe(0)
})
