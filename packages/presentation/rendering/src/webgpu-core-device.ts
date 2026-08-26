const THREE_WEBGPU_FEATURES = [
  "core-features-and-limits", "depth-clip-control", "depth32float-stencil8",
  "texture-compression-bc", "texture-compression-bc-sliced-3d",
  "texture-compression-etc2", "texture-compression-astc", "texture-compression-astc-sliced-3d",
  "timestamp-query", "indirect-first-instance", "shader-f16", "rg11b10ufloat-renderable",
  "bgra8unorm-storage", "float32-filterable", "float32-blendable", "clip-distances",
  "dual-source-blending", "subgroups", "texture-formats-tier1", "texture-formats-tier2",
] as const

export type WebGpuCoreAdapter = {
  features: { has(feature: string): boolean }
  info?: { vendor?: string; architecture?: string; device?: string; description?: string; isFallbackAdapter?: boolean }
  limits?: { maxTextureDimension2D?: number; maxBufferSize?: number }
  requestDevice(descriptor: { requiredFeatures: string[] }): Promise<any>
}

export async function requestCoreWebGpuDevice(
  gpu: { requestAdapter(options: { powerPreference?: "low-power" | "high-performance" }): Promise<WebGpuCoreAdapter | null> },
  powerPreference?: "low-power" | "high-performance",
): Promise<{ adapter: WebGpuCoreAdapter; device: any; features: string[] }> {
  // Omitting featureLevel requests the WebGPU core profile. Three.js currently
  // requests compatibility explicitly, which disables MSAA and core limits.
  const adapter = await gpu.requestAdapter(powerPreference ? { powerPreference } : {})
  if (!adapter) throw new Error("WebGPU core hardware adapter is unavailable")
  const features = THREE_WEBGPU_FEATURES.filter(feature => adapter.features.has(feature))
  const device = await adapter.requestDevice({ requiredFeatures: features })
  return { adapter, device, features }
}
