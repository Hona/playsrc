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
    | Readonly<{ kind: "world"; staticPropRoot: boolean; depthRange: readonly [number, number] }>
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
export type ModelOccurrenceMatrix = Readonly<{ entity: number; model: string; matrix: Float32Array }>
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
}>
export type EnvironmentMark = Readonly<{
  status: number
  kind: number
  enabled: boolean
  dynamic: boolean
  material: string
  fragments: readonly EnvironmentFragment[]
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
}>
export type WaterVolumeFact = Readonly<{
  index: number
  surfaceZ: number
  minimumZ: number
  bounds: readonly [readonly [number, number, number], readonly [number, number, number]]
  leaves: readonly number[]
  cubemapKind: number
  cubemapSample: number | null
}>
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
  sky: Readonly<{ name: string; faces: readonly Readonly<{ face: number; material: string; sha256: string }>[] }> | null
  cubemapFacts: readonly CubemapFact[]
  waterSurfaceFacts: readonly WaterSurfaceFact[]
  waterVolumeFacts: readonly WaterVolumeFact[]
  controllerFacts: readonly Readonly<{ entity: number; classname: string; kind: number }>[]
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
function parseEnvironment(bytes: Uint8Array): EnvironmentArtifact {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PENV" || r.u32() !== 1) throw new ArtifactError("environment identity")
  const profile = r.u8()
  if ((profile !== 0 && profile !== 1) || r.u8() || r.u8() || r.u8()) throw new ArtifactError("environment profile")
  const identity = hex(r.take(32)),
    v = Array.from({ length: 10 }, () => r.u32()),
    markRecords: EnvironmentMark[] = []
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
      fragments.push(Object.freeze({ model, face, positions, normals, uv, indices }))
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
      rgba = r.blob(256 * 1024 * 1024).slice()
    if (width * height * 4 !== rgba.length) throw new ArtifactError("environment texture length")
    textures.push(Object.freeze({ material, logicalPath, width, height, sha256, rgba }))
  }
  const sky =
    r.u8() === 1
      ? Object.freeze({
          name: r.text(),
          faces: Object.freeze(
            Array.from({ length: r.u32() }, () =>
              Object.freeze({ face: r.u8(), material: r.text(), sha256: hex(r.take(32)) }),
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
    markRecords: Object.freeze(markRecords),
    textures: Object.freeze(textures),
    sky,
    cubemapFacts,
    waterSurfaceFacts,
    waterVolumeFacts,
    controllerFacts,
  })
}

function magic(r: Reader, value: string): void {
  if (r.decoder.decode(r.take(4)) !== value || r.u32() !== 1) throw new ArtifactError(`${value} identity`)
}

function parseMaterialStates(r: Reader): ReadonlyMap<string, StaticMaterialState> {
  magic(r, "PMST")
  const states = new Map<string, StaticMaterialState>()
  for (let count = r.u32(); count > 0; count--) {
    const identity = r.text().toLowerCase()
    if (states.has(identity)) throw new ArtifactError("material state identity")
    const values = Array.from({ length: 24 }, () => r.u8())
    const samplingAvailable = values[16] !== 0xff
    const booleans = [1, 4, 6, 7, 11, 12, 13, 14, 15, ...(samplingAvailable ? [21, 22, 23] : [])]
    if (booleans.some((index) => values[index]! > 1)) throw new ArtifactError("material state boolean")
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
      alphaTestReference: r.f32(),
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
      sha256 = hex(r.take(32)), rgba = r.blob(256 * 1024 * 1024).slice()
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
  magic(r, "PMTX")
  const output = Array.from({ length: r.u32() }, () => {
    const entity = r.u32(), model = r.text(), matrix = new Float32Array(12)
    for (let index = 0; index < matrix.length; index++) matrix[index] = r.f32()
    return Object.freeze({ entity, model, matrix })
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
    output.set(identity, Object.freeze({
      identity,
      shader: (["vertex-lit-generic", "eye-refract", "eyes"] as const)[shaderCode]!,
      vertexRequirements,
      bindings: Object.freeze(bindings),
      environmentMap,
      state,
    }))
  }
  return output
}

function parseAuthoredTextures(r: Reader): ReadonlyMap<string, AuthoredTextureArtifact> {
  magic(r, "PMIP")
  const output = new Map<string, AuthoredTextureArtifact>()
  for (let count = r.u32(); count > 0; count--) {
    const logicalPath = r.text().toLowerCase(), sourceSha256 = hex(r.take(32)), width = r.u32(), height = r.u32(), depth = r.u32(),
      mipCount = r.u8(), scalarCode = r.u8(), frameCount = r.u16(), faces = Object.freeze(Array.from({ length: r.u32() }, () => r.u8()))
    if (!logicalPath.startsWith("materials/") || output.has(logicalPath) || !width || !height || !depth || !mipCount || !frameCount ||
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
      const planeWidth = r.u32(), planeHeight = r.u32(), rgba = r.blob(256 * 1024 * 1024).slice(), target = expected[index]!
      const componentBytes = scalarCode === 0 ? 1 : 2
      if (mip !== target[0] || frame !== target[1] || face !== target[2] || slice !== target[3] ||
        planeWidth !== Math.max(1, width >> mip) || planeHeight !== Math.max(1, height >> mip) || rgba.length !== planeWidth * planeHeight * 4 * componentBytes) {
        throw new ArtifactError("authored texture plane")
      }
      planes.push(Object.freeze({ mip, frame, face, slice, width: planeWidth, height: planeHeight, rgba }))
    }
    output.set(logicalPath, Object.freeze({
      logicalPath, sourceSha256, width, height, depth, mipCount, frameCount, faces,
      scalarEncoding: scalarCode === 0 ? "u8" : "f16",
      sampling: Object.freeze({ wrapS, wrapT, wrapU, minFilter, magFilter, anisotropyLevel, mipmapped: mipmapped === 1, noLod: noLod === 1, allMips: allMips === 1 }),
      planes: Object.freeze(planes),
    }))
  }
  return output
}
export async function parsePresentationArtifacts(bytes: Uint8Array): Promise<PresentationArtifacts> {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PTF2" || r.u32() !== 6) throw new ArtifactError("artifact identity")
  const modelCount = r.u32(),
    textureCount = r.u32(),
    directionalCount = r.u32(),
    particleMaterialCount = r.u32()
  if (modelCount > 256 || textureCount > 4096 || directionalCount > 4096 || particleMaterialCount > 65536)
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
    const descriptorKind = r.u8(), descriptorDetail = r.u8()
    if (descriptorDetail > 1 || r.u8() || r.u8()) throw new ArtifactError("model descriptor header")
    let descriptor: ModelArtifact["descriptor"]
    if (descriptorKind === 0 && profile === 0) {
      const root = descriptorDetail, depthRange = Object.freeze([r.f32(), r.f32()]) as readonly [number, number]
      if (!r.take(32).every((value) => value === 0)) throw new ArtifactError("world model descriptor reserved bytes")
      if (root > 1) throw new ArtifactError("world model descriptor")
      descriptor = Object.freeze({ kind: "world", staticPropRoot: root === 1, depthRange })
    } else if (descriptorKind === 1 && profile === 1) {
      if (descriptorDetail !== 0) throw new ArtifactError("viewmodel descriptor detail")
      const horizontalFov4By3 = r.f32(), minimumFov = r.f32(), maximumFov = r.f32(), near = r.f32()
      const depthRange = Object.freeze([r.f32(), r.f32()]) as readonly [number, number]
      const drawsAfterWorld = r.u8(), opaqueBeforeTranslucent = r.u8(), handedness = r.u8()
      if (drawsAfterWorld !== 1 || opaqueBeforeTranslucent !== 1 || handedness !== 0 || r.u8() || !r.take(12).every((value) => value === 0)) throw new ArtifactError("viewmodel descriptor")
      descriptor = Object.freeze({
        kind: "viewmodel", horizontalFov4By3, minimumFov, maximumFov, near, depthRange,
        drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true,
      })
    } else throw new ArtifactError("model descriptor profile")
    const artifact = r.blob(0).slice()
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
      rgba = r.blob(256 * 1024 * 1024).slice()
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
      rgba = r.blob(256 * 1024 * 1024).slice(),
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
  const environment = parseEnvironment(r.blob(4 * 1024 * 1024))
  const materialStates = parseMaterialStates(r)
  const particleTextures = parseParticleTextures(r)
  const audio = parseAudio(r)
  const modelOccurrences = parseOccurrenceMatrices(r)
  const modelMaterials = parseModelMaterials(r)
  const authoredTextures = parseAuthoredTextures(r)
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
  })
}
