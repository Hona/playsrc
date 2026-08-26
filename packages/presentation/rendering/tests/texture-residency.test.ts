import { describe, expect, test } from "bun:test"
import { OwnedResourceGeneration } from "../src/resource-generation"
import { SharedTextureResidency } from "../src/texture-residency"

describe("authored GPU texture residency", () => {
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
