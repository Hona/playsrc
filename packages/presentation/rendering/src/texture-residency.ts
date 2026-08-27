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
  readonly #queueCompletion?: () => Promise<unknown>
  readonly #resources = new Map<string, ResidentTexture<T>>()
  readonly #sequences = new Map<string, Set<string>>()
  readonly #selected = new Map<string, string>()
  #evictions = 0
  #source?: SharedTextureResidency<T>
  readonly #borrowed = new Map<string, T>()

  constructor(generation: OwnedResourceGeneration, maximumFrames = 4, queueCompletion?: () => Promise<unknown>, source?: SharedTextureResidency<T>) {
    if (!Number.isSafeInteger(maximumFrames) || maximumFrames < 1) {
      throw new Error("animated texture residency bound is invalid")
    }
    this.#generation = generation
    this.#maximumFrames = maximumFrames
    this.#queueCompletion = queueCompletion
    this.#source = source
    // Retained graphs can refer to a lazily selected authored input that the
    // replacement's bind-pose templates do not visit. Keep the complete pinned
    // set for this exact resource closure, never animated LRU frames/consumers.
    if (source) for (const [identity, resource] of source.#resources) {
      if (!resource.pinned) continue
      this.#borrowed.set(identity, resource.value)
      this.#resources.set(identity, { value: resource.value, sequence: null, pinned: true, consumers: new Set() })
    }
  }

  retain(identity: string, create: () => T): T {
    if (!identity) throw new Error("texture residency identity is invalid")
    const existing = this.#resources.get(identity)
    if (existing) {
      existing.pinned = true
      return existing.value
    }
    const prior = this.#source?.#resources.get(identity)
    const value = prior?.pinned ? prior.value : this.#generation.add(create())
    if (prior?.pinned) this.#borrowed.set(identity, value)
    this.#resources.set(identity, { value, sequence: null, pinned: true, consumers: new Set() })
    return value
  }

  select(sequence: string, frame: number, consumer: string, create: () => T, authoredFrameCount = this.#maximumFrames): T {
    if (!sequence || !consumer || !Number.isSafeInteger(frame) || frame < 0
      || !Number.isSafeInteger(authoredFrameCount) || authoredFrameCount < 1) {
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
      while (frames.size > Math.max(this.#maximumFrames, authoredFrameCount)) {
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
        if (this.#queueCompletion) this.#generation.releaseAfter(entry.value, this.#queueCompletion())
        else this.#generation.release(entry.value)
        this.#evictions += 1
      }
    }
    return resource.value
  }

  selected(consumer: string): T | undefined {
    const identity = this.#selected.get(consumer)
    return identity === undefined ? undefined : this.#resources.get(identity)?.value
  }

  commitTransfers(resources: readonly OwnedResource[] = []): void {
    if (!this.#source) return
    this.#source.#generation.transferTo(this.#generation, [...this.#borrowed.values(), ...resources])
    for (const identity of this.#borrowed.keys()) this.#source.#resources.delete(identity)
    this.#borrowed.clear()
    this.#source = undefined
  }

  clear(): void {
    this.#borrowed.clear()
    this.#source = undefined
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
