import { expect, test } from "bun:test"
import { THREE, TSL, createCompilerParityOwner } from "./fixtures/model-compiler-parity"
import { ModelLightingGraphs, bindModelLighting } from "../src/model-lighting-graphs"
import { modelMaterialGraph, modelMaterialGraphKey, type ModelMaterialGraphInput } from "../src/model-material-graphs"
import { createSourceModelLightingUniforms } from "../src/source-model-lighting"

const phong = { maskSource: 0, invertMask: false, albedoTint: true, exponent: 12, exponentFactor: 0,
  tint: [1, .5, .25] as const, boost: .3, packedFresnel: [1, 2, 3] as const, rim: { exponent: 4, boost: 2, exponentTextureAlphaMask: false } }
const fixture = (): ModelMaterialGraphInput => {
  const texture = new THREE.DataTexture(null, 32, 64)
  return { shader: "vertex-lit-generic", state: { halfLambert: false, phong }, base: TSL.texture(texture, TSL.uv()),
    baseTexture: { texture, sourceFormat: 0 }, exposure: TSL.float(1) }
}
const lighting = { ambientLight: true, cameraPosition: [1, 2, 3], lightingOrigin: [0, 0, 0], localEnvironment: null,
  ambientCube: Array.from({ length: 6 }, (_, i) => [i / 6, .3, .1]), localLights: [], staticLightVertex: false, staticLightTexel: false }

test("changed authored numbers retain programs but each selection binds the new immutable state", () => {
  const input = fixture(), otherTexture = input.baseTexture!.texture.clone()
  const changed = { ...input, state: { ...input.state, phong: { ...phong, exponent: 50, boost: 2, tint: [.1, .2, .3] as const,
    packedFresnel: [0, .5, 1] as const, rim: { ...phong.rim, exponent: 12 } } }, baseTexture: { texture: otherTexture, sourceFormat: 0 }, base: TSL.texture(otherTexture, TSL.uv()) }
  expect(modelMaterialGraphKey(changed)).toBe(modelMaterialGraphKey(input))
  const owner = createCompilerParityOwner(), geometry = new THREE.BoxGeometry()
  owner.admit("original", geometry, "panel", input, { lighting }, false)
  owner.admit("changed", geometry, "panel", changed, { lighting: { ...lighting, ambientLight: false, cameraPosition: [3, 2, 1] } }, false)
  owner.admit("restored", geometry, "panel", input, { lighting }, false)
  const result = owner.verifyLifetime()
  expect(result.programs).toBe(1)
  expect(result.retainedCompilerStates).toBe(0)
  expect(result.repeatedBuilds).toBe(0)
  geometry.dispose(); input.baseTexture!.texture.dispose(); otherTexture.dispose()
})

test("each shader feature, sampler interpretation and fragment contract invalidates the material graph", () => {
  const input = fixture(), key = modelMaterialGraphKey(input)
  for (const state of [
    { ...phong, maskSource: 1 }, { ...phong, invertMask: true }, { ...phong, albedoTint: false },
    { ...phong, exponent: -1 }, { ...phong, exponentFactor: 100 }, { ...phong, rim: null },
    { ...phong, rim: { ...phong.rim, exponentTextureAlphaMask: true } },
  ]) expect(modelMaterialGraphKey({ ...input, state: { ...input.state, phong: state } })).not.toBe(key)
  expect(modelMaterialGraphKey({ ...input, state: { halfLambert: true } })).not.toBe(modelMaterialGraphKey({ ...input, state: { halfLambert: false } }))
  expect(modelMaterialGraphKey({ ...input, shader: "eyes", state: { ...input.state, dilation: .5 } })).not.toBe(key)
  expect(modelMaterialGraphKey({ ...input, textures: { warp: new THREE.Texture() } })).not.toBe(key)
  expect(modelMaterialGraphKey({ ...input, baseTexture: { ...input.baseTexture!, sourceFormat: 12 } })).not.toBe(key)
  expect(modelMaterialGraphKey({ ...input, fragment: { alphaModulation: .5, blendEnabled: true, alphaOwnership: { opacity: true } } as any })).not.toBe(key)
})

test("scene/device graph owners and failed construction cannot share or publish a partial graph", () => {
  const input = fixture(), first = new ModelLightingGraphs(), second = new ModelLightingGraphs(), mesh = new THREE.Mesh()
  bindModelLighting(mesh, createSourceModelLightingUniforms())
  const graph = modelMaterialGraph(mesh, first, input)
  expect(modelMaterialGraph(mesh, first, input)).toBe(graph)
  expect(modelMaterialGraph(mesh, second, input)).not.toBe(graph)
  const failed = new ModelLightingGraphs()
  expect(() => modelMaterialGraph(mesh, failed, input, () => { throw new Error("cancelled admission") })).toThrow("cancelled admission")
  expect(failed.size).toBe(0)
  expect(modelMaterialGraph(mesh, failed, input)).not.toBe(graph)
  expect(failed.size).toBe(1)
  for (const owner of [first, second, failed]) { owner.releaseDrawReferences(); owner.releaseDrawReferences(); expect(owner.phong.tint.value).toBeNull() }
})

test("sampler role aliasing is part of the actual compiler bind layout", () => {
  const input = fixture(), first = new THREE.DataTexture(null, 32, 32), second = first.clone()
  const state = { ...input.state, phong: { ...phong, exponentFactor: 100 } }
  const aliased = { ...input, state, textures: { warp: first, exponent: first } }
  const distinct = { ...input, state, textures: { warp: first, exponent: second } }
  expect(modelMaterialGraphKey(aliased)).not.toBe(modelMaterialGraphKey(distinct))
  const owner = createCompilerParityOwner(), geometry = new THREE.BoxGeometry()
  owner.admit("aliased warp/exponent", geometry, "panel", aliased, { lighting }, false)
  owner.admit("distinct warp/exponent", geometry, "panel", distinct, { lighting }, false)
  expect(owner.verifyLifetime().programs).toBe(2)
  geometry.dispose(); first.dispose(); second.dispose(); input.baseTexture!.texture.dispose()
})
