import { expect, test } from "bun:test"
import { RenderTarget } from "three/webgpu"
import Textures from "three/src/renderers/common/Textures.js"
import { resizeSampledRenderTargets } from "../src/render-target-resize"

test("initializes resized attachment generations before an earlier pass can sample them", () => {
  let generation = 0, initialized = false
  const operations: string[] = []
  const target = { setSize(width: number, height: number) { initialized = false; operations.push(`resize:${width}x${height}`) } }
  const initialize = () => {
    if (!initialized) { initialized = true; generation += 1; operations.push("initialize-attachments") }
  }
  resizeSampledRenderTargets([null, target], 960, 640, initialize)
  const sampledGeneration = generation
  operations.push("encode-texture-consumer")
  initialize()
  operations.push("encode-render-target-writer")
  expect(generation).toBe(sampledGeneration)
  expect(operations).toEqual(["resize:960x640", "initialize-attachments", "encode-texture-consumer", "encode-render-target-writer"])
})

test("fresh water target admission removes only the unused sampled-first allocation, across replacement and MSAA", () => {
  const run = (eager: boolean, samples: number) => {
    const records: { kind: string; name: string; bytes: number | null; id: number }[] = []
    const data = new WeakMap<object, any>()
    let id = 0, writes = 0, live = 0, peak = 0
    const backend = {
      get(key: object) { let value = data.get(key); if (!value) data.set(key, value = {}); return value },
      delete(key: object) { data.delete(key) },
      createTexture(texture: any, options: any) {
        const bytes = texture.isDepthTexture ? null : options.width * options.height * 4
        const allocation = { id: ++id, bytes, destroyed: false }
        this.get(texture).texture = allocation
        live += bytes ?? 0; peak = Math.max(peak, live)
        records.push({ kind: "create", name: texture.name, bytes, id })
      },
      destroyTexture(texture: any) {
        const allocation = this.get(texture).texture
        if (allocation && !allocation.destroyed) {
          allocation.destroyed = true; live -= allocation.bytes ?? 0
          records.push({ kind: "destroy", name: texture.name, bytes: allocation.bytes, id: allocation.id })
        }
      },
      updateTexture() { writes++ },
    }
    const manager = new Textures({ getRenderTarget: () => null }, backend, { memory: { renderTargets: 0 }, createTexture() {}, destroyTexture() {} })
    for (let generation = 1; generation <= 3; generation++) {
      const target = new RenderTarget(1280, 720, { depthBuffer: true, samples })
      target.texture.name = `water-${generation}`
      if (eager) manager.updateRenderTarget(target)
      manager.updateTexture(target.texture) // actual compile-time sampled binding
      const compiled = backend.get(target.texture).texture
      manager.updateRenderTarget(target) // later water pass admission
      expect(compiled.destroyed).toBe(!eager)
      const admitted = backend.get(target.texture).texture
      manager.updateTexture(target.texture); manager.updateRenderTarget(target)
      expect(backend.get(target.texture).texture).toBe(admitted)
      expect(target).toMatchObject({ width: 1280, height: 720, depthBuffer: true, samples })
      target.dispose(); target.dispose(); expect(live).toBe(0)
    }
    expect(writes).toBe(0)
    return { colorCreates: records.filter(record => record.kind === "create" && record.bytes !== null).length,
      colorBytes: records.filter(record => record.kind === "create").reduce((n, record) => n + (record.bytes ?? 0), 0), peak }
  }
  for (const samples of [0, 1, 4]) {
    expect(run(false, samples)).toEqual({ colorCreates: 6, colorBytes: 22118400, peak: 3686400 })
    expect(run(true, samples)).toEqual({ colorCreates: 3, colorBytes: 11059200, peak: 3686400 })
  }
})

test("initializes every exact target independently and surfaces failure before any later target is changed", () => {
  const sizes: number[] = []
  const first = { setSize() { sizes.push(1) } }, second = { setSize() { sizes.push(2) } }
  expect(() => resizeSampledRenderTargets([first, second], 1280, 720, () => { throw new Error("allocation rejected") })).toThrow("allocation rejected")
  expect(sizes).toEqual([1])
})

test("pinned Three texture management cannot replace a resized water texture after a consumer is encoded", () => {
  const run = (eager: boolean) => {
    const target = new RenderTarget(1280, 720)
    const data = new WeakMap<object, any>()
    let allocations = 0
    const backend = {
      get(key: object) { let value = data.get(key); if (!value) data.set(key, value = {}); return value },
      delete(key: object) { data.delete(key) },
      createTexture(texture: object) { this.get(texture).texture = { generation: ++allocations, destroyed: false } },
      destroyTexture(texture: object) { const value = this.get(texture).texture; if (value) value.destroyed = true },
    }
    const manager = new Textures({ getRenderTarget: () => target }, backend, { memory: { renderTargets: 0 }, createTexture() {}, destroyTexture() {} })
    manager.updateRenderTarget(target)
    if (eager) resizeSampledRenderTargets([target], 960, 640, value => manager.updateRenderTarget(value))
    else target.setSize(960, 640)
    manager.updateTexture(target.texture)
    const encodedConsumer = backend.get(target.texture).texture
    manager.updateRenderTarget(target)
    const replacedBeforeSubmit = encodedConsumer.destroyed
    const stillCurrent = backend.get(target.texture).texture === encodedConsumer
    target.dispose()
    return { replacedBeforeSubmit, stillCurrent }
  }
  expect(run(false)).toEqual({ replacedBeforeSubmit: true, stillCurrent: false })
  expect(run(true)).toEqual({ replacedBeforeSubmit: false, stillCurrent: true })
})
