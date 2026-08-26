import type { Camera } from "@playsrc/presentation-rendering"

export type PresentedCameraRevisions = Readonly<{
  generation: number
  viewportRevision: number
  preparedRevision: number
  viewRevision: number
  mouseRevision: number
  snapRevision: number
  tick: bigint
}>

export type AuthoredSkyCamera = Readonly<{
  origin: readonly [number, number, number]
  scale: number
  area: number
}>

export type PresentedCamera = Readonly<{
  revisions: PresentedCameraRevisions
  main: Camera
  sky: Camera | null
  controller: AuthoredSkyCamera | null
}>

export function selectPresentedCamera(camera: Camera, override: Partial<Camera> | undefined): Camera {
  return override && Array.isArray(override.position) && override.position.length === 3
    && override.position.every(Number.isFinite)
    && Number.isFinite(override.yawDegrees) && Number.isFinite(override.pitchDegrees)
    ? Object.freeze({
      ...camera,
      position: Object.freeze([...override.position]) as readonly [number, number, number],
      yawDegrees: override.yawDegrees!,
      pitchDegrees: override.pitchDegrees!,
    })
    : camera
}

export function presentCamera(
  camera: Camera,
  revisions: PresentedCameraRevisions,
  controller: AuthoredSkyCamera | null,
): PresentedCamera {
  const position = Object.freeze([...camera.position]) as readonly [number, number, number]
  const main = Object.freeze({ ...camera, position })
  if (!controller) return Object.freeze({ revisions: Object.freeze({ ...revisions }), main, sky: null, controller: null })
  const scale = controller.scale > 0 ? controller.scale : 1
  const authored = Object.freeze({ ...controller, origin: Object.freeze([...controller.origin]) as readonly [number, number, number] })
  const sky = Object.freeze({
    ...main,
    position: Object.freeze(authored.origin.map((value, axis) => value + position[axis]! / scale)) as readonly [number, number, number],
    near: 2,
    far: 32_768 * 1.732050807569,
  })
  return Object.freeze({ revisions: Object.freeze({ ...revisions }), main, sky, controller: authored })
}

function equalCamera(left: Camera | null, right: Camera | null): boolean {
  return left === right || left !== null && right !== null
    && left.position.every((value, axis) => Object.is(value, right.position[axis]))
    && Object.is(left.yawDegrees, right.yawDegrees)
    && Object.is(left.pitchDegrees, right.pitchDegrees)
    && Object.is(left.verticalFovDegrees, right.verticalFovDegrees)
    && Object.is(left.near, right.near)
    && Object.is(left.far, right.far)
}

export function equivalentPresentedVisibility(left: PresentedCamera, right: PresentedCamera): boolean {
  const first = left.revisions, second = right.revisions
  if (first.generation !== second.generation || first.viewportRevision !== second.viewportRevision
    || first.preparedRevision !== second.preparedRevision || first.tick !== second.tick
    || !equalCamera(left.main, right.main) || !equalCamera(left.sky, right.sky)) return false
  const original = left.controller, candidate = right.controller
  return original === candidate || original !== null && candidate !== null
    && original.origin.every((value, axis) => Object.is(value, candidate.origin[axis]))
    && Object.is(original.scale, candidate.scale) && Object.is(original.area, candidate.area)
}

export function currentPresentedCamera(
  presented: PresentedCamera,
  current: Pick<PresentedCameraRevisions, "generation" | "viewportRevision" | "viewRevision" | "mouseRevision" | "snapRevision">,
): boolean {
  const revisions = presented.revisions
  return revisions.generation === current.generation
    && revisions.viewportRevision === current.viewportRevision
    && revisions.viewRevision === current.viewRevision
    && revisions.mouseRevision === current.mouseRevision
    && revisions.snapRevision === current.snapRevision
}
