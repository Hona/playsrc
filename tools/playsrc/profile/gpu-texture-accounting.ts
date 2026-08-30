/** Serializable, opt-in API lifecycle accounting. No GPU object is retained by
 * a counter. Returned texture objects are not evidence of physical residency:
 * validation, implicit GC, device loss and driver retirement are not measured. */
export function installGpuTextureAccounting(host: any = globalThis, traceOwners: boolean | "attachments" = false) {
  if (host.__playsrcGpuTextureAccounting) return host.__playsrcGpuTextureAccounting as ReturnType<typeof totalState>
  type FormatTotal = { textures: number; knownBytes: number; unknownByteTextures: number }
  const total = () => ({ textures: 0, knownBytes: 0, unknownByteTextures: 0,
    compressedTextures: 0, compressedBytes: 0, formats: {} as Record<string, FormatTotal> })
  const totalState = () => ({ schema: "playsrc-gpu-texture-api-allocation-v1", interpretation:
    "Live means created minus explicit destroys; implicit GC, device loss, validation and driver retirement are not observed. knownBytes excludes implementation-defined/unknown format sizes and driver padding. Not physical GPU residency. created/destroyed/writeTexture counters are cumulative; upload source bytes are input spans, not measured bus traffic.",
    live: total(), created: total(), destroyedTextures: 0, peakKnownBytes: 0,
    writeTextureCalls: 0, writeTextureSourceBytes: 0 })
  const state = totalState()
  const allocations = new WeakMap<object, { format: string; bytes: number | null; compressed: boolean; id: number; owner: string; usage: number }>()
  const owners = { records: [] as any[], dropped: 0 }
  if (traceOwners) host.__playsrcTextureOwners = owners
  let ordinal = 0
  const recordOwner = (record: { kind: string; usage?: number; [key: string]: unknown }) => {
    if (!traceOwners) return
    // Whole-soak ownership evidence needs attachment creation/retirement, not
    // per-frame upload/pass records. Aggregate upload counters remain intact.
    if (traceOwners === "attachments" && (!(Number(record.usage) & 16) || !["create", "destroy"].includes(record.kind))) return
    if (owners.records.length < 16384) owners.records.push({ at: performance.now(), ...record })
    else owners.dropped++
  }
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
    if (["bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm"].includes(format)) return [4, 4, 8]
    if (["bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb"].includes(format)) return [4, 4, 16]
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
    const record = { format, bytes, compressed: layout !== undefined && layout[0] > 1, id: ++ordinal, owner: texture.label || "unlabelled", usage: texture.usage }
    allocations.set(texture, record)
    add(state.live, record, 1); add(state.created, record, 1)
    state.peakKnownBytes = Math.max(state.peakKnownBytes, state.live.knownBytes)
    recordOwner({ kind: "create", ...record, width: texture.width, height: texture.height, depth: texture.depthOrArrayLayers,
      mips: texture.mipLevelCount, samples: texture.sampleCount, usage: texture.usage })
    return texture
  } })
  Object.defineProperty(host.GPUTexture.prototype, "destroy", { configurable: true, writable: true, value(this: any, ...args: any[]) {
    const value = destroy.apply(this, args)
    const record = allocations.get(this)
    if (record) { allocations.delete(this); add(state.live, record, -1); state.destroyedTextures++; recordOwner({ kind: "destroy", ...record }) }
    return value
  } })
  Object.defineProperty(host.GPUQueue.prototype, "writeTexture", { configurable: true, writable: true, value(this: any, ...args: any[]) {
    const value = write.apply(this, args)
    state.writeTextureCalls++; state.writeTextureSourceBytes += args[1].byteLength
    const record = allocations.get(args[0].texture)
    recordOwner({ kind: "upload", id: record?.id ?? null, bytes: args[1].byteLength })
    return value
  } })
  if (traceOwners === true && host.GPUTexture.prototype.createView && host.GPUCommandEncoder?.prototype.beginRenderPass) {
    const views = new WeakMap<object, { id: number; owner: string }>()
    const view = host.GPUTexture.prototype.createView, pass = host.GPUCommandEncoder.prototype.beginRenderPass
    host.GPUTexture.prototype.createView = function (...args: any[]) {
      const result = view.apply(this, args), record = allocations.get(this)
      if (record?.owner.startsWith("playsrc-water-")) views.set(result, { id: record.id, owner: record.owner })
      return result
    }
    host.GPUCommandEncoder.prototype.beginRenderPass = function (...args: any[]) {
      const result = pass.apply(this, args)
      // The checked Three owner supplies an ordinary array, not an iterable
      // descriptor whose native consumption could be repeated by the probe.
      if (Array.isArray(args[0].colorAttachments)) for (const attachment of args[0].colorAttachments) {
        const record = attachment && views.get(attachment.view)
        if (record) recordOwner({ kind: "attachment", ...record, loadOp: attachment.loadOp, storeOp: attachment.storeOp })
      }
      return result
    }
  }
  Object.defineProperty(host, "__playsrcGpuTextureAccounting", { value: state, configurable: true })
  return state
}
