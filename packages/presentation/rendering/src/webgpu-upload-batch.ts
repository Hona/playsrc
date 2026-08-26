import { sharedUpload } from "./shared-upload"

type UploadRange = Readonly<{ start: number; count: number }>

type UploadArray = ArrayBuffer | ArrayBufferView

type UploadBuffer = { readonly size?: number; readonly label?: string; destroy(): void }

type UploadEncoder = {
  copyBufferToBuffer(source: UploadBuffer, sourceOffset: number, destination: UploadBuffer, destinationOffset: number, size: number): void
  finish(): unknown
}

type UploadQueue = {
  writeBuffer(buffer: UploadBuffer, offset: number, data: Uint8Array, dataOffset?: number, size?: number): void
  submit(buffers: Iterable<unknown>): void
  onSubmittedWorkDone?(): Promise<unknown>
}

type UploadDevice = {
  readonly queue: UploadQueue
  createBuffer(descriptor: { label: string; size: number; usage: number }): UploadBuffer
  createCommandEncoder(descriptor: { label: string }): UploadEncoder
}

type UploadAttribute = {
  array: UploadArray
  updateRanges: UploadRange[]
  clearUpdateRanges(): void
  isInterleavedBufferAttribute?: boolean
  data?: UploadAttribute
}

type UploadBinding = { buffer: UploadArray; updateRanges: UploadRange[]; name?: string }

export type UploadBatchBackend = {
  readonly device: UploadDevice
  get(identity: object): { buffer?: UploadBuffer; _paddedItemSize?: number }
  updateBinding(binding: UploadBinding): void
  updateAttribute(attribute: UploadAttribute): void
}

type PendingCopy = { destination: UploadBuffer; sourceOffset: number; destinationOffset: number; size: number }
type SharedUpload = NonNullable<ReturnType<typeof sharedUpload>>
type StagedSource = Readonly<{ offset: number; revision: number; start: number; size: number }>

const COPY_SRC = 0x04
const COPY_DST = 0x08
const INITIAL_CAPACITY = 65_536
const BATCHES = new WeakMap<UploadBatchBackend, WebGpuUploadBatch>()

// Used by headed parity capture to draw the identical scene with the native
// full-buffer transport, without changing shaders, poses, or retained shadows.
export function withReferenceGpuUploads<T>(backend: UploadBatchBackend, draw: () => T): T {
  const batch = BATCHES.get(backend)
  if (!batch) throw new Error("WebGPU reference upload owner is unavailable")
  return batch.reference(draw)
}

function bytesOf(array: UploadArray): Uint8Array {
  return ArrayBuffer.isView(array)
    ? new Uint8Array(array.buffer, array.byteOffset, array.byteLength)
    : new Uint8Array(array)
}

function equalRange(left: Uint8Array, right: Uint8Array, start: number, end: number): boolean {
  for (let index = start; index < end; index += 1) if (left[index] !== right[index]) return false
  return true
}

export class WebGpuUploadBatch {
  readonly #backend: UploadBatchBackend
  readonly #queue: UploadQueue
  readonly #submit: UploadQueue["submit"]
  readonly #submitDescriptor: PropertyDescriptor | undefined
  readonly #updateBinding: UploadBatchBackend["updateBinding"]
  readonly #updateAttribute: UploadBatchBackend["updateAttribute"]
  readonly #shadows = new WeakMap<UploadBuffer, Uint8Array>()
  readonly #trackedDestinations = new WeakSet<UploadBuffer>()
  readonly #copies: PendingCopy[] = []
  readonly #retired = new Set<UploadBuffer>()
  readonly #sharedDestinations = new WeakMap<UploadBuffer, Readonly<{ source: SharedUpload; revision: number }>>()
  readonly #sources = new Map<SharedUpload, StagedSource[]>()
  #bytes = new Uint8Array(INITIAL_CAPACITY)
  #length = 0
  #staging?: UploadBuffer
  #disposed = false

