import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import {
  ExposureController,
  SOURCE_LDR,
  SOURCE_PC_INTEGER_HDR,
  sourceShaderGammaToLinear,
  validateRenderConfiguration,
  type CanvasAlphaMode,
  type CanvasFormat,
  type ExposureConfiguration,
  type ExposureSnapshot,
  type OutputColorSpace,
  type RenderConfiguration,
  type ToneOperator,
} from "./color-output"
import { applyParticleDepthState, configureWorldLightmap, worldMaterialSide } from "./material-state"
import { projectedDecalDepthBias, projectedDecalReceiverIsValid } from "./decal-occlusion"
import { OwnedResourceGeneration } from "./resource-generation"
import { FramePacingController, type FramePacingRecord } from "./frame-pacing"
import { fillParticleBatchRanges, type MutableParticleBatchRange } from "./particle-batches"
import { writeParticleQuad } from "./particle-geometry"
import { installOrderedWebGpuBundles, type OrderedBundleBackend } from "./ordered-webgpu-bundles"
import { RetainedLeafVisibility, RetainedVisibilityError, RetainedWorldVisibility } from "./retained-visibility"
import { RetainedStaticSceneGroup } from "./static-scene-group"
import { distanceFadeOpacity, quantizeStaticPropOpacity, screenFadeOpacity } from "./static-prop-fade"
import { executeViewModelDepthPhase } from "./viewmodel-depth-phase"
import { selectDiagnosticModelBase } from "./diagnostic-model"
import { sourceHorizontal4By3FovToVertical, sourceViewportDepthRange } from "./source-camera"
import {
  validateDynamicLights,
  validateShadows,
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
  createSourceViewFogUniforms,
  createSourceWaterFogUniforms,
  createSourceWaterMaterial,
  sourceViewFogNode,
  sourceWaterFogFragment,
  type SourceWaterFogUniforms,
  type SourceWaterShaderState,
} from "./source-water"
import { sourceWaterTangentAttributes } from "./source-water-geometry"
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
  sourceShaderGammaToLinear,
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
  SourceWaterError,
  createSourceWaterFogUniforms,
  createSourceWaterMaterial,
  evaluateSourceWaterPixel,
  sourceWaterFogAlpha,
  sourceWaterProjectiveCoordinates,
  type SourceWaterFogAlphaRequest,
  type SourceWaterFogUniforms,
  type SourceWaterGpuInput,
  type SourceWaterGpuMaterial,
  type SourceWaterPixel,
  type SourceWaterPixelRequest,
  type SourceWaterPixelResult,
  type SourceWaterSampler,
  type SourceWaterShaderState,
} from "./source-water"
export {
  sourceWaterTangentAttributes,
  type SourceWaterGeometryInput,
  type SourceWaterTangentAttributes,
} from "./source-water-geometry"
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
export type FogInput=Readonly<{enabled:boolean;radial:boolean;primary:readonly[number,number,number,number];start:number;end:number;maximumDensity:number;farZ:number|null}>
export type VisibilityFrame=Readonly<{worldIdentity:string;cacheIdentity:string;outsideWorld:boolean;sky:0|1|2;eyeLeaf:number|null;leaves:readonly number[];areas:readonly number[];surfaces:Uint32Array;water:WaterFramePlan}>

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
  visibility?:VisibilityFrame
  fog?:FogInput
  sky3d?:Readonly<{camera:Camera;visibility:VisibilityFrame;fog:FogInput}>
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
  identity: string
  mapMaterial: number | null
  shader: "dx90" | "dx9-hdr"
  opacity: "opaque" | "translucent"
  textures: readonly Readonly<{
    role: number
    disposition: "source" | "environment" | "render-target"
    colorRead: "srgb" | "linear" | "format-dependent"
    parameter: string
    reference: string
    logicalPath: string | null
  }>[]
  bottomMaterial: string | null
  underwaterOverlay: string | null
  baseFrame: EffectiveInput<number>
  normalFrame: EffectiveInput<number>
  environmentFrame: EffectiveInput<number>
  normalTransform: Readonly<{
    parameter: string
    matrix: Float32Array
    origin: EffectiveInput<number>["origin"]
    proxyMutated: boolean
  }>
  scale: EffectiveInput<readonly [number, number]>
  time: EffectiveInput<number>
  waterDepth: EffectiveInput<number>
  aboveWater: EffectiveInput<boolean>
  reflectAmount: EffectiveInput<number>
  refractAmount: EffectiveInput<number>
  reflectTint: EffectiveInput<readonly [number, number, number]>
  refractTint: EffectiveInput<readonly [number, number, number]>
  reflectionBlendFactor: EffectiveInput<number>
  fog: Readonly<{
    enabled: EffectiveInput<boolean> | null
    color: EffectiveInput<readonly [number, number, number]>
    start: EffectiveInput<number>
    end: EffectiveInput<number>
  }>
  cheapStart: EffectiveInput<number>
  cheapEnd: EffectiveInput<number>
  forceCheap: EffectiveInput<boolean>
  forceExpensive: EffectiveInput<boolean>
  reflectEntities: EffectiveInput<boolean>
  blurRefraction: EffectiveInput<boolean>
  noLowEndLightmap: EffectiveInput<boolean>
  scroll: readonly [
    EffectiveInput<readonly [number, number, number]>,
    EffectiveInput<readonly [number, number, number]>,
  ]
  fresnel: Readonly<{ cheapEnabled: boolean; expensiveConstant: readonly [number, number, number, number] }>
  requiredInputs: readonly number[]
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
    targetFaces: readonly number[]
    renderOrder: number
    normalOffset: number
    polygonOffset: "none" | "decal"
  }>[]
  textures: readonly EnvironmentTextureInput[]
  collisionWorldIdentity: string
  receiverSnapshotRevision: bigint
  placementRevision: bigint
  authoredTextures: ReadonlyMap<string, AuthoredTextureInput>
  waterSurfaceFacts: readonly Readonly<{ face:number; model:number; material:number; selected:boolean; plane:readonly[number,number,number,number]; bindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>}>[]
  waterVolumeFacts: readonly Readonly<{index:number;surfaceZ:number;minimumZ:number;bounds:readonly[readonly[number,number,number],readonly[number,number,number]];surfaceMaterial:number;bottomMaterial:unknown;leaves:readonly number[];clusters:readonly number[];areas:readonly number[];contents:number;plane:readonly[number,number,number,number];surfaceTranslucent:boolean;bottomTranslucent:boolean|null;surfaceBindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>;bottomBindings:Readonly<{environment:boolean;reflection:boolean;refraction:boolean}>|null}>[]
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
  shader: "unlit-generic" | "vertex-lit-generic" | "eye-refract" | "eyes"
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
  state: Readonly<{ kind: "unlit-generic" | "vertex-lit-generic" | "eye-refract" | "eyes" }>
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
  sourceFormat: number | null
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
export type StaticPropInput=Readonly<{
  count:number;source:Uint32Array;dictionaryModel:Uint32Array;presentationModel:Uint32Array;transform:Float32Array;skin:Int32Array;body:Uint32Array;lod:Uint32Array;fades:Float32Array;flags:Uint32Array;solidity:Uint8Array;ownership:Uint8Array;lightingKind:Uint8Array;lightingOrigin:Float32Array;leafOffsets:Uint32Array;leaves:Uint16Array;areas:Uint16Array;vhvObjects:Uint32Array;runtimeAmbient:Float32Array;runtimeLightOffsets:Uint32Array;runtimeLights:readonly Readonly<{source:number;kind:number;style:number;ratio:number;direction:readonly[number,number,number];intensity:readonly[number,number,number]}>[];models:readonly string[];vhv:readonly Readonly<{occurrence:number;model:number;profile:0|1;vertexCount:number;meshes:readonly Readonly<{primitive:number;lod:number;vertexCount:number;colors:Uint8Array}>[]}>[]
}>

