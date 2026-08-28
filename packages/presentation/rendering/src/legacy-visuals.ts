import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { SourcePixelVisibility } from "./pixel-visibility"
import { createWorldClipGroup, prepareWorldViewPipelines } from "./world-pipeline-preparation"
import { sourceWaterFogFragment,type SourceViewFogUniforms,type SourceWaterFogUniforms } from "./source-water"

export type LegacyVisualProgram=Readonly<{srgb:boolean;vertexRgb:boolean;vertexAlpha:boolean;vertexGamma:boolean;gammaExposure:boolean;worldRenderable:boolean;modulation:readonly[number,number,number,number];cable:boolean}>
export type LegacyViewKind=0|1|2|3|4
export type LegacyVisualFrameSet=Readonly<Partial<Record<LegacyViewKind,LegacyVisualFrame>>>

export function legacyFog(color:any,mode:number,view:SourceViewFogUniforms,water:SourceWaterFogUniforms):any {
  if(mode===2)return color
  const fogColor=mode===1?TSL.vec3(0):view.color
  const factor=TSL.positionView.z.negate().sub(view.start).div(view.end.sub(view.start)).clamp(0,view.maximumDensity)
  const ranged=TSL.vec4(TSL.mix(color.rgb,fogColor,view.enabled.greaterThan(0).select(factor,0)),color.a)
  const submerged=sourceWaterFogFragment(color,mode===1?{...water,fogColor:TSL.uniform(new THREE.Vector3(),"vec3")}:water)
  const fogged=water.enabled.greaterThan(0).select(submerged,ranged)
  return TSL.attribute("legacyFog","float").greaterThan(0).select(fogged,color)
}

