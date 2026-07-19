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
import { configureWorldLightmap, sourceDepthBias, worldMaterialSide } from "./material-state"
import { OwnedResourceGeneration } from "./resource-generation"
import { FramePacingController, type FramePacingRecord } from "./frame-pacing"
import { particleBatchRanges } from "./particle-batches"
import { selectDiagnosticModelBase } from "./diagnostic-model"
import { sourceHorizontal4By3FovToVertical, sourceViewportDepthRange } from "./source-camera"
import {
  validateDynamicLights,
  validateShadows,
  visibleTriangleIndices,
  type DynamicLightInput,
  type ShadowInput,
} from "./frame-foundations"
import {
  prepareModelDrawInputs,
  validateModelEyeStates,
  validateModelLightingInput,
  type ModelDrawRequirement,
  type ModelEyeState,
  type ModelLightingInput,
} from "./model-lighting"
import { buildSourceSkyGeometry } from "./source-environment"
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
  ModelLightingError,
  ambientCubeLuminance,
  dilateIrisUv,
  evaluateAmbientCube,
  evaluateLocalLight,
  evaluateModelLighting,
  intersectEyeSphere,
  projectEyeCoordinate,
  prepareModelDrawInputs,
  validateModelEyeStates,
  validateModelLightingInput,
  type ModelEyeState,
  type ModelDrawRequirement,
  type ModelLightingInput,
  type ModelLocalLight,
  type SourceVector3,
  type SourceVector4,
} from "./model-lighting"
export {
  SourceEnvironmentError,
  buildSourceSkyGeometry,
  decodeRgbsBilinear,
  evaluateCheapWater,
  evaluateExpensiveWater,
  evaluateFogColor,
  linearFogFactor,
  sampleAuthoredCubemap,
  selectNearestCubemap,
  sourceCubemapCoordinate,
  sourceSkyFaceVisible,
  waterBlurOffsets,
  type AuthoredCubemap,
  type CheapWaterRequest,
  type CubemapSample,
  type ExpensiveWaterRequest,
  type FogState,
  type SkyFace,
  type SkyFaceGeometry,
} from "./source-environment"
export {
  FRAME_PHASES,
  FrameContractError,
  SourceViewStack,
  executeFrameGraph,
  interpolatePresentation,
  orderTransparentItems,
  validateDynamicLights,
  validateShadows,
  validateViewState,
  visibleTriangleIndices,
  type ClipPlane,
  type DepthStencilState,
  type DynamicLightInput,
  type FrameGraphResult,
  type FramePhase,
  type InterpolationField,
  type InterpolationPolicy,
  type InterpolationRequest,
  type ShadowInput,
  type TransparentItem,
  type TransparentOperation,
  type ViewState,
} from "./frame-foundations"
export { FramePacingController, FramePacingError, type FramePacingRecord } from "./frame-pacing"
export { selectDiagnosticModelBase, type DiagnosticModelBaseDisposition } from "./diagnostic-model"
export { AtomicResourceError, AtomicResourceSlot, type ReplaceableResource } from "./atomic-resources"
export {
  DebugCaptureError,
  compareDebugPlanes,
  createDebugCapture,
  validateAlignedCaptureManifest,
  type AlignedCaptureManifest,
  type DebugCapture,
  type DebugPlane,
  type DebugPlaneInput,
  type DebugPlaneKind,
  type PlaneComparison,
  type PlaneComparisonResult,
} from "./debug-capture"
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

export type Effect = DynamicLightInput

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
    viewmodel?: null | Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back"; reflected: boolean; drawDisposition: "draw" | "suppressed-success" | "suppressed" }>
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
  modelLighting?: ModelLightingInput
  eyeStates?: readonly ModelEyeState[]
  currentFramebufferAvailable?: boolean
  gameProxyValuesAvailable?: boolean
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
export type WaterFramePass = Readonly<{
  kind: "reflection" | "refraction" | "main" | "intersection"
  origin: readonly [number, number, number]
  angles: readonly [number, number, number]
  renderAboveWater: boolean
  renderUnderWater: boolean
  renderWaterSurface: boolean
  drawEntities: boolean
  drawSky2d: boolean
  clip: null | Readonly<{ height: number; keep: "above" | "below" }>
  forcedVisibilityLeaf: number | null
  fog: Readonly<{ kind: "world" }> | Readonly<{ kind: "water"; volume: number; heightFog: boolean }>
  surfaces: Uint32Array
}>
export type WaterFramePlan = Readonly<{
  visibleWater: null | Readonly<{
    volume: number
    visibleLeaf: number
    eyeLeaf: number
    eyeInVolume: boolean
    surfaceZ: number
    distanceToWater: number | null
    material: string
    translucent: boolean
    evaluated: Readonly<{ normalFrame: number; normalTransform: Float32Array; cheapStart: number; cheapEnd: number }>
  }>
  render: Readonly<{ cheap: boolean; reflect: boolean; refract: boolean; reflectEntities: boolean; drawSurface: boolean; opaque: boolean }>
  nearPlaneIntersects: boolean
  passes: readonly WaterFramePass[]
}>

export type Frame = Readonly<{
  camera: Camera
  effects: readonly Effect[]
  shadows?: readonly ShadowInput[]
  particles?: readonly ParticleItem[]
  models?: readonly ModelItem[]
  lightStyles?: readonly Readonly<{ style: number; scalar: number }>[]
  exposureHistogram?: Uint32Array
  deltaSeconds?: number
  capture?: FrameCaptureRequest
  visibility?: Readonly<{
    worldIdentity: string
    cacheIdentity: string
    outsideWorld: boolean
    sky: 0 | 1 | 2
    eyeLeaf: number | null
    leaves: readonly number[]
    areas: readonly number[]
    surfaces: Uint32Array
    water: WaterFramePlan
  }>
  brushModels?:Readonly<{sourceIdentity:bigint;registryIdentity:bigint;tick:bigint;entityRevision:bigint;collisionRevision:bigint;models:readonly Readonly<{sourceIndex:number;model:number;worldPosition:readonly[number,number,number];worldAngles:readonly[number,number,number];renderMode:number;color:readonly[number,number,number,number];renderFx:number;effects:number;draw:boolean;mover:unknown}>[]}>
  collisionWorldIdentity?: string
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
  lightmapUv: Float32Array
  visibility: Readonly<
    | { kind: "world"; leaves: readonly number[]; clusters: readonly number[]; areas: readonly number[] }
    | { kind: "brush-model"; entity: bigint; model: number }
  >
}>
type EffectiveInput<T> = Readonly<{ value: T; origin: "authored" | "shader-initializer" | "type-initializer" }>
export type WaterMaterialInput = Readonly<{
  identity:string
  mapMaterial:number|null
  opacity:"opaque"|"translucent"
  textures:readonly Readonly<{role:number;disposition:"source"|"environment"|"render-target";logicalPath:string|null}>[]
  normalFrame:EffectiveInput<number>
  normalTransform:Readonly<{matrix:Float32Array;proxyMutated:boolean}>
  aboveWater:EffectiveInput<boolean>
  reflectAmount:EffectiveInput<number>
  refractAmount:EffectiveInput<number>
  reflectTint:EffectiveInput<readonly[number,number,number]>
  refractTint:EffectiveInput<readonly[number,number,number]>
  reflectionBlendFactor:EffectiveInput<number>
  fog:Readonly<{enabled:EffectiveInput<boolean>|null;color:EffectiveInput<readonly[number,number,number]>;start:EffectiveInput<number>;end:EffectiveInput<number>}>
  forceCheap:EffectiveInput<boolean>
  forceExpensive:EffectiveInput<boolean>
  reflectEntities:EffectiveInput<boolean>
  blurRefraction:EffectiveInput<boolean>
  fresnel:Readonly<{cheapEnabled:boolean;expensiveConstant:readonly[number,number,number,number]}>
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
    sourceIndex: number
    receiver: null | Readonly<{ entity: bigint | null; model: number; parentEntity: number | null; localOrigin: readonly [number, number, number]; origin: readonly [number, number, number]; angles: readonly [number, number, number] }>
    normalOffset: number
    polygonOffset: "none" | "decal"
  }>[]
  textures: readonly EnvironmentTextureInput[]
  collisionWorldIdentity: string
  receiverSnapshotRevision: bigint
  placementRevision: bigint
  authoredTextures: ReadonlyMap<string, AuthoredTextureInput>
  waterSurfaceFacts: readonly Readonly<{ face:number; model:number; material:number; selected:boolean; plane:readonly[number,number,number,number]; bindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>}>[]
  waterVolumeFacts: readonly Readonly<{index:number;surfaceZ:number;minimumZ:number;surfaceMaterial:number;bottomMaterial:unknown;leaves:readonly number[];clusters:readonly number[];areas:readonly number[];contents:number;plane:readonly[number,number,number,number];surfaceTranslucent:boolean;bottomTranslucent:boolean|null;surfaceBindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>;bottomBindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>|null}>[]
  waterMaterials: ReadonlyMap<string, WaterMaterialInput>
  leafMinimumDistanceToWater: Uint16Array
  sky:null|Readonly<{name:string;faces:readonly Readonly<{face:number;material:string;encoding:"srgb"|"linear"|"hdr-rgbs";selectedTextures:readonly Readonly<{logicalPath:string;sha256:string}>[]}>[]}>
  cubemapFacts:readonly Readonly<{index:number;origin:readonly[number,number,number];logicalPath:string;sha256:string;width:number;height:number}>[]
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
  alphaModulation: number
  alphaOwnership: Readonly<{
    baseTextureAvailable: boolean
    opacity: boolean
    alphaTest: boolean
    selfIlluminationMask: boolean
    environmentMask: boolean
    phongMask: boolean
    tintMask: boolean
    vertexAlpha: boolean
    materialAlphaModulation: boolean
  }>
  fragmentDiscard: Readonly<{
    kind: "none" | "alpha"
    source: "base-texture-or-one" | "shader-output"
    pass: "greater" | "greater-or-equal"
    reference: number
  }>
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
  opacity: "opaque" | "translucent"
  framebuffer: "none" | "potential" | "current"
  requiredInputs: readonly string[]
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
  modelFacing?: ReadonlyMap<string, Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>>
  modelMaterials?: ReadonlyMap<string, ModelMaterialInput>
  authoredTextures?: ReadonlyMap<string, AuthoredTextureInput>
  brushModels?:readonly Readonly<{index:number;surfaceRange:readonly[number,number];vertexCount:number;triangleCount:number;materials:readonly number[]}>[]
  modelDrawInputs?: readonly Readonly<{ entity: number; lighting: ModelLightingInput; eyes: readonly ModelEyeState[] }>[]
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
  submission: null
  exposure: ExposureSnapshot
  visibleProjectedMarks: number
  waterPasses: readonly ("reflection"|"refraction"|"main"|"intersection")[]
  waterStateRestored: boolean
  timings: Readonly<{
    particleItems: number
    particleBatches: number
    dynamicItemsMilliseconds: number
    worldMilliseconds: number
    viewModelMilliseconds: number
    totalMilliseconds: number
  }>
  viewModelPass?: Readonly<{
    depthRange: readonly [number, number]
    viewportRestored: boolean
  }>
  pacing: readonly FramePacingRecord[]
  capture?: FrameCapture
}>

