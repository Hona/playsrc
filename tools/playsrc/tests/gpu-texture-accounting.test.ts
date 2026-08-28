import { expect, test } from "bun:test"
import { installGpuTextureAccounting } from "../profile/gpu-texture-accounting"
import { installBrowserFrameProfiler } from "../profile/browser-frame-profiler"

function fixture(serialized = false) {
  const calls = { create: 0, destroy: 0, write: 0 }
  let failure: "create" | "destroy" | "write" | null = null
  const error = new Error("native failure")
  class GPUTexture {
    width: number; height: number; depthOrArrayLayers: number
    dimension: string; format: string; mipLevelCount: number; sampleCount: number
    constructor(descriptor: any) {
      const size = descriptor.size
      ;[this.width, this.height, this.depthOrArrayLayers] = Array.isArray(size) ? [size[0], size[1] ?? 1, size[2] ?? 1] : [size.width, size.height ?? 1, size.depthOrArrayLayers ?? 1]
      this.dimension = descriptor.dimension ?? "2d"; this.format = descriptor.format
      this.mipLevelCount = descriptor.mipLevelCount ?? 1; this.sampleCount = descriptor.sampleCount ?? 1
    }
    destroy() { calls.destroy++; if (failure === "destroy") throw error; return "destroyed" }
  }
  class GPUDevice { createTexture(descriptor: any) { calls.create++; if (failure === "create") throw error; return new GPUTexture(descriptor) } }
  class GPUQueue { writeTexture(..._args: any[]) { calls.write++; if (failure === "write") throw error; return "written" } }
  const host = { GPUTexture, GPUDevice, GPUQueue }
  const install = serialized ? new Function(`return (${installGpuTextureAccounting.toString()})`)() as typeof installGpuTextureAccounting : installGpuTextureAccounting
  const state = install(host), device = new GPUDevice(), queue = new GPUQueue()
  return { state, device, queue, calls, error, host, fail: (value: typeof failure) => { failure = value } }
}

function reconcile(state: ReturnType<typeof installGpuTextureAccounting>) {
  for (const total of [state.live, state.created]) {
    const formats = Object.values(total.formats)
    expect(formats.reduce((sum, f) => sum + f.knownBytes, 0)).toBe(total.knownBytes)
    expect(formats.reduce((sum, f) => sum + f.textures, 0)).toBe(total.textures)
    expect(formats.reduce((sum, f) => sum + f.unknownByteTextures, 0)).toBe(total.unknownByteTextures)
    expect(total.compressedBytes).toBeLessThanOrEqual(total.knownBytes)
    expect(total.compressedTextures).toBeLessThanOrEqual(total.textures)
  }
  expect(state.created.textures - state.destroyedTextures).toBe(state.live.textures)
}

test("serialized authoritative owner reconciles replacement, cancellation and repeated destruction without clearing cumulative totals", () => {
  const { state, device, queue, calls, host } = fixture(true)
  expect(installGpuTextureAccounting(host)).toBe(state)
  const old = device.createTexture({ size: [8, 8], format: "bc1-rgba-unorm-srgb", mipLevelCount: 4 })
  // 4 + 1 + 1 + 1 eight-byte blocks, including sub-block mips.
  expect(state.live.compressedBytes).toBe(56)
  const candidate = device.createTexture({ size: [4, 4], format: "rgba8unorm", sampleCount: 4 })
  reconcile(state); expect(state.live.knownBytes).toBe(312)
  expect(queue.writeTexture({}, new Uint8Array(1024).subarray(16, 80), {}, {})).toBe("written")
  candidate.destroy() // canceled candidate; active compressed texture survives
  reconcile(state); expect(state.live.compressedBytes).toBe(56)
  const replacement = device.createTexture({ size: { width: 4, height: 4 }, format: "bc3-rgba-unorm" })
  old.destroy(); old.destroy()
  reconcile(state)
  expect(state.live).toMatchObject({ textures: 1, knownBytes: 16, compressedTextures: 1, compressedBytes: 16 })
  expect(state.live.formats["bc1-rgba-unorm-srgb"]).toEqual({ textures: 0, knownBytes: 0, unknownByteTextures: 0 })
  replacement.destroy(); reconcile(state)
  expect(state.live.knownBytes).toBe(0)
  expect(state.created).toMatchObject({ textures: 3, knownBytes: 328, compressedTextures: 2, compressedBytes: 72 })
  expect(state).toMatchObject({ destroyedTextures: 3, peakKnownBytes: 312, writeTextureCalls: 1, writeTextureSourceBytes: 64 })
  expect(calls).toEqual({ create: 3, destroy: 4, write: 1 })
})

