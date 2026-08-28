import * as THREE from "three/webgpu"

export function pipelinePreparationIdentity(mesh: THREE.Mesh): string {
  const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
  const shaders = materials.map(material => [
    material.customProgramCacheKey(), material.transparent, material.side, material.blending,
    material.depthWrite, material.depthTest, material.alphaTest, material.userData.sourcePreparationIdentity ?? "",
  ].join(":"))
  const attributes = Object.entries(mesh.geometry.attributes)
    .map(([name, attribute]) => `${name}:${attribute.itemSize}:${attribute.normalized}:${attribute.array.constructor.name}`)
    .sort()
  return `${mesh.type}|${mesh.layers.mask}|${shaders.join("|")}|${attributes.join("|")}`
}

export function prepareReachablePipelineVisibility(
  root: THREE.Object3D,
  excluded: readonly THREE.Mesh[] = [],
  excludedGroups: readonly THREE.Object3D[] = [],
  eligible: (mesh: THREE.Mesh) => boolean = () => true,
): Readonly<{ variants: number; restore(): void }> {
  const skipped = new Set(excluded)
  const skippedGroups = new Set(excludedGroups)
  const hiddenParents = new Set<THREE.Object3D>()
  const identities = new Set<string>()
  const changes: { object: THREE.Object3D; visible: boolean; frustumCulled: boolean; bundle?: boolean }[] = []

  root.traverse(object => {
    const bundle = object as THREE.Object3D & { isBundleGroup?: boolean }
    const previous = { object, visible: object.visible, frustumCulled: object.frustumCulled, ...(bundle.isBundleGroup === true ? { bundle: true } : {}) }
    const hidden = skippedGroups.has(object) || object.parent !== null && hiddenParents.has(object.parent)
    if (hidden) hiddenParents.add(object)
    if (bundle.isBundleGroup === true) bundle.isBundleGroup = false
    if (object instanceof THREE.Mesh) {
      const identity = hidden || skipped.has(object) || !eligible(object) ? null : pipelinePreparationIdentity(object)
      object.visible = identity !== null && !identities.has(identity)
      if (identity !== null) identities.add(identity)
      object.frustumCulled = false
    } else {
      object.visible = !hidden
    }
    changes.push(previous)
  })

  return Object.freeze({
    variants: identities.size,
    restore() {
      for (const { object, visible, frustumCulled, bundle } of changes) {
        object.visible = visible
        object.frustumCulled = frustumCulled
        if (bundle) (object as THREE.Object3D & { isBundleGroup: boolean }).isBundleGroup = true
      }
    },
  })
}
