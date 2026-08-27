import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { installNodeBuilderInstrumentation, observeStaticPropUse, RendererFrameInstrumentation, type BrowserFrameProfiler } from "../src/frame-instrumentation"

test("cold-model pass attribution does not turn loading into completed gameplay", () => {
  const profile: BrowserFrameProfiler = { active: false, captureModelPrograms: true, currentPass: null, completedFrames: [], counters: {}, capabilities: { timestampQuery: false, longAnimationFrame: false }, losses: [] }
  const info = { autoReset: true, reset() {}, render: { drawCalls: 0, frameCalls: 0, triangles: 0 }, memory: {} }
  const instrumentation = new RendererFrameInstrumentation(info, profile)
  instrumentation.pass("main", () => expect(profile.currentPass?.identity).toBe("main"))
  expect(profile.currentPass).toBeNull()
  expect(instrumentation.complete()).toBeUndefined()
  expect(profile.completedFrames).toHaveLength(0)
  expect(profile.counters.completedFrames).toBeUndefined()
})

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

test("static-prop first-use evidence preserves draw callbacks, excludes loading and retains only scalar identities", () => {
  const { profile, info } = fixture(false)
  const mesh = new THREE.Mesh(), material = mesh.material as THREE.Material
  mesh.userData.materialIdentity = "authored-prop"
  mesh.userData.sourceStaticFade = { value: .75 }
  let calls = 0
  const original = mesh.onBeforeRender = () => { calls++ }
  observeStaticPropUse(mesh, profile, 2, 208)
  const draw = () => mesh.onBeforeRender(null as any, null as any, null as any, mesh.geometry, material, null as any)
  draw()
  expect(profile.firstStaticPropUses).toBeUndefined()
  profile.active = true
  new RendererFrameInstrumentation(info, profile).pass("main", draw)
  draw()
  expect(calls).toBe(3)
  expect(mesh.onBeforeRender).toBe(original)
  expect(profile.firstStaticPropUses).toHaveLength(1)
  expect(profile.firstStaticPropUses![0]).toMatchObject({ generation: 2, source: 208, material: material.id, identity: "authored-prop", pass: "main", fade: .75 })
})

describe("completed multi-pass renderer instrumentation", () => {
  test("accumulates sky, world, viewmodel and HUD until exactly one completed-frame reset", () => {
    const { info, profile, resets } = fixture()
    const instrumentation = new RendererFrameInstrumentation(info, profile, { has: feature => feature === "timestamp-query" })
    for (const identity of ["sky3d", "main", "viewmodel", "hud-model"]) {
      instrumentation.pass(identity, () => { info.render.drawCalls += 2; info.render.frameCalls += 1; info.render.triangles += 9; profile.currentPass!.submissions += 1 })
    }
    instrumentation.poseUpload(96)
    instrumentation.indexUpload(24)
    instrumentation.indirectUpload(8)
    instrumentation.invalidateBundle()
    profile.counters.bundleEncodes = 2
    profile.counters.bundleEncodeMilliseconds = 1.25
    expect(resets()).toBe(1)
    expect(instrumentation.complete()).toMatchObject({
      drawCalls: 8, frameCalls: 4, triangles: 36, timestampMilliseconds: 12,
      poseUploadBytes: 96, indexUploadBytes: 24, indirectUploadBytes: 8, bundleInvalidations: 1, bundleEncodes: 2, bundleEncodeMilliseconds: 1.25,
      memory: { textures: 3, texturesSize: 128, attributesSize: 64, uniformBuffersSize: 16, total: 208 },
      passes: ["sky3d", "main", "viewmodel", "hud-model"].map(identity => ({ identity, submissions: 1, drawCalls: 2 })),
    })
    expect(resets()).toBe(2)
    expect(info.autoReset).toBe(false)
    expect(instrumentation.complete()).toMatchObject({ bundleEncodes: 0, bundleEncodeMilliseconds: 0, indirectUploadBytes: 0 })
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

  test("attributes actual node builds and pipeline creation to the conceptual pass", () => {
    const { info, profile } = fixture()
    profile.counters.renderPipelines = 0
    profile.counters.nodeBuilderMisses = 0
    const manager = { _createNodeBuilder: () => ({ build() { return "authored" }, vertexShader: "vertex", fragmentShader: "fragment" }) }
    const original = manager._createNodeBuilder
    const restore = installNodeBuilderInstrumentation(manager, profile)
    const instrumentation = new RendererFrameInstrumentation(info, profile)
    instrumentation.pass("main", () => {
      expect(manager._createNodeBuilder().build()).toBe("authored")
      profile.counters.renderPipelines! += 1
    })
    expect(instrumentation.complete()?.passes[0]).toMatchObject({ identity: "main", renderPipelines: 1, nodeBuilderMisses: 1 })
    expect(profile.counters.nodeBuilderMilliseconds).toBeGreaterThanOrEqual(0)
    expect(profile.nodeBuilds).toHaveLength(1)
    expect(profile.nodeBuilds![0]).toMatchObject({ pass: "main", vertexCharacters: 6, fragmentCharacters: 8 })
    restore()
    expect(manager._createNodeBuilder).toBe(original)
  })
})
