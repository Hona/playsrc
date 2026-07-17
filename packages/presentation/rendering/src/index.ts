import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import {
  ExposureController,
  SOURCE_LDR,
  SOURCE_PC_INTEGER_HDR,
  validateRenderConfiguration,
  type CanvasAlphaMode,
  type CanvasFormat,
  type ExposureConfiguration,
  type ExposureSnapshot,
  type OutputColorSpace,
  type RenderConfiguration,
  type ToneOperator,
} from "./color-output"
import { configureWorldLightmap, worldMaterialSide } from "./material-state"
import { OwnedResourceGeneration } from "./resource-generation"
import {
  buildRuntimeLightmap,
  parseRuntimeMap,
  parseRuntimeMapVerified,
  type ProfileRequirement,
  type RuntimeLightmap,
  type RuntimeMap,
  type RuntimeMaterial,
} from "./runtime-map"

export {
  ExposureController,
  SOURCE_EXPOSURE,
  SOURCE_LDR,
  SOURCE_PC_INTEGER_HDR,
  applyColorOutput,
  buildLuminanceHistogram,
  exposureTarget,
  histogramBoundary,
  linearToSrgb,
  validateRenderConfiguration,
  type CanvasAlphaMode,
  type CanvasFormat,
  type ExposureConfiguration,
  type ExposureSnapshot,
  type OutputColorSpace,
  type RenderConfiguration,
  type ToneOperator,
} from "./color-output"
export {
  LightingInputError,
  SOURCE_BUMP_BASIS,
  directionalWeights,
  evaluateDirectionalLightmap,
  prepareWorldLights,
  sampleAmbientCube,
  type AmbientCubeRequest,
  type DirectionalLightingInput,
  type PreparedWorldLight,
  type WorldLightRequest,
} from "./lighting"
export {
  RuntimeMapError,
  buildRuntimeLightmap,
  parseRuntimeMap,
  parseRuntimeMapVerified,
  validateRuntimeMapHashes,
  type HdrProfile,
  type LightingMember,
  type ProfileRequirement,
  type Rgb,
  type RuntimeAmbientIndex,
  type RuntimeAmbientSample,
  type RuntimeInput,
  type RuntimeLightmap,
  type RuntimeMap,
  type RuntimeProfileMaterial,
  type RuntimeWorldLight,
  type SurfaceLighting,
  type SurfaceLightingKind,
} from "./runtime-map"

const MAX_EFFECTS = 4_096
const MAX_DIMENSION = 8_192
const HASH = /^[0-9a-f]{64}$/

type Canvas = HTMLCanvasElement | OffscreenCanvas

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
  angles?: readonly [number, number, number]
  orientation?: readonly [number, number, number, number]
  scale: number
  skin?: number
  viewModel?: boolean
}>

export type FrameCaptureRequest = Readonly<{ format: "image/png" }>

export type Frame = Readonly<{
  camera: Camera
  effects: readonly Effect[]
  models?: readonly ModelItem[]
  lightStyles?: readonly Readonly<{ style: number; scalar: number }>[]
  exposureHistogram?: Uint32Array
  deltaSeconds?: number
  capture?: FrameCaptureRequest
  visibility?: Readonly<{ worldIdentity: string; cacheIdentity: string; surfaces: Uint32Array }>
}>

export type DirectionalTextureInput = Readonly<{
  material: string
  kind: "normal" | "ssbump"
  logicalPath: string
  sha256: string
  width: number
  height: number
  rgba: Uint8Array
  uvTransform: readonly [number, number, number, number, number, number]
}>
export type EnvironmentTextureInput = Readonly<{
  material: string
  logicalPath: string
  width: number
  height: number
  sha256: string
  rgba: Uint8Array
}>
export type EnvironmentFragmentInput = Readonly<{
  model: number
  face: number
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
}>
export type EnvironmentInput = Readonly<{
  profile: "ldr" | "hdr"
  identity: string
  clusters: number
  nodes: number
  leaves: number
  skySurfaces: number
  cubemaps: number
  waterSurfaces: number
  waterVolumes: number
  marks: number
  markFragments: number
  controllers: number
  markRecords: readonly Readonly<{
    status: number
    kind: number
    enabled: boolean
    dynamic: boolean
    material: string
    fragments: readonly EnvironmentFragmentInput[]
  }>[]
  textures: readonly EnvironmentTextureInput[]
}>

export type MapLoadRequest = Readonly<{
  payload: Uint8Array
  payloadSha256: string
  lightStyles?: readonly Readonly<{ style: number; scalar: number }>[]
  directionalTextures?: readonly DirectionalTextureInput[]
  modelTextures?: readonly Readonly<{
    material: string
    logicalPath: string
    width: number
    height: number
    sha256: string
    rgba: Uint8Array
  }>[]
  environment?: EnvironmentInput
  diagnostic?: boolean
  signal?: AbortSignal
}>

export type SceneDiagnostic = Readonly<{
  code: "MissingMaterial" | "MissingDirectionalInput" | "MissingProfileInput" | "UnsupportedProfileInput"
  identity: string
  detail: string
}>

export type SceneResult = Readonly<{
  payloadSha256: string
  lightingProfile: "ldr" | "hdr"
  deviceGeneration: number
  sceneGeneration: number
  drawableSurfaces: number
  drawBatches: number
  lightingSampleCount: number
  directionalFaces: number
  worldLights: number
  ambientSamples: number
  requirements: readonly ProfileRequirement[]
  diagnostics: readonly SceneDiagnostic[]
  resources: Readonly<{ geometries: number; materials: number; textures: number }>
  environment?: MapLoadRequest["environment"]
  environmentDrawables: number
}>

export type FrameCapture = Readonly<{
  format: "image/png"
  sha256: string
  bytes: Uint8Array
}>

