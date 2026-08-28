/** Serializable, opt-in API lifecycle accounting. No GPU object is retained by
 * a counter. Returned texture objects are not evidence of physical residency:
 * validation, implicit GC, device loss and driver retirement are not measured. */
export function installGpuTextureAccounting(host: any = globalThis) {
  if (host.__playsrcGpuTextureAccounting) return host.__playsrcGpuTextureAccounting as ReturnType<typeof totalState>
  type FormatTotal = { textures: number; knownBytes: number; unknownByteTextures: number }
  const total = () => ({ textures: 0, knownBytes: 0, unknownByteTextures: 0,
    compressedTextures: 0, compressedBytes: 0, formats: {} as Record<string, FormatTotal> })
  const totalState = () => ({ schema: "playsrc-gpu-texture-api-allocation-v1", interpretation:
    "Explicit API object creation/destruction; knownBytes excludes implementation-defined/unknown format sizes and driver padding. Not physical GPU residency. Upload source bytes are writeTexture input spans, not measured bus traffic.",
    live: total(), created: total(), destroyedTextures: 0, peakKnownBytes: 0,
    writeTextureCalls: 0, writeTextureSourceBytes: 0 })
  const state = totalState()
  const allocations = new WeakMap<object, { format: string; bytes: number | null; compressed: boolean }>()
  // WebGPU texel block sizes. Opaque depth formats are intentionally unknown,
  // not guessed from a nominal depth precision or the host's allocation size.
  // https://www.w3.org/TR/webgpu/#texture-format-caps
  const scalar = new Map<string, number>()
  for (const [bytes, formats] of [
    [1, "r8unorm r8snorm r8uint r8sint stencil8"],
    [2, "r16uint r16sint r16float r16unorm r16snorm rg8unorm rg8snorm rg8uint rg8sint depth16unorm"],
    [4, "r32uint r32sint r32float rg16uint rg16sint rg16float rg16unorm rg16snorm rgba8unorm rgba8unorm-srgb rgba8snorm rgba8uint rgba8sint bgra8unorm bgra8unorm-srgb rgb9e5ufloat rgb10a2uint rgb10a2unorm rg11b10ufloat depth32float"],
    [8, "rg32uint rg32sint rg32float rgba16uint rgba16sint rgba16float rgba16unorm rgba16snorm"],
    [16, "rgba32uint rgba32sint rgba32float"],
  ] as const) for (const format of formats.split(" ")) scalar.set(format, bytes)
  const block = (format: string): readonly [number, number, number] | undefined => {
    if (/^bc(?:1-rgba|4-r)-(?:unorm(?:-srgb)?|snorm)$/.test(format)) return [4, 4, 8]
    if (/^bc(?:2-rgba|3-rgba|5-rg|6h-rgb|7-rgba)-(?:unorm(?:-srgb)?|snorm|ufloat|float)$/.test(format)) return [4, 4, 16]
    if (/^(?:etc2-rgb8|etc2-rgb8a1)-unorm(?:-srgb)?$/.test(format) || /^eac-r11-(?:unorm|snorm)$/.test(format)) return [4, 4, 8]
    if (/^etc2-rgba8-unorm(?:-srgb)?$/.test(format) || /^eac-rg11-(?:unorm|snorm)$/.test(format)) return [4, 4, 16]
    const astc = /^astc-(4x4|5x4|5x5|6x5|6x6|8x5|8x6|8x8|10x5|10x6|10x8|10x10|12x10|12x12)-unorm(?:-srgb)?$/.exec(format)
    if (astc) { const [width, height] = astc[1]!.split("x").map(Number); return [width!, height!, 16] }
    const bytes = scalar.get(format)
    return bytes === undefined ? undefined : [1, 1, bytes]
  }
  const add = (target: ReturnType<typeof total>, record: { format: string; bytes: number | null; compressed: boolean }, sign: number) => {
    const format = target.formats[record.format] ??= { textures: 0, knownBytes: 0, unknownByteTextures: 0 }
    target.textures += sign; format.textures += sign
    if (record.bytes === null) { target.unknownByteTextures += sign; format.unknownByteTextures += sign }
    else { target.knownBytes += sign * record.bytes; format.knownBytes += sign * record.bytes }
    if (record.compressed) { target.compressedTextures += sign; target.compressedBytes += sign * (record.bytes ?? 0) }
  }
  const create = host.GPUDevice?.prototype?.createTexture, destroy = host.GPUTexture?.prototype?.destroy
  const write = host.GPUQueue?.prototype?.writeTexture
  if ([create, destroy, write].some(method => typeof method !== "function")) throw new Error("GPU texture lifecycle accounting requires native creation/destruction/writeTexture")
  Object.defineProperty(host.GPUDevice.prototype, "createTexture", { configurable: true, writable: true, value(this: any, ...args: any[]) {
    const texture = create.apply(this, args)
    // Read normalized, immutable API properties, never consume a descriptor's
    // iterable or getters for a second time after native validation.
    const format = texture.format as string, layout = block(format)
    let bytes: number | null = layout ? 0 : null
    if (layout) for (let mip = 0; mip < texture.mipLevelCount; mip++) {
      const width = Math.max(1, Math.floor(texture.width / 2 ** mip))
      const height = Math.max(1, Math.floor(texture.height / 2 ** mip))
      const depth = texture.dimension === "3d" ? Math.max(1, Math.floor(texture.depthOrArrayLayers / 2 ** mip)) : texture.depthOrArrayLayers
      bytes! += Math.ceil(width / layout[0]) * Math.ceil(height / layout[1]) * layout[2] * depth * texture.sampleCount
    }
    if (bytes !== null && (!Number.isSafeInteger(bytes) || bytes < 0)) bytes = null
    const record = { format, bytes, compressed: layout !== undefined && layout[0] > 1 }
    allocations.set(texture, record)
    add(state.live, record, 1); add(state.created, record, 1)
    state.peakKnownBytes = Math.max(state.peakKnownBytes, state.live.knownBytes)
    return texture
  } })
  Object.defineProperty(host.GPUTexture.prototype, "destroy", { configurable: true, writable: true, value(this: any, ...args: any[]) {
    const value = destroy.apply(this, args)
    const record = allocations.get(this)
    if (record) { allocations.delete(this); add(state.live, record, -1); state.destroyedTextures++ }
    return value
  } })
  Object.defineProperty(host.GPUQueue.prototype, "writeTexture", { configurable: true, writable: true, value(this: any, ...args: any[]) {
    const value = write.apply(this, args)
    state.writeTextureCalls++; state.writeTextureSourceBytes += args[1].byteLength
    return value
  } })
  Object.defineProperty(host, "__playsrcGpuTextureAccounting", { value: state, configurable: true })
  return state
}
