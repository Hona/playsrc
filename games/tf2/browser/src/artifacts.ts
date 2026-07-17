const LIMIT = 512 * 1024 * 1024
export type ModelArtifact = Readonly<{
  identity: string
  profile: "world" | "viewmodel"
  sha256: string
  bytes: Uint8Array
  skinCount: number
  bodygroupCounts: readonly number[]
  attachments: ReadonlySet<string>
  sequences: readonly Readonly<{ label: string; activity: string }>[]
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
export async function parsePresentationArtifacts(bytes: Uint8Array): Promise<PresentationArtifacts> {
  const r = new Reader(bytes)
  if (r.decoder.decode(r.take(4)) !== "PTF2" || r.u32() !== 4) throw new ArtifactError("artifact identity")
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
      attachments = new Set<string>()
    for (let n = r.u32(); n > 0; n--) attachments.add(r.text().toLowerCase())
    const sequences = [] as { label: string; activity: string }[]
    for (let n = r.u32(); n > 0; n--) sequences.push(Object.freeze({ label: r.text(), activity: r.text() }))
    const artifact = r.blob(64 * 1024 * 1024 - 1).slice(),
      q = new Reader(artifact)
    if (
      q.decoder.decode(q.take(4)) !== "PSMP" ||
      q.u16() !== 1 ||
      q.u8() !== profile ||
      q.u8() ||
      q.u64() !== BigInt(artifact.length)
    )
      throw new ArtifactError("model artifact header")
    q.take(4)
    if (q.text() !== identity || (await digest(artifact)) !== sha256) throw new ArtifactError("model artifact identity")
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
  if (r.offset !== bytes.length) throw new ArtifactError("trailing bytes")
  return Object.freeze({
    models,
    textures: Object.freeze(textures),
    directionalTextures: Object.freeze(directionalTextures),
    particleMaterials,
    environment,
  })
}
