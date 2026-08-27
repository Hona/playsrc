import type { PresentationArtifacts } from "./artifacts"
import type { Snapshot } from "./codec"
import type { ModelPoseRequest } from "./presentation"

/** Query only authored animations selected by the map authority. These requests
 * do not dispatch entity inputs, sample simulation, or advance an animation. */
export function mapPropPipelinePoseRequests(artifacts: PresentationArtifacts, snapshot: Snapshot, camera: {
  position: readonly [number, number, number]; yawDegrees: number; pitchDegrees: number; far: number
}, aspect: number): readonly Readonly<{ request: ModelPoseRequest; pass: "world" }>[] {
  const selected = new Map(snapshot.entityPresentation.studioAnimations.map(animation => [animation.sourceIndex, {
    activity: animation.sequence, elapsedSeconds: animation.elapsedSeconds, body: undefined as number | undefined,
  }]))
  // Authored doors and lockers need not have received their first input yet.
  // Rust emits only a sequence actually declared for that map model.
  for (const occurrence of artifacts.modelOccurrences) if (occurrence.pipelineAnimation && !selected.has(occurrence.entity)) {
    selected.set(occurrence.entity, { activity: occurrence.pipelineAnimation, elapsedSeconds: 0, body: occurrence.body })
  }
  for (const event of snapshot.regenerateAnimationEvents) {
    const closed = snapshot.tick >= event.closeTick
    selected.set(event.associatedModel, {
      activity: closed ? event.closeAnimation : event.openAnimation,
      elapsedSeconds: Math.max(0, Number(snapshot.tick - (closed ? event.closeTick : event.openTick)) * 0.015), body: event.body,
    })
  }
  const variants = new Set<string>()
  const output: { request: ModelPoseRequest; pass: "world" }[] = []
  for (const [identity, animation] of selected) {
    const occurrence = artifacts.modelOccurrences.find(value => value.entity === identity)
    const state = snapshot.entityPresentation.studioModels.find(value => value.sourceIndex === identity)
    if (!occurrence || !state) throw new Error(`Map prop pipeline occurrence unavailable: ${identity}`)
    const key = `${occurrence.model}:${occurrence.skin}`
    if (variants.has(key)) continue
    variants.add(key)
    output.push({ pass: "world", request: {
      identity, model: occurrence.model, skin: occurrence.skin, activity: animation.activity,
      elapsedSeconds: animation.elapsedSeconds, previousElapsedSeconds: animation.elapsedSeconds,
      currentTimeSeconds: Number(snapshot.tick) * 0.015, frameTimeSeconds: 0, planarSpeed: 0,
      screenAspectRatio: aspect, worldFarPlane: camera.far, lod: 0, bodygroups: [], packedBody: animation.body ?? occurrence.body,
      lighting: { origin: state.worldPosition, angles: state.worldAngles,
        cameraPosition: camera.position, cameraAngles: [camera.pitchDegrees, camera.yawDegrees, 0] },
    } })
  }
  return output
}
