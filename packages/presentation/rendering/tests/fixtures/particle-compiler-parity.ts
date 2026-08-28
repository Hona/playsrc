import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeManager from "three/src/renderers/common/nodes/NodeManager.js"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { ParticleMaterialGraphs, particleMaterialNodes } from "../../src/particle-material-graphs"
import { createSourceWaterFogUniforms } from "../../src/source-water"
import { sourceTextureLayout } from "../../src/source-texture-layout"
import { particlePreparationSides } from "../../src/particle-pipeline"
import { installRenderObjectLifetime } from "../../src/render-object-lifetime"

export function verifyParticleCompilerParity(inputs: readonly any[], hdr: boolean) {
  const require = (condition: unknown, message: string) => { if (!condition) throw new Error(message) }
  const equal = (a: unknown, b: unknown, message: string) => require(JSON.stringify(a) === JSON.stringify(b), message)
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.hasFeature = () => false
  let builds = 0
  const nodes = new NodeManager(renderer, { createNodeBuilder: (mesh: THREE.Mesh) => { builds++; return new THREE.WGSLNodeBuilder(mesh, renderer) } })
  const manager = new RenderObjects(renderer, nodes, {}, { delete() {} }, { deleteForRender() {} }, {})
  const lifetime = installRenderObjectLifetime(manager), graphs = new ParticleMaterialGraphs()
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(), lights = TSL.lights([]), context = { id: 1 }
  const frame = new NodeFrame(), root = new THREE.Group(), geometry = new THREE.BufferGeometry(), textures: THREE.Texture[] = []
  for (const [name, size] of [["position", 3], ["particleCenterOrientation", 4], ["uv", 2], ["particleUvNext", 2], ["particleSheetBlend", 1], ["particleColor", 4]] as const) {
    geometry.setAttribute(name, new THREE.BufferAttribute(new Float32Array(size * 4), size).setUsage(THREE.DynamicDrawUsage))
  }
  geometry.setIndex([0, 1, 2, 0, 2, 3])
  const waterFog = createSourceWaterFogUniforms(), exposure = TSL.uniform(1), depthTexture = new THREE.Texture(), depth = TSL.texture(depthTexture)
  const fog = { start: TSL.uniform(10), end: TSL.uniform(1000), maximumDensity: TSL.uniform(1), enabled: TSL.uniform(1) }
  const retained: any[] = [], records: any[] = [], states = new Set(), dedicatedStates = new Set()
  const build = (mesh: THREE.Mesh) => nodes.getForRender(manager.get(mesh, mesh.material, scene, camera, lights, context, null))
  const update = (state: any, mesh: THREE.Mesh) => {
    frame.object = mesh; frame.camera = camera; frame.renderer = renderer; frame.scene = scene; frame.material = mesh.material as THREE.Material
    frame.frameId++; frame.renderId++
    for (const node of state.updateBeforeNodes) frame.updateBeforeNode(node)
    for (const node of state.updateNodes) frame.updateNode(node)
    for (const group of state.bindings) for (const binding of group.bindings) if (binding.textureNode) binding.update()
    return state.bindings.map((group: any) => group.bindings.map((binding: any) => ({ kind: binding.constructor.name, visibility: binding.visibility, byteLength: binding.byteLength,
      uniforms: binding.uniforms?.map((uniform: any) => { const value = uniform.getValue(); return { kind: uniform.constructor.name, boundary: uniform.boundary, itemSize: uniform.itemSize,
        value: value?.toArray ? value.toArray() : value } }), texture: binding.texture?.uuid ?? null })))
  }
  try {
    for (const input of inputs) {
      require(input.state && input.sourceSha256, "Exact particle state/texture identity is required")
      const layout = sourceTextureLayout(input.sourceFormat, input.scalarEncoding)!
      require(layout, "Unsupported actual particle texture")
      const texture = layout.compressed === null ? new THREE.DataTexture(null, input.width, input.height) : new THREE.CompressedTexture([], input.width, input.height, layout.compressed, layout.type)
      texture.type = layout.type; texture.format = layout.format; texture.flipY = false; texture.generateMipmaps = false
      texture.colorSpace = input.additiveSprite && !input.additiveSprite.srgb ? THREE.NoColorSpace : THREE.SRGBColorSpace
      textures.push(texture)
      const options = { transparent: true, side: input.state.cull === 1 ? THREE.DoubleSide : THREE.FrontSide,
        depthTest: input.state.depthTest, depthWrite: input.state.depthWrite, fog: !input.additiveSprite && input.state.fog !== 2 }
      const template = new THREE.MeshBasicNodeMaterial(options)
      template.forceSinglePass = Boolean(input.spriteCard || input.additiveSprite)
      const graphInput = { texture, state: input.state, spriteCard: input.spriteCard, additive: input.additiveSprite, waterFog, depth, exposure, hdr, fog }
      for (const side of particlePreparationSides(template)) {
        const material = template.clone(); material.side = side; material.toneMapped = false
        const shared = graphs.get(material, graphInput), dedicated = particleMaterialNodes(graphInput)
        material.colorNode = shared.color; if (shared.position) material.positionNode = shared.position
        const originalMaterial = template.clone(); originalMaterial.side = side; originalMaterial.toneMapped = false
        originalMaterial.colorNode = dedicated.color; if (dedicated.position) originalMaterial.positionNode = dedicated.position
        const mesh = new THREE.Mesh(geometry, material), original = new THREE.Mesh(geometry, originalMaterial)
        root.add(mesh, original)
        const state = build(mesh), reference = build(original)
        dedicatedStates.add(reference)
        equal(state.vertexShader, reference.vertexShader, `${input.material}: vertex WGSL differs`)
        equal(state.fragmentShader, reference.fragmentShader, `${input.material}: fragment WGSL differs`)
        const bindings = update(state, mesh)
        equal(bindings, update(reference, original), `${input.material}: layout/draw bindings differ`)
        states.add(state); retained.push({ mesh, state, bindings })
        records.push({ material: input.material, side, vertex: state.vertexShader, fragment: state.fragmentShader })
        lifetime.release(original); original.removeFromParent(); originalMaterial.dispose()
      }
      template.dispose()
    }
    const before = builds
    for (const { mesh, state, bindings } of retained.toReversed()) {
      require(build(mesh) === state, "Warm particle compiler state rebuilt")
      equal(update(state, mesh), bindings, "Particle draw borrowed another material's texture")
      require(mesh.material.clone().userData.sourceParticleTexture === undefined, "Particle texture binding was serialized")
    }
    require(builds === before, "Warm particle compilation allocated")
    return { draws: retained.length, compilerStates: states.size, dedicatedCompilerStates: dedicatedStates.size, graphFamilies: graphs.size, warmBuilds: builds - before, records }
  } finally {
    lifetime.release(root)
    require(lifetime.size === 0 && nodes.nodeBuilderCache.size === 0, "Retired particle compiler states remain")
    for (const { mesh } of retained) mesh.material.dispose()
    for (const texture of textures) texture.dispose()
    depthTexture.dispose(); geometry.dispose(); lifetime.restore()
  }
}
