import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeManager from "three/src/renderers/common/nodes/NodeManager.js"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { ModelLightingGraphs, bindModelTexture, bindModelLighting, modelBaseTextureShape, perObjectModelTextures, transferModelBindings } from "../src/model-lighting-graphs"
import { createSourceModelLightingUniforms, sourceModelSurfaceNode } from "../src/source-model-lighting"
import { installRenderObjectLifetime } from "../src/render-object-lifetime"

test("different base planes share one actual compiler state without sharing their draw binding", () => {
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.hasFeature = () => false
  const graphs = new ModelLightingGraphs(), textures = [new THREE.Texture(), new THREE.Texture()]
  const sampler = TSL.texture(textures[0]!, TSL.uv())
  const warps = [new THREE.Texture(), new THREE.Texture()], exponents = [new THREE.Texture(), new THREE.Texture()]
  const warp = TSL.texture(warps[0]!, TSL.uv()), exponent = TSL.texture(exponents[0]!, TSL.uv())
  const phong = { maskSource: 0, invertMask: false, albedoTint: true, exponent: 5, exponentFactor: 100,
    tint: [1, 1, 1] as const, boost: 1, packedFresnel: [1, 1, 1] as const, rim: { exponent: 8, boost: .8, exponentTextureAlphaMask: true } }
  const surface = sourceModelSurfaceNode(sampler, graphs.lighting, { halfLambert: true, phong, phongUniforms: graphs.phong, diffuseWarp: warp, exponentTexture: exponent }, TSL.float(1)).color
  const color = perObjectModelTextures(surface, [{ name: "sourceBaseTexture", node: sampler }, { name: "sourceWarpTexture", node: warp }, { name: "sourceExponentTexture", node: exponent }])
  const meshes = textures.map((texture, index) => {
    const material = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
    material.colorNode = color
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(), material)
    bindModelLighting(mesh, createSourceModelLightingUniforms()); bindModelTexture(mesh, "sourceBaseTexture", texture)
    bindModelTexture(mesh, "sourceWarpTexture", warps[index]!); bindModelTexture(mesh, "sourceExponentTexture", exponents[index]!)
    graphs.bindPhong(mesh, index === 0 ? phong : { ...phong, boost: .3, tint: [.2, 1, .5], packedFresnel: [1, .4, 1], rim: { ...phong.rim, exponent: 4 } })
    return mesh
  })
  let builds = 0
  const backend = { createNodeBuilder: (mesh: THREE.Mesh) => { builds++; return new THREE.WGSLNodeBuilder(mesh, renderer) } }
  const nodes = new NodeManager(renderer, backend), manager = new RenderObjects(renderer, nodes, {}, { delete() {} }, { deleteForRender() {} }, {})
  const lifetime = installRenderObjectLifetime(manager), root = new THREE.Group()
  meshes.forEach(mesh => root.add(mesh))
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(), context = { id: 1 }, lights = TSL.lights([])
  const build = (mesh: THREE.Mesh) => nodes.getForRender(manager.get(mesh, mesh.material, scene, camera, lights, context, null))
  const first = build(meshes[0]!)
  expect(build(meshes[1]!)).toBe(first)
  expect(builds).toBe(1)
  const events = first.updateNodes.filter((node: any) => node.type === "EventNode")
  expect(events).toHaveLength(1)
  for (const index of [1, 0, 1, 0]) {
    events[0]!.update({ object: meshes[index] } as any)
    expect(sampler.value).toBe(textures[index])
    expect(warp.value).toBe(warps[index]); expect(exponent.value).toBe(exponents[index])
    const boost = graphs.phong.boost as any
    const frame = new NodeFrame(); frame.object = meshes[index]!
    frame.updateNode(boost._beforeNodes[0])
    expect(boost.value).toBe(index === 0 ? 1 : .3)
    expect(meshes[index]!.clone().userData).toEqual({})
  }
  const originalMaterial = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
  originalMaterial.colorNode = sourceModelSurfaceNode(TSL.texture(textures[0]!, TSL.uv()), graphs.lighting,
    { halfLambert: true, phong, diffuseWarp: warps[0], exponentTexture: exponents[0] }, TSL.float(1)).color
  const original = new THREE.Mesh(new THREE.BoxGeometry(), originalMaterial)
  bindModelLighting(original, createSourceModelLightingUniforms())
  const before = build(original)
  expect(first.vertexShader).toBe(before.vertexShader)
  expect(first.fragmentShader).toBe(before.fragmentShader)
  const posed = new THREE.SkinnedMesh()
  transferModelBindings(meshes[0]!, posed)
  expect(posed.userData.sourceBaseTexture).toBe(textures[0])
  expect(posed.clone().userData).toEqual({})
  lifetime.release(root); lifetime.release(original)
  graphs.releaseDrawReferences()
  expect(graphs.phong.boost.value).toBeNull()
  expect(nodes.nodeBuilderCache.size).toBe(0)
  lifetime.restore()
})

test("base sampler interpretation remains structural while plane identity is not", () => {
  const a = new THREE.Texture(), b = new THREE.Texture()
  expect(modelBaseTextureShape(a, 0)).toBe(modelBaseTextureShape(b, 0))
  expect(modelBaseTextureShape(a, 1)).not.toBe(modelBaseTextureShape(b, 0))
  b.colorSpace = THREE.SRGBColorSpace
  expect(modelBaseTextureShape(a, 0)).not.toBe(modelBaseTextureShape(b, 0))
  b.colorSpace = a.colorSpace; b.flipY = !a.flipY
  expect(modelBaseTextureShape(a, 0)).not.toBe(modelBaseTextureShape(b, 0))
})
