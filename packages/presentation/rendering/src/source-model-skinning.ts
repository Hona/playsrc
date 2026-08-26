import * as THREE from "three/webgpu"
import { ownSharedUpload, type SharedUploadRange } from "./shared-upload"

const MAX_SOURCE_BONES = 256
const SOURCE_MATRIX_VALUES = 12
const AUTHORED_SKELETON_BONES = new WeakMap<THREE.Skeleton, number>()
const PUBLISH_PALETTE = new WeakMap<THREE.Skeleton, ReturnType<typeof ownSharedUpload>>()
const OFFSET_MATRIX = new THREE.Matrix4()

class SourceModelSkeleton extends THREE.Skeleton {
  override update(): void {
    // Source publishes finalized matrices with its pose, not with Three's
    // render-frame clock. A render/compute pass must never mutate a published
    // palette behind its revision (including multiple poses in one RAF).
  }
}

export function sourceModelBoneCount(skeleton: THREE.Skeleton): number {
  const count = AUTHORED_SKELETON_BONES.get(skeleton)
  if (count === undefined) throw new Error("authored model skeleton owner is unavailable")
  return count
}

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
  const skeleton = new SourceModelSkeleton(bones, Array.from({ length: capacity }, () => new THREE.Matrix4()))
  for (let bone = 0; bone < capacity; bone += 1) bones[bone]!.matrixWorld.toArray(skeleton.boneMatrices, bone * 16)
  AUTHORED_SKELETON_BONES.set(skeleton, count)
  PUBLISH_PALETTE.set(skeleton, ownSharedUpload(skeleton.boneMatrices))
  updateSourceModelSkeleton(skeleton, matrices)
  return skeleton
}

export function updateSourceModelSkeleton(skeleton: THREE.Skeleton, matrices: Float32Array): number {
  const count = AUTHORED_SKELETON_BONES.get(skeleton)
  if (count === undefined || count * SOURCE_MATRIX_VALUES !== matrices.length) {
    throw new Error("authored model bone count differs")
  }
  let changed = 0
  const ranges: SharedUploadRange[] = []
  let start = -1
  for (let bone = 0; bone < count; bone += 1) {
    const offset = bone * SOURCE_MATRIX_VALUES
    const matrix = skeleton.bones[bone]!.matrixWorld
    const elements = matrix.elements
    if (
      elements[0] === matrices[offset] && elements[4] === matrices[offset + 1]
      && elements[8] === matrices[offset + 2] && elements[12] === matrices[offset + 3]
      && elements[1] === matrices[offset + 4] && elements[5] === matrices[offset + 5]
      && elements[9] === matrices[offset + 6] && elements[13] === matrices[offset + 7]
      && elements[2] === matrices[offset + 8] && elements[6] === matrices[offset + 9]
      && elements[10] === matrices[offset + 10] && elements[14] === matrices[offset + 11]
    ) {
      if (start >= 0) {
        ranges.push({ start: start * 64, count: (bone - start) * 64 })
        start = -1
      }
      continue
    }
    if (start < 0) start = bone
    matrix.set(
      matrices[offset]!, matrices[offset + 1]!, matrices[offset + 2]!, matrices[offset + 3]!,
      matrices[offset + 4]!, matrices[offset + 5]!, matrices[offset + 6]!, matrices[offset + 7]!,
      matrices[offset + 8]!, matrices[offset + 9]!, matrices[offset + 10]!, matrices[offset + 11]!,
      0, 0, 0, 1,
    )
    OFFSET_MATRIX.multiplyMatrices(matrix, skeleton.boneInverses[bone]!).toArray(skeleton.boneMatrices, bone * 16)
    changed += 1
  }
  if (start >= 0) ranges.push({ start: start * 64, count: (count - start) * 64 })
  if (changed) PUBLISH_PALETTE.get(skeleton)?.(ranges)
  return changed * 16 * Float32Array.BYTES_PER_ELEMENT
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
