import type { PresentationArtifacts } from "./artifacts"
import { tf2ClassPresentation } from "./class"
import type { Tf2Class } from "./codec"
import type { ModelPoseRequest } from "./presentation"

export function classPreviewBaseActivity(identity: Tf2Class): string {
  return identity === 5 ? "ACT_MP_STAND_SECONDARY" : identity === 8 || identity === 9 ? "ACT_MP_STAND_MELEE" : "ACT_MP_STAND_PRIMARY"
}

/** Pose metadata for the selected team's already-resident class resources.
 * These are presentation queries, never class commands, weapon inputs or ticks.
 * The worker supplies authored palette/eye data; no skeleton is inferred in JS. */
export function classPipelinePoseRequests(artifacts: PresentationArtifacts, skin: number, camera: {
  position: readonly [number, number, number]; yawDegrees: number; pitchDegrees: number; far: number
}, aspect: number): readonly Readonly<{ request: ModelPoseRequest; pass: "panel" | "view" | "world" }>[] {
  const output: { request: ModelPoseRequest; pass: "panel" | "view" | "world" }[] = []
  const common = {
    previousElapsedSeconds: 0, elapsedSeconds: 0, currentTimeSeconds: 0, frameTimeSeconds: 0, planarSpeed: 0,
    screenAspectRatio: aspect, worldFarPlane: camera.far, lod: 0,
    lighting: { origin: camera.position, angles: [0, 0, 0] as const,
      cameraPosition: camera.position, cameraAngles: [camera.pitchDegrees, camera.yawDegrees, 0] as const },
  }
  for (let identity = 1; identity <= 9; identity++) {
    const model = tf2ClassPresentation(identity as Tf2Class).model
    const artifact = artifacts.models.get(model)
    if (!artifact) throw new Error(`Class pipeline model unavailable: ${model}`)
    output.push({ pass: "panel", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin, classSelection: true,
      activity: classPreviewBaseActivity(identity as Tf2Class),
      bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
    for (const worldSkin of [0, 1]) output.push({ pass: "world", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin: worldSkin,
      activity: "ACT_MP_STAND_PRIMARY", bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
  }
  for (const [model, artifact] of artifacts.models) {
    if (artifact.profile !== "viewmodel") continue
    const sequence = artifact.sequences[0]
    if (!sequence) throw new Error(`Class pipeline sequence unavailable: ${model}`)
    output.push({ pass: "view", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin: skin < artifact.skinCount ? skin : 0,
      activity: sequence.label, bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
  }
  // Nine class queries each include their exact authored carried item.
  if (output.length + 9 > 96) throw new Error("Class pipeline resource bound exceeded")
  return Object.freeze(output.map(value => Object.freeze(value)))
}
