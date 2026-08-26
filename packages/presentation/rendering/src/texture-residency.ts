import { OwnedResourceGeneration, type OwnedResource } from "./resource-generation"

type ResidentTexture<T extends OwnedResource> = {
  value: T
  sequence: string | null
  pinned: boolean
  consumers: Set<string>
}

export type TextureResidencySnapshot = Readonly<{
  resources: number
  pinned: number
  animated: number
  evictions: number
}>

export class SharedTextureResidency<T extends OwnedResource> {
  readonly #generation: OwnedResourceGeneration
  readonly #maximumFrames: number
  readonly #resources = new Map<string, ResidentTexture<T>>()
  readonly #sequences = new Map<string, Set<string>>()
  readonly #selected = new Map<string, string>()
  #evictions = 0

  constructor(generation: OwnedResourceGeneration, maximumFrames = 4) {
    if (!Number.isSafeInteger(maximumFrames) || maximumFrames < 1) {
      throw new Error("animated texture residency bound is invalid")
    }
    this.#generation = generation
    this.#maximumFrames = maximumFrames
  }

  retain(identity: string, create: () => T): T {
    if (!identity) throw new Error("texture residency identity is invalid")
    const existing = this.#resources.get(identity)
    if (existing) {
      existing.pinned = true
      return existing.value
    }
    const value = this.#generation.add(create())
    this.#resources.set(identity, { value, sequence: null, pinned: true, consumers: new Set() })
    return value
  }

  select(sequence: string, frame: number, consumer: string, create: () => T): T {
    if (!sequence || !consumer || !Number.isSafeInteger(frame) || frame < 0) {
      throw new Error("animated texture selection is invalid")
    }
    const identity = `${sequence}:${frame}`
    const selected = this.#selected.get(consumer)
    if (selected !== identity) {
      if (selected) this.#resources.get(selected)?.consumers.delete(consumer)
      this.#selected.set(consumer, identity)
    }
    let resource = this.#resources.get(identity)
    if (!resource) {
      const value = this.#generation.add(create())
      resource = { value, sequence, pinned: false, consumers: new Set() }
      this.#resources.set(identity, resource)
      let frames = this.#sequences.get(sequence)
      if (!frames) this.#sequences.set(sequence, frames = new Set())
      frames.add(identity)
    } else if (resource.sequence === null && !resource.pinned) {
      throw new Error("animated texture residency ownership is invalid")
    }
    resource.consumers.add(consumer)
    const frames = this.#sequences.get(sequence)
    if (frames) {
      frames.delete(identity)
      frames.add(identity)
      while (frames.size > this.#maximumFrames) {
        let candidate: string | undefined
        for (const key of frames) {
          const entry = this.#resources.get(key)
          if (entry !== undefined && !entry.pinned && entry.consumers.size === 0) {
            candidate = key
            break
          }
        }
        if (!candidate) break
        const entry = this.#resources.get(candidate)!
        frames.delete(candidate)
        this.#resources.delete(candidate)
        this.#generation.release(entry.value)
        this.#evictions += 1
      }
    }
    return resource.value
  }

  selected(consumer: string): T | undefined {
    const identity = this.#selected.get(consumer)
    return identity === undefined ? undefined : this.#resources.get(identity)?.value
  }

  clear(): void {
    this.#resources.clear()
    this.#sequences.clear()
    this.#selected.clear()
  }

  snapshot(): TextureResidencySnapshot {
    let pinned = 0
    for (const resource of this.#resources.values()) pinned += Number(resource.pinned)
    return Object.freeze({
      resources: this.#resources.size,
      pinned,
      animated: this.#resources.size - pinned,
      evictions: this.#evictions,
    })
  }
}
