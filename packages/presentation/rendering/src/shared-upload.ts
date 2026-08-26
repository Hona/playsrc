export type SharedUploadRange = Readonly<{ start: number; count: number }>
type SharedUpload = { revision: number; ranges: readonly SharedUploadRange[] }

const OWNERS = new WeakMap<ArrayBufferView, SharedUpload>()

// Only an exclusive CPU owner may publish revisions. Consumers never mutate the
// source, and each revision is immutable until the next owner publication.
export function ownSharedUpload(source: ArrayBufferView): (ranges?: readonly SharedUploadRange[]) => void {
  if (OWNERS.has(source)) throw new Error("shared GPU upload already has an owner")
  const owner: SharedUpload = { revision: 0, ranges: [] }
  OWNERS.set(source, owner)
  return (ranges = []) => {
    let end = 0
    for (const range of ranges) {
      if (!Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.count)
        || range.start < end || range.count <= 0 || range.start % 4 !== 0 || range.count % 4 !== 0
        || range.start + range.count > source.byteLength) throw new Error("shared GPU upload ranges are invalid")
      end = range.start + range.count
    }
    owner.ranges = ranges
    owner.revision += 1
  }
}

export function sharedUpload(source: ArrayBuffer | ArrayBufferView): Readonly<SharedUpload> | undefined {
  return ArrayBuffer.isView(source) ? OWNERS.get(source) : undefined
}