export type ResizeResult = Readonly<{
  width: number
  height: number
  suspended: boolean
  deviceGeneration: number
  attachmentGeneration: number
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
      | "GpuValidationFailed"
      | "DeviceLost"
      | "CaptureFailed",
    message: string,
  ) {
    super(message)
    this.name = "RenderingError"
  }
}

type WaterMeshResource = Readonly<{
  mesh: THREE.Mesh
  materialIdentity: string
}>
type WaterMaterialResource = Readonly<{material:THREE.MeshBasicNodeMaterial;normalFrames:readonly THREE.DataTexture[];normalNode:any}>

type SceneResources = {
  map: RuntimeMap
  payload: Uint8Array
  payloadSha256: string
  loadRequest: Omit<MapLoadRequest, "payload" | "signal">
  group: THREE.Group
  modelTemplates: Map<string, THREE.Group>
  modelOccurrenceInstances:Map<number,THREE.Group>
  brushModelTemplates:Map<number,THREE.Group>
  particleTextures: Map<string, THREE.DataTexture>
  particleMaterials: Map<string, THREE.MeshBasicNodeMaterial>
  particleBatchMaterials: Map<string, THREE.MeshBasicNodeMaterial>
  materialStates: ReadonlyMap<string, MaterialStateInput>
  disposables: OwnedResourceGeneration
  lightmapTextures: readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?]
  exposureUniform: ReturnType<typeof TSL.uniform>
  diagnostics: readonly SceneDiagnostic[]
  worldBatches: readonly {
    mesh: THREE.Mesh
    faces: Uint32Array
    sourceIndices: Uint32Array
    index: THREE.BufferAttribute
    transparent: boolean
  }[]
  projectedMarks: readonly { mesh: THREE.Mesh; face: number; sourceIndex: number; visibility: EnvironmentFragmentInput["visibility"] }[]
  waterMeshes: readonly WaterMeshResource[]
  waterMaterials: ReadonlyMap<string,WaterMaterialResource>
  cubemapTextures: ReadonlyMap<number,THREE.CubeTexture>
  skyGroup: THREE.Group | null
  reflectionTarget: THREE.RenderTarget
  refractionTarget: THREE.RenderTarget
  result: SceneResult
  disposed: boolean
}

type Backend = THREE.WebGPURenderer & {
  backend: {
    isWebGPUBackend?: boolean
    device?: Readonly<{
      limits: Readonly<{ maxTextureDimension2D: number }>
      queue: Readonly<{ onSubmittedWorkDone(): Promise<unknown> }>
    }>
    context?: Readonly<{
      unconfigure(): void
      getConfiguration?(): null | Readonly<{ format: string; alphaMode?: string; colorSpace?: string }>
    }>
  }
  onDeviceLost: (info: unknown) => void
}

type WebGpuNavigator = Navigator & Readonly<{ gpu?: Readonly<{ getPreferredCanvasFormat(): string }> }>

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

