import * as THREE from "three/webgpu"

/** Retire only occurrence-owned resources, once even when a cloak overlay and
 * base mesh share geometry/palette. Templates remain generation-owned. */
export function disposeDynamicModel(instance: THREE.Object3D, materialDisposed?: () => void): void {
  instance.removeFromParent()
  const skeletons = new Set<THREE.Skeleton>()
  const materials = new Set<THREE.Material>()
  const geometries = new Set<THREE.BufferGeometry>()
  instance.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return
    if (object instanceof THREE.SkinnedMesh) skeletons.add(object.skeleton)
    if (object.userData.dynamicMaterial === true) {
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) materials.add(material)
    }
    if (object.userData.dynamicGeometry === true) geometries.add(object.geometry)
  })
  for (const material of materials) { materialDisposed?.(); material.dispose() }
  for (const geometry of geometries) geometry.dispose()
  for (const skeleton of skeletons) skeleton.dispose()
}
