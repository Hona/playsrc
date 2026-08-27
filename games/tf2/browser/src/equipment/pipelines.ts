import type { EquipmentModelArtifacts } from "../artifacts"
import type { ModelPoseRequest } from "../presentation"

/** One palette per admitted model/skin, shared by all required material passes. */
export function equipmentPipelinePoseRequests(artifacts: Pick<EquipmentModelArtifacts, "models" | "geometry">, aspect: number): readonly ModelPoseRequest[] {
  const requests: ModelPoseRequest[] = []
  for (const geometry of artifacts.geometry) {
    const [model, encodedSkin] = geometry.logicalPath.split("#skin=")
    const skin = encodedSkin === undefined ? 0 : Number(encodedSkin)
    const artifact = artifacts.models.get(model!)
    if (!artifact || !Number.isSafeInteger(skin) || skin < 0 || skin >= artifact.skinCount) throw new Error(`Equipment pipeline model unavailable: ${geometry.logicalPath}`)
    const sequence = artifact.sequences[0]
    if (!sequence) throw new Error(`Equipment pipeline sequence unavailable: ${model}`)
    requests.push({
      identity: 0xfffa0000 + requests.length, model: model!, skin, activity: sequence.label,
      preparation: true, modelPanel: true, previousElapsedSeconds: 0, elapsedSeconds: 0, currentTimeSeconds: 0, frameTimeSeconds: 0,
      planarSpeed: 0, screenAspectRatio: aspect, worldFarPlane: 16384 * Math.sqrt(3), lod: 0,
      bodygroups: artifact.bodygroupCounts.map(() => 0),
      lighting: { origin: [0, 0, 0], angles: [0, 0, 0], cameraPosition: [0, 0, 0], cameraAngles: [0, 0, 0] },
    })
  }
  return requests
}
