export class RetainedVisibilityError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "RetainedVisibilityError"
  }
}

export type RetainedWorldBatch = Readonly<{
  faces: Uint32Array
  sourceIndices: Uint32Array
  targetIndices: Uint32Array
  transparent: boolean
}>

type FacePosting = Readonly<{ batch: number; triangles: Uint32Array }>

type BatchState = {
  readonly input: RetainedWorldBatch
  readonly selected: Uint32Array
  count: number
  previousCount: number
  changed: boolean
}

export class RetainedWorldVisibility {
  readonly #postings: ReadonlyMap<number, readonly FacePosting[]>
  readonly #seen = new Map<number, number>()
  readonly #batches: readonly BatchState[]
  #epoch = 0

  constructor(batches: readonly RetainedWorldBatch[], source?: RetainedWorldVisibility) {
    const groups = source ? undefined : new Map<number, Map<number, number[]>>()
    if (source && source.#batches.length !== batches.length) {
      throw new RetainedVisibilityError("shared world-face batch identity differs")
    }
    this.#batches = batches.map((input, batch) => {
      if (
        input.sourceIndices.length % 3 !== 0
        || input.faces.length !== input.sourceIndices.length / 3
        || input.targetIndices.length < input.sourceIndices.length
        || (source && (
          source.#batches[batch]!.input.faces !== input.faces
          || source.#batches[batch]!.input.sourceIndices !== input.sourceIndices
          || source.#batches[batch]!.input.transparent !== input.transparent
        ))
      ) throw new RetainedVisibilityError("world-face index input is invalid")
      if (groups) {
        for (let triangle = 0; triangle < input.faces.length; triangle += 1) {
          const face = input.faces[triangle]!
          let byBatch = groups.get(face)
          if (!byBatch) groups.set(face, byBatch = new Map())
          let triangles = byBatch.get(batch)
          if (!triangles) byBatch.set(batch, triangles = [])
          triangles.push(triangle)
        }
      }
      return {
        input,
        selected: new Uint32Array(input.faces.length),
        count: 0,
        previousCount: input.sourceIndices.length,
        changed: false,
      }
    })
    if (source) {
      this.#postings = source.#postings
    } else {
      const postings = new Map<number, readonly FacePosting[]>()
      for (const [face, byBatch] of groups!) {
        postings.set(face, Object.freeze(Array.from(byBatch, ([batch, triangles]) =>
          Object.freeze({ batch, triangles: Uint32Array.from(triangles) }))))
      }
      this.#postings = postings
    }
  }

  apply(surfaces: Uint32Array): boolean {
    this.#epoch += 1
    if (!Number.isSafeInteger(this.#epoch)) {
      this.#seen.clear()
      this.#epoch = 1
    }
    for (let index = 0; index < surfaces.length; index += 1) {
      const face = surfaces[index]!
      if (this.#seen.get(face) === this.#epoch) {
        throw new RetainedVisibilityError("visibility contains a duplicate world face")
      }
      this.#seen.set(face, this.#epoch)
    }

    for (const state of this.#batches) state.count = 0
    for (let index = 0; index < surfaces.length; index += 1) {
      const postings = this.#postings.get(surfaces[index]!)
      if (!postings) continue
      for (const posting of postings) {
        const state = this.#batches[posting.batch]!
        if (state.input.transparent) continue
        state.selected.set(posting.triangles, state.count)
        state.count += posting.triangles.length
      }
    }
    for (let index = surfaces.length - 1; index >= 0; index -= 1) {
      const postings = this.#postings.get(surfaces[index]!)
      if (!postings) continue
      for (const posting of postings) {
        const state = this.#batches[posting.batch]!
        if (!state.input.transparent) continue
        state.selected.set(posting.triangles, state.count)
        state.count += posting.triangles.length
      }
    }

    let changed = false
    for (const state of this.#batches) {
      if (!state.input.transparent && state.count > 1) {
        state.selected.subarray(0, state.count).sort()
      }
      const indexCount = state.count * 3
      let different = indexCount !== state.previousCount
      const source = state.input.sourceIndices
      const target = state.input.targetIndices
      for (let index = 0; index < state.count; index += 1) {
        const from = state.selected[index]! * 3
        const to = index * 3
        const first = source[from]!
        const second = source[from + 1]!
        const third = source[from + 2]!
        if (!different && (target[to] !== first || target[to + 1] !== second || target[to + 2] !== third)) {
          different = true
        }
        target[to] = first
        target[to + 1] = second
        target[to + 2] = third
      }
      state.changed = different
      state.previousCount = indexCount
      changed ||= different
    }
    return changed
  }

  has(face: number): boolean {
    return this.#seen.get(face) === this.#epoch
  }

  count(batch: number): number {
    const state = this.#batches[batch]
    if (!state) throw new RetainedVisibilityError("world batch identity is invalid")
    return state.previousCount
  }

  changed(batch: number): boolean {
    const state = this.#batches[batch]
    if (!state) throw new RetainedVisibilityError("world batch identity is invalid")
    return state.changed
  }
}

export type RetainedLeafOccurrence = Readonly<{
  ownership: 0 | 1
  leaves: Uint16Array
}>

export class RetainedLeafVisibility {
  readonly #postings: readonly [Map<number, Uint32Array>, Map<number, Uint32Array>]
  readonly #seen: Uint32Array
  readonly #selected: Uint32Array
  #epoch = 0
  #count = 0

  constructor(occurrences: readonly RetainedLeafOccurrence[]) {
    const collected: [Map<number, number[]>, Map<number, number[]>] = [new Map(), new Map()]
    for (let occurrence = 0; occurrence < occurrences.length; occurrence += 1) {
      const input = occurrences[occurrence]!
      for (let index = 0; index < input.leaves.length; index += 1) {
        const leaf = input.leaves[index]!
        let values = collected[input.ownership].get(leaf)
        if (!values) collected[input.ownership].set(leaf, values = [])
        values.push(occurrence)
      }
    }
    this.#postings = [
      new Map(Array.from(collected[0], ([leaf, values]) => [leaf, Uint32Array.from(values)])),
      new Map(Array.from(collected[1], ([leaf, values]) => [leaf, Uint32Array.from(values)])),
    ]
    this.#seen = new Uint32Array(occurrences.length)
    this.#selected = new Uint32Array(occurrences.length)
  }

  select(leaves: readonly number[], ownership: 0 | 1): number {
    this.#epoch += 1
    if (this.#epoch > 0xffff_ffff) {
      this.#seen.fill(0)
      this.#epoch = 1
    }
    this.#count = 0
    const postings = this.#postings[ownership]
    for (let leaf = 0; leaf < leaves.length; leaf += 1) {
      const occurrences = postings.get(leaves[leaf]!)
      if (!occurrences) continue
      for (let index = 0; index < occurrences.length; index += 1) {
        const occurrence = occurrences[index]!
        if (this.#seen[occurrence] === this.#epoch) continue
        this.#seen[occurrence] = this.#epoch
        this.#selected[this.#count++] = occurrence
      }
    }
    if (this.#count > 1) this.#selected.subarray(0, this.#count).sort()
    return this.#count
  }

  at(index: number): number {
    if (!Number.isSafeInteger(index) || index < 0 || index >= this.#count) {
      throw new RetainedVisibilityError("static-prop candidate identity is invalid")
    }
    return this.#selected[index]!
  }
}
