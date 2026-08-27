import { describe, expect, test } from "bun:test"
import { installBrowserFrameProfiler } from "../profile/browser-frame-profiler"

function host(supported: string[] = []) {
  class GPUQueue { submit(_buffers: unknown[]): void {} writeBuffer(_buffer: unknown, _offset: number, _data: Uint8Array): void {} }
  class GPUDevice { createTexture(): object { return {} } createBuffer(): object { return {} } createShaderModule(): object { return {} } createRenderPipeline(): object { return {} } }
  class GPUCommandEncoder { beginRenderPass(): object { return {} } }
  class GPURenderBundleEncoder { finish(): object { return {} } }
  const observers = new Map<string, (list: any) => void>()
  class Observer {
    static supportedEntryTypes = supported
    constructor(readonly callback: (list: any) => void) {}
    observe(options: { type: string }): void { observers.set(options.type, this.callback) }
  }
  return { GPUQueue, GPUDevice, GPUCommandEncoder, GPURenderBundleEncoder, PerformanceObserver: Observer, performance: { now: () => 20 }, observers }
}

describe("opt-in structured browser frame profiler", () => {
  test("retains the application adapter and device, native promise identity, and exact pipeline matrix shapes", async () => {
    let adapterRequests = 0, deviceRequests = 0
    const browser = host()
    class Device extends browser.GPUDevice {
      queue = new browser.GPUQueue()
      lost = new Promise(() => {})
      addEventListener() {}
      features = new Set(["timestamp-query"])
      limits = { maxUniformBufferBindingSize: 65_536 }
    }
    const device = new Device()
    const devicePromise = Promise.resolve(device)
    class GPUAdapter {
      info = { vendor: "nvidia", architecture: "ampere", device: "2460", description: "actual application adapter" }
      requestDevice(_options: unknown) { deviceRequests += 1; return devicePromise }
    }
    const adapter = new GPUAdapter()
    const adapterPromise = Promise.resolve(adapter)
    class GPU { requestAdapter(_options: unknown) { adapterRequests += 1; return adapterPromise } }
    const environment = { ...browser, GPUDevice: Device, GPUAdapter, GPU, crypto: { subtle: { digest: async () => new Uint8Array(32).fill(0xab).buffer } } }
    const state = installBrowserFrameProfiler(environment)
    expect(new GPU().requestAdapter({ powerPreference: "high-performance" })).toBe(adapterPromise)
    await adapterPromise
    expect(adapter.requestDevice({ requiredFeatures: ["timestamp-query"] })).toBe(devicePromise)
    await devicePromise
    expect(adapterRequests).toBe(1)
    expect(deviceRequests).toBe(1)
    expect(state.adapters[0]).toMatchObject({ id: 1, powerPreference: "high-performance", info: { vendor: "nvidia", backend: null, backendType: null } })
    expect(state.devices[0]).toMatchObject({ id: 1, adapter: 1, features: ["timestamp-query"], limits: { maxUniformBufferBindingSize: 65_536 } })
    const vertex = (device.createShaderModule as any)({ code: "var<uniform> bones: array< mat4x4<f32>, 64 >;" })
    const fragment = (device.createShaderModule as any)({ code: "var<uniform> matrices: array<mat4x4<f32>,128u>;" })
    await Promise.resolve()
    expect(state.counters.shaderModules).toBe(0)
    state.active = true
    ;(device.createRenderPipeline as any)({ vertex: { module: vertex }, fragment: { module: fragment, targets: [{ format: "bgra8unorm" }] }, multisample: { count: 4 } })
    device.queue.submit([])
    expect(state.gpuOperations).toMatchObject([
      { kind: "createRenderPipeline", device: 1, vertexShader: 1, fragmentShader: 2, sampleCount: 4, targetFormats: ["bgra8unorm"] },
      { kind: "submit", device: 1 },
    ])
    expect(state.shaders.map((shader: any) => shader.literalMatrixArrayCounts)).toEqual([[64], [128]])
    expect(state.shaders.every((shader: any) => shader.sha256 === "ab".repeat(32) && shader.code === undefined)).toBe(true)
  })
  test("has no counters before explicit sample activation and preserves GPU return values", () => {
    const browser = host()
    const state = installBrowserFrameProfiler(browser)
    expect(installBrowserFrameProfiler(browser)).toBe(state)
    new browser.GPUQueue().submit([{}, {}])
    expect(new browser.GPUDevice().createTexture()).toEqual({})
    expect(state.counters.submissions).toBe(0)
    expect(state.counters.textures).toBe(0)
    expect(state.capabilities).toEqual({ timestampQuery: false, longAnimationFrame: false })
  })

  test("attributes submissions, resources, upload bytes, bundles and render passes to the current conceptual pass", () => {
    const browser = host()
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    state.currentPass = { submissions: 0, commandBuffers: 0, renderPasses: 0 }
    const queue = new browser.GPUQueue()
    queue.submit([{}, {}])
    queue.writeBuffer({}, 0, new Uint8Array(12))
    const device = new browser.GPUDevice()
    device.createTexture(); device.createBuffer(); device.createShaderModule(); device.createRenderPipeline()
    new browser.GPUCommandEncoder().beginRenderPass()
    new browser.GPURenderBundleEncoder().finish()
    expect(state.currentPass).toEqual({ submissions: 1, commandBuffers: 2, renderPasses: 1 })
    expect(state.counters).toMatchObject({ submissions: 1, commandBuffers: 2, renderPasses: 1, queueWriteCalls: 1, queueWriteBytes: 12, textures: 1, buffers: 1, shaderModules: 1, renderPipelines: 1, bundleEncodes: 1 })
    expect(state.queueWrites.histogram).toEqual({ "1-16": 1 })
    expect(state.queueWrites.phases["outside-pass"]).toEqual({ calls: 1, bytes: 12 })
    expect(state.queueWrites.resources.unlabeled).toMatchObject({ calls: 1, bytes: 12, minimumOffset: 0, maximumOffset: 12 })
    expect(state.queueWrites.stacks[0]).toMatchObject({ call: 1, resource: "unlabeled", bytes: 12 })
  })

  test("attributes actual typed-array write ranges to their visible phase and GPU resource", () => {
    const browser = host()
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    state.currentPass = { identity: "world" }
    ;(new browser.GPUQueue().writeBuffer as any)({ label: "bindingBuffer_model" }, 16, new Float32Array(12), 2, 4)
    expect(state.counters).toMatchObject({ queueWriteCalls: 1, queueWriteBytes: 16 })
    expect(state.queueWrites.phases.world).toEqual({ calls: 1, bytes: 16 })
    expect(state.queueWrites.resources.bindingBuffer_model).toEqual({ calls: 1, bytes: 16, minimumOffset: 16, maximumOffset: 32 })
  })

  test("retains async queue/readback timing without taking ownership of the native promise", async () => {
    let now = 20
    let finish!: () => void
    let reject!: (error: Error) => void
    const nativeReadback = new Promise<void>(resolve => { finish = resolve })
    const nativeFence = new Promise<void>((_resolve, fail) => { reject = fail })
    class GPUBuffer { mapAsync() { return nativeReadback } }
    class GPUQueue { submit() {} writeBuffer() {} onSubmittedWorkDone() { return nativeFence } }
    const browser = { ...host(), GPUBuffer, GPUQueue, performance: { now: () => now } }
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    expect(new browser.GPUBuffer().mapAsync()).toBe(nativeReadback)
    expect(new browser.GPUQueue().onSubmittedWorkDone()).toBe(nativeFence)
    expect(state.gpuOperations.map((record: any) => record.end)).toEqual([undefined, undefined])
    now = 525
    finish(); reject(new Error("device lost"))
    await nativeReadback
    await expect(nativeFence).rejects.toThrow("device lost")
    expect(state.gpuOperations).toMatchObject([{ kind: "mapAsync", at: 20, end: 525 }, { kind: "onSubmittedWorkDone", at: 20, end: 525, failed: true }])
  })

  test("bounds GPU probes explicitly without stopping native queue work", () => {
    const browser = host()
    const state = installBrowserFrameProfiler(browser)
    const queue = new browser.GPUQueue()
    queue.submit([])
    expect(state.gpuOperations).toHaveLength(0)
    state.active = true
    for (let index = 0; index < 16_390; index += 1) queue.submit([])
    expect(state.gpuOperations).toHaveLength(16_384)
    expect(state.gpuOperationsDropped).toBe(6)
    expect(state.counters.submissions).toBe(16_390)
  })

  test("preserves native throws, exact upload ranges, and stable resource identity", () => {
    const failure = new Error("invalid pipeline")
    class GPUDevice { createRenderPipeline() { throw failure } }
    const browser = { ...host(), GPUDevice }
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    expect(() => new browser.GPUDevice().createRenderPipeline()).toThrow(failure)
    const queue = new browser.GPUQueue(), buffer = { label: "palette" }
    ;(queue.writeBuffer as any)(buffer, 0, new Float32Array(12), 2, 4)
    ;(queue.writeBuffer as any)(buffer, 0, new Float32Array(12), 2)
    expect(state.gpuOperations[0]).toMatchObject({ kind: "createRenderPipeline", failed: true })
    expect(state.gpuOperations.slice(1)).toMatchObject([{ bytes: 16, label: "palette" }, { bytes: 40, label: "palette" }])
    expect(state.gpuOperations[1].resource).toBe(state.gpuOperations[2].resource)
  })

  test("feature-detects long animation frames and preserves script, function, layout and completed-frame attribution", () => {
    const browser = host(["long-animation-frame", "longtask"])
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    state.completedFrames.push({ tick: 8, displayFrame: 3, drawSurfaces: 70, props: 4 })
    browser.observers.get("long-animation-frame")!({ getEntries: () => [{
      startTime: 10, duration: 80, blockingDuration: 22, renderStart: 45, styleAndLayoutStart: 70,
      scripts: [{ sourceURL: "runtime.ts", sourceFunctionName: "render", duration: 55, forcedStyleAndLayoutDuration: 4 }],
    }] })
    expect(state.capabilities.longAnimationFrame).toBe(true)
    expect(state.longAnimationFrames[0]).toMatchObject({ duration: 80, blockingDuration: 22, styleAndLayoutMilliseconds: 20, tick: 8, displayFrame: 3, scripts: [{ url: "runtime.ts", function: "render", duration: 55 }] })
  })

  test("records actual transferred Worker output bytes without cloning gameplay snapshots", () => {
    class Worker {
      listener?: (event: { data: unknown }) => void
      addEventListener(_type: string, listener: (event: { data: unknown }) => void): void { this.listener = listener }
      postMessage(_message: unknown): void {}
    }
    const browser = { ...host(), Worker }
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    const worker = new browser.Worker("gameplay.js")
    worker.postMessage({ id: 7, kind: "models", batch: new Uint8Array(12) })
    ;(worker as any).__playsrcProfileReply({ id: 7, kind: "models", output: new ArrayBuffer(96) })
    expect(state.worker[0]).toMatchObject({ kind: "models", bytes: 12, receivedBytes: 96, finished: 20 })
    expect(state.counters.workerPending).toBe(0)
  })

  test("accounts for visibility companions sharing a model dispatch and their actual queue depth", () => {
    class Worker {
      listener?: (event: { data: unknown }) => void
      addEventListener(_type: string, listener: (event: { data: unknown }) => void): void { this.listener = listener }
      postMessage(_message: unknown): void {}
    }
    const browser = { ...host(), Worker }
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    const worker = new browser.Worker("gameplay.js")
    worker.postMessage({ id: 8, kind: "models", batch: new Uint8Array(24), visibility: { id: 9, views: [{}, {}] } })
    expect(state.worker).toMatchObject([
      { kind: "models", bytes: 24, pending: 0, sharedDispatch: false },
      { kind: "visibility", bytes: 112, pending: 1, sharedDispatch: true, views: 2 },
    ])
    expect(state.counters.workerMaximumPending).toBe(2)
    ;(worker as any).__playsrcProfileReply({ id: 8, output: new ArrayBuffer(48) })
    ;(worker as any).__playsrcProfileReply({ id: 9, outputs: [new ArrayBuffer(32), new ArrayBuffer(16)], timings: { queueMilliseconds: 7 } })
    expect(state.worker[1]).toMatchObject({ receivedBytes: 48, timings: { queueMilliseconds: 7 } })
    expect(state.counters.workerPending).toBe(0)
  })

  test("captures renderer-reported destroyed-texture validation errors without suppressing console output", () => {
    const logged: unknown[][] = []
    const browser = { ...host(), console: { error: (...arguments_: unknown[]) => logged.push(arguments_) } }
    const state = installBrowserFrameProfiler(browser)
    browser.console.error("THREE.WebGPURenderer: Uncaptured WebGPU GPUValidationError: Destroyed texture used in a submit.")
    expect(state.counters.validationErrors).toBe(1)
    state.active = true
    browser.console.error("THREE.WebGPURenderer:", new Error("GPUValidationError: Destroyed texture used in a submit."))
    expect(logged).toHaveLength(2)
    expect(state.counters.validationErrors).toBe(2)
    expect(state.losses[0]).toMatchObject({ kind: "validation", at: 20 })
    expect(state.losses[0].message).toContain("Destroyed texture")
  })
  test("distinguishes resident shared model bytes from transferred response bytes", () => {
    class Worker {
      listener?: (event: { data: unknown }) => void
      addEventListener(_type: string, listener: (event: { data: unknown }) => void): void { this.listener = listener }
      postMessage(_message: unknown): void {}
    }
    const browser = { ...host(), Worker }
    const state = installBrowserFrameProfiler(browser)
    state.active = true
    const worker = new browser.Worker("gameplay.js")
    worker.postMessage({ id: 9, kind: "models", batch: new Uint8Array(12) })
    ;(worker as any).__playsrcProfileReply({ id: 9, kind: "models", output: new SharedArrayBuffer(256), byteOffset: 32, byteLength: 96 })
    expect(state.worker).toHaveLength(1)
    expect(state.worker[0]).toMatchObject({ receivedBytes: 0, sharedBytes: 96 })
    expect(state.counters.workerPending).toBe(0)
  })

  test("retains unsampled device/resource failures but permits explicit teardown", async () => {
    let lose!: (info: object) => void
    let uncaptured!: (event: object) => void
    const device = { lost: new Promise(resolve => { lose = resolve }), addEventListener: (_: string, listener: typeof uncaptured) => { uncaptured = listener } }
    class GPUAdapter { async requestDevice() { return device } }
    const browser = { ...host(), GPUAdapter }
    const state = installBrowserFrameProfiler(browser)
    expect(await new browser.GPUAdapter().requestDevice()).toBe(device)
    uncaptured({ error: new Error("invalid resource") })
    lose({ reason: "destroyed", message: "normal replacement" })
    await Promise.resolve()
    expect(state.losses).toHaveLength(1)
    expect(state.losses[0]).toMatchObject({ kind: "resource", message: "invalid resource" })
  })
})
