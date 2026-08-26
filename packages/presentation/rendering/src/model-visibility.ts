import * as THREE from "three/webgpu"

export function modelIntersectsViewFrustum(
  frustum: THREE.Frustum,
  root: THREE.Group,
  transform: THREE.Matrix4,
  scratchSphere: THREE.Sphere,
  scratchMatrix: THREE.Matrix4,
): boolean {
  let visible = false
  root.traverse((object) => {
    if (visible || !(object instanceof THREE.Mesh)) return
    if (!object.frustumCulled) {
      visible = true
      return
    }
    if (object.geometry.boundingSphere === null) object.geometry.computeBoundingSphere()
    const bounds = object.geometry.boundingSphere
    if (!bounds) throw new Error("Authored StudioModel primitive has no visibility bounds")
    scratchMatrix.multiplyMatrices(transform, object.matrixWorld)
    scratchSphere.copy(bounds).applyMatrix4(scratchMatrix)
    if (frustum.intersectsSphere(scratchSphere)) visible = true
  })
  return visible
}
