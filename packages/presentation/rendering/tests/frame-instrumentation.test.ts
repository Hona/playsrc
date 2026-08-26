import { describe, expect, test } from "bun:test"
import { RendererFrameInstrumentation, type BrowserFrameProfiler } from "../src/frame-instrumentation"

function fixture(active = true) {
  let resets = 0
  const info = {
    autoReset: true,
    reset() { resets += 1; info.render.drawCalls = 0; info.render.frameCalls = 0; info.render.triangles = 0 },
    render: { drawCalls: 0, frameCalls: 0, triangles: 0, timestamp: 12 },
    memory: { textures: 3, texturesSize: 128, geometries: 2, attributesSize: 64, uniformBuffers: 1, uniformBuffersSize: 16, programs: 4, total: 208 },
  }
  const profile: BrowserFrameProfiler = { active, currentPass: null, completedFrames: [], counters: {}, capabilities: { timestampQuery: false, longAnimationFrame: false }, losses: [] }
  return { info, profile, resets: () => resets }
}

describe("completed multi-pass renderer instrumentation", () => {
  test("accumulates sky, world, viewmodel and HUD until exactly one completed-frame reset", () => {
    const { info, profile, resets } = fixture()
    const instrumentation = new RendererFrameInstrumentation(info, profile, { has: feature => feature === "timestamp-query" })
    for (const identity of ["sky3d", "main", "viewmodel", "hud-model"]) {
      instrumentation.pass(identity, () => { info.render.drawCalls += 2; info.render.frameCalls += 1; info.render.triangles += 9; profile.currentPass!.submissions += 1 })
    }
    instrumentation.poseUpload(96)
    instrumentation.indexUpload(24)
    instrumentation.invalidateBundle()
    expect(resets()).toBe(1)
    expect(instrumentation.complete()).toMatchObject({
      drawCalls: 8, frameCalls: 4, triangles: 36, timestampMilliseconds: 12,
      poseUploadBytes: 96, indexUploadBytes: 24, bundleInvalidations: 1,
      memory: { textures: 3, texturesSize: 128, attributesSize: 64, uniformBuffersSize: 16, total: 208 },
      passes: ["sky3d", "main", "viewmodel", "hud-model"].map(identity => ({ identity, submissions: 1, drawCalls: 2 })),
    })
    expect(resets()).toBe(2)
    expect(info.autoReset).toBe(false)
  })

  test("does not sample disabled frames and honestly reports unsupported timestamp queries", () => {
    const { info, profile, resets } = fixture(false)
    const instrumentation = new RendererFrameInstrumentation(info, profile)
    expect(instrumentation.pass("main", () => 42)).toBe(42)
    instrumentation.poseUpload(20)
    expect(instrumentation.complete()).toBeUndefined()
    expect(resets()).toBe(2)
    profile.active = true
    expect(instrumentation.complete()).toMatchObject({ timestampMilliseconds: null, poseUploadBytes: 0, passes: [] })
  })

  test("restores enclosing pass identity if renderer work throws", () => {
    const { info, profile } = fixture()
    const instrumentation = new RendererFrameInstrumentation(info, profile)
    expect(() => instrumentation.pass("main", () => { throw new Error("failed") })).toThrow("failed")
    expect(profile.currentPass).toBeNull()
    expect(instrumentation.complete()?.passes[0]?.identity).toBe("main")
  })
})
