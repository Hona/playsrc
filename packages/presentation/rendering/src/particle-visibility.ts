import * as THREE from "three/webgpu"
import { SourcePixelVisibility } from "./pixel-visibility"

export type ParticleVisibilityProxy = Readonly<{ identity: bigint; vertices: Float32Array; clipFraction: number }>
export type ParticleVisibilitySample = Readonly<{ identity: bigint; visiblePixels: number; possiblePixels: number; clipFraction: number }>
type Counter = Pick<SourcePixelVisibility, "pending" | "bufferBytes" | "prepare" | "issue" | "dispose">
type View = { camera: THREE.Camera; target: THREE.RenderTarget | null; sky: boolean }
const EMPTY_SAMPLES: readonly ParticleVisibilitySample[] = Object.freeze([])

/** Native proxies and fading, real MSAA sample counts, and one frame submission. */
export class ParticleVisibilityQueries {
  readonly #factory: (device: GPUDevice) => Counter
  readonly #proxies = [new Map<bigint, ParticleVisibilityProxy>(), new Map<bigint, ParticleVisibilityProxy>()] as const
  readonly #samples = new Map<bigint, ParticleVisibilitySample>()
  readonly #last = new Map<bigint, ParticleVisibilitySample>()
  readonly #reads: (() => void)[] = []
  #device?: GPUDevice
  #counters: Counter[] = []
  #view?: View
  #generation = 0
  #failure: unknown
  #restore?: () => void
  #issued = 0
  #readbackBytes = 0
  #vertexBytes = 0
  #maxPossiblePixels = 0

