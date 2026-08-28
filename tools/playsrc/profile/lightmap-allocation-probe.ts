/** Opt-in rgba32float API observations, not driver residency or bus traffic.
 * Weak keys and scalar records only: the probe cannot extend resource lifetime. */
export function installLightmapAllocationProbe(host: any = globalThis) {
  const state = { liveBytes: 0, peakBytes: 0, createdBytes: 0, destroyedBytes: 0, uploadBytes: 0, uploadMilliseconds: 0,
    events: [] as { kind: string; at: number; id: number; bytes: number; liveBytes: number; milliseconds: number }[] }
  const textures = new WeakMap<object, { id: number; bytes: number }>()
  let ordinal = 0
  const record = (kind: string, value: { id: number; bytes: number }, milliseconds = 0) => {
    if (state.events.length < 256) state.events.push({ kind, at: performance.now(), ...value, liveBytes: state.liveBytes, milliseconds })
  }
  const create = host.GPUDevice.prototype.createTexture, destroy = host.GPUTexture.prototype.destroy, write = host.GPUQueue.prototype.writeTexture
  host.GPUDevice.prototype.createTexture = function (...args: any[]) {
    const texture = create.apply(this, args)
    if (texture.format !== "rgba32float") return texture
    let bytes = 0
    for (let mip = 0; mip < texture.mipLevelCount; mip++) bytes += Math.max(1, Math.floor(texture.width / 2 ** mip))
      * Math.max(1, Math.floor(texture.height / 2 ** mip))
      * (texture.dimension === "3d" ? Math.max(1, Math.floor(texture.depthOrArrayLayers / 2 ** mip)) : texture.depthOrArrayLayers) * texture.sampleCount * 16
    const value = { id: ++ordinal, bytes }; textures.set(texture, value)
    state.liveBytes += bytes; state.createdBytes += bytes; state.peakBytes = Math.max(state.peakBytes, state.liveBytes)
    record("create", value)
    return texture
  }
  host.GPUTexture.prototype.destroy = function (...args: any[]) {
    const result = destroy.apply(this, args), value = textures.get(this)
    if (value) { textures.delete(this); state.liveBytes -= value.bytes; state.destroyedBytes += value.bytes; record("destroy", value) }
    return result
  }
  host.GPUQueue.prototype.writeTexture = function (...args: any[]) {
    const value = textures.get(args[0].texture)
    if (!value) return write.apply(this, args)
    const started = performance.now(), result = write.apply(this, args), milliseconds = performance.now() - started
    const bytes = args[1].byteLength
    state.uploadBytes += bytes; state.uploadMilliseconds += milliseconds; record("upload", { id: value.id, bytes }, milliseconds)
    return result
  }
  return state
}
