import * as THREE from "three/webgpu"

export type MapModelTransform = Readonly<{
  sourceIndex: number
  worldPosition: readonly [number, number, number]
  worldAngles: readonly [number, number, number]
  draw: boolean
}>

export function applyMapModelRenderBounds(mesh: THREE.SkinnedMesh, bounds: readonly [readonly [number, number, number], readonly [number, number, number]]): void {
  const [minimum, maximum] = bounds
  if (![...minimum, ...maximum].every(Number.isFinite) || minimum.some((value, axis) => value > maximum[axis]!)) throw new Error("authored model render bounds are invalid")
  const sphere = mesh.boundingSphere ??= new THREE.Sphere()
  sphere.center.set((minimum[0] + maximum[0]) / 2, (minimum[1] + maximum[1]) / 2, (minimum[2] + maximum[2]) / 2)
  sphere.radius = Math.hypot(maximum[0] - minimum[0], maximum[1] - minimum[1], maximum[2] - minimum[2]) / 2
  mesh.frustumCulled = true
}

/** Keep the admitted occurrence and its GPU resources; only Entity owns motion. */
export function applyMapModelTransform(instance: THREE.Object3D, state: MapModelTransform): void {
  if (!instance.matrixAutoUpdate) {
    instance.matrix.decompose(instance.position, instance.quaternion, instance.scale)
    instance.matrixAutoUpdate = true
  }
  if (instance.userData.entityDrawState !== state) {
    instance.position.set(...state.worldPosition)
    const [pitch, yaw, roll] = state.worldAngles
    instance.rotation.set(THREE.MathUtils.degToRad(roll), THREE.MathUtils.degToRad(pitch), THREE.MathUtils.degToRad(yaw), "ZYX")
    // Map occurrences live below the retained static world root. That root
    // deliberately skips traversal on normal draws; dirtying a child cannot
    // propagate upward. Refresh only this changed occurrence before GPU node
    // uniforms/frustum tests consume matrixWorld, never the entire map.
    instance.updateMatrixWorld(true)
    instance.userData.entityDrawState = state
  }
  instance.visible = state.draw
}
