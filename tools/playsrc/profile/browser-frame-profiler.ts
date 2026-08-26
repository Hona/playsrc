export function installBrowserFrameProfiler(host: any = globalThis): any {
  if (host.__playsrcFrameProfiler) return host.__playsrcFrameProfiler

  const state = {
    active: false,
    currentPass: null as any,
    completedFrames: [] as any[],
    compositorFrames: [] as any[],
    animationCallbacks: [] as number[],
    worker: [] as any[],
    simulation: [] as any[],
    simulationDropped: 0,
    input: [] as { at: number; revision: number; kind: string }[],
    longTasks: [] as any[],
    longAnimationFrames: [] as any[],
    gpuTimestamps: [] as { frame: number; milliseconds: number }[],
    gpuOperations: [] as { kind: string; at: number; returned?: number; end?: number; failed?: boolean; resource?: number; label?: string; bytes?: number; phase?: string }[],
    gpuOperationsDropped: 0,
    losses: [] as any[],
    queueWrites: {
      histogram: {} as Record<string, number>,
      phases: {} as Record<string, { calls: number; bytes: number }>,
      resources: {} as Record<string, { calls: number; bytes: number; minimumOffset: number; maximumOffset: number }>,
      stacks: [] as { call: number; phase: string; resource: string; offset: number; bytes: number; stack: string }[],
    },
    capabilities: {
      timestampQuery: false,
      longAnimationFrame: Boolean(host.PerformanceObserver?.supportedEntryTypes?.includes("long-animation-frame")),
    },
    counters: {
      displayOffers: 0, displayRejectedBusy: 0, displayRejectedUnchanged: 0, displayStarted: 0,
      displayAbandoned: 0, displayCoalesced: 0, displayRecovered: 0, completedFrames: 0, submissions: 0, commandBuffers: 0,
      renderPasses: 0, buffers: 0, textures: 0, shaderModules: 0, renderPipelines: 0,
      computePipelines: 0, bundleEncodes: 0, bundleEncodeMilliseconds: 0, queueWriteCalls: 0, queueWriteBytes: 0, queueWriteMilliseconds: 0, textureWriteBytes: 0,
      destroyedBuffers: 0, destroyedTextures: 0, computePasses: 0,
      textureWrites: 0, commandEncoders: 0, mappedBuffers: 0,
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
  wrap(host.GPUQueue, "writeBuffer", ([buffer, bufferOffset, data, offset, size], _result, milliseconds) => {
    state.counters.queueWriteCalls += 1
    const bytesPerElement = ArrayBuffer.isView(data) ? (data as any).BYTES_PER_ELEMENT ?? 1 : 1
    const bytes = size === undefined ? Math.max(0, (data?.byteLength ?? 0) - (offset ?? 0) * bytesPerElement) : size * bytesPerElement
    state.counters.queueWriteBytes += bytes
    state.counters.queueWriteMilliseconds += milliseconds ?? 0
    const bucket = bytes <= 16 ? "1-16" : bytes <= 64 ? "17-64" : bytes <= 256 ? "65-256" : bytes <= 1024 ? "257-1024" : bytes <= 16384 ? "1025-16384" : "16385+"
    state.queueWrites.histogram[bucket] = (state.queueWrites.histogram[bucket] ?? 0) + 1
    const phase = String(state.currentPass?.identity ?? "outside-pass")
    const phaseRecord = state.queueWrites.phases[phase] ??= { calls: 0, bytes: 0 }
    phaseRecord.calls += 1
    phaseRecord.bytes += bytes
    const label = typeof buffer?.label === "string" && buffer.label ? buffer.label : "unlabeled"
    const resource = Object.hasOwn(state.queueWrites.resources, label) || Object.keys(state.queueWrites.resources).length < 48 ? label : "other"
    const resourceRecord = state.queueWrites.resources[resource] ??= { calls: 0, bytes: 0, minimumOffset: Number.MAX_SAFE_INTEGER, maximumOffset: 0 }
    resourceRecord.calls += 1
    resourceRecord.bytes += bytes
    resourceRecord.minimumOffset = Math.min(resourceRecord.minimumOffset, Number(bufferOffset ?? 0))
    resourceRecord.maximumOffset = Math.max(resourceRecord.maximumOffset, Number(bufferOffset ?? 0) + bytes)
    const call = state.counters.queueWriteCalls
    if (state.queueWrites.stacks.length < 96 && (call <= 12 || call % 257 === 0)) {
      state.queueWrites.stacks.push({ call, phase, resource: label, offset: Number(bufferOffset ?? 0), bytes, stack: String(new Error().stack ?? "").split("\n").slice(2, 8).join("\n") })
    }
  }, true)
  wrap(host.GPUQueue, "writeTexture", ([, data]) => {
    state.counters.textureWrites += 1
    state.counters.textureWriteBytes += data?.byteLength ?? 0
  })
  wrap(host.GPUCommandEncoder, "beginRenderPass", () => {
    state.counters.renderPasses += 1
    if (state.currentPass) state.currentPass.renderPasses += 1
  })
  wrap(host.GPUCommandEncoder, "beginComputePass", () => { state.counters.computePasses += 1 })
  wrap(host.GPUBuffer, "destroy", () => { state.counters.destroyedBuffers += 1 })
  wrap(host.GPUBuffer, "mapAsync", () => { state.counters.mappedBuffers += 1 })
  wrap(host.GPUTexture, "destroy", () => { state.counters.destroyedTextures += 1 })
  wrap(host.GPURenderBundleEncoder, "finish", (_arguments, _result, milliseconds) => {
    state.counters.bundleEncodes += 1
    state.counters.bundleEncodeMilliseconds += milliseconds ?? 0
  }, true)
  for (const [method, counter] of [
    ["createBuffer", "buffers"], ["createTexture", "textures"], ["createCommandEncoder", "commandEncoders"], ["createShaderModule", "shaderModules"],
    ["createRenderPipeline", "renderPipelines"], ["createRenderPipelineAsync", "renderPipelines"],
    ["createComputePipeline", "computePipelines"], ["createComputePipelineAsync", "computePipelines"],
  ] as const) wrap(host.GPUDevice, method, () => { state.counters[counter] += 1 })

  // Native promises and queue ordering stay owned by the application. Observe, never await a fence.
  const resources = new WeakMap<object, number>()
  let nextResource = 0
  for (const [owner, methods] of [
    [host.GPUQueue, ["submit", "writeBuffer", "writeTexture", "copyExternalImageToTexture", "onSubmittedWorkDone"]],
    [host.GPUBuffer, ["mapAsync"]],
    [host.GPUDevice, ["createRenderPipeline", "createRenderPipelineAsync"]],
    [host.GPUCanvasContext, ["configure", "unconfigure"]],
  ] as const) for (const method of methods) {
    const original = owner?.prototype?.[method]
    if (typeof original !== "function") continue
    Object.defineProperty(owner.prototype, method, { configurable: true, writable: true, value(this: any, ...arguments_: any[]) {
      if (!state.active) return original.apply(this, arguments_)
      if (state.gpuOperations.length >= 16_384) { state.gpuOperationsDropped += 1; return original.apply(this, arguments_) }
      const record: typeof state.gpuOperations[number] = { kind: method, at: host.performance.now(), phase: state.currentPass?.identity }
      const resource = method === "writeBuffer" ? arguments_[0] : this
      if (resource && typeof resource === "object") {
        if (!resources.has(resource)) resources.set(resource, ++nextResource)
        record.resource = resources.get(resource)
        if (typeof resource.label === "string") record.label = resource.label.slice(0, 128)
      }
      if (method === "writeBuffer") {
        const data = arguments_[2], offset = arguments_[3] ?? 0, size = arguments_[4]
        const elementBytes = ArrayBuffer.isView(data) ? (data as any).BYTES_PER_ELEMENT ?? 1 : 1
        record.bytes = size === undefined ? data.byteLength - offset * elementBytes : size * elementBytes
      }
      state.gpuOperations.push(record)
      try {
        const result = original.apply(this, arguments_)
        record.returned = host.performance.now()
        if (method === "mapAsync" || method === "onSubmittedWorkDone" || method === "createRenderPipelineAsync") {
          void result.then(() => { record.end = host.performance.now() }, () => { record.end = host.performance.now(); record.failed = true })
        } else record.end = record.returned
        return result
      } catch (error) { record.end = host.performance.now(); record.failed = true; throw error }
    } })
  }

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
          const shared = typeof SharedArrayBuffer === "function" && event.data?.output instanceof SharedArrayBuffer
          record.receivedBytes = shared ? 0 : event.data?.outputs?.reduce((total: number, output: ArrayBuffer) => total + output.byteLength, 0)
            ?? event.data?.output?.byteLength ?? event.data?.payload?.byteLength ?? 0
          record.sharedBytes = shared ? event.data.byteLength ?? 0 : 0
          if (event.data?.timings) record.timings = event.data.timings
          this.records.delete(event.data.id)
          state.counters.workerPending = Math.max(0, state.counters.workerPending - 1)
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (state.active && Number.isSafeInteger(message?.id) && typeof message?.kind === "string" && message.kind !== "release-model-output") {
          const started=host.performance.now()
          const register=(id:number,kind:string,bytes:number,sharedDispatch:boolean,views?:number)=>{
            const record={id,kind,started,bytes,pending:state.counters.workerPending,sharedDispatch,...(views===undefined?{}:{views})}
            this.records.set(id,record)
            state.worker.push(record)
            state.counters.workerPending+=1
            state.counters.workerMaximumPending=Math.max(state.counters.workerMaximumPending,state.counters.workerPending)
          }
          const views=Array.isArray(message.views)?message.views.length:undefined
          register(message.id,message.kind,message.command?.byteLength??message.batch?.byteLength??message.bsp?.byteLength??(views??0)*56,false,views)
          if(message.kind==="models"&&Number.isSafeInteger(message.visibility?.id)){
            const companionViews=Array.isArray(message.visibility.views)?message.visibility.views.length:0
            register(message.visibility.id,"visibility",companionViews*56,true,companionViews)
          }
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

  const consoleError = host.console?.error
  if (typeof consoleError === "function") {
    host.console.error = function(this: any, ...arguments_: any[]) {
      if (state.active) {
        const message = arguments_.map((value) => value instanceof Error ? value.message : String(value)).join(" ")
        if (/GPUValidationError|Destroyed texture/i.test(message)) {
          state.counters.validationErrors += 1
          state.losses.push({ kind: "validation", at: host.performance.now(), message })
        }
      }
      return consoleError.apply(this, arguments_)
    }
  }

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
