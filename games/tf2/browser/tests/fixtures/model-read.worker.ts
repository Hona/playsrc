import { reclaimModelReads } from "../../src/model-read-ownership"

const ownership = new Int32Array(new SharedArrayBuffer(4))
const output = new Uint32Array(new SharedArrayBuffer(12))
const leases = new Map<number, { slot: number }>()
let recycled = 0

self.onmessage = (event: MessageEvent<{ id: number; kind: "model" | "inspect" | "shutdown" }>) => {
  reclaimModelReads(ownership, leases, () => { output.fill(0xffff_ffff); recycled++ })
  const request = event.data
  if (request.kind === "model") {
    if (leases.size !== 0) throw new Error("Reused an unread model allocation")
    output.set([request.id, 0x8000_0000, 0x3f80_0000])
    leases.set(request.id, { slot: 0 })
    Atomics.store(ownership, 0, request.id | 0)
  }
  self.postMessage({ id: request.id, output: output.buffer, ownership: ownership.buffer, retained: leases.size, recycled })
  if (request.kind === "shutdown") {
    if (leases.size !== 0) throw new Error("Disposed an unread model allocation")
    self.close()
  }
}
