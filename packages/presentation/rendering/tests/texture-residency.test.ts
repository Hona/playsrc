import { describe, expect, test } from "bun:test"
import { OwnedResourceGeneration } from "../src/resource-generation"
import { SharedTextureResidency } from "../src/texture-residency"

describe("authored GPU texture residency", () => {
  test("exact replacement hands off pinned objects once and drops old animated consumers", async () => {
    const disposed: string[] = []
    const create = (id: string) => ({ id, dispose() { disposed.push(id) } })
    const old = new OwnedResourceGeneration(1, 1), first = new SharedTextureResidency(old)
    const base = first.retain("base:srgb", () => create("base"))
    first.retain("lazy-cloak-normal", () => create("normal"))
    first.select("animated", 3, "old-water", () => create("animation"))
    old.activate()
    const next = new OwnedResourceGeneration(1, 2), second = new SharedTextureResidency(next, 4, undefined, first)
    expect(second.retain("base:srgb", () => { throw new Error("duplicate upload") })).toBe(base)
    expect(second.selected("old-water")).toBeUndefined()
    expect(second.snapshot().pinned).toBe(2)
    expect(next.snapshot().resources).toBe(0)
    second.commitTransfers()
    second.commitTransfers()
    next.activate()
    await old.retire(Promise.resolve())
    expect(disposed).toEqual(["animation"])
    expect(next.snapshot().resources).toBe(2)
    next.dispose()
    expect(disposed).toEqual(["animation", "base", "normal"])
  })

  test("a rejected replacement cannot dispose borrowed textures or partly transfer across devices", () => {
    let disposed = 0
    const old = new OwnedResourceGeneration(1, 1), first = new SharedTextureResidency(old)
    const texture = first.retain("base", () => ({ dispose() { disposed++ } }))
    old.activate()
    const failed = new OwnedResourceGeneration(1, 2), staged = new SharedTextureResidency(failed, 4, undefined, first)
    staged.clear(); failed.dispose()
    expect(disposed).toBe(0)
    expect(first.retain("base", () => { throw new Error("lost source") })).toBe(texture)
    expect(() => old.transferTo(new OwnedResourceGeneration(2, 2), [texture])).toThrow("transfer")
    expect(old.snapshot().resources).toBe(1)
    old.dispose()
    expect(disposed).toBe(1)
  })

  test("shares exact source identities across model, world, and decal owners", () => {
    const generation = new OwnedResourceGeneration(1, 1)
    const residency = new SharedTextureResidency(generation)
    let created = 0
    const create = () => ({ identity: ++created, dispose() {} })
    const world = residency.retain("source:srgb:0", create)
    const model = residency.retain("source:srgb:0", create)
    const animated = residency.select("source:srgb", 0, "water", create)
    expect(world).toBe(model)
    expect(animated).toBe(world)
    expect(created).toBe(1)
    expect(residency.snapshot()).toEqual({ resources: 1, pinned: 1, animated: 0, evictions: 0 })
  })

  test("materializes exact authored animation frames on demand inside a bounded resident chain", () => {
    const generation = new OwnedResourceGeneration(1, 1)
    const residency = new SharedTextureResidency(generation, 2)
    const disposed: number[] = []
    const create = (frame: number) => ({ frame, dispose() { disposed.push(frame) } })
    residency.select("normal:linear", 0, "water-a", () => create(0))
    generation.activate()
    residency.select("normal:linear", 1, "water-b", () => create(1))
    residency.select("normal:linear", 2, "water-a", () => create(2))
    expect(disposed).toEqual([0])
    expect(residency.selected("water-b")?.frame).toBe(1)
    expect(residency.selected("water-a")?.frame).toBe(2)
    residency.select("normal:linear", 3, "water-b", () => create(3))
    expect(disposed).toEqual([0, 1])
    expect(residency.snapshot()).toEqual({ resources: 2, pinned: 0, animated: 2, evictions: 2 })
    generation.dispose()
    expect(disposed.sort()).toEqual([0, 1, 2, 3])
  })

  test("never evicts an authored frame selected by another visible material", () => {
    const generation = new OwnedResourceGeneration(1, 1)
    const residency = new SharedTextureResidency(generation, 1)
    const disposed: number[] = []
    const create = (frame: number) => ({ frame, dispose() { disposed.push(frame) } })
    residency.select("normal:linear", 0, "world", () => create(0))
    residency.select("normal:linear", 1, "water", () => create(1))
    expect(disposed).toEqual([])
    expect(residency.snapshot().resources).toBe(2)
    residency.select("normal:linear", 1, "world", () => create(1))
    expect(disposed).toEqual([0])
    expect(residency.snapshot().resources).toBe(1)
  })

  test("reuses every exact authored animation frame across repeated complete cycles", () => {
    const generation = new OwnedResourceGeneration(1, 1)
    const residency = new SharedTextureResidency(generation)
    let created = 0
    const disposed: number[] = []
    for (let cycle = 0; cycle < 3; cycle += 1) {
      for (let frame = 0; frame < 9; frame += 1) {
        const selected = residency.select("authored-water:linear", frame, "water", () => {
          created += 1
          return { frame, dispose() { disposed.push(frame) } }
        }, 9)
        expect(selected.frame).toBe(frame)
      }
    }
    expect(created).toBe(9)
    expect(disposed).toEqual([])
    expect(residency.snapshot()).toEqual({ resources: 9, pinned: 0, animated: 9, evictions: 0 })
  })

  test("defers animated frame destruction until previous GPU submissions finish", async () => {
    const generation = new OwnedResourceGeneration(1, 1)
    let complete!: () => void
    const submitted = new Promise<void>((resolve) => { complete = resolve })
    const residency = new SharedTextureResidency(generation, 1, () => submitted)
    const disposed: number[] = []
    residency.select("animated", 0, "world", () => ({ dispose() { disposed.push(0) } }))
    generation.activate()
    residency.select("animated", 1, "world", () => ({ dispose() { disposed.push(1) } }))
    expect(disposed).toEqual([])
    expect(generation.snapshot().resources).toBe(2)
    complete()
    await submitted
    await Promise.resolve()
    await Promise.resolve()
    expect(disposed).toEqual([0])
  })
})
