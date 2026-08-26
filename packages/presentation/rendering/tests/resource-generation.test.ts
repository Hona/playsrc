import { expect, test } from "bun:test"
import { OwnedResourceGeneration } from "../src/resource-generation"

test("resource generations activate, wait, retire, and dispose each resource exactly once", async () => {
  const generation = new OwnedResourceGeneration(2, 7)
  const counts = [0, 0, 0]
  for (let index = 0; index < counts.length; index += 1) {
    generation.add({ dispose: () => { counts[index]! += 1 } })
  }
  expect(generation.snapshot()).toMatchObject({ state: "Staging", resources: 3, disposals: 0 })
  generation.activate()
  let release!: () => void
  const queue = new Promise<void>((resolve) => { release = resolve })
  const retiring = generation.retire(queue)
  expect(generation.snapshot().state).toBe("Retiring")
  expect(counts).toEqual([0, 0, 0])
  release()
  await retiring
  expect(generation.snapshot()).toMatchObject({ state: "Disposed", resources: 0, disposals: 3 })
  generation.dispose()
  expect(counts).toEqual([1, 1, 1])
  expect(() => generation.add({ dispose() {} })).toThrow()
})

test("active generations own on-demand resources and release evicted resources exactly once", async () => {
  const generation = new OwnedResourceGeneration(1, 2)
  const counts = [0, 0]
  const first = generation.add({ dispose: () => { counts[0]! += 1 } })
  generation.activate()
  generation.add({ dispose: () => { counts[1]! += 1 } })
  generation.release(first)
  expect(generation.snapshot()).toMatchObject({ state: "Active", resources: 1, disposals: 1 })
  expect(() => generation.release(first)).toThrow()
  await generation.retire(Promise.resolve())
  expect(counts).toEqual([1, 1])
  expect(generation.snapshot()).toMatchObject({ state: "Disposed", resources: 0, disposals: 2 })
})
