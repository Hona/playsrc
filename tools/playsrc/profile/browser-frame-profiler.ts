export function installBrowserFrameProfiler(host: any = globalThis): any {
  if (host.__playsrcFrameProfiler) return host.__playsrcFrameProfiler

  const state = {
    active: false,
    currentPass: null as any,
    completedFrames: [] as any[],
    animationCallbacks: [] as number[],
    worker: [] as any[],
    longTasks: [] as any[],
    longAnimationFrames: [] as any[],
    gpuTimestamps: [] as { frame: number; milliseconds: number }[],
    losses: [] as any[],
    capabilities: {
      timestampQuery: false,
      longAnimationFrame: Boolean(host.PerformanceObserver?.supportedEntryTypes?.includes("long-animation-frame")),
    },
    counters: {
      displayOffers: 0, displayRejectedBusy: 0, displayRejectedUnchanged: 0, displayStarted: 0,
      displayAbandoned: 0, completedFrames: 0, submissions: 0, commandBuffers: 0,
      renderPasses: 0, buffers: 0, textures: 0, shaderModules: 0, renderPipelines: 0,
      computePipelines: 0, bundleEncodes: 0, bundleEncodeMilliseconds: 0, queueWriteBytes: 0, textureWriteBytes: 0,
      destroyedBuffers: 0, destroyedTextures: 0, computePasses: 0,
      workerPending: 0, workerMaximumPending: 0, validationErrors: 0,
      nodeBuilderMisses: 0, nodeBuilderMilliseconds: 0, warmedPipelineVariants: 0, pipelineWarmupMilliseconds: 0,
    },
  }
  Object.defineProperty(host, "__playsrcFrameProfiler", { configurable: true, value: state })

  const wrap = (owner: any, method: string, observe: (arguments_: any[], result?: any, milliseconds?: number) => void, timed = false): void => {
    const original = owner?.prototype?.[method]
    if (typeof original !== "function") return
    Object.defineProperty(owner.prototype, method, {
      configurable: true,
      writable: true,
      value(this: any, ...arguments_: any[]) {
        const started = timed && state.active ? host.performance.now() : 0
        const result = original.apply(this, arguments_)
        if (state.active) observe(arguments_, result, started ? host.performance.now() - started : 0)
        return result
      },
    })
  }

  wrap(host.GPUQueue, "submit", ([buffers]) => {
    state.counters.submissions += 1
    state.counters.commandBuffers += buffers?.length ?? 0
    if (state.currentPass) { state.currentPass.submissions += 1; state.currentPass.commandBuffers += buffers?.length ?? 0 }
  })
  wrap(host.GPUQueue, "writeBuffer", ([, , data, offset, size]) => {
    const bytesPerElement = ArrayBuffer.isView(data) ? (data as any).BYTES_PER_ELEMENT ?? 1 : 1
    state.counters.queueWriteBytes += size === undefined ? Math.max(0, (data?.byteLength ?? 0) - (offset ?? 0) * bytesPerElement) : size * bytesPerElement
  })
  wrap(host.GPUQueue, "writeTexture", ([, data]) => { state.counters.textureWriteBytes += data?.byteLength ?? 0 })
  wrap(host.GPUCommandEncoder, "beginRenderPass", () => {
    state.counters.renderPasses += 1
    if (state.currentPass) state.currentPass.renderPasses += 1
  })
  wrap(host.GPUCommandEncoder, "beginComputePass", () => { state.counters.computePasses += 1 })
  wrap(host.GPUBuffer, "destroy", () => { state.counters.destroyedBuffers += 1 })
  wrap(host.GPUTexture, "destroy", () => { state.counters.destroyedTextures += 1 })
  wrap(host.GPURenderBundleEncoder, "finish", (_arguments, _result, milliseconds) => {
    state.counters.bundleEncodes += 1
    state.counters.bundleEncodeMilliseconds += milliseconds ?? 0
  }, true)
  for (const [method, counter] of [
    ["createBuffer", "buffers"], ["createTexture", "textures"], ["createShaderModule", "shaderModules"],
    ["createRenderPipeline", "renderPipelines"], ["createRenderPipelineAsync", "renderPipelines"],
    ["createComputePipeline", "computePipelines"], ["createComputePipelineAsync", "computePipelines"],
  ] as const) wrap(host.GPUDevice, method, () => { state.counters[counter] += 1 })

  const NativeWorker = host.Worker
  if (typeof NativeWorker === "function") {
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, any>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const record = this.records.get(event.data?.id)
          if (!record) return
          record.finished = host.performance.now()
          if (event.data?.timings) record.timings = event.data.timings
          this.records.delete(event.data.id)
          state.counters.workerPending = Math.max(0, state.counters.workerPending - 1)
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (state.active && Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const record = {
            kind: message.kind, started: host.performance.now(),
            bytes: message.command?.byteLength ?? message.batch?.byteLength ?? message.bsp?.byteLength ?? 0,
            pending: state.counters.workerPending,
          }
          this.records.set(message.id, record)
          state.worker.push(record)
          state.counters.workerPending += 1
          state.counters.workerMaximumPending = Math.max(state.counters.workerMaximumPending, state.counters.workerPending)
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(host, "Worker", { configurable: true, writable: true, value: ProfiledWorker })
  }

  const observe = (type: string, receive: (entry: any) => void): void => {
    if (!host.PerformanceObserver?.supportedEntryTypes?.includes(type)) return
    try {
      new host.PerformanceObserver((list: any) => {
        if (!state.active) return
        for (const entry of list.getEntries()) receive(entry)
      }).observe({ type, buffered: false })
    } catch {}
  }
  observe("longtask", entry => state.longTasks.push({ at: entry.startTime, duration: entry.duration }))
  observe("long-animation-frame", entry => {
    const completed = state.completedFrames.at(-1)
    state.longAnimationFrames.push({
      at: entry.startTime, duration: entry.duration, blockingDuration: entry.blockingDuration ?? 0,
      renderStart: entry.renderStart ?? null, styleAndLayoutStart: entry.styleAndLayoutStart ?? null,
      styleAndLayoutMilliseconds: entry.styleAndLayoutStart > 0 ? Math.max(0, entry.startTime + entry.duration - entry.styleAndLayoutStart) : 0,
      firstUiEventTimestamp: entry.firstUIEventTimestamp ?? null,
      tick: completed?.tick ?? null, displayFrame: completed?.displayFrame ?? null,
      visibleSurfaces: completed?.drawSurfaces ?? null, visibleStaticProps: completed?.props ?? null,
      gpu: { textures: state.counters.textures, buffers: state.counters.buffers, pipelines: state.counters.renderPipelines },
      scripts: Array.from(entry.scripts ?? [], (script: any) => ({
        url: script.sourceURL ?? "", function: script.sourceFunctionName ?? "", duration: script.duration ?? 0,
        executionStart: script.executionStart ?? null, forcedStyleAndLayoutDuration: script.forcedStyleAndLayoutDuration ?? 0,
        invoker: script.invoker ?? "", invokerType: script.invokerType ?? "",
      })),
    })
  })

  if (typeof host.addEventListener === "function") {
    host.addEventListener("error", (event: any) => {
      const message = String(event.message ?? event.error?.message ?? "")
      if (/GPUValidationError|WebGPU|device lost|context lost/i.test(message)) {
        if (/validation/i.test(message)) state.counters.validationErrors += 1
        state.losses.push({ kind: /context/i.test(message) ? "context" : /device/i.test(message) ? "device" : "resource", at: host.performance.now(), message })
      }
    })
    host.addEventListener("unhandledrejection", (event: any) => {
      const message = String(event.reason?.message ?? event.reason ?? "")
      if (/GPUValidationError/i.test(message)) {
        state.counters.validationErrors += 1
        state.losses.push({ kind: "validation", at: host.performance.now(), message })
      }
    })
  }
  return state
}
