type Geometry = { addEventListener(type: string, listener: () => void): void }
type GeometryDraw = { geometry: Geometry; getAttributes(): unknown[] }
type GeometryManager = { initGeometry(draw: GeometryDraw): void }

class GeometryAttributeOwner {
  constructor(readonly geometry: Geometry, readonly attributes: unknown[]) {}
  getAttributes() { return this.attributes }
}

/** Geometry initialization owns the exact uploaded attribute objects, not the
 * first render object's material, node builder, skeleton or scene. Source draw
 * layouts replace geometry when their attribute ownership changes. */
export function installGeometryAttributeLifetime(manager: GeometryManager): () => void {
  const original = manager.initGeometry
  const descriptor = Object.getOwnPropertyDescriptor(manager, "initGeometry")
  manager.initGeometry = function (draw) {
    original.call(this, new GeometryAttributeOwner(draw.geometry, draw.getAttributes()))
  }
  return () => {
    if (descriptor) Object.defineProperty(manager, "initGeometry", descriptor)
    else delete (manager as Partial<GeometryManager>).initGeometry
  }
}
