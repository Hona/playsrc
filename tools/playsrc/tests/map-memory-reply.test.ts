import { expect, test } from "bun:test"
import { mapMemoryReply } from "../profile/map-memory-reply"

test("atomic map-memory replies retain ordered ownership but never stale allocator gauges", () => {
  const ownership = { resourceSections: [{ generation: 1 }], resourceBytes: 100, allocatorLiveBytes: 999 }
  const atomic = { timings: { wasmLinearMemoryBytes: 1024, wasmAllocatorLiveBytes: 200, wasmAllocatorHighWaterBytes: 400 } }
  expect(mapMemoryReply(atomic, ownership)).toEqual({ ...ownership, wasmLinearBytes: 1024, allocatorLiveBytes: 200, allocatorHighWaterBytes: 400 })
  expect(mapMemoryReply({}, ownership)).toMatchObject({ resourceSections: ownership.resourceSections, allocatorLiveBytes: null, allocatorHighWaterBytes: null, wasmLinearBytes: null })
  const released = { __playsrcProfileMemory: { resourceSections: [], resourceBytes: 0, wasmLinearBytes: 1024, allocatorLiveBytes: 20, allocatorHighWaterBytes: 400 } }
  expect(mapMemoryReply(released, ownership)).toEqual(released.__playsrcProfileMemory)
  // This helper is injected into a headed page without module globals.
  const injected = new Function(`return (${mapMemoryReply.toString()})`)()
  expect(injected(atomic, ownership)).toEqual(mapMemoryReply(atomic, ownership))
})
