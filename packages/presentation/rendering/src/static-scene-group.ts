import * as THREE from "three/webgpu"

export class RetainedStaticSceneGroup extends THREE.Group {
  constructor() {
    super()
    this.matrixAutoUpdate = false
  }

  override updateMatrixWorld(force = false): void {
    if (force || this.matrixWorldNeedsUpdate) super.updateMatrixWorld(force)
  }
}
