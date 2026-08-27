import { NodeBuilder } from "three/webgpu"

type Uniform = { name: string }
type Builder = {
  getUniformFromNode(node: object, type: string, stage: string, name?: string | null): Uniform
}

export type BufferNamingBackend = { createNodeBuilder(...args: any[]): Builder }

/** Three's default WGSL buffer name contains a process-global NodeUniform ID.
 * Identical actor/material programs then miss both the program and Dawn caches.
 * Supply a declaration-local name before WGSL generation instead. Binding slots,
 * buffer ownership/layout, shader arithmetic and explicitly named nodes stay intact. */
export function installWebGpuBufferNames(backend: BufferNamingBackend): () => void {
  const original = backend.createNodeBuilder
  if (typeof original !== "function") throw new Error("WebGPU node builder is unavailable")
  const descriptor = Object.getOwnPropertyDescriptor(backend, "createNodeBuilder")
  backend.createNodeBuilder = function (...args) {
    const builder = original.apply(this, args)
    const uniform = builder.getUniformFromNode
    if (typeof uniform !== "function") {
      throw new Error("WebGPU uniform declaration contract is unavailable")
    }
    builder.getUniformFromNode = function (node, type, stage, name) {
      if (type === "buffer" || type === "storageBuffer" || type === "indirectStorageBuffer") {
        // Let Three's base declaration allocator own uniqueness, including
        // explicitly named uniforms/varyings. The WGSL override otherwise
        // replaces this already-registered local name with a global ID.
        name = NodeBuilder.prototype.getUniformFromNode.call(this, node, type, stage, name).name
      }
      return uniform.call(this, node, type, stage, name)
    }
    return builder
  }
  return () => {
    if (descriptor) Object.defineProperty(backend, "createNodeBuilder", descriptor)
    else delete (backend as Partial<BufferNamingBackend>).createNodeBuilder
  }
}
