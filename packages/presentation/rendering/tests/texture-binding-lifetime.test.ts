import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { installTextureBindingLifetime } from "../src/texture-binding-lifetime"

test("a retired bind group leaves every texture it visited without changing live invalidation order", () => {
  const data = new WeakMap<THREE.Texture, { bindGroups: Set<object> }>()
  const invalidated: object[] = [], retired: object[] = []
  const textures = {
    get(texture: THREE.Texture) { return data.get(texture)! },
    updateTexture(texture: THREE.Texture) {
      if (data.has(texture)) return
      const entry = { bindGroups: new Set<object>() }; data.set(texture, entry)
      const dispose = () => {
        invalidated.push(...entry.bindGroups)
        data.delete(texture); texture.removeEventListener("dispose", dispose)
      }
      texture.addEventListener("dispose", dispose)
    },
  }
  const backend = { deleteBindGroupData(group: object) { retired.push(group) } }
  const update = textures.updateTexture, remove = backend.deleteBindGroupData
  const restore = installTextureBindingLifetime(textures as any, backend)
  const a = new THREE.Texture(), b = new THREE.Texture(), old = {}, live = {}
  textures.updateTexture(a); textures.updateTexture(b)
  textures.get(a).bindGroups.add(old); textures.get(a).bindGroups.add(old)
  textures.get(b).bindGroups.add(old); textures.get(b).bindGroups.add(live)
  backend.deleteBindGroupData(old)
  expect(textures.get(a).bindGroups.size).toBe(0)
  expect([...textures.get(b).bindGroups]).toEqual([live])
  b.dispose()
  expect(invalidated).toEqual([live])
  backend.deleteBindGroupData(live)
  expect(retired).toEqual([old, live])
  textures.updateTexture(b)
  expect(textures.get(b).bindGroups.size).toBe(0)
  restore()
  expect(textures.updateTexture).toBe(update)
  expect(backend.deleteBindGroupData).toBe(remove)
})
