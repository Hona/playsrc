import { describe, expect, test } from "bun:test"
import { requestCoreWebGpuDevice } from "../src/webgpu-core-device"

describe("core-profile WebGPU adapter ownership", () => {
  test("requests the high-performance core adapter and retains compression, timestamps, and core features", async () => {
    const operations: unknown[] = []
    const features = new Set(["core-features-and-limits", "texture-compression-bc", "timestamp-query", "unknown-experimental"])
    const device = { features }
    const adapter = {
      features,
      requestDevice: async (descriptor: unknown) => { operations.push(descriptor); return device },
    }
    const result = await requestCoreWebGpuDevice({
      requestAdapter: async options => { operations.push(options); return adapter },
    }, "high-performance")
    expect(operations).toEqual([
      { powerPreference: "high-performance" },
      { requiredFeatures: ["core-features-and-limits", "texture-compression-bc", "timestamp-query"] },
    ])
    expect(result.device).toBe(device)
  })

  test("does not silently replace an unavailable core adapter with compatibility rendering", async () => {
    await expect(requestCoreWebGpuDevice({ requestAdapter: async () => null })).rejects.toThrow(/core hardware adapter/)
  })
})
