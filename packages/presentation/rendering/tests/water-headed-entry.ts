import * as THREE from "three/webgpu"
import { linearToSrgb } from "../src/color-output"
import {
  createSourceWaterMaterial,
  evaluateSourceWaterPixel,
  type SourceWaterGpuMaterial,
  type SourceWaterPixel,
  type SourceWaterShaderState,
} from "../src/source-water"

type Scenario = "refraction" | "reflection-left" | "reflection-right" | "underwater-blur" | "authored-frame-0" | "authored-frame-30"

function image(width: number, rgba: readonly number[]): THREE.DataTexture {
  const texture = new THREE.DataTexture(new Uint8Array(rgba), width, 1, THREE.RGBAFormat)
  texture.colorSpace = THREE.NoColorSpace
  texture.minFilter = THREE.NearestFilter
  texture.magFilter = THREE.NearestFilter
  texture.generateMipmaps = false
  texture.flipY = false
  texture.needsUpdate = true
  return texture
}

const canvas = document.querySelector("canvas")!
const renderer = new THREE.WebGPURenderer({ canvas, antialias: false, alpha: false })
await renderer.init()
if (!renderer.backend.isWebGPUBackend) throw new Error("Water evidence requires a visible WebGPU backend")
renderer.outputColorSpace = THREE.SRGBColorSpace
renderer.toneMapping = THREE.NoToneMapping
renderer.setSize(640, 480, false)
renderer.setClearColor(0x101820, 1)

const scene = new THREE.Scene()
const camera = new THREE.PerspectiveCamera(60, 640 / 480, 0.1, 100)
camera.coordinateSystem = renderer.coordinateSystem
camera.position.set(0, 0, 2)
camera.lookAt(0, 0, 0)
camera.updateProjectionMatrix()
camera.updateMatrixWorld()

const positions = new Float32Array([-0.8, -0.8, 0, 0.8, -0.8, 0, 0.8, 0.8, 0, -0.8, 0.8, 0])
const geometry = new THREE.BufferGeometry()
geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3))
geometry.setAttribute("normal", new THREE.Float32BufferAttribute([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1], 3))
geometry.setAttribute("sourceTangentS", new THREE.Float32BufferAttribute([1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0], 3))
geometry.setAttribute("sourceTangentT", new THREE.Float32BufferAttribute([0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0], 3))
geometry.setAttribute("uv", new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2))
geometry.setIndex([0, 1, 2, 0, 2, 3])

const wall = new THREE.Mesh(
  new THREE.PlaneGeometry(0.32, 0.7),
  new THREE.MeshBasicNodeMaterial({ color: 0x00cc00, depthTest: true, depthWrite: true }),
)
wall.position.set(-0.48, 0, 0.2)
scene.add(wall)

const normalCentered = image(1, [128, 128, 255, 255])
const normalLeft = image(1, [0, 128, 255, 255])
const normalRight = image(1, [255, 128, 255, 255])
const authoredSamples = ((window as any).__sourceWaterAuthoredFrames as readonly string[]).map((encoded) =>
  Uint8Array.from(atob(encoded), (value) => value.charCodeAt(0)),
)
const authoredNormals = authoredSamples.map((samples) => {
  if (samples.byteLength !== 256 * 256 * 3) throw new Error("Authored Water normal plane length is invalid")
  const rgba = new Uint8Array(256 * 256 * 4)
  for (let source = 0, target = 0; source < samples.byteLength; source += 3, target += 4) {
    rgba[target] = samples[source]!
    rgba[target + 1] = samples[source + 1]!
    rgba[target + 2] = samples[source + 2]!
    rgba[target + 3] = 255
  }
  const texture = new THREE.DataTexture(rgba, 256, 256, THREE.RGBAFormat)
  texture.colorSpace = THREE.NoColorSpace
  texture.minFilter = THREE.NearestFilter
  texture.magFilter = THREE.NearestFilter
  texture.generateMipmaps = false
  texture.flipY = false
  texture.needsUpdate = true
  return texture
})
const refraction = image(1, [64, 128, 192, 128])
const reflection = image(2, [255, 0, 0, 255, 0, 0, 255, 255])
const state = (overrides: Partial<SourceWaterShaderState> = {}): SourceWaterShaderState => Object.freeze({
  profile: "ldr",
  mode: "expensive",
  aboveWater: true,
  reflectAmount: 0.25,
  refractAmount: 0.32,
  reflectTint: [1, 1, 1],
  refractTint: [0.25, 0.5, 0.75],
  fogColor: [0, 0, 0],
  fogStart: 0,
  fogEnd: 100,
  blurRefraction: false,
  hasBaseTexture: false,
  cheapBlend: false,
  cheapStart: 500,
  cheapEnd: 1000,
  reflectionBlendFactor: 1,
  fresnelEnabled: true,
  linearLightScale: 1,
  environmentScale: 1,
  ...overrides,
})

