import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { bindSourceModelMesh, createSourceModelSkeleton, sourceModelBoneCount, updateSourceModelSkeleton } from "../src/source-model-skinning"
import { sharedUpload } from "../src/shared-upload"

describe("authored Source GPU bone skinning", () => {
  const matrices = Float32Array.from([
    1, 0, 0, 10, 0, 1, 0, 20, 0, 0, 1, 30,
    0, -1, 0, 4, 1, 0, 0, 5, 0, 0, 1, 6,
  ])

  test("preserves Source row-major 3×4 transforms and weighted bind positions", () => {
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute("position", new THREE.BufferAttribute(Float32Array.from([2, 3, 4]), 3))
    geometry.setAttribute("normal", new THREE.BufferAttribute(Float32Array.from([0, 1, 0]), 3))
    geometry.setAttribute("tangent", new THREE.BufferAttribute(Float32Array.from([1, 0, 0, -1]), 4))
    geometry.setAttribute("skinIndex", new THREE.BufferAttribute(Uint16Array.from([0, 1, 0, 0]), 4))
    geometry.setAttribute("skinWeight", new THREE.BufferAttribute(Float32Array.from([0.25, 0.75, 0, 0]), 4))
    const skeleton = createSourceModelSkeleton(matrices)
    const mesh = bindSourceModelMesh(geometry, new THREE.MeshBasicNodeMaterial(), skeleton)
    const posed = mesh.applyBoneTransform(0, new THREE.Vector3(2, 3, 4))
    expect(posed.x).toBeCloseTo(3.75, 6)
    expect(posed.y).toBeCloseTo(11, 6)
    expect(posed.z).toBeCloseTo(16, 6)
    expect(mesh.bindMode).toBe(THREE.DetachedBindMode)

    const parent = new THREE.Group()
    parent.position.set(100, 200, 300)
    parent.add(mesh)
    parent.updateMatrixWorld(true)
    expect(mesh.bindMatrixInverse.elements).toEqual(new THREE.Matrix4().elements)
    expect(updateSourceModelSkeleton(skeleton, matrices)).toBe(0)
    const changed = matrices.slice()
    changed[3] = 11
    expect(updateSourceModelSkeleton(skeleton, changed)).toBe(64)
    expect(sharedUpload(skeleton.boneMatrices)?.ranges).toEqual([{ start: 0, count: 64 }])
    expect(skeleton.bones[0]!.matrixWorld.elements[12]).toBe(11)
    expect(updateSourceModelSkeleton(skeleton, changed)).toBe(0)
    skeleton.dispose()
    geometry.dispose()
  })

  test("rejects malformed, empty, oversized, and changed bone palettes", () => {
    expect(() => createSourceModelSkeleton(new Float32Array())).toThrow("invalid")
    expect(() => createSourceModelSkeleton(new Float32Array(11))).toThrow("invalid")
    expect(() => createSourceModelSkeleton(new Float32Array(257 * 12))).toThrow("invalid")
    const skeleton = createSourceModelSkeleton(matrices)
    expect(() => updateSourceModelSkeleton(skeleton, matrices.subarray(0, 12))).toThrow("count differs")
    skeleton.dispose()
  })

  test("shares one bounded GPU shader palette shape across unequal authored bone counts", () => {
    const first = createSourceModelSkeleton(new Float32Array(65 * 12))
    const second = createSourceModelSkeleton(new Float32Array(97 * 12))
    expect(first.bones).toHaveLength(128)
    expect(second.bones).toHaveLength(128)
    expect(sourceModelBoneCount(first)).toBe(65)
    expect(sourceModelBoneCount(second)).toBe(97)
    expect(() => sourceModelBoneCount(new THREE.Skeleton())).toThrow("owner is unavailable")
    expect(updateSourceModelSkeleton(first, new Float32Array(65 * 12))).toBe(0)
    const changed = new Float32Array(65 * 12)
    changed[64 * 12] = 1
    expect(updateSourceModelSkeleton(first, changed)).toBe(64)
    expect(sharedUpload(first.boneMatrices)?.ranges).toEqual([{ start: 64 * 64, count: 64 }])
    expect(() => updateSourceModelSkeleton(first, new Float32Array(97 * 12))).toThrow("count differs")
    first.dispose()
    second.dispose()
  })

  test("changes every authored matrix without losing exact row-major transforms", () => {
    const skeleton = createSourceModelSkeleton(matrices)
    const changed = matrices.slice()
    changed[0] = -2
    changed[23] = 19
    expect(updateSourceModelSkeleton(skeleton, changed)).toBe(128)
    expect(sharedUpload(skeleton.boneMatrices)?.ranges).toEqual([{ start: 0, count: 128 }])
    expect(skeleton.bones[0]!.matrixWorld.elements[0]).toBe(-2)
    expect(skeleton.bones[1]!.matrixWorld.elements[14]).toBe(19)
    skeleton.dispose()
  })

  test("publishes consecutive Source poses before any render and never retimes them during render/compute passes", () => {
    const skeleton = createSourceModelSkeleton(matrices)
    const first = skeleton.boneMatrices.slice()
    expect(first[12]).toBe(10)
    const changed = matrices.slice()
    changed[3] = 37
    updateSourceModelSkeleton(skeleton, changed)
    expect(skeleton.boneMatrices[12]).toBe(37)
    changed[3] = -12
    updateSourceModelSkeleton(skeleton, changed)
    expect(skeleton.boneMatrices[12]).toBe(-12)
    const revision = sharedUpload(skeleton.boneMatrices)!.revision
    const published = skeleton.boneMatrices.slice()
    for (let pass = 0; pass < 4; pass += 1) skeleton.update()
    expect(skeleton.boneMatrices).toEqual(published)
    expect(sharedUpload(skeleton.boneMatrices)!.revision).toBe(revision)
    const reference = new THREE.Skeleton(skeleton.bones, skeleton.boneInverses)
    reference.update()
    expect(skeleton.boneMatrices).toEqual(reference.boneMatrices)
    reference.dispose()
    skeleton.dispose()
  })
})
