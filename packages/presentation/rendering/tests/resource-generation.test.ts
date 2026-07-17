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
