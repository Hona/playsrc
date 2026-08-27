import { expect, test } from "bun:test"
import { reclaimModelReads } from "../src/model-read-ownership"

test("bounded shared leases retain unread allocations across interleaved generations", () => {
  const ownership = new Int32Array(new SharedArrayBuffer(64 * 4))
  const leases = new Map(Array.from({ length: 64 }, (_, slot) => {
    const id = 0xffff_ffc0 + slot
    Atomics.store(ownership, slot, id | 0)
    return [id, { slot, generation: slot % 2, bytes: new Uint8Array([slot]) }] as const
  }))
  const recycled: number[] = []
  const recycle = (lease: { slot: number; bytes: Uint8Array }) => { recycled.push(lease.slot); lease.bytes.fill(255) }
  reclaimModelReads(ownership, leases, recycle)
  expect(leases.size).toBe(64)
  expect(recycled).toEqual([])
  for (let slot = 0; slot < 64; slot += 2) Atomics.store(ownership, slot, 0)
  reclaimModelReads(ownership, leases, recycle)
  expect(recycled).toEqual(Array.from({ length: 32 }, (_, index) => index * 2))
  for (const lease of leases.values()) expect(lease.bytes[0]).toBe(lease.slot)
  reclaimModelReads(ownership, leases, recycle)
  expect(recycled).toHaveLength(32)
  for (let slot = 1; slot < 64; slot += 2) Atomics.store(ownership, slot, 0)
  reclaimModelReads(ownership, leases, recycle)
  expect(leases.size).toBe(0)
  expect(new Set(recycled).size).toBe(64)
})
