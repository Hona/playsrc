import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { ModelLightingGraphs, bindModelLighting, bindModelTexture, bindStaticPropFade, modelEnvironmentShape, perObjectModelTextures, transferModelBindings } from "../src/model-lighting-graphs"
import { createSourceModelLightingUniforms, createSourceModelEyeUniforms, sourceModelSurfaceNode } from "../src/source-model-lighting"

test("one graph reads independent lighting and eye values in alternating object order", () => {
  const graphs = new ModelLightingGraphs()
  const actors = [new THREE.Mesh(), new THREE.Mesh()]
  actors.forEach((actor, index) => {
    const lighting = createSourceModelLightingUniforms(), eyes = createSourceModelEyeUniforms()
    lighting.ambientEnabled.value = index
    lighting.ambient[2].value.set(index + .125, index + .25, index + .5)
    lighting.local[3].position.value.set(index * 17, -index, 12)
    eyes.irisU.value.set(index, 2, 3, 4)
    bindModelLighting(actor, lighting, eyes)
  })
  const read = (node: any, object: THREE.Mesh) => {
    if (node.isUniformNode) {
      const frame = new NodeFrame(); frame.object = object
      frame.updateNode(node._beforeNodes[0])
      return node.value
    }
    node.updateReference({ object }); node.updateValue()
    return node.node.value
  }
  for (const index of [0, 1, 0, 1]) {
    expect(read(graphs.lighting.ambientEnabled, actors[index]!)).toBe(index)
    expect(read(graphs.lighting.ambient[2], actors[index]!).toArray()).toEqual([index + .125, index + .25, index + .5])
    expect(read(graphs.lighting.local[3].position, actors[index]!).toArray()).toEqual([index * 17, -index, 12])
    expect(read(graphs.eyes.irisU, actors[index]!).toArray()).toEqual([index, 2, 3, 4])
    expect(Object.keys(actors[index]!.userData)).toEqual([])
    expect(actors[index]!.clone().userData).toEqual({})
  }
})

test("a verified graph handoff drops last-draw object references and rebinds without changing graph identity", () => {
  const graphs = new ModelLightingGraphs(), old = new THREE.Mesh(), next = new THREE.Mesh()
  const before = createSourceModelLightingUniforms(), after = createSourceModelLightingUniforms()
  before.ambientEnabled.value = 0; after.ambientEnabled.value = 1
  bindModelLighting(old, before); bindModelLighting(next, after)
  bindStaticPropFade(old, TSL.uniform(.25)); bindStaticPropFade(next, TSL.uniform(.75))
  const fade = graphs.staticFade as any
  fade.updateReference({ object: old }); fade.updateValue()
  expect(fade.node.value).toBe(.25)
  const node = graphs.lighting.ambientEnabled as any
  const frame = new NodeFrame(); frame.object = old
  frame.updateNode(node._beforeNodes[0])
  const graph = graphs.get("exact", () => TSL.vec4(1))
  expect(graphs.lighting.cameraPosition.value).toBe(before.cameraPosition.value)
  graphs.releaseDrawReferences()
  expect(graphs.lighting.cameraPosition.value).toBeNull()
  expect(node.value).toBeNull()
  expect(fade.reference).toBeNull()
  expect(graphs.get("exact", () => { throw new Error("rebuilt graph") })).toBe(graph)
  frame.object = next; frame.updateNode(node._beforeNodes[0])
  expect(node.value).toBe(1)
  fade.updateReference({ object: next }); fade.updateValue()
  expect(fade.node.value).toBe(.75)
})

test("graph structure is retained across actor replacement, but isolated across scene/fog/exposure owners", () => {
  const world = new ModelLightingGraphs(), panel = new ModelLightingGraphs(), nextGeneration = new ModelLightingGraphs()
  let created = 0
  const make = (owner: ModelLightingGraphs) => () => {
    created++
    return sourceModelSurfaceNode(TSL.vec4(1), owner.lighting, { halfLambert: true }, TSL.float(1)).color
  }
  const a = world.get("skin:primitive:none", make(world))
  expect(world.get("skin:primitive:none", make(world))).toBe(a)
  expect(world.get("skin:primitive:environment", make(world))).not.toBe(a)
  expect(panel.get("skin:primitive:none", make(panel))).not.toBe(a)
  expect(nextGeneration.get("skin:primitive:none", make(nextGeneration))).not.toBe(a)
  expect(nextGeneration.lighting).not.toBe(world.lighting)
  expect(created).toBe(4)
  expect(world.size).toBe(2)
  const first = new THREE.MeshBasicNodeMaterial(), second = first.clone()
  first.colorNode = a; second.colorNode = a
  expect(first.customProgramCacheKey()).toBe(second.customProgramCacheKey())
})

test("posing an already lit occurrence transfers bindings only to its replacement mesh", () => {
  const original = new THREE.Mesh(), skinned = new THREE.SkinnedMesh()
  const lighting = createSourceModelLightingUniforms(), eye = createSourceModelEyeUniforms(), environment = new THREE.CubeTexture()
  bindModelLighting(original, lighting, eye); bindModelTexture(original, "sourceEnvironment", environment)
  skinned.userData = { ...original.userData }
  transferModelBindings(original, skinned)
  expect(skinned.userData.sourceLighting).toBe(lighting)
  expect(skinned.userData.sourceEye).toBe(eye)
  expect(skinned.userData.sourceEnvironment).toBe(environment)
  expect(Object.keys(skinned.userData)).toEqual([])
  expect(original.clone().userData).toEqual({})
})

test("local cubemap changes are per-object bindings, not new graph identities", () => {
  const first = new THREE.CubeTexture(), second = new THREE.CubeTexture()
  expect(modelEnvironmentShape(first)).toBe(modelEnvironmentShape(second))
  const node = TSL.cubeTexture(first)
  expect(perObjectModelTextures(node, [{ name: "sourceEnvironment", node }])).not.toBe(node)
  const a = new THREE.Mesh(), b = new THREE.Mesh()
  bindModelTexture(a, "sourceEnvironment", first); bindModelTexture(b, "sourceEnvironment", second)
  for (const [object, texture] of [[a, first], [b, second], [a, first]] as const) {
    expect(object.userData.sourceEnvironment).toBe(texture)
  }
  bindModelTexture(a, "sourceEnvironment", second)
  expect(a.userData.sourceEnvironment).toBe(second)
  expect(a.clone().userData).toEqual({})
  second.colorSpace = THREE.SRGBColorSpace
  expect(modelEnvironmentShape(first)).not.toBe(modelEnvironmentShape(second))
  expect(modelEnvironmentShape(undefined)).toBe("none")
})
