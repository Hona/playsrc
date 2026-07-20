import { expect, test } from "bun:test"
import { particleBatchRanges } from "../src/particle-batches"

test("batches only consecutive particles with identical material and blend state", () => {
  const item = (material: string, blendSource = "source-alpha", blendDestination = "one-minus-source-alpha") => ({
    material,
    blendSource,
    blendDestination,
  })
  expect(particleBatchRanges([
    item("effects/rocket"),
    item("EFFECTS/ROCKET"),
    item("effects/smoke"),
    item("effects/rocket"),
    item("effects/rocket", "one", "one"),
  ])).toEqual([
    { start: 0, end: 2 },
    { start: 2, end: 3 },
    { start: 3, end: 4 },
    { start: 4, end: 5 },
  ])
})
