import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { createSourceModelLightingUniforms, createSourceModelEyeUniforms, sourceModelLightingReferences, sourceModelEyeReferences } from "../src/source-model-lighting"

test("shared shader references select each drawn occurrence's exact lighting and eye values", () => {
  const actors = [new THREE.Mesh(), new THREE.Mesh()]
  actors.forEach((actor, index) => {
    const lighting = createSourceModelLightingUniforms(), eye = createSourceModelEyeUniforms()
    lighting.ambientEnabled.value = index
    lighting.ambient[2].value.set(index + .125, index + .25, index + .5)
    lighting.local[3].position.value.set(index * 17, -index, 12)
    eye.irisU.value.set(index, 2, 3, 4)
    Object.defineProperty(actor.userData, "sourceLighting", { value: lighting })
    Object.defineProperty(actor.userData, "sourceEye", { value: eye })
  })
  const read = (reference: any, actor: THREE.Mesh) => {
    reference.updateReference({ object: actor })
    reference.updateValue()
    return reference.node.value
  }
  for (const index of [0, 1, 0, 1]) {
    const actor = actors[index]!
    expect(read(sourceModelLightingReferences.ambientEnabled, actor)).toBe(index)
    expect(read(sourceModelLightingReferences.ambient[2], actor).toArray()).toEqual([index + .125, index + .25, index + .5])
    expect(read(sourceModelLightingReferences.local[3].position, actor).toArray()).toEqual([index * 17, -index, 12])
    expect(read(sourceModelEyeReferences.irisU, actor).toArray()).toEqual([index, 2, 3, 4])
    expect(Object.keys(actor.userData)).toEqual([])
  }
  const node = sourceModelLightingReferences.ambientEnabled
  const a = new THREE.MeshBasicNodeMaterial(), b = a.clone()
  a.colorNode = node; b.colorNode = node
  expect(a.customProgramCacheKey()).toBe(b.customProgramCacheKey())
})