const clip = new THREE.Vector4(0, 0, 0, 1)
  .applyMatrix4(camera.matrixWorldInverse)
  .applyMatrix4(camera.projectionMatrix)
const clipPosition = [clip.x, clip.y, clip.z, clip.w] as const
let water: THREE.Mesh | undefined
let activeResource: SourceWaterGpuMaterial | undefined
let previousScenario: Scenario | undefined

function pixels(color: SourceWaterPixel): readonly [number, number, number, number] {
  return [
    Math.round(linearToSrgb(color[0]) * 255),
    Math.round(linearToSrgb(color[1]) * 255),
    Math.round(linearToSrgb(color[2]) * 255),
    255,
  ]
}

function scenario(name: Scenario) {
  const authored = name === "authored-frame-0" || name === "authored-frame-30"
  const frameIndex = name === "authored-frame-30" ? 1 : 0
  const retainedAnimation = name === "authored-frame-30" && previousScenario === "authored-frame-0" && activeResource && water
  if (water && !retainedAnimation) {
    scene.remove(water)
    ;(water.material as THREE.Material).dispose()
  }

  const normalTexture = authored
    ? authoredNormals[frameIndex]!
    : name === "reflection-left"
      ? normalLeft
      : name === "reflection-right"
        ? normalRight
        : normalCentered
  const below = name !== "refraction"
  const selected = state({
    aboveWater: !below,
    reflectAmount: authored ? 8 : 0.25,
    blurRefraction: name === "underwater-blur",
    fogStart: name === "underwater-blur" ? clipPosition[2] : 0,
    fogEnd: name === "underwater-blur" ? clipPosition[2] + 100 : 100,
  })
  const reflectionTexture = name.startsWith("reflection") || authored ? reflection : null
  const refractionTexture = name === "refraction" || name === "underwater-blur" ? refraction : null
  let resource: SourceWaterGpuMaterial
  if (retainedAnimation) {
    resource = activeResource!
    resource.normalNode.value = normalTexture
  } else {
    resource = createSourceWaterMaterial({
      geometry,
      state: selected,
      normal: normalTexture,
      reflection: reflectionTexture,
      refraction: refractionTexture,
      cubemap: null,
      refractionDepthEncoding: refractionTexture && !below ? "source-water-fog-alpha" : null,
    })
    water = new THREE.Mesh(geometry, resource.material)
    scene.add(water)
    activeResource = resource
  }
  renderer.render(scene, camera)

  const center = (128 * 256 + 128) * 3
  const normalColor = authored
    ? [
        authoredSamples[frameIndex]![center]! / 255,
        authoredSamples[frameIndex]![center + 1]! / 255,
        authoredSamples[frameIndex]![center + 2]! / 255,
        1,
      ]
    : name === "reflection-left"
      ? [0, 128 / 255, 1, 1]
      : name === "reflection-right"
        ? [1, 128 / 255, 1, 1]
        : [128 / 255, 128 / 255, 1, 1]
  const reference = evaluateSourceWaterPixel({
    state: selected,
    clipPosition,
    normalSample: normalColor as SourceWaterPixel,
    tangentEyeVector: [0, 0, 2],
    reflection: reflectionTexture
      ? { sample: (coordinate) => coordinate[0] < 0.5 ? [1, 0, 0, 1] : [0, 0, 1, 1] }
      : null,
    refraction: refractionTexture ? { sample: () => [64 / 255, 128 / 255, 192 / 255, 128 / 255] } : null,
  })
  const wallProjected = new THREE.Vector3(-0.48, 0, 0.2).project(camera)
  previousScenario = name
  return Object.freeze({
    scenario: name,
    expected: pixels(reference.rgba),
    water: Object.freeze({ x: 320, y: 240 }),
    wall: Object.freeze({
      x: Math.round((wallProjected.x * 0.5 + 0.5) * 640),
      y: Math.round((-wallProjected.y * 0.5 + 0.5) * 480),
    }),
    reference,
    retainedMaterialAnimation: Boolean(retainedAnimation),
    depthWrite: resource.material.depthWrite,
    transparent: resource.material.transparent,
  })
}

Object.assign(window, {
  __sourceWaterEvidenceReady: true,
  __sourceWaterEvidenceScenario: scenario,
})
