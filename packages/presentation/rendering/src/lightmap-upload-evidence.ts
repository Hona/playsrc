import * as THREE from "three/webgpu"
import { installSkinningEvidence } from "./skinning-evidence"
import { withImmediateGpuSubmissions } from "./webgpu-submission-batch"
import type { WorldLightmapTextures } from "./world-lightmap-textures"

// Explicit local headed correctness import only. The profiler registers its
// current scene through a local module route; production has no registry/hook.
// Compare retained GPU samples with a canonical unconditional reupload, then
// compare full authored color/normal/depth draws at the same camera/pose/time.
export function installLightmapUploadEvidence() {
  let textures: WorldLightmapTextures | undefined
  let renderer: THREE.WebGPURenderer | undefined
  let registeredIds: number[] = []
  let reading: Promise<unknown>[] = []
  let referenceUploadBytes = 0
  const digest = async (bytes: Uint8Array) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as BufferSource)), value => value.toString(16).padStart(2, "0")).join("")
  const readPlane = async (renderer: THREE.WebGPURenderer, texture: THREE.DataTexture) => {
    const backend = renderer.backend as any, device = backend.device as GPUDevice
    const image = texture.image as { data: Float32Array; width: number; height: number }
    const row = image.width * 16, stride = Math.ceil(row / 256) * 256
    const buffer = device.createBuffer({ size: stride * image.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
    try {
      const encoder = device.createCommandEncoder()
      encoder.copyTextureToBuffer({ texture: backend.get(texture).texture }, { buffer, bytesPerRow: stride, rowsPerImage: image.height }, [image.width, image.height])
      // Copy BEFORE the reference upload, not after it. The immediate owner
      // flushes prior real draws without changing their queue-timeline order.
      withImmediateGpuSubmissions(device.queue, () => device.queue.submit([encoder.finish()]))
      await buffer.mapAsync(GPUMapMode.READ)
      const gpu = new Uint8Array(buffer.getMappedRange()), canonical = new Uint8Array(image.data.buffer, image.data.byteOffset, image.data.byteLength)
      const packed = new Uint8Array(canonical.length)
      for (let y = 0; y < image.height; y++) packed.set(gpu.subarray(y * stride, y * stride + row), y * row)
      let mismatches = 0
      for (let offset = 0; offset < packed.length; offset++) if (packed[offset] !== canonical[offset]) mismatches++
      return { textureId: texture.id, bytes: canonical.length, mismatches, sha256: await digest(packed), canonicalSha256: await digest(canonical) }
    } finally { buffer.destroy() }
  }
  const pixels = installSkinningEvidence(draw => {
    if (!textures || !renderer) throw new Error("No registered lightmap")
    const backend = renderer.backend as any, update = backend.updateTexture
    backend.updateTexture = function (texture: THREE.DataTexture, ...args: any[]) {
      const result = update.call(this, texture, ...args)
      if (textures!.includes(texture)) referenceUploadBytes += texture.image.data.byteLength
      return result
    }
    for (const texture of textures) texture!.needsUpdate = true
    try { draw() } finally { backend.updateTexture = update }
  }, (scene, camera, owner) => {
    let world = false
    scene.traverseVisible(object => { if ((object as any).isBundleGroup && object.children.length) world = true })
    if (!world || !camera.layers.isEnabled(0)) return false
    if (!textures) throw new Error("No registered lightmap")
    renderer = owner
    reading = textures.map(texture => readPlane(owner, texture!))
    for (const promise of reading) void promise.catch(() => {})
    return true
  })
  return {
    register(values: WorldLightmapTextures) { textures = values; registeredIds = values.map(value => value!.id) },
    async capture() {
      const parity = await pixels.capture("world-lightmap", "*", true) as object
      if (!reading.length) throw new Error("No authored world draw")
      const planes = await Promise.all(reading)
      return { performanceSample: false, oracle: "unconditional canonical lightmap upload", registeredIds, referenceUploadBytes, planes, parity }
    },
    dispose() { pixels.dispose(); textures = undefined; renderer = undefined; reading = []; registeredIds = [] },
  }
}
