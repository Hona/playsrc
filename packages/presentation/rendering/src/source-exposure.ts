const HISTOGRAM_BUCKETS = 16
const QUERY_SWEEP_FRAMES = 16

const HISTOGRAM_SHADER = `
struct Histogram {
  bins: array<atomic<u32>, 16>,
};

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> output: Histogram;

var<workgroup> group_bins: array<atomic<u32>, 16>;

@compute @workgroup_size(16, 16)
fn main(
  @builtin(global_invocation_id) global: vec3<u32>,
  @builtin(local_invocation_index) local: u32,
) {
  if (local < 16u) {
    atomicStore(&group_bins[local], 0u);
  }
  workgroupBarrier();

  let dimensions = textureDimensions(screen);
  let minimum_x = u32(f32(dimensions.x) * 0.05);
  let minimum_y = u32(f32(dimensions.y) * 0.075);
  let maximum_x = dimensions.x - minimum_x;
  let maximum_y = dimensions.y - minimum_y;
  if (global.x >= minimum_x && global.x < maximum_x &&
      global.y >= minimum_y && global.y < maximum_y) {
    let color = textureLoad(screen, vec2<i32>(global.xy), 0).rgb;
    let luminance = clamp(dot(color, vec3<f32>(0.2125, 0.7154, 0.0721)), 0.0, 1.0);
    let bucket = min(15u, u32(pow(luminance, 1.0 / 1.5) * 16.0));
    atomicAdd(&group_bins[bucket], 1u);
  }
  workgroupBarrier();

  if (local < 16u) {
    let count = atomicLoad(&group_bins[local]);
    if (count != 0u) {
      atomicAdd(&output.bins[local], count);
    }
  }
}
`

export class SourceExposureSampler {
  readonly #device: any
  readonly #context: any
  readonly #format: string
  readonly #pipeline: any
  readonly #histogram: any
  readonly #readback: any
  #texture: any
  #bindGroup: any
  #width = 0
  #height = 0
  #frames = 0
  #pending = false
  #completed?: Uint32Array
  #disposed = false

  constructor(device: any, context: any, format: string) {
    this.#device = device
    this.#context = context
    this.#format = format
    this.#pipeline = device.createComputePipeline({
      layout: "auto",
      compute: { module: device.createShaderModule({ code: HISTOGRAM_SHADER }), entryPoint: "main" },
    })
    this.#histogram = device.createBuffer({
      size: HISTOGRAM_BUCKETS * 4,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
    })
    this.#readback = device.createBuffer({
      size: HISTOGRAM_BUCKETS * 4,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    })
  }

  take(): Uint32Array | undefined {
    const histogram = this.#completed
    this.#completed = undefined
    return histogram
  }

  sample(width: number, height: number): void {
    this.#frames += 1
    if (this.#disposed || this.#pending || this.#frames % QUERY_SWEEP_FRAMES !== 0 || width < 1 || height < 1) return
    if (width !== this.#width || height !== this.#height) {
      this.#texture?.destroy()
      this.#width = width
      this.#height = height
      this.#texture = this.#device.createTexture({
        size: { width, height },
        format: this.#format,
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      })
      this.#bindGroup = this.#device.createBindGroup({
        layout: this.#pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.#texture.createView() },
          { binding: 1, resource: { buffer: this.#histogram } },
        ],
      })
    }

    this.#device.queue.writeBuffer(this.#histogram, 0, new Uint32Array(HISTOGRAM_BUCKETS))
    const encoder = this.#device.createCommandEncoder()
    encoder.copyTextureToTexture(
      { texture: this.#context.getCurrentTexture() },
      { texture: this.#texture },
      { width, height },
    )
    const pass = encoder.beginComputePass()
    pass.setPipeline(this.#pipeline)
    pass.setBindGroup(0, this.#bindGroup)
    pass.dispatchWorkgroups(Math.ceil(width / 16), Math.ceil(height / 16))
    pass.end()
    encoder.copyBufferToBuffer(this.#histogram, 0, this.#readback, 0, HISTOGRAM_BUCKETS * 4)
    this.#device.queue.submit([encoder.finish()])
    this.#pending = true
    void this.#readback.mapAsync(GPUMapMode.READ).then(() => {
      if (!this.#disposed) {
        this.#completed = new Uint32Array(this.#readback.getMappedRange().slice(0))
      }
      this.#readback.unmap()
    }).catch(() => undefined).finally(() => {
      this.#pending = false
    })
  }

  dispose(): void {
    this.#disposed = true
    this.#texture?.destroy()
    this.#histogram.destroy()
    this.#readback.destroy()
  }
}
