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
