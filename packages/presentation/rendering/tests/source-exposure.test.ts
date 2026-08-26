import { describe, expect, test } from "bun:test"
import { SourceExposureSampler } from "../src/source-exposure"

describe("Source integer-HDR framebuffer histogram", () => {
  test("samples real center-region pixels once per complete sixteen-bin query sweep", async () => {
    const globals = globalThis as typeof globalThis & Record<string, unknown>
    const previous = Object.fromEntries(["GPUBufferUsage", "GPUTextureUsage", "GPUMapMode"].map((key) => [key, globals[key]]))
    globals.GPUBufferUsage = { STORAGE: 1, COPY_SRC: 2, COPY_DST: 4, MAP_READ: 8 }
    globals.GPUTextureUsage = { COPY_DST: 1, TEXTURE_BINDING: 2 }
    globals.GPUMapMode = { READ: 1 }
    let submissions = 0
    let copies = 0
    let destroyed = 0
    let code = ""
    const values = new Uint32Array(16)
    values[5] = 42
    const buffer = () => ({
      destroy: () => { destroyed += 1 },
      mapAsync: async () => undefined,
      getMappedRange: () => values.buffer,
      unmap: () => undefined,
    })
    const device = {
      createShaderModule: (input: { code: string }) => { code = input.code; return {} },
      createComputePipeline: () => ({ getBindGroupLayout: () => ({}) }),
      createBuffer: buffer,
      createTexture: () => ({ createView: () => ({}), destroy: () => { destroyed += 1 } }),
      createBindGroup: () => ({}),
      createCommandEncoder: () => ({
        copyTextureToTexture: () => { copies += 1 },
        beginComputePass: () => ({ setPipeline: () => undefined, setBindGroup: () => undefined, dispatchWorkgroups: () => undefined, end: () => undefined }),
        copyBufferToBuffer: () => undefined,
        finish: () => ({}),
      }),
      queue: { writeBuffer: () => undefined, submit: () => { submissions += 1 } },
    }
    try {
      const sampler = new SourceExposureSampler(device, { getCurrentTexture: () => ({}) }, "bgra8unorm")
      for (let frame = 0; frame < 15; frame += 1) sampler.sample(320, 180)
      expect(submissions).toBe(0)
      sampler.sample(320, 180)
      expect(submissions).toBe(1)
      expect(copies).toBe(1)
      await Promise.resolve()
      expect(sampler.take()?.[5]).toBe(42)
      expect(sampler.take()).toBeUndefined()
      expect(code).toContain("0.2125, 0.7154, 0.0721")
      expect(code).toContain("dimensions.x) * 0.05")
      expect(code).toContain("dimensions.y) * 0.075")
      sampler.dispose()
      expect(destroyed).toBe(3)
    } finally {
      for (const [key, value] of Object.entries(previous)) {
        if (value === undefined) delete globals[key]
        else globals[key] = value
      }
    }
  })
})
