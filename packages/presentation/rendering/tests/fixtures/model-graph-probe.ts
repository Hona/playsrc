import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { ModelLightingGraphs, bindModelLighting, bindModelEnvironment, perObjectModelEnvironment } from "../../src/model-lighting-graphs"
import { createSourceModelLightingUniforms, createSourceModelEyeUniforms, sourceEyeIrisNode, sourceModelSurfaceNode, updateSourceModelLightingUniforms } from "../../src/source-model-lighting"
import { bindSourceModelMesh, createSourceModelSkeleton, updateSourceModelSkeleton } from "../../src/source-model-skinning"
import { RetainedModelCache } from "../../src/retained-model-cache"

/** Diagnostic equivalence, not a gameplay/performance benchmark. Both paths
 * draw the same multi-object scene, including overlapping skinned geometry. */
export async function createModelGraphProbe() {
  const width = 640, height = 480
  const renderer = new THREE.WebGPURenderer({ antialias: false })
  await renderer.init()
  renderer.setSize(width, height)
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace
  renderer.toneMapping = THREE.NoToneMapping
  document.body.append(renderer.domElement)
  const scene = new THREE.Scene()
  scene.background = new THREE.Color(.03, .04, .05)
  const camera = new THREE.PerspectiveCamera(50, width / height, .1, 100)
  camera.position.set(0, 0, 5)
  const target = new THREE.RenderTarget(width, height)
  target.depthTexture = new THREE.DepthTexture(width, height, THREE.FloatType)
  const depthTarget = new THREE.RenderTarget(width, height, { type: THREE.FloatType, depthBuffer: false })
  const depthScene = new THREE.Scene(), depthCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
  const depthMaterial = new THREE.MeshBasicNodeMaterial()
  depthMaterial.colorNode = TSL.vec4(TSL.texture(target.depthTexture).r, 0, 0, 1)
  depthMaterial.toneMapped = false
  const depthQuad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), depthMaterial)
  depthScene.add(depthQuad)
  const graphs = new ModelLightingGraphs()
  const iris = new THREE.DataTexture(new Uint8Array([255, 30, 10, 255, 20, 220, 50, 255, 20, 30, 240, 255, 220, 180, 20, 255]), 2, 2)
  iris.needsUpdate = true
  const cubemaps = [[60, 10, 20, 255], [10, 35, 70, 255]].map(rgba => {
    const faces = Array.from({ length: 6 }, () => new THREE.DataTexture(new Uint8Array(rgba), 1, 1))
    const texture = new THREE.CubeTexture(faces)
    texture.needsUpdate = true
    return texture
  })
  const geometry = new THREE.SphereGeometry(.75, 20, 12)
  // Source winding with outward authored normals.
  const indices = geometry.index!.array
  for (let i = 0; i < indices.length; i += 3) [indices[i + 1], indices[i + 2]] = [indices[i + 2]!, indices[i + 1]!]
  const count = geometry.getAttribute("position").count
  geometry.setAttribute("skinIndex", new THREE.BufferAttribute(new Uint16Array(count * 4), 4))
  geometry.setAttribute("skinWeight", new THREE.BufferAttribute(Float32Array.from({ length: count * 4 }, (_, i) => Number(i % 4 === 0)), 4))
  const matrix = (index: number, phase: number) => Float32Array.from([1, 0, 0, .05 * index * phase, 0, 1, 0, .04 * phase, 0, 0, 1, .02 * phase])
  const actors = [0, 1, 2].map(index => {
    const lighting = createSourceModelLightingUniforms(), eye = createSourceModelEyeUniforms()
    const skeleton = createSourceModelSkeleton(matrix(index, 0))
    const original = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
    original.toneMapped = false
    const shared = original.clone()
    const mesh = bindSourceModelMesh(geometry, shared, skeleton)
    mesh.position.set((index - 1) * .9, (index % 2) * .35 - .15, index * .22)
    if (index === 1) mesh.scale.set(1, -.85, 1.1)
    bindModelLighting(mesh, lighting, eye)
    bindModelEnvironment(mesh, cubemaps[index % 2]!)
    const surface = { halfLambert: true, environment: { texture: cubemaps[0]!, tint: [1, 1, 1] as const, scale: 1 }, phong: {
      maskSource: 0, invertMask: false, albedoTint: false, exponent: 8, exponentFactor: 0,
      tint: [1, .7, .4] as const, boost: .5, packedFresnel: [.1, .4, 1] as const,
      rim: { exponent: 2, boost: .2, exponentTextureAlphaMask: false },
    } }
    const dedicated = sourceModelSurfaceNode(sourceEyeIrisNode(iris, eye, .4, true), lighting, surface, TSL.float(1))
    original.colorNode = dedicated.color
    shared.colorNode = graphs.get("eye-phong-environment", () => {
      const value = sourceModelSurfaceNode(sourceEyeIrisNode(iris, graphs.eyes, .4, true), graphs.lighting, surface, TSL.float(1))
      return perObjectModelEnvironment(value.color, value.environmentNode)
    })
    scene.add(mesh)
    return { mesh, lighting, eye, skeleton, original, shared, dedicated }
  })
  const parked = new RetainedModelCache<typeof actors[number]>(3, () => { throw Error("unexpected probe eviction") })
  const capture = async (shared: boolean) => {
    for (const actor of actors) actor.mesh.material = shared ? actor.shared : actor.original
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
      if (phase === 1) {
        const actor = actors[1]!
        actor.mesh.removeFromParent(); parked.retain("world:1:eye:0", actor)
      }
      if (phase === 2) scene.add(parked.take("world:1:eye:0")!.mesh)
      for (const [index, actor] of actors.entries()) {
        updateSourceModelSkeleton(actor.skeleton, matrix(index, phase))
        updateSourceModelLightingUniforms(actor.lighting, {
          lightingOrigin: [0, 0, 0], cameraPosition: [phase * .1, 0, 5], ambientLight: true,
          ambientCube: Array.from({ length: 6 }, (_, side) => [.03 * (index + 1), .02 * (phase + 1), .01 * (side + 1)]) as any,
          localLights: [{ kind: "point", position: [index - 1, 2, 4], color: [.3 + index * .2, .6, .4 + phase * .1],
            direction: [0, 0, -1], attenuation: [1, 0, 0], range: 0, falloff: 1, theta: 0, phi: Math.PI }],
          localEnvironment: null, staticLightVertex: false, staticLightTexel: false,
        })
        actor.eye.irisU.value.set(.4, 0, 0, .2 * index + phase * .1)
        actor.eye.irisV.value.set(0, .4, 0, .1 * index)
        const texture = cubemaps[(index + phase) % 2]!
        bindModelEnvironment(actor.mesh, texture)
        actor.dedicated.environmentNode.value = texture
      }
      const before = await capture(false), after = await capture(true)
      const mismatches = (a: Uint8Array, b: Uint8Array) => a.reduce((sum, value, i) => sum + Number(value !== b[i]), 0)
      const result = { phase, colorMismatches: mismatches(before.color, after.color), depthMismatches: mismatches(before.depth, after.depth),
        colorBytes: after.color.length, depthBytes: after.depth.length, graphs: graphs.size, visibleActors: actors.filter(actor => actor.mesh.parent === scene).length }
      const hash = async (bytes: Uint8Array) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))).map(value => value.toString(16).padStart(2, "0")).join("")
      return { ...result, samples: [220, 320, 420].map(x => ({ x, before: [...before.color.slice((240 * width + x) * 4, (240 * width + x) * 4 + 4)], after: [...after.color.slice((240 * width + x) * 4, (240 * width + x) * 4 + 4)] })), colorSha256: await hash(after.color), depthSha256: await hash(after.depth), beforePixels: before.pixels, afterPixels: after.pixels }
    },
    dispose() {
      for (const actor of actors) { actor.skeleton.dispose(); actor.original.dispose(); actor.shared.dispose() }
      for (const texture of cubemaps) texture.dispose()
      iris.dispose(); geometry.dispose(); depthQuad.geometry.dispose(); depthMaterial.dispose(); target.dispose(); depthTarget.dispose(); renderer.dispose()
    },
  }
}