export type MapLoadRequest = Readonly<{
  payload: Uint8Array
  payloadSha256: string
  lightStyles?: readonly Readonly<{ style: number; scalar: number }>[]
  directionalTextures?: readonly DirectionalTextureInput[]
  environment?: EnvironmentInput
  materialStates?: ReadonlyMap<string, MaterialStateInput>
  particleTextures?: readonly EnvironmentTextureInput[]
  modelOccurrences?: readonly Readonly<{ entity: number; model: string; matrix: Float32Array }>[]
  modelFacing?: ReadonlyMap<string, Readonly<{ frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>>
  modelMaterials?: ReadonlyMap<string, ModelMaterialInput>
  authoredTextures?: ReadonlyMap<string, AuthoredTextureInput>
  brushModels?:readonly Readonly<{index:number;surfaceRange:readonly[number,number];vertexCount:number;triangleCount:number;materials:readonly number[]}>[]
  modelDrawInputs?: readonly Readonly<{ entity: number; lighting: ModelLightingInput; eyes: readonly ModelEyeState[] }>[]
  staticProps?:StaticPropInput
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
  staticProps:Readonly<{total:number;main:number;sky3d:number;runtimeLit:number}>
  runtimeStaticProps:readonly Readonly<{source:number;origin:readonly[number,number,number];lightingOrigin:readonly[number,number,number];radius:number}>[]
  displacements: readonly Readonly<{
    source: number
    face: number
    material: string
    positions: Float32Array
    normals: Float32Array
    indices: Uint32Array
    bounds: readonly [readonly [number, number, number], readonly [number, number, number]]
    submittedTriangles: number
    cull: "back" | "none"
    depthTest: boolean
    depthWrite: boolean
    blend: boolean
    lighting: Readonly<{ kind: string; styles: readonly number[]; sampleStart: number; samplesPerLayer: number; layers: number }>
  }>[]
}>

export type FrameCapture = Readonly<{
  format: "image/png"
  sha256: string
  bytes: Uint8Array
}>

export type WaterPassTiming = Readonly<{
  kind: WaterFramePass["kind"]
  visibilityMilliseconds: number
  sceneMilliseconds: number
  clippingMilliseconds: number
  renderMilliseconds: number
  totalMilliseconds: number
}>

export type FrameResult = Readonly<{
  deviceGeneration: number
  sceneGeneration: number
  submission: null
  exposure: ExposureSnapshot
  visibleProjectedMarks: number
  waterPasses: readonly ("reflection"|"refraction"|"main"|"intersection")[]
  waterPassTimings: readonly WaterPassTiming[]
  waterStateRestored: boolean
  sky3dPass?:Readonly<{phases:readonly["sky3d","depth-reset","main","restore"];skySurfaces:number;skyProps:number;mainProps:number;visibleSkyPropSources:readonly number[];fog:Readonly<{start:number;end:number;primary:readonly[number,number,number,number]}>;stateRestored:boolean}>
  visibleMainStaticPropSources:readonly number[]
  runtimeStaticPropScreen:readonly Readonly<{source:number;x:number;y:number;width:number;height:number}>[]
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
    worldDepthCleared: boolean
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

export type GeometryEvidence = Readonly<{
  sceneGeneration: number
  samples: readonly Readonly<{
    x: number
    y: number
    disposition: "main-world" | "background"
    depth: number | null
    primitive: number | null
    object: number | null
    material: string | null
  }>[]
}>

export type WaterTargetEvidence = Readonly<{
  x: number
  y: number
  reflection: readonly [number, number, number, number]
  refraction: readonly [number, number, number, number]
}>

export interface Renderer {
  readonly configuration: RenderConfiguration
  readonly lifecycle: RendererLifecycle
  readonly deviceGeneration: number
  readonly sceneGeneration: number
  loadMap(request: MapLoadRequest): Promise<SceneResult>
  render(frame: Frame): Promise<FrameResult>
  captureGeometryEvidence(camera: Camera): GeometryEvidence
  captureWaterTargetEvidence(x: number, y: number): Promise<WaterTargetEvidence>
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
type WaterMaterialResource = Readonly<{
  material: THREE.MeshBasicNodeMaterial
  cheapMaterial: THREE.MeshBasicNodeMaterial | null
  normalFrames: readonly THREE.Texture[]
  normalNode: ReturnType<typeof TSL.texture>
  cheapNormalNode: ReturnType<typeof TSL.texture> | null
  fog: THREE.Fog | null
  state: WaterMaterialInput
}>
type WorldBatchResource = Readonly<{
  mesh: THREE.Mesh
  faces: Uint32Array
  sourceIndices: Uint32Array
  index: THREE.BufferAttribute
  transparent: boolean
}>
type ProjectedMarkResource = Readonly<{
  mesh: THREE.Mesh
  face: number
  sourceIndex: number
  visibility: EnvironmentFragmentInput["visibility"]
}>
type StaticPropResource = Readonly<{
  object: THREE.Group
  source: number
  ownership: 0 | 1
  leaves: Uint16Array
  origin: readonly [number, number, number]
  lightingOrigin: readonly [number, number, number] | null
  flags: number
  fadeMinimum: number
  fadeMaximum: number
  forcedFadeScale: number
  radius: number
  bounds: readonly [number, number, number, number, number, number]
  fadeUniform: ReturnType<typeof TSL.uniform>
}>

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
  waterFogUniforms: SourceWaterFogUniforms
  diagnostics: readonly SceneDiagnostic[]
  worldBundle: THREE.BundleGroup
  skyWorldBundle: THREE.BundleGroup
  mainTransparentWorld: THREE.Group
  skyTransparentWorld: THREE.Group
  worldBatches: readonly WorldBatchResource[]
  skyWorldBatches: readonly WorldBatchResource[]
  worldVisibility: RetainedWorldVisibility
  skyWorldVisibility: RetainedWorldVisibility
  modelLookup: ReadonlyMap<string, RuntimeMap["models"][number]>
  projectedMarksByFace: ReadonlyMap<number, readonly ProjectedMarkResource[]>
  brushProjectedMarks: readonly ProjectedMarkResource[]
  leafVisibility: RetainedLeafVisibility
  runtimeStaticPropInstances: readonly StaticPropResource[]
  projectedMarks: readonly ProjectedMarkResource[]
  waterMeshes: readonly WaterMeshResource[]
  waterMaterials: ReadonlyMap<string,WaterMaterialResource>
  cubemapTextures: ReadonlyMap<number,THREE.CubeTexture>
  skyGroup: THREE.Group | null
  mainStaticProps:THREE.Group
  skyStaticProps:THREE.Group
  staticPropInstances: readonly StaticPropResource[]
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
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
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
function runtimeStaticLightingNode(map:RuntimeMap,input:StaticPropInput,index:number):any{
  if(map.lighting.profile!=="hdr")throw new RenderingError("MissingInput","runtime static-prop lighting requires HDR world lights")
  const normal=TSL.normalWorld.normalize(),cube=Array.from({length:6},(_,side)=>TSL.vec3(...input.runtimeAmbient.subarray(index*18+side*3,index*18+side*3+3) as unknown as [number,number,number]))
  let lighting=normal.x.lessThan(0).select(cube[1],cube[0]).mul(normal.x.mul(normal.x)).add(normal.y.lessThan(0).select(cube[3],cube[2]).mul(normal.y.mul(normal.y))).add(normal.z.lessThan(0).select(cube[5],cube[4]).mul(normal.z.mul(normal.z)))
  for(let at=input.runtimeLightOffsets[index]!;at<input.runtimeLightOffsets[index+1]!;at++){
    const retained=input.runtimeLights[at]!,light=map.lighting.descriptor.worldLights[retained.source]
    if(!light||light.kind!==retained.kind||light.style!==retained.style)throw new RenderingError("IdentityMismatch","runtime static-prop world-light identity differs")
    const delta=TSL.vec3(...light.origin).sub(TSL.positionWorld),distance=delta.length(),direction=delta.normalize()
    let attenuation:any
    if(light.kind===3)attenuation=TSL.float(1)
    else if(light.kind===0)attenuation=distance.mul(distance).max(1).reciprocal().mul(direction.dot(TSL.vec3(...light.normal)).negate().max(0))
    else if(light.kind===4)attenuation=TSL.float(light.linearAttenuation).sub(distance).max(0)
    else attenuation=TSL.float(light.constantAttenuation).add(distance.mul(light.linearAttenuation)).add(distance.mul(distance).mul(light.quadraticAttenuation)).reciprocal()
    if(light.kind===2){const cone=direction.dot(TSL.vec3(...light.normal)).negate(),spread=Math.max(Number.EPSILON,light.stopDot-light.stopDot2),factor=cone.sub(light.stopDot2).div(spread).clamp(0,1);attenuation=attenuation.mul(light.exponent===0||light.exponent===1?factor:factor.pow(light.exponent))}
    const lightDirection=light.kind===3?TSL.vec3(...light.normal).negate():direction,diffuse=normal.dot(lightDirection).mul(0.5).add(0.5).clamp(0,1).pow(2)
    lighting=lighting.add(TSL.vec3(...light.intensity).mul(attenuation).mul(diffuse))
  }
  return lighting
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

function disposeScene(scene: Pick<SceneResources,"group"|"disposables"|"modelTemplates"|"modelOccurrenceInstances"|"disposed"> & Partial<Pick<SceneResources,"particleBatchMaterials">>): void {
  if (scene.disposed) return
  scene.disposed = true
  scene.group.clear()
  scene.disposables.dispose()
  scene.modelTemplates.clear()
  scene.modelOccurrenceInstances.clear()
  for(const material of scene.particleBatchMaterials?.values()??[])material.dispose()
  scene.particleBatchMaterials?.clear()
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

function textureFromAuthored(input: AuthoredTextureInput, colorSpace: string, frame = 0): THREE.Texture {
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
  const data = (plane: AuthoredTextureInput["planes"][number]) => {
    if (input.scalarEncoding === "f16") return new Uint16Array(plane.rgba.slice().buffer)
    return plane.rgba
  }
  const base = planes[0]!
  const compressedFormat = input.sourceFormat === null ? null : new Map<number, THREE.CompressedPixelFormat>([
    [13, THREE.RGBA_S3TC_DXT1_Format],
    [20, THREE.RGBA_S3TC_DXT1_Format],
    [14, THREE.RGBA_S3TC_DXT3_Format],
    [15, THREE.RGBA_S3TC_DXT5_Format],
  ]).get(input.sourceFormat)
  if (input.sourceFormat !== null && ![0,1,11,12,16,24].includes(input.sourceFormat) && compressedFormat === undefined) throw new RenderingError("UnsupportedFeature", `authored texture ${input.logicalPath} has unsupported source format ${input.sourceFormat}`)
  const mipmaps = planes.map((plane) => Object.freeze({ data: data(plane), width: plane.width, height: plane.height }))
  const texture = compressedFormat === null || compressedFormat === undefined
    ? new THREE.DataTexture(data(base), base.width, base.height, THREE.RGBAFormat, input.scalarEncoding === "u8" ? THREE.UnsignedByteType : THREE.HalfFloatType)
    : new THREE.CompressedTexture(mipmaps, base.width, base.height, compressedFormat, THREE.UnsignedByteType)
  texture.mipmaps = mipmaps
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
  const data=(plane:AuthoredTextureInput["planes"][number])=>input.scalarEncoding==="u8"?plane.rgba:new Uint16Array(plane.rgba.slice().buffer)
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
  const bias = projectedDecalDepthBias(state?.polygonOffset === 1 ? "decal" : "none")
  const transparent = state?.blendEnabled ?? (resolved.features & 1) !== 0
  return {
    transparent,
    blending: state?.blendEnabled ? THREE.CustomBlending : transparent ? THREE.NormalBlending : THREE.NoBlending,
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

function sourceFragmentColor(
  sample: any,
  state?: MaterialStateInput,
  waterFogUniforms?: SourceWaterFogUniforms,
): any {
  const alpha = sample.a.mul(state?.alphaModulation ?? 1)
  const authored = TSL.vec4(sample.rgb, alpha)
  const color = waterFogUniforms && !state?.blendEnabled
    ? sourceWaterFogFragment(authored, waterFogUniforms)
    : authored
  if (state?.fragmentDiscard.kind !== "alpha") return color
  return TSL.Fn(() => {
    const rejected = state.fragmentDiscard.pass === "greater"
      ? alpha.lessThanEqual(state.fragmentDiscard.reference)
      : alpha.lessThan(state.fragmentDiscard.reference)
    rejected.discard()
    return color
  })()
}

function detailColor(base: any, detail: RuntimeMaterial["detail"], texture?: THREE.DataTexture): any {
  if (!detail || !texture) return base
  if (detail.blendMode !== 0) throw new RenderingError("UnsupportedFeature", `detail blend mode ${detail.blendMode} is unavailable`)
  const uv = TSL.uv()
  const sample = TSL.texture(texture, TSL.vec2(uv.x.mul(detail.scale[0]), uv.y.mul(detail.scale[1])))
  const tint = TSL.vec3(detail.tint[0], detail.tint[1], detail.tint[2])
  const modulation = TSL.mix(TSL.vec3(1), sample.rgb.mul(tint).mul(2), detail.blendFactor)
  return TSL.vec4(base.rgb.mul(modulation), base.a)
}

function worldBaseColor(resolved: RuntimeMaterial, base: any, second?: THREE.DataTexture): any {
  if (resolved.shader !== 4) return base
  if (!second) throw new RenderingError("MissingInput", `WorldVertexTransition second texture ${resolved.logicalPath} is unavailable`)
  const blend = TSL.clamp(TSL.attribute("displacementAlpha", "float").div(255), 0, 1)
  return TSL.vec4(TSL.mix(base.rgb, TSL.texture(second, TSL.uv()).rgb, blend), base.a)
}

function worldNodeMaterial(
  resolved: RuntimeMaterial,
  identity: string,
  baseTexture: THREE.DataTexture | undefined,
  secondTexture: THREE.DataTexture | undefined,
  detailTexture: THREE.DataTexture | undefined,
  lightmaps: readonly [THREE.DataTexture, THREE.DataTexture?, THREE.DataTexture?, THREE.DataTexture?],
  directional: THREE.DataTexture | undefined,
  directionalKind: "normal" | "ssbump" | undefined,
  directionalUvTransform: DirectionalTextureInput["uvTransform"] | undefined,
  exposure: ReturnType<typeof TSL.uniform>,
  waterFogUniforms: SourceWaterFogUniforms,
  state?: MaterialStateInput,
): THREE.MeshBasicNodeMaterial {
  const material = new THREE.MeshBasicNodeMaterial(materialOptions(resolved, state))
  const base = detailColor(worldBaseColor(resolved, baseTexture ? TSL.texture(baseTexture, TSL.uv()) : TSL.vec4(TSL.color(debugColor(identity)), 1), secondTexture), resolved.detail, detailTexture)
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
  material.colorNode = sourceFragmentColor(
    TSL.vec4(base.rgb.mul(irradiance).mul(exposure), base.a),
    state,
    waterFogUniforms,
  )
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
  readonly #viewFogUniforms = createSourceViewFogUniforms()
  #waterClipping = new THREE.ClippingGroup()
  #waterClipPlane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0)
  #world = new THREE.Group()
  #effects = new THREE.Group()
  #particles = new THREE.Group()
  #viewModels = new THREE.Group()
  #camera = new THREE.PerspectiveCamera(75, 1, 1, 32_768)
  #viewCamera = new THREE.PerspectiveCamera(41.114, 1, 1, 32_768)
  #viewModelInstances = new Map<number, { model: string; root: THREE.Group; instance: THREE.Group; meshes: THREE.Mesh[]; seen: number }>()
  #dynamicModelInstances = new Map<number, { model: string; instance: THREE.Group; meshes?: THREE.Mesh[]; seen: number }>()
  #dynamicBrushInstances = new Map<number, { model: number; appearance: string; instance: THREE.Group; seen: number }>()
  #dynamicRevision = 0
  #particleBatchMeshes: { key: string; capacity: number; mesh: THREE.Mesh }[] = []
  readonly #particleBatchRanges: MutableParticleBatchRange[] = []
  readonly #cameraDirection = new THREE.Vector3()
  readonly #cameraTarget = new THREE.Vector3()
  readonly #projectionPoint = new THREE.Vector3()
  readonly #fadeUp = new THREE.Vector3()
  readonly #fadeFirst = new THREE.Vector3()
  readonly #fadeSecond = new THREE.Vector3()
  #visibleStaticIndices: [Set<number>, Set<number>] = [new Set(), new Set()]
  #nextVisibleStaticIndices: [Set<number>, Set<number>] = [new Set(), new Set()]
  #visibleStaticSources: [readonly number[], readonly number[]] = [Object.freeze([]), Object.freeze([])]
  #visibleWorldMarkFaces = new Set<number>()
  #visibleProjectedMarkCount = 0
  #particleBatchCount=0
  #stagedDynamic?:Readonly<{
    particles:Frame["particles"]
    models:Frame["models"]
    brushModels:Frame["brushModels"]
    viewModelDepthRange:readonly[number,number]|undefined
  }>
  #worldVisibilitySurfaces?: Uint32Array
  #worldVisibilityIdentity?: string
  #skyWorldVisibilitySurfaces?: Uint32Array
  #skyWorldVisibilityIdentity?: string
  #restoreOrderedBundles?: () => void
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
    this.#scene.matrixAutoUpdate = false
    ;(this.#scene as THREE.Scene & { fogNode?: unknown }).fogNode = sourceViewFogNode(this.#viewFogUniforms)
    this.#waterClipping.matrixAutoUpdate = false
    this.#waterClipping.enabled = false
    this.#waterClipping.clippingPlanes = [this.#waterClipPlane]
    this.#world.matrixAutoUpdate = false
    this.#scene.background = null
    this.#waterClipping.add(this.#world, this.#effects, this.#particles)
    this.#scene.add(this.#waterClipping, this.#camera, this.#viewCamera)
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

  captureGeometryEvidence(camera: Camera): GeometryEvidence {
    if (!this.#active || this.#lifecycle !== "Ready") throw new RenderingError("InvalidState", "renderer geometry evidence is unavailable")
    this.#setCamera(camera)
    this.#world.updateMatrixWorld(true)
    const raycaster = new THREE.Raycaster()
    const meshes = this.#active.worldBatches.filter((batch) => batch.mesh.visible).map((batch) => batch.mesh)
    const samples = [] as GeometryEvidence["samples"][number][]
    for (const y of [-0.8, -0.4, 0, 0.4, 0.8]) {
      for (const x of [-0.8, -0.4, 0, 0.4, 0.8]) {
        raycaster.setFromCamera(new THREE.Vector2(x, y), this.#camera)
        const intersection = raycaster.intersectObjects(meshes, false)[0]
        if (!intersection || intersection.faceIndex === undefined) {
          samples.push(Object.freeze({ x, y, disposition: "background", depth: null, primitive: null, object: null, material: null }))
          continue
        }
        const object = this.#active.worldBatches.findIndex((batch) => batch.mesh === intersection.object)
        const batch = this.#active.worldBatches[object]!
        const current = batch.index.array as Uint32Array
        const at = intersection.faceIndex * 3
        let sourceTriangle = -1
        for (let triangle = 0; triangle < batch.sourceIndices.length / 3; triangle += 1) {
          const source = triangle * 3
          if (batch.sourceIndices[source] === current[at] && batch.sourceIndices[source + 1] === current[at + 1] && batch.sourceIndices[source + 2] === current[at + 2]) {
            sourceTriangle = triangle
            break
          }
        }
        if (sourceTriangle < 0) throw new RenderingError("InvalidState", "visible primitive identity is unavailable")
        samples.push(Object.freeze({
          x,
          y,
          disposition: "main-world",
          depth: intersection.distance,
          primitive: batch.faces[sourceTriangle]!,
          object,
          material: String(batch.mesh.userData.materialIdentity),
        }))
      }
    }
    return Object.freeze({ sceneGeneration: this.#sceneGeneration, samples: Object.freeze(samples) })
  }

  async captureWaterTargetEvidence(x: number, y: number): Promise<WaterTargetEvidence> {
    const scene = this.#active
    if (!scene || this.#lifecycle !== "Ready") {
      throw new RenderingError("InvalidState", "renderer Water target evidence is unavailable")
    }
    if (
      !Number.isSafeInteger(x) || !Number.isSafeInteger(y)
      || x < 0 || y < 0
      || x >= scene.reflectionTarget.width || y >= scene.reflectionTarget.height
    ) {
      throw new RenderingError("MalformedInput", "Water target evidence coordinates are invalid")
    }
    const [reflection, refraction] = await Promise.all([
      this.#backend.readRenderTargetPixelsAsync(scene.reflectionTarget, x, y, 1, 1),
      this.#backend.readRenderTargetPixelsAsync(scene.refractionTarget, x, y, 1, 1),
    ])
    if (reflection.length !== 4 || refraction.length !== 4) {
      throw new RenderingError("CaptureFailed", "Water target evidence does not contain exact RGBA pixels")
    }
    return Object.freeze({
      x,
      y,
      reflection: Object.freeze(Array.from(reflection)) as readonly [number, number, number, number],
      refraction: Object.freeze(Array.from(refraction)) as readonly [number, number, number, number],
    })
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
      this.#restoreOrderedBundles = installOrderedWebGpuBundles(backend.backend as unknown as OrderedBundleBackend)
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
      this.#restoreOrderedBundles?.()
      this.#restoreOrderedBundles = undefined
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
    const payload = request.payload
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
      if (mark.status === 0 && (!state || (mark.polygonOffset === "decal") !== (state.polygonOffset === 1)
        || !projectedDecalReceiverIsValid(mark))) {
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
        texture.planes.some((plane) => plane.width < 1 || plane.height < 1 || plane.rgba.length !== (texture.sourceFormat === null
          ? plane.width * plane.height * 4 * (texture.scalarEncoding === "u8" ? 1 : 2)
          : [2, 3].includes(texture.sourceFormat)
            ? plane.width * plane.height * 3
            : [0, 1, 11, 12, 16].includes(texture.sourceFormat)
              ? plane.width * plane.height * 4
            : texture.sourceFormat === 24
              ? plane.width * plane.height * 8
              : Math.max(1, Math.ceil(plane.width / 4)) * Math.max(1, Math.ceil(plane.height / 4)) * ([13, 20].includes(texture.sourceFormat) ? 8 : 16)))) {
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
    const staticProps=request.staticProps
    if(staticProps){const count=staticProps.count;if(!Number.isSafeInteger(count)||count<0||count>65536||staticProps.source.length!==count||staticProps.dictionaryModel.length!==count||staticProps.presentationModel.length!==count||staticProps.transform.length!==count*6||staticProps.skin.length!==count||staticProps.body.length!==count||staticProps.lod.length!==count||staticProps.fades.length!==count*3||staticProps.flags.length!==count||staticProps.solidity.length!==count||staticProps.ownership.length!==count||staticProps.lightingKind.length!==count||staticProps.lightingOrigin.length!==count*3||staticProps.leafOffsets.length!==count+1||staticProps.vhvObjects.length!==count*2||staticProps.runtimeAmbient.length!==count*18||staticProps.runtimeLightOffsets.length!==count+1||staticProps.models.length!==modelFacing.size||staticProps.source.some((value,index)=>index>0&&value<=staticProps.source[index-1]!)||staticProps.presentationModel.some(value=>value>=staticProps.models.length)||staticProps.ownership.some(value=>value>1)||staticProps.lightingKind.some(value=>value>1)||staticProps.leafOffsets[count]!==staticProps.leaves.length||staticProps.leaves.length!==staticProps.areas.length||staticProps.runtimeLightOffsets[count]!==staticProps.runtimeLights.length)throw new RenderingError("MalformedInput","static-prop artifact input is invalid")}
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
      const stagedWorld = this.#world.children.map((child) => Object.freeze({ child, visible: child.visible }))
      for (const { child } of stagedWorld) child.visible = false
      this.#world.add(staged.group)
      this.#world.updateMatrixWorld(true)
      try {
        await this.#prepareWaterPipelines(staged, request.environment, request.signal, ordinal)
      } finally {
        this.#world.remove(staged.group)
        for (const { child, visible } of stagedWorld) child.visible = visible
      }
      this.#checkAbort(request.signal, ordinal)

      const prior = this.#active
      this.#clearDynamic(this.#effects)
      this.#dynamicModelInstances.clear()
      this.#dynamicBrushInstances.clear()
      this.#clearParticleBatches()
      this.#clearDynamic(this.#viewModels)
      this.#viewModelInstances.clear()
      this.#stagedDynamic = undefined
      this.#worldVisibilitySurfaces = undefined
      this.#worldVisibilityIdentity = undefined
      this.#skyWorldVisibilitySurfaces = undefined
      this.#skyWorldVisibilityIdentity = undefined
      this.#visibleStaticIndices[0].clear()
      this.#visibleStaticIndices[1].clear()
      this.#nextVisibleStaticIndices[0].clear()
      this.#nextVisibleStaticIndices[1].clear()
      this.#visibleStaticSources = [Object.freeze([]), Object.freeze([])]
      this.#visibleWorldMarkFaces.clear()
      this.#visibleProjectedMarkCount = 0
      this.#world.clear()
      this.#world.add(staged.group)
      this.#world.updateMatrixWorld(true)
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

  async #prepareWaterPipelines(
    scene: SceneResources,
    environment: EnvironmentInput | undefined,
    signal: AbortSignal | undefined,
    ordinal: number,
  ): Promise<void> {
    if (!environment || scene.waterMeshes.length === 0 || environment.waterVolumeFacts.length === 0) return
    const volume = environment.waterVolumeFacts[0]!
    const depth = volume.surfaceZ - volume.minimumZ
    if (!Number.isFinite(depth) || depth <= 0 || !volume.bounds.flat().every(Number.isFinite)) {
      throw new RenderingError("MalformedInput", "Water pipeline preparation requires complete finite volume bounds")
    }
    const centerX = (volume.bounds[0][0] + volume.bounds[1][0]) * 0.5
    const centerY = (volume.bounds[0][1] + volume.bounds[1][1]) * 0.5
    this.#backend.initRenderTarget(scene.reflectionTarget)
    this.#backend.initRenderTarget(scene.refractionTarget)
    const initializedTextures = new Set<THREE.Texture>()
    for (const water of scene.waterMaterials.values()) {
      for (const texture of water.normalFrames) {
        if (initializedTextures.has(texture)) continue
        this.#checkAbort(signal, ordinal)
        this.#backend.initTexture(texture)
        initializedTextures.add(texture)
      }
    }

    const previousPosition = this.#camera.position.clone()
    const previousQuaternion = this.#camera.quaternion.clone()
    const previousFog = this.#scene.fog
    const previousTarget = this.#backend.getRenderTarget()
    const waterVisibility = scene.waterMeshes.map((water) => water.mesh.visible)
    try {
      for (const water of scene.waterMeshes) water.mesh.visible = true
      const combinations = [
        { target: scene.reflectionTarget, height: volume.surfaceZ + depth * 0.5, keep: "above" as const },
        { target: scene.refractionTarget, height: volume.surfaceZ + depth * 0.5, keep: "below" as const },
        { target: null, height: volume.surfaceZ + depth * 0.5, keep: null },
        { target: null, height: volume.surfaceZ - depth * 0.5, keep: "below" as const },
      ]
      for (const combination of combinations) {
        this.#checkAbort(signal, ordinal)
        this.#camera.position.set(centerX, centerY, combination.height)
        this.#camera.lookAt(centerX + depth, centerY, volume.surfaceZ)
        this.#camera.updateMatrixWorld()
        this.#backend.setRenderTarget(combination.target)
        const restore = this.#setClip(combination.keep === null
          ? null
          : { height: volume.surfaceZ, keep: combination.keep })
        try {
          await this.#backend.compileAsync(this.#scene, this.#camera)
        } finally {
          restore()
        }
      }
      this.#checkAbort(signal, ordinal)
    } finally {
      this.#backend.setRenderTarget(previousTarget)
      this.#camera.position.copy(previousPosition)
      this.#camera.quaternion.copy(previousQuaternion)
      this.#camera.updateMatrixWorld()
      this.#setSceneFog(previousFog as THREE.Fog | null)
      for (let index = 0; index < scene.waterMeshes.length; index += 1) {
        scene.waterMeshes[index]!.mesh.visible = waterVisibility[index]!
      }
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
    group.matrixAutoUpdate = false
    const worldBundle = new THREE.BundleGroup()
    worldBundle.matrixAutoUpdate = false
    const skyWorldBundle = new THREE.BundleGroup()
    skyWorldBundle.matrixAutoUpdate = false
    skyWorldBundle.visible = false
    const mainTransparentWorld = new THREE.Group()
    const skyTransparentWorld = new THREE.Group()
    skyTransparentWorld.visible = false
    group.add(worldBundle, mainTransparentWorld, skyWorldBundle, skyTransparentWorld)
    const mainStaticProps = new RetainedStaticSceneGroup()
    const skyStaticProps = new RetainedStaticSceneGroup()
    skyStaticProps.visible = false
    const staticPropInstances: StaticPropResource[] = []
    group.add(mainStaticProps, skyStaticProps)
    const modelTemplates = new Map<string, THREE.Group>()
    const modelOccurrenceInstances=new Map<number,THREE.Group>()
    const brushModelTemplates=new Map<number,THREE.Group>()
    const particleTextures = new Map<string, THREE.DataTexture>()
    const particleMaterials = new Map<string, THREE.MeshBasicNodeMaterial>()
    const materialStates = new Map(request.materialStates ?? [])
    const disposables = new OwnedResourceGeneration(this.#deviceGeneration, sceneGeneration)
    const diagnostics: SceneDiagnostic[] = []
    const worldBatches: WorldBatchResource[] = []
    const skyWorldBatches: WorldBatchResource[] = []
    const projectedMarks: ProjectedMarkResource[] = []
    const waterMeshes: WaterMeshResource[]=[]
    const targetWidth=Math.max(1,Number((this.#canvas as {width?:number}).width??1)),targetHeight=Math.max(1,Number((this.#canvas as {height?:number}).height??1))
    const reflectionTarget=disposables.add(new THREE.RenderTarget(targetWidth,targetHeight,{depthBuffer:true}))
    const refractionTarget=disposables.add(new THREE.RenderTarget(targetWidth,targetHeight,{depthBuffer:true}))
    reflectionTarget.texture.colorSpace=THREE.NoColorSpace
    refractionTarget.texture.colorSpace=THREE.NoColorSpace
    const waterMaterials = new Map<string, WaterMaterialResource>()
    const waterNormalFrames = new Map<string, readonly THREE.Texture[]>()
    const cubemapTextures = new Map<number, THREE.CubeTexture>()
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
    const waterFogUniforms = createSourceWaterFogUniforms()
    const directionalGpu = new Map<string, { input: DirectionalTextureInput; texture: THREE.DataTexture }>()
    const authoredGpu = new Map<string, THREE.Texture>()
    const modelDrawInputs = new Map((request.modelDrawInputs ?? []).map((input) => [input.entity, input] as const))
    const modelsRequiringLighting = new Set(map.models
      .filter((model) => model.materials.some((material) => request.modelMaterials?.get(material.logicalPath.toLowerCase())?.shader !== "unlit-generic"))
      .map((model) => model.logicalPath))
    const missingLightingEntities = map.modelOccurrences
      .filter((occurrence) => modelsRequiringLighting.has(map.models[occurrence.model]!.logicalPath) && !modelDrawInputs.has(occurrence.entity))
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

    const createModelBase = (identity: string): Readonly<{texture:THREE.Texture;input:AuthoredTextureInput}> | undefined => {
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
      const input = request.authoredTextures?.get(binding.logicalPath.toLowerCase())
      if (!input) throw new RenderingError("MissingInput", `authored model texture ${binding.logicalPath} is unavailable`)
      let texture = authoredGpu.get(key)
      if (!texture) {
        texture = textureFromAuthored(input, binding.colorRead === "srgb" ? THREE.SRGBColorSpace : THREE.NoColorSpace)
        authoredGpu.set(key, texture)
        disposables.add(texture)
      }
      return Object.freeze({texture,input})
    }
    const createBase = (resolved: RuntimeMaterial, identity: string): THREE.DataTexture | undefined => {
      const state = materialStates.get(identity.toLowerCase())
      const source = resolved.baseTexture
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
    const loadCubemap = (fact: EnvironmentInput["cubemapFacts"][number]): THREE.CubeTexture => {
      const existing = cubemapTextures.get(fact.index)
      if (existing) return existing
      if (!Number.isSafeInteger(fact.index) || fact.index < 0 || !HASH.test(fact.sha256) || !finite(fact.origin)) {
        throw new RenderingError("MalformedInput", "cubemap occurrence input is invalid")
      }
      const authored = request.environment?.authoredTextures.get(fact.logicalPath.toLowerCase())
      if (!authored || authored.sourceSha256 !== fact.sha256 || authored.width !== fact.width || authored.height !== fact.height) {
        throw new RenderingError("IdentityMismatch", "cubemap authored texture identity differs")
      }
      const texture = textureFromAuthoredCubemap(authored, THREE.NoColorSpace)
      cubemapTextures.set(fact.index, texture)
      disposables.add(texture)
      return texture
    }

    const waterSurfacePlanes = new Map<number, readonly [number, number, number, number]>()
    for (const surface of request.environment?.waterSurfaceFacts ?? []) {
      if (!surface.selected) continue
      if (waterSurfacePlanes.has(surface.face)) {
        throw new RenderingError("IdentityMismatch", `Water surface ${surface.face} has duplicate selected planes`)
      }
      waterSurfacePlanes.set(surface.face, surface.plane)
    }

    const createWaterMaterial = (identity: string, geometry: THREE.BufferGeometry): WaterMaterialResource => {
      const key = identity.toLowerCase()
      const existing = waterMaterials.get(key)
      if (existing) return existing
      const state = request.environment?.waterMaterials.get(key)
      if (!state) throw new RenderingError("MissingInput", `Water material ${identity} is unavailable`)
      const expectedShader = this.configuration.lightingProfile === "hdr" ? "dx9-hdr" : "dx90"
      if (state.shader !== expectedShader) {
        throw new RenderingError("IdentityMismatch", `Water material ${identity} selected ${state.shader} instead of ${expectedShader}`)
      }

      const normalRequest = state.textures.find((texture) => texture.role === 8 && texture.disposition === "source")
      if (!normalRequest?.logicalPath || normalRequest.colorRead !== "linear") {
        throw new RenderingError("MissingInput", `Water linear normal texture ${identity} is unavailable`)
      }
      const authored = request.environment?.authoredTextures.get(normalRequest.logicalPath.toLowerCase())
      if (!authored) {
        throw new RenderingError("MissingInput", `Water authored normal ${normalRequest.logicalPath} is unavailable`)
      }
      if (!Number.isSafeInteger(state.normalFrame.value) || state.normalFrame.value < 0 || state.normalFrame.value >= authored.frameCount) {
        throw new RenderingError("MalformedInput", `Water normal frame ${state.normalFrame.value} is outside its authored chain`)
      }
      const normalIdentity = `${authored.logicalPath.toLowerCase()}:${authored.sourceSha256}`
      let normalFrames = waterNormalFrames.get(normalIdentity)
      if (!normalFrames) {
        normalFrames = Object.freeze(Array.from({ length: authored.frameCount }, (_, frame) => {
          const texture = textureFromAuthored(authored, THREE.NoColorSpace, frame)
          disposables.add(texture)
          return texture
        }))
        waterNormalFrames.set(normalIdentity, normalFrames)
      }

      const reflectionBinding = state.textures.find((texture) => texture.role === 16)
      const refractionBinding = state.textures.find((texture) => texture.role === 17)
      if (reflectionBinding && reflectionBinding.disposition !== "render-target") {
        throw new RenderingError("UnsupportedFeature", `Water reflection ${identity} is not an authored render target`)
      }
      if (refractionBinding && refractionBinding.disposition !== "render-target") {
        throw new RenderingError("UnsupportedFeature", `Water refraction ${identity} is not an authored render target`)
      }
      const environmentBinding = state.textures.find((texture) => texture.role === 12)
      let cubemap: THREE.CubeTexture | null = null
      if (environmentBinding?.logicalPath) {
        const selectedPath = this.configuration.lightingProfile === "hdr"
          ? environmentBinding.logicalPath.replace(/\.vtf$/i, ".hdr.vtf")
          : environmentBinding.logicalPath
        const fact = request.environment?.cubemapFacts.find((candidate) =>
          candidate.logicalPath.toLowerCase() === selectedPath.toLowerCase(),
        )
        if (!fact) throw new RenderingError("MissingInput", `Water cubemap ${selectedPath} is unavailable`)
        cubemap = loadCubemap(fact)
      } else if (environmentBinding?.disposition === "environment") {
        throw new RenderingError("MissingInput", `Water material ${identity} has no selected local cubemap identity`)
      }

      const shaderState = (mode: "expensive" | "cheap"): SourceWaterShaderState => Object.freeze({
        profile: this.configuration.lightingProfile,
        mode,
        aboveWater: state.aboveWater.value,
        reflectAmount: state.reflectAmount.value,
        refractAmount: state.refractAmount.value,
        reflectTint: state.reflectTint.value,
        refractTint: state.refractTint.value,
        fogColor: state.fog.color.value,
        fogStart: state.fog.start.value,
        fogEnd: state.fog.end.value,
        blurRefraction: state.blurRefraction.value,
        hasBaseTexture: state.textures.some((texture) => texture.role === 0),
        cheapBlend: mode === "cheap" && !state.forceCheap.value,
        cheapStart: state.cheapStart.value,
        cheapEnd: state.cheapEnd.value,
        reflectionBlendFactor: state.reflectionBlendFactor.value,
        fresnelEnabled: state.fresnel.cheapEnabled,
        linearLightScale: this.configuration.lightingProfile === "hdr" ? this.#exposure.snapshot().current : 1,
        environmentScale: this.configuration.lightingProfile === "hdr" ? 16 : 1,
      })

      const selectedMode = state.forceCheap.value || (!reflectionBinding && !refractionBinding) ? "cheap" : "expensive"
      const primary = createSourceWaterMaterial({
        geometry,
        state: shaderState(selectedMode),
        normal: normalFrames[state.normalFrame.value]!,
        reflection: reflectionBinding ? reflectionTarget.texture : null,
        refraction: refractionBinding ? refractionTarget.texture : null,
        cubemap,
        refractionDepthEncoding: refractionBinding && state.aboveWater.value ? "source-water-fog-alpha" : null,
        linearLightScale: exposureUniform,
      })
      disposables.add(primary.material)
      let cheapMaterial: THREE.MeshBasicNodeMaterial | null = null
      let cheapNormalNode: ReturnType<typeof TSL.texture> | null = null
      if (selectedMode === "expensive" && cubemap && !state.forceExpensive.value) {
        const cheap = createSourceWaterMaterial({
          geometry,
          state: shaderState("cheap"),
          normal: normalFrames[state.normalFrame.value]!,
          reflection: null,
          refraction: refractionBinding ? refractionTarget.texture : null,
          cubemap,
          refractionDepthEncoding: refractionBinding && state.aboveWater.value ? "source-water-fog-alpha" : null,
          linearLightScale: exposureUniform,
        })
        cheapMaterial = cheap.material
        cheapNormalNode = cheap.normalNode
        disposables.add(cheap.material)
      }
      const fog = state.fog.enabled?.value
        ? new THREE.Fog(
            new THREE.Color(
              sourceShaderGammaToLinear(state.fog.color.value[0]),
              sourceShaderGammaToLinear(state.fog.color.value[1]),
              sourceShaderGammaToLinear(state.fog.color.value[2]),
            ),
            state.fog.start.value,
            state.fog.end.value,
          )
        : null
      const resource = Object.freeze({
        material: primary.material,
        cheapMaterial,
        normalFrames,
        normalNode: primary.normalNode,
        cheapNormalNode,
        fog,
        state,
      })
      waterMaterials.set(key, resource)
      return resource
    }
    const createWorldMesh=(batch:RuntimeBatch):THREE.Mesh|null=>{
        const geometry = new THREE.BufferGeometry()
        geometry.setAttribute("position", new THREE.BufferAttribute(batch.positions, 3))
        geometry.setAttribute("normal", new THREE.BufferAttribute(batch.normals, 3))
        geometry.setAttribute("uv", new THREE.BufferAttribute(batch.uv, 2))
        geometry.setAttribute("uv1", new THREE.BufferAttribute(batch.lightmapUv, 2))
        geometry.setAttribute("lightmapKind", new THREE.BufferAttribute(batch.lightmapKind, 1))
        geometry.setAttribute("displacementAlpha", new THREE.BufferAttribute(batch.displacementAlpha, 1))
        const index = new THREE.BufferAttribute(batch.indices.slice(), 1)
        index.setUsage(THREE.DynamicDrawUsage)
        geometry.setIndex(index)
        geometry.computeBoundingSphere()
        disposables.add(geometry)
        const resolved = map.materials[batch.material]!
        const identity = resolved.logicalPath
        const materialState = materialStates.get(identity.toLowerCase())
        if (materialState?.noDraw) return null
        if (resolved.shader === 5) {
          const tangents = sourceWaterTangentAttributes({
            positions: batch.positions,
            normals: batch.normals,
            uv: batch.uv,
            indices: batch.indices,
            faces: batch.faces,
            surfacePlanes: waterSurfacePlanes,
          })
          geometry.setAttribute("sourceTangentS", new THREE.BufferAttribute(tangents.tangentS, 3))
          geometry.setAttribute("sourceTangentT", new THREE.BufferAttribute(tangents.tangentT, 3))
          const resource = createWaterMaterial(identity, geometry)
          const mesh = new THREE.Mesh(geometry, resource.material)
          mesh.userData.materialIdentity = identity
          waterMeshes.push(Object.freeze({ mesh, materialIdentity: identity.toLowerCase() }))
          return mesh
        }
        const baseTexture = createBase(resolved, identity)
        const secondTexture = resolved.secondTexture ? textureFromRgba(resolved.secondTexture, THREE.SRGBColorSpace) : undefined
        if (secondTexture) disposables.add(secondTexture)
        const detailTexture = resolved.detail ? textureFromRgba(resolved.detail.texture, THREE.NoColorSpace) : undefined
        if (detailTexture) disposables.add(detailTexture)
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
            secondTexture,
            detailTexture,
            lightmapTextures,
            supplied?.texture,
            supplied?.input.kind,
            supplied?.input.uvTransform,
            exposureUniform,
            waterFogUniforms,
            materialState,
          )
        } else {
          const nodeMaterial = new THREE.MeshBasicNodeMaterial(materialOptions(resolved, materialState))
          const base = detailColor(worldBaseColor(resolved, baseTexture ? TSL.texture(baseTexture, TSL.uv()) : TSL.vec4(TSL.color(debugColor(identity)), 1), secondTexture), resolved.detail, detailTexture)
          const irradiance = TSL.texture(lightmapTextures[0], TSL.uv(1)).rgb
          nodeMaterial.colorNode = sourceFragmentColor(
            TSL.vec4(base.rgb.mul(irradiance), base.a),
            materialState,
            waterFogUniforms,
          )
          nodeMaterial.toneMapped = false
          material = nodeMaterial
        }
        disposables.add(material)
        const mesh = new THREE.Mesh(geometry, material)
        mesh.matrixAutoUpdate = false
        mesh.updateMatrix()
        mesh.userData.materialIdentity = identity
        return mesh
    }
    try {
      for (const batch of map.batches) {
        const mesh = createWorldMesh(batch)
        if (!mesh) continue
        const index = mesh.geometry.getIndex()
        if (!index) throw new RenderingError("MalformedInput", "world batch has no index buffer")
        const transparent = (Array.isArray(mesh.material) ? mesh.material : [mesh.material]).some((material) => material.transparent)
        worldBatches.push({ mesh, faces: batch.faces, sourceIndices: batch.indices, index, transparent })
        ;(transparent ? mainTransparentWorld : worldBundle).add(mesh)

        const skyGeometry = new THREE.BufferGeometry()
        for (const name of Object.keys(mesh.geometry.attributes)) {
          skyGeometry.setAttribute(name, mesh.geometry.getAttribute(name))
        }
        const skyIndex = new THREE.BufferAttribute(batch.indices.slice(), 1).setUsage(THREE.DynamicDrawUsage)
        skyGeometry.setIndex(skyIndex)
        skyGeometry.boundingBox = mesh.geometry.boundingBox
        skyGeometry.boundingSphere = mesh.geometry.boundingSphere
        disposables.add(skyGeometry)
        const skyMesh = new THREE.Mesh(skyGeometry, mesh.material)
        skyMesh.matrixAutoUpdate = false
        skyMesh.updateMatrix()
        skyMesh.userData.materialIdentity = mesh.userData.materialIdentity
        skyMesh.userData.skyWater = map.materials[batch.material]?.shader === 5
        skyWorldBatches.push({ mesh: skyMesh, faces: batch.faces, sourceIndices: batch.indices, index: skyIndex, transparent })
        ;(transparent ? skyTransparentWorld : skyWorldBundle).add(skyMesh)
      }
      for(const model of map.brushModels){const template=new THREE.Group();for(const batch of model.batches){const mesh=createWorldMesh(batch);if(mesh)template.add(mesh)}brushModelTemplates.set(model.index,template)}

      const environmentTextures = new Map<string, THREE.DataTexture>()
      const authoredEnvironmentMaterials=new Set<string>()
      for (const texture of request.environment?.textures ?? []) {
        const authored=request.environment?.authoredTextures.get(texture.logicalPath.toLowerCase()),value=authored?textureFromAuthored(authored,THREE.SRGBColorSpace):textureFromRgba(texture, THREE.SRGBColorSpace, materialStates.get(texture.material.toLowerCase()))
        if(authored)authoredEnvironmentMaterials.add(texture.material.toLowerCase())
        environmentTextures.set(texture.material.toLowerCase(), value)
        disposables.add(value)
      }
      const cubemapIdentities = new Set<number>()
      for (const fact of request.environment?.cubemapFacts ?? []) {
        if (cubemapIdentities.has(fact.index)) {
          throw new RenderingError("MalformedInput", "cubemap occurrence input is invalid")
        }
        cubemapIdentities.add(fact.index)
        loadCubemap(fact)
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
          material.colorNode = sourceFragmentColor(TSL.texture(texture, TSL.uv()), state, waterFogUniforms)
          material.toneMapped = false
          disposables.add(material)
          const mesh = new THREE.Mesh(geometry, material)
          mesh.renderOrder = mark.renderOrder
          mesh.visible = false
          if (fragment.visibility.kind === "world") {
            mesh.matrixAutoUpdate = false
            mesh.updateMatrix()
          }
          projectedMarks.push({ mesh, face: fragment.face, sourceIndex: mark.sourceIndex, visibility: fragment.visibility })
          group.add(mesh)
        }
      }

      for (const model of map.models) {
        const template = new THREE.Group()
        for (let primitiveIndex=0;primitiveIndex<model.primitives.length;primitiveIndex+=1) {
          const primitive=model.primitives[primitiveIndex]!
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
           let sampled=baseTexture?TSL.texture(baseTexture.texture,TSL.uv()):undefined
           if(sampled&&baseTexture?.input.sourceFormat===1)sampled=sampled.abgr
           else if(sampled&&baseTexture?.input.sourceFormat===11)sampled=sampled.gbar
           else if(sampled&&baseTexture?.input.sourceFormat===12)sampled=sampled.bgra
           else if(sampled&&baseTexture?.input.sourceFormat===16)sampled=TSL.vec4(sampled.bgr,1)
            const base = selectDiagnosticModelBase(baseTexture !== undefined) === "authored-texture"
              ? sampled!
              : TSL.vec4(TSL.color(debugColor(`diagnostic:${resolved.logicalPath}`)), 1)
          material.colorNode = sourceFragmentColor(base, materialState, waterFogUniforms)
          material.toneMapped = false
          disposables.add(material)
          const mesh = new THREE.Mesh(geometry, material)
          mesh.userData.primitiveMaterial = primitive.material
          mesh.userData.sourcePrimitive=primitiveIndex
          mesh.userData.materialIdentity=resolved.logicalPath
          template.add(mesh)
        }
        modelTemplates.set(model.logicalPath, template)
      }
      if(request.staticProps){const props=request.staticProps,profile=this.configuration.lightingProfile==="hdr"?1:0
        for(let propIndex=0;propIndex<props.count;propIndex+=1){const modelIdentity=props.models[props.presentationModel[propIndex]!]!,key=modelKey(modelIdentity,props.skin[propIndex]!),template=modelTemplates.get(key);if(!template)throw new RenderingError("MissingInput",`static-prop model ${key} is unavailable`)
          if(props.body[propIndex]!==0)throw new RenderingError("UnsupportedFeature","nonzero static-prop body selection is unavailable")
          const instance=template.clone(true),lightingKind=props.lightingKind[propIndex]!,fadeUniform=TSL.uniform(1,"float"),meshes:THREE.Mesh[]=[];instance.traverse(value=>{if(value instanceof THREE.Mesh)meshes.push(value)})
          let colorMeshes:StaticPropInput["vhv"][number]["meshes"]=Object.freeze([])
          if(lightingKind===0){const objectIndex=props.vhvObjects[propIndex*2+profile]!,object=props.vhv[objectIndex];if(!object||object.occurrence!==props.source[propIndex]||object.profile!==profile||object.model!==props.dictionaryModel[propIndex])throw new RenderingError("IdentityMismatch","static-prop VHV occurrence identity differs");colorMeshes=Object.freeze(object.meshes.filter(mesh=>mesh.lod===props.lod[propIndex]))}
          let colorIndex=0
          for(const mesh of meshes){const sourceGeometry=mesh.geometry,geometry=new THREE.BufferGeometry();for(const name of Object.keys(sourceGeometry.attributes))geometry.setAttribute(name,sourceGeometry.getAttribute(name));geometry.setIndex(sourceGeometry.getIndex());geometry.boundingBox=sourceGeometry.boundingBox;geometry.boundingSphere=sourceGeometry.boundingSphere
            if(lightingKind===0){const color=colorMeshes[colorIndex++];const position=geometry.getAttribute("position");if(!color||color.vertexCount!==position.count||color.colors.length!==position.count*4)throw new RenderingError("IdentityMismatch","static-prop VHV mesh order differs");geometry.setAttribute("staticLighting",new THREE.Uint8BufferAttribute(color.colors,4,true))}
            disposables.add(geometry);mesh.geometry=geometry
            const original=mesh.material;if(Array.isArray(original)||!(original instanceof THREE.MeshBasicNodeMaterial))throw new RenderingError("UnsupportedFeature","static-prop model material family is unavailable")
            const identity=String(mesh.userData.materialIdentity),state=request.modelMaterials?.get(identity.toLowerCase())?.shader,material=original.clone(),base=original.colorNode??TSL.vec4(1,1,1,1),rgb=state==="unlit-generic"?base.rgb:base.rgb.mul(lightingKind===0?TSL.attribute("staticLighting","vec4").bgra.rgb:runtimeStaticLightingNode(map,props,propIndex)).mul(exposureUniform);material.colorNode=sourceFragmentColor(TSL.vec4(rgb,base.a.mul(fadeUniform)),materialStates.get(identity.toLowerCase()),waterFogUniforms);material.toneMapped=false;if((props.flags[propIndex]!&1)!==0){material.transparent=true;material.depthWrite=false}disposables.add(material);mesh.material=material
          }
          if(lightingKind===0&&colorIndex!==colorMeshes.length)throw new RenderingError("IdentityMismatch","static-prop VHV mesh closure differs")
          const position=props.transform.subarray(propIndex*6,propIndex*6+3),angles=props.transform.subarray(propIndex*6+3,propIndex*6+6);sourceTransform(instance,position,angles);instance.updateMatrix();instance.matrixAutoUpdate=false;instance.userData.staticPropSource=props.source[propIndex]
          const box = new THREE.Box3().setFromObject(instance)
          const sphere = box.getBoundingSphere(new THREE.Sphere())
          const leafStart = props.leafOffsets[propIndex]!
          const leafEnd = props.leafOffsets[propIndex + 1]!
          const ownership = props.ownership[propIndex] as 0 | 1
          const lightingOrigin = Number.isFinite(props.lightingOrigin[propIndex * 3])
            ? Object.freeze([props.lightingOrigin[propIndex * 3]!, props.lightingOrigin[propIndex * 3 + 1]!, props.lightingOrigin[propIndex * 3 + 2]!] as const)
            : null
          ;(ownership === 0 ? mainStaticProps : skyStaticProps).add(instance)
          staticPropInstances.push(Object.freeze({
            object: instance,
            source: props.source[propIndex]!,
            ownership,
            leaves: props.leaves.subarray(leafStart, leafEnd),
            origin: Object.freeze([position[0]!, position[1]!, position[2]!] as const),
            lightingOrigin,
            flags: props.flags[propIndex]!,
            fadeMinimum: props.fades[propIndex * 3]!,
            fadeMaximum: props.fades[propIndex * 3 + 1]!,
            forcedFadeScale: props.fades[propIndex * 3 + 2]!,
            radius: sphere.radius,
            bounds: Object.freeze([box.min.x, box.min.y, box.min.z, box.max.x, box.max.y, box.max.z] as const),
            fadeUniform,
          }))
        }
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
        if (!state) throw new RenderingError("MissingInput", `Particle material state ${texture.material} is unavailable`)
        requireMipInputs(texture.material, state)
        const value = textureFromRgba(texture, THREE.SRGBColorSpace, state)
        particleTextures.set(texture.material.toLowerCase(), value)
        disposables.add(value)
        const material = new THREE.MeshBasicNodeMaterial(materialOptions({
          logicalPath: texture.material, width: texture.width, height: texture.height, shader: 7, features: 1, textureRole: 0,
        }, state))
        applyParticleDepthState(material, state)
        const current = TSL.texture(value, TSL.uv())
        const next = TSL.texture(value, TSL.attribute("particleUvNext", "vec2"))
        const blend = TSL.attribute("particleSheetBlend", "float")
        const color = TSL.attribute("particleColor", "vec4")
        const sampled = current.mul(TSL.float(1).sub(blend)).add(next.mul(blend))
        material.colorNode = sourceFragmentColor(
          TSL.vec4(sampled.rgb.mul(color.rgb), sampled.a.mul(color.a)),
          state,
          waterFogUniforms,
        )
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
        mainStaticProps,
        skyStaticProps,
        staticPropInstances,
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
    const surfaceLighting = map.lighting.profile === "hdr"
      ? new Map(map.lighting.descriptor.surfaces.map((surface) => [surface.face, surface] as const))
      : new Map<number, never>()
    const displacementEvidence = map.displacements.map((displacement) => {
      const resolved = map.materials[displacement.material]!
      const state = materialStates.get(resolved.logicalPath.toLowerCase())
      const options = materialOptions(resolved, state)
      const lighting = surfaceLighting.get(displacement.face)
      const submittedTriangles = map.batches
        .filter((batch) => batch.material === displacement.material)
        .reduce((total, batch) => total + batch.faces.reduce((count, face) => count + Number(face === displacement.face), 0), 0)
      return Object.freeze({
        source: displacement.source,
        face: displacement.face,
        material: resolved.logicalPath,
        positions: displacement.positions,
        normals: displacement.normals,
        indices: displacement.indices,
        bounds: displacement.bounds,
        submittedTriangles,
        cull: options.side === THREE.DoubleSide ? "none" as const : "back" as const,
        depthTest: options.depthTest !== false,
        depthWrite: options.depthWrite !== false,
        blend: options.transparent === true,
        lighting: Object.freeze({
          kind: lighting?.kind ?? (displacement.lightOffset < 0 ? "unlit" : "flat"),
          styles: lighting?.styles ?? displacement.styles,
          sampleStart: lighting?.sampleStart ?? Math.max(0, displacement.lightOffset / 4),
          samplesPerLayer: lighting?.samplesPerLayer ?? displacement.lightmapWidth * displacement.lightmapHeight,
          layers: lighting?.layerCount ?? (displacement.lightOffset < 0 ? 0 : 1),
        }),
      })
    })
    const projectedMarksByFace = new Map<number, ProjectedMarkResource[]>()
    const brushProjectedMarks: ProjectedMarkResource[] = []
    for (const mark of projectedMarks) {
      if (mark.visibility.kind === "world") {
        let values = projectedMarksByFace.get(mark.face)
        if (!values) projectedMarksByFace.set(mark.face, values = [])
        values.push(mark)
      } else {
        brushProjectedMarks.push(mark)
      }
    }
    const runtimeStaticPropInstances = staticPropInstances.filter((_, index) => request.staticProps?.lightingKind[index] === 1)
    const createVisibility = (batches: readonly WorldBatchResource[], source?: RetainedWorldVisibility) => new RetainedWorldVisibility(
      batches.map((batch) => ({
        faces: batch.faces,
        sourceIndices: batch.sourceIndices,
        targetIndices: batch.index.array as Uint32Array,
        transparent: batch.transparent,
      })),
      source,
    )
    const worldVisibility = createVisibility(worldBatches)
    const skyWorldVisibility = createVisibility(skyWorldBatches, worldVisibility)
    const leafVisibility = new RetainedLeafVisibility(staticPropInstances)
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
      staticProps:Object.freeze({total:request.staticProps?.count??0,main:request.staticProps?request.staticProps.ownership.reduce((total,value)=>total+Number(value===0),0):0,sky3d:request.staticProps?request.staticProps.ownership.reduce((total,value)=>total+Number(value===1),0):0,runtimeLit:request.staticProps?request.staticProps.lightingKind.reduce((total,value)=>total+Number(value===1),0):0}),
      runtimeStaticProps:Object.freeze(runtimeStaticPropInstances.map(prop=>{if(!prop.lightingOrigin)throw new RenderingError("MissingInput","runtime static prop has no lighting origin");return Object.freeze({source:prop.source,origin:prop.origin,lightingOrigin:prop.lightingOrigin,radius:prop.radius})})),
      displacements: Object.freeze(displacementEvidence),
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
        environment: request.environment,
        materialStates: new Map(materialStates),
        particleTextures: request.particleTextures,
        modelOccurrences: request.modelOccurrences?.map((value) => Object.freeze({ ...value, matrix: value.matrix.slice() })),
        modelFacing: new Map(request.modelFacing ?? []),
        modelMaterials: new Map(request.modelMaterials ?? []),
        authoredTextures: new Map([...(request.authoredTextures ?? [])].map(([identity, texture]) => [identity, Object.freeze({
          ...texture,
          faces: Object.freeze([...texture.faces]),
          planes: Object.freeze(texture.planes.map((plane) => Object.freeze({ ...plane }))),
        })])),
        modelDrawInputs: request.modelDrawInputs?.map((input) => Object.freeze({
          entity: input.entity,
          lighting: structuredClone(input.lighting),
          eyes: Object.freeze(input.eyes.map((eye) => structuredClone(eye))),
        })),
        brushModels:request.brushModels?.map(model=>Object.freeze({...model,surfaceRange:Object.freeze([...model.surfaceRange]) as readonly[number,number],materials:Object.freeze([...model.materials])})),
        staticProps: request.staticProps,
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
      waterFogUniforms,
      diagnostics: Object.freeze(diagnostics),
      worldBundle,
      skyWorldBundle,
      mainTransparentWorld,
      skyTransparentWorld,
      worldBatches: Object.freeze(worldBatches),
      skyWorldBatches: Object.freeze(skyWorldBatches),
      worldVisibility,
      skyWorldVisibility,
      modelLookup: new Map(map.models.map((model) => [model.logicalPath, model])),
      projectedMarksByFace,
      brushProjectedMarks: Object.freeze(brushProjectedMarks),
      leafVisibility,
      runtimeStaticPropInstances: Object.freeze(runtimeStaticPropInstances),
      projectedMarks: Object.freeze(projectedMarks),
      waterMeshes:Object.freeze(waterMeshes),
      waterMaterials,
      cubemapTextures,
      skyGroup,
      mainStaticProps,
      skyStaticProps,
      staticPropInstances:Object.freeze(staticPropInstances),
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
        const visibilityChanged = this.#worldVisibilityIdentity !== frame.visibility.cacheIdentity
        this.#setWorldVisibility(frame.visibility.surfaces, frame.visibility.cacheIdentity)
        if (this.#active.skyGroup) this.#active.skyGroup.visible = frame.visibility.sky === 1
        if (!frame.collisionWorldIdentity || frame.collisionWorldIdentity !== this.#active.result.environment?.collisionWorldIdentity) {
          throw new RenderingError("IdentityMismatch", "mark collision-world identity differs")
        }
        if (visibilityChanged) {
          for (const face of this.#visibleWorldMarkFaces) {
            for (const mark of this.#active.projectedMarksByFace.get(face) ?? []) {
              if (mark.mesh.visible) this.#visibleProjectedMarkCount -= 1
              mark.mesh.visible = false
            }
          }
          this.#visibleWorldMarkFaces.clear()
          for (let index = 0; index < frame.visibility.surfaces.length; index += 1) {
            const face = frame.visibility.surfaces[index]!
            const marks = this.#active.projectedMarksByFace.get(face)
            if (!marks) continue
            this.#visibleWorldMarkFaces.add(face)
            for (const mark of marks) {
              if (!mark.mesh.visible) this.#visibleProjectedMarkCount += 1
              mark.mesh.visible = true
            }
          }
        }
        for (const mark of this.#active.brushProjectedMarks) {
          const visibility = mark.visibility
          if (visibility.kind !== "brush-model") continue
          const receiver = this.#findBrushModel(frame.brushModels?.models, visibility.entity, visibility.model)
          const visible = receiver?.draw === true
          if (mark.mesh.visible !== visible) this.#visibleProjectedMarkCount += visible ? 1 : -1
          mark.mesh.visible = visible
          if (receiver) sourceTransform(mark.mesh, receiver.worldPosition, receiver.worldAngles)
        }
        const water = frame.visibility.water
        const visibleWater = water.visibleWater
        if (visibleWater) {
          const resource = this.#active.waterMaterials.get(visibleWater.material.toLowerCase())
          if (!resource) throw new RenderingError("MissingInput", `current Water material ${visibleWater.material} is unavailable`)
          const frameIndex = ((visibleWater.evaluated.normalFrame % resource.normalFrames.length)
            + resource.normalFrames.length) % resource.normalFrames.length
          const texture = resource.normalFrames[frameIndex]!
          const matrix = visibleWater.evaluated.normalTransform
          texture.matrixAutoUpdate = false
          texture.matrix.set(
            matrix[0]!, matrix[1]!, matrix[3]!,
            matrix[4]!, matrix[5]!, matrix[7]!,
            matrix[12]!, matrix[13]!, matrix[15]!,
          )
          resource.normalNode.value = texture
          if (resource.cheapNormalNode) resource.cheapNormalNode.value = texture
          const selectedMaterial = water.render.cheap && !resource.state.forceCheap.value
            ? resource.cheapMaterial
            : resource.material
          if (!selectedMaterial) {
            throw new RenderingError("MissingInput", `current Water material ${visibleWater.material} has no selected cheap cubemap shader`)
          }
          for (const waterMesh of this.#active.waterMeshes) {
            waterMesh.mesh.material = selectedMaterial
            waterMesh.mesh.visible = water.render.drawSurface
              && waterMesh.materialIdentity === visibleWater.material.toLowerCase()
          }
        } else {
          for (const waterMesh of this.#active.waterMeshes) waterMesh.mesh.visible = false
        }
      }
      if (frame.exposureHistogram) this.#exposure.submit(frame.exposureHistogram)
      const exposure =
        this.configuration.lightingProfile === "hdr"
          ? this.#exposure.advance(frame.deltaSeconds ?? 0)
          : this.#exposure.snapshot()
      this.#active.exposureUniform.value = this.configuration.lightingProfile === "hdr" ? exposure.current : 1
      this.#setCamera(frame.camera)
      if(frame.visibility)this.#setStaticPropVisibility(frame.visibility.leaves,0,frame.camera)
      const dynamicItemsStarted = performance.now()
      const viewModelDepthRange = this.#stageDynamicItems(frame)
      const dynamicItemsMilliseconds = performance.now() - dynamicItemsStarted
      let viewModelPass: FrameResult["viewModelPass"],sky3dPass:FrameResult["sky3dPass"]
      let waterPasses: FrameResult["waterPasses"] = Object.freeze([])
      let waterPassTimings: FrameResult["waterPassTimings"] = Object.freeze([])
      let waterStateRestored = true
      let worldMilliseconds=0,viewModelMilliseconds=0
      if (!this.#suspended) {
        const worldStarted=performance.now()
        sky3dPass=this.#renderSky3dPass(frame)
        const waterResult=this.#renderWaterPasses(frame,sky3dPass!==undefined)
        worldMilliseconds=performance.now()-worldStarted
        waterPasses = waterResult.passes
        waterPassTimings = waterResult.timings
        waterStateRestored = waterResult.restored
        if (this.#viewModels.children.length > 0) {
          if (!viewModelDepthRange) throw new RenderingError("InvalidState", "viewmodel depth range is unavailable")
          this.#backend.autoClear = false
          const background = this.#scene.background
          const matrixWorldAutoUpdate = this.#scene.matrixWorldAutoUpdate
          this.#scene.background = null
          this.#scene.matrixWorldAutoUpdate = false
          const viewModelStarted=performance.now()
          try {
            const phase = executeViewModelDepthPhase({
              depthRange: viewModelDepthRange,
              clearWorldDepth: () => this.#backend.clearDepth(),
              setDepthRange: ([minimum, maximum]) => this.#backend.setViewport(0, 0, this.#viewportWidth, this.#viewportHeight, minimum, maximum),
              draw: () => this.#backend.render(this.#scene, this.#viewCamera),
            })
            viewModelPass = Object.freeze({
              depthRange: phase.depthRange,
              worldDepthCleared: phase.worldDepthCleared,
              viewportRestored: phase.depthRangeRestored,
            })
          } finally {
            viewModelMilliseconds=performance.now()-viewModelStarted
            this.#scene.background = background
            this.#scene.matrixWorldAutoUpdate = matrixWorldAutoUpdate
            this.#backend.autoClear = true
          }
        }
      }
      const capture = frame.capture ? await this.#capture(frame.capture) : undefined
      return Object.freeze({
        deviceGeneration: this.#deviceGeneration,
        sceneGeneration: this.#sceneGeneration,
        submission: null,
        exposure,
        visibleProjectedMarks: this.#visibleProjectedMarkCount,
        waterPasses,
        waterPassTimings,
        waterStateRestored,
        sky3dPass,
        visibleMainStaticPropSources: this.#visibleStaticSources[0],
        runtimeStaticPropScreen: this.#runtimeStaticPropScreen(),
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

  #setWorldVisibility(surfaces: Uint32Array, identity?: string, ownership: 0 | 1 = 0): void {
    const scene = this.#active
    if (!scene) throw new RenderingError("InvalidState", "renderer has no active world visibility resources")
    const priorIdentity = ownership === 0 ? this.#worldVisibilityIdentity : this.#skyWorldVisibilityIdentity
    if (identity !== undefined && identity === priorIdentity) return
    const prior = ownership === 0 ? this.#worldVisibilitySurfaces : this.#skyWorldVisibilitySurfaces
    if (surfaces === prior || (prior?.length === surfaces.length && surfaces.every((value, index) => value === prior[index]))) {
      if (ownership === 0) this.#worldVisibilityIdentity = identity
      else this.#skyWorldVisibilityIdentity = identity
      return
    }

    const index = ownership === 0 ? scene.worldVisibility : scene.skyWorldVisibility
    const batches = ownership === 0 ? scene.worldBatches : scene.skyWorldBatches
    try {
      index.apply(surfaces)
    } catch (error) {
      if (error instanceof RetainedVisibilityError) throw new RenderingError("MalformedInput", error.message)
      throw error
    }
    let bundleChanged = false
    for (let batchIndex = 0; batchIndex < batches.length; batchIndex += 1) {
      if (!index.changed(batchIndex)) continue
      const batch = batches[batchIndex]!
      const count = index.count(batchIndex)
      batch.index.needsUpdate = true
      batch.mesh.geometry.setDrawRange(0, count)
      batch.mesh.visible = count > 0 && !(ownership === 1 && batch.mesh.userData.skyWater === true)
      if (!batch.transparent) bundleChanged = true
    }
    if (bundleChanged) (ownership === 0 ? scene.worldBundle : scene.skyWorldBundle).needsUpdate = true
    if (ownership === 0) {
      this.#worldVisibilitySurfaces = surfaces
      this.#worldVisibilityIdentity = identity
    } else {
      this.#skyWorldVisibilitySurfaces = surfaces
      this.#skyWorldVisibilityIdentity = identity
    }
  }

  #setStaticPropVisibility(leaves: readonly number[], ownership: 0 | 1, camera: Camera): void {
    const scene = this.#active
    if (!scene) return
    const count = scene.leafVisibility.select(leaves, ownership)
    const prior = this.#visibleStaticIndices[ownership]
    const next = this.#nextVisibleStaticIndices[ownership]
    next.clear()
    let membershipChanged = false

    for (let candidate = 0; candidate < count; candidate += 1) {
      const index = scene.leafVisibility.at(candidate)
      const prop = scene.staticPropInstances[index]!
      let alpha = 1
      if ((prop.flags & 1) !== 0) {
        if ((prop.flags & 0x20) !== 0) {
          const up = this.#fadeUp.set(0, 1, 0).applyQuaternion(this.#camera.quaternion).multiplyScalar(prop.radius)
          const first = this.#fadeFirst.set(...prop.origin).add(up).project(this.#camera)
          const second = this.#fadeSecond.set(...prop.origin).sub(up).project(this.#camera)
          const pixelWidth = this.#viewportHeight * Math.abs(second.y - first.y)
          alpha = screenFadeOpacity(pixelWidth, prop.fadeMaximum, prop.fadeMinimum)
        } else {
          const dx = prop.origin[0] - camera.position[0]
          const dy = prop.origin[1] - camera.position[1]
          const dz = prop.origin[2] - camera.position[2]
          alpha = distanceFadeOpacity(dx * dx + dy * dy + dz * dz, prop.fadeMinimum, prop.fadeMaximum)
        }
      }
      alpha = quantizeStaticPropOpacity(alpha)
      prop.fadeUniform.value = alpha
      if (alpha > 0) {
        prop.object.visible = true
        next.add(index)
        if (!prior.has(index)) membershipChanged = true
      } else {
        prop.object.visible = false
      }
    }

    for (const index of prior) {
      if (next.has(index)) continue
      scene.staticPropInstances[index]!.object.visible = false
      membershipChanged = true
    }
    this.#visibleStaticIndices[ownership] = next
    this.#nextVisibleStaticIndices[ownership] = prior
    if (membershipChanged || next.size !== this.#visibleStaticSources[ownership].length) {
      const sources: number[] = []
      for (const index of next) sources.push(scene.staticPropInstances[index]!.source)
      this.#visibleStaticSources[ownership] = Object.freeze(sources)
    }
  }

  #runtimeStaticPropScreen(): FrameResult["runtimeStaticPropScreen"] {
    if (!this.#active || this.#active.runtimeStaticPropInstances.length === 0) return Object.freeze([])
    const output: FrameResult["runtimeStaticPropScreen"][number][] = []
    for (const prop of this.#active.runtimeStaticPropInstances) {
      if (prop.ownership !== 0 || !prop.object.visible) continue
      const bounds = prop.bounds
      let minimumX = Number.POSITIVE_INFINITY
      let maximumX = Number.NEGATIVE_INFINITY
      let minimumY = Number.POSITIVE_INFINITY
      let maximumY = Number.NEGATIVE_INFINITY
      for (let corner = 0; corner < 8; corner += 1) {
        const point = this.#projectionPoint.set(
          bounds[(corner & 4) === 0 ? 0 : 3]!,
          bounds[(corner & 2) === 0 ? 1 : 4]!,
          bounds[(corner & 1) === 0 ? 2 : 5]!,
        ).project(this.#camera)
        minimumX = Math.min(minimumX, point.x)
        maximumX = Math.max(maximumX, point.x)
        minimumY = Math.min(minimumY, point.y)
        maximumY = Math.max(maximumY, point.y)
      }
      output.push(Object.freeze({
        source: prop.source,
        x: (minimumX + 1) * 0.5 * this.#viewportWidth,
        y: (1 - maximumY) * 0.5 * this.#viewportHeight,
        width: (maximumX - minimumX) * 0.5 * this.#viewportWidth,
        height: (maximumY - minimumY) * 0.5 * this.#viewportHeight,
      }))
    }
    return Object.freeze(output)
  }

  #findBrushModel(
    models: NonNullable<Frame["brushModels"]>["models"] | undefined,
    sourceIdentity: bigint,
    modelIdentity: number,
  ): NonNullable<Frame["brushModels"]>["models"][number] | undefined {
    if (!models || sourceIdentity < 0n || sourceIdentity > BigInt(Number.MAX_SAFE_INTEGER)) return undefined
    const source = Number(sourceIdentity)
    let low = 0
    let high = models.length - 1
    while (low <= high) {
      const middle = (low + high) >>> 1
      const value = models[middle]!
      if (value.sourceIndex < source) low = middle + 1
      else if (value.sourceIndex > source) high = middle - 1
      else return value.model === modelIdentity ? value : undefined
    }
    return undefined
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
      const runtimeModel = this.#active!.modelLookup.get(modelKey(item.model, item.skin ?? 0))
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
      const position = item.position
      const previous = item.previousPosition
      if (
        !Number.isSafeInteger(item.identity)
        || item.identity < 1
        || !Number.isFinite(position[0])
        || !Number.isFinite(position[1])
        || !Number.isFinite(position[2])
        || !Number.isFinite(previous[0])
        || !Number.isFinite(previous[1])
        || !Number.isFinite(previous[2])
        || !Number.isFinite(item.radius)
        || !Number.isFinite(item.rollRadians)
        || !Number.isFinite(item.opacity)
        || !Number.isFinite(item.trailLength)
        || !Number.isFinite(item.ageSeconds)
        || !Number.isFinite(item.trailMinLength)
        || !Number.isFinite(item.trailMaxLength)
        || !Number.isFinite(item.trailFadeInSeconds)
        || item.radius < 0
        || item.opacity < 0
        || item.opacity > 1
        || item.orientationType !== 0
        || !this.#active!.particleTextures.has(item.material.toLowerCase())
        || !item.primarySheet
      ) throw new RenderingError("MalformedInput", "particle draw item is invalid")
    }
    if(this.#active!.result.environment&&!frame.visibility)throw new RenderingError("MissingInput","an exact visibility result is required")
    if(frame.visibility){if(!Array.isArray(frame.visibility.leaves)||!Array.isArray(frame.visibility.areas)||frame.visibility.sky<0||frame.visibility.sky>2||(frame.visibility.eyeLeaf!==null&&(!Number.isSafeInteger(frame.visibility.eyeLeaf)||frame.visibility.eyeLeaf<0)))throw new RenderingError("MalformedInput","visibility view state is invalid");const water=frame.visibility.water;if(water.passes.length<1||water.passes.length>4||water.passes.filter(pass=>pass.kind==="main").length!==1)throw new RenderingError("MalformedInput","Water view plan is invalid");let prior=-1;for(const pass of water.passes){const order=pass.kind==="reflection"?0:pass.kind==="refraction"?1:pass.kind==="main"?2:3;if(order<prior||!finite([...pass.origin,...pass.angles,pass.clip?.height??0])||pass.surfaces.length>100_000)throw new RenderingError("MalformedInput","Water pass is invalid");prior=order}if(water.visibleWater&&(!finite([water.visibleWater.surfaceZ,water.visibleWater.evaluated.cheapStart,water.visibleWater.evaluated.cheapEnd,...water.visibleWater.evaluated.normalTransform])||water.visibleWater.evaluated.normalTransform.length!==16))throw new RenderingError("MalformedInput","current Water state is invalid")}
    if(frame.brushModels){let prior=-1;for(const model of frame.brushModels.models){if(model.sourceIndex<=prior||model.model<1||model.model>=(this.#active!.loadRequest.brushModels?.length??0)||!finite([...model.worldPosition,...model.worldAngles])||model.renderMode<0||model.renderMode>10)throw new RenderingError("MalformedInput","brush-model publication record is invalid");prior=model.sourceIndex}}
  }

  #setCamera(input: Camera): void {
    this.#camera.position.set(...input.position)
    if (
      this.#camera.fov !== input.verticalFovDegrees
      || this.#camera.near !== input.near
      || this.#camera.far !== input.far
    ) {
      this.#camera.fov = input.verticalFovDegrees
      this.#camera.near = input.near
      this.#camera.far = input.far
      this.#camera.updateProjectionMatrix()
    }
    const yaw = THREE.MathUtils.degToRad(input.yawDegrees)
    const pitch = THREE.MathUtils.degToRad(input.pitchDegrees)
    this.#cameraDirection.set(
      Math.cos(pitch) * Math.cos(yaw),
      Math.cos(pitch) * Math.sin(yaw),
      -Math.sin(pitch),
    )
    this.#camera.lookAt(this.#cameraTarget.copy(this.#camera.position).add(this.#cameraDirection))
    this.#viewCamera.position.copy(this.#camera.position)
    this.#viewCamera.quaternion.copy(this.#camera.quaternion)
    if (this.#active?.skyGroup) {
      this.#active.skyGroup.position.copy(this.#camera.position)
      this.#active.skyGroup.scale.setScalar(input.far)
    }
  }

  #setWaterCamera(pass: WaterFramePass, frame: Camera): void {
    this.#camera.position.set(...pass.origin)
    if (
      this.#camera.fov !== frame.verticalFovDegrees
      || this.#camera.near !== frame.near
      || this.#camera.far !== frame.far
    ) {
      this.#camera.fov = frame.verticalFovDegrees
      this.#camera.near = frame.near
      this.#camera.far = frame.far
      this.#camera.updateProjectionMatrix()
    }
    const yaw = THREE.MathUtils.degToRad(pass.angles[1])
    const pitch = THREE.MathUtils.degToRad(pass.angles[0])
    this.#cameraDirection.set(Math.cos(pitch) * Math.cos(yaw), Math.cos(pitch) * Math.sin(yaw), -Math.sin(pitch))
    this.#camera.lookAt(this.#cameraTarget.copy(this.#camera.position).add(this.#cameraDirection))
    if (this.#active?.skyGroup) {
      this.#active.skyGroup.position.copy(this.#camera.position)
      this.#active.skyGroup.scale.setScalar(frame.far)
    }
  }

  #setClip(clip: WaterFramePass["clip"]): () => void {
    const previousEnabled = this.#waterClipping.enabled
    const previousNormal = this.#waterClipPlane.normal.z
    const previousConstant = this.#waterClipPlane.constant
    if (clip) {
      const direction = clip.keep === "above" ? 1 : -1
      this.#waterClipPlane.normal.set(0, 0, direction)
      this.#waterClipPlane.constant = -direction * clip.height
      this.#waterClipping.enabled = true
    } else {
      this.#waterClipping.enabled = false
    }
    return () => {
      this.#waterClipPlane.normal.set(0, 0, previousNormal)
      this.#waterClipPlane.constant = previousConstant
      this.#waterClipping.enabled = previousEnabled
    }
  }

  #setSceneFog(fog: THREE.Fog | null): void {
    this.#scene.fog = fog
    if (!fog) {
      this.#viewFogUniforms.enabled.value = 0
      return
    }
    this.#viewFogUniforms.enabled.value = 1
    this.#viewFogUniforms.start.value = fog.near
    this.#viewFogUniforms.end.value = fog.far
    this.#viewFogUniforms.maximumDensity.value = 1
    ;(this.#viewFogUniforms.color.value as THREE.Vector3).set(fog.color.r, fog.color.g, fog.color.b)
  }

  #fog(input:FogInput|undefined):THREE.Fog|null{
    if(!input?.enabled)return null
    if(input.radial||input.maximumDensity!==1||input.end<input.start)throw new RenderingError("UnsupportedEnvironment","selected fog contract is unavailable")
    return new THREE.Fog(new THREE.Color().setRGB(input.primary[0]/255,input.primary[1]/255,input.primary[2]/255,THREE.SRGBColorSpace),input.start,input.end)
  }

  #renderSky3dPass(frame: Frame): FrameResult["sky3dPass"] {
    const scene = this.#active
    const sky = frame.sky3d
    if (!sky || !scene || frame.visibility?.sky !== 2 || sky.visibility.worldIdentity !== scene.result.environment?.identity) {
      return undefined
    }
    for (let index = 0; index < sky.visibility.surfaces.length; index += 1) {
      if (scene.worldVisibility.has(sky.visibility.surfaces[index]!)) {
        throw new RenderingError("IdentityMismatch", "main and 3D-sky world surfaces overlap")
      }
    }

    const background = this.#scene.background
    const fog = this.#scene.fog
    const autoClear = this.#backend.autoClear
    const effects = this.#effects.visible
    const particles = this.#particles.visible
    const mainVisible = scene.mainStaticProps.visible
    const skyVisible = scene.skyStaticProps.visible
    const mainWorldVisible = scene.worldBundle.visible
    const skyWorldVisible = scene.skyWorldBundle.visible
    const mainTransparentVisible = scene.mainTransparentWorld.visible
    const skyTransparentVisible = scene.skyTransparentWorld.visible
    const modelVisibility = Array.from(scene.modelOccurrenceInstances.values(), (model) => model.visible)
    const markVisibility = scene.projectedMarks.map((mark) => mark.mesh.visible)
    const waterVisibility = scene.waterMeshes.map((water) => water.mesh.visible)
    let rendered = false

    try {
      this.#setWorldVisibility(sky.visibility.surfaces, sky.visibility.cacheIdentity, 1)
      this.#setStaticPropVisibility(sky.visibility.leaves, 1, sky.camera)
      scene.worldBundle.visible = false
      scene.mainTransparentWorld.visible = false
      scene.skyWorldBundle.visible = true
      scene.skyTransparentWorld.visible = true
      scene.mainStaticProps.visible = false
      scene.skyStaticProps.visible = true
      for (const model of scene.modelOccurrenceInstances.values()) model.visible = false
      for (const mark of scene.projectedMarks) mark.mesh.visible = false
      for (const water of scene.waterMeshes) water.mesh.visible = false
      this.#effects.visible = false
      this.#particles.visible = false
      if (scene.skyGroup) scene.skyGroup.visible = true
      this.#setSceneFog(this.#fog(sky.fog))
      this.#setCamera(sky.camera)
      this.#backend.autoClear = true
      this.#backend.render(this.#scene, this.#camera)
      rendered = true
      this.#backend.clearDepth()
    } finally {
      this.#setCamera(frame.camera)
      scene.worldBundle.visible = mainWorldVisible
      scene.mainTransparentWorld.visible = mainTransparentVisible
      scene.skyWorldBundle.visible = skyWorldVisible
      scene.skyTransparentWorld.visible = skyTransparentVisible
      scene.mainStaticProps.visible = mainVisible
      scene.skyStaticProps.visible = skyVisible
      let modelIndex = 0
      for (const model of scene.modelOccurrenceInstances.values()) model.visible = modelVisibility[modelIndex++] ?? true
      for (let index = 0; index < scene.projectedMarks.length; index += 1) {
        scene.projectedMarks[index]!.mesh.visible = markVisibility[index] ?? false
      }
      for (let index = 0; index < scene.waterMeshes.length; index += 1) {
        scene.waterMeshes[index]!.mesh.visible = waterVisibility[index] ?? false
      }
      this.#effects.visible = effects
      this.#particles.visible = particles
      if (scene.skyGroup) scene.skyGroup.visible = false
      this.#setSceneFog(this.#fog(frame.fog) ?? (fog as THREE.Fog | null))
      this.#scene.background = background
      this.#backend.autoClear = autoClear
    }
    if (!rendered) return undefined
    const visibleSkyPropSources = this.#visibleStaticSources[1]
    return Object.freeze({
      phases: Object.freeze(["sky3d", "depth-reset", "main", "restore"] as const),
      skySurfaces: sky.visibility.surfaces.length,
      skyProps: visibleSkyPropSources.length,
      mainProps: this.#visibleStaticSources[0].length,
      visibleSkyPropSources,
      fog: Object.freeze({ start: sky.fog.start, end: sky.fog.end, primary: sky.fog.primary }),
      stateRestored: this.#scene.background === background && this.#effects.visible === effects && this.#particles.visible === particles,
    })
  }

  #renderWaterPasses(
    frame: Frame,
    preserveColor = false,
  ): Readonly<{
    passes: FrameResult["waterPasses"]
    timings: FrameResult["waterPassTimings"]
    restored: boolean
  }> {
    const scene = this.#active
    if (!scene) throw new RenderingError("InvalidState", "renderer has no active Water resources")
    const plan = frame.visibility?.water
    if (!plan) {
      this.#backend.autoClear = !preserveColor
      this.#backend.render(this.#scene, this.#camera)
      this.#backend.autoClear = true
      return Object.freeze({ passes: Object.freeze(["main"] as const), timings: Object.freeze([]), restored: true })
    }

    const soleMain = plan.passes.length === 1 && plan.passes[0]?.kind === "main" ? plan.passes[0] : undefined
    if (
      !plan.visibleWater && soleMain && !soleMain.clip && !soleMain.renderWaterSurface
      && soleMain.drawEntities && soleMain.drawSky2d === (frame.visibility?.sky === 2)
    ) {
      const background = this.#scene.background
      this.#backend.autoClear = !preserveColor
      this.#scene.background = soleMain.drawSky2d ? background : null
      try {
        this.#backend.render(this.#scene, this.#camera)
      } finally {
        this.#scene.background = background
      }
      return Object.freeze({ passes: Object.freeze(["main"] as const), timings: Object.freeze([]), restored: true })
    }

    if (plan.visibleWater && plan.passes.some((pass) => pass.kind !== "main" && pass.drawSky2d) && !scene.skyGroup) {
      throw new RenderingError("MissingInput", "Water auxiliary view requests the unresolved 2D sky pass")
    }
    const waterResource = plan.visibleWater
      ? scene.waterMaterials.get(plan.visibleWater.material.toLowerCase())
      : undefined
    const waterState = waterResource?.state
    if (plan.visibleWater && !waterState) {
      throw new RenderingError("MissingInput", `Water material ${plan.visibleWater.material} is unavailable`)
    }

    const markVisibility = scene.projectedMarks.map((mark) => mark.mesh.visible)
    const waterVisibility = scene.waterMeshes.map((water) => water.mesh.visible)
    const background = this.#scene.background
    const previousFog = this.#scene.fog
    const previousClearColor = this.#backend.getClearColor(new THREE.Color()).clone()
    const previousClearAlpha = this.#backend.getClearAlpha()
    const effectsVisible = this.#effects.visible
    const particlesVisible = this.#particles.visible
    const skyVisible = scene.skyGroup?.visible ?? false
    const completed: ("reflection" | "refraction" | "main" | "intersection")[] = []
    const timings: WaterPassTiming[] = []
    const uniforms = scene.waterFogUniforms

    try {
      for (const pass of plan.passes) {
        const passStarted = performance.now()
        const visible = new Set(pass.surfaces)
        this.#setWorldVisibility(pass.surfaces)
        const visibilityMilliseconds = performance.now() - passStarted
        for (const mark of scene.projectedMarks) {
          if (mark.visibility.kind === "world") mark.mesh.visible = visible.has(mark.face)
        }
        for (let index = 0; index < scene.waterMeshes.length; index += 1) {
          scene.waterMeshes[index]!.mesh.visible = pass.renderWaterSurface && waterVisibility[index] === true
        }
        if (scene.skyGroup) scene.skyGroup.visible = pass.drawSky2d
        this.#effects.visible = pass.drawEntities
        this.#particles.visible = pass.drawEntities
        this.#scene.background = pass.drawSky2d ? background : null
        this.#setWaterCamera(pass, frame.camera)

        uniforms.enabled.value = 0
        if (pass.fog.kind === "water" && waterResource?.fog) {
          const fog = waterResource.state.fog
          const color = waterResource.fog.color
          this.#backend.setClearColor(color, 1)
          if (pass.fog.heightFog && pass.kind === "refraction") {
            const range = fog.end.value - fog.start.value
            uniforms.enabled.value = 1
            uniforms.waterHeight.value = plan.visibleWater!.surfaceZ
            uniforms.eyeHeight.value = pass.origin[2]
            uniforms.inverseFogRange.value = range === 0 ? 1 : 1 / range
            const exposure = Number(scene.exposureUniform.value)
            ;(uniforms.fogColor.value as THREE.Vector3).set(
              color.r * exposure,
              color.g * exposure,
              color.b * exposure,
            )
            this.#setSceneFog(null)
          } else {
            this.#setSceneFog(waterResource.fog)
          }
        } else {
          this.#backend.setClearColor(previousClearColor, previousClearAlpha)
          this.#setSceneFog(this.#fog(frame.fog))
        }

        const sceneMilliseconds = performance.now() - passStarted - visibilityMilliseconds
        const clipStarted = performance.now()
        const restoreClip = this.#setClip(pass.clip)
        const clippingMilliseconds = performance.now() - clipStarted
        let renderMilliseconds = 0
        try {
          this.#backend.setRenderTarget(
            pass.kind === "reflection"
              ? scene.reflectionTarget
              : pass.kind === "refraction"
                ? scene.refractionTarget
                : null,
          )
          this.#backend.autoClear = pass.kind === "main" && preserveColor ? false : pass.kind !== "intersection"
          const renderStarted = performance.now()
          this.#backend.render(this.#scene, this.#camera)
          renderMilliseconds = performance.now() - renderStarted
          completed.push(pass.kind)
        } finally {
          restoreClip()
        }
        timings.push(Object.freeze({
          kind: pass.kind,
          visibilityMilliseconds,
          sceneMilliseconds,
          clippingMilliseconds,
          renderMilliseconds,
          totalMilliseconds: performance.now() - passStarted,
        }))
      }
    } finally {
      uniforms.enabled.value = 0
      this.#backend.setRenderTarget(null)
      this.#backend.autoClear = true
      this.#backend.setClearColor(previousClearColor, previousClearAlpha)
      this.#setSceneFog(previousFog as THREE.Fog | null)
      this.#scene.background = background
      this.#effects.visible = effectsVisible
      this.#particles.visible = particlesVisible
      if (scene.skyGroup) scene.skyGroup.visible = skyVisible
      this.#setWorldVisibility(frame.visibility!.surfaces)
      for (let index = 0; index < scene.projectedMarks.length; index += 1) {
        scene.projectedMarks[index]!.mesh.visible = markVisibility[index]!
      }
      for (let index = 0; index < scene.waterMeshes.length; index += 1) {
        scene.waterMeshes[index]!.mesh.visible = waterVisibility[index]!
      }
      this.#setCamera(frame.camera)
    }

    if (!completed.includes("main")) throw new RenderingError("MalformedInput", "Water view plan omitted the main pass")
    return Object.freeze({
      passes: Object.freeze(completed),
      timings: Object.freeze(timings),
      restored: this.#backend.autoClear
        && this.#scene.background === background
        && this.#scene.fog === previousFog
        && this.#effects.visible === effectsVisible
        && this.#particles.visible === particlesVisible
        && uniforms.enabled.value === 0,
    })
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

  #createParticleBatchGeometry(capacity:number):THREE.BufferGeometry{
    const geometry=new THREE.BufferGeometry(),dynamic=(array:Float32Array,size:number)=>new THREE.BufferAttribute(array,size).setUsage(THREE.DynamicDrawUsage),indices=capacity*4>0xffff?new Uint32Array(capacity*6):new Uint16Array(capacity*6)
    for(let index=0;index<capacity;index+=1){const vertex=index*4;indices.set([vertex,vertex+1,vertex+2,vertex,vertex+2,vertex+3],index*6)}
    geometry.setAttribute("position",dynamic(new Float32Array(capacity*12),3));geometry.setAttribute("uv",dynamic(new Float32Array(capacity*8),2));geometry.setAttribute("particleUvNext",dynamic(new Float32Array(capacity*8),2));geometry.setAttribute("particleSheetBlend",dynamic(new Float32Array(capacity*4),1));geometry.setAttribute("particleColor",dynamic(new Float32Array(capacity*16),4));geometry.setIndex(new THREE.BufferAttribute(indices,1));return geometry
  }

  #updateParticleBatchGeometry(
    geometry: THREE.BufferGeometry,
    items: readonly ParticleItem[],
    start: number,
    end: number,
    camera: Camera,
  ): void {
    const positions = (geometry.getAttribute("position") as THREE.BufferAttribute).array as Float32Array
    const uv = (geometry.getAttribute("uv") as THREE.BufferAttribute).array as Float32Array
    const uvNext = (geometry.getAttribute("particleUvNext") as THREE.BufferAttribute).array as Float32Array
    const sheetBlend = (geometry.getAttribute("particleSheetBlend") as THREE.BufferAttribute).array as Float32Array
    const colors = (geometry.getAttribute("particleColor") as THREE.BufferAttribute).array as Float32Array
    for (let index = 0; index < end - start; index += 1) {
      const item = items[start + index]!
      writeParticleQuad(item, camera, positions, index * 12)
      const sample = item.primarySheet!
      const current = sample.current[0]!
      const next = sample.next[0]!
      const red = ((item.color >> 16) & 255) / 255
      const green = ((item.color >> 8) & 255) / 255
      const blue = (item.color & 255) / 255
      for (let vertex = 0; vertex < 4; vertex += 1) {
        const uvOffset = index * 8 + vertex * 2
        const colorOffset = index * 16 + vertex * 4
        const right = vertex === 1 || vertex === 2
        const bottom = vertex >= 2
        uv[uvOffset] = current[right ? 2 : 0]
        uv[uvOffset + 1] = current[bottom ? 3 : 1]
        uvNext[uvOffset] = next[right ? 2 : 0]
        uvNext[uvOffset + 1] = next[bottom ? 3 : 1]
        sheetBlend[index * 4 + vertex] = sample.blend
        colors[colorOffset] = red
        colors[colorOffset + 1] = green
        colors[colorOffset + 2] = blue
        colors[colorOffset + 3] = item.opacity
      }
    }
    ;(geometry.getAttribute("position") as THREE.BufferAttribute).needsUpdate = true
    ;(geometry.getAttribute("uv") as THREE.BufferAttribute).needsUpdate = true
    ;(geometry.getAttribute("particleUvNext") as THREE.BufferAttribute).needsUpdate = true
    ;(geometry.getAttribute("particleSheetBlend") as THREE.BufferAttribute).needsUpdate = true
    ;(geometry.getAttribute("particleColor") as THREE.BufferAttribute).needsUpdate = true
    geometry.setDrawRange(0, (end - start) * 6)
  }

  #clearParticleBatches():void{
    for(const retained of this.#particleBatchMeshes){this.#particles.remove(retained.mesh);retained.mesh.geometry.dispose()}
    this.#particleBatchMeshes=[];this.#particleBatchCount=0
  }

  #stageParticleBatches(
    items: readonly ParticleItem[],
    camera: Camera,
    factor: (value: ParticleItem["blendSource"]) => THREE.BlendingDstFactor,
  ): void {
    const count = fillParticleBatchRanges(items, this.#particleBatchRanges)
    this.#particleBatchCount = count
    for (let index = 0; index < count; index += 1) {
      const batch = this.#particleBatchRanges[index]!
      const first = items[batch.start]!
      const materialIdentity = first.material.toLowerCase()
      const key = `${materialIdentity}\0${first.blendSource}\0${first.blendDestination}`
      const required = batch.end - batch.start
      let retained = this.#particleBatchMeshes[index]
      if (!retained || retained.key !== key || retained.capacity < required) {
        if (retained) {
          this.#particles.remove(retained.mesh)
          retained.mesh.geometry.dispose()
        }
        let capacity = 1
        while (capacity < required) capacity *= 2
        const geometry = this.#createParticleBatchGeometry(capacity)
        let material = this.#active!.particleBatchMaterials.get(key)
        if (!material) {
          material = this.#active!.particleMaterials.get(materialIdentity)!.clone()
          material.blending = THREE.CustomBlending
          material.blendSrc = factor(first.blendSource)
          material.blendDst = factor(first.blendDestination)
          material.transparent = true
          const state = this.#active!.materialStates.get(materialIdentity)
          if (!state) throw new RenderingError("MissingInput", `Particle material state ${first.material} is unavailable`)
          applyParticleDepthState(material, state)
          this.#active!.particleBatchMaterials.set(key, material)
        }
        const mesh = new THREE.Mesh(geometry, material)
        mesh.frustumCulled = false
        this.#particles.add(mesh)
        retained = { key, capacity, mesh }
        this.#particleBatchMeshes[index] = retained
      }
      this.#updateParticleBatchGeometry(retained.mesh.geometry, items, batch.start, batch.end, camera)
      retained.mesh.renderOrder = batch.start
      retained.mesh.visible = true
    }
    while (this.#particleBatchMeshes.length > count) {
      const retained = this.#particleBatchMeshes.pop()!
      this.#particles.remove(retained.mesh)
      retained.mesh.geometry.dispose()
    }
  }

  #applyPose(
    instance: THREE.Group,
    pose: NonNullable<ModelItem["pose"]>,
    retainGeometry: boolean,
    retainedMeshes?: THREE.Mesh[],
  ): THREE.Mesh[] {
    const meshes = retainedMeshes ?? []
    if (!retainedMeshes) instance.traverse((object) => { if (object instanceof THREE.Mesh) meshes.push(object) })
    if (meshes.length !== pose.primitives.length) {
      throw new RenderingError("IdentityMismatch", "posed model primitive count differs")
    }
    for (let primitive = 0; primitive < meshes.length; primitive += 1) {
      const object = meshes[primitive]!
      const posed = pose.primitives[primitive]!
      if (
        posed.material !== object.userData.primitiveMaterial
        || posed.positions.length !== object.geometry.getAttribute("position").count * 3
        || posed.normals.length !== object.geometry.getAttribute("normal").count * 3
      ) throw new RenderingError("IdentityMismatch", "posed model primitive differs from its template")
      if (object.userData.dynamicMaterial !== true) {
        object.material = Array.isArray(object.material) ? object.material.map((material) => material.clone()) : object.material.clone()
        object.userData.dynamicMaterial = true
      }
      if (Array.isArray(object.material)) {
        for (const material of object.material) {
          material.transparent = posed.translucent
          material.depthWrite = !posed.translucent
        }
      } else {
        object.material.transparent = posed.translucent
        object.material.depthWrite = !posed.translucent
      }
      if (retainGeometry && object.userData.dynamicGeometry === true) {
        const position = object.geometry.getAttribute("position") as THREE.BufferAttribute
        const normal = object.geometry.getAttribute("normal") as THREE.BufferAttribute
        const tangent = object.geometry.getAttribute("tangent") as THREE.BufferAttribute | undefined
        ;(position.array as Float32Array).set(posed.positions)
        position.needsUpdate = true
        ;(normal.array as Float32Array).set(posed.normals)
        normal.needsUpdate = true
        if (tangent) {
          ;(tangent.array as Float32Array).set(posed.tangents)
          tangent.needsUpdate = true
        }
      } else {
        const geometry = object.geometry.clone()
        geometry.setAttribute("position", new THREE.BufferAttribute(posed.positions.slice(), 3))
        geometry.setAttribute("normal", new THREE.BufferAttribute(posed.normals.slice(), 3))
        geometry.setAttribute("tangent", new THREE.BufferAttribute(posed.tangents.slice(), 4))
        object.geometry = geometry
        object.userData.dynamicGeometry = true
      }
    }
    return meshes
  }

  #stageViewModel(item: ModelItem, frame: Frame): readonly [number, number] {
    const key = modelKey(item.model, item.skin ?? 0)
    let retained = this.#viewModelInstances.get(item.identity)
    if (!retained || retained.model !== key) {
      if (retained) this.#disposeDynamicInstance(retained.root)
      const instance = this.#active!.modelTemplates.get(key)!.clone(true)
      if (!item.pose) throw new RenderingError("MalformedInput", "viewmodel pose is missing")
      const meshes = this.#applyPose(instance, item.pose, false)
      const root = new THREE.Group()
      root.setRotationFromMatrix(new THREE.Matrix4().set(0, -1, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 1))
      root.add(instance)
      root.traverse((object) => object.layers.set(1))
      this.#viewModels.add(root)
      retained = { model: key, root, instance, meshes, seen: 0 }
      this.#viewModelInstances.set(item.identity, retained)
    } else if (item.pose) {
      this.#applyPose(retained.instance, item.pose, true, retained.meshes)
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
    const verticalFov = sourceHorizontal4By3FovToVertical(projection.horizontalFov4By3)
    if (this.#viewCamera.fov !== verticalFov || this.#viewCamera.near !== projection.near || this.#viewCamera.far !== frame.camera.far) {
      this.#viewCamera.fov = verticalFov
      this.#viewCamera.near = projection.near
      this.#viewCamera.far = frame.camera.far
      this.#viewCamera.updateProjectionMatrix()
    }
    return sourceViewportDepthRange(projection.depthRange)
  }

  #disposeDynamicInstance(instance: THREE.Group): void {
    instance.parent?.remove(instance)
    instance.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return
      if (object.userData.dynamicMaterial === true) {
        if (Array.isArray(object.material)) {
          for (const material of object.material) material.dispose()
        } else {
          object.material.dispose()
        }
      }
      if (object.userData.dynamicGeometry === true) object.geometry.dispose()
    })
  }

  #placeDynamicInstance(instance: THREE.Group, order: number): void {
    if (instance.parent !== this.#effects) this.#effects.add(instance)
    const children = this.#effects.children
    if (children[order] === instance) return
    const previous = children.indexOf(instance)
    if (previous < 0) throw new RenderingError("InvalidState", "dynamic occurrence escaped its owning scene")
    children.splice(previous, 1)
    children.splice(order, 0, instance)
  }

  #stageDynamicItems(frame: Frame): readonly [number, number] | undefined {
    const factor = (value: ParticleItem["blendSource"]) => value === "zero" ? THREE.ZeroFactor
      : value === "one" ? THREE.OneFactor
      : value === "source-alpha" ? THREE.SrcAlphaFactor
      : THREE.OneMinusSrcAlphaFactor
    const prior = this.#stagedDynamic
    if (prior && prior.particles === frame.particles && prior.models === frame.models && prior.brushModels === frame.brushModels) {
      this.#stageParticleBatches(frame.particles ?? [], frame.camera, factor)
      if (this.#viewCamera.far !== frame.camera.far) {
        this.#viewCamera.far = frame.camera.far
        this.#viewCamera.updateProjectionMatrix()
      }
      return prior.viewModelDepthRange
    }

    const revision = ++this.#dynamicRevision
    let effectOrder = 0
    let viewModelDepthRange: readonly [number, number] | undefined
    try {
      const particleItems = frame.particles ?? []
      for (const item of particleItems) {
        if (item.secondarySheet) {
          throw new RenderingError("UnsupportedFeature", "dual-sequence Particle rendering requires exact selected material support")
        }
      }

      for (const item of frame.brushModels?.models ?? []) {
        if (!item.draw) continue
        const template = this.#active!.brushModelTemplates.get(item.model)
        if (!template) continue
        const appearance = `${item.renderMode}:${item.color[0]}:${item.color[1]}:${item.color[2]}:${item.color[3]}`
        let retained = this.#dynamicBrushInstances.get(item.sourceIndex)
        if (!retained || retained.model !== item.model || retained.appearance !== appearance) {
          if (retained) this.#disposeDynamicInstance(retained.instance)
          const instance = template.clone(true)
          const modulation = new THREE.Color(item.color[0] / 255, item.color[1] / 255, item.color[2] / 255)
          instance.traverse((object) => {
            if (!(object instanceof THREE.Mesh)) return
            const original = object.material
            const prepare = (value: THREE.Material) => {
              const material = value.clone()
              if ("color" in material && material.color instanceof THREE.Color) material.color.multiply(modulation)
              material.opacity *= item.color[3] / 255
              material.transparent = material.transparent || item.renderMode !== 0 || item.color[3] < 255
              return material
            }
            object.material = Array.isArray(original) ? original.map(prepare) : prepare(original)
            object.userData.dynamicMaterial = true
          })
          retained = { model: item.model, appearance, instance, seen: revision }
          this.#dynamicBrushInstances.set(item.sourceIndex, retained)
        }
        retained.seen = revision
        sourceTransform(retained.instance, item.worldPosition, item.worldAngles)
        retained.instance.userData.identity = item.sourceIndex
        retained.instance.userData.renderMode = item.renderMode
        retained.instance.userData.renderFx = item.renderFx
        retained.instance.userData.effects = item.effects
        retained.instance.userData.mover = item.mover
        this.#placeDynamicInstance(retained.instance, effectOrder++)
      }

      for (const item of frame.models ?? []) {
        if (item.viewModel) {
          const nextDepthRange = this.#stageViewModel(item, frame)
          if (
            viewModelDepthRange
            && (viewModelDepthRange[0] !== nextDepthRange[0] || viewModelDepthRange[1] !== nextDepthRange[1])
          ) throw new RenderingError("IdentityMismatch", "viewmodel depth ranges differ in one pass")
          viewModelDepthRange = nextDepthRange
          this.#viewModelInstances.get(item.identity)!.seen = revision
          continue
        }

        const key = modelKey(item.model, item.skin ?? 0)
        let retained = this.#dynamicModelInstances.get(item.identity)
        if (!retained || retained.model !== key) {
          if (retained) this.#disposeDynamicInstance(retained.instance)
          retained = { model: key, instance: this.#active!.modelTemplates.get(key)!.clone(true), seen: revision }
          this.#dynamicModelInstances.set(item.identity, retained)
        }
        retained.seen = revision
        const staticInstance = this.#active!.modelOccurrenceInstances.get(item.identity)
        if (staticInstance) staticInstance.visible = false
        if (item.pose) {
          retained.meshes = this.#applyPose(retained.instance, item.pose, retained.meshes !== undefined, retained.meshes)
        }
        if (item.angles) sourceTransform(retained.instance, item.position, item.angles)
        else {
          retained.instance.position.set(...item.position)
          retained.instance.quaternion.set(...item.orientation!)
        }
        retained.instance.scale.setScalar(item.scale)
        retained.instance.userData.identity = item.identity
        this.#placeDynamicInstance(retained.instance, effectOrder++)
      }
      this.#stageParticleBatches(particleItems, frame.camera, factor)
    } catch (error) {
      if (error instanceof RenderingError) throw error
      throw new RenderingError("BoundExceeded", `render item staging failed: ${String(error)}`)
    }

    for (const [identity, retained] of this.#dynamicBrushInstances) {
      if (retained.seen === revision) continue
      this.#disposeDynamicInstance(retained.instance)
      this.#dynamicBrushInstances.delete(identity)
    }
    for (const [identity, retained] of this.#dynamicModelInstances) {
      if (retained.seen === revision) continue
      this.#disposeDynamicInstance(retained.instance)
      this.#dynamicModelInstances.delete(identity)
      const staticInstance = this.#active!.modelOccurrenceInstances.get(identity)
      if (staticInstance) staticInstance.visible = true
    }
    for (const [identity, retained] of this.#viewModelInstances) {
      if (retained.seen === revision) continue
      this.#disposeDynamicInstance(retained.root)
      this.#viewModelInstances.delete(identity)
    }
    this.#stagedDynamic = Object.freeze({
      particles: frame.particles,
      models: frame.models,
      brushModels: frame.brushModels,
      viewModelDepthRange,
    })
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
      if (this.#active) {
        this.#active.worldBundle.needsUpdate = true
        this.#active.skyWorldBundle.needsUpdate = true
      }
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
    this.#clearDynamic(this.#effects)
    this.#dynamicModelInstances.clear()
    this.#dynamicBrushInstances.clear()
    this.#clearParticleBatches()
    this.#clearDynamic(this.#viewModels)
    this.#viewModelInstances.clear()
    this.#stagedDynamic = undefined
    this.#worldVisibilitySurfaces = undefined
    this.#worldVisibilityIdentity = undefined
    this.#skyWorldVisibilitySurfaces = undefined
    this.#skyWorldVisibilityIdentity = undefined
    this.#visibleStaticIndices[0].clear()
    this.#visibleStaticIndices[1].clear()
    this.#nextVisibleStaticIndices[0].clear()
    this.#nextVisibleStaticIndices[1].clear()
    this.#visibleStaticSources = [Object.freeze([]), Object.freeze([])]
    this.#visibleWorldMarkFaces.clear()
    this.#visibleProjectedMarkCount = 0
    this.#world.clear()
    const oldBackend = this.#backend
    try {
      try {
        oldBackend.backend.context?.unconfigure()
      } catch {
        /* device already lost */
      }
      this.#restoreOrderedBundles?.()
      this.#restoreOrderedBundles = undefined
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
        this.#world.add(rebuilt.group)
        this.#world.updateMatrixWorld(true)
        await this.#prepareWaterPipelines(
          rebuilt,
          active.loadRequest.environment,
          undefined,
          this.#loadOrdinal,
        )
        this.#active = rebuilt
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
    this.#dynamicModelInstances.clear()
    this.#dynamicBrushInstances.clear()
    this.#clearParticleBatches()
    this.#clearDynamic(this.#viewModels)
    this.#viewModelInstances.clear()
    this.#stagedDynamic = undefined
    this.#worldVisibilitySurfaces = undefined
    this.#worldVisibilityIdentity = undefined
    this.#skyWorldVisibilitySurfaces = undefined
    this.#skyWorldVisibilityIdentity = undefined
    this.#visibleStaticIndices[0].clear()
    this.#visibleStaticIndices[1].clear()
    this.#nextVisibleStaticIndices[0].clear()
    this.#nextVisibleStaticIndices[1].clear()
    this.#visibleStaticSources = [Object.freeze([]), Object.freeze([])]
    this.#visibleWorldMarkFaces.clear()
    this.#visibleProjectedMarkCount = 0
    const active = this.#active
    this.#active = undefined
    this.#world.clear()
    if (active) await this.#retire(active)
    try {
      await this.#backend.backend.device?.queue.onSubmittedWorkDone()
    } catch {
      /* device already unavailable */
    }
    this.#restoreOrderedBundles?.()
    this.#restoreOrderedBundles = undefined
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
