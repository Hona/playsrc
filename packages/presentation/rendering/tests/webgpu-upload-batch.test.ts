import { describe, expect, test } from "bun:test"
import { WebGpuUploadBatch, withReferenceGpuUploads, type UploadBatchBackend } from "../src/webgpu-upload-batch"
import { WebGpuSubmissionBatch } from "../src/webgpu-submission-batch"
import { ownSharedUpload } from "../src/shared-upload"

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
  return { backend, queue, writes, submissions, copies, allocated, originals, buffer, metadata }
}

describe("persistent WebGPU frame upload batching", () => {
  test("reference transport bypasses batching without corrupting retained owner revisions", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = Uint32Array.from([1, 2])
    ownSharedUpload(palette)
    const binding = { buffer: palette, updateRanges: [] }
    state.buffer(binding)
    state.backend.updateBinding(binding)
    expect(() => withReferenceGpuUploads(state.backend, () => {})).toThrow("submitted")
    state.queue.submit(["first"])
    expect(() => withReferenceGpuUploads(state.backend, () => {
      state.backend.updateBinding(binding)
      throw new Error("reference draw failed")
    })).toThrow("reference draw failed")
    expect(state.originals.bindings).toBe(1)
    state.backend.updateBinding(binding)
    state.queue.submit(["unchanged"])
    expect(state.writes).toHaveLength(1)
    batch.dispose()
    expect(() => withReferenceGpuUploads(state.backend, () => {})).toThrow("owner is unavailable")
  })
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

  test("uploads an actor-owned skeleton palette once across distinct material bindings", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = Float32Array.from([1, 0, 0, 4, 0, 1, 0, 5, 0, 0, 1, 6, 0, 0, 0, 1])
    ownSharedUpload(palette)
    const bindings = Array.from({ length: 4 }, () => ({ buffer: palette, updateRanges: [] }))
    const destinations = bindings.map(binding => state.buffer(binding))
    for (const binding of bindings) state.backend.updateBinding(binding)
    state.queue.submit(["world"])
    expect(state.writes).toHaveLength(1)
    expect(state.writes[0]!.bytes).toHaveLength(palette.byteLength)
    expect(state.copies).toEqual(destinations.map(destination => ({ sourceOffset: 0, destination, destinationOffset: 0, size: palette.byteLength })))
    batch.dispose()
  })

  test("preserves distinct views and changed shared palettes in submission order", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const backing = Uint32Array.from([1, 2, 3, 4])
    const first = { buffer: backing.subarray(0, 2), updateRanges: [] }
    const second = { buffer: backing.subarray(2, 4), updateRanges: [] }
    const changed = { buffer: backing.subarray(0, 2), updateRanges: [] }
    for (const binding of [first, second, changed]) state.buffer(binding)
    state.backend.updateBinding(first)
    state.backend.updateBinding(second)
    backing[0] = 9
    state.backend.updateBinding(changed)
    state.queue.submit(["render"])
    expect(new Uint32Array(Uint8Array.from(state.writes[0]!.bytes).buffer)).toEqual(Uint32Array.from([1, 2, 3, 4, 9, 2]))
    expect(state.copies.map(copy => copy.sourceOffset)).toEqual([0, 8, 16])
    batch.dispose()
  })

  test("never overwrites a shared staging snapshot when the last destination changes", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = Uint32Array.from([1, 2])
    const publish = ownSharedUpload(palette)
    const first = { buffer: palette, updateRanges: [] }
    const second = { buffer: palette, updateRanges: [] }
    const firstDestination = state.buffer(first)
    const secondDestination = state.buffer(second)
    state.backend.updateBinding(first)
    state.backend.updateBinding(second)
    palette[0] = 9
    publish()
    state.backend.updateBinding(second)
    state.queue.submit(["render"])
    expect(new Uint32Array(Uint8Array.from(state.writes[0]!.bytes).buffer)).toEqual(Uint32Array.from([1, 2, 9, 2]))
    expect(state.copies).toEqual([
      { sourceOffset: 0, destination: firstDestination, destinationOffset: 0, size: 8 },
      { sourceOffset: 0, destination: secondDestination, destinationOffset: 0, size: 8 },
      { sourceOffset: 8, destination: secondDestination, destinationOffset: 0, size: 8 },
    ])
    batch.dispose()
  })

  test("does not merge destination-adjacent copies whose shared staging ranges are separated", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = Uint32Array.from([1, 2, 3, 4])
    const shared = palette.subarray(0, 2)
    ownSharedUpload(shared)
    const first = { buffer: shared, updateRanges: [] }
    const unrelated = { buffer: Uint32Array.from([5, 6]), updateRanges: [] }
    const second = { buffer: shared, updateRanges: [] }
    state.buffer(first)
    state.buffer(unrelated)
    const destination = state.buffer(second)
    state.backend.updateBinding(first)
    state.backend.updateBinding(unrelated)
    state.backend.updateBinding(second)
    const attribute = { array: palette, updateRanges: [{ start: 2, count: 2 }], clearUpdateRanges() { this.updateRanges = [] } }
    state.metadata.set(attribute, { buffer: destination })
    state.backend.updateAttribute(attribute)
    state.queue.submit(["render"])
    expect(state.copies.filter(copy => copy.destination === destination)).toEqual([
      { sourceOffset: 0, destination, destinationOffset: 0, size: 8 },
      { sourceOffset: 16, destination, destinationOffset: 8, size: 8 },
    ])
    batch.dispose()
  })

  test("shares only explicit owner revisions and invalidates a rebound destination", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = Uint32Array.from([1, 2])
    const publish = ownSharedUpload(palette)
    expect(() => ownSharedUpload(palette)).toThrow("already has an owner")
    const binding = { buffer: palette, updateRanges: [] }
    state.buffer(binding)
    state.backend.updateBinding(binding)
    state.queue.submit(["world"])
    state.backend.updateBinding(binding)
    state.queue.submit(["reflection"])
    expect(state.writes).toHaveLength(1)
    binding.buffer = Uint32Array.from([5, 6])
    state.backend.updateBinding(binding)
    state.queue.submit(["other"])
    binding.buffer = palette
    state.backend.updateBinding(binding)
    state.queue.submit(["restored"])
    expect(state.writes).toHaveLength(3)
    palette[1] = 9
    publish()
    state.backend.updateBinding(binding)
    state.queue.submit(["next-pose"])
    expect(new Uint32Array(Uint8Array.from(state.writes[3]!.bytes).buffer)).toEqual(Uint32Array.from([1, 9]))
    batch.dispose()
  })

  test("uploads exact dirty matrix runs, but fully initializes new and skipped-generation bindings", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const palette = new Float32Array(3 * 16)
    const publish = ownSharedUpload(palette)
    const first = { buffer: palette, updateRanges: [] }
    const skipped = { buffer: palette, updateRanges: [] }
    state.buffer(first)
    state.buffer(skipped)
    state.backend.updateBinding(first)
    state.backend.updateBinding(skipped)
    state.queue.submit(["first"])
    palette[16] = 2
    publish([{ start: 64, count: 64 }])
    state.backend.updateBinding(first)
    state.queue.submit(["sparse"])
    expect(state.writes[1]!.bytes).toHaveLength(64)
    expect(state.copies.at(-1)).toMatchObject({ destinationOffset: 64, size: 64 })
    palette[32] = 3
    publish([{ start: 128, count: 64 }])
    const fresh = { buffer: palette, updateRanges: [] }
    state.buffer(fresh)
    state.backend.updateBinding(fresh)
    state.backend.updateBinding(first)
    state.backend.updateBinding(skipped)
    state.queue.submit(["replacement"])
    expect(state.writes[2]!.bytes).toHaveLength(palette.byteLength)
    expect(state.copies.slice(-3).map(({ sourceOffset, destinationOffset, size }) => ({ sourceOffset, destinationOffset, size }))).toEqual([
      { sourceOffset: 0, destinationOffset: 0, size: 192 },
      { sourceOffset: 128, destinationOffset: 128, size: 64 },
      { sourceOffset: 0, destinationOffset: 0, size: 192 },
    ])
    expect(() => publish([{ start: 1, count: 64 }])).toThrow("ranges are invalid")
    expect(() => publish([{ start: 64, count: 65 }])).toThrow("ranges are invalid")
    expect(() => publish([{ start: 128, count: 128 }])).toThrow("ranges are invalid")
    expect(() => publish([{ start: 64, count: 64 }, { start: 0, count: 64 }])).toThrow("ranges are invalid")
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

  test("drops retired binding destinations before their staged copy can invalidate the GPU queue", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const staleBinding = { buffer: Uint32Array.from([1]), updateRanges: [] }
    const liveBinding = { buffer: Uint32Array.from([2]), updateRanges: [] }
    const stale = state.buffer(staleBinding)
    const live = state.buffer(liveBinding)
    state.backend.updateBinding(staleBinding)
    state.backend.updateBinding(liveBinding)
    stale.destroy()
    state.queue.submit(["render"])
    expect(state.copies).toEqual([{ sourceOffset: 4, destination: live, destinationOffset: 0, size: 4 }])
    expect(state.submissions).toEqual([[{ upload: true }, "render"]])
    batch.dispose()
  })

  test("submits an unchanged render without creating staging after every pending destination is destroyed", () => {
    const state = fixture()
    const batch = new WebGpuUploadBatch(state.backend)
    const binding = { buffer: Uint32Array.from([1]), updateRanges: [] }
    const destination = state.buffer(binding)
    state.backend.updateBinding(binding)
    destination.destroy()
    state.queue.submit(["render"])
    expect(state.allocated).toHaveLength(0)
    expect(state.copies).toHaveLength(0)
    expect(state.submissions).toEqual([["render"]])
    batch.dispose()
  })

  test("preserves upload-before-draw ordering while combining adjacent clean render submissions", () => {
    const state = fixture()
    const submissions = new WebGpuSubmissionBatch(state.queue)
    const uploads = new WebGpuUploadBatch(state.backend)
    const binding = { buffer: Uint32Array.from([1]), updateRanges: [{ start: 0, count: 1 }] }
    state.buffer(binding)
    submissions.begin()
    state.backend.updateBinding(binding)
    state.queue.submit(["world"])
    state.queue.submit(["viewmodel"])
    binding.buffer[0] = 2
    state.backend.updateBinding(binding)
    state.queue.submit(["hud"])
    submissions.finish()
    expect(state.submissions).toEqual([
      [{ upload: true }, "world", "viewmodel"],
      [{ upload: true }, "hud"],
    ])
    expect(state.writes).toHaveLength(2)
    uploads.dispose()
    submissions.dispose()
  })
})
