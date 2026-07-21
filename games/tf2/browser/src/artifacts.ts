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
export type ParticleTextureArtifact = SupplementalTexture & Readonly<{ materialPath: string }>
export type SoundScriptNode = Readonly<{ key: string; value: string | readonly SoundScriptNode[] }>
export type AudioArtifact = Readonly<{
  sourceSha256: string
  mixerSha256: string
  mixerGain: number
  logicalPath: string
  entries: readonly SoundScriptNode[]
}>
export type ModelOccurrenceMatrix = Readonly<{ entity: number; model: string; skin:number; body:number; origin:readonly[number,number,number]; angles:readonly[number,number,number]; matrix: Float32Array }>
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
  shader: "vertex-lit-generic" | "eye-refract" | "eyes"
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
  rgba: Uint8Array
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
export type FogArtifact = Readonly<{ enabled: boolean; blend: boolean; radial: boolean; direction: readonly [number, number, number]; primary: readonly [number, number, number, number]; secondary: readonly [number, number, number, number]; start: number; end: number; maximumDensity: number; farZ: number | null; transitionDuration: number }>
export type EnvironmentControllerArtifact = Readonly<{ entity: number; classname: string; kind: number; rawFields: readonly Readonly<{ key: string; value: string }>[]; state: unknown }>
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
  sky: Readonly<{ name: string; faces: readonly Readonly<{ face: number; material: string; sha256: string; encoding: "srgb" | "linear" | "hdr-rgbs"; selectedTextures: readonly Readonly<{ logicalPath: string; sha256: string }>[] }>[] }> | null
  cubemapFacts: readonly CubemapFact[]
  waterSurfaceFacts: readonly WaterSurfaceFact[]
  waterVolumeFacts: readonly WaterVolumeFact[]
  controllerFacts: readonly Readonly<{ entity: number; classname: string; kind: number }>[]
  collisionWorldIdentity: string
  receiverSnapshotRevision: bigint
  placementRevision: bigint
  leafMinimumDistanceToWater: Uint16Array
  waterMaterials: ReadonlyMap<string, WaterMaterialArtifact>
  authoredTextures: ReadonlyMap<string, AuthoredTextureArtifact>
  controllersState: readonly EnvironmentControllerArtifact[]
  masterFogController: number | null
}>
export type PresentationArtifacts = Readonly<{
  models: ReadonlyMap<string, ModelArtifact>
  textures: readonly SupplementalTexture[]
  particleMaterials: readonly string[]
  materialStates: ReadonlyMap<string, StaticMaterialState>
  particleTextures: readonly ParticleTextureArtifact[]
  audio: AudioArtifact
  modelOccurrences: readonly ModelOccurrenceMatrix[]
  modelMaterials: ReadonlyMap<string, ModelMaterialArtifact>
  authoredTextures: ReadonlyMap<string, AuthoredTextureArtifact>
  environment: EnvironmentArtifact
  brushModels:readonly BrushModelArtifact[]
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
    return this.take(1)[0]!
  }
  u16() {
    const v = this.view.getUint16(this.offset, true)
    this.take(2)
    return v
  }
  u32() {
    const v = this.view.getUint32(this.offset, true)
    this.take(4)
    return v
  }
  i32() {
    const v = this.view.getInt32(this.offset, true)
    this.take(4)
    return v
  }
  u64() {
    const v = this.view.getBigUint64(this.offset, true)
    this.take(8)
    return v
  }
  f32() {
    const v = this.view.getFloat32(this.offset, true)
    this.take(4)
    if (!Number.isFinite(v)) throw new ArtifactError("non-finite scalar")
    return v
  }
  blob(max = LIMIT) {
    const n = this.u32()
    if (n > max) throw new ArtifactError("field limit")
    return this.take(n)
  }
  text() {
    return this.decoder.decode(this.blob(4096))
  }
}
const hex = (bytes: Uint8Array) => Array.from(bytes, (v) => v.toString(16).padStart(2, "0")).join("")
const digest = async (bytes: Uint8Array) => hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)))
function parseEnvironment(bytes: Uint8Array, resources: ReadonlyMap<string, Uint8Array>): EnvironmentArtifact {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PENV" || r.u32() !== 4) throw new ArtifactError("environment identity")
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
  const authoredTextures = new Map<string, AuthoredTextureArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const texture = parseModelAuthoredTextureRecord(r, resources)
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
    let state: unknown
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
  let extendedSky = sky
  if (skyFaceCount !== (sky?.faces.length ?? 0)) throw new ArtifactError("sky texture extension count")
  if (sky) extendedSky = Object.freeze({ ...sky, faces: Object.freeze(sky.faces.map((face) => Object.freeze({ ...face, selectedTextures: Object.freeze(Array.from({ length: r.u32() }, () => Object.freeze({ logicalPath: r.text(), sha256: hex(r.take(32)) }))) }))) })
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
    authoredTextures,
    controllersState,
    masterFogController,
  })
}