function disposeScene(scene: Pick<SceneResources,"group"|"disposables"|"modelTemplates"|"modelOccurrenceInstances"|"disposed">): void {
  if (scene.disposed) return
  scene.disposed = true
  scene.group.clear()
  scene.disposables.dispose()
  scene.modelTemplates.clear()
  scene.modelOccurrenceInstances.clear()
  for(const material of scene.particleBatchMaterials.values())material.dispose()
  scene.particleBatchMaterials.clear()
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

function textureFromAuthored(input: AuthoredTextureInput, colorSpace: string, frame = 0): THREE.DataTexture {
  if (input.depth !== 1 || input.frameCount < 1 || !input.faces.includes(0) ||
    frame < 0 || frame >= input.frameCount || input.sampling.wrapS === 2 || input.sampling.wrapT === 2 || input.sampling.wrapU === 2) {
    throw new RenderingError("UnsupportedFeature", `authored 2D texture ${input.logicalPath} requires an unsupported topology or border sampler`)
  }
  const planes = input.planes
    .filter((plane) => plane.frame === frame && plane.face === 0 && plane.slice === 0)
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

function textureFromAuthoredCubemap(input: AuthoredTextureInput, colorSpace: string, frame = 0): THREE.CubeTexture {
  if (input.depth !== 1 || frame < 0 || frame >= input.frameCount || [0,1,2,3,4,5].some(face=>!input.faces.includes(face))) {
    throw new RenderingError("UnsupportedFeature", `authored cubemap ${input.logicalPath} requires an unsupported topology`)
  }
  const componentType=input.scalarEncoding==="u8"?THREE.UnsignedByteType:THREE.HalfFloatType
  const data=(plane:AuthoredTextureInput["planes"][number])=>input.scalarEncoding==="u8"?plane.rgba.slice():new Uint16Array(plane.rgba.slice().buffer)
  const level=(mip:number)=>Object.freeze({images:Object.freeze(Array.from({length:6},(_,face)=>{
    const plane=input.planes.find(value=>value.mip===mip&&value.frame===frame&&value.face===face&&value.slice===0)
    if(!plane)throw new RenderingError("MissingInput",`authored cubemap ${input.logicalPath} has an incomplete mip ${mip}`)
    const texture=new THREE.DataTexture(data(plane),plane.width,plane.height,THREE.RGBAFormat,componentType);texture.colorSpace=colorSpace;texture.flipY=false;texture.generateMipmaps=false;texture.needsUpdate=true;return texture
  }))})
  const base=level(0),texture=new THREE.CubeTexture([...base.images] as any)
  texture.mipmaps=Array.from({length:input.mipCount-1},(_,index)=>level(index+1)) as any
  texture.colorSpace=colorSpace;texture.type=componentType;texture.format=THREE.RGBAFormat;texture.flipY=false;texture.generateMipmaps=false;texture.minFilter=[THREE.NearestFilter,THREE.LinearFilter,THREE.LinearMipmapNearestFilter,THREE.LinearMipmapLinearFilter,THREE.LinearMipmapLinearFilter][input.sampling.minFilter]??THREE.LinearFilter;texture.magFilter=input.sampling.magFilter===0?THREE.NearestFilter:THREE.LinearFilter;texture.anisotropy=input.sampling.anisotropyLevel;texture.needsUpdate=true
  return texture
}

function textureFromLightmap(lightmap: RuntimeLightmap, plane: Float32Array): THREE.DataTexture {
  const texture = new THREE.DataTexture(plane, lightmap.width, lightmap.height, THREE.RGBAFormat, THREE.FloatType)
  configureWorldLightmap(texture, lightmap.profile)
  return texture
}

function materialOptions(resolved: RuntimeMaterial, state?: MaterialStateInput): THREE.MeshBasicMaterialParameters {
  const blendFactor = (value: number) => [THREE.ZeroFactor, THREE.OneFactor, THREE.SrcAlphaFactor, THREE.OneMinusSrcAlphaFactor][value] ?? THREE.OneFactor
  const bias = sourceDepthBias(state?.polygonOffset === 1 ? "decal" : "none")
  return {
    transparent: state?.blendEnabled ?? (resolved.features & 1) !== 0,
    blending: state?.blendEnabled ? THREE.CustomBlending : THREE.NormalBlending,
    blendSrc: state ? blendFactor(state.blendSource) : undefined,
    blendDst: state ? blendFactor(state.blendDestination) : undefined,
    alphaTest: 0,
    side: state?.cull === 1 ? THREE.DoubleSide : worldMaterialSide(resolved.features),
    depthTest: state?.depthTest ?? true,
    depthWrite: state?.depthWrite ?? true,
    depthFunc: state?.depthFunction === 0 ? THREE.LessDepth : THREE.LessEqualDepth,
    polygonOffset: bias.enabled,
    polygonOffsetFactor: bias.slopeScale,
    polygonOffsetUnits: bias.units,
    wireframe: state?.wireframe ?? false,
  }
}

function sourceModelSide(facing: Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>): THREE.Side {
  if (facing.cullFace !== "back") throw new RenderingError("MalformedInput", "StudioModel cull face is invalid")
  return facing.frontFace === "clockwise" ? THREE.BackSide : THREE.FrontSide
}

function sourceFragmentColor(sample: any, state?: MaterialStateInput): any {
  const alpha = sample.a.mul(state?.alphaModulation ?? 1)
  const color = TSL.vec4(sample.rgb, alpha)
  if (state?.fragmentDiscard.kind !== "alpha") return color
  return TSL.Fn(() => {
    const rejected = state.fragmentDiscard.pass === "greater"
      ? alpha.lessThanEqual(state.fragmentDiscard.reference)
      : alpha.lessThan(state.fragmentDiscard.reference)
    rejected.discard()
    return color
  })()
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
  material.colorNode = sourceFragmentColor(TSL.vec4(base.rgb.mul(irradiance).mul(exposure), base.a), state)
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
  #backend!: Backend
  #scene = new THREE.Scene()
  #world = new THREE.Group()
  #effects = new THREE.Group()
  #particles = new THREE.Group()
  #viewModels = new THREE.Group()
  #camera = new THREE.PerspectiveCamera(75, 1, 1, 32_768)
  #viewCamera = new THREE.PerspectiveCamera(41.114, 1, 1, 32_768)
  #viewModelInstances = new Map<number, { model: string; root: THREE.Group; instance: THREE.Group; meshes: THREE.Mesh[] }>()
  #particleBatchMeshes: { key: string; capacity: number; mesh: THREE.Mesh }[] = []
  #particleBatchCount=0
  #active?: SceneResources
  #renderBusy = false
  #loadOrdinal = 0
  #suspended = false
  #viewportWidth = 0
  #viewportHeight = 0
  #cssWidth = 0
  #cssHeight = 0
  #devicePixelRatio = 1
  #attachmentGeneration = 0
  #pacingHandle?: number
  #pacingCallback?: FramePacingCallback
  readonly #pacing = new FramePacingController({ now: () => performance.now() }, 256)

  constructor(request: RendererCreateRequest) {
    this.configuration = validateRenderConfiguration(request.configuration)
    this.#canvas = request.canvas
    this.#powerPreference = request.powerPreference
    this.#exposure = new ExposureController(this.configuration.exposure)
    this.#scene.background = null
    this.#scene.add(this.#world, this.#effects, this.#particles, this.#camera, this.#viewCamera)
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
    const gpu=(globalThis.navigator as WebGpuNavigator|undefined)?.gpu
    if (!gpu || !this.#canvas || typeof this.#canvas.getContext !== "function") {
      throw new RenderingError("UnsupportedEnvironment", "WebGPU canvas is unavailable")
    }
    if (gpu.getPreferredCanvasFormat() !== this.configuration.canvasFormat) {
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
    let context: Backend["backend"]["context"]
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
      const actual = context?.getConfiguration?.()
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
    if(!request.diagnostic)throw new RenderingError("UnsupportedFeature","ordinary rendering requires an explicit WebGPU frame encoder; the current Three.js adapter is diagnostic-only")
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
        !HASH.test(request.environment.collisionWorldIdentity) ||
        request.environment.receiverSnapshotRevision < 1n || request.environment.placementRevision < 1n ||
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
        !Number.isFinite(state.alphaModulation) || state.alphaModulation < 0 || state.alphaModulation > 1 ||
        state.fragmentDiscard.reference !== (state.fragmentDiscard.kind === "alpha" ? state.alphaTestReference : 0) ||
        state.alphaTest !== (state.fragmentDiscard.kind === "alpha") ||
        (state.fragmentDiscard.kind === "none" && (state.fragmentDiscard.source !== "base-texture-or-one" || state.fragmentDiscard.pass !== "greater")) ||
        Object.values(state.alphaOwnership).some((value) => typeof value !== "boolean") ||
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
    for (const mark of request.environment?.markRecords ?? []) {
      const state = materialStates.get(mark.material.toLowerCase())
      if (mark.status === 0 && (!mark.receiver || !state || (mark.polygonOffset === "decal") !== (state.polygonOffset === 1) ||
        mark.fragments.some((fragment) => fragment.visibility.kind === "brush-model"
          ? mark.receiver?.entity !== fragment.visibility.entity || mark.receiver.model !== fragment.visibility.model
          : fragment.model !== 0))) {
        throw new RenderingError("IdentityMismatch", "projected mark receiver or material contract differs")
      }
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
        new Set(material.requiredInputs).size !== material.requiredInputs.length ||
        material.requiredInputs.some((input) => ![
          "ambient-cube", "local-lights", "camera-position", "studio-eye-parameters",
          "local-environment", "current-framebuffer", "authored-texture-planes", "game-proxy-values",
        ].includes(input)) ||
        material.bindings.some((binding) => !authoredTextures.has(binding.logicalPath.toLowerCase()))) {
        throw new RenderingError("MalformedInput", "model material input is invalid")
      }
      modelMaterials.set(key, material)
    }
    const modelFacing = new Map(request.modelFacing ?? [])
    for (const model of map.models) {
      const identity = model.logicalPath.split("#skin=")[0]!.toLowerCase()
      const facing = modelFacing.get(identity)
      if (!facing) throw new RenderingError("MissingInput", `StudioModel facing ${identity} is unavailable`)
      sourceModelSide(facing)
    }
    const modelDrawInputs = new Map<number, NonNullable<MapLoadRequest["modelDrawInputs"]>[number]>()
    const modelEntities = new Set(map.modelOccurrences.map((occurrence) => occurrence.entity))
    for (const input of request.modelDrawInputs ?? []) {
      if (!modelEntities.has(input.entity) || modelDrawInputs.has(input.entity)) {
        throw new RenderingError("IdentityMismatch", "model draw-input identity differs")
      }
      try {
        validateModelLightingInput(input.lighting)
        validateModelEyeStates(input.eyes)
      } catch (error) {
        throw new RenderingError("MalformedInput", `model draw input is invalid: ${String(error)}`)
      }
      modelDrawInputs.set(input.entity, Object.freeze({ ...input, eyes: Object.freeze([...input.eyes]) }))
    }
    const materialIdentities = new Set([
      ...map.materials.map((material) => material.logicalPath.toLowerCase()),
      ...map.models.flatMap((model) => model.materials.map((material) => material.logicalPath.toLowerCase())),
    ])
    if ([...directionalInputs.keys()].some((identity) => !materialIdentities.has(identity))) {
      throw new RenderingError("MalformedInput", "directional texture names an unavailable material")
    }
    this.#checkAbort(request.signal, ordinal)
    const normalizedRequest = Object.freeze({
      ...request,
      materialStates,
      authoredTextures,
      modelMaterials,
      modelFacing,
      modelDrawInputs: Object.freeze([...modelDrawInputs.values()]),
    })
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
      this.#clearParticleBatches()
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
    const modelOccurrenceInstances=new Map<number,THREE.Group>()
    const brushModelTemplates=new Map<number,THREE.Group>()
    const particleTextures = new Map<string, THREE.DataTexture>()
    const particleMaterials = new Map<string, THREE.MeshBasicNodeMaterial>()
    const materialStates = new Map(request.materialStates ?? [])
    const disposables = new OwnedResourceGeneration(this.#deviceGeneration, sceneGeneration)
    const diagnostics: SceneDiagnostic[] = []
    const worldBatches: SceneResources["worldBatches"][number][] = []
    const projectedMarks: { mesh: THREE.Mesh; face: number; sourceIndex: number; visibility: EnvironmentFragmentInput["visibility"] }[] = []
    const waterMeshes: WaterMeshResource[]=[]
    const targetWidth=Math.max(1,Number((this.#canvas as {width?:number}).width??1)),targetHeight=Math.max(1,Number((this.#canvas as {height?:number}).height??1))
    const reflectionTarget=disposables.add(new THREE.RenderTarget(targetWidth,targetHeight,{depthBuffer:true}))
    const refractionTarget=disposables.add(new THREE.RenderTarget(targetWidth,targetHeight,{depthBuffer:true}))
    reflectionTarget.texture.colorSpace=THREE.NoColorSpace
    refractionTarget.texture.colorSpace=THREE.NoColorSpace
    const waterMaterials=new Map<string,WaterMaterialResource>()
    const cubemapTextures=new Map<number,THREE.CubeTexture>()
    let skyGroup:THREE.Group|null=null
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
    const modelDrawInputs = new Map((request.modelDrawInputs ?? []).map((input) => [input.entity, input] as const))
    const missingLightingEntities = map.modelOccurrences
      .filter((occurrence) => !modelDrawInputs.has(occurrence.entity))
      .map((occurrence) => occurrence.entity)
    if (missingLightingEntities.length > 0) {
      diagnostics.push(diagnostic(
        "MissingModelLighting",
        "model-lightcache",
        `explicit ModelLightingInput is unavailable for entities ${missingLightingEntities.join(",")}`,
      ))
    }
    const modelsWithEyes = new Set(map.models
      .filter((model) => model.materials.some((material) => {
        const shader = request.modelMaterials?.get(material.logicalPath.toLowerCase())?.shader
        return shader === "eye-refract" || shader === "eyes"
      }))
      .map((model) => model.logicalPath))
    const missingEyeEntities = map.modelOccurrences
      .filter((occurrence) => modelsWithEyes.has(map.models[occurrence.model]!.logicalPath) && !(modelDrawInputs.get(occurrence.entity)?.eyes.length))
      .map((occurrence) => occurrence.entity)
    if (missingEyeEntities.length > 0) {
      diagnostics.push(diagnostic(
        "MissingModelEyeState",
        "studio-eye-state",
        `explicit StudioModel eye state is unavailable for entities ${missingEyeEntities.join(",")}`,
      ))
    }
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
      const authored=request.environment?.authoredTextures.get(source.logicalPath.toLowerCase())
      if(authored){const key=`environment:${source.logicalPath.toLowerCase()}`,retained=authoredGpu.get(key);if(retained)return retained;const texture=textureFromAuthored(authored,THREE.SRGBColorSpace);authoredGpu.set(key,texture);disposables.add(texture);return texture}
      requireMipInputs(identity, state)
      const texture = textureFromRgba(source, THREE.SRGBColorSpace, state)
      disposables.add(texture)
      return texture
    }
    const createWaterMaterial=(identity:string):WaterMaterialResource=>{
      const key=identity.toLowerCase(),existing=waterMaterials.get(key);if(existing)return existing
      const state=request.environment?.waterMaterials.get(key)
      if(!state)throw new RenderingError("MissingInput",`Water material ${identity} is unavailable`)
      const normalRequest=state.textures.find(texture=>texture.role===8&&texture.disposition==="source")
      if(!normalRequest?.logicalPath)throw new RenderingError("MissingInput",`Water normal texture ${identity} is unavailable`)
      const authored=request.environment?.authoredTextures.get(normalRequest.logicalPath.toLowerCase())
      if(!authored)throw new RenderingError("MissingInput",`Water authored normal ${normalRequest.logicalPath} is unavailable`)
      const normalFrames=Object.freeze(Array.from({length:authored.frameCount},(_,frame)=>{const texture=textureFromAuthored(authored,THREE.NoColorSpace,frame);disposables.add(texture);return texture}))
      const normalNode=TSL.texture(normalFrames[state.normalFrame.value]??normalFrames[0]!)
      const material=new THREE.MeshBasicNodeMaterial({transparent:state.opacity==="translucent",depthTest:true,depthWrite:state.opacity==="opaque",side:THREE.FrontSide})
      material.colorNode=TSL.vec4(TSL.color(debugColor(`diagnostic-water:${identity}`)),state.opacity==="translucent"?0.5:1);material.toneMapped=false;disposables.add(material)
      const resource=Object.freeze({material,normalFrames,normalNode});waterMaterials.set(key,resource);return resource
    }
    const createWorldMesh=(batch:RuntimeBatch):THREE.Mesh|null=>{
        const geometry = new THREE.BufferGeometry()
        geometry.setAttribute("position", new THREE.BufferAttribute(batch.positions, 3))
        geometry.setAttribute("normal", new THREE.BufferAttribute(batch.normals, 3))
        geometry.setAttribute("uv", new THREE.BufferAttribute(batch.uv, 2))
        geometry.setAttribute("uv1", new THREE.BufferAttribute(batch.lightmapUv, 2))
        geometry.setAttribute("lightmapKind", new THREE.BufferAttribute(batch.lightmapKind, 1))
        const sourceIndices = batch.indices.slice()
        const index = new THREE.BufferAttribute(sourceIndices.slice(), 1)
        index.setUsage(THREE.DynamicDrawUsage)
        geometry.setIndex(index)
        geometry.computeBoundingSphere()
        disposables.add(geometry)
        const resolved = map.materials[batch.material]!
        const identity = resolved.logicalPath
        const materialState = materialStates.get(identity.toLowerCase())
        if (materialState?.noDraw) return null
        if(resolved.shader===5){const resource=createWaterMaterial(identity),mesh=new THREE.Mesh(geometry,resource.material);mesh.userData.materialIdentity=identity;waterMeshes.push(Object.freeze({mesh,materialIdentity:identity.toLowerCase()}));return mesh}
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
          const nodeMaterial = new THREE.MeshBasicNodeMaterial(materialOptions(resolved, materialState))
          const base = baseTexture ? TSL.texture(baseTexture, TSL.uv()) : TSL.vec4(TSL.color(debugColor(identity)), 1)
          const irradiance = TSL.texture(lightmapTextures[0], TSL.uv(1)).rgb
          nodeMaterial.colorNode = sourceFragmentColor(TSL.vec4(base.rgb.mul(irradiance), base.a), materialState)
          nodeMaterial.toneMapped = false
          material = nodeMaterial
        }
        disposables.add(material)
        const mesh = new THREE.Mesh(geometry, material)
        mesh.userData.materialIdentity = identity
        return mesh
    }
    try {
      for(const batch of map.batches){const mesh=createWorldMesh(batch);if(!mesh)continue;const index=mesh.geometry.getIndex();if(!index)throw new RenderingError("MalformedInput","world batch has no index buffer");worldBatches.push({mesh,faces:batch.faces.slice(),sourceIndices:batch.indices.slice(),index,transparent:(Array.isArray(mesh.material)?mesh.material: [mesh.material]).some(material=>material.transparent)});group.add(mesh)}
      for(const model of map.brushModels){const template=new THREE.Group();for(const batch of model.batches){const mesh=createWorldMesh(batch);if(mesh)template.add(mesh)}brushModelTemplates.set(model.index,template)}

      const environmentTextures = new Map<string, THREE.DataTexture>()
      const authoredEnvironmentMaterials=new Set<string>()
      for (const texture of request.environment?.textures ?? []) {
        const authored=request.environment?.authoredTextures.get(texture.logicalPath.toLowerCase()),value=authored?textureFromAuthored(authored,THREE.SRGBColorSpace):textureFromRgba(texture, THREE.SRGBColorSpace, materialStates.get(texture.material.toLowerCase()))
        if(authored)authoredEnvironmentMaterials.add(texture.material.toLowerCase())
        environmentTextures.set(texture.material.toLowerCase(), value)
        disposables.add(value)
      }
      for(const fact of request.environment?.cubemapFacts??[]){
        if(cubemapTextures.has(fact.index)||!HASH.test(fact.sha256)||!finite(fact.origin))throw new RenderingError("MalformedInput","cubemap occurrence input is invalid")
        const authored=request.environment!.authoredTextures.get(fact.logicalPath.toLowerCase())
        if(!authored||authored.sourceSha256!==fact.sha256||authored.width!==fact.width||authored.height!==fact.height)throw new RenderingError("IdentityMismatch","cubemap authored texture identity differs")
        const texture=textureFromAuthoredCubemap(authored,THREE.NoColorSpace);cubemapTextures.set(fact.index,texture);disposables.add(texture)
      }
      if(request.environment?.sky){
        const semantics=["right","left","back","front","up","down"] as const
        const faces=new Map<number,(typeof request.environment.sky.faces)[number]>()
        for(const face of request.environment.sky.faces){
          if(face.face<0||face.face>5||faces.has(face.face))throw new RenderingError("MalformedInput","2D sky face identity is invalid")
          faces.set(face.face,face)
        }
        if(faces.size!==6)throw new RenderingError("MissingInput","2D sky requires six semantic faces")
        if([...faces.values()].every(face=>face.encoding)){
          skyGroup=new THREE.Group()
          skyGroup.renderOrder=-1
          const geometryFaces=buildSourceSkyGeometry([0,0,0],1)
          for(let faceIndex=0;faceIndex<6;faceIndex+=1){
            const face=faces.get(faceIndex)!,geometryFace=geometryFaces.find(value=>value.face===semantics[faceIndex])!
            if(face.selectedTextures.length!==1)throw new RenderingError("UnsupportedFeature","the supplied 2D-sky shader encoding requires one selected texture")
            const selected=face.selectedTextures[0]!,authored=request.environment.authoredTextures.get(selected.logicalPath.toLowerCase())
            if(!authored||authored.sourceSha256!==selected.sha256)throw new RenderingError("IdentityMismatch","2D sky authored texture identity differs")
            const texture=textureFromAuthored(authored,face.encoding==="srgb"?THREE.SRGBColorSpace:THREE.NoColorSpace)
            if(face.encoding==="hdr-rgbs"){texture.minFilter=THREE.NearestMipmapNearestFilter;texture.magFilter=THREE.NearestFilter;texture.needsUpdate=true}
            disposables.add(texture)
            const geometry=new THREE.BufferGeometry(),positions=Float32Array.from(geometryFace.vertices.flatMap(vertex=>vertex.position)),uv=Float32Array.from(geometryFace.vertices.flatMap(vertex=>vertex.uv))
            geometry.setAttribute("position",new THREE.BufferAttribute(positions,3));geometry.setAttribute("uv",new THREE.BufferAttribute(uv,2));geometry.setIndex([...geometryFace.indices]);disposables.add(geometry)
            const material=new THREE.MeshBasicNodeMaterial({depthTest:false,depthWrite:false,side:THREE.BackSide})
            if(face.encoding==="hdr-rgbs"){
              const baseUv=TSL.uv(),fudge=0.01/Math.max(authored.width,authored.height),increment=TSL.vec2(0.5/authored.width-fudge,0.5/authored.height-fudge),uv00=baseUv.sub(increment),uv10=TSL.vec2(baseUv.x.add(increment.x),baseUv.y.sub(increment.y)),uv01=TSL.vec2(baseUv.x.sub(increment.x),baseUv.y.add(increment.y)),uv11=baseUv.add(increment),s00=TSL.texture(texture,uv00),s10=TSL.texture(texture,uv10),s01=TSL.texture(texture,uv01),s11=TSL.texture(texture,uv11),fraction=TSL.fract(uv00.mul(TSL.vec2(authored.width,authored.height))),top=TSL.mix(s00.rgb.mul(s00.a),s10.rgb.mul(s10.a),fraction.x),bottom=TSL.mix(s01.rgb.mul(s01.a),s11.rgb.mul(s11.a),fraction.x)
              material.colorNode=TSL.vec4(TSL.mix(top,bottom,fraction.y).mul(8),1)
            }else material.colorNode=TSL.texture(texture,TSL.uv())
            material.toneMapped=false;disposables.add(material)
            const mesh=new THREE.Mesh(geometry,material);mesh.userData.skyFace=geometryFace.face;mesh.renderOrder=-1;skyGroup.add(mesh)
          }
          group.add(skyGroup)
        }
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
          if(!authoredEnvironmentMaterials.has(mark.material.toLowerCase()))requireMipInputs(mark.material, state)
          const material = new THREE.MeshBasicNodeMaterial(materialOptions({ logicalPath: mark.material, width: 1, height: 1, shader: 3, features: 1, textureRole: 0 }, state))
          material.colorNode = sourceFragmentColor(TSL.texture(texture, TSL.uv()), state)
          material.toneMapped = false
          disposables.add(material)
          const mesh = new THREE.Mesh(geometry, material)
          projectedMarks.push({ mesh, face: fragment.face, sourceIndex: mark.sourceIndex, visibility: fragment.visibility })
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
          const material = new THREE.MeshBasicNodeMaterial({
            ...materialOptions(resolved, materialState),
            side: sourceModelSide(request.modelFacing!.get(model.logicalPath.split("#skin=")[0]!.toLowerCase())!),
          })
           const base = selectDiagnosticModelBase(baseTexture !== undefined) === "authored-texture"
             ? TSL.texture(baseTexture!, TSL.uv())
             : TSL.vec4(TSL.color(debugColor(`diagnostic:${resolved.logicalPath}`)), 1)
          material.colorNode = sourceFragmentColor(base, materialState)
          material.toneMapped = false
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
        modelOccurrenceInstances.set(occurrence.entity,instance)
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
        material.colorNode = sourceFragmentColor(TSL.vec4(sampled.rgb.mul(color.rgb), sampled.a.mul(color.a)), state)
        material.toneMapped = false
        particleMaterials.set(texture.material.toLowerCase(), material)
        disposables.add(material)
      }
    } catch (error) {
      const failed = {
        group,
        modelTemplates,
        modelOccurrenceInstances,
        brushModelTemplates,
        particleTextures,
        particleMaterials,
        skyGroup,
        cubemapTextures,
        materialStates,
        disposables,
        projectedMarks,
        disposed: false,
      }
      disposeScene(failed)
      if (error instanceof RenderingError) throw error
      throw new RenderingError("BoundExceeded", `runtime map GPU staging failed: ${String(error)}`)
    }

    const requirements = map.lighting.profile === "hdr" ? map.lighting.descriptor.requirements : Object.freeze([])
    const remainingRequirements=requirements.filter(requirement=>{
      if(requirement.identity==="map-environment-presentation")return !request.environment
      if(requirement.identity==="map-water-presentation")return !request.environment||request.environment.waterMaterials.size===0||request.environment.waterSurfaceFacts.length===0
      return true
    })
    for (const requirement of remainingRequirements) {
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
      requirements:Object.freeze(remainingRequirements),
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
        modelFacing: new Map(request.modelFacing ?? []),
        modelMaterials: new Map(request.modelMaterials ?? []),
        authoredTextures: new Map([...(request.authoredTextures ?? [])].map(([identity, texture]) => [identity, Object.freeze({
          ...texture,
          faces: Object.freeze([...texture.faces]),
          planes: Object.freeze(texture.planes.map((plane) => Object.freeze({ ...plane, rgba: plane.rgba.slice() }))),
        })])),
        modelDrawInputs: request.modelDrawInputs?.map((input) => Object.freeze({
          entity: input.entity,
          lighting: structuredClone(input.lighting),
          eyes: Object.freeze(input.eyes.map((eye) => structuredClone(eye))),
        })),
        brushModels:request.brushModels?.map(model=>Object.freeze({...model,surfaceRange:Object.freeze([...model.surfaceRange]) as readonly[number,number],materials:Object.freeze([...model.materials])})),
        diagnostic: request.diagnostic,
      },
      group,
      modelTemplates,
      modelOccurrenceInstances,
      brushModelTemplates,
      particleTextures,
      particleMaterials,
      particleBatchMaterials:new Map(),
      materialStates,
      disposables,
      lightmapTextures,
      exposureUniform,
      diagnostics: Object.freeze(diagnostics),
      worldBatches: Object.freeze(worldBatches),
      projectedMarks: Object.freeze(projectedMarks),
      waterMeshes:Object.freeze(waterMeshes),
      waterMaterials,
      cubemapTextures,
      skyGroup,
      reflectionTarget,
      refractionTarget,
      result,
      disposed: false,
    }
  }

  async render(frame: Frame): Promise<FrameResult> {
    const frameStarted = performance.now()
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
        this.#setWorldVisibility(frame.visibility.surfaces)
        if(this.#active.skyGroup)this.#active.skyGroup.visible=frame.visibility.sky===2
        if (!frame.collisionWorldIdentity || frame.collisionWorldIdentity !== this.#active.result.environment?.collisionWorldIdentity) {
          throw new RenderingError("IdentityMismatch", "mark collision-world identity differs")
        }
        for (const mark of this.#active.projectedMarks) {
          if (mark.visibility.kind === "world") {
            mark.mesh.visible = visible.has(mark.face)
            mark.mesh.matrixAutoUpdate = true
            sourceTransform(mark.mesh, [0, 0, 0], [0, 0, 0])
          } else {
            const visibility=mark.visibility
            const receiver = frame.brushModels?.models.find((model) =>
              BigInt(model.sourceIndex) === visibility.entity && model.model === visibility.model)
            mark.mesh.visible = receiver?.draw === true
            if (receiver) sourceTransform(mark.mesh, receiver.worldPosition, receiver.worldAngles)
          }
        }
        const water=frame.visibility.water,visibleWater=water.visibleWater
        if(visibleWater){const resource=this.#active.waterMaterials.get(visibleWater.material.toLowerCase());if(!resource)throw new RenderingError("MissingInput",`current Water material ${visibleWater.material} is unavailable`);const frameIndex=((visibleWater.evaluated.normalFrame%resource.normalFrames.length)+resource.normalFrames.length)%resource.normalFrames.length,texture=resource.normalFrames[frameIndex]!,matrix=visibleWater.evaluated.normalTransform;texture.matrixAutoUpdate=false;texture.matrix.set(matrix[0]!,matrix[1]!,matrix[3]!,matrix[4]!,matrix[5]!,matrix[7]!,matrix[12]!,matrix[13]!,matrix[15]!);texture.needsUpdate=true;resource.normalNode.value=texture;for(const waterMesh of this.#active.waterMeshes){waterMesh.mesh.material=resource.material;waterMesh.mesh.visible=water.render.drawSurface&&waterMesh.materialIdentity===visibleWater.material.toLowerCase()}}
        else for(const waterMesh of this.#active.waterMeshes)waterMesh.mesh.visible=false
      }
      if (frame.exposureHistogram) this.#exposure.submit(frame.exposureHistogram)
      const exposure =
        this.configuration.lightingProfile === "hdr"
          ? this.#exposure.advance(frame.deltaSeconds ?? 0)
          : this.#exposure.snapshot()
      this.#active.exposureUniform.value = this.configuration.lightingProfile === "hdr" ? exposure.current : 1
      this.#setCamera(frame.camera)
      const dynamicItemsStarted = performance.now()
      const viewModelDepthRange = this.#stageDynamicItems(frame)
      const dynamicItemsMilliseconds = performance.now() - dynamicItemsStarted
      let viewModelPass: FrameResult["viewModelPass"]
      let waterPasses:FrameResult["waterPasses"]=Object.freeze([]),waterStateRestored=true
      let worldMilliseconds=0,viewModelMilliseconds=0
      if (!this.#suspended) {
        const worldStarted=performance.now()
        const waterResult=await this.#renderWaterPasses(frame)
        worldMilliseconds=performance.now()-worldStarted
        waterPasses=waterResult.passes
        waterStateRestored=waterResult.restored
        if (this.#viewModels.children.length > 0) {
          if (!viewModelDepthRange) throw new RenderingError("InvalidState", "viewmodel depth range is unavailable")
          this.#backend.autoClear = false
          const background = this.#scene.background
          this.#scene.background = null
          this.#backend.setViewport(0, 0, this.#viewportWidth, this.#viewportHeight, viewModelDepthRange[0], viewModelDepthRange[1])
          const viewModelStarted=performance.now()
          try {
            await this.#backend.renderAsync(this.#scene, this.#viewCamera)
          } finally {
            viewModelMilliseconds=performance.now()-viewModelStarted
            this.#backend.setViewport(0, 0, this.#viewportWidth, this.#viewportHeight, 0, 1)
            this.#scene.background = background
            this.#backend.autoClear = true
          }
          viewModelPass = Object.freeze({ depthRange: viewModelDepthRange, viewportRestored: true })
        }
      }
      const capture = frame.capture ? await this.#capture(frame.capture) : undefined
      return Object.freeze({
        deviceGeneration: this.#deviceGeneration,
        sceneGeneration: this.#sceneGeneration,
        submission: null,
        exposure,
        visibleProjectedMarks: this.#active.projectedMarks.reduce((total, mark) => total + Number(mark.mesh.visible), 0),
        waterPasses,
        waterStateRestored,
        timings:Object.freeze({particleItems:frame.particles?.length??0,particleBatches:this.#particleBatchCount,dynamicItemsMilliseconds,worldMilliseconds,viewModelMilliseconds,totalMilliseconds:performance.now()-frameStarted}),
        viewModelPass,
        pacing:this.#pacing.records(),
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

  #setWorldVisibility(surfaces: Uint32Array): void {
    if (!this.#active) throw new RenderingError("InvalidState", "renderer has no active world visibility resources")
    const visible = new Set(surfaces)
    const order = new Map<number, number>()
    for (let index = 0; index < surfaces.length; index += 1) {
      const face = surfaces[index]!
      if (order.has(face)) throw new RenderingError("MalformedInput", "visibility contains a duplicate world face")
      order.set(face, index)
    }
    for (const batch of this.#active.worldBatches) {
      const selected = visibleTriangleIndices(
        batch.sourceIndices,
        batch.faces,
        visible,
        batch.transparent ? order : undefined,
      )
      const target = batch.index.array as Uint32Array
      target.set(selected)
      batch.index.needsUpdate = true
      batch.mesh.geometry.setDrawRange(0, selected.length)
      batch.mesh.visible = selected.length > 0
    }
  }

  #validateFrame(frame: Frame): void {
    if (
      frame.effects.length + (frame.shadows?.length ?? 0) + (frame.models?.length ?? 0) + (frame.particles?.length ?? 0)+(frame.brushModels?.models.length??0) > MAX_EFFECTS ||
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
    try {
      validateDynamicLights(frame.effects, MAX_EFFECTS)
      validateShadows(frame.shadows ?? [], MAX_EFFECTS)
    } catch (error) {
      throw new RenderingError("MalformedInput", `dynamic-light or shadow input is invalid: ${String(error)}`)
    }
    if ((frame.effects.length > 0 || (frame.shadows?.length ?? 0) > 0) && !this.#active!.loadRequest.diagnostic) {
      throw new RenderingError("UnsupportedFeature", "the active renderer backend has no accepted dynamic-light or shadow pass")
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
      if (!item.modelLighting && !this.#active!.loadRequest.diagnostic) {
        throw new RenderingError("MissingInput", `explicit model lighting ${item.identity} is unavailable`)
      }
      if (item.modelLighting) {
        try {
          validateModelLightingInput(item.modelLighting)
          validateModelEyeStates(item.eyeStates ?? [])
        } catch (error) {
          throw new RenderingError("MalformedInput", `model draw input ${item.identity} is invalid: ${String(error)}`)
        }
      }
      const runtimeModel = this.#active!.map.models.find((model) => model.logicalPath === modelKey(item.model, item.skin ?? 0))
      if (!runtimeModel) throw new RenderingError("IdentityMismatch", "runtime model draw identity differs")
      for (const [primitiveIndex, primitive] of runtimeModel.primitives.entries()) {
        const materialIdentity = runtimeModel.materials[primitive.material]!.logicalPath.toLowerCase()
        const material = this.#active!.loadRequest.modelMaterials?.get(materialIdentity)
        if (!material) continue
        try {
          prepareModelDrawInputs({
            primitive: item.pose?.primitives[primitiveIndex]?.primitive ?? primitiveIndex,
            required: material.requiredInputs as readonly ModelDrawRequirement[],
            lighting: item.modelLighting,
            eyes: item.eyeStates,
            currentFramebuffer: item.currentFramebufferAvailable === true,
            authoredTexturePlanes: true,
            gameProxyValues: item.gameProxyValuesAvailable === true,
          })
        } catch (error) {
          if (!this.#active!.loadRequest.diagnostic) {
            throw new RenderingError("MissingInput", `model draw ${item.identity} inputs are incomplete: ${String(error)}`)
          }
        }
      }
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
    if(this.#active!.result.environment&&!frame.visibility)throw new RenderingError("MissingInput","an exact visibility result is required")
    if(frame.visibility){if(!Array.isArray(frame.visibility.leaves)||!Array.isArray(frame.visibility.areas)||frame.visibility.sky<0||frame.visibility.sky>2||(frame.visibility.eyeLeaf!==null&&(!Number.isSafeInteger(frame.visibility.eyeLeaf)||frame.visibility.eyeLeaf<0)))throw new RenderingError("MalformedInput","visibility view state is invalid");const water=frame.visibility.water;if(water.passes.length<1||water.passes.length>4||water.passes.filter(pass=>pass.kind==="main").length!==1)throw new RenderingError("MalformedInput","Water view plan is invalid");let prior=-1;for(const pass of water.passes){const order=pass.kind==="reflection"?0:pass.kind==="refraction"?1:pass.kind==="main"?2:3;if(order<prior||!finite([...pass.origin,...pass.angles,pass.clip?.height??0])||pass.surfaces.length>100_000)throw new RenderingError("MalformedInput","Water pass is invalid");prior=order}if(water.visibleWater&&(!finite([water.visibleWater.surfaceZ,water.visibleWater.evaluated.cheapStart,water.visibleWater.evaluated.cheapEnd,...water.visibleWater.evaluated.normalTransform])||water.visibleWater.evaluated.normalTransform.length!==16))throw new RenderingError("MalformedInput","current Water state is invalid")}
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
    if(this.#active?.skyGroup){this.#active.skyGroup.position.copy(this.#camera.position);this.#active.skyGroup.scale.setScalar(input.far)}
  }

  #setWaterCamera(pass:WaterFramePass,frame:Camera):void{
    this.#camera.position.set(...pass.origin)
    this.#camera.fov=frame.verticalFovDegrees;this.#camera.near=frame.near;this.#camera.far=frame.far;this.#camera.updateProjectionMatrix()
    const yaw=THREE.MathUtils.degToRad(pass.angles[1]),pitch=THREE.MathUtils.degToRad(pass.angles[0]),direction=new THREE.Vector3(Math.cos(pitch)*Math.cos(yaw),Math.cos(pitch)*Math.sin(yaw),-Math.sin(pitch))
    this.#camera.lookAt(this.#camera.position.clone().add(direction))
    if(this.#active?.skyGroup){this.#active.skyGroup.position.copy(this.#camera.position);this.#active.skyGroup.scale.setScalar(frame.far)}
  }

  #setClip(clip:WaterFramePass["clip"]):()=>void{
    if(!clip)return()=>{}
    const saved=new Map<THREE.Material,THREE.Plane[]|null>()
    const plane=new THREE.Plane(new THREE.Vector3(0,0,clip.keep==="above"?1:-1),clip.keep==="above"?-clip.height:clip.height)
    for(const root of [this.#world,this.#effects,this.#particles])root.traverse(object=>{if(!(object instanceof THREE.Mesh))return;for(const material of Array.isArray(object.material)?object.material:[object.material]){if(!saved.has(material))saved.set(material,material.clippingPlanes);material.clippingPlanes=[plane]}})
    return()=>{for(const [material,value] of saved)material.clippingPlanes=value}
  }

  async #renderWaterPasses(frame:Frame):Promise<Readonly<{passes:FrameResult["waterPasses"];restored:boolean}>>{
    if(!this.#active)throw new RenderingError("InvalidState","renderer has no active Water resources")
    const plan=frame.visibility?.water
    if(!plan){this.#backend.autoClear=true;await this.#backend.renderAsync(this.#scene,this.#camera);return Object.freeze({passes:Object.freeze(["main"] as const),restored:true})}
    const markVisibility=this.#active.projectedMarks.map(mark=>mark.mesh.visible),waterVisibility=this.#active.waterMeshes.map(water=>water.mesh.visible),background=this.#scene.background,effectsVisible=this.#effects.visible,particlesVisible=this.#particles.visible,skyVisible=this.#active.skyGroup?.visible??false
    const completed:("reflection"|"refraction"|"main"|"intersection")[]=[]
    if(plan.visibleWater&&plan.passes.some(pass=>pass.kind!=="main"&&pass.drawSky2d)&&!this.#active.skyGroup)throw new RenderingError("MissingInput","Water auxiliary view requests the unresolved 2D sky pass")
    try{
      for(const pass of plan.passes){const visible=new Set(pass.surfaces);this.#setWorldVisibility(pass.surfaces);this.#active.projectedMarks.forEach(mark=>{if(mark.visibility.kind==="world")mark.mesh.visible=visible.has(mark.face)});this.#active.waterMeshes.forEach((water,index)=>water.mesh.visible=pass.renderWaterSurface&&waterVisibility[index]===true);if(this.#active.skyGroup)this.#active.skyGroup.visible=pass.drawSky2d;this.#effects.visible=pass.drawEntities;this.#particles.visible=pass.drawEntities;this.#scene.background=pass.drawSky2d?background:null;this.#setWaterCamera(pass,frame.camera);const restoreClip=this.#setClip(pass.clip);try{this.#backend.setRenderTarget(pass.kind==="reflection"?this.#active.reflectionTarget:pass.kind==="refraction"?this.#active.refractionTarget:null);this.#backend.autoClear=pass.kind!=="intersection";await this.#backend.renderAsync(this.#scene,this.#camera);completed.push(pass.kind)}finally{restoreClip()}}
    }finally{this.#backend.setRenderTarget(null);this.#backend.autoClear=true;this.#scene.background=background;this.#effects.visible=effectsVisible;this.#particles.visible=particlesVisible;if(this.#active.skyGroup)this.#active.skyGroup.visible=skyVisible;this.#setWorldVisibility(frame.visibility!.surfaces);this.#active.projectedMarks.forEach((mark,index)=>mark.mesh.visible=markVisibility[index]!);this.#active.waterMeshes.forEach((water,index)=>water.mesh.visible=waterVisibility[index]!);this.#setCamera(frame.camera)}
    if(!completed.includes("main"))throw new RenderingError("MalformedInput","Water view plan omitted the main pass")
    return Object.freeze({passes:Object.freeze(completed),restored:this.#backend.autoClear&&this.#scene.background===background&&this.#effects.visible===effectsVisible&&this.#particles.visible===particlesVisible})
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

  #writeParticlePositions(item: ParticleItem, camera: Camera, positions: Float32Array, offset: number): void {
    if (item.primitive === "trail") {
      const delta = new THREE.Vector3().fromArray(item.trailEndPosition).sub(new THREE.Vector3().fromArray(item.position))
      const center = new THREE.Vector3().fromArray(item.position)
      const tangent = center.clone().sub(new THREE.Vector3().fromArray(camera.position)).cross(delta).normalize()
      const halfWidth = item.trailWidth * 0.5
      const vertices = [center.clone().addScaledVector(tangent, halfWidth), center.clone().addScaledVector(tangent, -halfWidth)]
      vertices.push(vertices[1]!.clone().add(delta), vertices[0]!.clone().add(delta))
      vertices.forEach((value, index) => value.toArray(positions, offset + index * 3))
    } else {
      const yaw = THREE.MathUtils.degToRad(camera.yawDegrees), pitch = THREE.MathUtils.degToRad(camera.pitchDegrees)
      const right = new THREE.Vector3(Math.sin(yaw), -Math.cos(yaw), 0)
      const up = new THREE.Vector3(Math.sin(pitch) * Math.cos(yaw), Math.sin(pitch) * Math.sin(yaw), Math.cos(pitch))
      const cosine = Math.cos(item.rollRadians), sine = Math.sin(item.rollRadians)
      const rolledRight = right.clone().multiplyScalar(cosine).addScaledVector(up, sine)
      const rolledUp = up.clone().multiplyScalar(cosine).addScaledVector(right, -sine)
      const center = new THREE.Vector3().fromArray(item.position)
      const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const
      corners.forEach(([x, y], index) => center.clone().addScaledVector(rolledRight, x * item.radius).addScaledVector(rolledUp, y * item.radius).toArray(positions, offset + index * 3))
    }
  }

  #createParticleBatchGeometry(capacity:number):THREE.BufferGeometry{
    const geometry=new THREE.BufferGeometry(),dynamic=(array:Float32Array,size:number)=>new THREE.BufferAttribute(array,size).setUsage(THREE.DynamicDrawUsage),indices=capacity*4>0xffff?new Uint32Array(capacity*6):new Uint16Array(capacity*6)
    for(let index=0;index<capacity;index+=1){const vertex=index*4;indices.set([vertex,vertex+1,vertex+2,vertex,vertex+2,vertex+3],index*6)}
    geometry.setAttribute("position",dynamic(new Float32Array(capacity*12),3));geometry.setAttribute("uv",dynamic(new Float32Array(capacity*8),2));geometry.setAttribute("particleUvNext",dynamic(new Float32Array(capacity*8),2));geometry.setAttribute("particleSheetBlend",dynamic(new Float32Array(capacity*4),1));geometry.setAttribute("particleColor",dynamic(new Float32Array(capacity*16),4));geometry.setIndex(new THREE.BufferAttribute(indices,1));return geometry
  }

  #updateParticleBatchGeometry(geometry:THREE.BufferGeometry,items:readonly ParticleItem[],camera:Camera):void{
    const positions=(geometry.getAttribute("position") as THREE.BufferAttribute).array as Float32Array,uv=(geometry.getAttribute("uv") as THREE.BufferAttribute).array as Float32Array,uvNext=(geometry.getAttribute("particleUvNext") as THREE.BufferAttribute).array as Float32Array,sheetBlend=(geometry.getAttribute("particleSheetBlend") as THREE.BufferAttribute).array as Float32Array,colors=(geometry.getAttribute("particleColor") as THREE.BufferAttribute).array as Float32Array
    for(const [index,item] of items.entries()){
      this.#writeParticlePositions(item,camera,positions,index*12)
      const sample=item.primarySheet!,current=sample.current[0]!,next=sample.next[0]!,currentCorners=[[current[0],current[1]],[current[2],current[1]],[current[2],current[3]],[current[0],current[3]]] as const,nextCorners=[[next[0],next[1]],[next[2],next[1]],[next[2],next[3]],[next[0],next[3]]] as const,red=((item.color>>16)&255)/255,green=((item.color>>8)&255)/255,blue=(item.color&255)/255
      for(let vertex=0;vertex<4;vertex+=1){const uvOffset=index*8+vertex*2,colorOffset=index*16+vertex*4,currentCorner=currentCorners[vertex]!,nextCorner=nextCorners[vertex]!;uv[uvOffset]=currentCorner[0];uv[uvOffset+1]=currentCorner[1];uvNext[uvOffset]=nextCorner[0];uvNext[uvOffset+1]=nextCorner[1];sheetBlend[index*4+vertex]=sample.blend;colors[colorOffset]=red;colors[colorOffset+1]=green;colors[colorOffset+2]=blue;colors[colorOffset+3]=item.opacity}
    }
    for(const name of ["position","uv","particleUvNext","particleSheetBlend","particleColor"])(geometry.getAttribute(name) as THREE.BufferAttribute).needsUpdate=true
    geometry.setDrawRange(0,items.length*6)
  }

  #clearParticleBatches():void{
    for(const retained of this.#particleBatchMeshes){this.#particles.remove(retained.mesh);retained.mesh.geometry.dispose()}
    this.#particleBatchMeshes=[];this.#particleBatchCount=0
  }

  #stageParticleBatches(items:readonly ParticleItem[],camera:Camera,factor:(value:ParticleItem["blendSource"])=>THREE.BlendingDstFactor):void{
    const ranges=particleBatchRanges(items);this.#particleBatchCount=ranges.length
    for(const [index,batch] of ranges.entries()){
      const values=items.slice(batch.start,batch.end),first=values[0]!,key=`${first.material.toLowerCase()}\0${first.blendSource}\0${first.blendDestination}`,required=values.length;let retained=this.#particleBatchMeshes[index]
      if(!retained||retained.key!==key||retained.capacity<required){if(retained){this.#particles.remove(retained.mesh);retained.mesh.geometry.dispose()}let capacity=1;while(capacity<required)capacity*=2;const geometry=this.#createParticleBatchGeometry(capacity),materialKey=key;let material=this.#active!.particleBatchMaterials.get(materialKey);if(!material){material=this.#active!.particleMaterials.get(first.material.toLowerCase())!.clone();material.blending=THREE.CustomBlending;material.blendSrc=factor(first.blendSource);material.blendDst=factor(first.blendDestination);material.transparent=true;this.#active!.particleBatchMaterials.set(materialKey,material)}const mesh=new THREE.Mesh(geometry,material);mesh.frustumCulled=false;this.#particles.add(mesh);retained={key,capacity,mesh};this.#particleBatchMeshes[index]=retained}
      this.#updateParticleBatchGeometry(retained.mesh.geometry,values,camera);retained.mesh.renderOrder=batch.start;retained.mesh.userData.stableTieIdentities=Object.freeze(values.map(item=>item.stableTieIdentity));retained.mesh.visible=true
    }
    while(this.#particleBatchMeshes.length>ranges.length){const retained=this.#particleBatchMeshes.pop()!;this.#particles.remove(retained.mesh);retained.mesh.geometry.dispose()}
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
    const frameState = item.pose?.viewmodel
    if (!frameState) throw new RenderingError("MalformedInput", "complete viewmodel frame state is missing")
    const encoded = this.#active!.loadRequest.modelFacing?.get(item.model.toLowerCase())
    if (!encoded) throw new RenderingError("MissingInput", `StudioModel facing ${item.model} is unavailable`)
    const expectedFront = frameState.reflected
      ? encoded.frontFace === "clockwise" ? "counter-clockwise" : "clockwise"
      : encoded.frontFace
    if (frameState.cullFace !== encoded.cullFace || frameState.frontFace !== expectedFront) {
      throw new RenderingError("IdentityMismatch", "viewmodel effective facing differs from encoded facing and reflection")
    }
    sourceTransform(retained.instance, item.position, item.angles!)
    retained.instance.scale.set(item.scale, frameState.reflected ? -item.scale : item.scale, item.scale)
    retained.instance.visible = frameState.drawDisposition === "draw"
    const projection = item.viewModelProjection!
    if (projection.optionalViewSpaceYReflection !== frameState.reflected) {
      throw new RenderingError("IdentityMismatch", "viewmodel reflection state differs")
    }
    this.#viewCamera.fov = sourceHorizontal4By3FovToVertical(projection.horizontalFov4By3)
    this.#viewCamera.near = projection.near
    this.#viewCamera.far = frame.camera.far
    this.#viewCamera.updateProjectionMatrix()
    this.#viewCamera.projectionMatrixInverse.copy(this.#viewCamera.projectionMatrix).invert()
    return sourceViewportDepthRange(projection.depthRange)
  }

  #stageDynamicItems(frame: Frame): readonly [number, number] | undefined {
    const effects = new THREE.Group()
    for(const instance of this.#active!.modelOccurrenceInstances.values())instance.visible=true
    const activeViewModels = new Set<number>()
    let viewModelDepthRange: readonly [number, number] | undefined
    try {
      const factor=(value:ParticleItem["blendSource"])=>value==="zero"?THREE.ZeroFactor:value==="one"?THREE.OneFactor:value==="source-alpha"?THREE.SrcAlphaFactor:THREE.OneMinusSrcAlphaFactor
      const particleItems=frame.particles??[];for(const item of particleItems)if(item.secondarySheet)throw new RenderingError("UnsupportedFeature","dual-sequence Particle rendering requires exact selected material support")
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
        const staticInstance=this.#active!.modelOccurrenceInstances.get(item.identity);if(staticInstance)staticInstance.visible=false
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
      this.#stageParticleBatches(particleItems,frame.camera,factor)
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
    const prior = Object.freeze({
      cssWidth: this.#cssWidth,
      cssHeight: this.#cssHeight,
      devicePixelRatio: this.#devicePixelRatio,
      viewportWidth: this.#viewportWidth,
      viewportHeight: this.#viewportHeight,
      suspended: this.#suspended,
      aspect: this.#camera.aspect,
      physicalWidth: Math.floor(this.#cssWidth * this.#devicePixelRatio),
      physicalHeight: Math.floor(this.#cssHeight * this.#devicePixelRatio),
    })
    try {
      if(this.#active&&width>0&&height>0){this.#active.reflectionTarget.setSize(width,height);this.#active.refractionTarget.setSize(width,height)}
      this.#backend.setPixelRatio(devicePixelRatio)
      this.#backend.setSize(cssWidth, cssHeight, false)
      this.#viewportWidth = cssWidth
      this.#viewportHeight = cssHeight
      this.#cssWidth = cssWidth
      this.#cssHeight = cssHeight
      this.#devicePixelRatio = devicePixelRatio
      this.#camera.aspect = cssHeight === 0 ? 1 : cssWidth / cssHeight
      this.#viewCamera.aspect = this.#camera.aspect
      this.#camera.updateProjectionMatrix()
      this.#viewCamera.updateProjectionMatrix()
      this.#suspended = width === 0 || height === 0
      this.#pacing.suspend(this.#suspended)
      this.#attachmentGeneration += 1
    } catch (error) {
      if(this.#active&&prior.physicalWidth>0&&prior.physicalHeight>0){this.#active.reflectionTarget.setSize(prior.physicalWidth,prior.physicalHeight);this.#active.refractionTarget.setSize(prior.physicalWidth,prior.physicalHeight)}
      this.#backend.setPixelRatio(prior.devicePixelRatio)
      this.#backend.setSize(prior.cssWidth,prior.cssHeight,false)
      this.#viewportWidth=prior.viewportWidth;this.#viewportHeight=prior.viewportHeight;this.#cssWidth=prior.cssWidth;this.#cssHeight=prior.cssHeight;this.#devicePixelRatio=prior.devicePixelRatio;this.#suspended=prior.suspended;this.#camera.aspect=prior.aspect;this.#viewCamera.aspect=prior.aspect;this.#camera.updateProjectionMatrix();this.#viewCamera.updateProjectionMatrix()
      throw new RenderingError("GpuValidationFailed",`renderer attachment replacement failed: ${String(error)}`)
    }
    return Object.freeze({ width, height, suspended: this.#suspended, deviceGeneration: this.#deviceGeneration, attachmentGeneration:this.#attachmentGeneration })
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
    this.#pacing.start()
    const tick = (timestamp: number) => {
      this.#pacingHandle = undefined
      if (!this.#pacingCallback || this.#lifecycle === "Disposed" || this.#lifecycle === "Failed") return
      void this.#pacing.offer(timestamp,async()=>{
        const frame=await this.#pacingCallback?.(timestamp)
        if(frame)await this.render(frame)
        const submitMilliseconds=performance.now()
        await this.#backend.backend.device?.queue.onSubmittedWorkDone()
        return Object.freeze({submitMilliseconds,presentMilliseconds:performance.now()})
      }).catch(()=>{this.#pacingCallback=undefined;this.#pacing.stop()})
      this.#pacingHandle = requestAnimationFrame(tick)
    }
    this.#pacingHandle = requestAnimationFrame(tick)
  }

  stopFramePacing(): void {
    this.#pacingCallback = undefined
    this.#pacing.stop()
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
    this.#clearParticleBatches()
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
    this.#clearParticleBatches()
    this.#clearDynamic(this.#viewModels)
    this.#viewModelInstances.clear()
    const active = this.#active
    this.#active = undefined
    this.#world.clear()
    if (active) await this.#retire(active)
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
