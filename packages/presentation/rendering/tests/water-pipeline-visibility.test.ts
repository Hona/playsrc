import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { prepareWaterPipelineVisibility } from "../src/water-pipeline-visibility"

describe("targeted authored Water pipeline preparation", () => {
  test("stages only actual Water meshes and restores every prior mesh visibility", () => {
    const root = new THREE.Group()
    const opaque = new THREE.Mesh()
    const hiddenProp = new THREE.Mesh()
    const water = new THREE.Mesh()
    const hiddenWater = new THREE.Mesh()
    hiddenProp.visible = false
    hiddenWater.visible = false
    root.add(opaque, hiddenProp, water, hiddenWater)

    const restore = prepareWaterPipelineVisibility(root, [water, hiddenWater])
    expect([opaque.visible, hiddenProp.visible, water.visible, hiddenWater.visible]).toEqual([false, false, true, true])

    restore()
    expect([opaque.visible, hiddenProp.visible, water.visible, hiddenWater.visible]).toEqual([true, false, true, false])
  })
})
