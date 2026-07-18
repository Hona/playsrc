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
import { sourceHorizontal4By3FovToVertical, sourceViewportDepthRange } from "./source-camera"
import {
  buildRuntimeLightmap,
  parseRuntimeMap,
  parseRuntimeMapVerified,
  type ProfileRequirement,
  type RuntimeBatch,
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
export { sourceHorizontal4By3FovToVertical, sourceViewportDepthRange } from "./source-camera"
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
  pose?: Readonly<{
    primitives: readonly Readonly<{
      primitive: number
      material: number
      positions: Float32Array
      normals: Float32Array
      tangents: Float32Array
      translucent:boolean
    }>[]
  }>
  viewModelProjection?: Readonly<{
    kind: "viewmodel"
    horizontalFov4By3: number
    near: number
    depthRange: readonly [number, number]
    drawsAfterWorld: boolean
    opaqueBeforeTranslucent: boolean
    optionalViewSpaceYReflection: boolean
  }>
}>

export type ParticleItem = Readonly<{
  identity: number
  primitive: "sprite" | "trail"
  material: string
  position: readonly [number, number, number]
  previousPosition: readonly [number, number, number]
  trailEndPosition:readonly[number,number,number]
  radius: number
  rollRadians: number
  yawRadians:number
  color: number
  opacity: number
  trailLength: number
  trailWidth:number
  trailLengthScale:number
  ageSeconds: number
  trailMinLength: number
  trailMaxLength: number
  trailFadeInSeconds: number
  orientationType: number
  materialShader:"sprite-card"|"mesh-sprite"
  textureColorSpace:"srgb-texture-linear-tint"
  blendSource:"zero"|"one"|"source-alpha"|"one-minus-source-alpha"
  blendDestination:"zero"|"one"|"source-alpha"|"one-minus-source-alpha"
  stableTieIdentity:bigint
  primarySheet: Readonly<{
    current: readonly (readonly [number, number, number, number])[]
    next: readonly (readonly [number, number, number, number])[]
    blend: number
  }> | null
  secondarySheet:Readonly<{current:readonly(readonly[number,number,number,number])[];next:readonly(readonly[number,number,number,number])[];blend:number}>|null
}>

export type FrameCaptureRequest = Readonly<{ format: "image/png" }>

