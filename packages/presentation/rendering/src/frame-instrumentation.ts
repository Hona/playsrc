export type RendererPassProfile = {
  identity: string
  submissions: number
  commandBuffers: number
  renderPasses: number
  drawCalls: number
  milliseconds: number
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
  bundleInvalidations: number
}>

export type BrowserFrameProfiler = {
  active: boolean
  currentPass: RendererPassProfile | null
  completedFrames: Record<string, unknown>[]
  counters: Record<string, number>
  capabilities: { timestampQuery: boolean; longAnimationFrame: boolean }
  losses: { kind: string; at: number; message: string }[]
  gpuTimestamps?: { frame: number; milliseconds: number }[]
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
  #bundleInvalidations = 0

  constructor(info: RendererInfo, profile: BrowserFrameProfiler, features?: { has(feature: string): boolean }) {
    this.#info = info
    this.#profile = profile
    info.autoReset = false
    info.reset()
    profile.capabilities.timestampQuery = features?.has("timestamp-query") ?? false
  }

  pass<T>(identity: string, callback: () => T): T {
    if (!this.#profile.active) return callback()
    const prior = this.#profile.currentPass
    const pass: RendererPassProfile = { identity, submissions: 0, commandBuffers: 0, renderPasses: 0, drawCalls: 0, milliseconds: 0 }
    const draws = this.#info.render.drawCalls
    const started = performance.now()
    this.#profile.currentPass = pass
    try {
      return callback()
    } finally {
      pass.drawCalls = this.#info.render.drawCalls - draws
      pass.milliseconds = performance.now() - started
      this.#passes.push(pass)
      this.#profile.currentPass = prior
    }
  }

  poseUpload(bytes: number): void {
    if (this.#profile.active) this.#poseUploadBytes += bytes
  }

  indexUpload(bytes: number): void {
    if (this.#profile.active) this.#indexUploadBytes += bytes
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
        bundleInvalidations: this.#bundleInvalidations,
      }
    } finally {
      this.#passes = []
      this.#poseUploadBytes = 0
      this.#indexUploadBytes = 0
      this.#bundleInvalidations = 0
      this.#info.reset()
    }
  }
}
