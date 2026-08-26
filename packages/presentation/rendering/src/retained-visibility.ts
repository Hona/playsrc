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
  readonly #opaqueOffsets: Uint32Array | undefined
  readonly #opaquePostings: Uint32Array | undefined
  readonly #seen = new Map<number, number>()
  readonly #denseSeen: Uint32Array | undefined
  readonly #batches: readonly BatchState[]
  #epoch = 0

  constructor(batches: readonly RetainedWorldBatch[], source?: RetainedWorldVisibility) {
    const groups = source ? undefined : new Map<number, Map<number, number[]>>()
    let maximumFace = 0
    let opaqueTriangles = 0
    if (!source) {
      for (const batch of batches) {
        for (let triangle = 0; triangle < batch.faces.length; triangle += 1) {
          if (batch.faces[triangle]! > maximumFace) maximumFace = batch.faces[triangle]!
        }
      }
    }
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
          if (!input.transparent && maximumFace <= 1_048_576) {
            opaqueTriangles += 1
            continue
          }
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
      this.#denseSeen = source.#denseSeen ? new Uint32Array(source.#denseSeen.length) : undefined
      this.#opaqueOffsets = source.#opaqueOffsets
      this.#opaquePostings = source.#opaquePostings
    } else {
      if (maximumFace <= 1_048_576 && opaqueTriangles > 0) {
        const offsets = new Uint32Array(maximumFace + 2)
        for (const state of this.#batches) {
          if (state.input.transparent) continue
          for (let triangle = 0; triangle < state.input.faces.length; triangle += 1) {
            const face = state.input.faces[triangle]! + 1
            offsets[face] = offsets[face]! + 1
          }
        }
        for (let face = 1; face < offsets.length; face += 1) offsets[face] = offsets[face]! + offsets[face - 1]!
        const cursors = offsets.slice(0, -1)
        const postings = new Uint32Array(opaqueTriangles * 2)
        for (let batch = 0; batch < this.#batches.length; batch += 1) {
          const state = this.#batches[batch]!
          if (state.input.transparent) continue
          for (let triangle = 0; triangle < state.input.faces.length; triangle += 1) {
            const face = state.input.faces[triangle]!
            const offset = cursors[face]! * 2
            cursors[face] = cursors[face]! + 1
            postings[offset] = batch
            postings[offset + 1] = triangle
          }
        }
        this.#opaqueOffsets = offsets
        this.#opaquePostings = postings
      }
      const postings = new Map<number, readonly FacePosting[]>()
      for (const [face, byBatch] of groups!) {
        postings.set(face, Object.freeze(Array.from(byBatch, ([batch, triangles]) =>
          Object.freeze({ batch, triangles: Uint32Array.from(triangles) }))))
      }
      this.#postings = postings
      this.#denseSeen = maximumFace <= 1_048_576 ? new Uint32Array(maximumFace + 1) : undefined
    }
  }

  apply(surfaces: Uint32Array): boolean {
    this.#epoch += 1
    if (this.#epoch > 0xffff_ffff) {
      this.#seen.clear()
      this.#denseSeen?.fill(0)
      this.#epoch = 1
    }
    const dense = this.#denseSeen
    for (let index = 0; index < surfaces.length; index += 1) {
      const face = surfaces[index]!
      const previous = dense && face < dense.length ? dense[face] : this.#seen.get(face)
      if (previous === this.#epoch) {
        throw new RetainedVisibilityError("visibility contains a duplicate world face")
      }
      if (dense && face < dense.length) dense[face] = this.#epoch
      else this.#seen.set(face, this.#epoch)
    }

    const sparseOpaque = this.#opaqueOffsets === undefined || surfaces.length * 4 <= (dense?.length ?? 0)
    for (const state of this.#batches) state.count = 0
    for (let index = surfaces.length - 1; index >= 0; index -= 1) {
      const face = surfaces[index]!
      if (sparseOpaque && this.#opaqueOffsets && face + 1 < this.#opaqueOffsets.length) {
        const end = this.#opaqueOffsets[face + 1]!
        for (let opaque = this.#opaqueOffsets[face]!; opaque < end; opaque += 1) {
          const state = this.#batches[this.#opaquePostings![opaque * 2]!]!
          state.selected[state.count++] = this.#opaquePostings![opaque * 2 + 1]!
        }
      }
      const postings = this.#postings.get(face)
      if (postings) {
        for (const posting of postings) {
          const state = this.#batches[posting.batch]!
          state.selected.set(posting.triangles, state.count)
          state.count += posting.triangles.length
        }
      }
    }

    let changed = false
    for (const state of this.#batches) {
      const source = state.input.sourceIndices
      const target = state.input.targetIndices
      let different = false
      let selected = 0
      const indexed = state.input.transparent || sparseOpaque
      const limit = indexed ? state.count : state.input.faces.length
      if (!state.input.transparent && indexed && limit > 1) state.selected.subarray(0, limit).sort()
      for (let index = 0; index < limit; index += 1) {
        if (!indexed) {
          const face = state.input.faces[index]!
          if ((dense && face < dense.length ? dense[face] : this.#seen.get(face)) !== this.#epoch) continue
        }
        const from = (indexed ? state.selected[index]! : index) * 3
        const to = selected++ * 3
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
      state.count = selected
      const indexCount = selected * 3
      different ||= indexCount !== state.previousCount
      state.changed = different
      state.previousCount = indexCount
      changed ||= different
    }
    return changed
  }

  has(face: number): boolean {
    return (this.#denseSeen && face < this.#denseSeen.length ? this.#denseSeen[face] : this.#seen.get(face)) === this.#epoch
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
