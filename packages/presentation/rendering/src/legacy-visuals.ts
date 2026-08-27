import * as THREE from "three/webgpu"
import { SourcePixelVisibility } from "./pixel-visibility"

export type LegacyVisualFrame = Readonly<{
  proxies: readonly Readonly<{ source: number; clipFraction: number; vertices: Float32Array }>[]
  quads: readonly Readonly<{ source: number; material: number; frame:number;hdrScale:number;positions: Float32Array; color: Float32Array;uv:Float32Array }>[]
}>
export type PixelVisibilityFeedback = Readonly<{ source: number; submission: number; visible: number; possible: number; clipFraction: number }>

export class PixelFeedbackLedger {
  readonly #latest = new Map<number, PixelVisibilityFeedback>()
  readonly #completed = new Map<number, PixelVisibilityFeedback>()
  readonly #consumed = new Map<number, number>()
  submit(value: PixelVisibilityFeedback): void { this.#latest.set(value.source,value) }
  complete(value: PixelVisibilityFeedback): void { this.#completed.set(value.source,value); this.#latest.set(value.source,value) }
  consume(): readonly PixelVisibilityFeedback[] {
    return [...this.#latest.values()].map(pending=>{
      const completed=this.#completed.get(pending.source)
      if(completed && completed.submission>(this.#consumed.get(pending.source)??0)) {
        this.#consumed.set(pending.source,completed.submission)
        return completed
      }
      return pending
    })
  }
  snapshot(): readonly PixelVisibilityFeedback[] { return [...this.#latest.values()].map(value=>this.#completed.get(value.source)??value) }
  clear(): void { this.#latest.clear();this.#completed.clear();this.#consumed.clear() }
}

type Hook = { original: (...args: any[]) => any; owners: number; depth: GPUTexture | null }
const HOOKS = new WeakMap<object, Hook>()

/** Per-map, per-view legacy overlay pool. Native code owns all geometry/fading;
 * this pool submits exactly the prepared quads and feeds actual raster counts
 * back to that owner. Unpresented prepared frames never acknowledge a query.
 */
export class LegacyVisuals {
  readonly group = new THREE.Scene()
  readonly #backend: any
  readonly #counter: SourcePixelVisibility
  readonly #materials: readonly THREE.Material[]
  readonly #meshes: THREE.Mesh[] = []
  readonly #feedback = new PixelFeedbackLedger()
  #frame: LegacyVisualFrame | null = null
  #submission = 0
  #failure: unknown
  #disposed = false
  #samples = 0
  #afterSubmit: (() => void) | null = null

  constructor(backend: any, materials: readonly THREE.Material[]) {
    this.#backend = backend; this.#materials = materials
    this.#counter = new SourcePixelVisibility(backend.device)
    let hook = HOOKS.get(backend)
    if (!hook) {
      hook = { original: backend.finishRender, owners: 0, depth: null }
      const retained = hook
      backend.finishRender = function (context: any): any {
        if (context.depth) retained.depth = context.renderTarget
          ? backend.get(context.depthTexture).texture : backend.textureUtils.getDepthBuffer(context.depth, context.stencil)
        return retained.original.call(this, context)
      }
      HOOKS.set(backend, hook)
    }
    hook.owners++
  }

  async prepare(): Promise<void> { await Promise.all([this.#counter.prepare(1), this.#counter.prepare(4)]) }
  feedback(): readonly PixelVisibilityFeedback[] { return this.#feedback.consume() }
  evidence() { return { samples:this.#samples,quads:this.#frame?.quads??[],queries:this.#feedback.snapshot() } }
  finishFrame(): void { const read=this.#afterSubmit; this.#afterSubmit=null; read?.() }

  update(frame: LegacyVisualFrame): void {
    this.#frame = frame
    for (let index = 0; index < Math.max(frame.quads.length, this.#meshes.length); index++) {
      const quad = frame.quads[index]
      let mesh = this.#meshes[index]
      if (!quad) { if (mesh) mesh.visible = false; continue }
      const material = this.#materials[quad.material]
      if (!material) throw new Error("Legacy visual material is not admitted")
      if (!mesh) {
        const geometry = new THREE.BufferGeometry()
        geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(12),3).setUsage(THREE.DynamicDrawUsage))
        geometry.setAttribute("legacyColor", new THREE.BufferAttribute(new Float32Array(16),4).setUsage(THREE.DynamicDrawUsage))
        geometry.setAttribute("legacyHdr",new THREE.BufferAttribute(new Float32Array(4),1).setUsage(THREE.DynamicDrawUsage))
        geometry.setAttribute("uv",new THREE.BufferAttribute(new Float32Array(8),2).setUsage(THREE.DynamicDrawUsage))
        geometry.setIndex([0,2,1,0,3,2])
        mesh = new THREE.Mesh(geometry,material); mesh.frustumCulled = false
        this.#meshes.push(mesh); this.group.add(mesh)
      }
      mesh.material = material; mesh.visible = true
      // Source glow overlays retain their activation-list order, not distance sorting.
      mesh.renderOrder = index
      const position = mesh.geometry.getAttribute("position") as THREE.BufferAttribute
      const color = mesh.geometry.getAttribute("legacyColor") as THREE.BufferAttribute
      const uv=mesh.geometry.getAttribute("uv") as THREE.BufferAttribute,hdr=mesh.geometry.getAttribute("legacyHdr") as THREE.BufferAttribute
      ;(position.array as Float32Array).set(quad.positions)
      for (let vertex=0;vertex<4;vertex++) (color.array as Float32Array).set(quad.color,vertex*4)
      ;(uv.array as Float32Array).set(quad.uv);(hdr.array as Float32Array).fill(quad.hdrScale)
      position.needsUpdate = true; color.needsUpdate = true
      uv.needsUpdate=true;hdr.needsUpdate=true
    }
  }

  /** Called immediately after this view's scene draw, before depth reset. */
  capture(): void {
    if (this.#failure) throw this.#failure
    if (this.#disposed || !this.#frame?.proxies.length) return
    const depth = HOOKS.get(this.#backend)?.depth
    if (!depth) throw new Error("Legacy visual scene depth is unavailable")
    this.#samples=depth.sampleCount
    const proxies = this.#frame.proxies
    const vertices = new Float32Array(proxies.length * 20)
    proxies.forEach((proxy,index)=>vertices.set(proxy.vertices,index*20))
    const encoder = this.#backend.device.createCommandEncoder({label:"Legacy visual visibility"}) as GPUCommandEncoder
    const read = this.#counter.issue(encoder,depth,vertices)
    if (!read) return
    this.#backend.device.queue.submit([encoder.finish()])
    const submission = ++this.#submission
    for (const proxy of proxies) this.#feedback.submit({source:proxy.source,submission,visible:-1,possible:-1,clipFraction:proxy.clipFraction})
    // The gameplay frame batches queue.submit. Mapping before its final flush
    // can map an untouched zero buffer rather than wait for the encoded copy.
    this.#afterSubmit=()=>{
      void read().then(counts => {
        if (this.#disposed) return
        proxies.forEach((proxy,index)=>this.#feedback.complete({source:proxy.source,submission,visible:counts[index*2]!,possible:counts[index*2+1]!,clipFraction:proxy.clipFraction}))
      }).catch(error=>{if(!this.#disposed)this.#failure=error})
    }
  }

  dispose(): void {
    this.#disposed = true; this.#afterSubmit=null; this.#counter.dispose(); this.#feedback.clear()
    for (const mesh of this.#meshes) mesh.geometry.dispose()
    this.group.clear()
    const hook = HOOKS.get(this.#backend)!
    if (--hook.owners===0) { this.#backend.finishRender=hook.original; HOOKS.delete(this.#backend) }
  }
}