function magic(r: Reader, value: string): void {
  if (r.decoder.decode(r.take(4)) !== value || r.u32() !== 1) throw new ArtifactError(`${value} identity`)
}

function parseMaterialStates(r: Reader): ReadonlyMap<string, StaticMaterialState> {
  if (r.decoder.decode(r.take(4)) !== "PMST" || r.u32() !== 2) throw new ArtifactError("PMST identity")
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

function parseParticleTextures(r: Reader): readonly ParticleTextureArtifact[] {
  magic(r, "PPTM")
  const output: ParticleTextureArtifact[] = []
  const identities = new Set<string>()
  for (let count = r.u32(); count > 0; count--) {
    const material = r.text(), materialPath = r.text(), logicalPath = r.text(), width = r.u32(), height = r.u32(),
      sha256 = hex(r.take(32)), rgba = r.blob(256 * 1024 * 1024)
    if (identities.has(material.toLowerCase()) || width * height * 4 !== rgba.length) throw new ArtifactError("particle texture")
    identities.add(material.toLowerCase())
    output.push(Object.freeze({ material, materialPath, logicalPath, width, height, sha256, rgba }))
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
  magic(r, "PAUD")
  const sourceSha256 = hex(r.take(32)), mixerSha256 = hex(r.take(32)), mixerGain = r.f32(), logicalPath = r.text()
  if (mixerGain < 0) throw new ArtifactError("audio mixer gain")
  return Object.freeze({
    sourceSha256,
    mixerSha256,
    mixerGain,
    logicalPath,
    entries: Object.freeze(Array.from({ length: r.u32() }, () => soundNode(r))),
  })
}

function parseOccurrenceMatrices(r: Reader): readonly ModelOccurrenceMatrix[] {
  if (r.decoder.decode(r.take(4)) !== "PMTX" || r.u32() !== 2) throw new ArtifactError("PMTX identity")
  const output = Array.from({ length: r.u32() }, () => {
    const entity = r.u32(), model = r.text(),skin=r.i32(),body=r.i32(),origin=tuple3(r),angles=tuple3(r), matrix = new Float32Array(12)
    for (let index = 0; index < matrix.length; index++) matrix[index] = r.f32()
    if(skin<0||body<0)throw new ArtifactError("model occurrence selection")
    return Object.freeze({ entity, model,skin,body,origin,angles, matrix })
  })
  return Object.freeze(output)
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
  magic(r, "PMDL")
  const output = new Map<string, ModelMaterialArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase(), shaderCode = r.u8()
    if (!identity || output.has(identity) || shaderCode > 2 || r.u8()) throw new ArtifactError("model material identity")
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
    } else {
      const halfLambert = r.u8()
      if (halfLambert > 1 || r.u8() || r.u8() || r.u8()) throw new ArtifactError("eyes flags")
      state = Object.freeze({ kind: "eyes", halfLambert: halfLambert === 1, dilation: r.f32() })
    }
    const opacity=r.u8(),framebuffer=r.u8(),requirementCount=r.u8();if(opacity>1||framebuffer>2||requirementCount>8||r.u8())throw new ArtifactError("model draw state");const names=["ambient-cube","local-lights","camera-position","studio-eye-parameters","local-environment","current-framebuffer","authored-texture-planes","game-proxy-values"] as const,requiredInputs=Object.freeze(Array.from({length:requirementCount},()=>{const code=r.u8();if(code<1||code>8)throw new ArtifactError("model draw requirement");return names[code-1]!}))
    output.set(identity, Object.freeze({
      identity,
      shader: (["vertex-lit-generic", "eye-refract", "eyes"] as const)[shaderCode]!,
      vertexRequirements,
      bindings: Object.freeze(bindings),
      environmentMap,
      opacity:opacity===0?"opaque":"translucent",framebuffer:(["none","potential","current"] as const)[framebuffer]!,requiredInputs,
      state,
    }))
  }
  return output
}

