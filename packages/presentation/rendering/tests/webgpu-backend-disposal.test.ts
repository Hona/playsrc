import { expect, test } from "bun:test"
import { disposeWebGpuBackend } from "../src/webgpu-backend-disposal"

test("conversion targets retire before texture bookkeeping and externally owned output stays untouched", () => {
  const calls: string[] = []
  let textureRecords = true
  const output = { dispose() { throw new Error("external output is not the intermediate target") } }
  const target = { dispose() { if (textureRecords) calls.push("destroy-attachments") } }
  const renderer = { _frameBufferTargets: new Map([[output, target]]), dispose() { textureRecords = false; target.dispose(); calls.push("backend-disposed") } }
  disposeWebGpuBackend(renderer as any)
  expect(calls).toEqual(["destroy-attachments", "backend-disposed"])
})

test("disposal without a conversion target does not create one or submit a frame", () => {
  let disposed = 0
  const renderer = { _frameBufferTargets: new Map(), dispose() { disposed++ } }
  disposeWebGpuBackend(renderer as any)
  expect(disposed).toBe(1)
})
