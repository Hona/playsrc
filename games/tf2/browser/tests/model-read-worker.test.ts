import { expect, test } from "bun:test"

test("real Worker requests cannot reclaim a shared model while its reader owns it", async () => {
  // This is a transport correctness test, not a browser or FPS measurement.
  const worker = new Worker(new URL("./fixtures/model-read.worker.ts", import.meta.url).href)
  type Reply = { id: number; output: SharedArrayBuffer; ownership: SharedArrayBuffer; retained: number; recycled: number }
  const pending = new Map<number, { resolve: (value: Reply) => void; reject: (error: Error) => void }>()
  worker.onmessage = (event: MessageEvent<Reply>) => {
    pending.get(event.data.id)!.resolve(event.data)
    pending.delete(event.data.id)
  }
  worker.onerror = event => { for (const value of pending.values()) value.reject(new Error(event.message)); pending.clear() }
  const send = (id: number, kind: "model" | "inspect" | "shutdown") => new Promise<Reply>((resolve, reject) => {
    pending.set(id, { resolve, reject })
    worker.postMessage({ id, kind })
  })
  try {
    for (let frame = 0; frame < 64; frame++) {
      const id = 0xffff_ff00 + frame * 2
      const model = await send(id, "model")
      const ownership = new Int32Array(model.ownership)
      expect(Atomics.load(ownership, 0) >>> 0).toBe(id)
      const output = new Uint32Array(model.output)
      // An unrelated transaction overtakes decoding. It must not poison the
      // still-borrowed bytes, even though it can see the shared allocation.
      const inspected = await send(id + 1, "inspect")
      expect(inspected.retained).toBe(1)
      expect(inspected.recycled).toBe(frame)
      expect([...output]).toEqual([id, 0x8000_0000, 0x3f80_0000])
      const owned = output.slice()
      Atomics.store(ownership, 0, 0)
      expect([...owned]).toEqual([id, 0x8000_0000, 0x3f80_0000])
    }
    const stopped = await send(1, "shutdown")
    expect(stopped.retained).toBe(0)
    expect(stopped.recycled).toBe(64)
  } finally {
    worker.terminate()
  }
})
