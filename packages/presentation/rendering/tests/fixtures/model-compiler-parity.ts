import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeManager from "three/src/renderers/common/nodes/NodeManager.js"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { ModelLightingGraphs, bindModelLighting, bindModelTexture } from "../../src/model-lighting-graphs"
import { modelMaterialGraph, modelMaterialGraphKey, swizzleModelTexture, type ModelMaterialGraphInput } from "../../src/model-material-graphs"
import { sourceModelSurfaceNode, sourceEyeIrisNode, createSourceModelEyeUniforms, createSourceModelLightingUniforms, updateSourceModelLightingUniforms, updateSourceModelEyeUniforms } from "../../src/source-model-lighting"
import { sourceFragmentColor } from "../../src/source-fragment-color"
import { createSourceWaterFogUniforms } from "../../src/source-water"
import { installRenderObjectLifetime } from "../../src/render-object-lifetime"
import { installWebGpuBufferNames } from "../../src/webgpu-buffer-names"
import { bindSourceModelMesh, createSourceModelSkeleton } from "../../src/source-model-skinning"
import { verifyPipelinePreparation } from "./pipeline-preparation"

const require = (condition: unknown, message: string) => { if (!condition) throw new Error(message) }
const equal = (left: unknown, right: unknown, message: string) => require(JSON.stringify(left) === JSON.stringify(right), message)

