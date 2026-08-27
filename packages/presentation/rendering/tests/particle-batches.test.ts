import { expect, test } from "bun:test"
import { fillParticleBatchRanges, particleBatchRanges, type MutableParticleBatchRange } from "../src/particle-batches"

const item = (material: string, blendSource = "source-alpha", blendDestination = "one-minus-source-alpha") => ({
  material,
  sky: false,
  blendSource,
  blendDestination,
})

test("batches only consecutive particles with identical material and blend state", () => {
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

test("retains range identities while preserving rocket flash, debris, and smoke order", () => {
  const ranges: MutableParticleBatchRange[] = []
  expect(fillParticleBatchRanges([
    item("effects/brightglow_y_nomodel"),
    item("effects/debris/debris_chunk"),
    item("effects/smokelit2/smoke2lit"),
  ], ranges)).toBe(3)
  const first = ranges[0]
  expect(fillParticleBatchRanges([
    item("effects/brightglow_y_nomodel"),
    item("effects/brightglow_y_nomodel"),
    item("effects/smokelit2/smoke2lit"),
  ], ranges)).toBe(2)
  expect(ranges[0]).toBe(first)
  expect(ranges).toEqual([{ start: 0, end: 2 }, { start: 2, end: 3 }])
})

test("main and sky particles never share a billboard batch even with the same material", () => {
  expect(particleBatchRanges([item("smoke"), { ...item("smoke"), sky: true }, item("smoke")]))
    .toEqual([{ start: 0, end: 1 }, { start: 1, end: 2 }, { start: 2, end: 3 }])
})
