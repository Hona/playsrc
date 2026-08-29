import type * as THREE from "three/webgpu"

/** Three clears its texture manager before disposing conversion framebuffers.
 * With our externally owned GPUDevice that loses the attachment destroy path.
 * Retire only these renderer-owned intermediate targets while their texture
 * records/listeners still exist, then let normal backend disposal finish. */
export function disposeWebGpuBackend(renderer: THREE.WebGPURenderer): void {
  const owner = renderer as unknown as { _frameBufferTargets: Map<object, THREE.RenderTarget> }
  for (const target of owner._frameBufferTargets.values()) target.dispose()
  renderer.dispose()
}