export type FrameResult = Readonly<{
  deviceGeneration: number
  sceneGeneration: number
  submission: number
  exposure: ExposureSnapshot
  capture?: FrameCapture
}>

export type ResizeResult = Readonly<{
  width: number
  height: number
  suspended: boolean
  deviceGeneration: number
}>

export type RendererLifecycle = "Initializing" | "Ready" | "Recovering" | "Failed" | "Disposed"

export type RendererCreateRequest = Readonly<{
  canvas: Canvas
  configuration: RenderConfiguration
  powerPreference?: "low-power" | "high-performance"
}>

export type FramePacingCallback = (timestampMilliseconds: number) => Frame | undefined | Promise<Frame | undefined>

export interface Renderer {
  readonly configuration: RenderConfiguration
  readonly lifecycle: RendererLifecycle
  readonly deviceGeneration: number
  readonly sceneGeneration: number
  loadMap(request: MapLoadRequest): Promise<SceneResult>
  render(frame: Frame): Promise<FrameResult>
  resize(cssWidth: number, cssHeight: number, devicePixelRatio: number): ResizeResult
  startFramePacing(callback: FramePacingCallback): void
  stopFramePacing(): void
  dispose(): Promise<void>
}

export class RenderingError extends Error {
  constructor(
    readonly code:
      | "MalformedInput"
      | "MissingInput"
      | "IdentityMismatch"
      | "UnsupportedEnvironment"
      | "UnsupportedFeature"
      | "BoundExceeded"
      | "InvalidState"
      | "DeviceLost"
      | "CaptureFailed",
    message: string,
  ) {
    super(message)
    this.name = "RenderingError"
  }
}

type SceneResources = {
  map: RuntimeMap
  payload: Uint8Array
  payloadSha256: string
  loadRequest: Omit<MapLoadRequest, "payload" | "signal">
  group: THREE.Group
  modelTemplates: Map<string, THREE.Group>
  disposables: OwnedResourceGeneration
  lightmapTextures: readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?]
  exposureUniform: ReturnType<typeof TSL.uniform>
  diagnostics: readonly SceneDiagnostic[]
  worldBatches: readonly { mesh: THREE.Mesh; faces: Uint32Array }[]
  result: SceneResult
  disposed: boolean
}

type Backend = THREE.WebGPURenderer & {
  backend: {
    isWebGPUBackend?: boolean
    device?: GPUDevice
    context?: GPUCanvasContext
  }
  onDeviceLost: (info: unknown) => void
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
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer))
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")
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
function modelKey(model: string, skin: number) {
  return skin === 0 ? model : `${model}#skin=${skin}`
}

function disposeScene(scene: SceneResources): void {
  if (scene.disposed) return
  scene.disposed = true
  scene.group.clear()
  scene.disposables.dispose()
  scene.modelTemplates.clear()
}

function textureFromRgba(
  input: { rgba: Uint8Array; width: number; height: number },
  colorSpace: string,
): THREE.DataTexture {
  const texture = new THREE.DataTexture(input.rgba, input.width, input.height, THREE.RGBAFormat, THREE.UnsignedByteType)
  texture.colorSpace = colorSpace
  texture.wrapS = THREE.RepeatWrapping
  texture.wrapT = THREE.RepeatWrapping
  texture.flipY = true
  texture.needsUpdate = true
  return texture
}

function textureFromLightmap(lightmap: RuntimeLightmap, plane: Float32Array): THREE.DataTexture {
  const texture = new THREE.DataTexture(plane, lightmap.width, lightmap.height, THREE.RGBAFormat, THREE.FloatType)
  configureWorldLightmap(texture, lightmap.profile)
  return texture
}

function materialOptions(resolved: RuntimeMaterial): THREE.MeshBasicMaterialParameters {
  return {
    transparent: (resolved.features & 1) !== 0,
    alphaTest: (resolved.features & 4) !== 0 ? 0.5 : 0,
    side: worldMaterialSide(resolved.features),
  }
}

function worldNodeMaterial(
  resolved: RuntimeMaterial,
  identity: string,
  baseTexture: THREE.DataTexture | undefined,
  lightmaps: readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?],
  directional: THREE.DataTexture | undefined,
  directionalKind: "normal" | "ssbump" | undefined,
  directionalUvTransform: DirectionalTextureInput["uvTransform"] | undefined,
  exposure: ReturnType<typeof TSL.uniform>,
): THREE.MeshBasicNodeMaterial {
  const material = new THREE.MeshBasicNodeMaterial(materialOptions(resolved))
  const base = baseTexture ? TSL.texture(baseTexture, TSL.uv()) : TSL.vec4(TSL.color(debugColor(identity)), 1)
  const flat = TSL.texture(lightmaps[0], TSL.uv(1)).rgb
  let irradiance = flat
  if (directional && lightmaps[1] && lightmaps[2] && lightmaps[3] && directionalKind && directionalUvTransform) {
    const baseUv = TSL.uv()
    const directionalUv = TSL.vec2(
      baseUv.x
        .mul(directionalUvTransform[0])
        .add(baseUv.y.mul(directionalUvTransform[2]))
        .add(directionalUvTransform[4]),
      baseUv.x
        .mul(directionalUvTransform[1])
        .add(baseUv.y.mul(directionalUvTransform[3]))
        .add(directionalUvTransform[5]),
    )
    const source = TSL.texture(directional, directionalUv).rgb
    let weights
    if (directionalKind === "ssbump") {
      weights = TSL.max(source, TSL.vec3(0))
    } else {
      const normal = source.mul(2).sub(1)
      const projections = [
        TSL.clamp(TSL.dot(normal, TSL.vec3(0.8164966106414795, 0, 0.5773502588272095)), 0, 1),
        TSL.clamp(TSL.dot(normal, TSL.vec3(-0.40824833512306213, 0.7071067690849304, 0.5773502588272095)), 0, 1),
        TSL.clamp(TSL.dot(normal, TSL.vec3(-0.4082482159137726, -0.7071068286895752, 0.5773502588272095)), 0, 1),
      ]
      const squared = TSL.vec3(
        projections[0]!.mul(projections[0]),
        projections[1]!.mul(projections[1]),
        projections[2]!.mul(projections[2]),
      )
      weights = squared.div(TSL.max(squared.x.add(squared.y).add(squared.z), 1e-12))
    }
    const directionalLight = TSL.texture(lightmaps[1], TSL.uv(1))
      .rgb.mul(weights.x)
      .add(TSL.texture(lightmaps[2], TSL.uv(1)).rgb.mul(weights.y))
      .add(TSL.texture(lightmaps[3], TSL.uv(1)).rgb.mul(weights.z))
    irradiance = TSL.attribute("lightmapKind", "float").greaterThan(1.5).select(directionalLight, flat)
  }
  material.colorNode = TSL.vec4(base.rgb.mul(irradiance).mul(exposure), base.a)
  material.toneMapped = false
  return material
}

