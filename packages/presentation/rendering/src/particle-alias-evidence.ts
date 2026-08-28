import * as THREE from "three/webgpu"
import { createParticleQuadWriter } from "./particle-geometry"
import { createParticleAttributeUpdates, writeParticleAppearance } from "./particle-attributes"
import { installSkinningEvidence } from "./skinning-evidence"

// Explicit local headed diagnostic only. Registration retains weak references,
// not old scene resources. No production import or alternate rendering quality.
export function installParticleAliasEvidence() {
  const entries = new Map<string, { material: WeakRef<THREE.Material>; image: WeakRef<THREE.Texture>; geometry: WeakRef<THREE.BufferGeometry>; current: WeakRef<any>; next: WeakRef<any>; depth: WeakRef<any> }>()
  let owner: THREE.WebGPURenderer | undefined, grid: THREE.Scene | undefined, camera: THREE.PerspectiveCamera | undefined
  let display = false, inside = false
  const clones: THREE.Texture[] = [], meshes: THREE.Mesh[] = [], swaps: { current: any; next: any; image: THREE.Texture; separate: THREE.Texture }[] = []
  const materialNames: string[] = []
  const occluders: THREE.Mesh<THREE.BoxGeometry, THREE.MeshBasicNodeMaterial>[] = []
  const pixels = installSkinningEvidence(draw => {
    for (const swap of swaps) { swap.current.value = swap.separate; swap.next.value = swap.separate }
    try { draw() } finally { for (const swap of swaps) { swap.current.value = swap.image; swap.next.value = swap.image } }
  }, scene => scene === grid, true)
  const render = THREE.WebGPURenderer.prototype.render
  THREE.WebGPURenderer.prototype.render = function (scene, view) {
    const value = render.call(this, scene, view)
    if (inside || scene === grid || !view.layers.isEnabled(0)) return value
    let world = false
    scene.traverseVisible(object => { if ((object as any).isBundleGroup && object.children.length) world = true })
    if (!world) return value
    owner = this
    if (display && grid && camera) {
      inside = true
      const autoClear = this.autoClear
      this.autoClear = true
      try { render.call(this, grid, camera) } finally { inside = false; this.autoClear = autoClear }
    }
    return value
  }
  const prepare = () => {
    if (grid) return
    if (!owner) throw new Error("No authored gameplay renderer observed")
    const groups = new Map<object, any[]>()
    for (const [name, reference] of entries) {
      const image = reference.image.deref(), material = reference.material.deref(), geometry = reference.geometry.deref()
      const current = reference.current.deref(), next = reference.next.deref(), depth = reference.depth.deref()
      if (!image || !material || !geometry || !current || !next || !depth) continue
      const data = (image.mipmaps[0] as any)?.data
      if (!data || !(image as THREE.CompressedTexture).isCompressedTexture) continue
      const values = groups.get(data) ?? []; values.push({ name, image, material, geometry, current, next, depth }); groups.set(data, values)
    }
    const aliases = [...groups.values()].filter(values => values.length > 1)
    if (aliases.length !== 6 || aliases.reduce((count, values) => count + values.length - 1, 0) !== 8) throw new Error("Authored alias material closure differs")
    grid = new THREE.Scene(); grid.background = new THREE.Color(0x101820)
    const size = owner.getDrawingBufferSize(new THREE.Vector2())
    camera = new THREE.PerspectiveCamera(35, size.x / size.y, 1, 128); camera.up.set(0, 0, 1); camera.position.set(0, -20, 0); camera.lookAt(0, 0, 0)
    for (const values of aliases) for (const [index, entry] of values.entries()) {
      const geometry = entry.geometry.clone(), mesh = new THREE.Mesh(geometry, entry.material)
      mesh.frustumCulled = false; mesh.renderOrder = meshes.length
      mesh.onBeforeRender = (renderer, _scene, view) => {
        if ((entry.material as any).userData.sourceParticleDepth) entry.depth.capture(renderer, view, false)
      }
      meshes.push(mesh); materialNames.push(entry.name); grid.add(mesh)
      const occluder = new THREE.Mesh(new THREE.BoxGeometry(0.5, 1, 2.8), new THREE.MeshBasicNodeMaterial({ color: 0x304050 }))
      occluder.position.set((meshes.length - 1) % 7 * 2.8 - 8.4, -0.2, meshes.length <= 7 ? 2 : -2)
      occluders.push(occluder); grid.add(occluder)
      if (index > 0) {
        const separate = entry.image.clone(); separate.needsUpdate = true; clones.push(separate)
        swaps.push({ current: entry.current, next: entry.next, image: entry.image, separate })
      }
    }
  }
  const setPhase = (phase: number) => {
    const writer = createParticleQuadWriter({ position: [0, -20, 0], yawDegrees: 90, pitchDegrees: 0 })
    meshes.forEach((mesh, index) => {
      const center: [number, number, number] = [(index % 7 - 3) * 2.8, 0, index < 7 ? 2 : -2]
      const geometry = mesh.geometry, positions = geometry.getAttribute("position") as THREE.BufferAttribute
      writer({ primitive: "sprite", position: center, trailEndPosition: center, trailWidth: 0, radius: 1.2, rollRadians: 0, orientationType: 0 }, positions.array as Float32Array, 0)
      const orientation = geometry.getAttribute("particleCenterOrientation") as THREE.BufferAttribute
      for (let vertex = 0; vertex < 4; vertex++) orientation.setXYZW(vertex, ...center, 0)
      writeParticleAppearance({ color: 0xffffff, opacity: 0.85, primarySheet: { current: [[0,0,1,1]], next: [[1,0,0,1]], blend: phase ? (index % 2 ? 0.65 : 0.25) : 0 } },
        { uv: geometry.getAttribute("uv").array as Float32Array, uvNext: geometry.getAttribute("particleUvNext").array as Float32Array,
          sheetBlend: geometry.getAttribute("particleSheetBlend").array as Float32Array, colors: geometry.getAttribute("particleColor").array as Float32Array }, 0, createParticleAttributeUpdates())
      for (const attribute of Object.values(geometry.attributes)) (attribute as THREE.BufferAttribute).needsUpdate = true
      geometry.computeVertexNormals(); geometry.computeBoundingSphere()
    })
  }
  return {
    register(name: string, material: THREE.Material, image: THREE.Texture, geometry: THREE.BufferGeometry, current: any, next: any, depth: any) {
      entries.set(name, { material: new WeakRef(material), image: new WeakRef(image), geometry: new WeakRef(geometry), current: new WeakRef(current), next: new WeakRef(next), depth: new WeakRef(depth) })
    },
    async capture(phase: number) {
      prepare(); setPhase(phase); display = true
      const result = await pixels.capture(`particle-alias-${phase}`, "*", true)
      const backend = owner!.backend as any
      const separateBackings = swaps.map(swap => ({ separate: backend.get(swap.separate).texture !== backend.get(swap.image).texture,
        initialized: !!backend.get(swap.separate).texture && !!backend.get(swap.image).texture }))
      if (separateBackings.some(value => !value.separate || !value.initialized)) throw new Error("Reference did not bind independent live GPU images")
      return { performanceSample: false, fixture: "actual authored alias materials; Source quad/appearance writers with distinct legal UV/blend inputs", phase,
        materials: materialNames, groups: 6, separateReferenceImages: clones.length, separateBackings, occluders: occluders.length, result }
    },
    hide() { display = false },
    dispose() {
      display = false; THREE.WebGPURenderer.prototype.render = render; pixels.dispose()
      for (const mesh of meshes) mesh.geometry.dispose()
      for (const mesh of occluders) { mesh.geometry.dispose(); mesh.material.dispose() }
      for (const texture of clones) texture.dispose()
      entries.clear(); owner = undefined; grid = undefined; camera = undefined
    },
  }
}
