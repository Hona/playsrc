import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { createSourceModelLightingUniforms, updateSourceModelLightingUniforms, sourceModelSurfaceNode, sourceModelWorldNormal } from "../../src/source-model-lighting"
import { bindSourceModelMesh, createSourceModelSkeleton } from "../../src/source-model-skinning"

export async function createLightingProbe() {
  const renderer = new THREE.WebGPURenderer({ antialias: false })
  renderer.setSize(1120, 600)
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace
  renderer.toneMapping = THREE.NoToneMapping
  document.body.append(renderer.domElement)
  await renderer.init()
  const scene = new THREE.Scene()
  const camera = new THREE.OrthographicCamera(-5.6, 5.6, 3, -3, 0.1, 100)
  camera.position.set(0, 0, 10)
  const uniforms = createSourceModelLightingUniforms()
  const diffuse = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
  diffuse.colorNode = sourceModelSurfaceNode(TSL.vec4(1), uniforms, { halfLambert: false }, TSL.float(1)).color
  diffuse.toneMapped = false
  const normals = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
  normals.colorNode = sourceModelWorldNormal.mul(0.5).add(0.5)
  normals.toneMapped = false
  const observations: Array<{ name: string; x: number; y: number; normal: number[]; depth: number }> = []
  const geometries: THREE.BufferGeometry[] = [], skeletons: THREE.Skeleton[] = [], meshes: THREE.Mesh[] = []
  const normalize = (v: number[]) => v.map(component => component / Math.hypot(...v))
  for (const [row, pose] of ["bind", "bone", "weighted", "object", "mirrored", "nonuniform"].entries()) {
    for (const [column, normal] of [[0, 0, 1], [0, 0, -1], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], normalize([1, 1, 1])].entries()) {
      // Each box is an analytic vector probe: every vertex carries the same
      // authored normal, independent of the box's diagnostic footprint.
      const geometry = new THREE.BoxGeometry(0.65, 0.6, 0.6)
      const indices = geometry.index!.array
      for (let i = 0; i < indices.length; i += 3) [indices[i + 1], indices[i + 2]] = [indices[i + 2]!, indices[i + 1]!]
      const attribute = geometry.getAttribute("normal")
      for (let i = 0; i < attribute.count; i++) attribute.setXYZ(i, normal[0]!, normal[1]!, normal[2]!)
      let mesh: THREE.Mesh
      let expected = [...normal]
      if (pose === "bone" || pose === "weighted") {
        const weight = pose === "bone" ? 1 : 0.75
        const skeleton = createSourceModelSkeleton(Float32Array.from([
          0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0,
          1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0,
        ]))
        geometry.setAttribute("skinIndex", new THREE.BufferAttribute(Uint16Array.from({ length: attribute.count * 4 }, (_, i) => i % 4 === 1 ? 1 : 0), 4))
        geometry.setAttribute("skinWeight", new THREE.BufferAttribute(Float32Array.from({ length: attribute.count * 4 }, (_, i) => i % 4 === 0 ? weight : i % 4 === 1 ? 1 - weight : 0), 4))
        mesh = bindSourceModelMesh(geometry, diffuse, skeleton)
        expected = normalize([-normal[2]! * weight + normal[0]! * (1 - weight), normal[1]!, normal[0]! * weight + normal[2]! * (1 - weight)])
        skeletons.push(skeleton)
      } else mesh = new THREE.Mesh(geometry, diffuse)
      if (pose === "object") {
        mesh.rotation.y = -Math.PI / 2
        expected = [-normal[2]!, normal[1]!, normal[0]!]
      }
      if (pose === "mirrored") {
        mesh.scale.y = -1
        expected = [normal[0]!, -normal[1]!, normal[2]!]
      }
      if (pose === "nonuniform") {
        mesh.scale.set(1.5, 1, 0.5)
        expected = normalize([normal[0]! / 1.5, normal[1]!, normal[2]! / 0.5])
      }
      mesh.position.set((column - 3) * 1.5, 2.3 - row * 0.9, 0)
      scene.add(mesh)
      scene.updateMatrixWorld(true)
      const ray = new THREE.Raycaster(new THREE.Vector3(mesh.position.x, mesh.position.y, 10), new THREE.Vector3(0, 0, -1))
      const hit = ray.intersectObject(mesh)[0]
      if (!hit) throw new Error(`probe has no visible clockwise triangle: ${pose}:${column}`)
      observations.push({ name: `${pose}:${column}`, x: 560 + (column - 3) * 150, y: 300 - (2.3 - row * 0.9) * 100, normal: expected, depth: hit.distance })
      geometries.push(geometry); meshes.push(mesh)
    }
  }
  return {
    draw(mode: "above" | "below" | "front" | "ambient" | "normals") {
      const toward = mode === "below" ? [0, 0, -1] : mode === "front" ? [1, 0, 0] : [0, 0, 1]
      const cube = [0.1, 0.2, 0.3, 0.4, 0.8, 0.05]
      updateSourceModelLightingUniforms(uniforms, {
        lightingOrigin: [0, 0, 0], cameraPosition: [0, 0, 10],
        ambientCube: cube.map(value => [value, value, value]) as any, ambientLight: mode === "ambient",
        localLights: mode === "ambient" ? [] : [{ kind: "directional", direction: toward.map(value => -value) as any, color: [1, 1, 1], position: [0, 0, 10], attenuation: [1, 0, 0], range: 0, falloff: 0, theta: 0, phi: 0 }],
        localEnvironment: null, staticLightVertex: false, staticLightTexel: false,
      })
      for (const mesh of meshes) mesh.material = mode === "normals" ? normals : diffuse
      renderer.render(scene, camera)
      return observations.map(probe => {
        const n = probe.normal
        const intensity = mode === "ambient"
          ? n.reduce((sum, component, axis) => sum + component * component * cube[axis * 2 + Number(component < 0)]!, 0)
          : Math.max(0, n.reduce((sum, component, axis) => sum + component * toward[axis]!, 0))
        return { ...probe, expected: (mode === "normals" ? n.map(value => value * 0.5 + 0.5) : [intensity, intensity, intensity]).map(value => Math.round(value * 255)) }
      })
    },
    dispose() {
      for (const resource of [...geometries, ...skeletons, diffuse, normals]) resource.dispose()
      renderer.dispose()
    },
  }
}
