import type { PresentationArtifacts } from "./artifacts"
import { tf2ClassPresentation } from "./class"
import type { Tf2Class } from "./codec"
import type { ModelPoseRequest } from "./presentation"
import type { Tf2SupportedItem } from "./equipment/types"

export function classPreviewBaseActivity(identity: Tf2Class): string {
  return identity === 5 ? "ACT_MP_STAND_SECONDARY" : identity === 8 || identity === 9 ? "ACT_MP_STAND_MELEE" : "ACT_MP_STAND_PRIMARY"
}

/** Pose metadata for the selected team's already-resident class resources.
 * These are presentation queries, never class commands, weapon inputs or ticks.
 * The worker supplies authored palette/eye data; no skeleton is inferred in JS. */
export function classPipelinePoseRequests(artifacts: PresentationArtifacts, skin: number | null, camera: {
  position: readonly [number, number, number]; yawDegrees: number; pitchDegrees: number; far: number
}, aspect: number, inventory: readonly Tf2SupportedItem[]): readonly Readonly<{ request: ModelPoseRequest; pass: "panel" | "view" | "world" }>[] {
  const output: { request: ModelPoseRequest; pass: "panel" | "view" | "world" }[] = []
  const common = {
    preparation: true,
    previousElapsedSeconds: 0, elapsedSeconds: 0, currentTimeSeconds: 0, frameTimeSeconds: 0, planarSpeed: 0,
    screenAspectRatio: aspect, worldFarPlane: camera.far, lod: 0,
    lighting: { origin: camera.position, angles: [0, 0, 0] as const,
      cameraPosition: camera.position, cameraAngles: [camera.pitchDegrees, camera.yawDegrees, 0] as const },
  }
  for (let identity = 1; identity <= 9; identity++) {
    const model = tf2ClassPresentation(identity as Tf2Class).model
    const artifact = artifacts.models.get(model)
    if (!artifact) throw new Error(`Class pipeline model unavailable: ${model}`)
    const activity = classPreviewBaseActivity(identity as Tf2Class)
    const equippedItems = inventory.filter(item => item.weapon === null && item.classSlots.some(slot => slot.class === identity)).map(item => item.item)
    if (!artifact.sequences.some(sequence => sequence.activity === activity)) throw new Error(`Class pipeline standing pose unavailable: ${model}:${activity}`)
    if (skin !== null) output.push({ pass: "panel", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin, classSelection: true,
      equippedItems,
      activity,
      bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
    for (const worldSkin of [0, 1]) output.push({ pass: "world", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin: worldSkin,
      equippedItems,
      activity, bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
  }
  for (const [model, artifact] of artifacts.models) {
    if (skin === null || artifact.profile !== "viewmodel") continue
    const sequence = artifact.sequences[0]
    if (!sequence) throw new Error(`Class pipeline sequence unavailable: ${model}`)
    output.push({ pass: "view", request: {
      ...common, identity: 0xfffc0000 + output.length, model, skin: skin < artifact.skinCount ? skin : 0,
      activity: sequence.label, bodygroups: artifact.bodygroupCounts.map(() => 0),
    } })
  }
  // Held world weapons are not class-preview children: e.g. the Medic preview
  // carries a different weapon from its bot's initial gun. Prepare each already
  // admitted weapon's authored palette for both resident team skins as well.
  for(const model of new Set(inventory.filter(item=>item.weapon!==null&&item.modelPlayer).map(item=>item.modelPlayer))){
    const artifact=artifacts.models.get(model)
    if(!artifact)continue // Incremental equipment admission prepares nonresident models.
    const sequence=artifact.sequences[0]
    if(!sequence)throw new Error(`World weapon pipeline sequence unavailable: ${model}`)
    for(let worldSkin=0;worldSkin<Math.min(2,artifact.skinCount);worldSkin++)output.push({pass:"world",request:{
      ...common,identity:0xfffc0000+output.length,model,skin:worldSkin,activity:sequence.label,bodygroups:artifact.bodygroupCounts.map(()=>0),
    }})
  }
  // Nine class queries each include their exact authored carried item.
  if (output.length + 9 + output.reduce((sum, entry) => sum + (entry.request.equippedItems?.length ?? 0), 0) > 128) throw new Error("Class pipeline resource bound exceeded")
  return Object.freeze(output.map(value => Object.freeze(value)))
}
