import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"

type Copy = (texture: THREE.Texture, context: any, rectangle: THREE.Vector4) => void
type Hook = { original: Copy; wrapped: Copy; owners: Map<THREE.Texture, SourceParticleDepth> }
const HOOKS = new WeakMap<object, Hook>()

/** Resolves Source's compressed destination-alpha depth without reducing MSAA. */
export class SourceParticleDepth {
  readonly #backend: any
  readonly #textures = new Map<object | null, THREE.FramebufferTexture>()
  readonly #nodes = new Set<WeakRef<ReturnType<typeof TSL.texture>>>()
  readonly #pipelines = new Map<number, GPURenderPipeline>()
  readonly #uniforms = new Map<string, GPUBuffer>()
  readonly #groups = new WeakMap<GPUTexture, Map<string, GPUBindGroup>>()
  #texture: THREE.FramebufferTexture
  #call = -1
  #projection = [0, 0]
  #evidenceRequested = false
  #evidence: { before: GPUTexture; depth: GPUTexture; width: number; height: number; format: string; colorSpace: string } | null = null

  requestEvidence(): void { this.#evidenceRequested = true }
  get evidenceRequested(): boolean { return this.#evidenceRequested }

  async readEvidence(): Promise<{ before: Uint8Array; depth: Uint8Array; width: number; height: number; format: string; colorSpace: string } | null> {
    const evidence = this.#evidence
    if (!evidence) return null
    this.#evidence = null
    const device = this.#backend.device as GPUDevice
    const read = async (texture: GPUTexture) => {
      const stride = texture.format === "rgba16float" ? 8 : 4
      const bytesPerRow = Math.ceil(evidence.width * stride / 256) * 256
      const buffer = device.createBuffer({ size: bytesPerRow * evidence.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
      try {
        const encoder = device.createCommandEncoder({ label: "Particle depth evidence readback" })
        encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow }, [evidence.width, evidence.height]); device.queue.submit([encoder.finish()])
        await buffer.mapAsync(GPUMapMode.READ)
        const mapped = new Uint8Array(buffer.getMappedRange()), bytes = new Uint8Array(evidence.width * evidence.height * stride)
        for (let y = 0; y < evidence.height; y++) bytes.set(mapped.subarray(y * bytesPerRow, y * bytesPerRow + evidence.width * stride), y * evidence.width * stride)
        buffer.unmap(); return bytes
      } finally { buffer.destroy(); texture.destroy() }
    }
    const [before, depth] = await Promise.all([read(evidence.before), read(evidence.depth)])
    return { before, depth, width: evidence.width, height: evidence.height, format: evidence.format, colorSpace: evidence.colorSpace }
  }

  constructor(backend: any) {
    this.#backend = backend
    let hook = HOOKS.get(backend)
    if (!hook) {
      const original = backend.copyFramebufferToTexture.bind(backend) as Copy
      const owners = new Map<THREE.Texture, SourceParticleDepth>()
      const wrapped: Copy = (texture, context, rectangle) => {
        const owner = owners.get(texture)
        if (owner) owner.#resolve(texture, context)
        else original(texture, context, rectangle)
      }
      hook = { original, wrapped, owners }; HOOKS.set(backend, hook)
      backend.copyFramebufferToTexture = wrapped
    }
    this.#texture = this.#create(null)
  }

  #create(key: object | null): THREE.FramebufferTexture {
    const texture = new THREE.FramebufferTexture(1, 1)
    texture.internalFormat = "rgba8unorm" as any
    texture.minFilter = texture.magFilter = THREE.NearestFilter
    texture.colorSpace = THREE.NoColorSpace
    texture.generateMipmaps = false
    this.#textures.set(key, texture)
    HOOKS.get(this.#backend)!.owners.set(texture, this)
    return texture
  }

  async prepare(): Promise<void> {
    if (this.#pipelines.size === 2) return
    for (const samples of [1, 4]) {
      const depthType = samples === 1 ? "texture_depth_2d" : "texture_depth_multisampled_2d"
      const load = samples === 1 ? "textureLoad(depth, xy, 0)" : "textureLoad(depth, xy, sample)"
      const module = this.#backend.device.createShaderModule({ label: "Source particle destination-alpha depth", code: `
@group(0) @binding(0) var depth: ${depthType};
@group(0) @binding(1) var<uniform> projection: vec4f;
@vertex fn vertex(@builtin(vertex_index) i:u32) -> @builtin(position) vec4f {
  var points = array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));
  return vec4f(points[i],0,1);
}
@fragment fn fragment(@builtin(position) p:vec4f) -> @location(0) vec4f {
  let xy=vec2i(p.xy); var result=0.0;
  for(var sample=0;sample<${samples};sample++) {
    let d=${load};
    let clipZ=d*projection.y/(d+projection.x);
    result+=round(clamp(clipZ/192.0,0.0,1.0)*255.0)/255.0;
  }
  return vec4f(0,0,0,result/${samples}.0);
}` })
      const pipeline = await this.#backend.device.createRenderPipelineAsync({ label: `Source particle depth ${samples}x`, layout: "auto",
        vertex: { module, entryPoint: "vertex" }, fragment: { module, entryPoint: "fragment", targets: [{ format: "rgba8unorm" }] }, primitive: { topology: "triangle-list" } })
      this.#pipelines.set(samples, pipeline)
    }
  }

  sample(): ReturnType<typeof TSL.texture> {
    const node = TSL.texture(this.#texture, TSL.screenUV)
    this.#nodes.add(new WeakRef(node))
    return node
  }

  capture(renderer: THREE.WebGPURenderer, camera: THREE.Camera): void {
    // Three traverses onBeforeRender while compiling too; no live pass exists then.
    if (Array.isArray((renderer as any)._compilationPromises)) return
    if (this.#call === renderer.info.calls) return
    const target = renderer.getRenderTarget(), key = target ?? renderer.getCanvasTarget()
    const texture = this.#textures.get(key) ?? this.#create(key)
    if (this.#texture !== texture) {
      this.#texture = texture
      for (const reference of this.#nodes) { const node = reference.deref(); if (node) node.value = texture; else this.#nodes.delete(reference) }
    }
    const size = target ? { x: target.width, y: target.height } : renderer.getDrawingBufferSize(new THREE.Vector2())
    if (texture.image.width !== size.x || texture.image.height !== size.y) {
      texture.image.width = size.x; texture.image.height = size.y; texture.needsUpdate = true
    }
    this.#projection = [camera.projectionMatrix.elements[10]!, camera.projectionMatrix.elements[14]!]
    renderer.copyFramebufferToTexture(texture)
    this.#call = renderer.info.calls
  }

  #resolve(texture: THREE.Texture, context: any): void {
    const backend = this.#backend, device = backend.device as GPUDevice, state = backend.get(context)
    const source: GPUTexture = context.renderTarget ? backend.get(context.depthTexture).texture : backend.textureUtils.getDepthBuffer(context.depth, context.stencil)
    const destination: GPUTexture = backend.get(texture).texture
    const pipeline = this.#pipelines.get(source.sampleCount)
    if (!pipeline || !state.currentPass || destination.format !== "rgba8unorm") throw new Error("Source particle depth pass is not prepared")
    const projectionKey = this.#projection.join(":")
    let uniform = this.#uniforms.get(projectionKey)
    if (!uniform) {
      uniform = device.createBuffer({ label: "Source particle depth projection", size: 16, usage: GPUBufferUsage.UNIFORM, mappedAtCreation: true })
      new Float32Array(uniform.getMappedRange()).set(this.#projection)
      uniform.unmap(); this.#uniforms.set(projectionKey, uniform)
    }
    let groups = this.#groups.get(source)
    if (!groups) { groups = new Map(); this.#groups.set(source, groups) }
    let group = groups.get(projectionKey)
    if (!group) {
      group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [
        { binding: 0, resource: source.createView({ aspect: "depth-only" }) }, { binding: 1, resource: { buffer: uniform } },
      ] }); groups.set(projectionKey, group)
    }
    state.currentPass.end()
    if (this.#evidenceRequested) {
      const color: GPUTexture = context.renderTarget ? backend.get(context.textures[0]).texture : backend.context.getCurrentTexture()
      if (!["rgba8unorm", "bgra8unorm", "rgba16float"].includes(color.format)) throw new Error("Particle evidence output format is unavailable")
      this.#evidence?.before.destroy(); this.#evidence?.depth.destroy()
      const size = [destination.width, destination.height]
      this.#evidence = { before: device.createTexture({ size, format: color.format, usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC }),
        depth: device.createTexture({ size, format: "rgba8unorm", usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC }), width: destination.width, height: destination.height, format: color.format,
        colorSpace: context.renderTarget ? context.textures[0].colorSpace : backend.renderer.outputColorSpace }
      state.encoder.copyTextureToTexture({ texture: color }, { texture: this.#evidence.before }, size)
    }
    const pass = state.encoder.beginRenderPass({ label: "Source particle destination-alpha resolve", colorAttachments: [{ view: destination.createView(), loadOp: "clear", storeOp: "store", clearValue: [0,0,0,1] }] })
    if (context.scissor) { const { x, y, width, height } = context.scissorValue; pass.setScissorRect(x, y, width, height) }
    pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.draw(3); pass.end()
    if (this.#evidenceRequested && this.#evidence) {
      state.encoder.copyTextureToTexture({ texture: destination }, { texture: this.#evidence.depth }, [destination.width, destination.height])
      this.#evidenceRequested = false
    }
    for (const color of state.descriptor.colorAttachments) color.loadOp = "load"
    if (context.depth) state.descriptor.depthStencilAttachment.depthLoadOp = "load"
    if (context.stencil) state.descriptor.depthStencilAttachment.stencilLoadOp = "load"
    // Retain the original beginning timestamp; the final resumed end includes
    // opaque drawing, depth resolution and every transparent draw, not just the tail.
    const timestamps = state.descriptor.timestampWrites
    if (timestamps) state.descriptor.timestampWrites = { querySet: timestamps.querySet, endOfPassWriteIndex: timestamps.endOfPassWriteIndex }
    state.currentPass = state.encoder.beginRenderPass(state.descriptor)
    state.currentSets = { attributes: {}, bindingGroups: [], pipeline: null, index: null }
    if (context.viewport) backend.updateViewport(context)
    if (context.scissor) backend.updateScissor(context)
  }

  dispose(): void {
    const hook = HOOKS.get(this.#backend)!
    for (const texture of this.#textures.values()) { hook.owners.delete(texture); texture.dispose() }
    for (const buffer of this.#uniforms.values()) buffer.destroy()
    this.#evidence?.before.destroy(); this.#evidence?.depth.destroy(); this.#evidence = null
    this.#textures.clear(); this.#nodes.clear(); this.#uniforms.clear(); this.#pipelines.clear()
    if (hook.owners.size === 0) { if (this.#backend.copyFramebufferToTexture === hook.wrapped) this.#backend.copyFramebufferToTexture = hook.original; HOOKS.delete(this.#backend) }
  }
}