function parseAuthoredTextureRecord(r: Reader): AuthoredTextureArtifact {
  const logicalPath = r.text().toLowerCase(), sourceSha256 = hex(r.take(32)), width = r.u32(), height = r.u32(), depth = r.u32(),
      mipCount = r.u8(), scalarCode = r.u8(), frameCount = r.u16(), faces = Object.freeze(Array.from({ length: r.u32() }, () => r.u8()))
    if (!logicalPath.startsWith("materials/") || !width || !height || !depth || !mipCount || !frameCount ||
      scalarCode > 1 || faces.length < 1 || new Set(faces).size !== faces.length || faces.some((face) => face > 6)) throw new ArtifactError("authored texture header")
    const wrapS = r.u8(), wrapT = r.u8(), wrapU = r.u8(), minFilter = r.u8(), magFilter = r.u8(), anisotropyLevel = r.u8(),
      mipmapped = r.u8(), noLod = r.u8(), allMips = r.u8()
    if (wrapS > 2 || wrapT > 2 || wrapU > 2 || minFilter > 4 || magFilter > 2 || !anisotropyLevel || mipmapped > 1 || noLod > 1 || allMips > 1 || r.u8() || r.u8() || r.u8()) {
      throw new ArtifactError("authored texture sampling")
    }
    const expected: Array<readonly [number, number, number, number]> = []
    for (let mip = mipCount - 1; mip >= 0; mip--) {
      const slices = Math.max(1, depth >> mip)
      for (let frame = 0; frame < frameCount; frame++) for (const face of faces) for (let slice = 0; slice < slices; slice++) expected.push([mip, frame, face, slice])
    }
    const planeCount = r.u32()
    if (planeCount !== expected.length) throw new ArtifactError("authored texture plane count")
    const planes: AuthoredTexturePlane[] = []
    for (let index = 0; index < planeCount; index++) {
      const mip = r.u8(), face = r.u8(), frame = r.u16(), slice = r.u16()
      if (r.u16()) throw new ArtifactError("authored texture plane reserved field")
      const planeWidth = r.u32(), planeHeight = r.u32(), rgba = r.blob(256 * 1024 * 1024), target = expected[index]!
      const componentBytes = scalarCode === 0 ? 1 : 2
      if (mip !== target[0] || frame !== target[1] || face !== target[2] || slice !== target[3] ||
        planeWidth !== Math.max(1, width >> mip) || planeHeight !== Math.max(1, height >> mip) || rgba.length !== planeWidth * planeHeight * 4 * componentBytes) {
        throw new ArtifactError("authored texture plane")
      }
      planes.push(Object.freeze({ mip, frame, face, slice, width: planeWidth, height: planeHeight, rgba }))
    }
    return Object.freeze({
    logicalPath, sourceSha256, width, height, depth, mipCount, frameCount, faces,
    scalarEncoding: scalarCode === 0 ? "u8" : "f16",
    sourceFormat: null,
    sampling: Object.freeze({ wrapS, wrapT, wrapU, minFilter, magFilter, anisotropyLevel, mipmapped: mipmapped === 1, noLod: noLod === 1, allMips: allMips === 1 }),
    planes: Object.freeze(planes),
  })
}

function parseModelAuthoredTextureRecord(r: Reader, resources: ReadonlyMap<string, Uint8Array>): AuthoredTextureArtifact {
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
  return Object.freeze({
    logicalPath, sourceSha256, width, height, depth, mipCount, frameCount, faces,
    scalarEncoding: scalarCode === 0 ? "u8" : "f16", sourceFormat,
    sampling: Object.freeze({ wrapS, wrapT, wrapU, minFilter, magFilter, anisotropyLevel, mipmapped: mipmapped === 1, noLod: noLod === 1, allMips: allMips === 1 }),
    planes: Object.freeze(planes),
  })
}