test("array layers do not shrink with mips; volume depth does; format sizes and opaque depth remain explicit", () => {
  const { state, device } = fixture()
  const array = device.createTexture({ size: [8, 4, 6], format: "rgba16float", mipLevelCount: 4 })
  expect(state.live.knownBytes).toBe((32 + 8 + 2 + 1) * 6 * 8)
  array.destroy()
  const volume = device.createTexture({ size: [8, 4, 4], dimension: "3d", format: "r32float", mipLevelCount: 4 })
  expect(state.live.knownBytes).toBe((128 + 16 + 2 + 1) * 4)
  volume.destroy()
  for (const [format, bytes] of [["rg16float", 4], ["rgba32float", 16], ["stencil8", 1], ["depth16unorm", 2], ["depth32float", 4], ["bc4-r-snorm", 8], ["bc6h-rgb-ufloat", 16], ["etc2-rgb8-unorm", 8], ["eac-rg11-snorm", 16], ["astc-12x10-unorm-srgb", 16]] as const) {
    const t = device.createTexture({ size: [1, 1], format })
    expect(state.live.knownBytes).toBe(bytes); reconcile(state); t.destroy()
  }
  for (const format of ["depth24plus", "depth24plus-stencil8", "depth32float-stencil8", "unknown", "bc4-r-unorm-srgb", "bc2-rgba-snorm"]) device.createTexture({ size: [8, 8], format })
  expect(state.live).toMatchObject({ textures: 6, knownBytes: 0, unknownByteTextures: 6 })
  reconcile(state)
  expect(state.interpretation).toContain("Not physical GPU residency")
})

test("native failures and immutable normalized properties own accounting, not re-read descriptor getters", () => {
  const { state, device, queue, calls, error, fail } = fixture()
  fail("create"); expect(() => device.createTexture({})).toThrow(error); reconcile(state)
  fail(null)
  let reads = 0
  const texture = device.createTexture({ get size() { if (++reads > 1) throw new Error("read twice"); return [4, 4] }, format: "bc1-rgba-unorm" })
  expect(reads).toBe(1)
  fail("destroy"); expect(() => texture.destroy()).toThrow(error)
  expect(state.live.compressedBytes).toBe(8); reconcile(state)
  fail("write"); expect(() => queue.writeTexture({}, new Uint8Array(8), {}, {})).toThrow(error)
  expect(state.writeTextureCalls).toBe(0)
  fail(null); texture.destroy(); reconcile(state)
  expect(calls).toEqual({ create: 2, destroy: 2, write: 1 })
})

test("headed gameplay profiler chaining keeps active call counters separate from cumulative and live texture totals", () => {
  const { state, device, queue, calls, host } = fixture(true)
  const profile = installBrowserFrameProfiler({ ...host, performance: { now: () => 10 } })
  const prior = device.createTexture({ size: [4, 4], format: "bc1-rgba-unorm" })
  expect(profile.counters.textures).toBe(0)
  profile.active = true
  const next = device.createTexture({ size: [4, 4], format: "bc3-rgba-unorm" })
  queue.writeTexture({ texture: next }, new Uint8Array(16), {}, {})
  prior.destroy(); prior.destroy()
  expect(profile.counters).toMatchObject({ textures: 1, textureWrites: 1, textureWriteBytes: 16, destroyedTextures: 2 })
  expect(state).toMatchObject({ live: { textures: 1, compressedBytes: 16 }, created: { textures: 2, compressedBytes: 24 }, destroyedTextures: 1, writeTextureSourceBytes: 16 })
  expect(calls).toEqual({ create: 2, destroy: 2, write: 1 }); reconcile(state)
})