  constructor(factory: (device: GPUDevice) => Counter = device => new SourcePixelVisibility(device)) { this.#factory = factory }

  attach(renderer: THREE.WebGPURenderer): void {
    this.#restore?.()
    this.reset()
    const backend = renderer.backend as any, owner = this
    if (!backend.isWebGPUBackend) throw new Error("Particle visibility requires the WebGPU depth backend")
    this.#device = backend.device
    this.#counters = [this.#factory(backend.device), this.#factory(backend.device)]
    const finish = backend.finishRender
    backend.finishRender = function(context: any) {
      const view = owner.#view
      if (view && (context.renderTarget ?? null) === view.target) owner.#capture(this, context, view)
      return finish.call(this, context)
    }
    this.#restore = () => { backend.finishRender = finish }
  }

  async prepare(): Promise<void> {
    if (!this.#device) throw new Error("Particle visibility device is unavailable")
    if (!this.#counters.length) this.#counters = [this.#factory(this.#device), this.#factory(this.#device)]
    await Promise.all(this.#counters.flatMap(counter => [1, 4].flatMap(samples =>
      (["rgba8unorm", "bgra8unorm", "rgba16float"] as GPUTextureFormat[]).map(format => counter.prepare(samples, format)))))
  }

  beginPass(identity: string, renderer: THREE.WebGPURenderer, camera: THREE.Camera): void {
    const sky = identity === "sky3d"
    this.#view = (identity === "main" || sky) && this.#proxies[sky ? 1 : 0].size ? { camera, target: renderer.getRenderTarget(), sky } : undefined
  }
  endPass(): void { this.#view = undefined }

  #capture(backend: any, context: any, view: View): void {
    const proxies = this.#proxies[view.sky ? 1 : 0], counter = this.#counters[view.sky ? 1 : 0]
    if (!proxies.size || !counter || counter.pending) return
    const state = backend.get(context), attachment = state.descriptor?.colorAttachments?.[0]
    if (!context.depth || !state.currentPass || !attachment) return
    if (context.occlusionQueryCount) throw new Error("Particle visibility cannot split an active hardware occlusion query")
    const depth: GPUTexture = context.renderTarget ? backend.get(context.depthTexture).texture : backend.textureUtils.getDepthBuffer(context.depth, context.stencil)
    const color: GPUTexture = context.renderTarget ? backend.get(context.textures[0]).texture : backend.context.getCurrentTexture()
    const selected = [...proxies.values()].filter(proxy => proxy.clipFraction > 0)
    if (!selected.length) return
    const matrix = new THREE.Matrix4().multiplyMatrices(view.camera.projectionMatrix, view.camera.matrixWorldInverse)
    const point = new THREE.Vector4(), vertices = new Float32Array(selected.length * 20)
    for (let index = 0; index < selected.length; index++) for (let vertex = 0; vertex < 5; vertex++) {
      const source = selected[index]!.vertices, at = vertex * 3
      point.set(source[at]!, source[at + 1]!, source[at + 2]!, 1).applyMatrix4(matrix).toArray(vertices, index * 20 + vertex * 4)
    }
    state.currentPass.end()
    const read = counter.issue(state.encoder, depth, vertices, color.format, attachment)
    for (const color of state.descriptor.colorAttachments) color.loadOp = "load"
    state.descriptor.depthStencilAttachment.depthLoadOp = "load"
    if (context.stencil) state.descriptor.depthStencilAttachment.stencilLoadOp = "load"
    const timestamps = state.descriptor.timestampWrites
    if (timestamps) state.descriptor.timestampWrites = { querySet: timestamps.querySet, endOfPassWriteIndex: timestamps.endOfPassWriteIndex }
    state.currentPass = state.encoder.beginRenderPass(state.descriptor)
    state.currentSets = { attributes: {}, bindingGroups: [], pipeline: null, index: null }
    if (context.viewport) backend.updateViewport(context)
    if (context.scissor) backend.updateScissor(context)
    if (!read) return
    const generation = this.#generation
    this.#issued += selected.length; this.#vertexBytes += selected.length * 12 * 16
    this.#reads.push(() => { void read().then(values => {
      if (generation !== this.#generation) return
      this.#readbackBytes += values.byteLength
      for (let index = 0; index < selected.length; index++) {
        this.#maxPossiblePixels = Math.max(this.#maxPossiblePixels, values[index * 2 + 1]!)
        const proxy = selected[index]!
        if (!proxies.has(proxy.identity)) continue
        const sample = Object.freeze({ identity: proxy.identity, visiblePixels: values[index * 2]!, possiblePixels: values[index * 2 + 1]!, clipFraction: proxies.get(proxy.identity)!.clipFraction === 0 ? 0 : proxy.clipFraction })
        this.#samples.set(proxy.identity, sample); this.#last.set(proxy.identity, sample)
      }
    }).catch(error => { if (generation === this.#generation) this.#failure = error }) })
  }

  /** Called only after WebGpuFramePresentation has submitted the frame. */
  flushReads(): void { for (const read of this.#reads.splice(0)) read() }

  takeSamples(): readonly ParticleVisibilitySample[] {
    if (this.#failure) throw this.#failure
    if (!this.#samples.size) return EMPTY_SAMPLES
    const samples = [...this.#samples.values()].sort((a, b) => a.identity < b.identity ? -1 : a.identity > b.identity ? 1 : 0)
    this.#samples.clear()
    return samples
  }

  stage(items: readonly Readonly<{ visibility?: ParticleVisibilityProxy; sky?: boolean }>[]): void {
    this.#proxies[0].clear(); this.#proxies[1].clear()
    for (const item of items) if (item.visibility) {
      const proxy = item.visibility
      this.#proxies[item.sky ? 1 : 0].set(proxy.identity, proxy)
      if (proxy.clipFraction === 0) this.#samples.set(proxy.identity, { identity: proxy.identity, visiblePixels: -1, possiblePixels: -1, clipFraction: 0 })
    }
    for (const identity of this.#last.keys()) if (!this.#proxies[0].has(identity) && !this.#proxies[1].has(identity)) { this.#last.delete(identity); this.#samples.delete(identity) }
  }

  evidence() { return { issued: this.#issued, vertexBytes: this.#vertexBytes, readbackBytes: this.#readbackBytes, maxPossiblePixels: this.#maxPossiblePixels,
    bufferBytes: this.#counters.reduce((sum, counter) => sum + counter.bufferBytes, 0), world: this.#proxies[0].size, sky: this.#proxies[1].size,
    samples: [...this.#last.values()].map(sample => ({ ...sample, identity: sample.identity.toString() })) } }

  reset(): void {
    this.#generation++; this.#view = undefined; this.#failure = undefined
    this.#issued = 0; this.#readbackBytes = 0; this.#vertexBytes = 0; this.#maxPossiblePixels = 0
    this.#reads.length = 0; this.#samples.clear(); this.#last.clear(); this.#proxies[0].clear(); this.#proxies[1].clear()
    for (const counter of this.#counters) counter.dispose()
    this.#counters = []
  }

  dispose(): void { this.reset(); this.#restore?.(); this.#restore = undefined; this.#device = undefined }
}
