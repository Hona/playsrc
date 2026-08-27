export type RendererPassProfile = {
  identity: string
  submissions: number
  commandBuffers: number
  renderPasses: number
  drawCalls: number
  milliseconds: number
  renderPipelines: number
  nodeBuilderMisses: number
}

export type RendererMemoryProfile = Readonly<{
  textures: number
  texturesSize: number
  geometries: number
  attributes: number
  attributesSize: number
  indexAttributes: number
  indexAttributesSize: number
  uniformBuffers: number
  uniformBuffersSize: number
  programs: number
  programsSize: number
  renderTargets: number
  total: number
}>

export type RendererFrameProfile = Readonly<{
  drawCalls: number
  frameCalls: number
  triangles: number
  timestampMilliseconds: number | null
  memory: RendererMemoryProfile
  passes: readonly RendererPassProfile[]
  poseUploadBytes: number
  indexUploadBytes: number
  indirectUploadBytes: number
  bundleInvalidations: number
  bundleEncodes: number
  bundleEncodeMilliseconds: number
}>

export type BrowserFrameProfiler = {
  active: boolean
  currentPass: RendererPassProfile | null
  completedFrames: Record<string, unknown>[]
  counters: Record<string, number>
  capabilities: { timestampQuery: boolean; longAnimationFrame: boolean }
  losses: { kind: string; at: number; message: string }[]
  gpuTimestamps?: { frame: number; milliseconds: number }[]
  nodeBuilds?: { at: number; milliseconds: number; pass: string | null; material: string; vertexCharacters: number; fragmentCharacters: number }[]
  captureModelPrograms?: boolean
  modelPreparation?: { started: number; ended?: number; models: { model: string; skin: number; pass: string }[] }
  firstModelUses?: { at: number; model: string; skin: number; identity: number; pass: string | null }[]
}

type RendererInfo = {
  autoReset: boolean
  reset(): void
  render: { drawCalls: number; frameCalls: number; triangles: number; timestamp?: number }
  memory: Partial<Record<keyof RendererMemoryProfile, number>>
}

const MEMORY_FIELDS = [
  "textures", "texturesSize", "geometries", "attributes", "attributesSize",
  "indexAttributes", "indexAttributesSize", "uniformBuffers", "uniformBuffersSize",
  "programs", "programsSize", "renderTargets", "total",
] as const satisfies readonly (keyof RendererMemoryProfile)[]

export function browserFrameProfiler(): BrowserFrameProfiler | undefined {
  return (globalThis as typeof globalThis & { __playsrcFrameProfiler?: BrowserFrameProfiler }).__playsrcFrameProfiler
}

export class RendererFrameInstrumentation {
  readonly #info: RendererInfo
  readonly #profile: BrowserFrameProfiler
  #passes: RendererPassProfile[] = []
  #poseUploadBytes = 0
  #indexUploadBytes = 0
  #indirectUploadBytes = 0
  #bundleInvalidations = 0
  #bundleEncodes = 0
  #bundleEncodeMilliseconds = 0

  constructor(info: RendererInfo, profile: BrowserFrameProfiler, features?: { has(feature: string): boolean }) {
    this.#info = info
    this.#profile = profile
    info.autoReset = false
    info.reset()
    profile.capabilities.timestampQuery = features?.has("timestamp-query") ?? false
    this.#bundleEncodes = profile.counters.bundleEncodes ?? 0
    this.#bundleEncodeMilliseconds = profile.counters.bundleEncodeMilliseconds ?? 0
  }