export function createCompilerParityOwner() {
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.backend.renderer = renderer
  renderer.hasFeature = () => false
  let builds = 0
  const backend = { createNodeBuilder: (mesh: THREE.Mesh) => { builds++; return new THREE.WGSLNodeBuilder(mesh, renderer) } }
  installWebGpuBufferNames(backend)
  const nodes = new NodeManager(renderer, backend), manager = new RenderObjects(renderer, nodes, {}, { delete() {} }, { deleteForRender() {} }, {})
  const lifetime = installRenderObjectLifetime(manager)
  const graphs = new Map<string, ModelLightingGraphs>(), scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(), lights = TSL.lights([])
  const target = new THREE.Texture()
  renderer.backend.get(target).format = "rgba16float"
  const context = { textures: [target], sampleCount: 1, depth: true, depthTexture: null, stencil: false }
  const contexts = { panel: { ...context, id: 1 }, world: { ...context, id: 2 }, view: { ...context, id: 3 } }
  const root = new THREE.Group(), frame = new NodeFrame()
  const retained: any[] = [], records: any[] = []
  const programs = new Map<any, string>(), groups = new Map<any, any>()
  const build = (mesh: THREE.Mesh, pass: keyof typeof contexts) => nodes.getForRender(manager.get(mesh, mesh.material, scene, camera, lights, contexts[pass], null))
  const update = (state: any, mesh: THREE.Mesh) => { frame.object = mesh; frame.camera = camera; frame.renderer = renderer; frame.scene = scene; frame.material = mesh.material as THREE.Material
    frame.renderId++; frame.frameId++
    for (const node of state.updateBeforeNodes) frame.updateBeforeNode(node)
    for (const node of state.updateNodes) frame.updateNode(node)
    for (const group of state.bindings) for (const binding of group.bindings) if (binding.textureNode) binding.update() }
  const layout = (state: any) => state.bindings.map((group: any) => ({ name: group.name, bindings: group.bindings.map((binding: any) => ({
    kind: binding.constructor.name, visibility: binding.visibility, byteLength: binding.byteLength,
    uniforms: binding.uniforms?.map((uniform: any) => ({ kind: uniform.constructor.name, boundary: uniform.boundary, itemSize: uniform.itemSize })),
  })) }))
  const bindingValues = (state: any) => state.bindings.map((group: any) => group.bindings.map((binding: any) => {
    if (binding.uniforms) return binding.uniforms.map((uniform: any) => { const value = uniform.getValue()
      return value?.isMatrix4 || value?.isMatrix3 || value?.isVector4 || value?.isVector3 || value?.isVector2 || value?.isColor ? value.toArray() : value })
    return binding.texture?.uuid ?? binding.node?.value?.uuid ?? null
  }))
  return {
    records,
    get builds() { return builds },
    get programs() { return programs.size },
    async verifyPreparation() {
      return verifyPipelinePreparation(retained.map(({ mesh, pass }) =>
        manager.get(mesh, mesh.material, scene, camera, lights, contexts[pass as keyof typeof contexts], null)), renderer.backend)
    },
    admit(label: string, geometry: THREE.BufferGeometry, pass: keyof typeof contexts, input: ModelMaterialGraphInput, pose: any, skinned: boolean) {
      let graph = graphs.get(pass)
      if (!graph) graphs.set(pass, graph = new ModelLightingGraphs())
      const lighting = createSourceModelLightingUniforms(), eyes = createSourceModelEyeUniforms()
      updateSourceModelLightingUniforms(lighting, pose.lighting)
      if (pose.eye) updateSourceModelEyeUniforms(eyes, pose.eye)
      const material = new THREE.MeshBasicNodeMaterial({ side: input.fragment?.cull === 1 ? THREE.DoubleSide : THREE.BackSide,
        ...(input.fragment ? { transparent: input.fragment.blendEnabled, depthTest: input.fragment.depthTest, depthWrite: input.fragment.depthWrite } : {}) })
      material.toneMapped = false
      const skeleton = skinned ? createSourceModelSkeleton(pose.bones) : undefined
      const mesh = skeleton ? bindSourceModelMesh(geometry, material, skeleton) : new THREE.Mesh(geometry, material)
      if (pass === "view") mesh.layers.set(1)
      bindModelLighting(mesh, lighting, eyes)
      material.colorNode = modelMaterialGraph(mesh, graph, input)
      root.add(mesh)
      const state = build(mesh, pass)
      const count = builds
      require(build(mesh, pass) === state && builds === count, `${label}: repeated compiler state rebuilt`)
      update(state, mesh)
      const candidateValues = bindingValues(state), candidateLayout = layout(state)
      const originalMaterial = material.clone()
      const originalLighting = createSourceModelLightingUniforms(), originalEyes = createSourceModelEyeUniforms()
      updateSourceModelLightingUniforms(originalLighting, pose.lighting)
      if (pose.eye) updateSourceModelEyeUniforms(originalEyes, pose.eye)
      const eyeShader = input.shader === "eyes" || input.shader === "eye-refract"
      const base = eyeShader ? sourceEyeIrisNode(input.textures!.iris!, originalEyes, input.state.dilation!, input.shader === "eye-refract") : input.base
      const surface = sourceModelSurfaceNode(base, originalLighting, { halfLambert: input.state.phong ? true : input.state.halfLambert,
        phong: input.state.phong, diffuseWarp: input.textures?.warp, exponentTexture: input.textures?.exponent, environment: input.environment,
        ...(input.shader === "eye-refract" ? { eye: { ambientOcclusion: input.textures?.ambientOcclusion, ambientOcclusionColor: input.state.ambientOcclusionColor!, glossiness: input.state.glossiness! } } : {}) }, input.exposure)
      originalMaterial.colorNode = sourceFragmentColor(surface.color, input.fragment, input.waterFog)
      const original = skeleton ? bindSourceModelMesh(geometry, originalMaterial, skeleton) : new THREE.Mesh(geometry, originalMaterial)
      original.layers.mask = mesh.layers.mask; root.add(original)
      const dedicated = build(original, pass)
      update(dedicated, original)
      equal(state.vertexShader, dedicated.vertexShader, `${label}: vertex WGSL differs`)
      equal(state.fragmentShader, dedicated.fragmentShader, `${label}: fragment WGSL differs`)
      equal(candidateLayout, layout(dedicated), `${label}: bind layout differs`)
      const dedicatedValues = bindingValues(dedicated)
      if (JSON.stringify(candidateValues) !== JSON.stringify(dedicatedValues)) {
        const differences: any[] = []
        candidateValues.forEach((group: any, g: number) => group.forEach((binding: any, b: number) => {
          if (JSON.stringify(binding) !== JSON.stringify(dedicatedValues[g]?.[b])) differences.push({ group: g, binding: b, candidate: binding, dedicated: dedicatedValues[g]?.[b] })
        }))
        throw new Error(`${label}: draw binding values differ:${JSON.stringify(differences)}`)
      }
      programs.set(state, label)
      const graphKey = modelMaterialGraphKey(input), prior = groups.get(material.colorNode)
      if (prior && prior.pass === pass && prior.skinned === skinned && prior.geometry === geometry) require(prior.state === state, `${label}: equivalent compiler state did not reuse`)
      groups.set(material.colorNode, { pass, skinned, geometry, state })
      lifetime.release(original); original.removeFromParent(); originalMaterial.dispose()
      const record = { label, pass, skinned, graphKey, vertexShader: state.vertexShader, fragmentShader: state.fragmentShader,
        bindLayout: candidateLayout, bindingsEqual: true, repeatReuse: true }
      records.push(record)
      retained.push({ mesh, state, lighting, eyes, input, graph, geometry, skeleton, label, pass, candidateValues })
    },
    verifyLifetime() {
      const before = builds
      // Reverse draw order includes exact repeated selections with different
      // material/texture values; a previous primitive must not own this draw.
      for (const entry of retained.toReversed()) {
        require(build(entry.mesh, entry.pass) === entry.state, `${entry.label}: warm reuse`)
        update(entry.state, entry.mesh)
        equal(bindingValues(entry.state), entry.candidateValues, `${entry.label}: reverse selection borrowed another draw's values`)
        require(entry.mesh.clone().userData.sourceBaseTexture === undefined, `${entry.label}: enumerable texture binding`)
      }
      require(builds === before, "warm repeated selection rebuilt compiler state")
      for (const graph of graphs.values()) graph.releaseDrawReferences()
      for (const entry of retained) {
        update(entry.state, entry.mesh)
        equal(bindingValues(entry.state), entry.candidateValues, `${entry.label}: graph handoff rebound incorrectly`)
      }
      // Cancellation after construction and after compile owns the same cleanup.
      const cancelled = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicNodeMaterial())
      root.add(cancelled); lifetime.release(cancelled); cancelled.removeFromParent(); cancelled.geometry.dispose(); cancelled.material.dispose()
      lifetime.release(root); lifetime.release(root)
      require(lifetime.size === 0 && nodes.nodeBuilderCache.size === 0, "retired draws retained compiler states")
      for (const entry of retained) { entry.skeleton?.dispose(); entry.mesh.material.dispose() }
      for (const graph of graphs.values()) graph.releaseDrawReferences()
      require([...graphs.values()].every(graph => graph.lighting.cameraPosition.value === null && graph.phong.tint.value === null), "handoff retained draw values")
      lifetime.restore()
      return { coldBuilds: builds, draws: retained.length, programs: programs.size, graphs: [...graphs.values()].reduce((sum, graph) => sum + graph.size, 0),
        repeatedBuilds: builds - before, retiredDraws: lifetime.size, retainedCompilerStates: nodes.nodeBuilderCache.size }
    },
  }
}

export { THREE, TSL, swizzleModelTexture, createSourceWaterFogUniforms }
