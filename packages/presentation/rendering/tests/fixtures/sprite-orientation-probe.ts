import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { writeParticleQuad, writeParticleQuadIndices } from "../../src/particle-geometry"
import { spriteCardNodes } from "../../src/sprite-card"

/** Visible integration fixture using the production quad writer and SpriteCard nodes. */
export async function createSpriteOrientationProbe() {
  const renderer = new THREE.WebGPURenderer({ antialias: false, alpha: false })
  renderer.setPixelRatio(1)
  renderer.setSize(640, 480)
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace
  renderer.toneMapping = THREE.NoToneMapping
  document.body.append(renderer.domElement)
  await renderer.init()
  const scene = new THREE.Scene()
  scene.background = new THREE.Color(0)
  const camera = new THREE.PerspectiveCamera(90, 640 / 480, 0.01, 100)
  camera.up.set(0, 0, 1)
  camera.lookAt(1, 0, 0)
  const geometry = new THREE.BufferGeometry()
  const positions = new Float32Array(12), centers = new Float32Array(16), indices = new Uint16Array(6)
  writeParticleQuadIndices(indices)
  geometry.setIndex(new THREE.BufferAttribute(indices, 1))
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute("particleCenterOrientation", new THREE.BufferAttribute(centers, 4))
  const state = { depthBlend: false, blendFrames: false, addSelf: 0, overbright: 1, depthBlendScale: 1,
    minimumSize: 0, startFadeSize: 100, endFadeSize: 200, maximumSize: 100, maximumDistance: 1000, farFadeInterval: 100 }
  const nodes = spriteCardNodes(state, TSL.vec4(1, 0, 0, 1), TSL.vec4(1), TSL.vec4(1))
  const material = new THREE.MeshBasicNodeMaterial()
  material.positionNode = nodes.position
  material.colorNode = nodes.color
  material.transparent = true
  material.blending = THREE.CustomBlending
  material.blendSrc = THREE.SrcAlphaFactor
  material.blendDst = THREE.OneFactor
  material.depthTest = true
  material.depthWrite = false
  const mesh = new THREE.Mesh(geometry, material)
  mesh.frustumCulled = false
  scene.add(mesh)
  return {
    async draw(distance: number, yawRadians: number) {
      writeParticleQuad({ primitive: "sprite", position: [distance, 0, 0], trailEndPosition: [distance, 0, 0],
        radius: 1, trailWidth: 0, rollRadians: 0, yawRadians, orientationType: 1 },
      { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0 }, positions, 0)
      for (let vertex = 0; vertex < 4; vertex++) centers.set([distance, 0, 0, 1], vertex * 4)
      geometry.getAttribute("position").needsUpdate = true
      geometry.getAttribute("particleCenterOrientation").needsUpdate = true
      renderer.render(scene, camera)
      await renderer.backend.device.queue.onSubmittedWorkDone()
      // Read back real visible canvas pixels, never a synthesized image.
      const copy = document.createElement("canvas")
      copy.width = 640; copy.height = 480
      const context = copy.getContext("2d")!
      context.drawImage(renderer.domElement, 0, 0)
      const center = [...context.getImageData(320, 240, 1, 1).data]
      return { distance, yawRadians, center, pixels: copy.toDataURL("image/png") }
    },
    destroy() { geometry.dispose(); material.dispose(); renderer.dispose(); renderer.domElement.remove() },
  }
}