  pass<T>(identity: string, callback: () => T): T {
    if (!this.#profile.active) {
      if (!this.#profile.captureModelPrograms) return callback()
      const prior = this.#profile.currentPass
      this.#profile.currentPass = { identity, submissions: 0, commandBuffers: 0, renderPasses: 0, drawCalls: 0, milliseconds: 0, renderPipelines: 0, nodeBuilderMisses: 0 }
      try { return callback() } finally { this.#profile.currentPass = prior }
    }
    const prior = this.#profile.currentPass
    const pass: RendererPassProfile = { identity, submissions: 0, commandBuffers: 0, renderPasses: 0, drawCalls: 0, milliseconds: 0, renderPipelines: 0, nodeBuilderMisses: 0 }
    const draws = this.#info.render.drawCalls
    const pipelines = this.#profile.counters.renderPipelines ?? 0
    const misses = this.#profile.counters.nodeBuilderMisses ?? 0
    const started = performance.now()
    this.#profile.currentPass = pass
    try {
      return callback()
    } finally {
      pass.drawCalls = this.#info.render.drawCalls - draws
      pass.renderPipelines = (this.#profile.counters.renderPipelines ?? 0) - pipelines
      pass.nodeBuilderMisses = (this.#profile.counters.nodeBuilderMisses ?? 0) - misses
      pass.milliseconds = performance.now() - started
      this.#passes.push(pass)
      this.#profile.currentPass = prior
    }
  }

  poseUpload(bytes: number): void {
    if (this.#profile.active) this.#poseUploadBytes += bytes
  }

  dynamicModel(event: "created" | "reused" | "parked" | "disposed" | "materialCreated" | "materialDisposed" | "graphCreated"): void {
    if (!this.#profile.active) return
    const key = `dynamicModel.${event}`
    this.#profile.counters[key] = (this.#profile.counters[key] ?? 0) + 1
  }

  indexUpload(bytes: number): void {
    if (this.#profile.active) this.#indexUploadBytes += bytes
  }

  indirectUpload(bytes: number): void {
    if (this.#profile.active) this.#indirectUploadBytes += bytes
  }

  invalidateBundle(): void {
    if (this.#profile.active) this.#bundleInvalidations += 1
  }

  complete(): RendererFrameProfile | undefined {
    try {
      if (!this.#profile.active) return undefined
      const memory = {} as Record<keyof RendererMemoryProfile, number>
      for (const field of MEMORY_FIELDS) memory[field] = Number(this.#info.memory[field] ?? 0)
      const timestamp = this.#info.render.timestamp
      return {
        drawCalls: this.#info.render.drawCalls,
        frameCalls: this.#info.render.frameCalls,
        triangles: this.#info.render.triangles,
        timestampMilliseconds: this.#profile.capabilities.timestampQuery && typeof timestamp === "number" && timestamp > 0 ? timestamp : null,
        memory,
        passes: this.#passes,
        poseUploadBytes: this.#poseUploadBytes,
        indexUploadBytes: this.#indexUploadBytes,
        indirectUploadBytes: this.#indirectUploadBytes,
        bundleInvalidations: this.#bundleInvalidations,
        bundleEncodes: (this.#profile.counters.bundleEncodes ?? 0) - this.#bundleEncodes,
        bundleEncodeMilliseconds: (this.#profile.counters.bundleEncodeMilliseconds ?? 0) - this.#bundleEncodeMilliseconds,
      }
    } finally {
      this.#passes = []
      this.#poseUploadBytes = 0
      this.#indexUploadBytes = 0
      this.#indirectUploadBytes = 0
      this.#bundleInvalidations = 0
      this.#bundleEncodes = this.#profile.counters.bundleEncodes ?? 0
      this.#bundleEncodeMilliseconds = this.#profile.counters.bundleEncodeMilliseconds ?? 0
      this.#info.reset()
    }
  }
}

export function installNodeBuilderInstrumentation(
  manager: { _createNodeBuilder: (...arguments_: any[]) => any },
  profile: BrowserFrameProfiler,
): () => void {
  const original = manager._createNodeBuilder
  if (typeof original !== "function") throw new Error("WebGPU node-builder instrumentation backend is unavailable")
  manager._createNodeBuilder = function (...arguments_: any[]): any {
    if (!profile.active) return original.apply(this, arguments_)
    profile.counters.nodeBuilderMisses = (profile.counters.nodeBuilderMisses ?? 0) + 1
    const builder = original.apply(this, arguments_)
    const build = builder.build
    if (typeof build !== "function") throw new Error("WebGPU node-builder build contract is unavailable")
    builder.build = function (...args: any[]) {
      if (!profile.active) return build.apply(this, args)
      const started = performance.now()
      try { return build.apply(this, args) }
      finally {
        const milliseconds = performance.now() - started
        profile.counters.nodeBuilderMilliseconds = (profile.counters.nodeBuilderMilliseconds ?? 0) + milliseconds
        const records = profile.nodeBuilds ??= []
        if (records.length < 512) records.push({ at: started, milliseconds, pass: profile.currentPass?.identity ?? null,
          material: String(arguments_[0]?.object?.userData?.materialIdentity ?? arguments_[1]?.name ?? "").slice(0, 256),
          vertexCharacters: this.vertexShader?.length ?? 0, fragmentCharacters: this.fragmentShader?.length ?? 0 })
        else profile.counters.nodeBuildsDropped = (profile.counters.nodeBuildsDropped ?? 0) + 1
      }
    }
    return builder
  }
  return () => { manager._createNodeBuilder = original }
}
