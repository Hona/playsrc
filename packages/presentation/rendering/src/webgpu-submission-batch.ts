export type BatchedGpuQueue = {
  submit(buffers: readonly unknown[]): void
  writeBuffer(...arguments_: any[]): void
  writeTexture?(...arguments_: any[]): void
  copyExternalImageToTexture?(...arguments_: any[]): void
  onSubmittedWorkDone?(...arguments_: any[]): Promise<void>
}

const QUEUES = new WeakMap<BatchedGpuQueue, WebGpuSubmissionBatch>()

export function withImmediateGpuSubmissions<T>(queue: BatchedGpuQueue, operation: () => T): T {
  const batch = QUEUES.get(queue)
  if (!batch) throw new Error("WebGPU submission owner is unavailable")
  return batch.immediate(operation)
}

/** Preserve queue-timeline ordering while combining adjacent render submissions. */
export class WebGpuSubmissionBatch {
  readonly #queue: BatchedGpuQueue
  readonly #originals = new Map<string, (...arguments_: any[]) => any>()
  readonly #pending: unknown[] = []
  #active = false

  constructor(queue: BatchedGpuQueue) {
    if (typeof queue?.submit !== "function" || typeof queue.writeBuffer !== "function") {
      throw new Error("WebGPU submission queue is unavailable")
    }
    this.#queue = queue
    QUEUES.set(queue, this)
    this.#replace("submit", (buffers: readonly unknown[]) => {
      if (!this.#active) return this.#originals.get("submit")!.call(this.#queue, buffers)
      // Three.js reuses and clears its submission array after submit returns.
      for (const buffer of buffers) this.#pending.push(buffer)
    })
    for (const method of ["writeBuffer", "writeTexture", "copyExternalImageToTexture", "onSubmittedWorkDone"] as const) {
      if (typeof queue[method] !== "function") continue
      this.#replace(method, (...arguments_: any[]) => {
        this.flush()
        return this.#originals.get(method)!.apply(this.#queue, arguments_)
      })
    }
  }

  #replace(method: string, replacement: (...arguments_: any[]) => any): void {
    const original = (this.#queue as any)[method]
    this.#originals.set(method, original)
    Object.defineProperty(this.#queue, method, { configurable: true, writable: true, value: replacement })
  }

  begin(): void {
    if (this.#active) throw new Error("WebGPU submission batch is already active")
    this.#active = true
  }

  immediate<T>(operation: () => T): T {
    this.flush()
    const active = this.#active
    this.#active = false
    try { return operation() }
    finally { this.#active = active }
  }

  flush(): void {
    if (this.#pending.length === 0) return
    const buffers = this.#pending.splice(0)
    this.#originals.get("submit")!.call(this.#queue, buffers)
  }

  finish(): void {
    try { this.flush() }
    finally { this.#active = false }
  }

  dispose(): void {
    this.finish()
    QUEUES.delete(this.#queue)
    for (const [method, original] of this.#originals) {
      Object.defineProperty(this.#queue, method, { configurable: true, writable: true, value: original })
    }
    this.#originals.clear()
  }
}
