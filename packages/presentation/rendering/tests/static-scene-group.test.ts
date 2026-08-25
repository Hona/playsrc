import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { RetainedStaticSceneGroup } from "../src/static-scene-group"

test("retained static occurrence transforms update once and still honor explicit parent invalidation", () => {
  const root = new THREE.Group()
  root.matrixAutoUpdate = false
  const group = new RetainedStaticSceneGroup()
  const child = new THREE.Object3D()
  child.position.set(1, 2, 3)
  let updates = 0
  const update = child.updateMatrixWorld.bind(child)
  child.updateMatrixWorld = (force?: boolean) => {
    updates += 1
    update(force)
  }
  group.add(child)
  root.add(group)

  root.updateMatrixWorld(true)
  expect(updates).toBe(1)
  expect(child.getWorldPosition(new THREE.Vector3()).toArray()).toEqual([1, 2, 3])

  root.updateMatrixWorld(false)
  root.updateMatrixWorld(false)
  expect(updates).toBe(1)

  root.updateMatrixWorld(true)
  expect(updates).toBe(2)
})
