import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { ModelLightingGraphs, bindStaticPropFade } from "../../src/model-lighting-graphs"
import { sourceStaticVertexLightingNode } from "../../src/source-model-lighting"
import { installRenderObjectLifetime } from "../../src/render-object-lifetime"

/** Compare the old dedicated fade uniforms with the occurrence-bound material,
 * including overlapping VHV primitives, first visibility and draw retirement. */
export async function createStaticPropGraphProbe() {
  const renderer = new THREE.WebGPURenderer({ antialias: false })
  await renderer.init()
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace
  renderer.toneMapping = THREE.NoToneMapping
  renderer.setSize(640, 400)
  document.body.append(renderer.domElement)
  const lifetime = installRenderObjectLifetime((renderer as any)._objects)
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(50, 1.6, .1, 100)
  scene.background = new THREE.Color(.03, .04, .05)
  camera.position.set(0, 0, 5)
  const target = new THREE.RenderTarget(640, 400)
  target.depthTexture = new THREE.DepthTexture(640, 400, THREE.FloatType)
  const depthTarget = new THREE.RenderTarget(640, 400, { type: THREE.FloatType, depthBuffer: false })
  const depthScene = new THREE.Scene(), depthCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
  const depthMaterial = new THREE.MeshBasicNodeMaterial()
  depthMaterial.colorNode = TSL.vec4(TSL.texture(target.depthTexture).r, 0, 0, 1)
  const depthQuad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), depthMaterial)
  depthScene.add(depthQuad)
  const backdrop = new THREE.Mesh(new THREE.PlaneGeometry(8, 5), new THREE.MeshBasicNodeMaterial({ color: 0x38516b }))
  backdrop.position.z = -1
  scene.add(backdrop)
  const make = () => {
    const graphs = new ModelLightingGraphs(), root = new THREE.Group()
    const shared = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide, transparent: true, depthWrite: false })
    shared.colorNode = TSL.vec4(sourceStaticVertexLightingNode(), graphs.staticFade)
    shared.toneMapped = false
    const props = [0, 1, 2].map(index => {
      const geometry = new THREE.BoxGeometry(1.4, 1.4, .4)
      const indices = geometry.index!.array
      for (let i = 0; i < indices.length; i += 3) [indices[i + 1], indices[i + 2]] = [indices[i + 2]!, indices[i + 1]!]
      const colors = new Uint8Array(geometry.getAttribute("position").count * 4)
      for (let i = 0; i < colors.length; i += 4) colors.set([70 + index * 60, 180 - index * 35, 120 + index * 20, 255], i)
      geometry.setAttribute("staticLighting", new THREE.Uint8BufferAttribute(colors, 4, true))
      const fade = TSL.uniform(.25 + .25 * index)
      const dedicated = shared.clone()
      dedicated.colorNode = TSL.vec4(sourceStaticVertexLightingNode(), fade)
      const mesh = new THREE.Mesh(geometry, shared)
      mesh.position.set((index - 1) * .9, index % 2 ? .2 : -.2, index * .2)
      bindStaticPropFade(mesh, fade)
      root.add(mesh)
      return { mesh, dedicated, fade }
    })
    scene.add(root)
    return { root, props, shared, graphs }
  }
  let owner = make(), width = 640, height = 400
  const retire = () => {
    lifetime.release(owner.root); owner.graphs.releaseDrawReferences(); owner.root.removeFromParent()
    for (const prop of owner.props) { prop.mesh.geometry.dispose(); prop.dedicated.dispose() }
    owner.shared.dispose()
  }
  const capture = async (shared: boolean) => {
    for (const prop of owner.props) prop.mesh.material = shared ? owner.shared : prop.dedicated
    renderer.setRenderTarget(target); renderer.render(scene, camera)
    const color = new Uint8Array(await renderer.readRenderTargetPixelsAsync(target, 0, 0, width, height))
    renderer.setRenderTarget(depthTarget); renderer.render(depthScene, depthCamera)
    const depth = new Uint8Array((await renderer.readRenderTargetPixelsAsync(depthTarget, 0, 0, width, height)).buffer)
    renderer.setRenderTarget(null); renderer.render(scene, camera)
    await renderer.backend.device.queue.onSubmittedWorkDone()
    return { color, depth, pixels: renderer.domElement.toDataURL("image/png") }
  }
  return {
    async compare(phase: number) {
      if (phase === 2) { retire(); owner = make() }
      if (phase === 1 || phase === 3) {
        width = phase === 1 ? 480 : 640; height = phase === 1 ? 300 : 400
        renderer.setSize(width, height); target.setSize(width, height); depthTarget.setSize(width, height)
        camera.aspect = width / height; camera.updateProjectionMatrix()
      }
      for (const [index, prop] of owner.props.entries()) {
        prop.fade.value = [1, .25, .75, .5][(index + phase) % 4]!
        prop.mesh.material = owner.shared
        prop.mesh.visible = index === 0
      }
      // Prepare just the actual first occurrence, never the hidden siblings.
      renderer.setRenderTarget(target); await renderer.compileAsync(scene, camera)
      renderer.setRenderTarget(null); await renderer.compileAsync(scene, camera)
      const nodes = (renderer as any)._nodes, create = nodes._createNodeBuilder
      let builds = 0
      nodes._createNodeBuilder = function (...args: any[]) { builds++; return create.apply(this, args) }
      const programs = renderer.info.memory.programs
      for (const prop of owner.props) prop.mesh.visible = true
      renderer.setRenderTarget(target); renderer.render(scene, camera)
      renderer.setRenderTarget(null); renderer.render(scene, camera)
      nodes._createNodeBuilder = create
      const newPrograms = renderer.info.memory.programs - programs
      const before = await capture(false), after = await capture(true)
      const mismatches = (a: Uint8Array, b: Uint8Array) => a.reduce((sum, byte, i) => sum + Number(byte !== b[i]), 0)
      return { phase, width, height, builds, newPrograms, colorMismatches: mismatches(before.color, after.color), depthMismatches: mismatches(before.depth, after.depth),
        colorBytes: after.color.length, depthBytes: after.depth.length, draws: lifetime.size,
        samples: [.3, .5, .7].map(x => [...after.color.slice((Math.floor(height / 2) * width + Math.floor(width * x)) * 4, (Math.floor(height / 2) * width + Math.floor(width * x)) * 4 + 4)]),
        beforePixels: before.pixels, afterPixels: after.pixels }
    },
    dispose() {
      retire(); lifetime.release(scene); lifetime.release(depthScene)
      const draws = lifetime.size
      lifetime.restore(); backdrop.geometry.dispose(); backdrop.material.dispose(); depthQuad.geometry.dispose(); depthMaterial.dispose(); target.dispose(); depthTarget.dispose(); renderer.dispose()
      return draws
    },
  }
}
