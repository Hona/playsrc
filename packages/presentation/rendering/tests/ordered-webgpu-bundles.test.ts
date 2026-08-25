import { describe, expect, test } from "bun:test"
import { installOrderedWebGpuBundles, type OrderedBundleBackend } from "../src/ordered-webgpu-bundles"

describe("Source world-before-transparent WebGPU bundle submission", () => {
  test("draws the 2D sky before opaque world and depth-tested rocket effects after it", () => {
    const operations: string[] = []
    const context = {}
    const bundle = {}
    const state = {
      currentPass: { executeBundles: (bundles: readonly unknown[]) => operations.push(`world:${bundles.join(",")}`) },
      currentSets: { attributes: { 0: "old-vertex" }, bindingGroups: ["old-bind"], pipeline: "old-pipeline", index: "old-index" },
      renderBundles: [] as unknown[],
    }
    const original = (object: { context: object; object: { userData?: { skyFace?: unknown } } }) => {
      operations.push(object.object.userData?.skyFace === undefined ? "depth-tested-no-depth-write:rocket-flash" : "sky:right")
    }
    const backend: OrderedBundleBackend = {
      get: (identity: object) => identity === context ? state : { bundleGPU: "opaque-world" },
      addBundle: (_context: object, identity: object) => state.renderBundles.push(backend.get(identity).bundleGPU),
      draw: original,
    }
    const restore = installOrderedWebGpuBundles(backend)

    backend.addBundle(context, bundle)
    backend.draw({ context, object: { userData: { skyFace: "right" } } }, {})
    backend.draw({ context, object: { userData: {} } }, {})

    expect(operations).toEqual([
      "sky:right",
      "world:opaque-world",
      "depth-tested-no-depth-write:rocket-flash",
    ])
    expect(state.renderBundles).toEqual([])
    expect(state.currentSets).toEqual({ attributes: {}, bindingGroups: [], pipeline: null, index: null })
    restore()
    expect(backend.draw).toBe(original)
  })

  test("rejects pending commands without an active render pass", () => {
    const context = {}
    const backend: OrderedBundleBackend = {
      get: () => ({ renderBundles: ["world"] }),
      addBundle: () => undefined,
      draw: () => undefined,
    }
    installOrderedWebGpuBundles(backend)
    expect(() => backend.draw({ context, object: {} }, {})).toThrow(/pass state/i)
  })
})
