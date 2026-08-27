export const ADMISSION_EVENT_BYTES = 56
export const MAX_ADMISSION_EVENTS = 8192

/** Pointer-free diagnostic wire records; not gameplay state or a visible-frame claim. */
export function decodeAdmissionMetrics(bytes: DataView) {
  if (bytes.byteLength > MAX_ADMISSION_EVENTS * ADMISSION_EVENT_BYTES || bytes.byteLength % ADMISSION_EVENT_BYTES !== 0) throw new Error("Admission metrics bound exceeded")
  const events = []
  const u64 = (offset: number) => {
    const value = bytes.getBigUint64(offset, true)
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("Admission metrics integer is not exact")
    return Number(value)
  }
  for (let offset = 0; offset < bytes.byteLength; offset += ADMISSION_EVENT_BYTES) {
    const stage = bytes.getUint32(offset, true)
    if (stage < 1 || stage > 15) throw new Error("Admission metrics stage is invalid")
    events.push({ stage, actor: bytes.getUint32(offset + 4, true), tick: u64(offset + 8), at: u64(offset + 16) / 1e6, heapBytes: u64(offset + 24), allocations: u64(offset + 32), allocatedBytes: u64(offset + 40), value: u64(offset + 48) })
  }
  return events
}
