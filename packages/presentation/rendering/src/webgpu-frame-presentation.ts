import { WebGpuSubmissionBatch, type BatchedGpuQueue } from "./webgpu-submission-batch"

export type FramePresentationBackend<Target extends object> = {
  readonly needsFrameBufferTarget: boolean
  getRenderTarget(): Target | null
  setRenderTarget(target: Target | null): void
  _getFrameBufferTarget(): Target
  _renderOutput(target: Target): void
}

export class WebGpuFramePresentation<Target extends object> {
  readonly #backend: FramePresentationBackend<Target>
  readonly #submissions?: WebGpuSubmissionBatch
  #target: Target | undefined
  #begun = false

  constructor(backend: FramePresentationBackend<Target>, queue?: BatchedGpuQueue) {
    if (typeof backend._getFrameBufferTarget !== "function" || typeof backend._renderOutput !== "function") {
      throw new Error("WebGPU framebuffer presentation backend is unavailable")
    }
    this.#backend = backend
    if (queue) this.#submissions = new WebGpuSubmissionBatch(queue)
  }

  get active(): boolean { return this.#target !== undefined }
  get begun(): boolean { return this.#begun }
  get target(): Target | null { return this.#target ?? null }

  begin(): void {
    if (this.#begun) throw new Error("WebGPU gameplay frame is already active")
    this.#begun = true
    this.#submissions?.begin()
    if (!this.#backend.needsFrameBufferTarget) return
    if (this.#backend.getRenderTarget() !== null) {
      this.#begun = false
      this.#submissions?.finish()
      throw new Error("WebGPU gameplay frame has an unexpected render target")
    }
    const target = this.#backend._getFrameBufferTarget()
    this.#backend.setRenderTarget(target)
    this.#target = target
  }

  finish(): void {
    const target = this.#target
    if (!this.#begun) return
    try {
      if (target) {
        this.#target = undefined
        this.#backend.setRenderTarget(null)
        this.#backend._renderOutput(target)
      }
    } finally {
      this.#begun = false
      this.#submissions?.finish()
    }
  }

  abandon(): void {
    if (!this.#begun) return
    const target = this.#target
    this.#target = undefined
    this.#begun = false
    if (target) this.#backend.setRenderTarget(null)
    this.#submissions?.finish()
  }

  dispose(): void { this.abandon(); this.#submissions?.dispose() }
}
