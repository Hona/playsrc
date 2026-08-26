import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { PersistentWorldDraws } from "../src/persistent-world-draws"
import { RetainedWorldVisibility } from "../src/retained-visibility"

function fixture(count: number, allocated = true) {
  const uploads: { start: number; count: number; values: number[] }[] = []
  const released: THREE.BufferAttribute[] = []
  const backend = {
    get: () => allocated ? { buffer: {} } : {},
    updateAttribute(attribute: THREE.BufferAttribute) {
      for (const range of attribute.updateRanges) {
        uploads.push({ start: range.start, count: range.count, values: [...attribute.array.slice(range.start, range.start + range.count)] })
      }
      attribute.clearUpdateRanges()
    },
  }
  const draws = new PersistentWorldDraws(count, backend, attribute => released.push(attribute))
  return { draws, uploads, released }
}

describe("persistent exact indexed world render bundles", () => {
  test("retains one supported indirect buffer and exact bounded indexed commands across PVS changes", () => {
    const { draws, uploads } = fixture(3)
    const first = new THREE.BufferGeometry()
    const third = new THREE.BufferGeometry()
    draws.attach(first, 0, 12)
    draws.attach(third, 2, 6)

    expect(first.getIndirect()).toBe(draws.attribute)
    expect(third.getIndirect()).toBe(draws.attribute)
    expect(third.indirectOffset).toBe(40)
    expect([...draws.attribute.array]).toEqual([12, 1, 0, 0, 0, 0, 0, 0, 0, 0, 6, 1, 0, 0, 0])

    draws.update(0, 3)
    draws.update(2, 0)
    expect(draws.flush()).toBe(44)
    expect(uploads).toEqual([{ start: 0, count: 11, values: [3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] }])
    expect(draws.flush()).toBe(0)

    draws.update(2, 6)
    expect(draws.flush()).toBe(4)
    expect(uploads[1]).toEqual({ start: 10, count: 1, values: [6] })
    expect(first.drawRange).toEqual({ start: 0, count: 12 })
  })

  test("preserves isolated exact main and sky selections without changing command identities", () => {
    const sourceIndices = Uint32Array.from([0, 1, 2, 3, 4, 5, 6, 7, 8])
    const faces = Uint32Array.from([2, 4, 6])
    const mainIndices = sourceIndices.slice()
    const skyIndices = sourceIndices.slice()
    const mainVisibility = new RetainedWorldVisibility([{ faces, sourceIndices, targetIndices: mainIndices, transparent: false }])
    const skyVisibility = new RetainedWorldVisibility([{ faces, sourceIndices, targetIndices: skyIndices, transparent: false }], mainVisibility)
    const main = fixture(1)
    const sky = fixture(1)
    main.draws.attach(new THREE.BufferGeometry(), 0, 9)
    sky.draws.attach(new THREE.BufferGeometry(), 0, 9)

    mainVisibility.apply(Uint32Array.from([2, 6]))
    skyVisibility.apply(Uint32Array.from([4]))
    main.draws.update(0, mainVisibility.count(0))
    sky.draws.update(0, skyVisibility.count(0))
    main.draws.flush()
    sky.draws.flush()

    expect([...mainIndices.slice(0, main.draws.attribute.array[0])]).toEqual([0, 1, 2, 6, 7, 8])
    expect([...skyIndices.slice(0, sky.draws.attribute.array[0])]).toEqual([3, 4, 5])
    expect(main.draws.attribute).not.toBe(sky.draws.attribute)
  })

  test("stages updates before GPU allocation and rejects stale generations and out-of-bounds draws", () => {
    const { draws, uploads, released } = fixture(1, false)
    draws.attach(new THREE.BufferGeometry(), 0, 6)
    expect(() => draws.attach(new THREE.BufferGeometry(), 0, 6)).toThrow(/slot/i)
    expect(() => draws.update(0, 9)).toThrow(/capacity/i)
    expect(() => draws.update(0, 2)).toThrow(/capacity/i)
    expect(() => draws.update(1, 0)).toThrow(/identity/i)

    draws.update(0, 0)
    expect(draws.flush()).toBe(4)
    expect(uploads).toEqual([])
    draws.dispose()
    draws.dispose()
    expect(released).toEqual([draws.attribute])
    expect(() => draws.update(0, 3)).toThrow(/disposed/i)
    expect(() => draws.flush()).toThrow(/disposed/i)
  })
})