  constructor(backend: UploadBatchBackend) {
    if (!backend.device?.queue || typeof backend.updateBinding !== "function" || typeof backend.updateAttribute !== "function") {
      throw new Error("WebGPU upload batching backend is unavailable")
    }
    this.#backend = backend
    this.#queue = backend.device.queue
    this.#submit = this.#queue.submit
    this.#submitDescriptor = Object.getOwnPropertyDescriptor(this.#queue, "submit")
    this.#updateBinding = backend.updateBinding
    this.#updateAttribute = backend.updateAttribute
    BATCHES.set(backend, this)
    backend.updateBinding = binding => this.#binding(binding)
    backend.updateAttribute = attribute => this.#attribute(attribute)
    Object.defineProperty(this.#queue, "submit", {
      configurable: true,
      writable: true,
      value: (buffers: Iterable<unknown>) => {
        const upload = this.#flush()
        this.#submit.call(this.#queue, upload ? [upload, ...buffers] : buffers)
      },
    })
  }

  reference<T>(draw: () => T): T {
    if (this.#copies.length) throw new Error("reference GPU draw requires submitted staged uploads")
    const binding = this.#backend.updateBinding
    const attribute = this.#backend.updateAttribute
    this.#backend.updateBinding = value => sharedUpload(value.buffer)
      ? this.#updateBinding.call(this.#backend, value)
      : binding(value)
    try { return draw() }
    finally {
      this.#backend.updateBinding = binding
      this.#backend.updateAttribute = attribute
    }
  }

  #binding(binding: UploadBinding): void {
    const destination = this.#backend.get(binding).buffer
    if (!destination) return this.#updateBinding.call(this.#backend, binding)
    const buffer = binding.buffer
    const shared = sharedUpload(buffer)
    if (shared && binding.updateRanges.length === 0) {
      const prior = this.#sharedDestinations.get(destination)
      if (prior?.source === shared && prior.revision === shared.revision) return
      const bytes = bytesOf(buffer)
      if (prior?.source === shared && prior.revision === shared.revision - 1 && shared.ranges.length) {
        for (const range of shared.ranges) this.#stage(destination, range.start, bytes, range.start, range.count, shared)
      } else {
        this.#stage(destination, 0, bytes, 0, bytes.length, shared)
      }
      this.#sharedDestinations.set(destination, { source: shared, revision: shared.revision })
      this.#shadows.delete(destination)
      return
    }
    this.#sharedDestinations.delete(destination)
    const bytes = bytesOf(binding.buffer)
    let shadow = this.#shadows.get(destination)
    const initial = shadow === undefined || shadow.length !== bytes.length
    if (!shadow || shadow.length !== bytes.length) {
      shadow = new Uint8Array(bytes.length)
      this.#shadows.set(destination, shadow)
    }
    const elementSize = ArrayBuffer.isView(binding.buffer) ? (binding.buffer as { BYTES_PER_ELEMENT?: number }).BYTES_PER_ELEMENT ?? 1 : 1
    const ranges = binding.updateRanges.length ? binding.updateRanges : [{ start: 0, count: bytes.length / elementSize }]
    let first = -1
    let last = -1
    const emit = (): void => {
      if (first < 0) return
      if (initial || !equalRange(shadow!, bytes, first, last)) {
        shadow!.set(bytes.subarray(first, last), first)
        this.#stage(destination, first, bytes, first, last - first)
      }
      first = -1
      last = -1
    }
    for (const range of ranges) {
      const start = range.start * elementSize
      const end = start + range.count * elementSize
      if (first >= 0 && start > last) emit()
      if (first < 0) first = start
      last = Math.max(last, end)
    }
    emit()
  }

  #attribute(attribute: UploadAttribute): void {
    const identity = attribute.isInterleavedBufferAttribute ? attribute.data! : attribute
    const metadata = this.#backend.get(identity)
    if (!metadata.buffer || metadata._paddedItemSize !== undefined) {
      this.#updateAttribute.call(this.#backend, attribute)
      return
    }
    this.#sharedDestinations.delete(metadata.buffer)
    this.#shadows.delete(metadata.buffer)
    const bytes = bytesOf(identity.array)
    const elementSize = ArrayBuffer.isView(identity.array) ? (identity.array as { BYTES_PER_ELEMENT?: number }).BYTES_PER_ELEMENT ?? 1 : 1
    if (!identity.updateRanges.length) {
      this.#stage(metadata.buffer, 0, bytes, 0, bytes.length)
      return
    }
    for (const range of identity.updateRanges) {
      const start = range.start * elementSize
      this.#stage(metadata.buffer, start, bytes, start, range.count * elementSize)
    }
    identity.clearUpdateRanges()
  }

  #stage(destination: UploadBuffer, destinationOffset: number, source: Uint8Array, sourceOffset: number, size: number, shared?: SharedUpload): void {
    if (!size) return
    if (destinationOffset % 4 !== 0 || size % 4 !== 0 || sourceOffset < 0 || sourceOffset + size > source.length) {
      throw new Error("WebGPU upload range violates its exact four-byte buffer bounds")
    }
    if (!this.#trackedDestinations.has(destination)) {
      this.#trackedDestinations.add(destination)
      const destroy = destination.destroy
      const owner = new WeakRef(this)
      Object.defineProperty(destination, "destroy", {
        configurable: true,
        writable: true,
        value(this: UploadBuffer) {
          owner.deref()?.#discardDestination(this)
          return destroy.call(this)
        },
      })
    }
    const prior = this.#copies.at(-1)
    const sources = shared && this.#sources.get(shared)
    const existing = sources?.find(entry => entry.revision === shared!.revision && entry.start <= sourceOffset && entry.start + entry.size >= sourceOffset + size)
    if (existing) {
      this.#copies.push({ destination, sourceOffset: existing.offset + sourceOffset - existing.start, destinationOffset, size })
      return
    }
    const required = this.#length + size
    if (required > this.#bytes.length) {
      const capacity = Math.max(INITIAL_CAPACITY, 2 ** Math.ceil(Math.log2(required)))
      const expanded = new Uint8Array(capacity)
      expanded.set(this.#bytes.subarray(0, this.#length))
      this.#bytes = expanded
    }
    this.#bytes.set(source.subarray(sourceOffset, sourceOffset + size), this.#length)
    if (prior && prior.destination === destination && prior.destinationOffset + prior.size === destinationOffset && prior.sourceOffset + prior.size === this.#length) {
      prior.size += size
    } else {
      this.#copies.push({ destination, sourceOffset: this.#length, destinationOffset, size })
    }
    if (shared) {
      const entry = { offset: this.#length, revision: shared.revision, start: sourceOffset, size }
      if (sources) sources.push(entry)
      else this.#sources.set(shared, [entry])
    }
    this.#length = required
  }

  #discardDestination(destination: UploadBuffer): void {
    for (let index = this.#copies.length - 1; index >= 0; index -= 1) {
      if (this.#copies[index]!.destination === destination) this.#copies.splice(index, 1)
    }
    this.#shadows.delete(destination)
    this.#sharedDestinations.delete(destination)
    if (!this.#copies.length) {
      this.#length = 0
      this.#sources.clear()
    }
  }

  #flush(): unknown | undefined {
    if (!this.#length) return undefined
    if (!this.#staging || (this.#staging.size ?? 0) < this.#length) {
      const previous = this.#staging
      this.#staging = this.#backend.device.createBuffer({
        label: "playsrc-batched-frame-uploads",
        size: Math.max(INITIAL_CAPACITY, 2 ** Math.ceil(Math.log2(this.#length))),
        usage: COPY_SRC | COPY_DST,
      })
      if (previous) {
        this.#retired.add(previous)
        void this.#queue.onSubmittedWorkDone?.().then(() => {
          if (this.#retired.delete(previous)) previous.destroy()
        }).catch(() => undefined)
      }
    }
    this.#queue.writeBuffer(this.#staging, 0, this.#bytes, 0, this.#length)
    const encoder = this.#backend.device.createCommandEncoder({ label: "playsrc-batched-frame-upload-copies" })
    for (const copy of this.#copies) encoder.copyBufferToBuffer(this.#staging, copy.sourceOffset, copy.destination, copy.destinationOffset, copy.size)
    const command = encoder.finish()
    this.#copies.length = 0
    this.#length = 0
    this.#sources.clear()
    return command
  }

  dispose(): void {
    if (this.#disposed) return
    this.#disposed = true
    BATCHES.delete(this.#backend)
    this.#backend.updateBinding = this.#updateBinding
    this.#backend.updateAttribute = this.#updateAttribute
    if (this.#submitDescriptor) Object.defineProperty(this.#queue, "submit", this.#submitDescriptor)
    else delete (this.#queue as Partial<UploadQueue>).submit
    const staging = this.#staging
    this.#staging = undefined
    if (staging) this.#retired.add(staging)
    const retired = [...this.#retired]
    this.#retired.clear()
    const destroy = () => { for (const buffer of retired) buffer.destroy() }
    if (this.#queue.onSubmittedWorkDone) void this.#queue.onSubmittedWorkDone().then(destroy, destroy)
    else destroy()
    this.#copies.length = 0
    this.#length = 0
    this.#sources.clear()
  }
}
