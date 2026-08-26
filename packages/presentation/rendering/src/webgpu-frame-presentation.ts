export type FramePresentationBackend<Target extends object> = {
  readonly needsFrameBufferTarget: boolean
  getRenderTarget(): Target | null
  setRenderTarget(target: Target | null): void
  _getFrameBufferTarget(): Target
  _renderOutput(target: Target): void
}

export class WebGpuFramePresentation<Target extends object> {
  readonly #backend: FramePresentationBackend<Target>
  #target: Target | undefined

  constructor(backend: FramePresentationBackend<Target>) {
    if (typeof backend._getFrameBufferTarget !== "function" || typeof backend._renderOutput !== "function") {
      throw new Error("WebGPU framebuffer presentation backend is unavailable")
    }
    this.#backend = backend
  }

  get active(): boolean { return this.#target !== undefined }
  get target(): Target | null { return this.#target ?? null }

  begin(): void {
    if (this.#target) throw new Error("WebGPU gameplay frame is already active")
    if (!this.#backend.needsFrameBufferTarget) return
    if (this.#backend.getRenderTarget() !== null) throw new Error("WebGPU gameplay frame has an unexpected render target")
    const target = this.#backend._getFrameBufferTarget()
    this.#backend.setRenderTarget(target)
    this.#target = target
  }

  finish(): void {
    const target = this.#target
    if (!target) return
    this.#target = undefined
    this.#backend.setRenderTarget(null)
    this.#backend._renderOutput(target)
  }

  abandon(): void {
    if (!this.#target) return
    this.#target = undefined
    this.#backend.setRenderTarget(null)
  }
}