function parseAuthoredTextures(r: Reader, resources: ReadonlyMap<string, Uint8Array>): ReadonlyMap<string, AuthoredTextureArtifact> {
  if (r.decoder.decode(r.take(4)) !== "PMIP" || r.u32() !== 2) throw new ArtifactError("PMIP identity")
  const output = new Map<string, AuthoredTextureArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const texture = parseModelAuthoredTextureRecord(r, resources)
    if (output.has(texture.logicalPath)) throw new ArtifactError("authored texture identity")
    output.set(texture.logicalPath, texture)
  }
  return output
}
export async function parsePresentationArtifacts(bytes: Uint8Array, resources: ReadonlyMap<string, Uint8Array>): Promise<PresentationArtifacts> {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PTF2" || r.u32() !== 10) throw new ArtifactError("artifact identity")
  const modelCount = r.u32(),
    textureCount = r.u32(),
    directionalCount = r.u32(),
    particleMaterialCount = r.u32(),brushModelCount=r.u32()
  if (modelCount > 256 || textureCount > 4096 || directionalCount > 4096 || particleMaterialCount > 65536||brushModelCount<1||brushModelCount>4096)
    throw new ArtifactError("artifact count")
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
  const textures: SupplementalTexture[] = []
  for (let i = 0; i < textureCount; i++) {
    const material = r.text(),
      logicalPath = r.text(),
      width = r.u32(),
      height = r.u32(),
      sha256 = hex(r.take(32)),
      rgba = r.blob(256 * 1024 * 1024)
    if (width * height * 4 !== rgba.length || (await digest(rgba)) !== sha256)
      throw new ArtifactError("texture identity")
    textures.push(Object.freeze({ material, logicalPath, width, height, sha256, rgba }))
  }
  const directionalTextures: DirectionalTextureArtifact[] = []
  for (let i = 0; i < directionalCount; i++) {
    const material = r.text(),
      kindCode = r.u8()
    if ((kindCode !== 0 && kindCode !== 1) || r.u8() || r.u8() || r.u8()) throw new ArtifactError("directional header")
    const logicalPath = r.text(),
      width = r.u32(),
      height = r.u32(),
      sha256 = hex(r.take(32)),
      rgba = r.blob(256 * 1024 * 1024),
      uvTransform = Object.freeze([r.f32(), r.f32(), r.f32(), r.f32(), r.f32(), r.f32()]) as readonly [
        number,
        number,
        number,
        number,
        number,
        number,
      ]
    if (width * height * 4 !== rgba.length || (await digest(rgba)) !== sha256)
      throw new ArtifactError("directional texture identity")
    directionalTextures.push(
      Object.freeze({
        material,
        kind: kindCode === 0 ? "normal" : "ssbump",
        logicalPath,
        width,
        height,
        sha256,
        rgba,
        uvTransform,
      }),
    )
  }
  const particleMaterials = Object.freeze(Array.from({ length: particleMaterialCount }, () => r.text()))
  const environment = parseEnvironment(r.blob(512 * 1024 * 1024), resources)
  const materialStates = parseMaterialStates(r)
  if (r.decoder.decode(r.bytes.subarray(r.offset, r.offset + 4)) !== "PPTM") {
    throw new ArtifactError(`material state boundary ${r.offset}:${hex(r.bytes.subarray(r.offset, r.offset + 16))}`)
  }
  const particleTextures = parseParticleTextures(r)
  const audio = parseAudio(r)
  const modelOccurrences = parseOccurrenceMatrices(r)
  const modelMaterials = parseModelMaterials(r)
  const authoredTextures = parseAuthoredTextures(r, resources)
  const brushModels:BrushModelArtifact[]=[];let previousEnd=0;for(let expected=0;expected<brushModelCount;expected++){const index=r.u32(),minimum=tuple3(r),maximum=tuple3(r),origin=tuple3(r),headNode=r.i32(),start=r.u32(),end=r.u32(),vertexCount=r.u32(),triangleCount=r.u32(),mc=r.u32(),ec=r.u32();if(mc>65536||ec>65536)throw new ArtifactError("brush counts");const materials=Object.freeze(Array.from({length:mc},()=>r.u32())),entities=Object.freeze(Array.from({length:ec},()=>r.u32()));if(index!==expected||start!==previousEnd||end<start)throw new ArtifactError("brush descriptor");previousEnd=end;brushModels.push(Object.freeze({index,bounds:Object.freeze([minimum,maximum]) as BrushModelArtifact["bounds"],origin,headNode,surfaceRange:Object.freeze([start,end]) as readonly[number,number],vertexCount,triangleCount,materials,entities}))}
  if (r.offset !== bytes.length) throw new ArtifactError("trailing bytes")
  if (new Set(modelOccurrences.map((occurrence) => occurrence.entity)).size !== modelOccurrences.length)
    throw new ArtifactError("model occurrence identity")
  for (const material of modelMaterials.values()) {
    if (material.bindings.some((binding) => !authoredTextures.has(binding.logicalPath))) {
      throw new ArtifactError("model material authored texture closure")
    }
  }
  await Promise.all(particleTextures.map(async (texture) => {
    if ((await digest(texture.rgba)) !== texture.sha256) throw new ArtifactError("particle texture identity")
  }))
  return Object.freeze({
    models,
    textures: Object.freeze(textures),
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
  })
}
