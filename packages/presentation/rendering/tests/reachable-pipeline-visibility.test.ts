import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { prepareReachablePipelineVisibility } from "../src/reachable-pipeline-visibility"

describe("reachable Source map pipeline preparation", () => {
  test("exposes hidden bundle meshes, deduplicates equivalent shaders, and restores authored visibility", () => {
    const root = new THREE.Group()
    const bundle = new THREE.BundleGroup()
    bundle.visible = false
    const geometry = new THREE.BoxGeometry()
    const material = new THREE.MeshBasicNodeMaterial()
    const first = new THREE.Mesh(geometry, material)
    first.visible = false
    const duplicate = new THREE.Mesh(geometry, material)
    const transparent = new THREE.Mesh(geometry, new THREE.MeshBasicNodeMaterial({ transparent: true }))
    const water = new THREE.Mesh(geometry, new THREE.MeshBasicNodeMaterial())
    bundle.add(first, duplicate, transparent, water)
    root.add(bundle)

    const staged = prepareReachablePipelineVisibility(root, [water])
    expect(staged.variants).toBe(2)
    expect(bundle.isBundleGroup).toBe(false)
    expect(bundle.visible).toBe(true)
    expect([first.visible, duplicate.visible, transparent.visible, water.visible]).toEqual([true, false, true, false])
    expect(first.frustumCulled).toBe(false)

    staged.restore()
    expect(bundle.isBundleGroup).toBe(true)
    expect(bundle.visible).toBe(false)
    expect([first.visible, duplicate.visible, transparent.visible, water.visible]).toEqual([false, true, true, true])
    expect(first.frustumCulled).toBe(true)
  })

  test("does not upload unrelated retained static-prop hierarchies during world warmup", () => {
    const root = new THREE.Group()
    const retained = new THREE.Group()
    const prop = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicNodeMaterial())
    retained.add(prop)
    root.add(retained)
    const staged = prepareReachablePipelineVisibility(root, [], [retained])
    expect(staged.variants).toBe(0)
    expect(retained.visible).toBe(false)
    expect(prop.visible).toBe(false)
    staged.restore()
    expect(retained.visible).toBe(true)
    expect(prop.visible).toBe(true)
  })
})
