type Draw = (object: any, scene: any, camera: any, geometry: any, material: any, group: any, lights: any, clipping: any, pass?: string | null) => void
type Backend = { renderObject: Draw; getRenderObjectFunction(): Draw | null; setRenderObjectFunction(draw: Draw | null): void }

/** A synchronous draw may span many cold native uploads/pipeline creations.
 * Service the audio device between objects, without yielding or changing any
 * draw, render state, simulation tick or client presentation transaction. */
export function installRenderService(backend: Backend, service: () => void): void {
  const installed = backend.getRenderObjectFunction()
  let inside = false
  const wrap = (draw: Draw): Draw => function (object, scene, camera, geometry, material, group, lights, clipping, pass) {
    const nested = inside
    inside = true
    try {
      if (!nested) service()
      draw.call(backend, object, scene, camera, geometry, material, group, lights, clipping, pass)
    } finally { inside = nested }
  }
  // compileAsync selects renderObject directly instead of the installed draw
  // callback. Keep the same between-object service during cold preparation.
  backend.renderObject = wrap(backend.renderObject)
  backend.setRenderObjectFunction(installed ? wrap(installed) : backend.renderObject)
}
