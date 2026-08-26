import * as THREE from "three/webgpu"

const INDEXED_DRAW_WORDS = 5

type AttributeBackend = Readonly<{
  get(attribute: THREE.BufferAttribute): { buffer?: unknown }
  updateAttribute(attribute: THREE.BufferAttribute): void
}>

export class PersistentWorldDraws {
  readonly attribute: THREE.IndirectStorageBufferAttribute
  readonly #backend: AttributeBackend
  readonly #limits: Uint32Array
  readonly #release: (attribute: THREE.BufferAttribute) => void
  #first = Number.POSITIVE_INFINITY
  #last = 0
  #disposed = false

  constructor(count: number, backend: AttributeBackend, release: (attribute: THREE.BufferAttribute) => void) {
    if (!Number.isSafeInteger(count) || count < 0 || count > Math.floor(0xffff_ffff / INDEXED_DRAW_WORDS)) {
      throw new Error("persistent world draw capacity is invalid")
    }
    this.attribute = new THREE.IndirectStorageBufferAttribute(new Uint32Array(count * INDEXED_DRAW_WORDS), 1)
    this.#limits = new Uint32Array(count)
    this.#backend = backend
    this.#release = release
  }

  attach(geometry: THREE.BufferGeometry, slot: number, maximum: number): void {
    this.#validate(slot, maximum)
    if (this.#limits[slot] !== 0 || maximum === 0 || maximum % 3 !== 0) {
      throw new Error("persistent world draw slot is invalid")
    }
    this.#limits[slot] = maximum
    const offset = slot * INDEXED_DRAW_WORDS
    this.attribute.array[offset] = maximum
    this.attribute.array[offset + 1] = 1
    geometry.setIndirect(this.attribute, offset * Uint32Array.BYTES_PER_ELEMENT)
    geometry.setDrawRange(0, maximum)
  }

  update(slot: number, count: number): void {
    this.#validate(slot, count)
    if (count > this.#limits[slot]! || count % 3 !== 0 || this.#limits[slot] === 0) {
      throw new Error("persistent world draw count exceeds its authored index capacity")
    }
    const offset = slot * INDEXED_DRAW_WORDS
    if (this.attribute.array[offset] === count) return
    this.attribute.array[offset] = count
    this.#first = Math.min(this.#first, offset)
    this.#last = Math.max(this.#last, offset + 1)
  }

  flush(): number {
    if (this.#disposed) throw new Error("persistent world draw generation has been disposed")
    if (!Number.isFinite(this.#first)) return 0
    const start = this.#first
    const count = this.#last - start
    this.attribute.clearUpdateRanges()
    this.attribute.addUpdateRange(start, count)
    if (this.#backend.get(this.attribute).buffer !== undefined) this.#backend.updateAttribute(this.attribute)
    else this.attribute.needsUpdate = true
    this.#first = Number.POSITIVE_INFINITY
    this.#last = 0
    return count * Uint32Array.BYTES_PER_ELEMENT
  }

  dispose(): void {
    if (this.#disposed) return
    this.#disposed = true
    this.#release(this.attribute)
  }

  #validate(slot: number, count: number): void {
    if (this.#disposed) throw new Error("persistent world draw generation has been disposed")
    if (!Number.isSafeInteger(slot) || slot < 0 || slot >= this.#limits.length || !Number.isSafeInteger(count) || count < 0) {
      throw new Error("persistent world draw identity is invalid")
    }
  }
}
