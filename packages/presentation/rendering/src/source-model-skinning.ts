import * as THREE from "three/webgpu"

const MAX_SOURCE_BONES = 256
const SOURCE_MATRIX_VALUES = 12
const AUTHORED_SKELETON_BONES = new WeakMap<THREE.Skeleton, number>()

export function createSourceModelSkeleton(matrices: Float32Array): THREE.Skeleton {
  if (matrices.length === 0 || matrices.length % SOURCE_MATRIX_VALUES !== 0 || matrices.length > MAX_SOURCE_BONES * SOURCE_MATRIX_VALUES) {
    throw new Error("authored model bone matrices are invalid")
  }
  const count = matrices.length / SOURCE_MATRIX_VALUES
  const capacity = 2 ** Math.ceil(Math.log2(count))
  const bones = Array.from({ length: capacity }, () => {
    const bone = new THREE.Bone()
    bone.matrixAutoUpdate = false
    bone.matrixWorldAutoUpdate = false
    return bone
  })
  const skeleton = new THREE.Skeleton(bones, Array.from({ length: capacity }, () => new THREE.Matrix4()))
  AUTHORED_SKELETON_BONES.set(skeleton, count)
  updateSourceModelSkeleton(skeleton, matrices)
  return skeleton
}

export function updateSourceModelSkeleton(skeleton: THREE.Skeleton, matrices: Float32Array): number {
  const count = AUTHORED_SKELETON_BONES.get(skeleton)
  if (count === undefined || count * SOURCE_MATRIX_VALUES !== matrices.length) {
    throw new Error("authored model bone count differs")
  }
  for (let bone = 0; bone < count; bone += 1) {
    const offset = bone * SOURCE_MATRIX_VALUES
    skeleton.bones[bone]!.matrixWorld.set(
      matrices[offset]!, matrices[offset + 1]!, matrices[offset + 2]!, matrices[offset + 3]!,
      matrices[offset + 4]!, matrices[offset + 5]!, matrices[offset + 6]!, matrices[offset + 7]!,
      matrices[offset + 8]!, matrices[offset + 9]!, matrices[offset + 10]!, matrices[offset + 11]!,
      0, 0, 0, 1,
    )
  }
  return skeleton.bones.length * 16 * Float32Array.BYTES_PER_ELEMENT
}

export function bindSourceModelMesh(
  geometry: THREE.BufferGeometry,
  material: THREE.Material | THREE.Material[],
  skeleton: THREE.Skeleton,
): THREE.SkinnedMesh {
  const mesh = new THREE.SkinnedMesh(geometry, material)
  mesh.bindMode = THREE.DetachedBindMode
  mesh.bind(skeleton, new THREE.Matrix4())
  mesh.frustumCulled = false
  return mesh
}
