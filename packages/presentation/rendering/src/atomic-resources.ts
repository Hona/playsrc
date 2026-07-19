export type ReplaceableResource = Readonly<{ identity: string; dispose(): void | Promise<void> }>

export class AtomicResourceError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "AtomicResourceError"
  }
}

export class AtomicResourceSlot<T extends ReplaceableResource> {
  #active: T | null = null
  #generation = 0

  get generation(): number {
    return this.#generation
  }

  current(): T | null {
    return this.#active
  }

  async replace(create: () => T | Promise<T>, ready: (resource: T) => void | Promise<void>): Promise<T> {
    let staged: T | null = null
    try {
      staged = await create()
      if (!staged || !staged.identity) throw new AtomicResourceError("staged resource identity is invalid")
      await ready(staged)
    } catch (error) {
      if (staged) await staged.dispose()
      throw new AtomicResourceError(`resource replacement staging failed: ${String(error)}`)
    }
    const prior = this.#active
    this.#active = staged
    this.#generation += 1
    if (prior) await prior.dispose()
    return staged
  }

  async dispose(): Promise<void> {
    const active = this.#active
    this.#active = null
    if (active) await active.dispose()
  }
}
