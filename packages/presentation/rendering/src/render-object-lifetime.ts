type Geometry = { addEventListener(type: string, listener: () => void): void; removeEventListener(type: string, listener: () => void): void }
type RenderObject = { object: object; geometry: Geometry; onGeometryDispose: () => void; onDispose: () => void; dispose(): void; setGeometry(geometry: Geometry): void }
type RenderObjectManager = { createRenderObject(...args: any[]): RenderObject }
type Root = { traverse(visit: (object: object) => void): void }

/** Materials and geometry retain Three's per-draw dispose listeners. A resource
 * handoff must retire old draws separately, or those listeners retain the old
 * scene even though its immutable resources now belong to the replacement. */
export function installRenderObjectLifetime(manager: RenderObjectManager) {
  const original = manager.createRenderObject
  const descriptor = Object.getOwnPropertyDescriptor(manager, "createRenderObject")
  let objects = new WeakMap<object, Set<RenderObject>>()
  manager.createRenderObject = function (...args) {
    const renderObject = original.apply(this, args)
    let owned = objects.get(renderObject.object)
    if (!owned) objects.set(renderObject.object, owned = new Set())
    owned.add(renderObject)
    const setGeometry = renderObject.setGeometry
    renderObject.setGeometry = function (geometry) {
      const previous = this.geometry
      if (previous !== geometry) previous.removeEventListener("dispose", this.onGeometryDispose)
      setGeometry.call(this, geometry)
      if (previous !== geometry) geometry.addEventListener("dispose", this.onGeometryDispose)
    }
    const dispose = renderObject.onDispose
    renderObject.onDispose = () => {
      if (!owned.delete(renderObject)) return
      dispose.call(renderObject)
    }
    return renderObject
  }
  return {
    release(root: Root) {
      root.traverse(object => {
        const owned = objects.get(object)
        if (owned) for (const renderObject of [...owned]) renderObject.dispose()
        objects.delete(object)
      })
    },
    restore() {
      if (descriptor) Object.defineProperty(manager, "createRenderObject", descriptor)
      else delete (manager as Partial<RenderObjectManager>).createRenderObject
      objects = new WeakMap()
    },
  }
}
