import { sha256 } from "@noble/hashes/sha2.js"
import type { ModelEyeState, ModelLightingInput, ModelLocalLight } from "@playsrc/rendering"
import { decodeRuntimeModelRegistry } from "@playsrc/rendering/runtime-map"

const LIMIT = 512 * 1024 * 1024
export type ModelArtifact = Readonly<{
  identity: string
  profile: "world" | "viewmodel"
  sha256: string
  bytes: Uint8Array
  skinCount: number
  bodygroupCounts: readonly number[]
  attachments: ReadonlyMap<string, Float32Array>
  descriptor:
    | Readonly<{ kind: "world"; staticPropRoot: boolean; depthRange: readonly [number, number]; frontFace: "clockwise" | "counter-clockwise"; cullFace: "back" }>
    | Readonly<{
        kind: "viewmodel"
        horizontalFov4By3: number
        minimumFov: number
        maximumFov: number
        near: number
        depthRange: readonly [number, number]
        drawsAfterWorld: boolean
        opaqueBeforeTranslucent: boolean
        optionalViewSpaceYReflection: boolean
        frontFace: "clockwise" | "counter-clockwise"
        cullFace: "back"
      }>
  sequences: readonly Readonly<{
    label: string
    activity: string
    index: number
    timingAvailable: boolean
    framesPerSecond: number
    weightedFrameCount: number
    cyclesPerSecond: number
    durationSeconds: number
    looping: boolean
  }>[]
}>
export type StaticMaterialState = Readonly<{
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
  fog: number
  wireframe: boolean
  noDraw: boolean
  vertexColor: boolean
  vertexAlpha: boolean
  translucentQueue: boolean
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
export type ParticleTextureArtifact = AuthoredTextureArtifact & Readonly<{ material: string; materialPath: string; spriteCard: import("@playsrc/rendering").SpriteCardInput | null }>
export type SoundScriptNode = Readonly<{ key: string; value: string | readonly SoundScriptNode[] }>
export type AudioArtifact = Readonly<{
  unavailable: ReadonlySet<string>
  patches: ReadonlyMap<string, Readonly<{ sampleRate: number; frames: number; loopStartSeconds: number | null }>>
  mixerSha256: string
  mixerGain: number
  documents: readonly Readonly<{
    logicalPath: string
    sourceSha256: string
    entries: readonly SoundScriptNode[]
  }>[]
}>
export type ModelOccurrenceMatrix = Readonly<{ entity: number; model: string; skin:number; body:number; pipelineAnimation: string | null; origin:readonly[number,number,number]; angles:readonly[number,number,number]; matrix: Float32Array; lighting: ModelLightingInput; eyes: readonly ModelEyeState[] }>
export type BrushModelArtifact=Readonly<{index:number;bounds:readonly[readonly[number,number,number],readonly[number,number,number]];origin:readonly[number,number,number];headNode:number;surfaceRange:readonly[number,number];vertexCount:number;triangleCount:number;materials:readonly number[];entities:readonly number[]}>
export type ModelTextureBinding = Readonly<{
  kind: "material" | "model"
  role: number
  colorRead: "srgb" | "linear" | "format-dependent"
  logicalPath: string
}>
export type CloakState = Readonly<{ enabled: boolean; factor: number; colorTint: readonly [number, number, number]; refractAmount: number }>
export type ModelMaterialArtifact = Readonly<{
  identity: string
  cloakProxy: number
  shader: "unlit-generic" | "unlit-two-texture" | "modulate" | "vertex-lit-generic" | "eye-refract" | "eyes"
  vertexRequirements: number
  bindings: readonly ModelTextureBinding[]
  environmentMap: null | Readonly<{ tint: readonly [number, number, number]; contrast: number; saturation: number }>
  opacity:"opaque"|"translucent";framebuffer:"none"|"potential"|"current";requiredInputs:readonly string[]
  state:
    | Readonly<{
        kind: "vertex-lit-generic"
        halfLambert: boolean
        selfIllumination: null | Readonly<{ source: number; tint: readonly [number, number, number]; fresnel: readonly [number, number, number] }>
        phong: null | Readonly<{
          maskSource: number
          invertMask: boolean
          albedoTint: boolean
          exponent: number
          exponentFactor: number
          tint: readonly [number, number, number]
          boost: number
          fresnel: readonly [number, number, number]
          packedFresnel: readonly [number, number, number]
          rim: null | Readonly<{ exponent: number; boost: number; exponentTextureAlphaMask: boolean }>
        }>
        cloak: CloakState
        sheen: Readonly<{
          enabled: boolean
          sourceAlphaBlend: boolean
          depthWrite: boolean
          maskFrame: number
          maskDirection: number
          shaderIndex: number
          tint: readonly [number, number, number]
          maskScale: readonly [number, number]
          maskOffset: readonly [number, number]
        }>
      }>
    | Readonly<{
        kind: "eye-refract"
        sphereTextureKill: boolean
        raytraceSphere: boolean
        halfLambert: boolean
        dilation: number
        glossiness: number
        parallaxStrength: number
        corneaBumpStrength: number
        ambientOcclusionColor: readonly [number, number, number]
        eyeballRadius: number
        cloak: CloakState
      }>
    | Readonly<{ kind: "eyes"; halfLambert: boolean; dilation: number }>
    | Readonly<{ kind: "unlit-generic"; colorModulation: readonly [number, number, number] }>
    | Readonly<{ kind: "unlit-two-texture"; secondFrameRate: number | null; secondScrollRate: number | null; secondScrollAngle: number | null }>
    | Readonly<{ kind: "modulate" }>
}>
export type AuthoredTexturePlane = Readonly<{
  mip: number
  frame: number
  face: number
  slice: number
  width: number
  height: number
  rgba: Uint8Array
}>
export type AuthoredTextureArtifact = Readonly<{
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
  planes: readonly AuthoredTexturePlane[]
}>
export type SupplementalTexture = Readonly<{
  material: string
  logicalPath: string
  width: number
  height: number
  sha256: string
  rgba: Uint8Array
}>
export type DirectionalTextureArtifact = Readonly<{
  material: string
  kind: "normal" | "ssbump"
  logicalPath: string
  width: number
  height: number
  sha256: string
  authored: AuthoredTextureArtifact
  uvTransform: readonly [number, number, number, number, number, number]
}>
export type EnvironmentFragment = Readonly<{
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
export type EnvironmentMark = Readonly<{
  status: number
  kind: number
  enabled: boolean
  dynamic: boolean
  material: string
  fragments: readonly EnvironmentFragment[]
  sourceIndex: number
  entity: number | null
  overlayId: number | null
  origin: readonly [number, number, number]
  materialSha256: string | null
  receiver: null | Readonly<{
    entity: bigint | null
    model: number
    parentEntity: number | null
    localOrigin: readonly [number, number, number]
    origin: readonly [number, number, number]
    angles: readonly [number, number, number]
  }>
  targetFaces: readonly number[]
  renderOrder: number
  fadeDistancesSquared: readonly [number, number] | null
  lowPriority: boolean
  parentEntity: number | null
  activation: "map" | "input" | "compiled"
  lifetime: "permanent" | "pool-managed"
  normalOffset: number
  polygonOffset: "none" | "decal"
}>
export type CubemapFact = Readonly<{
  index: number
  origin: readonly [number, number, number]
  logicalPath: string
  sha256: string
  width: number
  height: number
}>
export type WaterSurfaceFact = Readonly<{
  profile: "ldr" | "hdr"
  selected: boolean
  face: number
  model: number
  material: number
  bounds: readonly [readonly [number, number, number], readonly [number, number, number]]
  cubemapKind: number
  cubemapSample: number | null
  textureInfo: number
  fogVolume: number | null
  plane: readonly [number, number, number, number]
  bindings: Readonly<{ environment: boolean; reflection: boolean; refraction: boolean }>
}>
export type WaterVolumeFact = Readonly<{
  index: number
  surfaceZ: number
  minimumZ: number
  bounds: readonly [readonly [number, number, number], readonly [number, number, number]]
  leaves: readonly number[]
  cubemapKind: number
  cubemapSample: number | null
  textureInfo: number
  surfaceMaterial: number
  bottomMaterial: null | Readonly<{ kind: "map"; index: number } | { kind: "dependency"; identity: string }>
  clusters: readonly number[]
  areas: readonly number[]
  contents: number
  plane: readonly [number, number, number, number]
  surfaceTranslucent: boolean
  bottomTranslucent: boolean | null
  surfaceBindings: Readonly<{ environment: boolean; reflection: boolean; refraction: boolean }>
  bottomBindings: Readonly<{ environment: boolean; reflection: boolean; refraction: boolean }> | null
}>
type Effective<T> = Readonly<{ value: T; origin: "authored" | "shader-initializer" | "type-initializer" }>
export type WaterMaterialArtifact = Readonly<{
  identity: string
  mapMaterial: number | null
  shader: "dx90" | "dx9-hdr"
  opacity: "opaque" | "translucent"
  textures: readonly Readonly<{ role: number; disposition: "source" | "environment" | "render-target"; colorRead: "srgb" | "linear" | "format-dependent"; parameter: string; reference: string; logicalPath: string | null }>[]
  bottomMaterial: string | null
  underwaterOverlay: string | null
  baseFrame: Effective<number>
  normalFrame: Effective<number>
  environmentFrame: Effective<number>
  normalTransform: Readonly<{ parameter: string; matrix: Float32Array; origin: Effective<number>["origin"]; proxyMutated: boolean }>
  scale: Effective<readonly [number, number]>
  time: Effective<number>
  waterDepth: Effective<number>
  aboveWater: Effective<boolean>
  reflectAmount: Effective<number>
  refractAmount: Effective<number>
  reflectTint: Effective<readonly [number, number, number]>
  refractTint: Effective<readonly [number, number, number]>
  reflectionBlendFactor: Effective<number>
  fog: Readonly<{ enabled: Effective<boolean> | null; color: Effective<readonly [number, number, number]>; start: Effective<number>; end: Effective<number> }>
  cheapStart: Effective<number>
  cheapEnd: Effective<number>
  forceCheap: Effective<boolean>
  forceExpensive: Effective<boolean>
  reflectEntities: Effective<boolean>
  blurRefraction: Effective<boolean>
  noLowEndLightmap: Effective<boolean>
  scroll: readonly [Effective<readonly [number, number, number]>, Effective<readonly [number, number, number]>]
  fresnel: Readonly<{ cheapEnabled: boolean; expensiveConstant: readonly [number, number, number, number] }>
  requiredInputs: readonly number[]
}>
export type RefractMaterialArtifact = Readonly<{
  identity: string
  normal: Readonly<{
    role: number
    disposition: "source"
    colorRead: "linear"
    parameter: string
    reference: string
    logicalPath: string
  }>
  blurAmount: 0 | 1
  ignoreDepth: boolean
  refractAmount: number
  refractTint: readonly [number, number, number]
  normalFrame: number
  normalTransform: Float32Array
}>
export type WorldMaterialTextureArtifact = Readonly<{
  role: number
  disposition: "source" | "environment" | "render-target"
  colorRead: "srgb" | "linear" | "format-dependent"
  parameter: string
  reference: string
  logicalPath: string | null
  initialFrame: number | null
  frameProxyMutated: boolean
  transform: Float32Array | null
  transformProxyMutated: boolean
}>
export type WorldMaterialArtifact = Readonly<{
  identity: string
  mapMaterial: number
  shader: "lightmapped-generic" | "world-vertex-transition"
  textures: readonly WorldMaterialTextureArtifact[]
  proxies: readonly Readonly<{ name: string; disposition: "handled" | "malformed" | "unsupported" }>[]
  environmentMap: null | Readonly<{ tint: readonly [number, number, number]; contrast: number; saturation: number }>
  fresnelReflection: number
}>
export type FogArtifact = Readonly<{ enabled: boolean; blend: boolean; radial: boolean; direction: readonly [number, number, number]; primary: readonly [number, number, number, number]; secondary: readonly [number, number, number, number]; start: number; end: number; maximumDensity: number; farZ: number | null; transitionDuration: number }>
export type EnvironmentControllerArtifact = Readonly<{ entity: number; classname: string; kind: number; rawFields: readonly Readonly<{ key: string; value: string }>[]; state:
  |FogArtifact
  |Readonly<{origin:readonly[number,number,number];scale:number;area:number;fog:FogArtifact}>
  |Readonly<{start:number;end:number}>
  |Readonly<{values:readonly number[]}>
  |Readonly<{angles:readonly[number,number,number];color:readonly number[];maximumDistance:number;disabled:boolean}>
  |Readonly<Record<never,never>> }>
export type EnvironmentArtifact = Readonly<{
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
  markRecords: readonly EnvironmentMark[]
  textures: readonly SupplementalTexture[]
  directionalTextures: readonly DirectionalTextureArtifact[]
  sky: Readonly<{ name: string; faces: readonly Readonly<{ face: number; material: string; sha256: string; encoding: "srgb" | "linear" | "hdr-rgbs"; selectedTextures: readonly Readonly<{ logicalPath: string; sha256: string }>[]; textureTransform: readonly number[]; color: readonly [number, number, number] }>[] }> | null
  cubemapFacts: readonly CubemapFact[]
  waterSurfaceFacts: readonly WaterSurfaceFact[]
  waterVolumeFacts: readonly WaterVolumeFact[]
  controllerFacts: readonly Readonly<{ entity: number; classname: string; kind: number }>[]
  collisionWorldIdentity: string
  receiverSnapshotRevision: bigint
  placementRevision: bigint
  leafMinimumDistanceToWater: Uint16Array
  waterMaterials: ReadonlyMap<string, WaterMaterialArtifact>
  refractMaterials: ReadonlyMap<string, RefractMaterialArtifact>
  worldMaterials: ReadonlyMap<string, WorldMaterialArtifact>
  authoredTextures: ReadonlyMap<string, AuthoredTextureArtifact>
  controllersState: readonly EnvironmentControllerArtifact[]
  masterFogController: number | null
}>
export type PresentationArtifacts = Readonly<{
  models: ReadonlyMap<string, ModelArtifact>
  particleMaterials: readonly string[]
  materialStates: ReadonlyMap<string, StaticMaterialState>
  particleTextures: readonly ParticleTextureArtifact[]
  audio: AudioArtifact
  modelOccurrences: readonly ModelOccurrenceMatrix[]
  modelMaterials: ReadonlyMap<string, ModelMaterialArtifact>
  authoredTextures: ReadonlyMap<string, AuthoredTextureArtifact>
  environment: EnvironmentArtifact
  brushModels:readonly BrushModelArtifact[]
  staticProps: StaticPropArtifact
}>
export type StaticPropArtifact = Readonly<{
  aggregateSha256: string
  modelCount: number
  count: number
  source: Uint32Array
  dictionaryModel: Uint32Array
  presentationModel: Uint32Array
  transform: Float32Array
  skin: Int32Array
  body: Uint32Array
  lod: Uint32Array
  fades: Float32Array
  flags: Uint32Array
  solidity: Uint8Array
  ownership: Uint8Array
  lightingKind: Uint8Array
  lightingOrigin: Float32Array
  leafOffsets: Uint32Array
  leaves: Uint16Array
  areas: Uint16Array
  vhvObjects: Uint32Array
  runtimeAmbient: Float32Array
  runtimeLightOffsets: Uint32Array
  runtimeLights: Readonly<{
    source: number
    kind: number
    style: number
    ratio: number
    direction: readonly [number, number, number]
    intensity: readonly [number, number, number]
    origin: readonly [number, number, number]
    normal: readonly [number, number, number]
    stopDot: number
    stopDot2: number
    exponent: number
    radius: number
    attenuation: readonly [number, number, number]
  }>[]
  models: readonly string[]
  vhv: readonly Readonly<{ occurrence:number;model:number;profile:0|1;sha256:string;joinSha256:string;vertexCount:number;meshes:readonly Readonly<{primitive:number;lod:number;vertexCount:number;colors:Uint8Array}>[]}>[]
  runtimeLightingCount: number
}>
export class ArtifactError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "ArtifactError"
  }
}
class Reader {
  readonly view: DataView
  offset = 0
  readonly decoder = new TextDecoder("utf-8", { fatal: true })
  constructor(readonly bytes: Uint8Array) {
    if (bytes.byteLength < 12 || bytes.byteLength > LIMIT) throw new ArtifactError("artifact length")
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  }
  take(n: number) {
    if (n < 0 || this.offset + n > this.bytes.length) throw new ArtifactError("artifact truncated")
    const value = this.bytes.subarray(this.offset, this.offset + n)
    this.offset += n
    return value
  }
  u8() {
    if (this.offset >= this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    return this.bytes[this.offset++]!
  }
  u16() {
    if (this.offset + 2 > this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    const v = this.view.getUint16(this.offset, true)
    this.offset += 2
    return v
  }
  u32() {
    if (this.offset + 4 > this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    const v = this.view.getUint32(this.offset, true)
    this.offset += 4
    return v
  }
  i32() {
    if (this.offset + 4 > this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    const v = this.view.getInt32(this.offset, true)
    this.offset += 4
    return v
  }
  u64() {
    if (this.offset + 8 > this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    const v = this.view.getBigUint64(this.offset, true)
    this.offset += 8
    return v
  }
  f32() {
    if (this.offset + 4 > this.bytes.byteLength) throw new ArtifactError("artifact truncated")
    const v = this.view.getFloat32(this.offset, true)
    this.offset += 4
    if (!Number.isFinite(v)) throw new ArtifactError("non-finite scalar")
    return v
  }
  blob(max = LIMIT) {
    const n = this.u32()
    if (n > max) throw new ArtifactError("field limit")
    return this.take(n)
  }
  decode(value: Uint8Array): string {
    return this.decoder.decode(value.buffer instanceof SharedArrayBuffer ? value.slice() : value)
  }
  text() {
    return this.decode(this.blob(4096))
  }
}
const hex = (bytes: Uint8Array) => Array.from(bytes, (v) => v.toString(16).padStart(2, "0")).join("")
const digest = async (bytes: Uint8Array) => bytes.buffer instanceof SharedArrayBuffer
  ? hex(sha256(bytes))
  : hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)))
function parseEnvironment(
  bytes: Uint8Array,
  resources: ReadonlyMap<string, Uint8Array>,
  sharedTextures: Map<string, AuthoredTextureArtifact>,
): EnvironmentArtifact {
  const r = new Reader(bytes)
  if (r.decode(r.take(4)) !== "PENV" || r.u32() !== 7) throw new ArtifactError("environment identity")
  const profile = r.u8()
  if ((profile !== 0 && profile !== 1) || r.u8() || r.u8() || r.u8()) throw new ArtifactError("environment profile")
  const identity = hex(r.take(32)),
    v = Array.from({ length: 10 }, () => r.u32()),
    markRecords: any[] = []
  for (let mark = 0; mark < v[7]!; mark++) {
    const status = r.u8(),
      kind = r.u8(),
      enabled = r.u8() === 1,
      dynamic = r.u8() === 1,
      material = r.text(),
      fragments: EnvironmentFragment[] = []
    for (let n = r.u32(); n > 0; n--) {
      const model = r.u32(),
        face = r.u32(),
        vertices = r.u32(),
        positions = new Float32Array(vertices * 3),
        normals = new Float32Array(vertices * 3),
        uv = new Float32Array(vertices * 2)
      for (let i = 0; i < positions.length; i++) positions[i] = r.f32()
      for (let i = 0; i < normals.length; i++) normals[i] = r.f32()
      for (let i = 0; i < uv.length; i++) uv[i] = r.f32()
      const indices = new Uint32Array(r.u32() * 3)
      for (let i = 0; i < indices.length; i++) indices[i] = r.u32()
      fragments.push(Object.freeze({ model, face, positions, normals, uv, indices, lightmapUv: new Float32Array(), visibility: null as any }))
    }
    markRecords.push(Object.freeze({ status, kind, enabled, dynamic, material, fragments: Object.freeze(fragments) }))
  }
  const textures: SupplementalTexture[] = []
  for (let n = r.u32(); n > 0; n--) {
    const material = r.text(),
      logicalPath = r.text(),
      width = r.u32(),
      height = r.u32(),
      sha256 = hex(r.take(32)),
      rgba = r.blob(256 * 1024 * 1024)
    if (width * height * 4 !== rgba.length) throw new ArtifactError("environment texture length")
    textures.push(Object.freeze({ material, logicalPath, width, height, sha256, rgba }))
  }
  const sky =
    r.u8() === 1
      ? Object.freeze({
          name: r.text(),
          faces: Object.freeze(
            Array.from({ length: r.u32() }, () =>
              (() => { const face=r.u8(),encoding=r.u8();if(encoding>2||r.u8()||r.u8())throw new ArtifactError("sky face encoding");return Object.freeze({ face, encoding:(["srgb","linear","hdr-rgbs"] as const)[encoding]!, material: r.text(), sha256: hex(r.take(32)), selectedTextures: Object.freeze([]) }) })(),
            ),
          ),
        })
      : null
  const cubemapFacts = Object.freeze(
    Array.from({ length: v[4]! }, () =>
      Object.freeze({
        index: r.u32(),
        origin: Object.freeze([r.i32(), r.i32(), r.i32()]) as readonly [number, number, number],
        logicalPath: r.text(),
        sha256: hex(r.take(32)),
        width: r.u32(),
        height: r.u32(),
      }),
    ),
  )
  const bounds = () =>
    Object.freeze([
      Object.freeze([r.f32(), r.f32(), r.f32()]),
      Object.freeze([r.f32(), r.f32(), r.f32()]),
    ]) as readonly [readonly [number, number, number], readonly [number, number, number]]
  const waterSurfaceFacts = Object.freeze(
    Array.from({ length: v[5]! }, () => {
      const waterProfile = r.u8(),
        selected = r.u8() === 1
      r.take(2)
      const face = r.u32(),
        model = r.u32(),
        material = r.u32(),
        waterBounds = bounds(),
        cubemapKind = r.u8()
      r.take(3)
      const sample = r.u32()
      return Object.freeze({
        profile: waterProfile === 0 ? ("ldr" as const) : ("hdr" as const),
        selected,
        face,
        model,
        material,
        bounds: waterBounds,
        cubemapKind,
        cubemapSample: sample === 0xffff_ffff ? null : sample,
      })
    }),
  )
  const waterVolumeFacts = Object.freeze(
    Array.from({ length: v[6]! }, () => {
      const index = r.u32(),
        surfaceZ = r.f32(),
        minimumZ = r.f32(),
        waterBounds = bounds(),
        leaves = Object.freeze(Array.from({ length: r.u32() }, () => r.u32())),
        cubemapKind = r.u8()
      r.take(3)
      const sample = r.u32()
      return Object.freeze({
        index,
        surfaceZ,
        minimumZ,
        bounds: waterBounds,
        leaves,
        cubemapKind,
        cubemapSample: sample === 0xffff_ffff ? null : sample,
      })
    }),
  )
  const controllerFacts = Object.freeze(
    Array.from({ length: v[9]! }, () => Object.freeze({ entity: r.u32(), classname: r.text(), kind: r.u8() })),
  )
  const collisionWorldIdentity = hex(r.take(32)), receiverSnapshotRevision = r.u64(), placementRevision = r.u64()
  if (r.u32() !== markRecords.length) throw new ArtifactError("environment mark extension count")
  for (let index = 0; index < markRecords.length; index++) {
    const base = markRecords[index]!, sourceIndex = r.u32(), entityValue = r.u32(), overlayValue = r.i32(), origin = tuple3(r)
    const hasMaterialSha = r.u8(), renderOrder = r.u8(), lowPriority = r.u8(), activation = r.u8(), lifetime = r.u8(), hasFade = r.u8(), polygonOffset = r.u8()
    if (hasMaterialSha > 1 || lowPriority > 1 || activation > 2 || lifetime > 1 || hasFade > 1 || polygonOffset > 1 || r.u8()) throw new ArtifactError("environment mark extension flags")
    const materialSha = hex(r.take(32)), fade = tuple2(r), normalOffset = r.f32(), parentValue = r.u32(), hasReceiver = r.u8()
    if (hasReceiver > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("environment mark receiver flag")
    let receiver: EnvironmentMark["receiver"] = null
    if (hasReceiver === 1) {
      const receiverEntity = r.u64(), model = r.u32(), receiverParent = r.u32()
      receiver = Object.freeze({
        entity: receiverEntity === 0xffff_ffff_ffff_ffffn ? null : receiverEntity,
        model,
        parentEntity: receiverParent === 0xffff_ffff ? null : receiverParent,
        localOrigin: tuple3(r), origin: tuple3(r), angles: tuple3(r),
      })
    } else r.take(52)
    const targetFaces = Object.freeze(Array.from({ length: r.u32() }, () => r.u32()))
    if (r.u32() !== base.fragments.length) throw new ArtifactError("environment mark fragment extension count")
    const fragments = base.fragments.map((fragment: any) => {
      const visibilityKind = r.u8()
      if (visibilityKind > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("environment mark visibility")
      let visibility: EnvironmentFragment["visibility"]
      if (visibilityKind === 0) {
        const leaves = r.u32(), clusters = r.u32(), areas = r.u32()
        visibility = Object.freeze({
          kind: "world" as const,
          leaves: Object.freeze(Array.from({ length: leaves }, () => r.u32())),
          clusters: Object.freeze(Array.from({ length: clusters }, () => { const value = r.u16(); return value >= 0x8000 ? value - 0x1_0000 : value })),
          areas: Object.freeze(Array.from({ length: areas }, () => r.u32())),
        })
      } else visibility = Object.freeze({ kind: "brush-model" as const, entity: r.u64(), model: r.u32() })
      const lightmapVertices = r.u32(), lightmapUv = new Float32Array(lightmapVertices * 2)
      if (lightmapVertices !== fragment.positions.length / 3) throw new ArtifactError("environment mark lightmap UV count")
      for (let value = 0; value < lightmapUv.length; value++) lightmapUv[value] = r.f32()
      return Object.freeze({ ...fragment, lightmapUv, visibility })
    })
    if ((hasMaterialSha === 0 && materialSha !== "0".repeat(64)) || (hasFade === 0 && (fade[0] !== 0 || fade[1] !== 0))) throw new ArtifactError("environment mark absent field")
    markRecords[index] = Object.freeze({
      ...base, sourceIndex, entity: entityValue === 0xffff_ffff ? null : entityValue,
      overlayId: overlayValue === -0x8000_0000 ? null : overlayValue, origin,
      materialSha256: hasMaterialSha === 1 ? materialSha : null, receiver, targetFaces, renderOrder,
      fadeDistancesSquared: hasFade === 1 ? fade : null, lowPriority: lowPriority === 1,
      parentEntity: parentValue === 0xffff_ffff ? null : parentValue,
      activation: (["map", "input", "compiled"] as const)[activation]!,
      lifetime: lifetime === 0 ? "permanent" as const : "pool-managed" as const,
      normalOffset, polygonOffset: polygonOffset === 0 ? "none" as const : "decal" as const,
      fragments: Object.freeze(fragments),
    })
  }
  const leafDistanceCount = r.u32(), leafMinimumDistanceToWater = new Uint16Array(leafDistanceCount)
  for (let index = 0; index < leafDistanceCount; index++) leafMinimumDistanceToWater[index] = r.u16()
  const originName = (value: number): Effective<number>["origin"] => {
    if (value > 2) throw new ArtifactError("water parameter origin")
    return (["authored", "shader-initializer", "type-initializer"] as const)[value]!
  }
  const effectiveF32 = (): Effective<number> => { const value = r.f32(), origin = originName(r.u8()); if (r.u8() || r.u8() || r.u8()) throw new ArtifactError("water scalar"); return Object.freeze({ value, origin }) }
  const effectiveI32 = (): Effective<number> => { const value = r.i32(), origin = originName(r.u8()); if (r.u8() || r.u8() || r.u8()) throw new ArtifactError("water integer"); return Object.freeze({ value, origin }) }
  const effectiveBool = (): Effective<boolean> => { const value = r.u8(), origin = originName(r.u8()); if (value > 1 || r.u8() || r.u8()) throw new ArtifactError("water boolean"); return Object.freeze({ value: value === 1, origin }) }
  const effectiveVec2 = (): Effective<readonly [number, number]> => { const value = tuple2(r), origin = originName(r.u8()); if (r.u8() || r.u8() || r.u8()) throw new ArtifactError("water vec2"); return Object.freeze({ value, origin }) }
  const effectiveVec3 = (): Effective<readonly [number, number, number]> => { const value = tuple3(r), origin = originName(r.u8()); if (r.u8() || r.u8() || r.u8()) throw new ArtifactError("water vec3"); return Object.freeze({ value, origin }) }
  const waterMaterials = new Map<string, WaterMaterialArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase(), mapValue = r.u32(), shader = r.u8(), opacity = r.u8(), textureCount = r.u8(), requiredCount = r.u8()
    if (!identity || waterMaterials.has(identity) || shader > 1 || opacity > 1 || textureCount > 6 || requiredCount > 64) throw new ArtifactError("water material header")
    const textures = Object.freeze(Array.from({ length: textureCount }, () => {
      const role = r.u8(), disposition = r.u8(), colorRead = r.u8()
      if (role > 18 || disposition > 2 || colorRead > 2 || r.u8()) throw new ArtifactError("water texture request")
      const parameter = r.text(), reference = r.text(), logicalPath = r.text()
      return Object.freeze({ role, disposition: (["source", "environment", "render-target"] as const)[disposition]!, colorRead: (["srgb", "linear", "format-dependent"] as const)[colorRead]!, parameter, reference, logicalPath: logicalPath || null })
    }))
    const bottomMaterialValue = r.text(), underwaterOverlayValue = r.text(), baseFrame = effectiveI32(), normalFrame = effectiveI32(), environmentFrame = effectiveI32(), normalParameter = r.text(), normalMatrix = new Float32Array(16)
    for (let index = 0; index < 16; index++) normalMatrix[index] = r.f32()
    const normalOrigin = originName(r.u8()), proxyMutated = r.u8(); if (proxyMutated > 1 || r.u8() || r.u8()) throw new ArtifactError("water normal transform")
    const scale = effectiveVec2(), time = effectiveF32(), waterDepth = effectiveF32(), aboveWater = effectiveBool(), reflectAmount = effectiveF32(), refractAmount = effectiveF32(), reflectTint = effectiveVec3(), refractTint = effectiveVec3(), reflectionBlendFactor = effectiveF32()
    const hasFogEnabled = r.u8(); if (hasFogEnabled > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("water fog enabled")
    const fogEnabledValue = effectiveBool(); if (hasFogEnabled === 0 && (fogEnabledValue.value || fogEnabledValue.origin !== "authored")) throw new ArtifactError("absent water fog enabled")
    const fogColor = effectiveVec3(), fogStart = effectiveF32(), fogEnd = effectiveF32(), cheapStart = effectiveF32(), cheapEnd = effectiveF32(), forceCheap = effectiveBool(), forceExpensive = effectiveBool(), reflectEntities = effectiveBool(), blurRefraction = effectiveBool(), noLowEndLightmap = effectiveBool(), scroll0 = effectiveVec3(), scroll1 = effectiveVec3()
    const cheapEnabled = r.u8(); if (cheapEnabled > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("water fresnel")
    const expensiveConstant = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [number, number, number, number]
    const requiredInputs = Object.freeze(Array.from({ length: requiredCount }, () => r.u8()))
    waterMaterials.set(identity, Object.freeze({
      identity, mapMaterial: mapValue === 0xffff_ffff ? null : mapValue, shader: shader === 0 ? "dx90" : "dx9-hdr", opacity: opacity === 0 ? "opaque" : "translucent", textures,
      bottomMaterial: bottomMaterialValue || null, underwaterOverlay: underwaterOverlayValue || null,
      baseFrame, normalFrame, environmentFrame, normalTransform: Object.freeze({ parameter: normalParameter, matrix: normalMatrix, origin: normalOrigin, proxyMutated: proxyMutated === 1 }), scale, time, waterDepth, aboveWater,
      reflectAmount, refractAmount, reflectTint, refractTint, reflectionBlendFactor,
      fog: Object.freeze({ enabled: hasFogEnabled === 1 ? fogEnabledValue : null, color: fogColor, start: fogStart, end: fogEnd }),
      cheapStart, cheapEnd, forceCheap, forceExpensive, reflectEntities, blurRefraction, noLowEndLightmap,
      scroll: Object.freeze([scroll0, scroll1]), fresnel: Object.freeze({ cheapEnabled: cheapEnabled === 1, expensiveConstant }), requiredInputs,
    }))
  }
  const refractMaterials = new Map<string, RefractMaterialArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase(), role = r.u8(), disposition = r.u8(), colorRead = r.u8()
    if (!identity || refractMaterials.has(identity) || role !== 8 || disposition !== 0 || colorRead !== 1 || r.u8()) {
      throw new ArtifactError("refract material normal")
    }
    const parameter = r.text(), reference = r.text(), logicalPath = r.text()
    const blurAmount = r.u8(), ignoreDepth = r.u8()
    if (!logicalPath || blurAmount > 1 || ignoreDepth > 1 || r.u8() || r.u8()) {
      throw new ArtifactError("refract material flags")
    }
    const refractAmount = r.f32(), refractTint = tuple3(r), normalFrame = r.i32()
    const normalTransform = new Float32Array(16)
    for (let index = 0; index < normalTransform.length; index++) normalTransform[index] = r.f32()
    refractMaterials.set(identity, Object.freeze({
      identity,
      normal: Object.freeze({ role, disposition: "source", colorRead: "linear", parameter, reference, logicalPath }),
      blurAmount: blurAmount as 0 | 1,
      ignoreDepth: ignoreDepth === 1,
      refractAmount,
      refractTint,
      normalFrame,
      normalTransform,
    }))
  }
  const worldMaterials = new Map<string, WorldMaterialArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase(), mapMaterial = r.u32()
    const shader = r.u8(), textureCount = r.u8(), proxyCount = r.u8(), hasEnvironment = r.u8()
    if (!identity || worldMaterials.has(identity) || (shader !== 1 && shader !== 4)
      || textureCount > 18 || proxyCount > 64 || hasEnvironment > 1) {
      throw new ArtifactError("world material header")
    }
    const textures = Object.freeze(Array.from({ length: textureCount }, () => {
      const role = r.u8(), disposition = r.u8(), colorRead = r.u8()
      if (role > 17 || disposition > 2 || colorRead > 2 || r.u8()) throw new ArtifactError("world texture request")
      const parameter = r.text(), reference = r.text(), logicalPath = r.text()
      const hasFrame = r.u8(), frameProxyMutated = r.u8(), hasTransform = r.u8(), transformProxyMutated = r.u8()
      if ([hasFrame, frameProxyMutated, hasTransform, transformProxyMutated].some((value) => value > 1)
        || (!hasFrame && frameProxyMutated) || (!hasTransform && transformProxyMutated)) {
        throw new ArtifactError("world texture animation flags")
      }
      const frame = r.i32(), matrix = new Float32Array(16)
      for (let index = 0; index < matrix.length; index++) matrix[index] = r.f32()
      if ((!hasFrame && frame !== 0) || (!hasTransform && matrix.some((value) => value !== 0))) {
        throw new ArtifactError("world texture absent animation state")
      }
      return Object.freeze({
        role,
        disposition: (["source", "environment", "render-target"] as const)[disposition]!,
        colorRead: (["srgb", "linear", "format-dependent"] as const)[colorRead]!,
        parameter,
        reference,
        logicalPath: logicalPath || null,
        initialFrame: hasFrame ? frame : null,
        frameProxyMutated: frameProxyMutated === 1,
        transform: hasTransform ? matrix : null,
        transformProxyMutated: transformProxyMutated === 1,
      })
    }))
    const proxies = Object.freeze(Array.from({ length: proxyCount }, () => {
      const name = r.text(), disposition = r.u8()
      if (!name || disposition > 2 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("world material proxy")
      return Object.freeze({ name, disposition: (["handled", "malformed", "unsupported"] as const)[disposition]! })
    }))
    const environmentMap = hasEnvironment
      ? Object.freeze({ tint: tuple3(r), contrast: r.f32(), saturation: r.f32() })
      : null
    const fresnelReflection = r.f32()
    if (Boolean(environmentMap) !== textures.some((texture) => texture.role === 12)) {
      throw new ArtifactError("world material environment binding")
    }
    worldMaterials.set(identity, Object.freeze({
      identity,
      mapMaterial,
      shader: shader === 1 ? "lightmapped-generic" : "world-vertex-transition",
      textures,
      proxies,
      environmentMap,
      fresnelReflection,
    }))
  }
  const authoredTextures = new Map<string, AuthoredTextureArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const texture = parseModelAuthoredTextureRecord(r, resources, sharedTextures)
    if (authoredTextures.has(texture.logicalPath)) throw new ArtifactError("environment authored texture identity")
    authoredTextures.set(texture.logicalPath, texture)
  }
  if (r.u32() !== waterSurfaceFacts.length) throw new ArtifactError("water surface extension count")
  const extendedWaterSurfaces = waterSurfaceFacts.map((surface) => {
    const face = r.u32(), textureInfo = r.u32(), fogValue = r.u32(), plane = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [number, number, number, number]
    const environment = r.u8(), reflection = r.u8(), refraction = r.u8(); if (environment > 1 || reflection > 1 || refraction > 1 || r.u8()) throw new ArtifactError("water surface bindings")
    if (face !== surface.face) throw new ArtifactError("water surface extension identity")
    return Object.freeze({ ...surface, textureInfo, fogVolume: fogValue === 0xffff_ffff ? null : fogValue, plane, bindings: Object.freeze({ environment: environment === 1, reflection: reflection === 1, refraction: refraction === 1 }) })
  })
  if (r.u32() !== waterVolumeFacts.length) throw new ArtifactError("water volume extension count")
  const extendedWaterVolumes = waterVolumeFacts.map((volume) => {
    const textureInfo = r.u32(), surfaceMaterial = r.u32(), bottomKind = r.u8(); if (bottomKind > 2 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("water bottom material")
    const bottomMaterial = bottomKind === 0 ? null : bottomKind === 1 ? Object.freeze({ kind: "map" as const, index: r.u32() }) : Object.freeze({ kind: "dependency" as const, identity: r.text() })
    const contents = r.u32(), plane = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [number, number, number, number]
    const clusters = Object.freeze(Array.from({ length: r.u32() }, () => { const value = r.u16(); return value >= 0x8000 ? value - 0x1_0000 : value })), areas = Object.freeze(Array.from({ length: r.u32() }, () => r.u32())), surfaceTranslucent = r.u8(), bottomTranslucent = r.u8()
    if (surfaceTranslucent > 1 || bottomTranslucent > 2 || r.u8() || r.u8()) throw new ArtifactError("water volume translucency")
    const surfaceEnvironment=r.u8(),surfaceReflection=r.u8(),surfaceRefraction=r.u8(),hasBottomBindings=r.u8(),bottomEnvironment=r.u8(),bottomReflection=r.u8(),bottomRefraction=r.u8();if([surfaceEnvironment,surfaceReflection,surfaceRefraction,hasBottomBindings,bottomEnvironment,bottomReflection,bottomRefraction].some(value=>value>1)||r.u8()||(!hasBottomBindings&&(bottomEnvironment||bottomReflection||bottomRefraction)))throw new ArtifactError("water volume bindings")
    return Object.freeze({ ...volume, textureInfo, surfaceMaterial, bottomMaterial, clusters, areas, contents, plane, surfaceTranslucent: surfaceTranslucent === 1, bottomTranslucent: bottomTranslucent === 0 ? null : bottomTranslucent === 2, surfaceBindings:Object.freeze({environment:surfaceEnvironment===1,reflection:surfaceReflection===1,refraction:surfaceRefraction===1}),bottomBindings:hasBottomBindings===1?Object.freeze({environment:bottomEnvironment===1,reflection:bottomReflection===1,refraction:bottomRefraction===1}):null })
  })
  const fogState = (): FogArtifact => {
    const enabled = r.u8(), blend = r.u8(), radial = r.u8(), hasFar = r.u8()
    if (enabled > 1 || blend > 1 || radial > 1 || hasFar > 1) throw new ArtifactError("fog flags")
    const direction = tuple3(r), primary = Object.freeze([r.u8(), r.u8(), r.u8(), r.u8()]) as readonly [number, number, number, number], secondary = Object.freeze([r.u8(), r.u8(), r.u8(), r.u8()]) as readonly [number, number, number, number], start = r.f32(), end = r.f32(), maximumDensity = r.f32(), far = r.f32(), transitionDuration = r.f32()
    if (hasFar === 0 && far !== 0) throw new ArtifactError("absent fog far Z")
    return Object.freeze({ enabled: enabled === 1, blend: blend === 1, radial: radial === 1, direction, primary, secondary, start, end, maximumDensity, farZ: hasFar === 1 ? far : null, transitionDuration })
  }
  if (r.u32() !== controllerFacts.length) throw new ArtifactError("environment controller extension count")
  const controllersState = Object.freeze(controllerFacts.map((controller) => {
    const entity = r.u32(), rawFields = Object.freeze(Array.from({ length: r.u32() }, () => Object.freeze({ key: r.text(), value: r.text() }))), kind = r.u8()
    if (entity !== controller.entity || kind !== controller.kind) throw new ArtifactError("environment controller extension identity")
    let state: EnvironmentControllerArtifact["state"]
    if (kind === 0) state = fogState()
    else if (kind === 1) state = Object.freeze({ origin: tuple3(r), scale: r.i32(), area: r.u32(), fog: fogState() })
    else if (kind === 2) state = Object.freeze({ start: r.f32(), end: r.f32() })
    else if (kind === 3) state = Object.freeze({ values: Object.freeze(Array.from({ length: 26 }, () => r.f32())) })
    else if (kind === 4) { const angles = tuple3(r), color = Object.freeze([r.u8(), r.u8(), r.u8(), r.u8()]), maximumDistance = r.f32(), disabled = r.u8(); if (disabled > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("shadow controller"); state = Object.freeze({ angles, color, maximumDistance, disabled: disabled === 1 }) }
    else if (kind === 5) state = Object.freeze({})
    else throw new ArtifactError("environment controller kind")
    return Object.freeze({ ...controller, rawFields, state })
  }))
  const masterFogValue = r.u32(), masterFogController = masterFogValue === 0xffff_ffff ? null : masterFogValue, skyFaceCount = r.u32()
  let extendedSky: EnvironmentArtifact["sky"] = null
  if (skyFaceCount !== (sky?.faces.length ?? 0)) throw new ArtifactError("sky texture extension count")
  if (sky) extendedSky = Object.freeze({ ...sky, faces: Object.freeze(sky.faces.map((face) => Object.freeze({ ...face, selectedTextures: Object.freeze(Array.from({ length: r.u32() }, () => Object.freeze({ logicalPath: r.text(), sha256: hex(r.take(32)) }))), textureTransform: Object.freeze(Array.from({ length: 16 }, () => r.f32())), color: tuple3(r) }))) })
  if (r.offset !== bytes.length) throw new ArtifactError("environment trailing bytes")
  return Object.freeze({
    profile: profile === 0 ? "ldr" : "hdr",
    identity,
    clusters: v[0]!,
    nodes: v[1]!,
    leaves: v[2]!,
    skySurfaces: v[3]!,
    cubemaps: v[4]!,
    waterSurfaces: v[5]!,
    waterVolumes: v[6]!,
    marks: v[7]!,
    markFragments: v[8]!,
    controllers: v[9]!,
    markRecords: Object.freeze(markRecords) as readonly EnvironmentMark[],
    textures: Object.freeze(textures),
    sky: extendedSky,
    cubemapFacts,
    waterSurfaceFacts: Object.freeze(extendedWaterSurfaces),
    waterVolumeFacts: Object.freeze(extendedWaterVolumes),
    controllerFacts,
    collisionWorldIdentity,
    receiverSnapshotRevision,
    placementRevision,
    leafMinimumDistanceToWater,
    waterMaterials,
    refractMaterials,
    worldMaterials,
    authoredTextures,
    controllersState,
    masterFogController,
  })
}

function magic(r: Reader, value: string): void {
  if (r.decode(r.take(4)) !== value || r.u32() !== 1) throw new ArtifactError(`${value} identity`)
}

function parseMaterialStates(r: Reader): ReadonlyMap<string, StaticMaterialState> {
  if (r.decode(r.take(4)) !== "PMST" || r.u32() !== 2) throw new ArtifactError("PMST identity")
  const states = new Map<string, StaticMaterialState>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase()
    if (states.has(identity)) throw new ArtifactError("material state identity")
    const values = Array.from({ length: 24 }, () => r.u8())
    const samplingAvailable = values[16] !== 0xff
    const booleans = [1, 4, 6, 7, 11, 12, 13, 14, 15, ...(samplingAvailable ? [21, 22, 23] : [])]
    if (booleans.some((index) => values[index]! > 1)) throw new ArtifactError("material state boolean")
    const alphaTestReference = r.f32(), alphaModulation = r.f32(), ownership = r.u16(), discard = r.u8(), source = r.u8(), pass = r.u8()
    if (ownership > 0x1ff || discard > 1 || source > 1 || pass > 1 || r.u8()) throw new ArtifactError("material alpha contract")
    const discardReference = r.f32()
    if ((discard === 0 && (source !== 0 || pass !== 0 || discardReference !== 0)) ||
      (discard === 1 && discardReference !== alphaTestReference)) throw new ArtifactError("material fragment discard")
    states.set(identity, Object.freeze({
      lighting: values[0]!,
      blendEnabled: values[1] === 1,
      blendSource: values[2]!,
      blendDestination: values[3]!,
      alphaTest: values[4] === 1,
      cull: values[5]!,
      depthTest: values[6] === 1,
      depthWrite: values[7] === 1,
      depthFunction: values[8]!,
      polygonOffset: values[9]!,
      fog: values[10]!,
      wireframe: values[11] === 1,
      noDraw: values[12] === 1,
      vertexColor: values[13] === 1,
      vertexAlpha: values[14] === 1,
      translucentQueue: values[15] === 1,
      wrapS: values[16]!,
      wrapT: values[17]!,
      wrapU: values[18]!,
      minFilter: values[19]!,
      magFilter: values[20]!,
      mipmapped: values[21] === 1,
      noLod: values[22] === 1,
      allMips: values[23] === 1,
      samplingAvailable,
      alphaTestReference,
      alphaModulation,
      alphaOwnership: Object.freeze({
        baseTextureAvailable: (ownership & 1) !== 0,
        opacity: (ownership & 2) !== 0,
        alphaTest: (ownership & 4) !== 0,
        selfIlluminationMask: (ownership & 8) !== 0,
        environmentMask: (ownership & 16) !== 0,
        phongMask: (ownership & 32) !== 0,
        tintMask: (ownership & 64) !== 0,
        vertexAlpha: (ownership & 128) !== 0,
        materialAlphaModulation: (ownership & 256) !== 0,
      }),
      fragmentDiscard: Object.freeze({
        kind: discard === 0 ? "none" : "alpha",
        source: source === 0 ? "base-texture-or-one" : "shader-output",
        pass: pass === 0 ? "greater" : "greater-or-equal",
        reference: discardReference,
      }),
    }))
  }
  return states
}

function parseParticleTextures(r: Reader, resources: ReadonlyMap<string, Uint8Array>, sharedTextures: Map<string, AuthoredTextureArtifact>): readonly ParticleTextureArtifact[] {
  if (r.decode(r.take(4)) !== "PPTM" || r.u32() !== 3) throw new ArtifactError("PPTM identity")
  const output: ParticleTextureArtifact[] = []
  const identities = new Set<string>()
  for (let count = r.u32(); count > 0; count--) {
    const material = r.text(), materialPath = r.text()
    const texture = parseModelAuthoredTextureRecord(r, resources, sharedTextures)
    if (identities.has(material.toLowerCase())) throw new ArtifactError("particle texture")
    identities.add(material.toLowerCase())
    const present = r.u32()
    if (present > 1) throw new ArtifactError("SpriteCard presence")
    let spriteCard: ParticleTextureArtifact["spriteCard"] = null
    if (present) {
      const depthBlend = r.u32(), blendFrames = r.u32()
      if (depthBlend > 1 || blendFrames > 1) throw new ArtifactError("SpriteCard flags")
      spriteCard = Object.freeze({ depthBlend: depthBlend === 1, blendFrames: blendFrames === 1,
        addSelf: r.f32(), overbright: r.f32(), depthBlendScale: r.f32(), minimumSize: r.f32(), startFadeSize: r.f32(), endFadeSize: r.f32(), maximumSize: r.f32(), maximumDistance: r.f32(), farFadeInterval: r.f32() })
    }
    output.push(Object.freeze({ ...texture, material, materialPath, spriteCard }))
  }
  return Object.freeze(output)
}

function soundNode(r: Reader): SoundScriptNode {
  const key = r.text(), kind = r.u8()
  if (kind === 0) return Object.freeze({ key, value: r.text() })
  if (kind !== 1) throw new ArtifactError("sound node kind")
  return Object.freeze({ key, value: Object.freeze(Array.from({ length: r.u32() }, () => soundNode(r))) })
}

function parseAudio(r: Reader): AudioArtifact {
  if (r.decode(r.take(4)) !== "PAUD" || r.u32() !== 4) throw new ArtifactError("PAUD identity")
  const mixerSha256 = hex(r.take(32)), mixerGain = r.f32(), count = r.u32()
  if (mixerGain < 0 || count < 1 || count > 5) throw new ArtifactError("audio mixer or document count")
  const documents = Array.from({ length: count }, () => Object.freeze({
    logicalPath: r.text(),
    sourceSha256: hex(r.take(32)),
    entries: Object.freeze(Array.from({ length: r.u32() }, () => soundNode(r))),
  }))
  if (new Set(documents.map((document) => document.logicalPath)).size !== documents.length) {
    throw new ArtifactError("audio documents repeat a logical identity")
  }
  const patches = new Map<string, Readonly<{ sampleRate: number; frames: number; loopStartSeconds: number | null }>>()
  const patchCount = r.u32()
  if (patchCount > 128) throw new ArtifactError("sound patch count")
  for (let index = 0; index < patchCount; index++) {
    const path = r.text(), sampleRate = r.u32(), frames = r.u32(), cue = r.u32()
    if (patches.has(path) || sampleRate === 0 || frames === 0 || (cue !== 0xffff_ffff && cue >= frames)) throw new ArtifactError("sound patch metadata")
    patches.set(path, Object.freeze({ sampleRate, frames, loopStartSeconds: cue === 0xffff_ffff ? null : cue / sampleRate }))
  }
  const unavailable = new Set<string>(), unavailableCount = r.u32()
  if (unavailableCount > 128) throw new ArtifactError("sound precache absence count")
  for (let index = 0; index < unavailableCount; index++) {
    const path = r.text()
    if (!path.startsWith("sound/") || path !== path.toLowerCase() || path.includes("\\") || path.split("/").some(part => !part || part === "." || part === "..") || unavailable.has(path) || patches.has(path)) {
      throw new ArtifactError("sound precache absence")
    }
    unavailable.add(path)
  }
  return Object.freeze({ mixerSha256, mixerGain, documents: Object.freeze(documents), patches, unavailable })
}

function parseOccurrenceMatrices(r: Reader): readonly ModelOccurrenceMatrix[] {
  if (r.decode(r.take(4)) !== "PMTX" || r.u32() !== 5) throw new ArtifactError("PMTX identity")
  const output = Array.from({ length: r.u32() }, () => {
    const entity = r.u32(), model = r.text(),skin=r.i32(),body=r.i32(),pipelineAnimation=r.text() || null,origin=tuple3(r),angles=tuple3(r), matrix = new Float32Array(12)
    for (let index = 0; index < matrix.length; index++) matrix[index] = r.f32()
    if(skin<0||body<0)throw new ArtifactError("model occurrence selection")
    const present = r.u8(), count = r.u8(), ambient = r.u8()
    if (present !== 1 || count > 4 || ambient !== 1 || r.u8()) throw new ArtifactError("model occurrence lighting")
    const lightingOrigin = tuple3(r), cameraPosition = tuple3(r)
    const ambientCube = Object.freeze(Array.from({ length: 6 }, () => tuple3(r))) as ModelLightingInput["ambientCube"]
    const localLights = Object.freeze(Array.from({ length: count }, (): ModelLocalLight => {
      const kind = r.u8()
      if (kind > 2 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model occurrence local light")
      const color = tuple3(r), position = tuple3(r), direction = tuple3(r)
      const range = r.f32(), falloff = r.f32(), attenuation = tuple3(r), theta = r.f32(), phi = r.f32()
      return Object.freeze({ kind: (["point", "directional", "spot"] as const)[kind]!, color, position,
        direction, range, falloff, attenuation, theta, phi })
    }))
    const environment = r.text()
    const lighting = Object.freeze({ lightingOrigin, cameraPosition, ambientCube, localLights,
      localEnvironment: environment || null, ambientLight: true, staticLightVertex: false,
      staticLightTexel: false }) satisfies ModelLightingInput
    const eyes = Object.freeze(Array.from({ length: r.u32() }, (): ModelEyeState => {
      const primitive = r.u32(), mesh = r.u32(), eyeball = r.u32(), texture = r.u32()
      const worldOrigin = tuple3(r), authoredUp = tuple3(r)
      const row = () => Object.freeze([r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [number, number, number, number]
      return Object.freeze({ primitive, mesh, eyeball, texture, worldOrigin, authoredUp,
        irisU: row(), irisV: row(), glintU: row(), glintV: row() })
    }))
    return Object.freeze({ entity, model,skin,body,pipelineAnimation,origin,angles, matrix, lighting, eyes })
  })
  return Object.freeze(output)
}

export function parseModelOccurrenceMatrices(bytes: Uint8Array): readonly ModelOccurrenceMatrix[] {
  const reader = new Reader(bytes)
  const occurrences = parseOccurrenceMatrices(reader)
  if (reader.offset !== bytes.byteLength) throw new ArtifactError("PMTX trailing bytes")
  return occurrences
}

function tuple2(r: Reader): readonly [number, number] {
  return Object.freeze([r.f32(), r.f32()])
}
function tuple3(r: Reader): readonly [number, number, number] {
  return Object.freeze([r.f32(), r.f32(), r.f32()])
}
function cloak(r: Reader): CloakState {
  const enabled = r.u8()
  if (enabled > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model cloak flags")
  return Object.freeze({ enabled: enabled === 1, factor: r.f32(), colorTint: tuple3(r), refractAmount: r.f32() })
}

function parseModelMaterials(r: Reader): ReadonlyMap<string, ModelMaterialArtifact> {
  if (r.decode(r.take(4)) !== "PMDL" || r.u32() !== 3) throw new ArtifactError("PMDL identity")
  const output = new Map<string, ModelMaterialArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase(), shaderCode = r.u8(), cloakProxy = r.u8()
    if (!identity || output.has(identity) || shaderCode > 5 || cloakProxy > 7) throw new ArtifactError("model material identity")
    const vertexRequirements = r.u16(), bindings: ModelTextureBinding[] = [], bindingIdentities = new Set<string>()
    for (let bindingCount = r.u32(); bindingCount > 0; bindingCount--) {
      const kind = r.u8(), role = r.u8(), colorRead = r.u8()
      if (kind > 1 || colorRead > 2 || r.u8()) throw new ArtifactError("model texture binding")
      const logicalPath = r.text().toLowerCase(), key = `${kind}:${role}`
      if (!logicalPath.startsWith("materials/") || bindingIdentities.has(key)) throw new ArtifactError("model texture binding identity")
      bindingIdentities.add(key)
      bindings.push(Object.freeze({
        kind: kind === 0 ? "material" : "model",
        role,
        colorRead: (["srgb", "linear", "format-dependent"] as const)[colorRead]!,
        logicalPath,
      }))
    }
    const hasEnvironment = r.u8()
    if (hasEnvironment > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model environment state")
    const environmentValues = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32(), r.f32()])
    if (hasEnvironment === 0 && environmentValues.some((value) => value !== 0)) throw new ArtifactError("absent model environment state")
    const environmentMap = hasEnvironment === 1
      ? Object.freeze({ tint: Object.freeze(environmentValues.slice(0, 3)) as readonly [number, number, number], contrast: environmentValues[3]!, saturation: environmentValues[4]! })
      : null
    let state: ModelMaterialArtifact["state"]
    if (shaderCode === 0) {
      const halfLambert = r.u8(), hasSelfIllumination = r.u8(), selfSource = r.u8(), hasPhong = r.u8()
      if (halfLambert > 1 || hasSelfIllumination > 1 || selfSource > 3 || hasPhong > 1) throw new ArtifactError("vertex-lit model state")
      const selfValues = Object.freeze(Array.from({ length: 6 }, () => r.f32()))
      if (hasSelfIllumination === 0 && (selfSource !== 0 || selfValues.some((value) => value !== 0))) throw new ArtifactError("absent self illumination")
      const maskSource = r.u8(), invertMask = r.u8(), albedoTint = r.u8(), hasRim = r.u8()
      if (maskSource > 2 || invertMask > 1 || albedoTint > 1 || hasRim > 1) throw new ArtifactError("model Phong flags")
      const phongValues = Object.freeze(Array.from({ length: 12 }, () => r.f32()))
      const rimExponent = r.f32(), rimBoost = r.f32(), rimMask = r.u8()
      if (rimMask > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model rim state")
      if (hasPhong === 0 && (maskSource !== 0 || invertMask !== 0 || albedoTint !== 0 || hasRim !== 0 || phongValues.some((value) => value !== 0) || rimExponent !== 0 || rimBoost !== 0 || rimMask !== 0)) {
        throw new ArtifactError("absent model Phong state")
      }
      if (hasRim === 0 && (rimExponent !== 0 || rimBoost !== 0 || rimMask !== 0)) throw new ArtifactError("absent model rim state")
      const cloakState = cloak(r), sheenEnabled = r.u8(), sheenSourceAlpha = r.u8(), sheenDepthWrite = r.u8()
      if (sheenEnabled > 1 || sheenSourceAlpha > 1 || sheenDepthWrite > 1 || r.u8()) throw new ArtifactError("model sheen flags")
      const maskFrame = r.i32(), maskDirection = r.i32(), shaderIndex = r.i32(), sheenTint = tuple3(r), maskScale = tuple2(r), maskOffset = tuple2(r)
      state = Object.freeze({
        kind: "vertex-lit-generic",
        halfLambert: halfLambert === 1,
        selfIllumination: hasSelfIllumination === 1 ? Object.freeze({
          source: selfSource,
          tint: Object.freeze(selfValues.slice(0, 3)) as readonly [number, number, number],
          fresnel: Object.freeze(selfValues.slice(3, 6)) as readonly [number, number, number],
        }) : null,
        phong: hasPhong === 1 ? Object.freeze({
          maskSource,
          invertMask: invertMask === 1,
          albedoTint: albedoTint === 1,
          exponent: phongValues[0]!, exponentFactor: phongValues[1]!,
          tint: Object.freeze(phongValues.slice(2, 5)) as readonly [number, number, number],
          boost: phongValues[5]!,
          fresnel: Object.freeze(phongValues.slice(6, 9)) as readonly [number, number, number],
          packedFresnel: Object.freeze(phongValues.slice(9, 12)) as readonly [number, number, number],
          rim: hasRim === 1 ? Object.freeze({ exponent: rimExponent, boost: rimBoost, exponentTextureAlphaMask: rimMask === 1 }) : null,
        }) : null,
        cloak: cloakState,
        sheen: Object.freeze({
          enabled: sheenEnabled === 1, sourceAlphaBlend: sheenSourceAlpha === 1, depthWrite: sheenDepthWrite === 1,
          maskFrame, maskDirection, shaderIndex, tint: sheenTint, maskScale, maskOffset,
        }),
      })
    } else if (shaderCode === 1) {
      const sphereTextureKill = r.u8(), raytraceSphere = r.u8(), halfLambert = r.u8()
      if (sphereTextureKill > 1 || raytraceSphere > 1 || halfLambert > 1 || r.u8()) throw new ArtifactError("eye-refract flags")
      const dilation = r.f32(), glossiness = r.f32(), parallaxStrength = r.f32(), corneaBumpStrength = r.f32(), ambientOcclusionColor = tuple3(r), eyeballRadius = r.f32()
      state = Object.freeze({
        kind: "eye-refract", sphereTextureKill: sphereTextureKill === 1, raytraceSphere: raytraceSphere === 1,
        halfLambert: halfLambert === 1, dilation, glossiness, parallaxStrength, corneaBumpStrength,
        ambientOcclusionColor, eyeballRadius, cloak: cloak(r),
      })
    } else if (shaderCode === 2) {
      const halfLambert = r.u8()
      if (halfLambert > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("eyes flags")
      state = Object.freeze({ kind: "eyes", halfLambert: halfLambert === 1, dilation: r.f32() })
    } else if (shaderCode === 4) {
      const frame = r.u8(), scroll = r.u8()
      if (frame > 1 || scroll > 1 || r.u8() || r.u8()) throw new ArtifactError("unlit two-texture proxy flags")
      const frameRate = r.f32(), scrollRate = r.f32(), scrollAngle = r.f32()
      if ((frame === 0 && frameRate !== 0) || (scroll === 0 && (scrollRate !== 0 || scrollAngle !== 0))) {
        throw new ArtifactError("unlit two-texture proxy values")
      }
      state = Object.freeze({ kind: "unlit-two-texture", secondFrameRate: frame ? frameRate : null,
        secondScrollRate: scroll ? scrollRate : null, secondScrollAngle: scroll ? scrollAngle : null })
    } else if (shaderCode === 3) {
      state = Object.freeze({ kind: "unlit-generic", colorModulation: tuple3(r) })
    } else {
      state = Object.freeze({ kind: "modulate" })
    }
    const opacity=r.u8(),framebuffer=r.u8(),requirementCount=r.u8();if(opacity>1||framebuffer>2||requirementCount>8||r.u8())throw new ArtifactError("model draw state");const names=["ambient-cube","local-lights","camera-position","studio-eye-parameters","local-environment","current-framebuffer","authored-texture-planes","game-proxy-values"] as const,requiredInputs=Object.freeze(Array.from({length:requirementCount},()=>{const code=r.u8();if(code<1||code>8)throw new ArtifactError("model draw requirement");return names[code-1]!}))
    output.set(identity, Object.freeze({
      identity,
      cloakProxy,
      shader: (["vertex-lit-generic", "eye-refract", "eyes", "unlit-generic", "unlit-two-texture", "modulate"] as const)[shaderCode]!,
      vertexRequirements,
      bindings: Object.freeze(bindings),
      environmentMap,
      opacity:opacity===0?"opaque":"translucent",framebuffer:(["none","potential","current"] as const)[framebuffer]!,requiredInputs,
      state,
    }))
  }
  return output
}

function parseModelAuthoredTextureRecord(
  r: Reader,
  resources: ReadonlyMap<string, Uint8Array>,
  sharedTextures: Map<string, AuthoredTextureArtifact>,
): AuthoredTextureArtifact {
  const logicalPath = r.text().toLowerCase(), sourceSha256 = hex(r.take(32)), width = r.u32(), height = r.u32(), depth = r.u32(),
    mipCount = r.u8(), scalarCode = r.u8(), frameCount = r.u16(), faces = Object.freeze(Array.from({ length: r.u32() }, () => r.u8()))
  if (!logicalPath.startsWith("materials/") || !width || !height || !depth || !mipCount || !frameCount || scalarCode > 1
    || faces.length < 1 || new Set(faces).size !== faces.length || faces.some((face) => face > 6)) throw new ArtifactError("model authored texture header")
  const wrapS = r.u8(), wrapT = r.u8(), wrapU = r.u8(), minFilter = r.u8(), magFilter = r.u8(), anisotropyLevel = r.u8(),
    mipmapped = r.u8(), noLod = r.u8(), allMips = r.u8()
  if (wrapS > 2 || wrapT > 2 || wrapU > 2 || minFilter > 4 || magFilter > 2 || !anisotropyLevel || mipmapped > 1 || noLod > 1 || allMips > 1 || r.u8() || r.u8() || r.u8()) {
    throw new ArtifactError("model authored texture sampling")
  }
  const expected: Array<readonly [number, number, number, number]> = []
  for (let mip = mipCount - 1; mip >= 0; mip--) {
    const slices = Math.max(1, depth >> mip)
    for (let frame = 0; frame < frameCount; frame++) for (const face of faces) for (let slice = 0; slice < slices; slice++) expected.push([mip, frame, face, slice])
  }
  const planeCount = r.u32()
  if (planeCount !== expected.length) throw new ArtifactError("model authored texture plane count")
  const source = resources.get(logicalPath)
  const planes: AuthoredTexturePlane[] = []
  let sourceFormat: number | null = null
  for (let index = 0; index < planeCount; index++) {
    const mip = r.u8(), face = r.u8(), frame = r.u16(), slice = r.u16()
    if (r.u16()) throw new ArtifactError("model authored texture plane reserved field")
    const planeWidth = r.u32(), planeHeight = r.u32(), storage = r.u8()
    if (r.u8() || r.u8() || r.u8()) throw new ArtifactError("model authored texture storage reserved field")
    const format = r.i32(), target = expected[index]!
    let rgba: Uint8Array
    if (storage === 0 && format === -1) {
      rgba = r.blob(256 * 1024 * 1024)
      const componentBytes = scalarCode === 0 ? 1 : 2
      if (rgba.length !== planeWidth * planeHeight * 4 * componentBytes) throw new ArtifactError("model authored decoded plane")
    } else if (storage === 1 && [0, 1, 2, 3, 11, 12, 13, 14, 15, 16, 20, 24].includes(format) && source) {
      const offset = r.u32(), length = r.u32(), expectedLength = [2, 3].includes(format)
        ? planeWidth * planeHeight * 3
        : [0, 1, 11, 12, 16].includes(format)
          ? planeWidth * planeHeight * 4
        : format === 24
          ? planeWidth * planeHeight * 8
          : Math.max(1, Math.ceil(planeWidth / 4)) * Math.max(1, Math.ceil(planeHeight / 4)) * ([13, 20].includes(format) ? 8 : 16)
      if (offset + length > source.length || length !== expectedLength || (sourceFormat !== null && sourceFormat !== format)
        || (format === 24) !== (scalarCode === 1)) throw new ArtifactError("model authored source plane")
      sourceFormat = format
      rgba = source.subarray(offset, offset + length)
    } else throw new ArtifactError("model authored texture storage")
    if (mip !== target[0] || frame !== target[1] || face !== target[2] || slice !== target[3]
      || planeWidth !== Math.max(1, width >> mip) || planeHeight !== Math.max(1, height >> mip)) throw new ArtifactError("model authored texture plane")
    planes.push(Object.freeze({ mip, frame, face, slice, width: planeWidth, height: planeHeight, rgba }))
  }
  const texture = Object.freeze({
    logicalPath, sourceSha256, width, height, depth, mipCount, frameCount, faces,
    scalarEncoding: scalarCode === 0 ? "u8" : "f16", sourceFormat,
    sampling: Object.freeze({ wrapS, wrapT, wrapU, minFilter, magFilter, anisotropyLevel, mipmapped: mipmapped === 1, noLod: noLod === 1, allMips: allMips === 1 }),
    planes: Object.freeze(planes),
  }) satisfies AuthoredTextureArtifact
  const sharedIdentity = `${logicalPath}:${sourceFormat ?? "decoded"}`
  const shared = sharedTextures.get(sharedIdentity)
  if (!shared) {
    sharedTextures.set(sharedIdentity, texture)
    return texture
  }
  if (shared.sourceSha256 !== sourceSha256 || shared.width !== width || shared.height !== height
    || shared.depth !== depth || shared.mipCount !== mipCount || shared.frameCount !== frameCount
    || shared.sourceFormat !== sourceFormat || shared.scalarEncoding !== texture.scalarEncoding
    || shared.planes.length !== planes.length || shared.faces.some((face, index) => face !== faces[index])
    || Object.entries(shared.sampling).some(([key, value]) => value !== texture.sampling[key as keyof typeof texture.sampling])) {
    throw new ArtifactError("shared authored texture identity")
  }
  return shared
}

function parseAuthoredTextures(
  r: Reader,
  resources: ReadonlyMap<string, Uint8Array>,
  sharedTextures: Map<string, AuthoredTextureArtifact>,
): ReadonlyMap<string, AuthoredTextureArtifact> {
  if (r.decode(r.take(4)) !== "PMIP" || r.u32() !== 2) throw new ArtifactError("PMIP identity")
  const output = new Map<string, AuthoredTextureArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const texture = parseModelAuthoredTextureRecord(r, resources, sharedTextures)
    if (output.has(texture.logicalPath)) throw new ArtifactError("authored texture identity")
    output.set(texture.logicalPath, texture)
  }
  return output
}

function parseStaticPropVhv(bytes:Uint8Array,expectedSha256:string):StaticPropArtifact["vhv"]{
  const r=new Reader(bytes)
  if(r.decode(r.take(4))!=="PVHA"||r.u32()!==2)throw new ArtifactError("static prop VHV identity")
  const count=r.u32();if(count>8192)throw new ArtifactError("static prop VHV count")
  const objects:StaticPropArtifact["vhv"][number][]=[];let previous=-1
  for(let index=0;index<count;index++){
    const occurrence=r.u32(),model=r.u32(),profile=r.u8();if(profile>1||!r.take(3).every(value=>value===0)||occurrence*2+profile<=previous)throw new ArtifactError("static prop VHV order");previous=occurrence*2+profile
    const meshCount=r.u32(),vertexCount=r.u32(),sha256=hex(r.take(32)),parsed=hex(r.take(32)),joinSha256=hex(r.take(32));if(meshCount>8192||sha256!==parsed)throw new ArtifactError("static prop VHV record")
    const records=[] as {primitive:number;lod:number;vertexCount:number;start:number;end:number}[]
    for(let mesh=0;mesh<meshCount;mesh++){const primitive=r.u32();r.u32();r.u32();const lod=r.u32();r.u32();r.u32();const vertices=r.u32(),start=r.u32(),end=r.u32();if(end<start)throw new ArtifactError("static prop VHV mesh");records.push({primitive,lod,vertexCount:vertices,start,end})}
    r.text();const source=r.blob(256*1024*1024);if(source.length<1)throw new ArtifactError("static prop VHV source")
    const meshes=Object.freeze(records.map(record=>{if(record.end>source.length||record.end-record.start!==record.vertexCount*4)throw new ArtifactError("static prop VHV colors");return Object.freeze({primitive:record.primitive,lod:record.lod,vertexCount:record.vertexCount,colors:source.subarray(record.start,record.end)})}))
    objects.push(Object.freeze({occurrence,model,profile:profile as 0|1,sha256,joinSha256,vertexCount,meshes}))
  }
  if(r.offset!==bytes.length||expectedSha256.length!==64)throw new ArtifactError("static prop VHV trailing bytes")
  return Object.freeze(objects)
}

function parseStaticProps(r: Reader, expectedModelCount: number,models:readonly string[],resources:ReadonlyMap<string,Uint8Array>): StaticPropArtifact {
  const start = r.offset
  if (r.decode(r.take(4)) !== "PSPA" || r.u32() !== 2) throw new ArtifactError("static prop identity")
  const aggregateSha256 = hex(r.take(32)), modelCount = r.u32(), count = r.u32()
  if (modelCount !== expectedModelCount || count > 65_536) throw new ArtifactError("static prop count")
  const source=new Uint32Array(count),dictionaryModel=new Uint32Array(count),presentationModel=new Uint32Array(count),transform=new Float32Array(count*6),skin=new Int32Array(count),body=new Uint32Array(count),lod=new Uint32Array(count),fades=new Float32Array(count*3),flags=new Uint32Array(count),solidity=new Uint8Array(count),ownership=new Uint8Array(count),lightingKind=new Uint8Array(count),lightingOrigin=new Float32Array(count*3),leafOffsets=new Uint32Array(count+1),vhvObjects=new Uint32Array(count*2),runtimeAmbient=new Float32Array(count*18),runtimeLightOffsets=new Uint32Array(count+1),leafValues:number[]=[],areaValues:number[]=[],runtimeLights:StaticPropArtifact["runtimeLights"][number][]=[]
  vhvObjects.fill(0xffff_ffff);lightingOrigin.fill(Number.NaN)
  let previous = -1, runtimeLightingCount = 0
  for (let index = 0; index < count; index++) {
    source[index]=r.u32();dictionaryModel[index]=r.u32();presentationModel[index]=r.u32();body[index]=r.u32();lod[index]=r.u32()
    for(let n=0;n<6;n++)transform[index*6+n]=r.f32()
    skin[index]=r.i32();for(let n=0;n<3;n++)fades[index*3+n]=r.f32();flags[index]=r.u32();solidity[index]=r.u8()
    const ownershipCode = r.u8(), hasLightingOrigin = r.u8(), lightingCode = r.u8()
    if (source[index]! <= previous || presentationModel[index]! >= modelCount || ownershipCode > 1 || hasLightingOrigin > 1 || lightingCode > 1)
      throw new ArtifactError("static prop record")
    previous = source[index]!;ownership[index]=ownershipCode;lightingKind[index]=lightingCode
    for(let n=0;n<3;n++){const value=r.f32();if(hasLightingOrigin)lightingOrigin[index*3+n]=value}
    const leafCount = r.u32()
    if (leafCount > 1_000_000) throw new ArtifactError("static prop leaves")
    leafOffsets[index]=leafValues.length;for(let leaf=0;leaf<leafCount;leaf++){leafValues.push(r.u16());areaValues.push(r.u16())}
    runtimeLightOffsets[index]=runtimeLights.length
    if (lightingCode === 0) {
      vhvObjects[index*2]=r.u32();vhvObjects[index*2+1]=r.u32()
    } else {
      runtimeLightingCount++
      r.take(32)
      for(let n=0;n<18;n++)runtimeAmbient[index*18+n]=r.f32()
      const lights = r.u32()
      if (lights > 4) throw new ArtifactError("static prop lights")
      for (let light = 0; light < lights; light++) {
        const lightSource=r.u32(),kind=r.i32(),style=r.u8()
        if (!r.take(3).every((value) => value === 0)) throw new ArtifactError("static prop light reserved")
        const ratio=r.f32(),direction=Object.freeze([r.f32(),r.f32(),r.f32()]) as readonly[number,number,number],intensity=Object.freeze([r.f32(),r.f32(),r.f32()]) as readonly[number,number,number]
        const origin=Object.freeze([r.f32(),r.f32(),r.f32()]) as readonly[number,number,number],normal=Object.freeze([r.f32(),r.f32(),r.f32()]) as readonly[number,number,number]
        const stopDot=r.f32(),stopDot2=r.f32(),exponent=r.f32(),radius=r.f32(),attenuation=Object.freeze([r.f32(),r.f32(),r.f32()]) as readonly[number,number,number]
        runtimeLights.push(Object.freeze({source:lightSource,kind,style,ratio,direction,intensity,origin,normal,stopDot,stopDot2,exponent,radius,attenuation}))
      }
    }
  }
  leafOffsets[count]=leafValues.length;runtimeLightOffsets[count]=runtimeLights.length
  const sectionLength = r.offset - start
  if (r.u32() !== sectionLength || r.decode(r.take(4)) !== "PSPF") throw new ArtifactError("static prop footer")
  const aggregate=resources.get("derived/static-prop-lighting.pvha")
  if(count===0&&aggregateSha256!=="0".repeat(64))throw new ArtifactError("empty static prop VHV identity")
  if(count!==0&&!aggregate)throw new ArtifactError("static prop VHV aggregate missing")
  const vhv=count===0?Object.freeze([]):parseStaticPropVhv(aggregate!,aggregateSha256)
  if(vhv.length!==count*2-runtimeLightingCount*2)throw new ArtifactError("static prop VHV closure")
  return Object.freeze({aggregateSha256,modelCount,count,source,dictionaryModel,presentationModel,transform,skin,body,lod,fades,flags,solidity,ownership,lightingKind,lightingOrigin,leafOffsets,leaves:Uint16Array.from(leafValues),areas:Uint16Array.from(areaValues),vhvObjects,runtimeAmbient,runtimeLightOffsets,runtimeLights:Object.freeze(runtimeLights),models:Object.freeze([...models]),vhv,runtimeLightingCount})
}
export type EquipmentModelArtifacts = Pick<PresentationArtifacts, "models" | "materialStates" | "modelMaterials" | "authoredTextures" | "particleMaterials" | "particleTextures"> & Readonly<{ geometry: readonly import("@playsrc/rendering/runtime-map").RuntimeModel[] }>

export function parseEquipmentModelArtifacts(bytes: Uint8Array, resources: ReadonlyMap<string, Uint8Array>): EquipmentModelArtifacts {
  const r = new Reader(bytes)
  if (r.decode(r.take(4)) !== "PEQM" || r.u32() !== 2) throw new ArtifactError("equipment model identity")
  const models = parseModelHeaders(r, r.u32())
  const materialStates = parseMaterialStates(r)
  const modelMaterials = parseModelMaterials(r)
  const sharedTextures = new Map<string, AuthoredTextureArtifact>()
  const authoredTextures = parseAuthoredTextures(r, resources, sharedTextures)
  const geometry = decodeRuntimeModelRegistry(r.blob(64 * 1024 * 1024))
  const count = r.u32()
  if (count > 4096) throw new ArtifactError("equipment particle material count")
  const particleMaterials = Object.freeze(Array.from({ length: count }, () => r.text()))
  const particleTextures = parseParticleTextures(r, resources, sharedTextures)
  for (const texture of particleTextures) {
    if (!materialStates.has(texture.material.toLowerCase())) throw new ArtifactError("equipment particle material state")
  }
  if (r.offset !== bytes.byteLength) throw new ArtifactError("equipment model trailing bytes")
  for (const material of modelMaterials.values()) if (material.bindings.some((binding) => !authoredTextures.has(binding.logicalPath))) throw new ArtifactError("equipment texture binding")
  return Object.freeze({ models, materialStates, modelMaterials, authoredTextures, geometry, particleMaterials, particleTextures })
}

function parseModelHeaders(r: Reader, modelCount: number): ReadonlyMap<string, ModelArtifact> {
  if (modelCount > 4096) throw new ArtifactError("model count")
  const models = new Map<string, ModelArtifact>()
  for (let i = 0; i < modelCount; i++) {
    const identity = r.text(),
      profile = r.u8()
    if ((profile !== 0 && profile !== 1) || r.u8() || r.u8() || r.u8() || models.has(identity))
      throw new ArtifactError("model header")
    const sha256 = hex(r.take(32)),
      skinCount = r.u32(),
      bodygroupCounts = Object.freeze(Array.from({ length: r.u32() }, () => r.u32())),
      attachments = new Map<string, Float32Array>()
    for (let n = r.u32(); n > 0; n--) {
      const name = r.text().toLowerCase(), matrix = new Float32Array(12)
      for (let index = 0; index < matrix.length; index++) matrix[index] = r.f32()
      if (attachments.has(name)) throw new ArtifactError("model attachment identity")
      attachments.set(name, matrix)
    }
    const sequences = [] as ModelArtifact["sequences"][number][]
    for (let n = r.u32(); n > 0; n--) {
      const label = r.text(), activity = r.text(), index = r.u32(), timingAvailable = r.u8()
      if (timingAvailable > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model sequence timing availability")
      const framesPerSecond = r.f32(),
        weightedFrameCount = r.f32(), cyclesPerSecond = r.f32(), durationSeconds = r.f32(), looping = r.u8()
      if (looping > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("model sequence timing")
      sequences.push(Object.freeze({ label, activity, index, timingAvailable: timingAvailable === 1, framesPerSecond, weightedFrameCount, cyclesPerSecond, durationSeconds, looping: looping === 1 }))
    }
    const descriptorKind = r.u8(), descriptorDetail = r.u8(), frontFace = r.u8(), cullFace = r.u8()
    if (descriptorDetail > 1 || frontFace > 1 || cullFace !== 0) throw new ArtifactError("model descriptor header")
    let descriptor: ModelArtifact["descriptor"]
    if (descriptorKind === 0 && profile === 0) {
      const root = descriptorDetail, depthRange = Object.freeze([r.f32(), r.f32()]) as readonly [number, number]
      if (!r.take(32).every((value) => value === 0)) throw new ArtifactError("world model descriptor reserved bytes")
      if (root > 1) throw new ArtifactError("world model descriptor")
      descriptor = Object.freeze({ kind: "world", staticPropRoot: root === 1, depthRange, frontFace: frontFace === 0 ? "clockwise" : "counter-clockwise", cullFace: "back" })
    } else if (descriptorKind === 1 && profile === 1) {
      if (descriptorDetail !== 0) throw new ArtifactError("viewmodel descriptor detail")
      const horizontalFov4By3 = r.f32(), minimumFov = r.f32(), maximumFov = r.f32(), near = r.f32()
      const depthRange = Object.freeze([r.f32(), r.f32()]) as readonly [number, number]
      const drawsAfterWorld = r.u8(), opaqueBeforeTranslucent = r.u8(), handedness = r.u8()
      if (drawsAfterWorld !== 1 || opaqueBeforeTranslucent !== 1 || handedness !== 0 || r.u8() || !r.take(12).every((value) => value === 0)) throw new ArtifactError("viewmodel descriptor")
      descriptor = Object.freeze({
        kind: "viewmodel", horizontalFov4By3, minimumFov, maximumFov, near, depthRange,
        drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true,
        frontFace: frontFace === 0 ? "clockwise" : "counter-clockwise", cullFace: "back",
      })
    } else throw new ArtifactError("model descriptor profile")
    const artifact = r.blob(0)
    models.set(
      identity,
      Object.freeze({
        identity,
        profile: profile === 0 ? "world" : "viewmodel",
        sha256,
        bytes: artifact,
        skinCount,
        bodygroupCounts,
        attachments,
        sequences: Object.freeze(sequences),
        descriptor,
      }),
    )
  }
  return models
}

export async function parsePresentationArtifacts(bytes: Uint8Array, resources: ReadonlyMap<string, Uint8Array>): Promise<PresentationArtifacts> {
  const r = new Reader(bytes)
  if (r.decode(r.take(4)) !== "PTF2" || r.u32() !== 14) throw new ArtifactError("artifact identity")
  const modelCount = r.u32(), directionalCount = r.u32(), particleMaterialCount = r.u32(), brushModelCount = r.u32()
  if (modelCount > 4096 || directionalCount > 4096 || particleMaterialCount > 65536 || brushModelCount < 1 || brushModelCount > 4096) throw new ArtifactError("artifact count")
  const models = parseModelHeaders(r, modelCount)
  const directionalRecords: Omit<DirectionalTextureArtifact, "authored">[] = []
  for (let i = 0; i < directionalCount; i++) {
    const material = r.text(),
      kindCode = r.u8()
    if ((kindCode !== 0 && kindCode !== 1) || r.u8() || r.u8() || r.u8()) throw new ArtifactError("directional header")
    const logicalPath = r.text(),
      width = r.u32(),
      height = r.u32(),
      sha256 = hex(r.take(32)),
      uvTransform = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [
        number,
        number,
        number,
        number,
        number,
        number,
      ]
    if (width < 1 || height < 1) throw new ArtifactError("directional texture identity")
    directionalRecords.push(
      Object.freeze({
        material,
        kind: kindCode === 0 ? "normal" : "ssbump",
        logicalPath,
        width,
        height,
        sha256,
        uvTransform,
      }),
    )
  }
  const particleMaterials = Object.freeze(Array.from({ length: particleMaterialCount }, () => r.text()))
  const sharedTextures = new Map<string, AuthoredTextureArtifact>()
  const environment = parseEnvironment(r.blob(512 * 1024 * 1024), resources, sharedTextures)
  const directionalTextures = directionalRecords.map((record): DirectionalTextureArtifact => {
    const authored = environment.authoredTextures.get(record.logicalPath.toLowerCase())
    if (!authored || authored.width !== record.width || authored.height !== record.height
      || authored.sourceSha256 !== record.sha256) {
      throw new ArtifactError("directional authored texture identity")
    }
    return Object.freeze({ ...record, authored })
  })
  const materialStates = parseMaterialStates(r)
  if (r.decode(r.bytes.subarray(r.offset, r.offset + 4)) !== "PPTM") {
    throw new ArtifactError(`material state boundary ${r.offset}:${hex(r.bytes.subarray(r.offset, r.offset + 16))}`)
  }
  const particleTextures = parseParticleTextures(r, resources, sharedTextures)
  for (const texture of particleTextures) {
    if (!materialStates.has(texture.material.toLowerCase())) {
      throw new ArtifactError(`particle material state ${texture.material}`)
    }
  }
  const audio = parseAudio(r)
  const modelOccurrences = parseOccurrenceMatrices(r)
  const modelMaterials = parseModelMaterials(r)
  const authoredTextures = parseAuthoredTextures(r, resources, sharedTextures)
  const brushModels:BrushModelArtifact[]=[];let previousEnd=0;for(let expected=0;expected<brushModelCount;expected++){const index=r.u32(),minimum=tuple3(r),maximum=tuple3(r),origin=tuple3(r),headNode=r.i32(),start=r.u32(),end=r.u32(),vertexCount=r.u32(),triangleCount=r.u32(),mc=r.u32(),ec=r.u32();if(mc>65536||ec>65536)throw new ArtifactError("brush counts");const materials=Object.freeze(Array.from({length:mc},()=>r.u32())),entities=Object.freeze(Array.from({length:ec},()=>r.u32()));if(index!==expected||start!==previousEnd||end<start)throw new ArtifactError("brush descriptor");previousEnd=end;brushModels.push(Object.freeze({index,bounds:Object.freeze([minimum,maximum]) as BrushModelArtifact["bounds"],origin,headNode,surfaceRange:Object.freeze([start,end]) as readonly[number,number],vertexCount,triangleCount,materials,entities}))}
  const staticProps = parseStaticProps(r, modelCount,[...models.keys()],resources)
  if (staticProps.count !== 0 && await digest(resources.get("derived/static-prop-lighting.pvha")!) !== staticProps.aggregateSha256) throw new ArtifactError("static prop VHV aggregate hash")
  if (r.offset !== bytes.length) throw new ArtifactError("trailing bytes")
  if (new Set(modelOccurrences.map((occurrence) => occurrence.entity)).size !== modelOccurrences.length)
    throw new ArtifactError("model occurrence identity")
  for (const material of modelMaterials.values()) {
    if (material.bindings.some((binding) => !authoredTextures.has(binding.logicalPath))) {
      throw new ArtifactError("model material authored texture closure")
    }
  }
  return Object.freeze({
    models,
    directionalTextures: Object.freeze(directionalTextures),
    particleMaterials,
    materialStates,
    particleTextures,
    audio,
    modelOccurrences,
    modelMaterials,
    authoredTextures,
    environment,
    brushModels:Object.freeze(brushModels),
    staticProps,
  })
}
