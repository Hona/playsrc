/** Reclaim only after the reader's release-store, before admitting more work.
 * The shared word is the ownership acknowledgement; no second message is needed.
 * Recycling never runs on the main thread or while a decoder still owns a view.
 */
export function reclaimModelReads<T extends { slot: number }>(
  ownership: Int32Array,
  leases: Map<number, T>,
  recycle: (lease: T) => void,
): void {
  for (const [id, lease] of leases) {
    if (Atomics.load(ownership, lease.slot) !== 0) continue
    leases.delete(id)
    recycle(lease)
  }
}
