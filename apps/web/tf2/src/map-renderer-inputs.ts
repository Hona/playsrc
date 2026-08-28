import type {PresentationArtifacts} from "@playsrc/game-tf2-browser/artifacts"

/** Initial loading, replacement and rollback admit the same authored owners. */
export function mapRendererInputs(artifacts:PresentationArtifacts){
  return {
    directionalTextures:artifacts.directionalTextures,
    environment:artifacts.environment,
    particleTextures:artifacts.particleTextures,
    legacyVisualTextures:artifacts.legacyVisualTextures,
    modelOccurrences:artifacts.modelOccurrences,
    modelDrawInputs:artifacts.modelOccurrences.map(occurrence=>Object.freeze({entity:occurrence.entity,lighting:occurrence.lighting,eyes:occurrence.eyes})),
    modelMaterials:artifacts.modelMaterials,
    authoredTextures:artifacts.authoredTextures,
    brushModels:artifacts.brushModels,
    staticProps:artifacts.staticProps,
  }
}
