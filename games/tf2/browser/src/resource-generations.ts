export type ResourceSection = Readonly<{ pointer: number; length: number; authoredBacking: boolean }>
type Owner = ResourceSection & { references: number }
type Generation = { sections: Owner[]; byteLength?: number; sha256?: string }
type Residency = Readonly<{
  uniqueBytes: number
  referencedBytes: number
  sharedBytes: number
  generations: readonly Readonly<{ generation: number; exclusiveBytes: number; sharedBytes: number; bytes: readonly number[] }>[]
}>

/** Worker-owned leases. A section is freed only after its final generation retires. */
export class ResourceGenerations {
  readonly #generations = new Map<number, Generation>()
  readonly #release: (section: ResourceSection) => void
  #retiredThrough = 0
  #residency?: Residency

  constructor(release: (section: ResourceSection) => void) { this.#release = release }

  get(generation: number): Generation | undefined { return this.#generations.get(generation) }
  keys(): MapIterator<number> { return this.#generations.keys() }
  values(): MapIterator<Generation> { return this.#generations.values() }
  entries(): MapIterator<[number, Generation]> { return this.#generations.entries() }

  residency(): Residency {
    if (this.#residency) return this.#residency
    const unique = new Set<Owner>()
    let referencedBytes = 0
    const generations = [...this.#generations].map(([generation, value]) => {
      let exclusiveBytes = 0, sharedBytes = 0
      const bytes = value.sections.map((section) => {
        unique.add(section)
        referencedBytes += section.length
        if (section.references === 1) exclusiveBytes += section.length
        else sharedBytes += section.length
        return section.length
      })
      return Object.freeze({ generation, exclusiveBytes, sharedBytes, bytes: Object.freeze(bytes) })
    })
    let uniqueBytes = 0, sharedBytes = 0
    for (const section of unique) {
      uniqueBytes += section.length
      if (section.references > 1) sharedBytes += section.length
    }
    return this.#residency = Object.freeze({ uniqueBytes, referencedBytes, sharedBytes, generations: Object.freeze(generations) })
  }

  writable(generation: number): boolean {
    const value = this.get(generation)
    return this.#unretired(generation)
      && (!value || (value.sha256 === undefined && value.sections.length < MAX_GRAPH_CHUNKS))
  }

  #unretired(generation: number): boolean {
    return Number.isSafeInteger(generation) && generation > this.#retiredThrough && generation <= 0xffff_ffff
  }

  finalizable(generation: number): boolean {
    const value = this.get(generation)
    return this.#unretired(generation) && value !== undefined && value.sha256 === undefined
      && value.sections.length > 0 && value.sections.length <= MAX_GRAPH_CHUNKS
  }

  loadable(generation: number): boolean {
    return this.#unretired(generation) && this.get(generation)?.sha256 !== undefined
  }

  finalize(generation: number, byteLength: number, sha256: string): boolean {
    const value = this.get(generation)
    if (!value || !this.finalizable(generation) || !/^[0-9a-f]{64}$/.test(sha256)
      || byteLength !== 12 + value.sections.reduce((total, section) => total + section.length - 12, 0)) return false
    value.byteLength = byteLength
    value.sha256 = sha256
    return true
  }

  #append(generation: number, owner: Owner): void {
    this.#residency = undefined
    const value = this.get(generation) ?? { sections: [] }
    value.sections.push(owner)
    this.#generations.set(generation, value)
  }

  adopt(generation: number, section: ResourceSection): void {
    if (!this.writable(generation)) throw new Error("Resource generation is not writable")
    this.#append(generation, { ...section, references: 1 })
  }

  retain(generation: number, sourceGeneration: number, sectionIndex: number): boolean {
    const source = this.get(sourceGeneration)
    if (!this.writable(generation) || sourceGeneration >= generation || !source?.sha256
      || !Number.isSafeInteger(sectionIndex) || sectionIndex < 0) return false
    const owner = source.sections[sectionIndex]
    if (!owner) return false
    owner.references += 1
    this.#append(generation, owner)
    return true
  }

  release(generation: number, retire = true): boolean {
    if (!Number.isSafeInteger(generation) || generation < 1 || generation > 0xffff_ffff) return false
    if (retire) this.#retiredThrough = Math.max(this.#retiredThrough, generation)
    const value = this.get(generation)
    if (!value) return false
    this.#residency = undefined
    this.#generations.delete(generation)
    for (const owner of value.sections) {
      owner.references -= 1
      if (owner.references === 0) this.#release(owner)
    }
    return true
  }
}
import { MAX_GRAPH_CHUNKS } from "@playsrc/asset-store/graph"
