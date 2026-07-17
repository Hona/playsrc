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
export async function parsePresentationArtifacts(bytes: Uint8Array): Promise<PresentationArtifacts> {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PTF2" || r.u32() !== 5) throw new ArtifactError("artifact identity")
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
    const artifact = r.blob(64 * 1024 * 1024 - 1).slice(),
      q = new Reader(artifact)
    if (
      q.decoder.decode(q.take(4)) !== "PSMP" ||
      q.u16() !== 2 ||
      q.u8() !== profile ||
      q.u8() ||
      q.u64() !== BigInt(artifact.length)
    )
      throw new ArtifactError("model artifact header")
    q.take(8)
    const artifactIdentity = q.text()
    q.take(36)
    const descriptorKind = q.u8()
    for (let field = 0; field < 5; field++) if (q.u8() !== 0) throw new ArtifactError("model geometry descriptor")
    if (q.u8() !== 0) throw new ArtifactError("model angle descriptor")
    let descriptor: ModelArtifact["descriptor"]
    if (descriptorKind === 0 && profile === 0) {
      const root = q.u8(), depthRange = Object.freeze([q.f32(), q.f32()]) as readonly [number, number]
      if (root > 1) throw new ArtifactError("world model descriptor")
      descriptor = Object.freeze({ kind: "world", staticPropRoot: root === 1, depthRange })
    } else if (descriptorKind === 1 && profile === 1) {
      const horizontalFov4By3 = q.f32(), minimumFov = q.f32(), maximumFov = q.f32(), near = q.f32()
      if (q.u8() !== 0) throw new ArtifactError("viewmodel far descriptor")
      const depthRange = Object.freeze([q.f32(), q.f32()]) as readonly [number, number]
      const drawsAfterWorld = q.u8(), opaqueBeforeTranslucent = q.u8(), handedness = q.u8()
      if (drawsAfterWorld !== 1 || opaqueBeforeTranslucent !== 1 || handedness !== 0) throw new ArtifactError("viewmodel descriptor")
      descriptor = Object.freeze({
        kind: "viewmodel", horizontalFov4By3, minimumFov, maximumFov, near, depthRange,
        drawsAfterWorld: true, opaqueBeforeTranslucent: true, optionalViewSpaceYReflection: true,
      })
    } else throw new ArtifactError("model descriptor profile")
    if (artifactIdentity !== identity || (await digest(artifact)) !== sha256) throw new ArtifactError("model artifact identity")
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
  if (r.offset !== bytes.length) throw new ArtifactError("trailing bytes")
  if (new Set(modelOccurrences.map((occurrence) => occurrence.entity)).size !== modelOccurrences.length)
    throw new ArtifactError("model occurrence identity")
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
    environment,
  })
}
