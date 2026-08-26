export type OwnedResource = Readonly<{ dispose(): void }>
export type ResourceGenerationState = "Staging" | "Active" | "Retiring" | "Disposed"

export type ResourceGenerationSnapshot = Readonly<{
  deviceGeneration: number
  sceneGeneration: number
  state: ResourceGenerationState
  resources: number
  disposals: number
}>

export class OwnedResourceGeneration implements Iterable<OwnedResource> {
  readonly deviceGeneration: number
  readonly sceneGeneration: number
  #state: ResourceGenerationState = "Staging"
  #resources = new Set<OwnedResource>()
  #releasing = new Set<OwnedResource>()
  #disposals = 0

  constructor(deviceGeneration: number, sceneGeneration: number) {
    if (!Number.isSafeInteger(deviceGeneration) || deviceGeneration < 1 || !Number.isSafeInteger(sceneGeneration) || sceneGeneration < 1) {
      throw new Error("resource generation identity is invalid")
    }
    this.deviceGeneration = deviceGeneration
    this.sceneGeneration = sceneGeneration
  }

  add<T extends OwnedResource>(resource: T): T {
    if ((this.#state !== "Staging" && this.#state !== "Active") || this.#resources.has(resource)) {
      throw new Error("resource cannot be added to this generation")
    }
    this.#resources.add(resource)
    return resource
  }

  release(resource: OwnedResource): void {
    if ((this.#state !== "Staging" && this.#state !== "Active") || this.#releasing.has(resource) || !this.#resources.delete(resource)) {
      throw new Error("resource cannot be released from this generation")
    }
    try { resource.dispose() } finally { this.#disposals += 1 }
  }

  releaseAfter(resource: OwnedResource, queueCompletion: Promise<unknown>): void {
    if ((this.#state !== "Staging" && this.#state !== "Active") || this.#releasing.has(resource) || !this.#resources.has(resource)) {
      throw new Error("resource cannot be released from this generation")
    }
    this.#releasing.add(resource)
    void queueCompletion.catch(() => {}).then(() => {
      this.#releasing.delete(resource)
      if (!this.#resources.delete(resource)) return
      try { resource.dispose() } finally { this.#disposals += 1 }
    })
  }

  activate(): void {
    if (this.#state !== "Staging") throw new Error("resource generation cannot activate")
    this.#state = "Active"
  }

  async retire(queueCompletion: Promise<unknown>): Promise<void> {
    if (this.#state === "Disposed") return
    if (this.#state !== "Active") throw new Error("only an active resource generation can retire")
    this.#state = "Retiring"
    try { await queueCompletion } catch { /* device loss makes queue completion unavailable */ }
    this.dispose()
  }

  dispose(): void {
    if (this.#state === "Disposed") return
    this.#state = "Disposed"
    for (const resource of this.#resources) {
      try { resource.dispose() } catch { /* disposal is best-effort after ownership is terminal */ }
      this.#disposals += 1
    }
    this.#resources.clear()
    this.#releasing.clear()
  }

  snapshot(): ResourceGenerationSnapshot {
    return Object.freeze({
      deviceGeneration: this.deviceGeneration,
      sceneGeneration: this.sceneGeneration,
      state: this.#state,
      resources: this.#resources.size,
      disposals: this.#disposals,
    })
  }

  [Symbol.iterator](): Iterator<OwnedResource> {
    return this.#resources[Symbol.iterator]()
  }
}
