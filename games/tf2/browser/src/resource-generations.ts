export type ResourceSection = Readonly<{ pointer: number; length: number; authoredBacking: boolean }>
type Owner = ResourceSection & { references: number }
type Generation = { sections: Owner[]; byteLength?: number; sha256?: string }

/** Worker-owned leases. A section is freed only after its final generation retires. */
export class ResourceGenerations {
  readonly #generations = new Map<number, Generation>()
  readonly #release: (section: ResourceSection) => void

  constructor(release: (section: ResourceSection) => void) { this.#release = release }

  get(generation: number): Generation | undefined { return this.#generations.get(generation) }
  keys(): MapIterator<number> { return this.#generations.keys() }
  values(): MapIterator<Generation> { return this.#generations.values() }
  entries(): MapIterator<[number, Generation]> { return this.#generations.entries() }

  writable(generation: number): boolean {
    const value = this.get(generation)
    return Number.isSafeInteger(generation) && generation > 0 && generation <= 0xffff_ffff
      && (!value || (value.sha256 === undefined && value.sections.length < 1024))
  }

  #append(generation: number, owner: Owner): void {
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

  release(generation: number): boolean {
    const value = this.get(generation)
    if (!value) return false
    this.#generations.delete(generation)
    for (const owner of value.sections) {
      owner.references -= 1
      if (owner.references === 0) this.#release(owner)
    }
    return true
  }
}
