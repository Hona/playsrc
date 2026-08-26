type Range = Readonly<{ start: number; length: number; value: unknown }>

/** One transaction's immutable decoded ranges. Bytes are exclusively owned by
 * the snapshot stream, never exposed to renderer/HUD callers or shared WASM. */
export class SnapshotRanges {
  readonly #view: DataView
  readonly #ranges = new Map<string, Range[]>()
  readonly #arrays = new Map<string, readonly unknown[]>()
  #previous: SnapshotRanges | undefined
  decoded = 0
  reused = 0
  reusedBytes = 0

  constructor(bytes: Uint8Array, previous?: SnapshotRanges) {
    this.#view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    this.#previous = previous
  }

  read<T>(key: string, index: number, start: number, length: number, decode: () => T): T {
    const prior = this.#previous?.#ranges.get(key)?.[index]
    const same = prior !== undefined && prior.length === length && this.#equal(prior, start)
    const value = same ? prior!.value as T : decode()
    if (same) { this.reused++; this.reusedBytes += length } else this.decoded++
    this.#store(key, index, start, length, value)
    return value
  }

  section<T extends { readonly length: number }>(key: string, start: number, available: number, decode: () => T): T {
    const prior = this.#previous?.#ranges.get(key)?.[0]
    const same = prior !== undefined && prior.length <= available && this.#equal(prior, start)
    const value = same ? prior!.value as T : decode()
    if (same) { this.reused++; this.reusedBytes += value.length } else this.decoded++
    this.#store(key, 0, start, value.length, value)
    return value
  }

  #equal(prior: Range, start: number): boolean {
    const length = prior.length
    const view = this.#previous!.#view
    let at = 0
    for (; at + 4 <= length; at += 4) {
      if (this.#view.getUint32(start + at, true) !== view.getUint32(prior.start + at, true)) return false
    }
    for (; at < length; at++) {
      if (this.#view.getUint8(start + at) !== view.getUint8(prior.start + at)) return false
    }
    return true
  }

  #store(key: string, index: number, start: number, length: number, value: unknown): void {
    let ranges = this.#ranges.get(key)
    if (!ranges) { ranges = []; this.#ranges.set(key, ranges) }
    ranges[index] = { start, length, value }
  }

  finish(): void { this.#previous = undefined }

  array<T>(key: string, values: readonly T[]): readonly T[] {
    const prior = this.#previous?.#arrays.get(key)
    const value = prior?.length === values.length && values.every((item, index) => item === prior[index])
      ? prior as readonly T[] : Object.freeze(values)
    this.#arrays.set(key, value)
    return value
  }
}
