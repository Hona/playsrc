type Draw = (object: any, scene: any, camera: any, geometry: any, material: any, group: any, lights: any, clipping: any, pass?: string | null) => void
type Backend = { renderObject: Draw; getRenderObjectFunction(): Draw | null; setRenderObjectFunction(draw: Draw | null): void }

/** A synchronous draw may span many cold native uploads/pipeline creations.
 * Service the audio device between objects, without yielding or changing any
 * draw, render state, simulation tick or client presentation transaction. */
export function installRenderService(backend: Backend, service: () => void): void {
  const draw = backend.getRenderObjectFunction() ?? backend.renderObject
  backend.setRenderObjectFunction(function (object, scene, camera, geometry, material, group, lights, clipping, pass) {
    service()
    draw.call(backend, object, scene, camera, geometry, material, group, lights, clipping, pass)
  })
}
