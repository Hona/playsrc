import { expect, test } from "bun:test"
import { disposeWebGpuBackend } from "../src/webgpu-backend-disposal"

test("conversion targets retire before texture bookkeeping and externally owned output stays untouched", () => {
  const calls: string[] = []
  let textureRecords = true
  const output = { dispose() { throw new Error("external output is not the intermediate target") } }
  const target = { dispose() { if (textureRecords) calls.push("destroy-attachments") } }
  const renderer = { _frameBufferTargets: new Map([[output, target]]), getCanvasTarget: () => ({ colorTexture: {}, depthTexture: {} }), backend: { has: () => false },
    dispose() { textureRecords = false; target.dispose(); calls.push("backend-disposed") } }
  disposeWebGpuBackend(renderer as any)
  expect(calls).toEqual(["destroy-attachments", "backend-disposed"])
})

test("disposal without a conversion target does not create one or submit a frame", () => {
  let disposed = 0
  const renderer = { _frameBufferTargets: new Map(), getCanvasTarget: () => ({ colorTexture: {}, depthTexture: {} }), backend: { has: () => false }, dispose() { disposed++ } }
  disposeWebGpuBackend(renderer as any)
  expect(disposed).toBe(1)
})

test("canvas depth/MSAA color attachments retire directly, with no allocation or second destruction", () => {
  const colorTexture = {}, depthTexture = {}, live = new Set([colorTexture, depthTexture]), destroyed: object[] = []
  const renderer = { _frameBufferTargets: new Map(), getCanvasTarget: () => ({ colorTexture, depthTexture }),
    backend: { has: (texture: object) => live.has(texture), destroyTexture: (texture: object) => { expect(live.delete(texture)).toBe(true); destroyed.push(texture) } },
    dispose() { expect(live.size).toBe(0) } }
  disposeWebGpuBackend(renderer as any); disposeWebGpuBackend(renderer as any)
  expect(destroyed).toEqual([colorTexture, depthTexture])
})
