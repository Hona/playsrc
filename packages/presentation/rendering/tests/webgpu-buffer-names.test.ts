import { expect, test } from "bun:test"
import { buffer, storage, uniform } from "three/tsl"
import { StorageBufferAttribute, WGSLNodeBuilder } from "three/webgpu"
import { installWebGpuBufferNames } from "../src/webgpu-buffer-names"

function backend() {
  return { createNodeBuilder: () => new WGSLNodeBuilder(null, {}) }
}

test("equivalent palette programs do not inherit process-global node uniform IDs", () => {
  const gpu = backend()
  installWebGpuBufferNames(gpu)
  const create = () => {
    const builder = gpu.createNodeBuilder()
    builder.getUniformFromNode(uniform(1), "float", "vertex")
    const palette = buffer(new Float32Array(128 * 16), "mat4", 128)
    return builder.getUniformFromNode(palette, "buffer", "vertex").name
  }
  const first = create()
  for (let i = 0; i < 10; i++) create()
  expect(create()).toBe(first)
})

test("stable declarations retain separate palettes, binding slots, sizes and explicit names", () => {
  const gpu = backend()
  installWebGpuBufferNames(gpu)
  const build = (count: number) => {
    const builder = gpu.createNodeBuilder()
    const data = new Float32Array(count * 16)
    data[0] = count
    const palette = buffer(data, "mat4", count)
    const first = builder.getUniformFromNode(palette, "buffer", "vertex")
    expect(builder.getUniformFromNode(palette, "buffer", "vertex")).toBe(first)
    const second = builder.getUniformFromNode(buffer(new Float32Array(16), "mat4", 1), "buffer", "vertex", "explicitPalette")
    expect(second.name).toBe("explicitPalette")
    expect(second.name).not.toBe(first.name)
    const bindings = builder.getBindings()[0].bindings
    expect(bindings).toHaveLength(2)
    expect(bindings[0].buffer).toBe(data)
    return { data, bindings, code: builder.getUniforms("vertex") }
  }
  const a = build(128), b = build(128), small = build(64)
  expect(a.code).toBe(b.code)
  expect(a.bindings[0]).not.toBe(b.bindings[0])
  expect(a.data).not.toBe(b.data)
  expect(small.code).not.toBe(a.code)
  expect(a.code).toContain("@binding( 0 ) @group( 0 )")
  expect(a.code).toContain("@binding( 1 ) @group( 0 )")
  expect(a.code).toContain("array< mat4x4<f32>, 128 >")
})

test("buffer names are stable across vertex, fragment and storage declarations without renaming ordinary uniforms", () => {
  const gpu = backend()
  const original = gpu.createNodeBuilder
  const restore = installWebGpuBufferNames(gpu)
  const build = () => {
    const builder = gpu.createNodeBuilder()
    const ordinary = builder.getUniformFromNode(uniform(1), "float", "vertex")
    expect(ordinary.name).toBe("nodeUniform0")
    const palette = buffer(new Float32Array(16), "mat4", 1)
    const vertex = builder.getUniformFromNode(palette, "buffer", "vertex")
    const fragment = builder.getUniformFromNode(palette, "buffer", "fragment")
    const data = new StorageBufferAttribute(new Float32Array(16), 4)
    const values = storage(data, "vec4").toReadOnly()
    const compute = builder.getUniformFromNode(values, "storageBuffer", "compute")
    expect(new Set([ordinary.name, vertex.name, fragment.name, compute.name]).size).toBe(4)
    return [ordinary.name, vertex.name, fragment.name, compute.name]
  }
  expect(build()).toEqual(build())
  restore()
  expect(gpu.createNodeBuilder).toBe(original)
})

test("Three's declaration allocator resolves a prior explicit name without aliasing buffers", () => {
  const gpu = backend()
  installWebGpuBufferNames(gpu)
  const builder = gpu.createNodeBuilder()
  builder.shaderStage = "vertex"
  const explicit = builder.getUniformFromNode(buffer(new Float32Array(16), "mat4", 1), "buffer", "vertex", "nodeUniform1")
  const automatic = builder.getUniformFromNode(buffer(new Float32Array(16), "mat4", 1), "buffer", "vertex")
  expect(explicit.name).toBe("nodeUniform1")
  expect(automatic.name).toBe("nodeUniform1_1")
  expect(builder.getBindings()[0].bindings).toHaveLength(2)
})

test("an explicit buffer name cannot alias a preceding automatic declaration", () => {
  const gpu = backend()
  installWebGpuBufferNames(gpu)
  const builder = gpu.createNodeBuilder()
  builder.shaderStage = "vertex"
  const automatic = builder.getUniformFromNode(buffer(new Float32Array(16), "mat4", 1), "buffer", "vertex")
  const explicit = builder.getUniformFromNode(buffer(new Float32Array(16), "mat4", 1), "buffer", "vertex", automatic.name)
  expect(explicit.name).not.toBe(automatic.name)
  expect(explicit.name).toBe(`${automatic.name}_1`)
})
