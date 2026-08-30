import type * as THREE from "three/webgpu"

/** Three clears its texture manager before disposing conversion framebuffers.
 * With our externally owned GPUDevice that loses the attachment destroy path.
 * Retire only these renderer-owned intermediate targets while their texture
 * records/listeners still exist, then let normal backend disposal finish. */
export function disposeWebGpuBackend(renderer: THREE.WebGPURenderer): void {
  const owner = renderer as unknown as {
    _frameBufferTargets: Map<object, THREE.RenderTarget>
    getCanvasTarget(): { colorTexture: THREE.Texture; depthTexture: THREE.Texture }
    backend: { has(texture: THREE.Texture): boolean; destroyTexture(texture: THREE.Texture): void }
  }
  for (const target of owner._frameBufferTargets.values()) target.dispose()
  // The final canvas pass allocates these through WebGPUTextureUtils directly,
  // without the texture manager's disposal listeners. Do not acquire a canvas
  // image or create missing attachments while releasing an unused renderer.
  const canvas = owner.getCanvasTarget()
  for (const texture of [canvas.colorTexture, canvas.depthTexture]) {
    if (owner.backend.has(texture)) owner.backend.destroyTexture(texture)
  }
  renderer.dispose()
}
