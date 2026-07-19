import { describe, expect, test } from "bun:test"
import {
  FRAME_PHASES,
  SourceViewStack,
  executeFrameGraph,
  interpolatePresentation,
  orderTransparentItems,
  validateDynamicLights,
  validateShadows,
  visibleTriangleIndices,
  type ViewState,
} from "../src/frame-foundations"

const view = (identity: string): ViewState => Object.freeze({
  identity,
  cameraIdentity: `${identity}:camera`,
  projectionIdentity: `${identity}:projection`,
  viewport: [0, 0, 320, 180],
  colorTarget: `${identity}:color`,
  depthStencilTarget: `${identity}:depth-stencil`,
  depthStencil: Object.freeze({
    depthLoad: "clear", depthStore: "store", depthClear: 1, depthCompare: "less-equal", depthWrite: true,
    depthRange: [0, 1], depthBias: 0, depthBiasSlopeScale: 0, depthBiasClamp: 0,
    stencilLoad: "clear", stencilStore: "store", stencilClear: 0,
    stencilReadMask: 0xff, stencilWriteMask: 0xff, stencilReference: 0,
  }),
  clips: Object.freeze([]),
  fogIdentity: null,
  visibilityIdentity: `${identity}:visibility`,
  drawFlags: 1,
  debug: "color",
  framebufferCopy: null,
})

describe("transparent ordering and per-face visibility", () => {
  test("traverses leaves and source order backwards, copies immediately, and draws ignore-depth last", () => {
    const operations = orderTransparentItems([10, 20, 30], [
      { identity: 1n, family: "world", leaf: 20, sourceOrder: 1, ignoreDepth: false, framebuffer: "none" },
      { identity: 2n, family: "entity", leaf: 30, sourceOrder: 1, ignoreDepth: false, framebuffer: "full-frame" },
      { identity: 3n, family: "particle", leaf: 30, sourceOrder: 2, ignoreDepth: false, framebuffer: "none" },
      { identity: 4n, family: "sprite", leaf: 10, sourceOrder: 9, ignoreDepth: true, framebuffer: "power-of-two" },
    ])
    expect(operations.map((operation) => operation.kind === "draw" ? `draw:${operation.item.identity}` : `copy:${operation.consumer}`)).toEqual([
      "draw:3", "copy:2", "draw:2", "draw:1", "copy:4", "draw:4",
    ])
  })

  test("filters every triangle independently and orders translucent faces back-to-front", () => {
    const indices = new Uint32Array([0,1,2, 3,4,5, 6,7,8, 9,10,11])
    const faces = new Uint32Array([7, 8, 7, 9])
    expect([...visibleTriangleIndices(indices, faces, new Set([7]))]).toEqual([0,1,2, 6,7,8])
    expect([...visibleTriangleIndices(indices, faces, new Set([7,9]), new Map([[7,0],[9,2]]))]).toEqual([9,10,11, 0,1,2, 6,7,8])
    expect(() => visibleTriangleIndices(indices, new Uint32Array([7]), new Set([7]))).toThrow(/invalid/i)
  })
})

describe("view stack and frame graph", () => {
  test("restores every nested view value and rejects mismatched pop", () => {
    const stack = new SourceViewStack(3)
    stack.push(view("main"))
    stack.push({ ...view("reflection"), clips: [{ normal: [0,0,1], distance: 20, keep: "positive" }], fogIdentity: "water" })
    expect(stack.current()?.identity).toBe("reflection")
    expect(stack.pop("reflection")).toEqual(view("main"))
    expect(stack.depth).toBe(1)
    expect(() => stack.pop("wrong")).toThrow(/identity/i)
  })

  test("executes the fixed phase order and rolls staged work back in reverse on failure", async () => {
    const success = await executeFrameGraph(() => ({}))
    expect(success.phases).toEqual(FRAME_PHASES)
    expect(success.submitted).toBe(true)

    const rollback: string[] = []
    await expect(executeFrameGraph((phase) => {
      if (phase === "main-translucent") throw new Error("sentinel")
      return { rollback: () => rollback.push(phase) }
    })).rejects.toThrow(/main-translucent.*7 rollbacks/i)
    expect(rollback).toEqual([
      "main-opaque-depth", "auxiliary-views", "upload", "prune-and-sort", "derive-views", "interpolate-and-pose", "validate-input",
    ])
    const completeRollback: string[] = []
    await expect(executeFrameGraph((phase) => {
      if (phase === "derive-views") throw new Error("phase")
      return { rollback: () => { completeRollback.push(phase); if (phase === "interpolate-and-pose") throw new Error("cleanup") } }
    })).rejects.toThrow(/2 rollbacks.*cleanup/i)
    expect(completeRollback).toEqual(["interpolate-and-pose", "validate-input"])
  })
})

