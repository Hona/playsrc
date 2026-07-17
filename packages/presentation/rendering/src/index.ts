import * as THREE from "three/webgpu"
import { parseRuntimeMap, type RuntimeMap } from "./runtime-map"
import { configureWorldLightmap, worldMaterialSide } from "./material-state"

const MAX_EFFECTS = 4096
const MAX_DIMENSION = 8192
const HASH = /^[0-9a-f]{64}$/

export type Camera = Readonly<{
  position: readonly [number, number, number]
  yawDegrees: number
  pitchDegrees: number
  verticalFovDegrees: number
  near: number
  far: number
}>

export type Effect = Readonly<{
  identity: number
  position: readonly [number, number, number]
  radius: number
  color: number
  opacity: number
}>
export type ModelItem = Readonly<{
  identity: number
  model: string
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
  scale: number
  viewModel?: boolean
}>

export type Frame = Readonly<{
  camera: Camera
  effects: readonly Effect[]
  models?: readonly ModelItem[]
}>

export type SceneResult = Readonly<{
  payloadSha256: string
  drawableSurfaces: number
  drawBatches: number
  diagnostics: readonly Readonly<{ code: "MissingMaterial"; identity: string }>[]
}>

export class RenderingError extends Error {
  constructor(
    readonly code: "MalformedInput" | "MissingInput" | "UnsupportedEnvironment" | "BoundExceeded" | "InvalidState",
    message: string,
  ) {
    super(message)
    this.name = "RenderingError"
  }
}

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

function debugColor(identity: string): number {
  let hash = 0x811c9dc5
  for (const byte of new TextEncoder().encode(identity)) {
    hash ^= byte
    hash = Math.imul(hash, 0x01000193)
  }
  return ((hash >>> 8) & 0x7f7f7f) | 0x404040
}

async function digest(bytes: Uint8Array): Promise<string> {
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")
}

