import { expect, test } from "bun:test"
import { SourceParticleDepth } from "../src/particle-depth"

test("native pipeline traversal cannot copy depth or end an expired render pass", () => {
  let copies = 0
  const backend = { copyFramebufferToTexture() { copies++ } }
  const depth = new SourceParticleDepth(backend)
  const renderer = { _compilationPromises: [], info: { calls: 12 }, getRenderTarget() { throw new Error("no render target during compilation") } }
  expect(() => depth.capture(renderer as any, undefined as any)).not.toThrow()
  expect(copies).toBe(0)
  depth.dispose()
})
