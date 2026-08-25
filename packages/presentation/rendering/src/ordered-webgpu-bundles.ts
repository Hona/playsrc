type PipelineState = {
  attributes: Record<number, unknown>
  bindingGroups: unknown[]
  pipeline: unknown
  index: unknown
}

type RenderPass = Readonly<{ executeBundles(bundles: readonly unknown[]): void }>

type RenderContextData = {
  currentPass?: RenderPass | null
  currentSets?: PipelineState
  renderBundles?: unknown[]
}

type BackendRenderObject = Readonly<{
  context: object
  object: Readonly<{ userData?: Readonly<{ skyFace?: unknown }> }>
}>

export type OrderedBundleBackend = {
  get(identity: object): RenderContextData & { bundleGPU?: unknown }
  addBundle(context: object, bundle: object): void
  draw(object: BackendRenderObject, info: unknown): void
}

export function installOrderedWebGpuBundles(backend: OrderedBundleBackend): () => void {
  const original = backend.draw
  if (typeof original !== "function" || typeof backend.addBundle !== "function" || typeof backend.get !== "function") {
    throw new Error("WebGPU render-bundle backend is unavailable")
  }

  backend.draw = (object: BackendRenderObject, info: unknown): void => {
    const state = backend.get(object.context)
    const pending = state.renderBundles
    if (pending && pending.length > 0 && object.object.userData?.skyFace === undefined) {
      if (!state.currentPass || typeof state.currentPass.executeBundles !== "function") {
        throw new Error("WebGPU render-bundle pass state is unavailable")
      }
      state.currentPass.executeBundles(pending)
      pending.length = 0
      state.currentSets = { attributes: {}, bindingGroups: [], pipeline: null, index: null }
    }
    original.call(backend, object, info)
  }

  return () => {
    backend.draw = original
  }
}
