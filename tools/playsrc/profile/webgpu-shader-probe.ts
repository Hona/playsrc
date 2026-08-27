/** Opt-in, bounded shader evidence for native compiler attribution. Install before
 * application startup; capture only pipelines requested in the active sample.
 * Does not modify descriptors, request a device, wait on a fence or retain GPUs. */
export function installWebGpuShaderProbe(host: any = globalThis) {
  if (host.__playsrcShaderProbe) return host.__playsrcShaderProbe
  const state = { adapters: [] as any[], devices: [] as any[], sources: [] as any[], pipelines: [] as any[], dropped: 0, sourceCharacters: 0 }
  Object.defineProperty(host, "__playsrcShaderProbe", { configurable: true, value: state })
  const modules = new WeakMap<object, { code: string; label: string; id?: number }>()
  const layouts = new WeakMap<object, any>()
  const groups = new WeakMap<object, any>()
  const adapterIds = new WeakMap<object, number>()
  const deviceIds = new WeakMap<object, number>()
  const info = (value: any) => value ? {
    vendor: value.vendor, architecture: value.architecture, device: value.device,
    description: value.description, isFallbackAdapter: value.isFallbackAdapter,
  } : null
  for (const [owner, method, records] of [
    [host.GPU, "requestAdapter", state.adapters], [host.GPUAdapter, "requestDevice", state.devices],
  ] as const) {
    const original = owner?.prototype?.[method]
    if (typeof original !== "function") continue
    Object.defineProperty(owner.prototype, method, { configurable: true, writable: true, value(this: any, ...args: any[]) {
      const promise = original.apply(this, args)
      const descriptor = args[0] ?? {}
      if (records.length >= 16) { state.dropped += 1; return promise }
      const record: any = { id: records.length, at: host.performance.now() }
      if (method === "requestAdapter") record.request = { powerPreference: descriptor.powerPreference, forceFallbackAdapter: descriptor.forceFallbackAdapter, featureLevel: descriptor.featureLevel }
      else {
        record.adapter = adapterIds.get(this) ?? null
        record.adapterInfo = info(this.info)
        record.request = { label: descriptor.label, requiredFeatures: Array.isArray(descriptor.requiredFeatures) ? [...descriptor.requiredFeatures] : null,
          requiredLimits: descriptor.requiredLimits ? { ...descriptor.requiredLimits } : null }
      }
      records.push(record)
      void promise.then((result: any) => {
        record.completed = host.performance.now()
        if (!result) { record.unavailable = true; return }
        if (method === "requestAdapter") { adapterIds.set(result, record.id); record.info = info(result.info) }
        else { deviceIds.set(result, record.id); record.features = Array.from(result.features ?? []) }
      }, () => { record.failed = true; record.completed = host.performance.now() })
      return promise
    } })
  }
  const prototype = host.GPUDevice?.prototype
  if (!prototype) return state
  const wrap = (name: string, observe: (args: any[], result: any, at: number, returned: number, device: object) => void) => {
    const original = prototype[name]
    if (typeof original !== "function") return
    Object.defineProperty(prototype, name, { configurable: true, writable: true, value(this: any, ...args: any[]) {
      const at = host.performance.now()
      const result = original.apply(this, args)
      observe(args, result, at, host.performance.now(), this)
      return result
    } })
  }
  wrap("createShaderModule", ([descriptor], result) => {
    if (result && typeof descriptor?.code === "string" && descriptor.code.length <= 1024 * 1024) {
      modules.set(result, { code: descriptor.code, label: String(descriptor.label ?? "").slice(0, 256) })
    }
  })
  wrap("createBindGroupLayout", ([descriptor], result) => {
    if (!result || !Array.isArray(descriptor?.entries)) return
    groups.set(result, descriptor.entries.map((entry: any) => ({
      binding: entry.binding, visibility: entry.visibility,
      ...(entry.buffer ? { buffer: { ...entry.buffer } } : {}),
      ...(entry.texture ? { texture: { ...entry.texture } } : {}),
      ...(entry.storageTexture ? { storageTexture: { ...entry.storageTexture } } : {}),
      ...(entry.sampler ? { sampler: { ...entry.sampler } } : {}),
    })))
  })
  wrap("createPipelineLayout", ([descriptor], result) => {
    if (result && Array.isArray(descriptor?.bindGroupLayouts)) layouts.set(result, descriptor.bindGroupLayouts.map((group: object) => groups.get(group) ?? null))
  })
  const source = (module: object) => {
    const value = modules.get(module)
    if (!value) return null
    if (value.id !== undefined) return value.id
    if (state.sourceCharacters + value.code.length > 8 * 1024 * 1024) { state.dropped += 1; return null }
    value.id = state.sources.length
    state.sources.push({ id: value.id, code: value.code, label: value.label })
    state.sourceCharacters += value.code.length
    return value.id
  }
  for (const method of ["createRenderPipeline", "createRenderPipelineAsync"]) wrap(method, ([descriptor], result, at, returned, device) => {
    if (!host.__playsrcFrameProfiler?.active && !host.__playsrcFrameProfiler?.captureModelPrograms) return
    if (state.pipelines.length >= 512) { state.dropped += 1; return }
    const record: any = {
      method, at, returned, device: deviceIds.get(device) ?? null, label: String(descriptor.label ?? "").slice(0, 256),
      phase: host.__playsrcFrameProfiler.currentPass?.identity ?? null,
      admission: host.__playsrcFrameProfiler.active ? "sample" : host.__playsrcFrameProfiler.modelPreparation?.ended === undefined ? "preparation" : "post-preparation",
      vertex: { source: source(descriptor.vertex.module), entryPoint: descriptor.vertex.entryPoint,
        buffers: Array.isArray(descriptor.vertex.buffers) ? descriptor.vertex.buffers.map((buffer: any) => buffer ? { ...buffer, attributes: Array.isArray(buffer.attributes) ? buffer.attributes.map((attribute: any) => ({ ...attribute })) : null } : null) : null },
      fragment: descriptor.fragment ? { source: source(descriptor.fragment.module), entryPoint: descriptor.fragment.entryPoint,
        targets: Array.isArray(descriptor.fragment.targets) ? descriptor.fragment.targets.map((target: any) => target ? { ...target, ...(target.blend ? { blend: { color: { ...target.blend.color }, alpha: { ...target.blend.alpha } } } : {}) } : null) : null } : null,
      primitive: { ...descriptor.primitive }, depthStencil: descriptor.depthStencil ? { ...descriptor.depthStencil } : null,
      multisample: { ...descriptor.multisample }, layout: typeof descriptor.layout === "object" ? layouts.get(descriptor.layout) ?? null : descriptor.layout,
    }
    state.pipelines.push(record)
    if (method === "createRenderPipelineAsync") void result.then(() => { record.completed = host.performance.now() }, () => { record.completed = host.performance.now(); record.failed = true })
  })
  return state
}
