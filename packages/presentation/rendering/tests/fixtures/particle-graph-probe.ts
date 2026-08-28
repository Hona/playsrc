import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { ParticleMaterialGraphs, particleMaterialNodes, bindParticleTexture } from "../../src/particle-material-graphs"
import { createSourceWaterFogUniforms } from "../../src/source-water"
import { StaticMaterialGraphs } from "../../src/static-material-graphs"
import { ModelLightingGraphs, bindStaticPropFade } from "../../src/model-lighting-graphs"
import { sourceFragmentColor } from "../../src/source-fragment-color"
import { sourceStaticVertexLightingNode } from "../../src/source-model-lighting"

/** Real overlapping translucent pixels and depth, not a performance sample. */
export async function createParticleGraphProbe(staticMaterials = false) {
  const width = 640, height = 480, renderer = new THREE.WebGPURenderer({ antialias: false })
  await renderer.init()
  if (!renderer.backend.isWebGPUBackend) throw new Error("Particle parity requires WebGPU")
  renderer.setSize(width, height); renderer.outputColorSpace = THREE.LinearSRGBColorSpace; renderer.toneMapping = THREE.NoToneMapping
  document.body.append(renderer.domElement)
  const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera(50, width / height, .1, 100)
  scene.background = new THREE.Color(.03, .04, .05); camera.position.z = 5
  const target = new THREE.RenderTarget(width, height)
  target.depthTexture = new THREE.DepthTexture(width, height, THREE.FloatType)
  const depthTarget = new THREE.RenderTarget(width, height, { type: THREE.FloatType, depthBuffer: false })
  const depthMaterial = new THREE.MeshBasicNodeMaterial()
  depthMaterial.colorNode = TSL.vec4(TSL.texture(target.depthTexture).r, 0, 0, 1); depthMaterial.toneMapped = false
  const depthScene = new THREE.Scene(), depthCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
  const depthQuad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), depthMaterial); depthScene.add(depthQuad)
  const blocker = new THREE.Mesh(new THREE.PlaneGeometry(.3, 2), new THREE.MeshBasicNodeMaterial({ color: 0x808080 }))
  blocker.position.set(.3, 0, 1); scene.add(blocker)
  const planes = [[240, 40, 20, 180], [20, 90, 240, 210], [40, 220, 60, 240]].map(rgba => {
    const texture = new THREE.DataTexture(new Uint8Array(rgba), 1, 1); texture.colorSpace = THREE.SRGBColorSpace; texture.needsUpdate = true; return texture
  })
  const graphs = new ParticleMaterialGraphs(), waterFog = createSourceWaterFogUniforms()
  const lighting = new ModelLightingGraphs(), exposure = TSL.uniform(1)
  const staticGraphs = new StaticMaterialGraphs(waterFog, exposure, lighting.staticFade)
  const bases = planes.map(texture => TSL.texture(texture, TSL.uv()))
  const actors = planes.map((texture, index) => {
    const geometry = new THREE.PlaneGeometry(1.6, 1.6)
    geometry.setAttribute("particleCenterOrientation", new THREE.Float32BufferAttribute(new Float32Array(16), 4))
    geometry.setAttribute("particleUvNext", geometry.getAttribute("uv").clone())
    geometry.setAttribute("particleSheetBlend", new THREE.Float32BufferAttribute([0, .25, .5, 1], 1))
    geometry.setAttribute("particleColor", new THREE.Float32BufferAttribute(Array.from({ length: 16 }, (_, i) => i % 4 === 3 ? .6 : 1), 4))
    const original = new THREE.MeshBasicNodeMaterial({ transparent: true, depthWrite: false }), shared = original.clone()
    original.toneMapped = false; shared.toneMapped = false
    const state = { alphaModulation: .8, blendEnabled: true, alphaOwnership: { opacity: false }, fragmentDiscard: { kind: "none" }, fog: 1 } as any
    const input = { texture, state, waterFog, depth: TSL.vec4(1), exposure: TSL.float(1), hdr: false,
      fog: { start: TSL.float(0), end: TSL.float(100), enabled: TSL.float(1), maximumDensity: TSL.float(1) } }
    const samplers: any[] = []
    if(!staticMaterials) {
      original.colorNode = particleMaterialNodes(input, (nodes, color) => { samplers.push(...nodes); return color }).color
      shared.colorNode = graphs.get(shared, input).color
    }
    const mesh = new THREE.Mesh(geometry, shared); mesh.position.set((index - 1) * .85, (index % 2) * .35 - .15, index * .2); scene.add(mesh)
    const fade = TSL.uniform(1)
    if(staticMaterials) {
      geometry.setAttribute("staticLighting", new THREE.Uint8BufferAttribute(Array.from({length:16},(_,i)=>i%4===3?255:120+index*30),4,true))
      const base=bases[index===2?1:0], before=sourceFragmentColor(base,state,waterFog)
      original.colorNode=sourceFragmentColor(TSL.vec4(before.rgb.mul(sourceStaticVertexLightingNode()).mul(exposure),TSL.float(1).mul(lighting.staticFade)),state,waterFog,true)
      shared.colorNode=staticGraphs.vertex(staticGraphs.template(base,state),state,false,true,fade)
      bindStaticPropFade(mesh,fade)
    }
    return { original, shared, mesh, samplers, fade }
  })
  const capture = async (shared: boolean) => {
    for (const actor of actors) actor.mesh.material = shared ? actor.shared : actor.original
    renderer.setRenderTarget(target); await renderer.compileAsync(scene, camera); renderer.render(scene, camera)
    const color = new Uint8Array(await renderer.readRenderTargetPixelsAsync(target, 0, 0, width, height))
    renderer.setRenderTarget(depthTarget); renderer.render(depthScene, depthCamera)
    const depth = new Uint8Array((await renderer.readRenderTargetPixelsAsync(depthTarget, 0, 0, width, height)).buffer)
    renderer.setRenderTarget(null); renderer.render(scene, camera); await renderer.backend.device.queue.onSubmittedWorkDone()
    return { color, depth, pixels: renderer.domElement.toDataURL("image/png") }
  }
  return {
    async compare(phase: number) {
      if (phase === 1) actors[1]!.mesh.removeFromParent()
      if (phase === 2) scene.add(actors[1]!.mesh)
      for (const [index, actor] of actors.entries()) {
        const texture = planes[(index + phase) % planes.length]!
        if(!staticMaterials) {
          bindParticleTexture(actor.shared, texture)
          for (const sampler of actor.samplers) sampler.value = texture
        }
        actor.fade.value=1-(index+phase)%3*.2
        actor.mesh.position.y += .03 * phase
      }
      const before = await capture(false), after = await capture(true)
      const mismatches = (a: Uint8Array, b: Uint8Array) => a.reduce((sum, value, index) => sum + Number(value !== b[index]), 0)
      return { phase, colorMismatches: mismatches(before.color, after.color), depthMismatches: mismatches(before.depth, after.depth),
        graphs: staticMaterials?staticGraphs.size:graphs.size, colorBytes: after.color.length, depthBytes: after.depth.length,
        visibleActors: actors.filter(actor => actor.mesh.parent === scene).length, beforePixels: before.pixels, afterPixels: after.pixels }
    },
    dispose() {
      for (const actor of actors) { actor.original.dispose(); actor.shared.dispose(); actor.mesh.geometry.dispose() }
      for (const texture of planes) texture.dispose()
      blocker.geometry.dispose(); blocker.material.dispose(); depthQuad.geometry.dispose(); depthMaterial.dispose(); target.dispose(); depthTarget.dispose(); renderer.dispose()
    },
  }
}