export type LegacyVisualFrame = Readonly<{
  proxies: readonly Readonly<{ source: number; clipFraction: number; vertices: Float32Array }>[]
  quads: readonly Readonly<{ source: number; material: number; frame:number;layer:0|1|2;hdrScale:number;origin:Float32Array;positions: Float32Array; color: Float32Array;uv:Float32Array }>[]
  meshes:readonly Readonly<{material:number;sources:Uint32Array;positions:Float32Array;uv:Float32Array;color:Uint8Array;indices:Uint32Array}>[]
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

function quadGeometry():THREE.BufferGeometry{
  const geometry=new THREE.BufferGeometry()
  for(const [name,size] of [["position",3],["legacyColor",4],["legacyHdr",1],["legacyFog",1],["uv",2]] as const)geometry.setAttribute(name,new THREE.BufferAttribute(new Float32Array(4*size),size).setUsage(THREE.DynamicDrawUsage))
  geometry.setIndex([0,2,1,0,3,2]);return geometry
}

/** Per-map, per-view legacy overlay pool. Native code owns all geometry/fading;
 * this pool submits exactly the prepared quads and feeds actual raster counts
 * back to that owner. Unpresented prepared frames never acknowledge a query.
 */
export class LegacyVisuals {
  readonly group = new THREE.Scene()
  readonly world = new THREE.Group()
  readonly noDepth = new THREE.Scene()
  readonly noDepthClip = new THREE.ClippingGroup()
  readonly #backend: any
  readonly #counter: SourcePixelVisibility
  readonly #materials: readonly (readonly THREE.Material[])[]
  readonly #meshes: THREE.Mesh[] = []
  readonly #ropeMeshes:THREE.Mesh[]=[]
  #ropeDraws=0
  readonly #feedback = new PixelFeedbackLedger()
  #frame: LegacyVisualFrame | null = null
  #submission = 0
  #failure: unknown
  #disposed = false
  #samples = 0
  #afterSubmit: (() => void) | null = null
  #queryTarget:GPUTexture|null=null

  constructor(backend: any, materials: readonly (readonly THREE.Material[])[]) {
    this.#backend = backend; this.#materials = materials
    this.#counter = new SourcePixelVisibility(backend.device)
    this.noDepth.add(this.noDepthClip)
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

  async prepare(): Promise<void> { await Promise.all([this.#counter.prepare(1,"r8unorm"), this.#counter.prepare(4,"r8unorm")]) }
  async prepareMaterials(renderer:THREE.WebGPURenderer,camera:THREE.Camera,world:THREE.Scene,plane=new THREE.Plane(),waterTargets:readonly THREE.RenderTarget[]=[]):Promise<void>{
    const geometry=quadGeometry(),rope=quadGeometry(),group=createWorldClipGroup(plane),ropeGroup=createWorldClipGroup()
    rope.setAttribute("legacyColor",new THREE.BufferAttribute(new Uint8Array(16),4,true).setUsage(THREE.DynamicDrawUsage))
    for(const material of this.#materials.flat()){
      const mesh=new THREE.Mesh(material.userData.sourceRope?rope:geometry,material);mesh.frustumCulled=false
      if(material.userData.sourceRope&&!ropeGroup.parent)group.add(ropeGroup)
      ;(material.userData.sourceRope?ropeGroup:group).add(mesh)
    }
    try{
      if(group.children.length){await prepareWorldViewPipelines(renderer,group,camera,world,group,waterTargets);await prepareWorldViewPipelines(renderer,group,camera,this.noDepth,group,waterTargets);group.enabled=false;await renderer.compileAsync(group,camera,this.group)}
    }finally{ropeGroup.clear();group.clear();geometry.dispose();rope.dispose()}
  }
  feedback(): readonly PixelVisibilityFeedback[] { return this.#feedback.consume() }
  evidence() { return { samples:this.#samples,quads:this.#frame?.quads??[],meshes:this.#frame?.meshes??[],ropeDraws:this.#ropeDraws,ropeObjects:this.#ropeMeshes.map(mesh=>({visible:mesh.visible,parentVisible:mesh.parent?.visible,drawCount:mesh.geometry.drawRange.count,material:(mesh.material as THREE.Material).name,depthTest:(mesh.material as THREE.Material).depthTest,depthWrite:(mesh.material as THREE.Material).depthWrite})),queries:this.#feedback.snapshot() } }
  finishFrame(): void { const read=this.#afterSubmit; this.#afterSubmit=null; read?.() }

  update(frame: LegacyVisualFrame): void {
    this.#frame = frame
    for (let index = 0; index < Math.max(frame.quads.length, this.#meshes.length); index++) {
      const quad = frame.quads[index]
      let mesh = this.#meshes[index]
      if (!quad) { if (mesh) mesh.visible = false; continue }
      const material = this.#materials[quad.material]?.[quad.frame]
      if (!material) throw new Error("Legacy visual material is not admitted")
      if (!mesh) {
        const geometry=quadGeometry()
        mesh = new THREE.Mesh(geometry,material); mesh.frustumCulled = false
        this.#meshes.push(mesh)
      }
      const group=quad.layer===0?this.world:quad.layer===1?this.noDepthClip:this.group
      if(mesh.parent!==group)group.add(mesh)
      mesh.material = material; mesh.visible = true
      // Source glow overlays retain their activation-list order, not distance sorting.
      mesh.renderOrder = quad.layer===2?index:0
      const position = mesh.geometry.getAttribute("position") as THREE.BufferAttribute
      const color = mesh.geometry.getAttribute("legacyColor") as THREE.BufferAttribute
      const uv=mesh.geometry.getAttribute("uv") as THREE.BufferAttribute,hdr=mesh.geometry.getAttribute("legacyHdr") as THREE.BufferAttribute
      const fog=mesh.geometry.getAttribute("legacyFog") as THREE.BufferAttribute
      ;(position.array as Float32Array).set(quad.positions)
      ;(color.array as Float32Array).set(quad.color)
      ;(uv.array as Float32Array).set(quad.uv);(hdr.array as Float32Array).fill(quad.hdrScale)
      ;(fog.array as Float32Array).fill(quad.layer===2?0:1)
      position.needsUpdate = true; color.needsUpdate = true
      uv.needsUpdate=true;hdr.needsUpdate=true
      fog.needsUpdate=true
      mesh.geometry.computeBoundingSphere();mesh.geometry.boundingSphere!.center.fromArray(quad.origin)
    }
    for(let index=0;index<Math.max(frame.meshes.length,this.#ropeMeshes.length);index++){
      const input=frame.meshes[index];let mesh=this.#ropeMeshes[index]
      if(!input){if(mesh)mesh.visible=false;continue}
      const material=this.#materials[input.material]?.[0]
      if(!material)throw new Error("Legacy rope material is not admitted")
      if(!mesh){mesh=new THREE.Mesh(new THREE.BufferGeometry(),material);mesh.frustumCulled=false;mesh.onBeforeRender=()=>{this.#ropeDraws++};this.world.add(mesh);this.#ropeMeshes.push(mesh)}
      mesh.visible=true;mesh.material=material;mesh.renderOrder=0x100000+index
      let geometry=mesh.geometry
      const vertices=input.positions.length/3
      if((geometry.getAttribute("position")?.count??0)<vertices||(geometry.index?.count??0)<input.indices.length){
        const capacity=2**Math.ceil(Math.log2(Math.max(vertices,geometry.getAttribute("position")?.count??0)))
        const indices=2**Math.ceil(Math.log2(Math.max(input.indices.length,geometry.index?.count??0)))
        geometry.dispose();geometry=new THREE.BufferGeometry();mesh.geometry=geometry
        for(const [name,size]of [["position",3],["uv",2],["legacyHdr",1],["legacyFog",1]]as const){
          const values=new Float32Array(capacity*size);if(name==="legacyHdr"||name==="legacyFog")values.fill(1)
          geometry.setAttribute(name,new THREE.BufferAttribute(values,size).setUsage(THREE.DynamicDrawUsage))
        }
        geometry.setAttribute("legacyColor",new THREE.BufferAttribute(new Uint8Array(capacity*4),4,true).setUsage(THREE.DynamicDrawUsage))
        geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(indices),1).setUsage(THREE.DynamicDrawUsage))
      }
      for(const [name,values]of [["position",input.positions],["uv",input.uv],["legacyColor",input.color]]as const){
        const attribute=geometry.getAttribute(name)as THREE.BufferAttribute;(attribute.array as Float32Array|Uint8Array).set(values);attribute.clearUpdateRanges();attribute.addUpdateRange(0,values.length);attribute.needsUpdate=true
      }
      geometry.index!.array.set(input.indices);geometry.index!.clearUpdateRanges();geometry.index!.addUpdateRange(0,input.indices.length);geometry.index!.needsUpdate=true
      geometry.setDrawRange(0,input.indices.length)
    }
  }

  /** Called immediately after this view's scene draw, before depth reset. */
  capture(camera:THREE.Camera): void {
    if (this.#failure) throw this.#failure
    if (this.#disposed || !this.#frame?.proxies.length) return
    const depth = HOOKS.get(this.#backend)?.depth
    if (!depth) throw new Error("Legacy visual scene depth is unavailable")
    this.#samples=depth.sampleCount
    const proxies = this.#frame.proxies
    const vertices = new Float32Array(proxies.length * 20)
    proxies.forEach((proxy,index)=>vertices.set(proxy.vertices,index*20))
    const encoder = this.#backend.device.createCommandEncoder({label:"Legacy visual visibility"}) as GPUCommandEncoder
    const matrices=new Float32Array(32);matrices.set(camera.matrixWorldInverse.elements);matrices.set(camera.projectionMatrix.elements,16)
    if(!this.#queryTarget||this.#queryTarget.width!==depth.width||this.#queryTarget.height!==depth.height||this.#queryTarget.sampleCount!==depth.sampleCount){
      this.#queryTarget?.destroy();this.#queryTarget=this.#backend.device.createTexture({label:"Legacy visibility raster target",size:[depth.width,depth.height],format:"r8unorm",sampleCount:depth.sampleCount,usage:GPUTextureUsage.RENDER_ATTACHMENT})
    }
    const read = this.#counter.issue(encoder,depth,vertices,matrices,"r8unorm",{view:this.#queryTarget.createView(),loadOp:"clear",clearValue:[0,0,0,0],storeOp:"discard"})
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
    this.#disposed = true; this.#afterSubmit=null; this.#counter.dispose(); this.#queryTarget?.destroy(); this.#feedback.clear()
    for (const mesh of this.#meshes) mesh.geometry.dispose()
    for (const mesh of this.#ropeMeshes) mesh.geometry.dispose()
    this.group.clear();this.world.removeFromParent();this.world.clear();this.noDepth.clear()
    const hook = HOOKS.get(this.#backend)!
    if (--hook.owners===0) { this.#backend.finishRender=hook.original; HOOKS.delete(this.#backend) }
  }
}
