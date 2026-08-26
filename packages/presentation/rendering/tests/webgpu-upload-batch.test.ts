import { describe, expect, test } from "bun:test"
import { WebGpuUploadBatch, type UploadBatchBackend } from "../src/webgpu-upload-batch"

function fixture() {
  const writes: { buffer: any; offset: number; bytes: number[] }[] = []
  const submissions: unknown[][] = []
  const copies: { sourceOffset: number; destination: any; destinationOffset: number; size: number }[] = []
  const allocated: any[] = []
  const queue = {
    writeBuffer(buffer: any, offset: number, data: Uint8Array, dataOffset = 0, size = data.length - dataOffset) {
      writes.push({ buffer, offset, bytes: [...data.subarray(dataOffset, dataOffset + size)] })
    },
    submit(buffers: Iterable<unknown>) { submissions.push([...buffers]) },
  }
  const metadata = new WeakMap<object, any>()
  const originals = { bindings: 0, attributes: 0 }
  const backend: UploadBatchBackend = {
    device: {
      queue,
      createBuffer(descriptor) {
        const result = { ...descriptor, destroyed: false, destroy() { result.destroyed = true } }
        allocated.push(result)
        return result
      },
      createCommandEncoder() {
        return {
          copyBufferToBuffer(_source, sourceOffset, destination, destinationOffset, size) { copies.push({ sourceOffset, destination, destinationOffset, size }) },
          finish() { return { upload: true } },
        }
      },
    },
    get(identity) { return metadata.get(identity) ?? {} },
    updateBinding() { originals.bindings += 1 },
    updateAttribute() { originals.attributes += 1 },
  }
  const buffer = (identity: object, extra = {}) => {
    const destination = { label: "destination", destroy() {} }
    metadata.set(identity, { buffer: destination, ...extra })
    return destination
  }
  return { backend, queue, writes, submissions, copies, allocated, originals, buffer }
}

describe("persistent WebGPU frame upload batching", () => {
  test("packs distinct uniforms and posed attributes into one real queue write and preserves command order", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const binding = { buffer: Uint32Array.from([1, 2, 3, 4]), updateRanges: [{ start: 0, count: 1 }, { start: 2, count: 2 }] }
    const uniform = state.buffer(binding)
    const attribute = { array: Uint32Array.from([8, 9, 10]), updateRanges: [{ start: 1, count: 2 }], clearUpdateRanges() { this.updateRanges = [] } }
    const vertices = state.buffer(attribute)
    state.backend.updateBinding(binding)
    state.backend.updateAttribute(attribute)
    expect(state.writes).toHaveLength(0)
    state.queue.submit(["render"])
    expect(state.writes).toHaveLength(1)
    expect(new Uint32Array(Uint8Array.from(state.writes[0]!.bytes).buffer)).toEqual(Uint32Array.from([1, 3, 4, 9, 10]))
    expect(state.copies).toEqual([
      { sourceOffset: 0, destination: uniform, destinationOffset: 0, size: 4 },
      { sourceOffset: 4, destination: uniform, destinationOffset: 8, size: 8 },
      { sourceOffset: 12, destination: vertices, destinationOffset: 4, size: 8 },
    ])
    expect(state.submissions).toEqual([[{ upload: true }, "render"]])
    expect(attribute.updateRanges).toEqual([])
    batch.dispose()
  })

  test("caches exact immutable uniforms, uploads changed ranges, and merges contiguous ranges", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const binding = { buffer: Uint32Array.from([1, 2, 3]), updateRanges: [{ start: 0, count: 1 }, { start: 1, count: 1 }] }
    state.buffer(binding)
    state.backend.updateBinding(binding)
    state.queue.submit(["first"])
    expect(state.copies[0]).toMatchObject({ destinationOffset: 0, size: 8 })
    state.backend.updateBinding(binding)
    state.queue.submit(["unchanged"])
    expect(state.writes).toHaveLength(1)
    expect(state.submissions[1]).toEqual(["unchanged"])
    binding.buffer[1] = 7
    binding.updateRanges = [{ start: 1, count: 1 }]
    state.backend.updateBinding(binding)
    state.queue.submit(["changed"])
    expect(state.writes).toHaveLength(2)
    expect(state.copies[1]).toMatchObject({ destinationOffset: 4, size: 4 })
    expect(state.allocated).toHaveLength(1)
    batch.dispose()
  })

  test("preserves padded backend ownership and restores the original queue on disposal", () => {
    const state = fixture()
    const original = state.queue.submit
    const batch = new WebGpuUploadBatch(state.backend)
    const attribute = { array: Uint32Array.from([1]), updateRanges: [], clearUpdateRanges() {} }
    state.buffer(attribute, { _paddedItemSize: 4 })
    state.backend.updateAttribute(attribute)
    expect(state.originals.attributes).toBe(1)
    batch.dispose()
    expect(state.queue.submit).toBe(original)
    state.backend.updateAttribute(attribute)
    expect(state.originals.attributes).toBe(2)
  })
})
