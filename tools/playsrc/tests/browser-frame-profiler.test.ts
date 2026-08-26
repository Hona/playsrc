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
    worker.listener!({ data: { id: 7, kind: "models", output: new ArrayBuffer(96) } })
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
    worker.listener!({ data: { id: 8, output: new ArrayBuffer(48) } })
    worker.listener!({ data: { id: 9, outputs: [new ArrayBuffer(32), new ArrayBuffer(16)], timings: { queueMilliseconds: 7 } } })
    expect(state.worker[1]).toMatchObject({ receivedBytes: 48, timings: { queueMilliseconds: 7 } })
    expect(state.counters.workerPending).toBe(0)
  })

  test("captures renderer-reported destroyed-texture validation errors without suppressing console output", () => {
    const logged: unknown[][] = []
    const browser = { ...host(), console: { error: (...arguments_: unknown[]) => logged.push(arguments_) } }
    const state = installBrowserFrameProfiler(browser)
    browser.console.error("THREE.WebGPURenderer: Uncaptured WebGPU GPUValidationError: Destroyed texture used in a submit.")
    expect(state.counters.validationErrors).toBe(0)
    state.active = true
    browser.console.error("THREE.WebGPURenderer:", new Error("GPUValidationError: Destroyed texture used in a submit."))
    expect(logged).toHaveLength(2)
    expect(state.counters.validationErrors).toBe(1)
    expect(state.losses[0]).toMatchObject({ kind: "validation", at: 20 })
    expect(state.losses[0].message).toContain("Destroyed texture")
  })
})
