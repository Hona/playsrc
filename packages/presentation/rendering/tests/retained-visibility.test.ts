import { describe, expect, test } from "bun:test"
import { RetainedLeafVisibility, RetainedWorldVisibility } from "../src/retained-visibility"

function batch(faces: readonly number[], transparent = false) {
  const sourceIndices = Uint32Array.from(faces.flatMap((_, index) => [index * 3, index * 3 + 1, index * 3 + 2]))
  return { faces: Uint32Array.from(faces), sourceIndices, targetIndices: sourceIndices.slice(), transparent }
}

describe("retained world-face postings", () => {
  test("preserves original opaque triangle order and reverses translucent face order", () => {
    const opaque = batch([7, 8, 7, 9])
    const translucent = batch([7, 8, 7, 9], true)
    const visibility = new RetainedWorldVisibility([opaque, translucent])

    expect(visibility.apply(Uint32Array.from([9, 7]))).toBe(true)
    expect(visibility.count(0)).toBe(9)
    expect([...opaque.targetIndices.slice(0, visibility.count(0))]).toEqual([0, 1, 2, 6, 7, 8, 9, 10, 11])
    expect([...translucent.targetIndices.slice(0, visibility.count(1))]).toEqual([0, 1, 2, 6, 7, 8, 9, 10, 11])

    expect(visibility.apply(Uint32Array.from([7, 9]))).toBe(true)
    expect(visibility.changed(0)).toBe(false)
    expect(visibility.changed(1)).toBe(true)
    expect([...translucent.targetIndices.slice(0, visibility.count(1))]).toEqual([9, 10, 11, 0, 1, 2, 6, 7, 8])
  })

  test("retains byte-identical selected indices without upload invalidation", () => {
    const opaque = batch([2, 4, 2])
    const visibility = new RetainedWorldVisibility([opaque])
    expect(visibility.apply(Uint32Array.from([2]))).toBe(true)
    expect(visibility.apply(Uint32Array.from([2, 999]))).toBe(false)
    expect(visibility.changed(0)).toBe(false)
    expect(visibility.count(0)).toBe(6)
    expect([...opaque.targetIndices.slice(0, 6)]).toEqual([0, 1, 2, 6, 7, 8])
  })

  test("rejects duplicate visible faces before mutating the prior selection", () => {
    const opaque = batch([2, 4])
    const visibility = new RetainedWorldVisibility([opaque])
    visibility.apply(Uint32Array.from([4]))
    const before = opaque.targetIndices.slice()
    expect(() => visibility.apply(Uint32Array.from([2, 2]))).toThrow(/duplicate world face/i)
    expect(opaque.targetIndices).toEqual(before)
    expect(visibility.count(0)).toBe(3)
  })

  test("retains dense opaque source order and sparse high-face duplicate rejection", () => {
    const opaque = batch([40, 7, 18, 40, 3, 18, 92, 7])
    const visibility = new RetainedWorldVisibility([opaque])
    expect(visibility.apply(Uint32Array.from([7, 40, 18, 0xffff_fffe]))).toBe(true)
    expect([...opaque.targetIndices.slice(0, visibility.count(0))]).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 21, 22, 23,
    ])
    expect(visibility.has(0xffff_fffe)).toBe(true)
    const before = opaque.targetIndices.slice()
    expect(() => visibility.apply(Uint32Array.from([0xffff_fffe, 0xffff_fffe]))).toThrow(/duplicate world face/i)
    expect(opaque.targetIndices).toEqual(before)
  })

  test("selects sparse opaque postings across batches without scanning unrelated triangles", () => {
    const first = batch([9, 1, 8, 1, 7, 2, 6])
    const second = batch([4, 2, 4, 1, 8])
    const visibility = new RetainedWorldVisibility([first, second])

    expect(visibility.apply(Uint32Array.from([2, 1]))).toBe(true)
    expect([...first.targetIndices.slice(0, visibility.count(0))]).toEqual([3, 4, 5, 9, 10, 11, 15, 16, 17])
    expect([...second.targetIndices.slice(0, visibility.count(1))]).toEqual([3, 4, 5, 9, 10, 11])
    expect(visibility.apply(Uint32Array.from([1, 2]))).toBe(false)
  })

  test("retains sparse opaque source-face postings outside the dense face bound", () => {
    const opaque = batch([0xffff_fffe, 3, 0xffff_fffe, 9])
    const visibility = new RetainedWorldVisibility([opaque])

    expect(visibility.apply(Uint32Array.from([0xffff_fffe]))).toBe(true)
    expect([...opaque.targetIndices.slice(0, visibility.count(0))]).toEqual([0, 1, 2, 6, 7, 8])
    expect(visibility.has(0xffff_fffe)).toBe(true)
  })

  test("shares immutable face postings while keeping main and authored-sky selections independent", () => {
    const main = batch([2, 4, 6])
    const sky = { ...main, targetIndices: main.sourceIndices.slice() }
    const mainVisibility = new RetainedWorldVisibility([main])
    const skyVisibility = new RetainedWorldVisibility([sky], mainVisibility)

    mainVisibility.apply(Uint32Array.from([2, 6]))
    skyVisibility.apply(Uint32Array.from([4]))
    expect([...main.targetIndices.slice(0, mainVisibility.count(0))]).toEqual([0, 1, 2, 6, 7, 8])
    expect([...sky.targetIndices.slice(0, skyVisibility.count(0))]).toEqual([3, 4, 5])
    expect(mainVisibility.has(2)).toBe(true)
    expect(mainVisibility.has(4)).toBe(false)
    expect(skyVisibility.has(4)).toBe(true)
  })

  test("rejects mismatched triangle and target-buffer lengths", () => {
    expect(() => new RetainedWorldVisibility([{
      faces: Uint32Array.from([1]),
      sourceIndices: Uint32Array.from([0, 1]),
      targetIndices: Uint32Array.from([0, 1]),
      transparent: false,
    }])).toThrow(/invalid/i)
  })
})

describe("retained static-prop leaf postings", () => {
  test("deduplicates multiple visible leaves and retains original occurrence order", () => {
    const index = new RetainedLeafVisibility([
      { ownership: 0, leaves: Uint16Array.from([2, 4]) },
      { ownership: 1, leaves: Uint16Array.from([4]) },
      { ownership: 0, leaves: Uint16Array.from([8, 2]) },
      { ownership: 0, leaves: Uint16Array.from([4]) },
    ])
    expect(index.select([4, 2, 4], 0)).toBe(3)
    expect([index.at(0), index.at(1), index.at(2)]).toEqual([0, 2, 3])
    expect(index.select([4, 2], 1)).toBe(1)
    expect(index.at(0)).toBe(1)
    expect(index.select([99], 0)).toBe(0)
    expect(() => index.at(0)).toThrow(/identity/i)
  })
})
