import * as THREE from "three/webgpu"
import * as ownerModule from "../src/index"
import { installParticleAliasEvidence } from "../src/particle-alias-evidence"
import { WebGpuSubmissionBatch } from "../src/webgpu-submission-batch"

// Test bundle exposes the actual private owner; normal application code never
// imports this entry or receives a test-only owner interface.
const api = globalThis as any
api.prepare = async () => {
  const input = await (await fetch("/inputs.json")).json()
  const views = input.views.map((view: any) => {
    if (!["Uint8Array", "Uint16Array", "Uint32Array", "Float32Array"].includes(view.type)) throw new Error("Unexpected canonical view type")
    const bytes = Uint8Array.from(atob(view.data), value => value.charCodeAt(0))
    return new api[view.type](bytes.buffer)
  })
  const data = JSON.parse(input.encoded, (_key: string, value: any) => value && typeof value === "object" && "$view" in value ? views[value.$view] : value)
  const canvas = document.querySelector("canvas")!
  const renderer = new THREE.WebGPURenderer({ canvas, antialias: false }); renderer.setSize(1280, 720, false); await renderer.init()
  const device = (renderer.backend as any).device as GPUDevice
  const submissions = new WebGpuSubmissionBatch(device.queue)
  if (!device.features.has("texture-compression-bc")) throw new Error("Configured compressed formats unavailable")
  const errors: string[] = []
  device.addEventListener("uncapturederror", event => errors.push(String((event as GPUUncapturedErrorEvent).error)))
  const owned: { label: string; destroyed: boolean }[] = [], create = device.createTexture.bind(device)
  device.createTexture = descriptor => {
    const texture = create(descriptor)
    if (descriptor.label?.startsWith("authored:")) {
      const record = { label: descriptor.label, destroyed: false }; owned.push(record)
      const destroy = texture.destroy.bind(texture); texture.destroy = () => { record.destroyed = true; destroy() }
    }
    return texture
  }
  const evidence = installParticleAliasEvidence(); api.__playsrcParticleAliasEvidence = evidence
  const owner = new (ownerModule as any).RendererOwner({ canvas, configuration: (ownerModule as any).SOURCE_LDR, sampleCount: 1,
    textureQuality: { mipOffset: 1, trilinear: false, anisotropy: 1 } })
  owner.offlineParticleScene(renderer, data.textures, data.states)
  await owner.prepareParticlePipelines({ position: [0, 0, 4], yawDegrees: 0, pitchDegrees: 0, verticalFovDegrees: 75, near: 1, far: 32768 })
  const scene = new THREE.Scene(), bundle = new THREE.BundleGroup(), geometry = new THREE.PlaneGeometry(1, 1), material = new THREE.MeshBasicNodeMaterial({ color: 0x101820 })
  bundle.add(new THREE.Mesh(geometry, material)); scene.add(bundle)
  const camera = new THREE.PerspectiveCamera(75, 1280 / 720, .1, 100); camera.position.z = 4
  let frame = 0
  const draw = () => { renderer.render(scene, camera); frame = requestAnimationFrame(draw) }; draw()
  api.capture = (phase: number) => evidence.capture(phase)
  api.finish = async () => {
    cancelAnimationFrame(frame); evidence.dispose(); owner.offlineDispose(); geometry.dispose(); material.dispose()
    await device.queue.onSubmittedWorkDone()
    const report = { performanceSample: false, errors, ownedCreated: owned.length, terminalLiveAuthored: owned.filter(value => !value.destroyed).length, graphSha256: input.manifest.graphSha256 }
    renderer.dispose(); submissions.dispose(); return report
  }
  return { logicalMaterials: data.textures.length, graphSha256: input.manifest.graphSha256 }
}
