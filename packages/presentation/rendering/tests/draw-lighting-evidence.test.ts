import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { ModelLightingGraphs } from "../src/model-lighting-graphs"
import { installDrawLightingEvidence } from "../src/draw-lighting-evidence"
import { RendererFrameInstrumentation } from "../src/frame-instrumentation"

test("explicit differential evidence restores renderer, pass and lighting descriptors", () => {
  const graph = new ModelLightingGraphs()
  const prototype = Object.getPrototypeOf((graph.lighting.ambientEnabled as any)._beforeNodes[0])
  const before = Object.getOwnPropertyDescriptor(prototype, "update")
  const render = THREE.WebGPURenderer.prototype.render, pass = RendererFrameInstrumentation.prototype.pass
  const evidence = installDrawLightingEvidence()
  expect(THREE.WebGPURenderer.prototype.render).not.toBe(render)
  evidence.dispose()
  expect(Object.getOwnPropertyDescriptor(prototype, "update")).toEqual(before)
  expect(THREE.WebGPURenderer.prototype.render).toBe(render)
  expect(RendererFrameInstrumentation.prototype.pass).toBe(pass)
})
