import { expect, test } from "bun:test"
import { installWebGpuShaderProbe } from "../profile/webgpu-shader-probe"

test("shader probe preserves native results and snapshots mutable descriptors without GPU ownership", async () => {
  const pending = Promise.resolve({})
  class Device {
    createShaderModule(_descriptor: any) { return {} }
    createBindGroupLayout(_descriptor: any) { return {} }
    createPipelineLayout(_descriptor: any) { return {} }
    createRenderPipeline(_descriptor: any) { return { pipeline: true } }
    createRenderPipelineAsync(_descriptor: any) { return pending }
  }
  const host = { GPUDevice: Device, performance: { now: () => 1 }, __playsrcFrameProfiler: { active: false, currentPass: { identity: "main" } } }
  const probe = installWebGpuShaderProbe(host)
  const device = new Device()
  const module = device.createShaderModule({ code: "@vertex fn main() {}", label: "vertex" })
  const group = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: 1, buffer: { type: "uniform" } }] })
  const layout = device.createPipelineLayout({ bindGroupLayouts: [group] })
  const descriptor = { label: "native-pipeline", layout, vertex: { module, buffers: [{ arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] }] } }
  device.createRenderPipeline(descriptor)
  expect(probe.pipelines).toHaveLength(0)
  host.__playsrcFrameProfiler.active = true
  expect(device.createRenderPipeline(descriptor)).toEqual({ pipeline: true })
  expect(device.createRenderPipelineAsync(descriptor)).toBe(pending)
  descriptor.label = "reset"
  descriptor.vertex.buffers[0]!.attributes[0]!.format = "float32x2"
  await pending
  expect(probe.pipelines).toHaveLength(2)
  expect(probe.sources).toEqual([{ id: 0, code: "@vertex fn main() {}", label: "vertex" }])
  expect(probe.pipelines[0]).toMatchObject({ label: "native-pipeline", phase: "main", layout: [[{ binding: 0, visibility: 1, buffer: { type: "uniform" } }]], vertex: { source: 0, buffers: [{ arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] }] } })
  expect(probe.pipelines[1].completed).toBe(1)
  expect(installWebGpuShaderProbe(host)).toBe(probe)
})

test("probe can be serialized into CDP init script and bounds retained pipelines", () => {
  class Device {
    createShaderModule(_descriptor: any) { return {} }
    createRenderPipeline(_descriptor: any) { return {} }
  }
  const host = { GPUDevice: Device, performance: { now: () => 0 }, __playsrcFrameProfiler: { active: true } }
  const install = new Function(`return (${installWebGpuShaderProbe.toString()})`)()
  const probe = install(host)
  const device = new Device()
  const module = device.createShaderModule({ code: "shader" })
  for (let i = 0; i < 520; i++) device.createRenderPipeline({ vertex: { module }, layout: "auto" })
  expect(probe.pipelines).toHaveLength(512)
  expect(probe.dropped).toBe(8)
  expect(probe.sources).toHaveLength(1)
})

test("probe joins pipelines to the actual requested device without changing native promises or options", async () => {
  const calls: any[][] = []
  class Device {
    features = new Set(["texture-compression-bc"])
    createRenderPipeline(_descriptor: any) { return {} }
  }
  const device = new Device(), devicePromise = Promise.resolve(device)
  class Adapter {
    info = { vendor: "nvidia", architecture: "ampere", isFallbackAdapter: false }
    requestDevice(...args: any[]) { calls.push(args); return devicePromise }
  }
  const adapter = new Adapter(), adapterPromise = Promise.resolve(adapter)
  class Gpu { requestAdapter(...args: any[]) { calls.push(args); return adapterPromise } }
  const host = { GPU: Gpu, GPUAdapter: Adapter, GPUDevice: Device, performance: { now: () => 0 }, __playsrcFrameProfiler: { active: true } }
  const probe = installWebGpuShaderProbe(host)
  expect(new Gpu().requestAdapter()).toBe(adapterPromise)
  await adapterPromise
  const options = { requiredFeatures: ["texture-compression-bc"] }
  expect(adapter.requestDevice(options)).toBe(devicePromise)
  await devicePromise
  device.createRenderPipeline({ vertex: { module: {} } })
  expect(calls).toEqual([[], [options]])
  expect(probe.devices[0]).toMatchObject({ adapter: 0, adapterInfo: adapter.info, features: ["texture-compression-bc"], request: options })
  expect(probe.pipelines[0].device).toBe(0)
})
