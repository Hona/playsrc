import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { bindSourceModelMesh, createSourceModelSkeleton, updateSourceModelSkeleton } from "../src/source-model-skinning"

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
    expect(updateSourceModelSkeleton(skeleton, matrices)).toBe(128)
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
    expect(updateSourceModelSkeleton(first, new Float32Array(65 * 12))).toBe(128 * 64)
    expect(() => updateSourceModelSkeleton(first, new Float32Array(97 * 12))).toThrow("count differs")
    first.dispose()
    second.dispose()
  })
})