export type Frame = Readonly<{
  camera: Camera
  effects: readonly Effect[]
  particles?: readonly ParticleItem[]
  models?: readonly ModelItem[]
  lightStyles?: readonly Readonly<{ style: number; scalar: number }>[]
  exposureHistogram?: Uint32Array
  deltaSeconds?: number
  capture?: FrameCaptureRequest
  visibility?: Readonly<{ worldIdentity: string; cacheIdentity: string; surfaces: Uint32Array }>
  brushModels?:Readonly<{sourceIdentity:bigint;registryIdentity:bigint;tick:bigint;entityRevision:bigint;collisionRevision:bigint;models:readonly Readonly<{sourceIndex:number;model:number;worldPosition:readonly[number,number,number];worldAngles:readonly[number,number,number];renderMode:number;color:readonly[number,number,number,number];renderFx:number;effects:number;draw:boolean;mover:unknown}>[]}>
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

export type MaterialStateInput = Readonly<{
  lighting: number
  blendEnabled: boolean
  blendSource: number
  blendDestination: number
  alphaTest: boolean
  cull: number
  depthTest: boolean
  depthWrite: boolean
  depthFunction: number
  polygonOffset: number
  wireframe: boolean
  noDraw: boolean
  wrapS: number
  wrapT: number
  wrapU: number
  minFilter: number
  magFilter: number
  mipmapped: boolean
  noLod: boolean
  allMips: boolean
  samplingAvailable: boolean
  alphaTestReference: number
}>
export type ModelMaterialInput = Readonly<{
  identity: string
  shader: "vertex-lit-generic" | "eye-refract" | "eyes"
  vertexRequirements: number
  bindings: readonly Readonly<{
    kind: "material" | "model"
    role: number
    colorRead: "srgb" | "linear" | "format-dependent"
    logicalPath: string
  }>[]
  environmentMap: unknown
  state: Readonly<{ kind: "vertex-lit-generic" | "eye-refract" | "eyes" }>
}>
export type AuthoredTextureInput = Readonly<{
  logicalPath: string
  sourceSha256: string
  width: number
  height: number
  depth: number
  mipCount: number
  frameCount: number
  faces: readonly number[]
  scalarEncoding: "u8" | "f16"
  sampling: Readonly<{
    wrapS: number
    wrapT: number
    wrapU: number
    minFilter: number
    magFilter: number
    anisotropyLevel: number
    mipmapped: boolean
    noLod: boolean
    allMips: boolean
  }>
  planes: readonly Readonly<{
    mip: number
    frame: number
    face: number
    slice: number
    width: number
    height: number
    rgba: Uint8Array
  }>[]
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
  materialStates?: ReadonlyMap<string, MaterialStateInput>
  particleTextures?: readonly EnvironmentTextureInput[]
  modelOccurrences?: readonly Readonly<{ entity: number; model: string; matrix: Float32Array }>[]
  modelMaterials?: ReadonlyMap<string, ModelMaterialInput>
  authoredTextures?: ReadonlyMap<string, AuthoredTextureInput>
  brushModels?:readonly Readonly<{index:number;surfaceRange:readonly[number,number];vertexCount:number;triangleCount:number;materials:readonly number[]}>[]
  diagnostic?: boolean
  signal?: AbortSignal
}>

export type SceneDiagnostic = Readonly<{
  code: "MissingMaterial" | "MissingTextureMips" | "MissingModelLighting" | "MissingModelEyeState" | "MissingDirectionalInput" | "MissingProfileInput" | "UnsupportedProfileInput"
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
  visibleProjectedMarks: number
  viewModelPass?: Readonly<{
    depthRange: readonly [number, number]
    viewportRestored: boolean
  }>
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
  brushModelTemplates:Map<number,THREE.Group>
  particleTextures: Map<string, THREE.DataTexture>
  particleMaterials: Map<string, THREE.MeshBasicNodeMaterial>
  materialStates: ReadonlyMap<string, MaterialStateInput>
  disposables: OwnedResourceGeneration
  lightmapTextures: readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?]
  exposureUniform: ReturnType<typeof TSL.uniform>
  diagnostics: readonly SceneDiagnostic[]
  worldBatches: readonly { mesh: THREE.Mesh; faces: Uint32Array }[]
  projectedMarks: readonly { mesh: THREE.Mesh; face: number }[]
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

function modelOccurrenceMatrices(
  map: RuntimeMap,
  input: MapLoadRequest["modelOccurrences"],
): ReadonlyMap<number, NonNullable<MapLoadRequest["modelOccurrences"]>[number]> {
  const supplied = input ?? []
  if (supplied.length !== map.modelOccurrences.length) {
    throw new RenderingError("MissingInput", "exact model occurrence matrices are incomplete")
  }
  const expected = new Map(map.modelOccurrences.map((occurrence) => [occurrence.entity, occurrence]))
  const matrices = new Map<number, NonNullable<MapLoadRequest["modelOccurrences"]>[number]>()
  for (const occurrence of supplied) {
    const target = expected.get(occurrence.entity)
    if (
      !target ||
      matrices.has(occurrence.entity) ||
      occurrence.model !== map.models[target.model]?.logicalPath ||
      occurrence.matrix.length !== 12 ||
      ![...occurrence.matrix].every(Number.isFinite)
    ) {
      throw new RenderingError("IdentityMismatch", "model occurrence matrix identity differs")
    }
    matrices.set(occurrence.entity, occurrence)
  }
  return matrices
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
  state?: Pick<MaterialStateInput, "wrapS" | "wrapT" | "minFilter" | "magFilter">,
): THREE.DataTexture {
  const texture = new THREE.DataTexture(input.rgba, input.width, input.height, THREE.RGBAFormat, THREE.UnsignedByteType)
  texture.colorSpace = colorSpace
  const wrap = (value: number) => value === 0 ? THREE.RepeatWrapping : THREE.ClampToEdgeWrapping
  texture.wrapS = wrap(state?.wrapS ?? 0)
  texture.wrapT = wrap(state?.wrapT ?? 0)
  texture.minFilter = (state?.minFilter ?? 2) === 0 ? THREE.NearestFilter : THREE.LinearFilter
  texture.magFilter = (state?.magFilter ?? 1) === 0 ? THREE.NearestFilter : THREE.LinearFilter
  texture.generateMipmaps = false
  texture.flipY = false
  texture.needsUpdate = true
  return texture
}

function textureFromAuthored(input: AuthoredTextureInput, colorSpace: string): THREE.DataTexture {
  if (input.depth !== 1 || input.frameCount < 1 || !input.faces.includes(0) ||
    input.sampling.wrapS === 2 || input.sampling.wrapT === 2 || input.sampling.wrapU === 2) {
    throw new RenderingError("UnsupportedFeature", `authored 2D texture ${input.logicalPath} requires an unsupported topology or border sampler`)
  }
  const planes = input.planes
    .filter((plane) => plane.frame === 0 && plane.face === 0 && plane.slice === 0)
    .sort((left, right) => left.mip - right.mip)
  if (planes.length !== input.mipCount || planes.some((plane, mip) => plane.mip !== mip)) {
    throw new RenderingError("MissingInput", `authored texture ${input.logicalPath} has an incomplete selected mip chain`)
  }
  const data = (plane: AuthoredTextureInput["planes"][number]) => input.scalarEncoding === "u8"
    ? plane.rgba.slice()
    : new Uint16Array(plane.rgba.slice().buffer)
  const base = planes[0]!
  const texture = new THREE.DataTexture(
    data(base),
    base.width,
    base.height,
    THREE.RGBAFormat,
    input.scalarEncoding === "u8" ? THREE.UnsignedByteType : THREE.HalfFloatType,
  )
  texture.mipmaps = planes.map((plane) => Object.freeze({ data: data(plane), width: plane.width, height: plane.height }))
  texture.colorSpace = colorSpace
  const wrap = (value: number) => value === 0 ? THREE.RepeatWrapping : THREE.ClampToEdgeWrapping
  texture.wrapS = wrap(input.sampling.wrapS)
  texture.wrapT = wrap(input.sampling.wrapT)
  texture.minFilter = [
    THREE.NearestFilter,
    THREE.LinearFilter,
    THREE.LinearMipmapNearestFilter,
    THREE.LinearMipmapLinearFilter,
    THREE.LinearMipmapLinearFilter,
  ][input.sampling.minFilter] ?? THREE.LinearFilter
  texture.magFilter = input.sampling.magFilter === 0 ? THREE.NearestFilter : THREE.LinearFilter
  texture.anisotropy = input.sampling.anisotropyLevel
  texture.generateMipmaps = false
  texture.flipY = false
  texture.needsUpdate = true
  return texture
}

function textureFromLightmap(lightmap: RuntimeLightmap, plane: Float32Array): THREE.DataTexture {
  const texture = new THREE.DataTexture(plane, lightmap.width, lightmap.height, THREE.RGBAFormat, THREE.FloatType)
  configureWorldLightmap(texture, lightmap.profile)
  return texture
}

function materialOptions(resolved: RuntimeMaterial, state?: MaterialStateInput): THREE.MeshBasicMaterialParameters {
  const blendFactor = (value: number) => [THREE.ZeroFactor, THREE.OneFactor, THREE.SrcAlphaFactor, THREE.OneMinusSrcAlphaFactor][value] ?? THREE.OneFactor
  return {
    transparent: state?.blendEnabled ?? (resolved.features & 1) !== 0,
    blending: state?.blendEnabled ? THREE.CustomBlending : THREE.NormalBlending,
    blendSrc: state ? blendFactor(state.blendSource) : undefined,
    blendDst: state ? blendFactor(state.blendDestination) : undefined,
    alphaTest: state?.alphaTest ? state.alphaTestReference : (resolved.features & 4) !== 0 ? 0.7 : 0,
    side: state?.cull === 1 ? THREE.DoubleSide : worldMaterialSide(resolved.features),
    depthTest: state?.depthTest ?? true,
    depthWrite: state?.depthWrite ?? true,
    depthFunc: state?.depthFunction === 0 ? THREE.LessDepth : THREE.LessEqualDepth,
    polygonOffset: state?.polygonOffset === 1,
    polygonOffsetFactor: state?.polygonOffset === 1 ? -0.5 : 0,
    polygonOffsetUnits: state?.polygonOffset === 1 ? -262_144 : 0,
    wireframe: state?.wireframe ?? false,
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
  state?: MaterialStateInput,
): THREE.MeshBasicNodeMaterial {
  const material = new THREE.MeshBasicNodeMaterial(materialOptions(resolved, state))
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
  #viewCamera = new THREE.PerspectiveCamera(41.114, 1, 1, 32_768)
  #viewModelInstances = new Map<number, { model: string; root: THREE.Group; instance: THREE.Group; meshes: THREE.Mesh[] }>()
  #effectGeometry = new THREE.SphereGeometry(1, 10, 6)
  #active?: SceneResources
  #renderBusy = false
  #loadOrdinal = 0
  #suspended = false
  #viewportWidth = 0
  #viewportHeight = 0
  #pacingHandle?: number
  #pacingCallback?: FramePacingCallback
  #pacingBusy = false

  constructor(request: RendererCreateRequest) {
    this.configuration = validateRenderConfiguration(request.configuration)
    this.#canvas = request.canvas
    this.#powerPreference = request.powerPreference
    this.#exposure = new ExposureController(this.configuration.exposure)
    this.#scene.background = null
    this.#scene.add(this.#world, this.#effects, this.#camera, this.#viewCamera)
    this.#viewCamera.add(this.#viewModels)
    this.#viewCamera.layers.set(1)
    this.#viewModels.layers.set(1)
    this.#camera.up.set(0, 0, 1)
    this.#viewCamera.up.set(0, 0, 1)
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
      this.#camera.coordinateSystem = backend.coordinateSystem
      this.#viewCamera.coordinateSystem = backend.coordinateSystem
      this.#camera.updateProjectionMatrix()
      this.#viewCamera.updateProjectionMatrix()
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
    if(!request.brushModels||request.brushModels.length<1)throw new RenderingError("MissingInput","complete brush-model descriptors are required")
    for(let index=0;index<request.brushModels.length;index++){const descriptor=request.brushModels[index]!,geometry=map.brushModels.find(model=>model.index===index);if(descriptor.index!==index||descriptor.surfaceRange[1]<descriptor.surfaceRange[0]||(geometry&&geometry.batches.some(batch=>!descriptor.materials.includes(batch.material))))throw new RenderingError("IdentityMismatch","brush-model geometry differs from its descriptor")}
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
    for (const texture of request.particleTextures ?? []) {
      if (texture.width * texture.height * 4 !== texture.rgba.byteLength || (await digest(texture.rgba)) !== texture.sha256)
        throw new RenderingError("MalformedInput", "particle texture input is invalid")
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
    const materialStates = new Map<string, MaterialStateInput>()
    for (const [identity, state] of request.materialStates ?? []) {
      const key = identity.toLowerCase()
      if (
        !key ||
        materialStates.has(key) ||
        !Number.isFinite(state.alphaTestReference) ||
        typeof state.samplingAvailable !== "boolean" ||
        typeof state.mipmapped !== "boolean" ||
        typeof state.noLod !== "boolean" ||
        typeof state.allMips !== "boolean" ||
        (state.samplingAvailable && (
          state.wrapS < 0 || state.wrapS > 2 ||
          state.wrapT < 0 || state.wrapT > 2 ||
          state.wrapU < 0 || state.wrapU > 2 ||
          state.minFilter < 0 || state.minFilter > 4 ||
          state.magFilter < 0 || state.magFilter > 2
        ))
      ) throw new RenderingError("MalformedInput", "material state input is invalid")
      materialStates.set(key, Object.freeze({ ...state }))
    }
    const authoredTextures = new Map<string, AuthoredTextureInput>()
    for (const [identity, texture] of request.authoredTextures ?? []) {
      const key = identity.toLowerCase()
      if (key !== texture.logicalPath.toLowerCase() || authoredTextures.has(key) || !HASH.test(texture.sourceSha256) ||
        texture.width < 1 || texture.height < 1 || texture.depth < 1 || texture.mipCount < 1 || texture.frameCount < 1 ||
        texture.faces.length < 1 || new Set(texture.faces).size !== texture.faces.length ||
        texture.sampling.wrapS < 0 || texture.sampling.wrapS > 2 || texture.sampling.wrapT < 0 || texture.sampling.wrapT > 2 ||
        texture.sampling.wrapU < 0 || texture.sampling.wrapU > 2 || texture.sampling.minFilter < 0 || texture.sampling.minFilter > 4 ||
        texture.sampling.magFilter < 0 || texture.sampling.magFilter > 2 || texture.sampling.anisotropyLevel < 1 ||
        texture.planes.some((plane) => plane.width < 1 || plane.height < 1 ||
          plane.rgba.length !== plane.width * plane.height * 4 * (texture.scalarEncoding === "u8" ? 1 : 2))) {
        throw new RenderingError("MalformedInput", "authored model texture input is invalid")
      }
      authoredTextures.set(key, texture)
    }
    const modelMaterials = new Map<string, ModelMaterialInput>()
    for (const [identity, material] of request.modelMaterials ?? []) {
      const key = identity.toLowerCase()
      if (key !== material.identity.toLowerCase() || modelMaterials.has(key) || material.vertexRequirements < 0 ||
        material.bindings.some((binding) => !authoredTextures.has(binding.logicalPath.toLowerCase()))) {
        throw new RenderingError("MalformedInput", "model material input is invalid")
      }
      modelMaterials.set(key, material)
    }
    const materialIdentities = new Set([
      ...map.materials.map((material) => material.logicalPath.toLowerCase()),
      ...map.models.flatMap((model) => model.materials.map((material) => material.logicalPath.toLowerCase())),
    ])
    if ([...directionalInputs.keys()].some((identity) => !materialIdentities.has(identity))) {
      throw new RenderingError("MalformedInput", "directional texture names an unavailable material")
    }
    this.#checkAbort(request.signal, ordinal)
    const normalizedRequest = Object.freeze({ ...request, materialStates, authoredTextures, modelMaterials })
    const staged = this.#buildScene(
      map,
      payload,
      request.payloadSha256,
      directionalInputs,
      normalizedRequest,
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
    const brushModelTemplates=new Map<number,THREE.Group>()
    const particleTextures = new Map<string, THREE.DataTexture>()
    const particleMaterials = new Map<string, THREE.MeshBasicNodeMaterial>()
    const materialStates = new Map(request.materialStates ?? [])
    const disposables = new OwnedResourceGeneration(this.#deviceGeneration, sceneGeneration)
    const diagnostics: SceneDiagnostic[] = []
    const worldBatches: { mesh: THREE.Mesh; faces: Uint32Array }[] = []
    const projectedMarks: { mesh: THREE.Mesh; face: number }[] = []
    const occurrenceMatrices = modelOccurrenceMatrices(map, request.modelOccurrences)
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
    const authoredGpu = new Map<string, THREE.DataTexture>()
    let missingModelLighting = false
    let missingModelEyeState = false
    const missingMipInputs = new Set<string>()
    const requireMipInputs = (identity: string, state: MaterialStateInput | undefined): void => {
      const key = identity.toLowerCase()
      if (!state?.samplingAvailable || !state.mipmapped || missingMipInputs.has(key)) return
      missingMipInputs.add(key)
      diagnostics.push(diagnostic("MissingTextureMips", identity, "the supplied texture contains mip zero only"))
    }
    for (const [identity, input] of directionalInputs) {
      const texture = textureFromRgba(input, THREE.NoColorSpace)
      directionalGpu.set(identity, { input, texture })
      disposables.add(texture)
    }

    const supplemental = new Map(
      (request.modelTextures ?? []).map((texture) => [texture.material.toLowerCase(), texture] as const),
    )
    const createModelBase = (identity: string): THREE.DataTexture | undefined => {
      const material = request.modelMaterials?.get(identity.toLowerCase())
      if (!material) {
        diagnostics.push(diagnostic("MissingMaterial", identity, "typed model material state is unavailable"))
        return undefined
      }
      if ((material.shader === "eye-refract" || material.shader === "eyes") && !missingModelEyeState) {
        missingModelEyeState = true
        diagnostics.push(diagnostic("MissingModelEyeState", "game-eye-target", "current game-owned eye targets and per-draw StudioModel eye states are unavailable"))
      } else if (material.shader === "vertex-lit-generic" && !missingModelLighting) {
        missingModelLighting = true
        diagnostics.push(diagnostic("MissingModelLighting", "model-lightcache", "current model lightcache selections and ModelLightingInput records are unavailable"))
      }
      const binding = material.bindings.find((value) => value.kind === "material" && value.role === 0)
      if (!binding) return undefined
      if (binding.colorRead === "format-dependent") {
        throw new RenderingError("MissingInput", `model texture ${binding.logicalPath} lacks a resolved color interpretation`)
      }
      const key = `${binding.logicalPath.toLowerCase()}:${binding.colorRead}`
      let texture = authoredGpu.get(key)
      if (!texture) {
        const input = request.authoredTextures?.get(binding.logicalPath.toLowerCase())
        if (!input) throw new RenderingError("MissingInput", `authored model texture ${binding.logicalPath} is unavailable`)
        texture = textureFromAuthored(input, binding.colorRead === "srgb" ? THREE.SRGBColorSpace : THREE.NoColorSpace)
        authoredGpu.set(key, texture)
        disposables.add(texture)
      }
      return texture
    }
    const createBase = (resolved: RuntimeMaterial, identity: string): THREE.DataTexture | undefined => {
      const state = materialStates.get(identity.toLowerCase())
      const source = resolved.baseTexture ?? supplemental.get(identity.toLowerCase())
      if (!source) {
        diagnostics.push(diagnostic("MissingMaterial", identity, "resolved base texture is unavailable"))
        return undefined
      }
      requireMipInputs(identity, state)
      const texture = textureFromRgba(source, THREE.SRGBColorSpace, state)
      disposables.add(texture)
      return texture
    }
    const createWorldMesh=(batch:RuntimeBatch):THREE.Mesh|null=>{
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
        const materialState = materialStates.get(identity.toLowerCase())
        if (materialState?.noDraw) return null
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
            materialState,
          )
        } else {
          material = new THREE.MeshBasicMaterial({
            ...materialOptions(resolved, materialState),
            color: baseTexture ? 0xffffff : debugColor(identity),
            map: baseTexture,
            lightMap: lightmapTextures[0],
            lightMapIntensity: 1,
            toneMapped: false,
          })
        }
        disposables.add(material)
        const mesh = new THREE.Mesh(geometry, material)
        mesh.userData.materialIdentity = identity
        return mesh
    }
    try {
      for(const batch of map.batches){const mesh=createWorldMesh(batch);if(!mesh)continue;worldBatches.push({mesh,faces:batch.faces.slice()});group.add(mesh)}
      for(const model of map.brushModels){const template=new THREE.Group();for(const batch of model.batches){const mesh=createWorldMesh(batch);if(mesh)template.add(mesh)}brushModelTemplates.set(model.index,template)}

      const environmentTextures = new Map<string, THREE.DataTexture>()
      for (const texture of request.environment?.textures ?? []) {
        const value = textureFromRgba(texture, THREE.SRGBColorSpace, materialStates.get(texture.material.toLowerCase()))
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
          const state = materialStates.get(mark.material.toLowerCase())
          if (!state) throw new RenderingError("MissingInput", `projected mark state ${mark.material} is unavailable`)
          requireMipInputs(mark.material, state)
          const material = new THREE.MeshBasicMaterial({
            ...materialOptions({ logicalPath: mark.material, width: 1, height: 1, shader: 3, features: 1, textureRole: 0 }, state),
            map: texture,
            toneMapped: false,
          })
          disposables.add(material)
          const mesh = new THREE.Mesh(geometry, material)
          projectedMarks.push({ mesh, face: fragment.face })
          worldBatches.push({ mesh, faces: Uint32Array.of(fragment.face) })
          group.add(mesh)
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
          const materialState = materialStates.get(resolved.logicalPath.toLowerCase())
          if (materialState?.noDraw) continue
          const baseTexture = createModelBase(resolved.logicalPath)
          const material = new THREE.MeshBasicMaterial({
            ...materialOptions(resolved, materialState),
            color: baseTexture ? 0xffffff : debugColor(resolved.logicalPath),
            map: baseTexture,
            toneMapped: false,
          })
          disposables.add(material)
          const mesh = new THREE.Mesh(geometry, material)
          mesh.userData.primitiveMaterial = primitive.material
          template.add(mesh)
        }
        modelTemplates.set(model.logicalPath, template)
      }
      for (const occurrence of map.modelOccurrences) {
        const model = map.models[occurrence.model]!
        const instance = modelTemplates.get(model.logicalPath)!.clone(true)
        instance.userData.entity = occurrence.entity
        const m = occurrenceMatrices.get(occurrence.entity)!.matrix
        instance.matrix.set(m[0]!, m[1]!, m[2]!, m[3]!, m[4]!, m[5]!, m[6]!, m[7]!, m[8]!, m[9]!, m[10]!, m[11]!, 0, 0, 0, 1)
        instance.matrixAutoUpdate = false
        group.add(instance)
      }
      for (const texture of request.particleTextures ?? []) {
        const state = materialStates.get(texture.material.toLowerCase())
        requireMipInputs(texture.material, state)
        const value = textureFromRgba(texture, THREE.SRGBColorSpace, state)
        particleTextures.set(texture.material.toLowerCase(), value)
        disposables.add(value)
        const material = new THREE.MeshBasicNodeMaterial(materialOptions({
          logicalPath: texture.material, width: texture.width, height: texture.height, shader: 7, features: 1, textureRole: 0,
        }, state))
        const current = TSL.texture(value, TSL.uv())
        const next = TSL.texture(value, TSL.attribute("particleUvNext", "vec2"))
        const blend = TSL.attribute("particleSheetBlend", "float")
        const color = TSL.attribute("particleColor", "vec4")
        const sampled = current.mul(TSL.float(1).sub(blend)).add(next.mul(blend))
        material.colorNode = TSL.vec4(sampled.rgb.mul(color.rgb), sampled.a.mul(color.a))
        material.toneMapped = false
        particleMaterials.set(texture.material.toLowerCase(), material)
        disposables.add(material)
      }
    } catch (error) {
      const failed = {
        group,
        modelTemplates,
        brushModelTemplates,
        particleTextures,
        particleMaterials,
        materialStates,
        disposables,
        projectedMarks,
        disposed: false,
      } as SceneResources
      disposeScene(failed)
      if (error instanceof RenderingError) throw error
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
        materialStates: new Map(materialStates),
        particleTextures: request.particleTextures?.map((texture) => Object.freeze({ ...texture, rgba: texture.rgba.slice() })),
        modelOccurrences: request.modelOccurrences?.map((value) => Object.freeze({ ...value, matrix: value.matrix.slice() })),
        modelMaterials: new Map(request.modelMaterials ?? []),
        authoredTextures: new Map([...(request.authoredTextures ?? [])].map(([identity, texture]) => [identity, Object.freeze({
          ...texture,
          faces: Object.freeze([...texture.faces]),
          planes: Object.freeze(texture.planes.map((plane) => Object.freeze({ ...plane, rgba: plane.rgba.slice() }))),
        })])),
        brushModels:request.brushModels?.map(model=>Object.freeze({...model,surfaceRange:Object.freeze([...model.surfaceRange]) as readonly[number,number],materials:Object.freeze([...model.materials])})),
        diagnostic: request.diagnostic,
      },
      group,
      modelTemplates,
      brushModelTemplates,
      particleTextures,
      particleMaterials,
      materialStates,
      disposables,
      lightmapTextures,
      exposureUniform,
      diagnostics: Object.freeze(diagnostics),
      worldBatches: Object.freeze(worldBatches),
      projectedMarks: Object.freeze(projectedMarks),
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
      const viewModelDepthRange = this.#stageDynamicItems(frame)
      let viewModelPass: FrameResult["viewModelPass"]
      if (!this.#suspended) {
        this.#backend.autoClear = true
        await this.#backend.renderAsync(this.#scene, this.#camera)
        if (this.#viewModels.children.length > 0) {
          if (!viewModelDepthRange) throw new RenderingError("InvalidState", "viewmodel depth range is unavailable")
          this.#backend.autoClear = false
          const background = this.#scene.background
          this.#scene.background = null
          this.#backend.setViewport(0, 0, this.#viewportWidth, this.#viewportHeight, viewModelDepthRange[0], viewModelDepthRange[1])
          try {
            await this.#backend.renderAsync(this.#scene, this.#viewCamera)
          } finally {
            this.#backend.setViewport(0, 0, this.#viewportWidth, this.#viewportHeight, 0, 1)
            this.#scene.background = background
            this.#backend.autoClear = true
          }
          viewModelPass = Object.freeze({ depthRange: viewModelDepthRange, viewportRestored: true })
        }
        this.#submission += 1
      }
      const capture = frame.capture ? await this.#capture(frame.capture) : undefined
      return Object.freeze({
        deviceGeneration: this.#deviceGeneration,
        sceneGeneration: this.#sceneGeneration,
        submission: this.#submission,
        exposure,
        visibleProjectedMarks: this.#active.projectedMarks.reduce((total, mark) => total + Number(mark.mesh.visible), 0),
        viewModelPass,
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
      frame.effects.length + (frame.models?.length ?? 0) + (frame.particles?.length ?? 0)+(frame.brushModels?.models.length??0) > MAX_EFFECTS ||
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
      if (item.viewModel && (!item.viewModelProjection || item.viewModelProjection.kind !== "viewmodel"))
        throw new RenderingError("MalformedInput", "viewmodel projection is missing")
      if (item.viewModelProjection) {
        const projection = item.viewModelProjection
        try {
          sourceHorizontal4By3FovToVertical(projection.horizontalFov4By3)
          sourceViewportDepthRange(projection.depthRange)
        } catch {
          throw new RenderingError("MalformedInput", "viewmodel projection is invalid")
        }
        if (
          !Number.isFinite(projection.near) ||
          projection.near <= 0 ||
          !projection.drawsAfterWorld ||
          !projection.opaqueBeforeTranslucent ||
          typeof projection.optionalViewSpaceYReflection !== "boolean"
        ) {
          throw new RenderingError("MalformedInput", "viewmodel projection is invalid")
        }
      }
    }
    for (const item of frame.particles ?? []) {
      if (!Number.isSafeInteger(item.identity) || item.identity < 1 || !finite([
        ...item.position, ...item.previousPosition, item.radius, item.rollRadians, item.opacity, item.trailLength,
        item.ageSeconds, item.trailMinLength, item.trailMaxLength, item.trailFadeInSeconds,
      ]) || item.radius < 0 || item.opacity < 0 || item.opacity > 1 || item.orientationType !== 0 ||
        !this.#active!.particleTextures.has(item.material.toLowerCase()) || !item.primarySheet)
        throw new RenderingError("MalformedInput", "particle draw item is invalid")
    }
    if(frame.brushModels){let prior=-1;for(const model of frame.brushModels.models){if(model.sourceIndex<=prior||model.model<1||model.model>=(this.#active!.loadRequest.brushModels?.length??0)||!finite([...model.worldPosition,...model.worldAngles])||model.renderMode<0||model.renderMode>10)throw new RenderingError("MalformedInput","brush-model publication record is invalid");prior=model.sourceIndex}}
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
    this.#viewCamera.position.copy(this.#camera.position)
    this.#viewCamera.quaternion.copy(this.#camera.quaternion)
  }

  #clearDynamic(group: THREE.Group): void {
    for (const child of [...group.children]) {
      group.remove(child)
      child.traverse((object) => {
        if (!(object instanceof THREE.Mesh)) return
        if (object.userData.dynamicMaterial === true) {
          for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.dispose()
        }
        if (object.userData.dynamicGeometry === true) object.geometry.dispose()
      })
    }
  }

  #particleGeometry(item: ParticleItem, camera: Camera): THREE.BufferGeometry {
    const geometry = new THREE.BufferGeometry()
    const positions = new Float32Array(12)
    const uv = new Float32Array([0, 0, 1, 0, 1, 1, 0, 1])
    if (item.primitive === "trail") {
      const delta = new THREE.Vector3().fromArray(item.trailEndPosition).sub(new THREE.Vector3().fromArray(item.position))
      const center = new THREE.Vector3().fromArray(item.position)
      const tangent = center.clone().sub(new THREE.Vector3().fromArray(camera.position)).cross(delta).normalize()
      const halfWidth = item.trailWidth * 0.5
      const vertices = [center.clone().addScaledVector(tangent, halfWidth), center.clone().addScaledVector(tangent, -halfWidth)]
      vertices.push(vertices[1]!.clone().add(delta), vertices[0]!.clone().add(delta))
      vertices.forEach((value, index) => value.toArray(positions, index * 3))
    } else {
      const yaw = THREE.MathUtils.degToRad(camera.yawDegrees), pitch = THREE.MathUtils.degToRad(camera.pitchDegrees)
      const right = new THREE.Vector3(Math.sin(yaw), -Math.cos(yaw), 0)
      const up = new THREE.Vector3(Math.sin(pitch) * Math.cos(yaw), Math.sin(pitch) * Math.sin(yaw), Math.cos(pitch))
      const cosine = Math.cos(item.rollRadians), sine = Math.sin(item.rollRadians)
      const rolledRight = right.clone().multiplyScalar(cosine).addScaledVector(up, sine)
      const rolledUp = up.clone().multiplyScalar(cosine).addScaledVector(right, -sine)
      const center = new THREE.Vector3().fromArray(item.position)
      const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const
      corners.forEach(([x, y], index) => center.clone().addScaledVector(rolledRight, x * item.radius).addScaledVector(rolledUp, y * item.radius).toArray(positions, index * 3))
    }
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3))
    geometry.setAttribute("uv", new THREE.BufferAttribute(uv, 2))
    geometry.setIndex([0, 1, 2, 0, 2, 3])
    return geometry
  }

  #applyPose(instance: THREE.Group, pose: NonNullable<ModelItem["pose"]>, retainGeometry: boolean): THREE.Mesh[] {
    let primitive = 0
    const meshes: THREE.Mesh[] = []
    instance.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return
      const posed = pose.primitives[primitive++]
      if (!posed || posed.material !== object.userData.primitiveMaterial || posed.positions.length !== object.geometry.getAttribute("position").count * 3 || posed.normals.length !== object.geometry.getAttribute("normal").count * 3)
        throw new RenderingError("IdentityMismatch", "posed model primitive differs from its template")
      if(object.userData.dynamicMaterial!==true){object.material=Array.isArray(object.material)?object.material.map(material=>material.clone()):object.material.clone();object.userData.dynamicMaterial=true}for(const material of Array.isArray(object.material)?object.material:[object.material]){material.transparent=posed.translucent;material.depthWrite=!posed.translucent}
      if (retainGeometry && object.userData.dynamicGeometry === true) {
        const position = object.geometry.getAttribute("position") as THREE.BufferAttribute
        const normal = object.geometry.getAttribute("normal") as THREE.BufferAttribute
        const tangent = object.geometry.getAttribute("tangent") as THREE.BufferAttribute | undefined
        ;(position.array as Float32Array).set(posed.positions); position.needsUpdate = true
        ;(normal.array as Float32Array).set(posed.normals); normal.needsUpdate = true
        if (tangent) { (tangent.array as Float32Array).set(posed.tangents); tangent.needsUpdate = true }
      } else {
        const geometry = object.geometry.clone()
        geometry.setAttribute("position", new THREE.BufferAttribute(posed.positions.slice(), 3))
        geometry.setAttribute("normal", new THREE.BufferAttribute(posed.normals.slice(), 3))
        geometry.setAttribute("tangent", new THREE.BufferAttribute(posed.tangents.slice(), 4))
        object.geometry = geometry
        object.userData.dynamicGeometry = true
      }
      meshes.push(object)
    })
    if (primitive !== pose.primitives.length) throw new RenderingError("IdentityMismatch", "posed model primitive count differs")
    return meshes
  }

  #stageViewModel(item: ModelItem, frame: Frame): readonly [number, number] {
    const key = modelKey(item.model, item.skin ?? 0)
    let retained = this.#viewModelInstances.get(item.identity)
    if (!retained || retained.model !== key) {
      if (retained) {
        this.#viewModels.remove(retained.root)
        retained.root.traverse((object) => {
          if (object instanceof THREE.Mesh && object.userData.dynamicGeometry === true) object.geometry.dispose()
        })
      }
      const instance = this.#active!.modelTemplates.get(key)!.clone(true)
      if (!item.pose) throw new RenderingError("MalformedInput", "viewmodel pose is missing")
      const meshes = this.#applyPose(instance, item.pose, false)
      const root = new THREE.Group()
      root.setRotationFromMatrix(new THREE.Matrix4().set(0, -1, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 1))
      root.add(instance)
      root.traverse((object) => object.layers.set(1))
      this.#viewModels.add(root)
      retained = { model: key, root, instance, meshes }
      this.#viewModelInstances.set(item.identity, retained)
    } else if (item.pose) {
      this.#applyPose(retained.instance, item.pose, true)
    }
    sourceTransform(retained.instance, item.position, item.angles!)
    retained.instance.scale.setScalar(item.scale)
    const projection = item.viewModelProjection!
    this.#viewCamera.fov = sourceHorizontal4By3FovToVertical(projection.horizontalFov4By3)
    this.#viewCamera.near = projection.near
    this.#viewCamera.far = frame.camera.far
    this.#viewCamera.updateProjectionMatrix()
    this.#viewCamera.projectionMatrixInverse.copy(this.#viewCamera.projectionMatrix).invert()
    return sourceViewportDepthRange(projection.depthRange)
  }

  #stageDynamicItems(frame: Frame): readonly [number, number] | undefined {
    const effects = new THREE.Group()
    const activeViewModels = new Set<number>()
    let viewModelDepthRange: readonly [number, number] | undefined
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
      const factor=(value:ParticleItem["blendSource"])=>value==="zero"?THREE.ZeroFactor:value==="one"?THREE.OneFactor:value==="source-alpha"?THREE.SrcAlphaFactor:THREE.OneMinusSrcAlphaFactor
      for(const [order,item] of (frame.particles??[]).entries()){if(item.secondarySheet)throw new RenderingError("UnsupportedFeature","dual-sequence Particle rendering requires exact selected material support");const geometry=this.#particleGeometry(item,frame.camera),sample=item.primarySheet!,current=sample.current[0]!,next=sample.next[0]!,corners=[[current[0],current[1]],[current[2],current[1]],[current[2],current[3]],[current[0],current[3]]],nextCorners=[[next[0],next[1]],[next[2],next[1]],[next[2],next[3]],[next[0],next[3]]];geometry.setAttribute("uv",new THREE.BufferAttribute(Float32Array.from(corners.flat()),2));geometry.setAttribute("particleUvNext",new THREE.BufferAttribute(Float32Array.from(nextCorners.flat()),2));geometry.setAttribute("particleSheetBlend",new THREE.BufferAttribute(Float32Array.from({length:4},()=>sample.blend),1));const red=((item.color>>16)&255)/255,green=((item.color>>8)&255)/255,blue=(item.color&255)/255;geometry.setAttribute("particleColor",new THREE.BufferAttribute(Float32Array.from({length:16},(_,i)=>[red,green,blue,item.opacity][i%4]!),4));const material=this.#active!.particleMaterials.get(item.material.toLowerCase())!.clone();material.blending=THREE.CustomBlending;material.blendSrc=factor(item.blendSource);material.blendDst=factor(item.blendDestination);material.transparent=true;const mesh=new THREE.Mesh(geometry,material);mesh.renderOrder=order;mesh.userData.dynamicGeometry=true;mesh.userData.dynamicMaterial=true;mesh.userData.stableTieIdentity=item.stableTieIdentity;effects.add(mesh)}
      for(const item of frame.brushModels?.models??[]){if(!item.draw)continue;const template=this.#active!.brushModelTemplates.get(item.model);if(!template)continue;const instance=template.clone(true);sourceTransform(instance,item.worldPosition,item.worldAngles);instance.userData.identity=item.sourceIndex;instance.userData.renderMode=item.renderMode;instance.userData.renderFx=item.renderFx;instance.userData.effects=item.effects;instance.userData.mover=item.mover;const modulation=new THREE.Color(item.color[0]/255,item.color[1]/255,item.color[2]/255);instance.traverse(object=>{if(!(object instanceof THREE.Mesh))return;const original=object.material,materials=(Array.isArray(original)?original:[original]).map(value=>{const material=value.clone();if("color" in material&&material.color instanceof THREE.Color)material.color.multiply(modulation);material.opacity*=item.color[3]/255;material.transparent=material.transparent||item.renderMode!==0||item.color[3]<255;return material});object.material=Array.isArray(original)?materials:materials[0]!;object.userData.dynamicMaterial=true});effects.add(instance)}
      for (const item of frame.models ?? []) {
        if (item.viewModel) {
          const nextDepthRange = this.#stageViewModel(item, frame)
          if (viewModelDepthRange && (viewModelDepthRange[0] !== nextDepthRange[0] || viewModelDepthRange[1] !== nextDepthRange[1])) {
            throw new RenderingError("IdentityMismatch", "viewmodel depth ranges differ in one pass")
          }
          viewModelDepthRange = nextDepthRange
          activeViewModels.add(item.identity)
          continue
        }
        const instance = this.#active!.modelTemplates.get(modelKey(item.model, item.skin ?? 0))!.clone(true)
        if (item.pose) {
          this.#applyPose(instance, item.pose, false)
        }
        if (item.angles) sourceTransform(instance, item.position, item.angles)
        else {
          instance.position.set(...item.position)
          instance.quaternion.set(...item.orientation!)
        }
        instance.scale.setScalar(item.scale)
        instance.userData.identity = item.identity
        effects.add(instance)
      }
    } catch (error) {
      this.#clearDynamic(effects)
      if (error instanceof RenderingError) throw error
      throw new RenderingError("BoundExceeded", `render item staging failed: ${String(error)}`)
    }
    this.#clearDynamic(this.#effects)
    for (const child of [...effects.children]) this.#effects.add(child)
    for (const [identity, retained] of this.#viewModelInstances) {
      if (activeViewModels.has(identity)) continue
      this.#viewModels.remove(retained.root)
      retained.root.traverse((object) => {
        if (object instanceof THREE.Mesh && object.userData.dynamicGeometry === true) object.geometry.dispose()
      })
      this.#viewModelInstances.delete(identity)
    }
    return viewModelDepthRange
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
    this.#viewportWidth = cssWidth
    this.#viewportHeight = cssHeight
    this.#camera.aspect = cssHeight === 0 ? 1 : cssWidth / cssHeight
    this.#viewCamera.aspect = this.#camera.aspect
    this.#camera.updateProjectionMatrix()
    this.#viewCamera.updateProjectionMatrix()
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
    this.#viewModelInstances.clear()
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
