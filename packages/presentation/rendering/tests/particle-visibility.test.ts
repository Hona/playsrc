import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { ParticleVisibilityQueries } from "../src/particle-visibility"

function fixture() {
  let submitted = false, readCalls = 0, issued = 0, ended = 0
  const releases: ((values: Uint32Array) => void)[] = []
  const counters: any[] = []
  const queries = new ParticleVisibilityQueries(() => {
    const counter = { pending: false, bufferBytes: 208, prepare: async () => {}, dispose: () => {},
      issue: (_encoder: unknown, _depth: unknown, vertices: Float32Array, format: string, color: unknown) => {
        expect(counter.pending).toBe(false); expect(vertices.length).toBe(20); expect(format).toBe("rgba8unorm"); expect(color).toBeTruthy()
        issued++; counter.pending = true
        return async () => {
          expect(submitted).toBe(true); readCalls++
          try { return await new Promise<Uint32Array>(resolve => releases.push(resolve)) }
          finally { counter.pending = false }
        }
      },
    }
    counters.push(counter); return counter
  })
  const pass = () => ({ end: () => { ended++ } })
  const state: any = { currentPass: pass(), descriptor: { colorAttachments: [{ view: {} }], depthStencilAttachment: {} },
    encoder: { beginRenderPass: () => pass() } }
  const backend = { isWebGPUBackend: true, device: {}, get: () => state, finishRender: () => state.currentPass.end(),
    textureUtils: { getDepthBuffer: () => ({ sampleCount: 4 }) }, context: { getCurrentTexture: () => ({ format: "rgba8unorm" }) } }
  const renderer = { backend, getRenderTarget: () => null } as any
  const camera = new THREE.PerspectiveCamera(); camera.projectionMatrix.identity(); camera.matrixWorldInverse.identity()
  queries.attach(renderer)
  const draw = (name = "main") => {
    state.currentPass = pass(); queries.beginPass(name, renderer, camera)
    backend.finishRender({ depth: true, renderTarget: null }); queries.endPass()
  }
  const flush = () => { submitted = true; queries.flushReads() }
  return { queries, draw, flush, releases, counters, counts: () => ({ issued, ended, readCalls }) }
}

test("sample readback starts after submission, holds pending queries, and uses exact integer counts", async () => {
  const f = fixture(), proxy = { identity: 7n, vertices: new Float32Array(15), clipFraction: 0.5 }
  await f.queries.prepare()
  f.queries.stage([{ visibility: proxy }]); f.draw()
  expect(f.counts()).toEqual({ issued: 1, ended: 2, readCalls: 0 })
  f.draw(); expect(f.counts().issued).toBe(1)
  f.flush(); expect(f.counts().readCalls).toBe(1)
  f.releases[0]!(new Uint32Array([40, 80])); await Promise.resolve(); await Promise.resolve()
  expect(f.queries.takeSamples()).toEqual([{ identity: 7n, visiblePixels: 40, possiblePixels: 80, clipFraction: 0.5 }])
  expect(f.queries.evidence()).toMatchObject({ issued: 1, readbackBytes: 8, vertexBytes: 192 })
  f.queries.dispose()
})

test("sky queries use their own pass and retired generations never accept late readback", async () => {
  const f = fixture(), proxy = { identity: 9n, vertices: new Float32Array(15), clipFraction: 1 }
  await f.queries.prepare(); f.queries.stage([{ visibility: proxy, sky: true }])
  f.draw(); expect(f.counts().issued).toBe(0)
  f.draw("sky3d"); expect(f.counts().issued).toBe(1)
  f.flush(); f.queries.reset()
  f.releases[0]!(new Uint32Array([1, 2])); await Promise.resolve(); await Promise.resolve()
  expect(f.queries.takeSamples()).toEqual([])
  f.queries.dispose()
})
