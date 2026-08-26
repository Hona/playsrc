import { describe, expect, test } from "bun:test"
import { WebGpuFramePresentation, type FramePresentationBackend } from "../src/webgpu-frame-presentation"

describe("completed WebGPU framebuffer presentation", () => {
  test("retains one authored linear framebuffer across ordered passes and converts exactly once", () => {
    const target = {}
    const operations: string[] = []
    let selected: object | null = null
    const backend: FramePresentationBackend<object> = {
      needsFrameBufferTarget: true,
      getRenderTarget: () => selected,
      setRenderTarget: value => { selected = value; operations.push(value ? "retain-linear-framebuffer" : "canvas") },
      _getFrameBufferTarget: () => target,
      _renderOutput: value => { expect(value).toBe(target); operations.push("source-output-color-transform") },
    }
    const frame = new WebGpuFramePresentation(backend)
    frame.begin()
    for (const phase of ["sky3d", "world", "viewmodel", "hud-model"]) {
      expect(frame.target).toBe(target)
      operations.push(phase)
    }
    frame.finish()
    expect(operations).toEqual(["retain-linear-framebuffer", "sky3d", "world", "viewmodel", "hud-model", "canvas", "source-output-color-transform"])
    expect(frame.active).toBe(false)
  })

  test("preserves direct canvas rendering and rejects overlapping framebuffer ownership", () => {
    const backend: FramePresentationBackend<object> = {
      needsFrameBufferTarget: false, getRenderTarget: () => null,
      setRenderTarget: () => { throw new Error("unexpected target") },
      _getFrameBufferTarget: () => ({}), _renderOutput: () => { throw new Error("unexpected output") },
    }
    const direct = new WebGpuFramePresentation(backend)
    direct.begin()
    direct.finish()
    const occupied = new WebGpuFramePresentation({ ...backend, needsFrameBufferTarget: true, getRenderTarget: () => ({}) })
    expect(() => occupied.begin()).toThrow(/unexpected render target/)
  })

  test("restores canvas ownership after abandoned frames without presenting stale pixels", () => {
    let selected: object | null = null
    let outputs = 0
    const frame = new WebGpuFramePresentation({
      needsFrameBufferTarget: true, getRenderTarget: () => selected,
      setRenderTarget: value => { selected = value },
      _getFrameBufferTarget: () => ({}), _renderOutput: () => { outputs += 1 },
    })
    frame.begin()
    frame.abandon()
    expect(selected).toBeNull()
    expect(outputs).toBe(0)
  })

  test("retires a queued framebuffer generation without a late output submission", () => {
    let selected: object | null = null
    const operations: string[] = []
    const queue = { submit: (buffers: readonly unknown[]) => { operations.push(...buffers as string[]) }, writeBuffer() {} }
    const original = queue.submit
    const frame = new WebGpuFramePresentation({
      needsFrameBufferTarget: true, getRenderTarget: () => selected,
      setRenderTarget: value => { selected = value },
      _getFrameBufferTarget: () => ({}), _renderOutput: () => { operations.push("stale-output") },
    }, queue)
    frame.begin()
    queue.submit(["old-world"])
    frame.abandon()
    frame.dispose()
    operations.push("destroy-target")
    frame.finish()
    expect(operations).toEqual(["old-world", "destroy-target"])
    expect(frame.begun).toBe(false)
    expect(selected).toBeNull()
    expect(queue.submit).toBe(original)
  })
})
