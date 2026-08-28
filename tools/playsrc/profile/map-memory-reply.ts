/** Atomic replies carry current allocator gauges, not structured-clone extras.
 * Resource ownership changes only on ordered control replies; keep that last
 * observation separately rather than dropping it or inventing fresh gauges. */
export function mapMemoryReply(response: any, ownership: any): Record<string, unknown> {
  const memory = response.__playsrcProfileMemory
  const timings = response.timings
  return {
    ...ownership,
    ...memory,
    resourceSections: memory?.resourceSections ?? ownership?.resourceSections ?? null,
    wasmLinearBytes: memory?.wasmLinearBytes ?? timings?.wasmLinearMemoryBytes ?? null,
    allocatorLiveBytes: memory?.allocatorLiveBytes ?? timings?.wasmAllocatorLiveBytes ?? null,
    allocatorHighWaterBytes: memory?.allocatorHighWaterBytes ?? timings?.wasmAllocatorHighWaterBytes ?? null,
  }
}