describe("presentation interpolation and typed light/shadow requirements", () => {
  test("handles linear, shortest-angle, quaternion, discrete, and discontinuity policies", () => {
    const output = interpolatePresentation({
      fraction: 0.5,
      discontinuities: new Set(["teleport"]),
      fields: [
        { identity: "position", policy: "linear", previous: [0,10,20], current: [10,20,30] },
        { identity: "angle", policy: "shortest-angle-degrees", previous: 350, current: 10 },
        { identity: "rotation", policy: "quaternion", previous: [0,0,0,1], current: [0,0,1,0] },
        { identity: "skin", policy: "discrete", previous: 0, current: 2 },
        { identity: "teleport", policy: "linear", previous: [0,0,0], current: [100,200,300] },
      ],
    })
    expect(output.get("position")).toEqual([5,15,25])
    expect(output.get("angle")).toBe(360)
    const rotation = output.get("rotation") as readonly number[]
    expect(rotation[0]).toBe(0)
    expect(rotation[1]).toBe(0)
    expect(rotation[2]).toBeCloseTo(Math.SQRT1_2, 12)
    expect(rotation[3]).toBeCloseTo(Math.SQRT1_2, 12)
    expect(output.get("skin")).toBe(2)
    expect(output.get("teleport")).toEqual([100,200,300])
    expect(() => interpolatePresentation({ fraction: 1.1, discontinuities: new Set(), fields: [] })).toThrow(/fraction/i)
  })

  test("accepts complete dynamic-light and shadow records and rejects missing bounds", () => {
    const light = Object.freeze({
      identity: 1n, kind: "dlight" as const, key: 7, flags: 0, origin: [0,0,10] as const,
      radius: 100, color: [255,128,64] as const, colorExponent: 1,
      dieTime: 3, decayPerSecond: 10, minimumLight: 0.01, style: 0, styleScalar: 1,
      direction: [0,0,-1] as const, innerAngle: 30, outerAngle: 45, currentTime: 2,
    })
    expect(validateDynamicLights([light], 1)).toEqual([light])
    expect(() => validateDynamicLights([light], 0)).toThrow(/bound/i)
    const shadow = Object.freeze({
      identity: 2n, kind: "render-to-texture" as const, caster: 1n, receivers: [3n],
      worldToShadow: new Float32Array([1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]),
      projectionOrigin: [0,0,10] as const, projectionDirection: [0,0,-1] as const, projectionSize: [32,32] as const,
      casterOrigin: [0,0,0] as const, casterBounds: [[-1,-1,-1],[1,1,1]] as const, leaves: [4,5],
      maximumHeight: 100, falloffOffset: 10, falloffAmount: 0.5, falloffBias: 8,
      textureIdentity: "shadow:2", textureOrigin: [0,0] as const, textureSize: [32,32] as const,
      clipPlanes: Object.freeze([]), depthBias: 1, slopeScaleBias: 2, enabled: true,
      viewIdentity: "main", projectedLight: null,
    })
    expect(validateShadows([shadow], 1)).toEqual([shadow])
    expect(() => validateShadows([{ ...shadow, falloffBias: 256 }], 1)).toThrow(/invalid/i)
  })
})
