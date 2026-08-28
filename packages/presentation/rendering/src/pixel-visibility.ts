const PIPELINES=new WeakMap<GPUDevice,Map<string,Promise<GPURenderPipeline>>>()

/** Raster sample counts for Source's partial pixel-visibility queries.
 * WebGPU occlusion queries are only guaranteed to distinguish zero/nonzero.
 * Count actual covered samples instead, against the unchanged scene MSAA depth.
 * Proxy geometry and temporal fading belong to the native world implementation.
 */
export class SourcePixelVisibility {
  readonly #device: GPUDevice
  readonly #pipelines = new Map<string, GPURenderPipeline>()
  #vertices: GPUBuffer | null = null
  #counts: GPUBuffer | null = null
  #readback: GPUBuffer | null = null
  #matrices:GPUBuffer|null=null
  #capacity = 0
  #pending = false
  #disposed = false

  constructor(device: GPUDevice) { this.#device = device }
  get pending(): boolean { return this.#pending }
  get bufferBytes(): number { return this.#capacity * (12 * 16 + 8 + 8)+(this.#matrices?128:0) }

  async prepare(sampleCount: number, format: GPUTextureFormat): Promise<void> {
    const key = `${sampleCount}:${format}`
    if (this.#pipelines.has(key)) return
    let shared=PIPELINES.get(this.#device)
    if(!shared){shared=new Map();PIPELINES.set(this.#device,shared)}
    let pending=shared.get(key)
    if(!pending){pending=this.#createPipeline(sampleCount,format);shared.set(key,pending)}
    const pipeline=await pending
    if(!this.#disposed)this.#pipelines.set(key,pipeline)
  }

  #createPipeline(sampleCount:number,format:GPUTextureFormat):Promise<GPURenderPipeline> {
    const multisampled = sampleCount > 1
    const module = this.#device.createShaderModule({ label: "Source pixel visibility sample counting", code: `
      @group(0) @binding(0) var sceneDepth: ${multisampled ? "texture_depth_multisampled_2d" : "texture_depth_2d"};
      @group(0) @binding(1) var<storage, read_write> counts: array<atomic<u32>>;
      struct Matrices { view: mat4x4f, projection: mat4x4f };
      @group(0) @binding(2) var<uniform> matrices: Matrices;
      struct Vertex { @builtin(position) position: vec4f, @location(0) @interpolate(flat) query: u32 };
      @vertex fn vertex(@location(0) position: vec4f, @builtin(instance_index) query: u32) -> Vertex {
        return Vertex(matrices.projection * (matrices.view * position), query);
      }
      @fragment fn fragment(input: Vertex${multisampled ? ", @builtin(sample_index) sample: u32" : ""}) -> @location(0) vec4f {
        let depth = textureLoad(sceneDepth, vec2i(input.position.xy), ${multisampled ? "i32(sample)" : "0"});
        atomicAdd(&counts[input.query * 2u + 1u], 1u);
        if (input.position.z <= depth) { atomicAdd(&counts[input.query * 2u], 1u); }
        return vec4f(0.0);
      }` })
    return this.#device.createRenderPipelineAsync({
      label: "Source pixel visibility sample counting", layout: "auto",
      vertex: { module, entryPoint: "vertex", buffers: [{ arrayStride: 16, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x4" }] }] },
      fragment: { module, entryPoint: "fragment", targets: [{ format, writeMask: 0 }] },
      primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: "back" },
      multisample: { count: sampleCount },
    })
  }

  /** Five world-space vertices per query: center, top-left, top-right,
   * bottom-right, bottom-left. A pending GPU read must not be overwritten.
   * The caller submits the encoder before invoking the returned read operation.
   */
  issue(encoder: GPUCommandEncoder, depth: GPUTexture, proxies: Float32Array, matrices:Float32Array, format: GPUTextureFormat, color: GPURenderPassColorAttachment): (() => Promise<Uint32Array>) | null {
    if (this.#disposed) throw new Error("Pixel visibility has been disposed")
    if (this.#pending || proxies.length === 0) return null
    if (proxies.length % 20 !== 0 || !proxies.every(Number.isFinite)) throw new Error("Invalid pixel visibility proxy vertices")
    if(matrices.length!==32||!matrices.every(Number.isFinite))throw new Error("Invalid pixel visibility camera matrices")
    const pipeline = this.#pipelines.get(`${depth.sampleCount}:${format}`)
    if (!pipeline) throw new Error("Pixel visibility sample pipeline is not prepared")
    const count = proxies.length / 20
    if (count > this.#capacity) {
      this.#vertices?.destroy(); this.#counts?.destroy(); this.#readback?.destroy()
      this.#capacity = count
      this.#vertices = this.#device.createBuffer({ label: "Pixel visibility proxy fans", size: count * 12 * 16, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST })
      this.#counts = this.#device.createBuffer({ label: "Pixel visibility sample counts", size: count * 8, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST })
      this.#readback = this.#device.createBuffer({ label: "Pixel visibility readback", size: count * 8, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST })
    }
    const vertices = new Float32Array(count * 12 * 4)
    // Source's clockwise fan is expressed in the renderer's CCW front convention.
    const fan = [0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 1, 4]
    for (let query = 0; query < count; query++) for (let index = 0; index < fan.length; index++) {
      const source = query * 20 + fan[index]! * 4
      vertices.set(proxies.subarray(source, source + 4), (query * 12 + index) * 4)
    }
    this.#device.queue.writeBuffer(this.#vertices!, 0, vertices)
    this.#matrices??=this.#device.createBuffer({label:"Pixel visibility camera matrices",size:128,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST})
    this.#device.queue.writeBuffer(this.#matrices,0,matrices)
    encoder.clearBuffer(this.#counts!)
    const group = this.#device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [
      { binding: 0, resource: depth.createView({ aspect: "depth-only" }) },
      { binding: 1, resource: { buffer: this.#counts! } },
      { binding: 2, resource: { buffer: this.#matrices } },
    ] })
    // The caller owns the compatible color attachment and its load/store policy.
    // No color samples are written; scene depth remains the query authority.
    const pass = encoder.beginRenderPass({ label: "Source pixel visibility", colorAttachments: [color] })
    pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.setVertexBuffer(0, this.#vertices!)
    for (let query = 0; query < count; query++) pass.draw(12, 1, query * 12, query)
    pass.end()
    const readback = this.#readback!
    encoder.copyBufferToBuffer(this.#counts!, 0, readback, 0, count * 8)
    this.#pending = true
    let started = false
    return async () => {
      if (started) throw new Error("Pixel visibility read already started")
      started = true
      try {
        await readback.mapAsync(GPUMapMode.READ, 0, count * 8)
        return new Uint32Array(readback.getMappedRange(0, count * 8)).slice()
      } finally {
        if (readback.mapState === "mapped") readback.unmap()
        this.#pending = false
      }
    }
  }

  dispose(): void {
    this.#disposed = true
    this.#vertices?.destroy(); this.#counts?.destroy(); this.#readback?.destroy();this.#matrices?.destroy()
    this.#pipelines.clear()
  }
}
