import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { createSourceModelLightingUniforms, sourceModelSurfaceNode, type SourceModelPhongState } from "../src/source-model-lighting"
import { installWebGpuBufferNames } from "../src/webgpu-buffer-names"

const phong: SourceModelPhongState = {
  maskSource: 0, invertMask: false, albedoTint: false, exponent: 5, exponentFactor: 0,
  tint: [1, 1, 1], boost: 1, packedFresnel: [5, 3, 4],
  rim: { exponent: 8, boost: 0.8, exponentTextureAlphaMask: false },
}

// Real Three WGSL generation without a device, a browser or a GPU submission.
function build(parameters: SourceModelPhongState, options: { exponentTexture?: boolean; warp?: boolean; eye?: boolean } = {}) {
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.hasFeature = () => false
  const material = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
  material.colorNode = sourceModelSurfaceNode(TSL.uniform(new THREE.Vector4(1, 1, 1, 0.5)), createSourceModelLightingUniforms(), {
    halfLambert: true, phong: parameters,
    ...(options.exponentTexture ? { exponentTexture: new THREE.Texture() } : {}),
    ...(options.warp ? { diffuseWarp: new THREE.Texture() } : {}),
    ...(options.eye ? { eye: { glossiness: 0.7, ambientOcclusionColor: [0.2, 0.3, 0.4] as const, ambientOcclusion: new THREE.Texture() } } : {}),
  }, TSL.float(1)).color
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(), material)
  const backend = { createNodeBuilder: () => new THREE.WGSLNodeBuilder(mesh, renderer) }
  installWebGpuBufferNames(backend)
  const builder = backend.createNodeBuilder()
  builder.scene = new THREE.Scene()
  builder.camera = new THREE.PerspectiveCamera()
  builder.build()
  return builder
}

test("authored Phong numbers reuse the exact program with independent uniforms", () => {
  const a = build(phong)
  const b = build({ ...phong, exponent: 80, boost: 0.3, packedFresnel: [8, 5, 10], rim: { ...phong.rim!, exponent: 4, boost: 1 } })
  expect(a.vertexShader).toBe(b.vertexShader)
  expect(a.fragmentShader).toBe(b.fragmentShader)
  const uniforms = (builder: THREE.WGSLNodeBuilder) => builder.uniforms.fragment.filter((u: any) => u.node.isUniformNode).map((u: any) => u.node)
  const first = uniforms(a), second = uniforms(b)
  expect(first).toHaveLength(second.length)
  // Three's camera/model references are intentionally shared. Material-owned
  // parameter uniforms must not be: the second graph cannot change the first.
  const exponentIndex = second.findIndex((node: any) => node.value === 80)
  expect(exponentIndex).toBeGreaterThanOrEqual(0)
  expect(first[exponentIndex]).not.toBe(second[exponentIndex])
  expect(first[exponentIndex].value).toBe(5)
  second[exponentIndex].value = 40
  expect(first[exponentIndex].value).toBe(5)
  second[exponentIndex].value = 80
  expect(first.map((node: any) => node.value)).toContain(5)
  expect(second.map((node: any) => node.value)).toContain(80)
})

test("structural material features still produce distinct shaders and bindings", () => {
  const variants = [
    build(phong),
    build({ ...phong, maskSource: 1 }),
    build({ ...phong, invertMask: true }),
    build({ ...phong, rim: null }),
    build({ ...phong, exponentFactor: 100 }, { exponentTexture: true }),
    build({ ...phong, exponent: -1 }, { exponentTexture: true }),
    build(phong, { warp: true }),
    build(phong, { eye: true }),
  ]
  expect(new Set(variants.map(builder => builder.fragmentShader)).size).toBe(variants.length)
  // An exponent texture is not sampled with an explicit exponent and no
  // texture-driven tint/rim mask; that is the same reachable program.
  expect(build(phong, { exponentTexture: true }).fragmentShader).toBe(variants[0]!.fragmentShader)
})
