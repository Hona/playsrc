import * as THREE from "three/webgpu"

export function prepareWaterPipelineVisibility(root: THREE.Object3D, water: readonly THREE.Mesh[]): () => void {
  const selected = new Set(water)
  const changes: { mesh: THREE.Mesh; visible: boolean }[] = []
  root.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return
    const visible = selected.has(object)
    if (object.visible === visible) return
    changes.push({ mesh: object, visible: object.visible })
    object.visible = visible
  })
  return () => {
    for (const { mesh, visible } of changes) mesh.visible = visible
  }
}