function diagnostic(code: SceneDiagnostic["code"], identity: string, detail: string): SceneDiagnostic {
  return Object.freeze({ code, identity, detail })
}

async function validateDirectionalInputs(
  inputs: readonly DirectionalTextureInput[],
): Promise<Map<string, DirectionalTextureInput>> {
  const result = new Map<string, DirectionalTextureInput>()
  for (const input of inputs) {
    const identity = input.material.toLowerCase()
    if (
      !input.material ||
      !input.logicalPath ||
      !HASH.test(input.sha256) ||
      (input.kind !== "normal" && input.kind !== "ssbump") ||
      !Number.isSafeInteger(input.width) ||
      input.width < 1 ||
      input.width > MAX_DIMENSION ||
      !Number.isSafeInteger(input.height) ||
      input.height < 1 ||
      input.height > MAX_DIMENSION ||
      input.width * input.height * 4 !== input.rgba.byteLength ||
      input.uvTransform.length !== 6 ||
      !input.uvTransform.every(Number.isFinite) ||
      result.has(identity) ||
      (await digest(input.rgba)) !== input.sha256
    ) {
      throw new RenderingError("MalformedInput", "directional texture input is invalid")
    }
    result.set(
      identity,
      Object.freeze({
        ...input,
        rgba: input.rgba.slice(),
        uvTransform: Object.freeze([...input.uvTransform]) as DirectionalTextureInput["uvTransform"],
      }),
    )
  }
  return result
}

class RendererOwner implements Renderer {
  readonly configuration: RenderConfiguration
  readonly #canvas: Canvas
  readonly #powerPreference: "low-power" | "high-performance" | undefined
  readonly #exposure: ExposureController
  #lifecycle: RendererLifecycle = "Initializing"
  #deviceGeneration = 0
  #sceneGeneration = 0
  #submission = 0
  #backend!: Backend
  #scene = new THREE.Scene()
  #world = new THREE.Group()
  #effects = new THREE.Group()
  #viewModels = new THREE.Group()
  #camera = new THREE.PerspectiveCamera(75, 1, 1, 32_768)
  #effectGeometry = new THREE.SphereGeometry(1, 10, 6)
  #active?: SceneResources
  #renderBusy = false
  #loadOrdinal = 0
  #suspended = false
  #pacingHandle?: number
  #pacingCallback?: FramePacingCallback
  #pacingBusy = false

  constructor(request: RendererCreateRequest) {
    this.configuration = validateRenderConfiguration(request.configuration)
    this.#canvas = request.canvas
    this.#powerPreference = request.powerPreference
    this.#exposure = new ExposureController(this.configuration.exposure)
    this.#scene.background = null
    this.#scene.add(this.#world, this.#effects, this.#camera)
    this.#camera.add(this.#viewModels)
    this.#camera.up.set(0, 0, 1)
  }

  get lifecycle(): RendererLifecycle {
    return this.#lifecycle
  }
  get deviceGeneration(): number {
    return this.#deviceGeneration
  }
  get sceneGeneration(): number {
    return this.#sceneGeneration
  }

  async initialize(): Promise<this> {
    if (!globalThis.navigator?.gpu || !this.#canvas || typeof this.#canvas.getContext !== "function") {
      throw new RenderingError("UnsupportedEnvironment", "WebGPU canvas is unavailable")
    }
    if (navigator.gpu.getPreferredCanvasFormat() !== this.configuration.canvasFormat) {
      throw new RenderingError("UnsupportedFeature", "preferred WebGPU canvas format differs from the required format")
    }
    this.#backend = await this.#createBackend()
    this.#deviceGeneration = 1
    this.#lifecycle = "Ready"
    return this
  }

