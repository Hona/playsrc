import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeManager from "three/src/renderers/common/nodes/NodeManager.js"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import { ModelLightingGraphs, bindStaticPropFade } from "../src/model-lighting-graphs"
import { sourceStaticVertexLightingNode } from "../src/source-model-lighting"
import { installRenderObjectLifetime } from "../src/render-object-lifetime"

test("VHV fade is an occurrence binding, not a new program on first visibility, resize or replacement", () => {
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.hasFeature = () => false
  let builds = 0
  const backend = { createNodeBuilder: (mesh: THREE.Mesh) => { builds++; return new THREE.WGSLNodeBuilder(mesh, renderer) } }
  const nodes = new NodeManager(renderer, backend)
  const manager = new RenderObjects(renderer, nodes, {}, { delete() {} }, { deleteForRender() {} }, {})
  const lifetime = installRenderObjectLifetime(manager)
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(), context = { id: 1 }
  const lights = TSL.lights([])
  for (let generation = 0; generation < 2; generation++) {
    const graphs = new ModelLightingGraphs()
    const material = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide, transparent: true, depthWrite: false })
    material.colorNode = TSL.vec4(sourceStaticVertexLightingNode(), graphs.staticFade)
    material.toneMapped = false
    const root = new THREE.Group()
    const meshes = [0, 1].map(index => {
      const geometry = new THREE.BoxGeometry()
      geometry.setAttribute("staticLighting", new THREE.Uint8BufferAttribute(new Uint8Array(geometry.getAttribute("position").count * 4).fill(80 + index * 60), 4, true))
      const mesh = new THREE.Mesh(geometry, material)
      bindStaticPropFade(mesh, TSL.uniform(index === 0 ? .25 : .75))
      root.add(mesh)
      return mesh
    })
    const build = (mesh: THREE.Mesh) => nodes.getForRender(manager.get(mesh, material, scene, camera, lights, context, null))
    const prepared = build(meshes[0]!)
    expect(builds).toBe(generation + 1)
    // Previously each fading occurrence captured its own UniformNode in a
    // unique graph, even when its VHV layout/material/pass matched this one.
    meshes[1]!.visible = false
    meshes[1]!.visible = true
    expect(build(meshes[1]!)).toBe(prepared)
    camera.aspect = 1.75; camera.updateProjectionMatrix()
    expect(build(meshes[1]!)).toBe(prepared)
    expect(builds).toBe(generation + 1)
    const draw = manager.get(meshes[1]!, material, scene, camera, lights, context, null)
    const otherPass = manager.get(meshes[1]!, material, scene, camera, lights, { id: 2 }, null)
    expect(otherPass.initialCacheKey).not.toBe(draw.initialCacheKey)
    const otherSide = material.clone(); otherSide.side = THREE.FrontSide
    expect(manager.get(meshes[1]!, otherSide, scene, camera, lights, context, null).initialCacheKey).not.toBe(draw.initialCacheKey)
    const fade = graphs.staticFade as any
    for (const index of [1, 0, 1, 0]) {
      fade.updateReference({ object: meshes[index] }); fade.updateValue()
      expect(fade.node.value).toBe(index === 0 ? .25 : .75)
      expect(meshes[index]!.clone().userData).toEqual({})
    }
    expect(meshes[0]!.geometry.getAttribute("staticLighting").getX(0)).not.toBe(meshes[1]!.geometry.getAttribute("staticLighting").getX(0))
    lifetime.release(root)
    graphs.releaseDrawReferences()
    expect(fade.reference).toBeNull()
    expect(nodes.nodeBuilderCache.size).toBe(0)
    expect(lifetime.size).toBe(0)
    material.dispose()
    otherSide.dispose()
    for (const mesh of meshes) mesh.geometry.dispose()
  }
  lifetime.restore()
})
