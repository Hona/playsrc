import * as THREE from "three/webgpu"

// On-demand profiling only. Raycaster's interpolated normal is unskinned and
// face-forwarded, so it cannot diagnose authored Studio lighting directions.
export function modelNormalEvidence(hit: THREE.Intersection): Readonly<{
  worldPosition: readonly number[]
  worldNormal: readonly number[]
}> {
  const mesh = hit.object
  if (!(mesh instanceof THREE.Mesh) || !hit.face) throw new Error("model normal evidence has no triangle")
  const vertices = [hit.face.a, hit.face.b, hit.face.c]
  const points = vertices.map(index => mesh.getVertexPosition(index, new THREE.Vector3()))
  const barycentric = THREE.Triangle.getBarycoord(mesh.worldToLocal(hit.point.clone()), points[0]!, points[1]!, points[2]!, new THREE.Vector3())
  if (!barycentric) throw new Error("model normal evidence triangle is degenerate")
  const normal = new THREE.Vector3()
  for (const [corner, index] of vertices.entries()) {
    const value = new THREE.Vector3().fromBufferAttribute(mesh.geometry.getAttribute("normal"), index)
    if (mesh instanceof THREE.SkinnedMesh) {
      const indices = new THREE.Vector4().fromBufferAttribute(mesh.geometry.getAttribute("skinIndex"), index)
      const weights = new THREE.Vector4().fromBufferAttribute(mesh.geometry.getAttribute("skinWeight"), index)
      const blend = new THREE.Matrix4()
      blend.elements.fill(0)
      for (let influence = 0; influence < 4; influence++) {
        const weight = weights.getComponent(influence)
        if (weight === 0) continue
        const bone = indices.getComponent(influence)
        const matrix = new THREE.Matrix4().multiplyMatrices(mesh.skeleton.bones[bone]!.matrixWorld, mesh.skeleton.boneInverses[bone]!)
        for (let element = 0; element < 16; element++) blend.elements[element]! += matrix.elements[element]! * weight
      }
      blend.premultiply(mesh.bindMatrixInverse).multiply(mesh.bindMatrix)
      value.transformDirection(blend)
    }
    normal.addScaledVector(value.applyMatrix3(new THREE.Matrix3().getNormalMatrix(mesh.matrixWorld)), barycentric.getComponent(corner))
  }
  return Object.freeze({ worldPosition: Object.freeze(hit.point.toArray()), worldNormal: Object.freeze(normal.normalize().toArray()) })
}