export async function createRenderer(canvas: HTMLCanvasElement): Promise<Readonly<{
  loadMap(payload: Uint8Array, payloadSha256: string, debugMissingMaterials: boolean): Promise<SceneResult>
  render(frame: Frame): Promise<void>
  resize(cssWidth: number, cssHeight: number, devicePixelRatio: number): void
  dispose(): void
}>> {
  if (!globalThis.navigator?.gpu || !(canvas instanceof HTMLCanvasElement)) {
    throw new RenderingError("UnsupportedEnvironment", "WebGPU canvas is unavailable")
  }
  const renderer = new THREE.WebGPURenderer({ canvas, antialias: true, alpha: false })
  await renderer.init()
  if (!(renderer.backend as { isWebGPUBackend?: boolean }).isWebGPUBackend) {
    renderer.dispose()
    throw new RenderingError("UnsupportedEnvironment", "Three.js selected a non-WebGPU backend")
  }
  renderer.outputColorSpace = THREE.SRGBColorSpace
  const scene = new THREE.Scene()
  scene.background = new THREE.Color(0x111820)
  const world = new THREE.Group()
  const effects = new THREE.Group()
  const viewModels = new THREE.Group()
  const camera = new THREE.PerspectiveCamera(75, 1, 1, 32_768)
  scene.add(world, effects, camera)
  camera.add(viewModels)
  camera.up.set(0, 0, 1)
  const effectGeometry = new THREE.SphereGeometry(1, 10, 6)
  let map: RuntimeMap | undefined
  let disposed = false
  let modelTemplates = new Map<string, THREE.Group>()
  let modelResources: Array<THREE.BufferGeometry | THREE.Material | THREE.Texture> = []
  let worldLightmap: THREE.DataTexture | undefined

  function clear(group: THREE.Group, disposeGeometry = true): void {
    for (const child of [...group.children]) {
      group.remove(child)
      if (child instanceof THREE.Mesh) {
        if (disposeGeometry) child.geometry.dispose()
        if (child.userData.texture instanceof THREE.Texture) child.userData.texture.dispose()
        const materials = Array.isArray(child.material) ? child.material : [child.material]
        for (const material of materials) material.dispose()
      }
    }
  }

  function disposeModels(): void {
    for (const resource of modelResources) resource.dispose()
    modelResources = []
    modelTemplates = new Map()
  }

  function materialFor(
    resolved: RuntimeMap["materials"][number],
    identity: string,
    lightMap?: THREE.DataTexture,
  ) {
    const texture = resolved.baseTexture
      ? new THREE.DataTexture(
          resolved.baseTexture.rgba,
          resolved.baseTexture.width,
          resolved.baseTexture.height,
          THREE.RGBAFormat,
          THREE.UnsignedByteType,
        )
      : undefined
    if (texture) {
      texture.colorSpace = THREE.SRGBColorSpace
      texture.wrapS = THREE.RepeatWrapping
      texture.wrapT = THREE.RepeatWrapping
      texture.flipY = true
      texture.needsUpdate = true
    }
    const material = new THREE.MeshBasicMaterial({
      color: texture ? 0xffffff : debugColor(identity),
      map: texture,
      lightMap,
      lightMapIntensity: 1,
      transparent: (resolved.features & 1) !== 0,
      alphaTest: (resolved.features & 4) !== 0 ? 0.5 : 0,
      side: worldMaterialSide(resolved.features),
    })
    return { material, texture }
  }

  function sourceTransform(object: THREE.Object3D, position: readonly number[], angles: readonly number[]): void {
    object.position.set(position[0]!, position[1]!, position[2]!)
    object.rotation.set(
      THREE.MathUtils.degToRad(angles[2]!),
      THREE.MathUtils.degToRad(angles[0]!),
      THREE.MathUtils.degToRad(angles[1]!),
      "ZYX",
    )
  }

  return Object.freeze({
    async loadMap(payload: Uint8Array, payloadSha256: string, debugMissingMaterials: boolean): Promise<SceneResult> {
      if (disposed) throw new RenderingError("InvalidState", "renderer is disposed")
      if (!HASH.test(payloadSha256) || await digest(payload) !== payloadSha256) {
        throw new RenderingError("MalformedInput", "runtime map payload identity differs")
      }
      const parsed = parseRuntimeMap(payload)
      const missing = [
        ...parsed.materials,
        ...parsed.models.flatMap((model) => model.materials),
      ].filter((material) => !material.baseTexture)
      if (!debugMissingMaterials && missing.length > 0) {
        throw new RenderingError("MissingInput", "resolved material and texture descriptors are unavailable")
      }
      const staged = new THREE.Group()
      const stagedTemplates = new Map<string, THREE.Group>()
      const stagedResources: Array<THREE.BufferGeometry | THREE.Material | THREE.Texture> = []
      const stagedLightmap = parsed.lightmap
        ? new THREE.DataTexture(
            parsed.lightmap.rgba,
            parsed.lightmap.width,
            parsed.lightmap.height,
            THREE.RGBAFormat,
            THREE.FloatType,
          )
        : undefined
      if (stagedLightmap) {
        configureWorldLightmap(stagedLightmap)
      }
      try {
        for (const batch of parsed.batches) {
          const geometry = new THREE.BufferGeometry()
          geometry.setAttribute("position", new THREE.BufferAttribute(batch.positions, 3))
          geometry.setAttribute("normal", new THREE.BufferAttribute(batch.normals, 3))
          geometry.setAttribute("uv", new THREE.BufferAttribute(batch.uv, 2))
          geometry.setAttribute("uv1", new THREE.BufferAttribute(batch.lightmapUv, 2))
          geometry.setIndex(new THREE.BufferAttribute(batch.indices, 1))
          geometry.computeBoundingSphere()
          const materialIdentity = parsed.materials[batch.material]!.logicalPath
          const resolved = parsed.materials[batch.material]!
          const { material, texture } = materialFor(resolved, materialIdentity, stagedLightmap)
          const mesh = new THREE.Mesh(geometry, material)
          mesh.userData.materialIdentity = materialIdentity
          mesh.userData.texture = texture
          staged.add(mesh)
        }
        for (const model of parsed.models) {
          const template = new THREE.Group()
          for (const primitive of model.primitives) {
            const geometry = new THREE.BufferGeometry()
            geometry.setAttribute("position", new THREE.BufferAttribute(primitive.positions, 3))
            geometry.setAttribute("normal", new THREE.BufferAttribute(primitive.normals, 3))
            geometry.setAttribute("uv", new THREE.BufferAttribute(primitive.uv, 2))
            geometry.setIndex(new THREE.BufferAttribute(primitive.indices, 1))
            geometry.computeBoundingSphere()
            const resolved = model.materials[primitive.material]!
            const created = materialFor(resolved, resolved.logicalPath)
            template.add(new THREE.Mesh(geometry, created.material))
            stagedResources.push(geometry, created.material)
            if (created.texture) stagedResources.push(created.texture)
          }
          stagedTemplates.set(model.logicalPath, template)
        }
        for (const occurrence of parsed.modelOccurrences) {
          const model = parsed.models[occurrence.model]!
          const instance = stagedTemplates.get(model.logicalPath)!.clone(true)
          instance.userData.entity = occurrence.entity
          sourceTransform(instance, occurrence.position, occurrence.angles)
          staged.add(instance)
        }
      } catch {
        clear(staged)
        for (const resource of stagedResources) resource.dispose()
        stagedLightmap?.dispose()
        throw new RenderingError("BoundExceeded", "runtime map GPU staging failed")
      }
      clear(world)
      disposeModels()
      worldLightmap?.dispose()
      for (const child of [...staged.children]) world.add(child)
      modelTemplates = stagedTemplates
      modelResources = stagedResources
      worldLightmap = stagedLightmap
      map = parsed
      const diagnostics = Object.freeze(missing.map((material) => Object.freeze({
        code: "MissingMaterial" as const,
        identity: material.logicalPath,
      })))
      return Object.freeze({
        payloadSha256,
        drawableSurfaces: parsed.drawableSurfaces,
        drawBatches: parsed.batches.length,
        diagnostics,
      })
    },
    async render(frame: Frame): Promise<void> {
      if (disposed || !map) throw new RenderingError("InvalidState", "renderer has no active map")
      if (
        frame.effects.length + (frame.models?.length ?? 0) > MAX_EFFECTS
        || !finite([
          ...frame.camera.position,
          frame.camera.yawDegrees,
          frame.camera.pitchDegrees,
          frame.camera.verticalFovDegrees,
          frame.camera.near,
          frame.camera.far,
        ])
        || frame.camera.verticalFovDegrees <= 0
        || frame.camera.verticalFovDegrees >= 180
        || frame.camera.near <= 0
        || frame.camera.far <= frame.camera.near
      ) {
        throw new RenderingError("MalformedInput", "render frame is invalid")
      }
      camera.position.set(...frame.camera.position)
      camera.fov = frame.camera.verticalFovDegrees
      camera.near = frame.camera.near
      camera.far = frame.camera.far
      camera.updateProjectionMatrix()
      const yaw = THREE.MathUtils.degToRad(frame.camera.yawDegrees)
      const pitch = THREE.MathUtils.degToRad(frame.camera.pitchDegrees)
      const direction = new THREE.Vector3(
        Math.cos(pitch) * Math.cos(yaw),
        Math.cos(pitch) * Math.sin(yaw),
        -Math.sin(pitch),
      )
      camera.lookAt(camera.position.clone().add(direction))
      for (const effect of frame.effects) {
        if (
          !Number.isSafeInteger(effect.identity)
          || effect.identity < 1
          || !Number.isSafeInteger(effect.color)
          || effect.color < 0
          || effect.color > 0xff_ffff
          || !finite([...effect.position, effect.radius, effect.opacity])
          || effect.radius <= 0
          || effect.opacity < 0
          || effect.opacity > 1
        ) {
          throw new RenderingError("MalformedInput", "render effect is invalid")
        }
      }
      const stagedEffects = new THREE.Group()
      const stagedViewModels = new THREE.Group()
      try {
        for (const effect of frame.effects) {
          const material = new THREE.MeshBasicMaterial({
            color: effect.color,
            transparent: effect.opacity < 1,
            opacity: effect.opacity,
          })
          const mesh = new THREE.Mesh(effectGeometry, material)
          mesh.position.set(...effect.position)
          mesh.scale.setScalar(effect.radius)
          mesh.userData.identity = effect.identity
          stagedEffects.add(mesh)
        }
        for (const item of frame.models ?? []) {
          if (
            !Number.isSafeInteger(item.identity)
            || item.identity < 1
            || !finite([...item.position, ...item.angles, item.scale])
            || item.scale <= 0
          ) {
            throw new RenderingError("MalformedInput", "render model item is invalid")
          }
          const template = modelTemplates.get(item.model)
          if (!template) throw new RenderingError("MissingInput", `runtime model ${item.model} is unavailable`)
          const instance = template.clone(true)
          sourceTransform(instance, item.position, item.angles)
          instance.scale.setScalar(item.scale)
          instance.userData.identity = item.identity
          if (item.viewModel) {
            const sourceBasis = new THREE.Matrix4().set(
              0, -1, 0, 0,
              0, 0, 1, 0,
              -1, 0, 0, 0,
              0, 0, 0, 1,
            )
            const wrapper = new THREE.Group()
            wrapper.setRotationFromMatrix(sourceBasis)
            wrapper.add(instance)
            stagedViewModels.add(wrapper)
          } else {
            stagedEffects.add(instance)
          }
        }
      } catch {
        clear(stagedEffects, false)
        throw new RenderingError("BoundExceeded", "render effect staging failed")
      }
      clear(effects, false)
      clear(viewModels, false)
      for (const child of [...stagedEffects.children]) effects.add(child)
      for (const child of [...stagedViewModels.children]) viewModels.add(child)
      await renderer.renderAsync(scene, camera)
    },
    resize(cssWidth: number, cssHeight: number, devicePixelRatio: number): void {
      if (
        disposed
        || !finite([cssWidth, cssHeight, devicePixelRatio])
        || cssWidth < 0
        || cssHeight < 0
        || devicePixelRatio <= 0
        || cssWidth * devicePixelRatio > MAX_DIMENSION
        || cssHeight * devicePixelRatio > MAX_DIMENSION
      ) {
        throw new RenderingError("BoundExceeded", "renderer dimensions are invalid")
      }
      renderer.setPixelRatio(devicePixelRatio)
      renderer.setSize(cssWidth, cssHeight, false)
      camera.aspect = cssHeight === 0 ? 1 : cssWidth / cssHeight
      camera.updateProjectionMatrix()
    },
    dispose(): void {
      if (disposed) return
      disposed = true
      clear(world)
      clear(effects, false)
      clear(viewModels, false)
      disposeModels()
      worldLightmap?.dispose()
      worldLightmap = undefined
      effectGeometry.dispose()
      renderer.dispose()
    },
  })
}
