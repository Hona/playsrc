import * as THREE from "three/webgpu"
import { resizeSampledRenderTargets } from "../src/render-target-resize"

// Hardware correctness of the exact mutable-target/consumer boundary. This
// does not claim to be a gameplay screenshot or a performance measurement.
const api = globalThis as any
api.prepare = async () => {
  const canvas = document.querySelector("canvas")!, renderer = new THREE.WebGPURenderer({ canvas, antialias: false })
  renderer.setSize(1280, 720, false); await renderer.init()
  const device = (renderer.backend as any).device as GPUDevice, errors: string[] = [], allocations: any[] = []
  device.addEventListener("uncapturederror", event => errors.push(String((event as GPUUncapturedErrorEvent).error)))
  const create = device.createTexture.bind(device), nativeRecords = new WeakMap<GPUTexture, any>()
  device.createTexture = descriptor => {
    const value = create(descriptor)
    const record = { label: descriptor.label ?? "", format: descriptor.format, samples: descriptor.sampleCount ?? 1, destroyed: false, waterDepth: false }
    allocations.push(record); nativeRecords.set(value, record)
    const destroy = value.destroy.bind(value); value.destroy = () => { record.destroyed = true; destroy() }
    return value
  }
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, .1, 10); camera.position.z = 2
  const scene = new THREE.Scene(); scene.background = new THREE.Color(0x0000ff)
  const nearGeometry = new THREE.PlaneGeometry(.6, .6), farGeometry = new THREE.PlaneGeometry(1.3, 1.3)
  const nearMaterial = new THREE.MeshBasicNodeMaterial({ color: 0x00ff00 }), farMaterial = new THREE.MeshBasicNodeMaterial({ color: 0xff0000 })
  nearMaterial.toneMapped = farMaterial.toneMapped = false
  const near = new THREE.Mesh(nearGeometry, nearMaterial), far = new THREE.Mesh(farGeometry, farMaterial)
  near.position.set(.1, .1, .2); near.renderOrder = 1; far.position.z = 0; far.renderOrder = 2; scene.add(near, far)
  const output = new THREE.RenderTarget(1280, 720, { depthBuffer: true })
  const consumerGeometry = new THREE.PlaneGeometry(2, 2), consumerScene = new THREE.Scene()
  const digest = async (bytes: Uint8Array) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as BufferSource)), value => value.toString(16).padStart(2, "0")).join("")
  const records: any[] = []
  let displayed: THREE.Mesh | undefined, frame = 0
  const draw = () => { if (displayed) { renderer.setRenderTarget(null); renderer.render(consumerScene, camera) }; frame = requestAnimationFrame(draw) }
  draw()
  api.capture = async (mode: "reference" | "candidate") => {
    if (displayed) (displayed.material as THREE.Material).dispose()
    displayed = undefined; consumerScene.clear()
    const began = allocations.length, phases = []
    for (let generation = 1; generation <= 2; generation++) {
      const target = new THREE.RenderTarget(1280, 720, { depthBuffer: true })
      target.texture.name = `playsrc-water-refraction-${mode}-${generation}`; target.texture.colorSpace = THREE.NoColorSpace
      const material = new THREE.MeshBasicNodeMaterial({ map: target.texture }); material.toneMapped = false
      const mesh = new THREE.Mesh(consumerGeometry, material); consumerScene.add(mesh)
      renderer.setRenderTarget(null)
      if (mode === "candidate") renderer.initRenderTarget(target)
      await renderer.compileAsync(consumerScene, camera)
      if (mode === "reference") renderer.initRenderTarget(target)
      for (const resized of [false, true]) {
        if (resized) resizeSampledRenderTargets([target], 1296, 736, value => renderer.initRenderTarget(value))
        const depth = (renderer as any)._textures.get(target).depthTexture
        const depthNative = (renderer.backend as any).get(depth).texture as GPUTexture
        const depthRecord = nativeRecords.get(depthNative)
        if (!depthRecord || !depthRecord.format.startsWith("depth") || depthNative.width !== target.width || depthNative.height !== target.height) throw new Error("Water target depth owner differs")
        depthRecord.waterDepth = true
        renderer.setRenderTarget(target); renderer.render(scene, camera)
        renderer.setRenderTarget(output); renderer.render(consumerScene, camera)
        const data = await renderer.readRenderTargetPixelsAsync(output, 0, 0, 1280, 720) as Uint8Array
        let green = 0, red = 0, blue = 0
        for (let pixel = 0; pixel < data.length; pixel += 4) {
          if (data[pixel + 1]! > 240 && data[pixel]! < 8 && data[pixel + 2]! < 8) green++
          if (data[pixel]! > 240 && data[pixel + 1]! < 8 && data[pixel + 2]! < 8) red++
          if (data[pixel + 2]! > 240 && data[pixel]! < 8 && data[pixel + 1]! < 8) blue++
        }
        if (Math.min(green, red, blue) < 1000) throw new Error("Actual depth-tested target/consumer pixels are missing")
        phases.push({ generation, resized, width: target.width, height: target.height, sha256: await digest(data), green, red, blue })
      }
      // Copy the final consumer frame to the visible canvas before retiring its
      // mutable source. The displayed result itself has an independent target.
      consumerScene.remove(mesh); material.dispose(); target.dispose()
    }
    const material = new THREE.MeshBasicNodeMaterial({ map: output.texture }); material.toneMapped = false
    displayed = new THREE.Mesh(consumerGeometry, material); consumerScene.add(displayed)
    const owned = allocations.slice(began).filter(value => value.label.startsWith("playsrc-water-") || value.waterDepth)
    await device.queue.onSubmittedWorkDone()
    await new Promise(requestAnimationFrame)
    const record = { mode, phases, allocations: owned, primaryAllocations: owned.filter(value => value.format === "rgba8unorm" && value.samples === 1).length, depthAllocations: owned.filter(value => value.waterDepth).length, terminalLive: owned.filter(value => !value.destroyed).length, errors: [...errors], performanceSample: false }
    records.push(record); return record
  }
  api.finish = async () => {
    cancelAnimationFrame(frame)
    if (displayed) (displayed.material as THREE.Material).dispose()
    consumerScene.clear(); consumerGeometry.dispose(); nearGeometry.dispose(); farGeometry.dispose(); nearMaterial.dispose(); farMaterial.dispose(); output.dispose()
    await device.queue.onSubmittedWorkDone(); renderer.dispose()
    return { records: records.length, errors, terminalLiveWater: allocations.filter(value => (value.label.startsWith("playsrc-water-") || value.waterDepth) && !value.destroyed).length }
  }
}