  async #createBackend(): Promise<Backend> {
    const backend = new THREE.WebGPURenderer({
      canvas: this.#canvas,
      antialias: true,
      alpha: this.configuration.alphaMode === "premultiplied",
      powerPreference: this.#powerPreference,
    }) as Backend
    backend.onDeviceLost = () => {
      void this.#recover()
    }
    let context: GPUCanvasContext | undefined
    try {
      await backend.init()
      if (!backend.backend.isWebGPUBackend) throw new Error("fallback backend")
      backend.outputColorSpace = THREE.SRGBColorSpace
      backend.toneMapping = THREE.NoToneMapping
      context = backend.backend.context
      const actual = (
        context as GPUCanvasContext & { getConfiguration?(): GPUCanvasConfiguration | null }
      ).getConfiguration?.()
      if (actual) {
        if (
          actual.format !== this.configuration.canvasFormat ||
          actual.alphaMode !== this.configuration.alphaMode ||
          (actual.colorSpace ?? "srgb") !== this.configuration.outputColorSpace
        ) {
          throw new Error("canvas configuration mismatch")
        }
      }
      return backend
    } catch (error) {
      backend.dispose()
      try {
        context?.unconfigure()
      } catch {
        /* already unconfigured */
      }
      throw new RenderingError("UnsupportedEnvironment", `WebGPU renderer initialization failed: ${String(error)}`)
    }
  }

  async loadMap(request: MapLoadRequest): Promise<SceneResult> {
    this.#requireReady()
    if (!HASH.test(request.payloadSha256))
      throw new RenderingError("MalformedInput", "runtime map payload SHA-256 is invalid")
    const ordinal = ++this.#loadOrdinal
    const payload = request.payload.slice()
    this.#checkAbort(request.signal, ordinal)
    if ((await digest(payload)) !== request.payloadSha256)
      throw new RenderingError("IdentityMismatch", "runtime map payload identity differs")
    this.#checkAbort(request.signal, ordinal)
    let map: RuntimeMap
    try {
      map = await parseRuntimeMapVerified(payload)
    } catch (error) {
      throw new RenderingError("MalformedInput", `runtime map validation failed: ${String(error)}`)
    }
    if (map.lighting.profile !== this.configuration.lightingProfile) {
      throw new RenderingError("IdentityMismatch", "runtime map lighting profile differs from renderer configuration")
    }
    if (map.lighting.profile === "hdr" && request.lightStyles) {
      map = Object.freeze({ ...map, lightmap: buildRuntimeLightmap(map, request.lightStyles) })
    }
    if (!map.lightmap) throw new RenderingError("MissingInput", "explicit light-style scalars are required")
    const directionalInputs = await validateDirectionalInputs(request.directionalTextures ?? [])
    if (
      request.environment &&
      (request.environment.profile !== map.lighting.profile ||
        !HASH.test(request.environment.identity) ||
        Object.values(request.environment).some(
          (value) => typeof value === "number" && (!Number.isSafeInteger(value) || value < 0),
        ))
    )
      throw new RenderingError("MalformedInput", "world environment input is invalid")
    for (const texture of request.environment?.textures ?? []) {
      if (
        texture.width * texture.height * 4 !== texture.rgba.byteLength ||
        (await digest(texture.rgba)) !== texture.sha256
      )
        throw new RenderingError("MalformedInput", "environment texture input is invalid")
    }
    const modelTextures = new Map<string, NonNullable<MapLoadRequest["modelTextures"]>[number]>()
    for (const texture of request.modelTextures ?? []) {
      if (
        modelTextures.has(texture.material.toLowerCase()) ||
        texture.width * texture.height * 4 !== texture.rgba.byteLength ||
        (await digest(texture.rgba)) !== texture.sha256
      )
        throw new RenderingError("MalformedInput", "model texture input is invalid")
      modelTextures.set(texture.material.toLowerCase(), texture)
    }
    const materialIdentities = new Set([
      ...map.materials.map((material) => material.logicalPath.toLowerCase()),
      ...map.models.flatMap((model) => model.materials.map((material) => material.logicalPath.toLowerCase())),
    ])
    if ([...directionalInputs.keys()].some((identity) => !materialIdentities.has(identity))) {
      throw new RenderingError("MalformedInput", "directional texture names an unavailable material")
    }
    this.#checkAbort(request.signal, ordinal)
    const staged = this.#buildScene(
      map,
      payload,
      request.payloadSha256,
      directionalInputs,
      request,
      this.#sceneGeneration + 1,
    )
    try {
      this.#checkAbort(request.signal, ordinal)
      if (staged.diagnostics.length > 0 && !request.diagnostic) {
        const missing = staged.diagnostics.some((item) => item.code.startsWith("Missing"))
        throw new RenderingError(
          missing ? "MissingInput" : "UnsupportedFeature",
          "required material, lighting, or profile presentation inputs are unavailable",
        )
      }
      const prior = this.#active
      this.#world.clear()
      this.#world.add(staged.group)
      this.#scene.background = request.diagnostic ? new THREE.Color(0x111820) : null
      this.#active = staged
      this.#sceneGeneration += 1
      if (prior) await this.#retire(prior)
      return staged.result
    } catch (error) {
      disposeScene(staged)
      throw error
    }
  }

  #buildScene(
    map: RuntimeMap,
    payload: Uint8Array,
    payloadSha256: string,
    directionalInputs: Map<string, DirectionalTextureInput>,
    request: MapLoadRequest,
    sceneGeneration: number,
  ): SceneResources {
    const group = new THREE.Group()
    const modelTemplates = new Map<string, THREE.Group>()
    const disposables = new OwnedResourceGeneration(this.#deviceGeneration, sceneGeneration)
    const diagnostics: SceneDiagnostic[] = []
    const worldBatches: { mesh: THREE.Mesh; faces: Uint32Array }[] = []
    const lightmap = map.lightmap
    if (!lightmap) throw new RenderingError("MissingInput", "runtime lightmap is unavailable")
    const lightmapTextures: [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?] = [
      textureFromLightmap(lightmap, lightmap.flat),
    ]
    disposables.add(lightmapTextures[0])
    if (lightmap.directional) {
      for (let index = 0; index < 3; index += 1) {
        const texture = textureFromLightmap(lightmap, lightmap.directional[index]!)
        lightmapTextures[index + 1] = texture
        disposables.add(texture)
      }
    }
    const exposureUniform = TSL.uniform(
      this.configuration.lightingProfile === "hdr" ? this.#exposure.snapshot().current : 1,
      "float",
    )
    const directionalGpu = new Map<string, { input: DirectionalTextureInput; texture: THREE.DataTexture }>()
    for (const [identity, input] of directionalInputs) {
      const texture = textureFromRgba(input, THREE.NoColorSpace)
      directionalGpu.set(identity, { input, texture })
      disposables.add(texture)
    }

    const supplemental = new Map(
      (request.modelTextures ?? []).map((texture) => [texture.material.toLowerCase(), texture] as const),
    )
    const createBase = (resolved: RuntimeMaterial, identity: string): THREE.DataTexture | undefined => {
      const source = resolved.baseTexture ?? supplemental.get(identity.toLowerCase())
      if (!source) {
        diagnostics.push(diagnostic("MissingMaterial", identity, "resolved base texture is unavailable"))
        return undefined
      }
      const texture = textureFromRgba(source, THREE.SRGBColorSpace)
      disposables.add(texture)
      return texture
    }
    try {
      for (const batch of map.batches) {
        const geometry = new THREE.BufferGeometry()
        geometry.setAttribute("position", new THREE.BufferAttribute(batch.positions, 3))
        geometry.setAttribute("normal", new THREE.BufferAttribute(batch.normals, 3))
        geometry.setAttribute("uv", new THREE.BufferAttribute(batch.uv, 2))
        geometry.setAttribute("uv1", new THREE.BufferAttribute(batch.lightmapUv, 2))
        geometry.setAttribute("lightmapKind", new THREE.BufferAttribute(batch.lightmapKind, 1))
        geometry.setIndex(new THREE.BufferAttribute(batch.indices, 1))
        geometry.computeBoundingSphere()
        disposables.add(geometry)
        const resolved = map.materials[batch.material]!
        const identity = resolved.logicalPath
        const baseTexture = createBase(resolved, identity)
        const kinds = new Set(batch.lightmapKind)
        const requiresNormal = kinds.has(2)
        const requiresSsbump = kinds.has(3)
        const supplied = directionalGpu.get(identity.toLowerCase())
        if (
          (requiresNormal && supplied?.input.kind !== "normal") ||
          (requiresSsbump && supplied?.input.kind !== "ssbump")
        ) {
          diagnostics.push(
            diagnostic(
              "MissingDirectionalInput",
              identity,
              `a ${requiresSsbump ? "ssbump" : "normal"} texture plane is required`,
            ),
          )
        }
        let material: THREE.Material
        if (map.lighting.profile === "hdr") {
          material = worldNodeMaterial(
            resolved,
            identity,
            baseTexture,
            lightmapTextures,
            supplied?.texture,
            supplied?.input.kind,
            supplied?.input.uvTransform,
            exposureUniform,
          )
        } else {
          material = new THREE.MeshBasicMaterial({
            ...materialOptions(resolved),
            color: baseTexture ? 0xffffff : debugColor(identity),
            map: baseTexture,
            lightMap: lightmapTextures[0],
            lightMapIntensity: 1,
            toneMapped: false,
          })
        }
        disposables.add(material)
        const mesh = new THREE.Mesh(geometry, material)
        worldBatches.push({ mesh, faces: batch.faces.slice() })
        mesh.userData.materialIdentity = identity
        group.add(mesh)
      }

      const environmentTextures = new Map<string, THREE.DataTexture>()
      for (const texture of request.environment?.textures ?? []) {
        const value = textureFromRgba(texture, THREE.SRGBColorSpace)
        environmentTextures.set(texture.material.toLowerCase(), value)
        disposables.add(value)
      }
      for (const mark of request.environment?.markRecords ?? []) {
        if (mark.status !== 0 || !mark.enabled) continue
        const texture = environmentTextures.get(mark.material.toLowerCase())
        if (!texture) {
          diagnostics.push(diagnostic("MissingMaterial", mark.material, "projected mark texture is unavailable"))
          continue
        }
        for (const fragment of mark.fragments) {
          const geometry = new THREE.BufferGeometry()
          geometry.setAttribute("position", new THREE.BufferAttribute(fragment.positions, 3))
          geometry.setAttribute("normal", new THREE.BufferAttribute(fragment.normals, 3))
          geometry.setAttribute("uv", new THREE.BufferAttribute(fragment.uv, 2))
          geometry.setIndex(new THREE.BufferAttribute(fragment.indices, 1))
          disposables.add(geometry)
          const material = new THREE.MeshBasicMaterial({
            map: texture,
            transparent: true,
            depthWrite: false,
            polygonOffset: true,
            polygonOffsetFactor: -1,
            toneMapped: false,
          })
          disposables.add(material)
          group.add(new THREE.Mesh(geometry, material))
        }
      }

      for (const model of map.models) {
        const template = new THREE.Group()
        for (const primitive of model.primitives) {
          const geometry = new THREE.BufferGeometry()
          geometry.setAttribute("position", new THREE.BufferAttribute(primitive.positions, 3))
          geometry.setAttribute("normal", new THREE.BufferAttribute(primitive.normals, 3))
          geometry.setAttribute("uv", new THREE.BufferAttribute(primitive.uv, 2))
          geometry.setIndex(new THREE.BufferAttribute(primitive.indices, 1))
          geometry.computeBoundingSphere()
          disposables.add(geometry)
          const resolved = model.materials[primitive.material]!
          const baseTexture = createBase(resolved, resolved.logicalPath)
          const material = new THREE.MeshBasicMaterial({
            ...materialOptions(resolved),
            color: baseTexture ? 0xffffff : debugColor(resolved.logicalPath),
            map: baseTexture,
            toneMapped: false,
          })
          disposables.add(material)
          template.add(new THREE.Mesh(geometry, material))
        }
        modelTemplates.set(model.logicalPath, template)
      }
      for (const occurrence of map.modelOccurrences) {
        const model = map.models[occurrence.model]!
        const instance = modelTemplates.get(model.logicalPath)!.clone(true)
        instance.userData.entity = occurrence.entity
        sourceTransform(instance, occurrence.position, occurrence.angles)
        group.add(instance)
      }
    } catch (error) {
      const failed = {
        group,
        modelTemplates,
        disposables,
        disposed: false,
      } as SceneResources
      disposeScene(failed)
      throw new RenderingError("BoundExceeded", `runtime map GPU staging failed: ${String(error)}`)
    }

    const requirements = map.lighting.profile === "hdr" ? map.lighting.descriptor.requirements : Object.freeze([])
    for (const requirement of requirements) {
      diagnostics.push(
        diagnostic(
          requirement.disposition === "Missing" ? "MissingProfileInput" : "UnsupportedProfileInput",
          requirement.identity,
          requirement.reason,
        ),
      )
    }
    diagnostics.sort(
      (left, right) => left.code.localeCompare(right.code) || left.identity.localeCompare(right.identity),
    )
    const directionalFaces = map.lighting.profile === "hdr" ? map.lighting.descriptor.directionalFaces : 0
    const worldLights = map.lighting.profile === "hdr" ? map.lighting.descriptor.worldLights.length : 0
    const ambientSamples = map.lighting.profile === "hdr" ? map.lighting.descriptor.ambientSamples.length : 0
    const result: SceneResult = Object.freeze({
      payloadSha256,
      lightingProfile: map.lighting.profile,
      deviceGeneration: this.#deviceGeneration,
      sceneGeneration,
      drawableSurfaces: map.drawableSurfaces,
      drawBatches: map.batches.length,
      lightingSampleCount: map.lightingSampleCount,
      directionalFaces,
      worldLights,
      ambientSamples,
      requirements,
      diagnostics: Object.freeze(diagnostics),
      resources: Object.freeze({
        geometries: [...disposables].filter((value) => value instanceof THREE.BufferGeometry).length,
        materials: [...disposables].filter((value) => value instanceof THREE.Material).length,
        textures: [...disposables].filter((value) => value instanceof THREE.Texture).length,
      }),
      environment: request.environment,
      environmentDrawables:
        request.environment?.markRecords
          .filter((mark) => mark.status === 0 && mark.enabled)
          .reduce((total, mark) => total + mark.fragments.length, 0) ?? 0,
    })
    disposables.activate()
    return {
      map,
      payload,
      payloadSha256,
      loadRequest: {
        payloadSha256,
        lightStyles: request.lightStyles?.map((value) => Object.freeze({ ...value })),
        directionalTextures: [...directionalInputs.values()],
        modelTextures: request.modelTextures?.map((texture) =>
          Object.freeze({ ...texture, rgba: texture.rgba.slice() }),
        ),
        environment: request.environment,
        diagnostic: request.diagnostic,
      },
      group,
      modelTemplates,
      disposables,
      lightmapTextures,
      exposureUniform,
      diagnostics: Object.freeze(diagnostics),
      worldBatches: Object.freeze(worldBatches),
      result,
      disposed: false,
    }
  }

  async render(frame: Frame): Promise<FrameResult> {
    this.#requireReady()
    if (!this.#active) throw new RenderingError("InvalidState", "renderer has no active map")
    if (this.#renderBusy) throw new RenderingError("InvalidState", "a render is already in progress")
    this.#validateFrame(frame)
    this.#renderBusy = true
    try {
      if (frame.lightStyles && this.#active.map.lighting.profile === "hdr") {
        const lightmap = buildRuntimeLightmap(this.#active.map, frame.lightStyles)
        this.#replaceLightmapData(this.#active, lightmap)
      }
      if (frame.visibility) {
        if (
          frame.visibility.worldIdentity !== this.#active.result.environment?.identity ||
          !HASH.test(frame.visibility.cacheIdentity)
        )
          throw new RenderingError("IdentityMismatch", "visibility result identity differs from the active environment")
        const visible = new Set(frame.visibility.surfaces)
        for (const batch of this.#active.worldBatches)
          batch.mesh.visible = batch.faces.some((face) => visible.has(face))
      }
      if (frame.exposureHistogram) this.#exposure.submit(frame.exposureHistogram)
      const exposure =
        this.configuration.lightingProfile === "hdr"
          ? this.#exposure.advance(frame.deltaSeconds ?? 0)
          : this.#exposure.snapshot()
      this.#active.exposureUniform.value = this.configuration.lightingProfile === "hdr" ? exposure.current : 1
      this.#setCamera(frame.camera)
      this.#stageDynamicItems(frame)
      if (!this.#suspended) {
        await this.#backend.renderAsync(this.#scene, this.#camera)
        this.#submission += 1
      }
      const capture = frame.capture ? await this.#capture(frame.capture) : undefined
      return Object.freeze({
        deviceGeneration: this.#deviceGeneration,
        sceneGeneration: this.#sceneGeneration,
        submission: this.#submission,
        exposure,
        capture,
      })
    } finally {
      this.#renderBusy = false
    }
  }

  #replaceLightmapData(scene: SceneResources, lightmap: RuntimeLightmap): void {
    const planes = [lightmap.flat, ...(lightmap.directional ?? [])]
    if (planes.length !== scene.lightmapTextures.filter(Boolean).length) {
      throw new RenderingError("MalformedInput", "lightmap plane count changed")
    }
    for (let index = 0; index < planes.length; index += 1) {
      const texture = scene.lightmapTextures[index]!
      const image = texture.image as { data: Float32Array; width: number; height: number }
      image.data = planes[index]!
      image.width = lightmap.width
      image.height = lightmap.height
      texture.needsUpdate = true
    }
  }

  #validateFrame(frame: Frame): void {
    if (
      frame.effects.length + (frame.models?.length ?? 0) > MAX_EFFECTS ||
      !finite([
        ...frame.camera.position,
        frame.camera.yawDegrees,
        frame.camera.pitchDegrees,
        frame.camera.verticalFovDegrees,
        frame.camera.near,
        frame.camera.far,
        frame.deltaSeconds ?? 0,
      ]) ||
      frame.camera.verticalFovDegrees <= 0 ||
      frame.camera.verticalFovDegrees >= 180 ||
      frame.camera.near <= 0 ||
      frame.camera.far <= frame.camera.near ||
      (frame.deltaSeconds ?? 0) < 0 ||
      (frame.exposureHistogram && frame.exposureHistogram.length !== 16)
    ) {
      throw new RenderingError("MalformedInput", "render frame is invalid")
    }
    for (const effect of frame.effects) {
      if (
        !Number.isSafeInteger(effect.identity) ||
        effect.identity < 1 ||
        !Number.isSafeInteger(effect.color) ||
        effect.color < 0 ||
        effect.color > 0xff_ffff ||
        !finite([...effect.position, effect.radius, effect.opacity]) ||
        effect.radius < 0 ||
        effect.opacity < 0 ||
        effect.opacity > 1
      )
        throw new RenderingError("MalformedInput", "render effect is invalid")
    }
    for (const item of frame.models ?? []) {
      const transform = item.angles ?? item.orientation ?? []
      if (
        !Number.isSafeInteger(item.identity) ||
        item.identity < 1 ||
        !finite([...item.position, ...transform, item.scale]) ||
        (item.angles === undefined) === (item.orientation === undefined) ||
        (item.orientation !== undefined &&
          (item.orientation.length !== 4 || Math.abs(item.orientation.reduce((s, v) => s + v * v, 0) - 1) > 1e-4)) ||
        !Number.isSafeInteger(item.skin ?? 0) ||
        (item.skin ?? 0) < 0 ||
        item.scale <= 0
      )
        throw new RenderingError("MalformedInput", "render model item is invalid")
      if (!this.#active!.modelTemplates.has(modelKey(item.model, item.skin ?? 0))) {
        throw new RenderingError("MissingInput", `runtime model ${item.model} skin ${item.skin ?? 0} is unavailable`)
      }
    }
  }

  #setCamera(input: Camera): void {
    this.#camera.position.set(...input.position)
    this.#camera.fov = input.verticalFovDegrees
    this.#camera.near = input.near
    this.#camera.far = input.far
    this.#camera.updateProjectionMatrix()
    const yaw = THREE.MathUtils.degToRad(input.yawDegrees)
    const pitch = THREE.MathUtils.degToRad(input.pitchDegrees)
    const direction = new THREE.Vector3(
      Math.cos(pitch) * Math.cos(yaw),
      Math.cos(pitch) * Math.sin(yaw),
      -Math.sin(pitch),
    )
    this.#camera.lookAt(this.#camera.position.clone().add(direction))
  }

  #clearDynamic(group: THREE.Group): void {
    for (const child of [...group.children]) {
      group.remove(child)
      if (child instanceof THREE.Mesh && child.userData.dynamicMaterial === true) child.material.dispose()
    }
  }

  #stageDynamicItems(frame: Frame): void {
    const effects = new THREE.Group()
    const viewModels = new THREE.Group()
    try {
      for (const effect of frame.effects) {
        const material = new THREE.MeshBasicMaterial({
          color: effect.color,
          transparent: effect.opacity < 1,
          opacity: effect.opacity,
          toneMapped: false,
        })
        const mesh = new THREE.Mesh(this.#effectGeometry, material)
        mesh.userData.dynamicMaterial = true
        mesh.position.set(...effect.position)
        mesh.scale.setScalar(effect.radius)
        mesh.userData.identity = effect.identity
        effects.add(mesh)
      }
      for (const item of frame.models ?? []) {
        const instance = this.#active!.modelTemplates.get(modelKey(item.model, item.skin ?? 0))!.clone(true)
        if (item.angles) sourceTransform(instance, item.position, item.angles)
        else {
          instance.position.set(...item.position)
          instance.quaternion.set(...item.orientation!)
        }
        instance.scale.setScalar(item.scale)
        instance.userData.identity = item.identity
        if (item.viewModel) {
          instance.traverse((object) => {
            if (object instanceof THREE.Mesh) {
              object.material = (Array.isArray(object.material) ? object.material : [object.material]).map(
                (material) => {
                  const m = material.clone()
                  m.depthTest = false
                  m.depthWrite = false
                  return m
                },
              )
              if ((object.material as THREE.Material[]).length === 1)
                object.material = (object.material as THREE.Material[])[0]!
            }
          })
          const wrapper = new THREE.Group()
          wrapper.setRotationFromMatrix(new THREE.Matrix4().set(0, -1, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 1))
          wrapper.add(instance)
          viewModels.add(wrapper)
        } else effects.add(instance)
      }
    } catch (error) {
      this.#clearDynamic(effects)
      throw new RenderingError("BoundExceeded", `render item staging failed: ${String(error)}`)
    }
    this.#clearDynamic(this.#effects)
    this.#clearDynamic(this.#viewModels)
    for (const child of [...effects.children]) this.#effects.add(child)
    for (const child of [...viewModels.children]) this.#viewModels.add(child)
  }

  async #capture(request: FrameCaptureRequest): Promise<FrameCapture> {
    try {
      let blob: Blob
      if ("convertToBlob" in this.#canvas) {
        blob = await this.#canvas.convertToBlob({ type: request.format })
      } else {
        const canvas = this.#canvas as HTMLCanvasElement
        blob = await new Promise<Blob>((resolve, reject) =>
          canvas.toBlob(
            (value: Blob | null) => (value ? resolve(value) : reject(new Error("canvas returned no capture"))),
            request.format,
          ),
        )
      }
      const bytes = new Uint8Array(await blob.arrayBuffer())
      return Object.freeze({ format: request.format, sha256: await digest(bytes), bytes })
    } catch (error) {
      throw new RenderingError("CaptureFailed", `canvas capture failed: ${String(error)}`)
    }
  }

  resize(cssWidth: number, cssHeight: number, devicePixelRatio: number): ResizeResult {
    this.#requireReady()
    const width = Math.floor(cssWidth * devicePixelRatio)
    const height = Math.floor(cssHeight * devicePixelRatio)
    const deviceLimit = this.#backend.backend.device?.limits.maxTextureDimension2D ?? MAX_DIMENSION
    const limit = Math.min(MAX_DIMENSION, deviceLimit)
    if (
      !finite([cssWidth, cssHeight, devicePixelRatio]) ||
      cssWidth < 0 ||
      cssHeight < 0 ||
      devicePixelRatio <= 0 ||
      !Number.isSafeInteger(width) ||
      !Number.isSafeInteger(height) ||
      width > limit ||
      height > limit
    )
      throw new RenderingError("BoundExceeded", "renderer dimensions are invalid")
    this.#backend.setPixelRatio(devicePixelRatio)
    this.#backend.setSize(cssWidth, cssHeight, false)
    this.#camera.aspect = cssHeight === 0 ? 1 : cssWidth / cssHeight
    this.#camera.updateProjectionMatrix()
    this.#suspended = width === 0 || height === 0
    return Object.freeze({ width, height, suspended: this.#suspended, deviceGeneration: this.#deviceGeneration })
  }

  startFramePacing(callback: FramePacingCallback): void {
    this.#requireReady()
    if (this.#pacingHandle !== undefined || this.#pacingCallback) {
      throw new RenderingError("InvalidState", "frame pacing is already active")
    }
    if (typeof globalThis.requestAnimationFrame !== "function") {
      throw new RenderingError("UnsupportedEnvironment", "animation-frame scheduling is unavailable")
    }
    this.#pacingCallback = callback
    const tick = (timestamp: number) => {
      this.#pacingHandle = undefined
      if (!this.#pacingCallback || this.#lifecycle === "Disposed" || this.#lifecycle === "Failed") return
      if (!this.#pacingBusy && !this.#suspended && this.#lifecycle === "Ready") {
        this.#pacingBusy = true
        Promise.resolve(this.#pacingCallback(timestamp))
          .then((frame) => (frame ? this.render(frame) : undefined))
          .catch(() => undefined)
          .finally(() => {
            this.#pacingBusy = false
          })
      }
      this.#pacingHandle = requestAnimationFrame(tick)
    }
    this.#pacingHandle = requestAnimationFrame(tick)
  }

  stopFramePacing(): void {
    this.#pacingCallback = undefined
    if (this.#pacingHandle !== undefined && typeof globalThis.cancelAnimationFrame === "function") {
      cancelAnimationFrame(this.#pacingHandle)
    }
    this.#pacingHandle = undefined
  }

  async #recover(): Promise<void> {
    if (this.#lifecycle === "Disposed" || this.#lifecycle === "Recovering") return
    this.#lifecycle = "Recovering"
    this.#loadOrdinal += 1
    this.stopFramePacing()
    const active = this.#active
    this.#active = undefined
    this.#world.clear()
    const oldBackend = this.#backend
    try {
      try {
        oldBackend.backend.context?.unconfigure()
      } catch {
        /* device already lost */
      }
      oldBackend.dispose()
      this.#backend = await this.#createBackend()
      this.#deviceGeneration += 1
      if (active) {
        const directional = await validateDirectionalInputs(active.loadRequest.directionalTextures ?? [])
        const map = await parseRuntimeMapVerified(active.payload)
        const rebuiltMap =
          map.lighting.profile === "hdr" && active.loadRequest.lightStyles
            ? Object.freeze({ ...map, lightmap: buildRuntimeLightmap(map, active.loadRequest.lightStyles) })
            : map
        const rebuilt = this.#buildScene(
          rebuiltMap,
          active.payload,
          active.payloadSha256,
          directional,
          active.loadRequest as MapLoadRequest,
          this.#sceneGeneration + 1,
        )
        this.#active = rebuilt
        this.#world.add(rebuilt.group)
        this.#scene.background = active.loadRequest.diagnostic ? new THREE.Color(0x111820) : null
        this.#sceneGeneration += 1
        disposeScene(active)
      }
      this.#lifecycle = "Ready"
    } catch {
      if (active) disposeScene(active)
      this.#lifecycle = "Failed"
    }
  }

  async #retire(scene: SceneResources): Promise<void> {
    try {
      await this.#backend.backend.device?.queue.onSubmittedWorkDone()
    } catch {
      /* loss invalidated the generation */
    }
    disposeScene(scene)
  }

  #checkAbort(signal: AbortSignal | undefined, ordinal: number): void {
    if (signal?.aborted || ordinal !== this.#loadOrdinal) {
      throw new RenderingError("InvalidState", "scene load was cancelled or replaced")
    }
  }

  #requireReady(): void {
    if (this.#lifecycle !== "Ready") {
      throw new RenderingError(
        this.#lifecycle === "Recovering" ? "DeviceLost" : "InvalidState",
        `renderer is ${this.#lifecycle}`,
      )
    }
  }

  async dispose(): Promise<void> {
    if (this.#lifecycle === "Disposed") return
    this.#lifecycle = "Disposed"
    this.#loadOrdinal += 1
    this.stopFramePacing()
    this.#clearDynamic(this.#effects)
    this.#clearDynamic(this.#viewModels)
    const active = this.#active
    this.#active = undefined
    this.#world.clear()
    if (active) await this.#retire(active)
    this.#effectGeometry.dispose()
    try {
      await this.#backend.backend.device?.queue.onSubmittedWorkDone()
    } catch {
      /* device already unavailable */
    }
    this.#backend.dispose()
    try {
      this.#backend.backend.context?.unconfigure()
    } catch {
      /* already unconfigured */
    }
  }
}

export async function createRenderer(request: RendererCreateRequest): Promise<Renderer> {
  const owner = await new RendererOwner(request).initialize()
  return owner
}
