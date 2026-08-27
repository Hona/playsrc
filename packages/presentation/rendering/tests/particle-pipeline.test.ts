import { expect, test } from "bun:test"
import type { MaterialStateInput } from "../src/index"
import { particlePipelineKey, particlePipelineVariant } from "../src/particle-pipeline"

test("prepares exactly the Rust-authored blend pair, not every blend combination", () => {
  for (const [source, destination] of [[2, 1], [2, 3], [1, 3], [1, 0]]) {
    const state = { blendSource: source, blendDestination: destination } as MaterialStateInput
    const variant = particlePipelineVariant("PARTICLES/Smoke", state)
    expect(variant.material).toBe("particles/smoke")
    expect(particlePipelineKey(variant)).toBe(`particles/smoke\0${variant.blendSource}\0${variant.blendDestination}`)
    expect(particlePipelineKey({ ...variant, material: "PARTICLES/SMOKE" })).toBe(particlePipelineKey(variant))
  }
  expect(particlePipelineVariant("flash", { blendSource: 2, blendDestination: 1 } as MaterialStateInput)).toEqual({ material: "flash", blendSource: "source-alpha", blendDestination: "one" })
  expect(() => particlePipelineVariant("bad", { blendSource: 4, blendDestination: 1 } as MaterialStateInput)).toThrow("Unsupported")
})
