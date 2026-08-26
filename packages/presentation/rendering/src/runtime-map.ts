const MAX_PAYLOAD_BYTES = 536_870_912
const MAX_MATERIALS = 65_536
const MAX_SURFACES = 1_000_000
const MAX_VERTICES = 16_777_216
const MAX_TRIANGLES = 16_777_216
const MAX_WORLD_LIGHTS = 1_000_000
const MAX_AMBIENT_SAMPLES = 4_000_000
const MAX_PROFILE_MATERIALS = 64
const MAX_INPUTS = 4_096
const MAX_PATH_BYTES = 1_024
const MAX_PROFILE_TEXTURE_BYTES = 64 * 1024 * 1024
const MAX_ATLAS_DIMENSION = 4_096
const LIGHTING_MEMBER_ROLES = 10
const WHITE = Object.freeze([1, 1, 1] as const)
const LITTLE_ENDIAN = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1

export type Rgb = readonly [number, number, number]

export type RuntimeMaterial = Readonly<{
  logicalPath: string
  width: number
  height: number
  shader: number
  features: number
  textureRole: number
  baseTexture?: Readonly<{
    logicalPath: string
    width: number
    height: number
  }>
  secondTexture?: Readonly<{ logicalPath: string; width: number; height: number }>
  detail?: Readonly<{
    texture: Readonly<{ logicalPath: string; width: number; height: number }>
    scale: readonly [number, number]
    blendMode: number
    blendFactor: number
    tint: Rgb
  }>
}>

export type RuntimeBatch = Readonly<{
  material: number
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  lightmapUv: Float32Array
  displacementAlpha: Float32Array
  lightmapKind: Float32Array
  indices: Uint32Array
  faces: Uint32Array
}>
export type RuntimeDisplacement = Readonly<{
  source: number
  face: number
  model: number
  material: number
  power: number
  positions: Float32Array
  normals: Float32Array
  lightmapUv: Float32Array
  indices: Uint32Array
  bounds: readonly [Rgb, Rgb]
  lightOffset: number
  styles: readonly [number, number, number, number]
  lightmapWidth: number
  lightmapHeight: number
}>
export type RuntimeBrushModel=Readonly<{index:number;batches:readonly RuntimeBatch[];drawableSurfaces:number}>

export type RuntimeModelPrimitive = Readonly<{
  material: number
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
}>

export type RuntimeModel = Readonly<{
  logicalPath: string
  materials: readonly RuntimeMaterial[]
  primitives: readonly RuntimeModelPrimitive[]
}>

export type RuntimeModelOccurrence = Readonly<{
  entity: number
  model: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>

export type LightingMemberSource =
  | Readonly<{ kind: "standard"; slot: number; version: number }>
  | Readonly<{ kind: "game"; id: string; version: number }>
  | Readonly<{ kind: "absent" }>

export type LightingMember = Readonly<{
  role: number
  source: LightingMemberSource
  encodedBytes: number
  decodedBytes: number
  encodedSha256: Uint8Array
  decodedSha256: Uint8Array
  itemCount: number
}>

export type SurfaceLightingKind = "unlit" | "flat" | "directional-normal" | "directional-ssbump"

export type SurfaceLighting = Readonly<{
  face: number
  kind: SurfaceLightingKind
  styleCount: number
  layerCount: 0 | 1 | 4
  sampleStart: number
  samplesPerLayer: number
  styles: readonly [number, number, number, number]
}>

export type RuntimeWorldLight = Readonly<{
  origin: Rgb
  intensity: Rgb
  normal: Rgb
  cluster: number
  kind: number
  style: number
  stopDot: number
  stopDot2: number
  exponent: number
  radius: number
  constantAttenuation: number
  linearAttenuation: number
  quadraticAttenuation: number
  flags: number
  textureInfo: number
  owner: number
}>

export type RuntimeAmbientIndex = Readonly<{ sampleCount: number; firstSample: number }>
export type RuntimeAmbientSample = Readonly<{
  cube: readonly [Rgb, Rgb, Rgb, Rgb, Rgb, Rgb]
  position: readonly [number, number, number]
}>

export type RuntimeProfileMaterial = Readonly<{
  logicalPath: string
  shader: number
  features: number
  textureRole: number
  texture: Readonly<{
    logicalPath: string
    width: number
    height: number
    format: number
    sourceSha256: Uint8Array
    sourceBytes: Uint8Array
  }>
}>

export type RuntimeInput = Readonly<{ role: number; logicalPath: string; sha256: Uint8Array }>

export type ProfileRequirement = Readonly<{
  family: "sky" | "water" | "environment"
  disposition: "Missing" | "Unsupported"
  identity: string
  reason: string
}>

export type HdrProfile = Readonly<{
  version: 1
  encoding: "linear-rgb-f32"
  outputRole: string
  compilerIdentity: string
  bspSha256: Uint8Array
  configurationSha256: Uint8Array
  lightingClosureSha256: Uint8Array
  members: readonly LightingMember[]
  lightmappedFaces: number
  directionalFaces: number
  surfaces: readonly SurfaceLighting[]
  worldLights: readonly RuntimeWorldLight[]
  ambientIndexes: readonly RuntimeAmbientIndex[]
  ambientSamples: readonly RuntimeAmbientSample[]
  propLighting: Readonly<{
    detailProps: number
    detailStyleSamples: number
    staticProps: number
    mapFlags: number
  }>
  profileMaterials: readonly RuntimeProfileMaterial[]
  consumedInputs: readonly RuntimeInput[]
  requirements: readonly ProfileRequirement[]
}>

type LightmapPlacement = Readonly<{
  face: number
  x: number
  y: number
  width: number
  height: number
}>

export type RuntimeLightmapLayout = Readonly<{
  width: number
  height: number
  gutter: number
  placements: ReadonlyMap<number, LightmapPlacement>
}>

export type RuntimeLightmap = Readonly<{
  width: number
  height: number
  profile: "ldr" | "hdr"
  flat: Float32Array
  directional?: readonly [Float32Array, Float32Array, Float32Array]
  styleScalars: ReadonlyMap<number, number>
}>

export type RuntimeLighting =
  | Readonly<{ profile: "ldr"; samples: Uint8Array }>
  | Readonly<{ profile: "hdr"; samples: Float32Array; descriptor: HdrProfile }>

export type RuntimeMap = Readonly<{
  schema: 6 | 7 | 8 | 9
  bspVersion: number
  mapRevision: number
  lightingProfile: 0 | 1
  materials: readonly RuntimeMaterial[]
  batches: readonly RuntimeBatch[]
  brushModels:readonly RuntimeBrushModel[]
  lightingSampleCount: number
  lighting: RuntimeLighting
  entityCount: number
  entityBytes: Uint8Array
  drawableSurfaces: number
  displacementSurfaces: number
  displacements: readonly RuntimeDisplacement[]
  models: readonly RuntimeModel[]
  modelOccurrences: readonly RuntimeModelOccurrence[]
  lightmapLayout: RuntimeLightmapLayout
  lightmap?: RuntimeLightmap
}>

export class RuntimeMapError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "RuntimeMapError"
  }
}

class Reader {
  readonly bytes: Uint8Array
  readonly view: DataView
  offset = 0

  constructor(bytes: Uint8Array) {
    this.bytes = bytes
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  }

  take(length: number): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0 || this.offset + length > this.bytes.byteLength) {
      throw new RuntimeMapError("runtime map record exceeds its bytes")
    }
    const result = this.bytes.subarray(this.offset, this.offset + length)
    this.offset += length
    return result
  }

  u8(): number {
    if (this.offset >= this.bytes.byteLength) throw new RuntimeMapError("runtime map record exceeds its bytes")
    return this.bytes[this.offset++]!
  }

  u16(): number {
    const offset = this.offset
    if (offset + 2 > this.bytes.byteLength) throw new RuntimeMapError("runtime map record exceeds its bytes")
    this.offset = offset + 2
    return this.view.getUint16(offset, true)
  }

  u32(): number {
    const offset = this.offset
    if (offset + 4 > this.bytes.byteLength) throw new RuntimeMapError("runtime map record exceeds its bytes")
    this.offset = offset + 4
    return this.view.getUint32(offset, true)
  }

  i32(): number {
    const offset = this.offset
    if (offset + 4 > this.bytes.byteLength) throw new RuntimeMapError("runtime map record exceeds its bytes")
    this.offset = offset + 4
    return this.view.getInt32(offset, true)
  }

  f32(): number {
    const offset = this.offset
    if (offset + 4 > this.bytes.byteLength) throw new RuntimeMapError("runtime map record exceeds its bytes")
    this.offset = offset + 4
    const value = this.view.getFloat32(offset, true)
    if (!Number.isFinite(value)) throw new RuntimeMapError("runtime map contains a non-finite scalar")
    return value
  }

  f32Array(length: number): Float32Array {
    const bytes = this.take(length * Float32Array.BYTES_PER_ELEMENT)
    let result: Float32Array
    if (LITTLE_ENDIAN && bytes.byteOffset % Float32Array.BYTES_PER_ELEMENT === 0) {
      result = new Float32Array(bytes.buffer, bytes.byteOffset, length)
    } else {
      result = new Float32Array(length)
      if (LITTLE_ENDIAN) new Uint8Array(result.buffer).set(bytes)
      else for (let index = 0; index < length; index += 1) result[index] = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getFloat32(index * 4, true)
    }
    for (let index = 0; index < result.length; index += 1) {
      if (!Number.isFinite(result[index]!)) throw new RuntimeMapError("runtime map contains a non-finite scalar")
    }
    return result
  }

  u32Array(length: number): Uint32Array {
    const bytes = this.take(length * Uint32Array.BYTES_PER_ELEMENT)
    if (LITTLE_ENDIAN && bytes.byteOffset % Uint32Array.BYTES_PER_ELEMENT === 0) {
      return new Uint32Array(bytes.buffer, bytes.byteOffset, length)
    }
    const result = new Uint32Array(length)
    if (LITTLE_ENDIAN) new Uint8Array(result.buffer).set(bytes)
    else for (let index = 0; index < length; index += 1) result[index] = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(index * 4, true)
    return result
  }

  sized(): Uint8Array {
    return this.take(this.u32())
  }
}

class TypedAccumulator<T extends Float32Array | Uint32Array> {
  #values: T
  #length = 0

  constructor(private readonly constructor_: { new (length: number): T }) {
    this.#values = new constructor_(0)
  }

  get length(): number { return this.#length }

  #reserve(count: number): number {
    const offset = this.#length
    const required = offset + count
    if (required > this.#values.length) {
      let capacity = Math.max(64, this.#values.length)
      while (capacity < required) capacity = Math.max(capacity + 1, Math.ceil(capacity * 1.5))
      const replacement = new this.constructor_(capacity)
      replacement.set(this.#values.subarray(0, this.#length))
      this.#values = replacement
    }
    this.#length = required
    return offset
  }

  append(values: ArrayLike<number>): void {
    const offset = this.#reserve(values.length)
    this.#values.set(values, offset)
  }

  appendOffset(values: Uint32Array, offset: number): void {
    const start = this.#reserve(values.length)
    for (let index = 0; index < values.length; index += 1) this.#values[start + index] = values[index]! + offset
  }

  fill(value: number, count: number): void {
    const offset = this.#reserve(count)
    this.#values.fill(value, offset, offset + count)
  }

  set(index: number, value: number): void {
    this.#values[index] = value
  }

  finish(): T {
    return this.#values.subarray(0, this.#length) as T
  }
}

type MutableBatch = {
  positions: TypedAccumulator<Float32Array>
  normals: TypedAccumulator<Float32Array>
  uv: TypedAccumulator<Float32Array>
  lightmapUv: TypedAccumulator<Float32Array>
  displacementAlpha: TypedAccumulator<Float32Array>
  vertexFaces: TypedAccumulator<Uint32Array>
  indices: TypedAccumulator<Uint32Array>
  faces: TypedAccumulator<Uint32Array>
}

function createMutableBatch(): MutableBatch {
  return {
    positions: new TypedAccumulator(Float32Array),
    normals: new TypedAccumulator(Float32Array),
    uv: new TypedAccumulator(Float32Array),
    lightmapUv: new TypedAccumulator(Float32Array),
    displacementAlpha: new TypedAccumulator(Float32Array),
    vertexFaces: new TypedAccumulator(Uint32Array),
    indices: new TypedAccumulator(Uint32Array),
    faces: new TypedAccumulator(Uint32Array),
  }
}

type CommonSurface = Readonly<{
  face: number
  lightOffset: number
  styles: readonly [number, number, number, number]
  lightmapWidth: number
  lightmapHeight: number
}>

type LightmapRecord = {
  surface: CommonSurface
  batch: MutableBatch
  uvStart: number
  uv: Float32Array
}

function bounded(value: number, maximum: number, field: string): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new RuntimeMapError(`${field} exceeds its limit`)
  }
  return value
}

function multiplyBounded(left: number, right: number, maximum: number, field: string): number {
  const value = left * right
  return bounded(value, maximum, field)
}

function utf8(reader: Reader, decoder: TextDecoder, field: string, maximum = MAX_PATH_BYTES): string {
  const bytes = reader.sized()
  if (bytes.byteLength < 1 || bytes.byteLength > maximum) {
    throw new RuntimeMapError(`${field} byte length is invalid`)
  }
  try {
    return decoder.decode(bytes)
  } catch {
    throw new RuntimeMapError(`${field} is not UTF-8`)
  }
}

function zeros(bytes: Uint8Array, field: string): void {
  if (bytes.some((value) => value !== 0)) throw new RuntimeMapError(`${field} reserved bytes are nonzero`)
}

function knownShader(shader: number): boolean {
  return (shader >= 1 && shader <= 10) || shader === 255
}

function knownTextureRole(role: number): boolean {
  return (role >= 0 && role <= 5) || role === 255
}

function resolvedMaterial(
  reader: Reader,
  decoder: TextDecoder,
  base: Pick<RuntimeMaterial, "logicalPath" | "width" | "height">,
  includeDetail: boolean,
): RuntimeMaterial {
  const shader = reader.u8()
  const features = reader.u8()
  const hasTexture = reader.u8()
  const textureRole = reader.u8()
  if (!knownShader(shader) || (features & ~0x3f) !== 0 || hasTexture > 1 || !knownTextureRole(textureRole)) {
    throw new RuntimeMapError("runtime material payload is invalid")
  }
  let baseTexture: RuntimeMaterial["baseTexture"]
  if (hasTexture === 1) {
    const logicalPath = utf8(reader, decoder, "runtime texture path")
    const width = reader.u32()
    const height = reader.u32()
    multiplyBounded(width, height, MAX_VERTICES, "runtime texture pixels")
    if (width < 1 || height < 1) {
      throw new RuntimeMapError("runtime texture identity is invalid")
    }
    baseTexture = Object.freeze({ logicalPath, width, height })
  }
  let detail: RuntimeMaterial["detail"]
  let secondTexture: RuntimeMaterial["secondTexture"]
  if (includeDetail) {
    const hasSecond = reader.u8()
    zeros(reader.take(3), "runtime second texture reserved")
    if (hasSecond > 1) throw new RuntimeMapError("runtime second texture disposition is invalid")
    if (hasSecond === 1) {
      const logicalPath = utf8(reader, decoder, "runtime second texture path")
      const width = reader.u32(), height = reader.u32()
      multiplyBounded(width, height, MAX_VERTICES, "runtime second texture pixels")
      if (width < 1 || height < 1) throw new RuntimeMapError("runtime second texture identity is invalid")
      secondTexture = Object.freeze({ logicalPath, width, height })
    }
    const hasDetail = reader.u8()
    zeros(reader.take(3), "runtime detail reserved")
    if (hasDetail > 1) throw new RuntimeMapError("runtime detail disposition is invalid")
    if (hasDetail === 1) {
      const logicalPath = utf8(reader, decoder, "runtime detail texture path")
      const width = reader.u32(), height = reader.u32()
      multiplyBounded(width, height, MAX_VERTICES, "runtime detail texture pixels")
      const scale = Object.freeze([reader.f32(), reader.f32()]) as readonly [number, number]
      const blendMode = reader.i32(), blendFactor = reader.f32(), tint = readRgb(reader)
      if (width < 1 || height < 1 || scale.some((value) => !Number.isFinite(value)) || blendMode < 0 || blendMode > 11 || !Number.isFinite(blendFactor) || tint.some((value) => !Number.isFinite(value))) {
        throw new RuntimeMapError("runtime detail payload is invalid")
      }
      detail = Object.freeze({ texture: Object.freeze({ logicalPath, width, height }), scale, blendMode, blendFactor, tint })
    }
  }
  return Object.freeze({ ...base, shader, features, textureRole, baseTexture, secondTexture, detail })
}

function readRgb(reader: Reader): Rgb {
  return Object.freeze([reader.f32(), reader.f32(), reader.f32()]) as Rgb
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.byteLength === right.byteLength && left.every((value, index) => value === right[index])
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  const length = Math.min(left.byteLength, right.byteLength)
  for (let index = 0; index < length; index += 1) {
    if (left[index] !== right[index]) return left[index]! - right[index]!
  }
  return left.byteLength - right.byteLength
}

function pushU16(target: number[], value: number): void {
  target.push(value & 0xff, value >>> 8 & 0xff)
}

function pushU32(target: number[], value: number): void {
  target.push(value & 0xff, value >>> 8 & 0xff, value >>> 16 & 0xff, value >>> 24 & 0xff)
}

function pushI32(target: number[], value: number): void {
  pushU32(target, value >>> 0)
}

async function digest(bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer))
}

async function validateLightingClosure(profile: HdrProfile): Promise<void> {
  const bytes: number[] = [...new TextEncoder().encode("playsrc-lighting-profile-v1"), 1]
  for (const member of profile.members) {
    bytes.push(member.role)
    if (member.source.kind === "standard") {
      bytes.push(1, member.source.slot)
      pushI32(bytes, member.source.version)
    } else if (member.source.kind === "game") {
      bytes.push(2, ...new TextEncoder().encode(member.source.id))
      pushU16(bytes, member.source.version)
    } else {
      bytes.push(0)
    }
    pushU32(bytes, member.encodedBytes)
    pushU32(bytes, member.decodedBytes)
    bytes.push(...member.encodedSha256, ...member.decodedSha256)
    pushU32(bytes, member.itemCount)
  }
  if (!equalBytes(await digest(new Uint8Array(bytes)), profile.lightingClosureSha256)) {
    throw new RuntimeMapError("HDR lighting member closure differs")
  }
}

function parseMember(reader: Reader, expectedRole: number): LightingMember {
  const role = reader.u8()
  if (role !== expectedRole) throw new RuntimeMapError("HDR lighting member roles are not ordered")
  const sourceKind = reader.u8()
  let source: LightingMemberSource
  if (sourceKind === 0) {
    zeros(reader.take(7), "absent lighting member")
    source = Object.freeze({ kind: "absent" })
  } else if (sourceKind === 1) {
    const slot = reader.u8()
    zeros(reader.take(2), "standard lighting member")
    const version = reader.i32()
    source = Object.freeze({ kind: "standard", slot, version })
  } else if (sourceKind === 2) {
    zeros(reader.take(3), "game lighting member")
    const idBytes = reader.take(4)
    if (idBytes.some((value) => value < 0x20 || value > 0x7e)) {
      throw new RuntimeMapError("game lighting member identity is invalid")
    }
    const id = new TextDecoder().decode(idBytes)
    const version = reader.u32()
    if (version > 0xffff) throw new RuntimeMapError("game lighting member version is invalid")
    source = Object.freeze({ kind: "game", id, version })
  } else {
    throw new RuntimeMapError("lighting member source kind is invalid")
  }
  const encodedBytes = reader.u32()
  const decodedBytes = reader.u32()
  const encodedSha256 = reader.take(32).slice()
  const decodedSha256 = reader.take(32).slice()
  const itemCount = reader.u32()
  if (source.kind === "absent") {
    if (
      encodedBytes !== 0 || decodedBytes !== 0 || itemCount !== 0
      || encodedSha256.some(Boolean) || decodedSha256.some(Boolean)
    ) {
      throw new RuntimeMapError("absent lighting member is nonzero")
    }
  } else if (encodedBytes === 0 || decodedBytes === 0 || encodedSha256.every((value) => value === 0) || decodedSha256.every((value) => value === 0)) {
    throw new RuntimeMapError("present lighting member is empty")
  }
  return Object.freeze({
    role,
    source,
    encodedBytes,
    decodedBytes,
    encodedSha256,
    decodedSha256,
    itemCount,
  })
}

function validateVtf(material: RuntimeProfileMaterial): void {
  const { sourceBytes, width, height, format } = material.texture
  if (sourceBytes.byteLength < 64 || sourceBytes.byteLength > MAX_PROFILE_TEXTURE_BYTES) {
    throw new RuntimeMapError("profile texture byte length is invalid")
  }
  const view = new DataView(sourceBytes.buffer, sourceBytes.byteOffset, sourceBytes.byteLength)
  if (!equalBytes(sourceBytes.subarray(0, 4), new Uint8Array([0x56, 0x54, 0x46, 0]))) {
    throw new RuntimeMapError("profile texture VTF identity is invalid")
  }
  const headerBytes = view.getUint32(12, true)
  if (
    headerBytes < 64 || headerBytes > sourceBytes.byteLength
    || view.getUint16(16, true) !== width
    || view.getUint16(18, true) !== height
    || view.getInt32(52, true) !== format
  ) {
    throw new RuntimeMapError("profile texture VTF metadata differs")
  }
}

function profileRequirements(materials: readonly RuntimeProfileMaterial[]): readonly ProfileRequirement[] {
  const result: ProfileRequirement[] = []
  for (const material of materials) {
    const family = material.shader === 8 ? "sky" : material.shader === 5 ? "water" : "environment"
    result.push(Object.freeze({
      family,
      disposition: "Unsupported" as const,
      identity: material.logicalPath,
      reason: `${family} decode, association, and draw inputs are not present in the runtime descriptor`,
    }))
  }
  for (const family of ["sky", "water", "environment"] as const) {
    if (!result.some((request) => request.family === family)) {
      result.push(Object.freeze({
        family,
        disposition: "Missing" as const,
        identity: `map-${family}-presentation`,
        reason: `the runtime descriptor supplies no ${family} presentation input`,
      }))
    }
  }
  return Object.freeze(result)
}

function parseHdrProfile(
  reader: Reader,
  decoder: TextDecoder,
  commonSurfaces: readonly CommonSurface[],
  lightingSampleCount: number,
): HdrProfile {
  if (decoder.decode(reader.take(4)) !== "PSHD") throw new RuntimeMapError("HDR profile identity is invalid")
  if (reader.u32() !== 1 || reader.u8() !== 1) throw new RuntimeMapError("HDR profile version or encoding is invalid")
  zeros(reader.take(3), "HDR profile")
  const outputRole = utf8(reader, decoder, "HDR output role")
  const compilerIdentity = utf8(reader, decoder, "HDR compiler identity")
  const bspSha256 = reader.take(32).slice()
  const configurationSha256 = reader.take(32).slice()
  const lightingClosureSha256 = reader.take(32).slice()
  if ([bspSha256, configurationSha256, lightingClosureSha256].some((hash) => hash.every((value) => value === 0))) {
    throw new RuntimeMapError("HDR profile identity hash is empty")
  }
  if (reader.u32() !== LIGHTING_MEMBER_ROLES) throw new RuntimeMapError("HDR lighting member count is invalid")
  const members = Object.freeze(Array.from(
    { length: LIGHTING_MEMBER_ROLES },
    (_, index) => parseMember(reader, index + 1),
  ))
  const lightmappedFaces = reader.u32()
  const directionalFaces = reader.u32()
  const surfaceCount = bounded(reader.u32(), MAX_SURFACES, "HDR surface count")
  if (surfaceCount !== commonSurfaces.length) throw new RuntimeMapError("HDR surface count differs")
  const surfaces: SurfaceLighting[] = []
  let measuredLightmapped = 0
  let measuredDirectional = 0
  for (let index = 0; index < surfaceCount; index += 1) {
    const face = reader.u32()
    const kindValue = reader.u8()
    const styleCount = reader.u8()
    const layerCount = reader.u8()
    if (reader.u8() !== 0) throw new RuntimeMapError("HDR surface reserved byte is nonzero")
    const sampleStart = reader.u32()
    const samplesPerLayer = reader.u32()
    const styles = Object.freeze([reader.u8(), reader.u8(), reader.u8(), reader.u8()]) as readonly [number, number, number, number]
    const common = commonSurfaces[index]!
    if (face !== common.face || styles.some((style, styleIndex) => style !== common.styles[styleIndex])) {
      throw new RuntimeMapError("HDR surface identity differs from the common surface")
    }
    const expectedStyleCount = styles.findIndex((style) => style === 255)
    const contiguousStyles = expectedStyleCount < 0 ? 4 : expectedStyleCount
    if (
      styleCount !== contiguousStyles
      || styles.slice(styleCount).some((style) => style !== 255)
      || styles.slice(0, styleCount).some((style) => style > 63)
    ) {
      throw new RuntimeMapError("HDR surface styles are invalid")
    }
    const kind = (["unlit", "flat", "directional-normal", "directional-ssbump"] as const)[kindValue]
    if (!kind) throw new RuntimeMapError("HDR surface lighting kind is invalid")
    const expectedLayers = kind === "unlit" ? 0 : kind === "flat" ? 1 : 4
    const expectedSamples = common.lightmapWidth * common.lightmapHeight
    if (
      layerCount !== expectedLayers
      || (kind === "unlit" && (styleCount !== 0 || sampleStart !== 0 || samplesPerLayer !== 0 || common.lightOffset >= 0))
      || (kind !== "unlit" && (
        styleCount < 1 || samplesPerLayer !== expectedSamples || common.lightOffset < 0
        || common.lightOffset % 4 !== 0 || sampleStart !== common.lightOffset / 4
      ))
    ) {
      throw new RuntimeMapError("HDR surface sample metadata is invalid")
    }
    const sampleEnd = sampleStart + styleCount * layerCount * samplesPerLayer
    if (!Number.isSafeInteger(sampleEnd) || sampleEnd > lightingSampleCount) {
      throw new RuntimeMapError("HDR surface sample range is invalid")
    }
    if (kind !== "unlit") measuredLightmapped += 1
    if (expectedLayers === 4) measuredDirectional += 1
    surfaces.push(Object.freeze({
      face,
      kind,
      styleCount,
      layerCount: layerCount as 0 | 1 | 4,
      sampleStart,
      samplesPerLayer,
      styles,
    }))
  }
  if (measuredLightmapped !== lightmappedFaces || measuredDirectional !== directionalFaces) {
    throw new RuntimeMapError("HDR surface classification counts differ")
  }

  const worldLightCount = bounded(reader.u32(), MAX_WORLD_LIGHTS, "world light count")
  const worldLights: RuntimeWorldLight[] = []
  for (let index = 0; index < worldLightCount; index += 1) {
    const origin = readRgb(reader)
    const intensity = readRgb(reader)
    const normal = readRgb(reader)
    const cluster = reader.i32()
    const kind = reader.i32()
    const style = reader.u8()
    zeros(reader.take(3), "world light")
    const stopDot = reader.f32()
    const stopDot2 = reader.f32()
    const exponent = reader.f32()
    const radius = reader.f32()
    const constantAttenuation = reader.f32()
    const linearAttenuation = reader.f32()
    const quadraticAttenuation = reader.f32()
    const flags = reader.i32()
    const textureInfo = reader.i32()
    const owner = reader.i32()
    if (kind < 0 || kind > 5 || style > 63 || radius < 0) {
      throw new RuntimeMapError(`world light record is invalid: kind=${kind} style=${style} radius=${radius} attenuation=${constantAttenuation},${linearAttenuation},${quadraticAttenuation}`)
    }
    worldLights.push(Object.freeze({
      origin,
      intensity,
      normal,
      cluster,
      kind,
      style,
      stopDot,
      stopDot2,
      exponent,
      radius,
      constantAttenuation,
      linearAttenuation,
      quadraticAttenuation,
      flags,
      textureInfo,
      owner,
    }))
  }

  const ambientIndexCount = bounded(reader.u32(), MAX_SURFACES, "ambient index count")
  const ambientIndexes = Object.freeze(Array.from({ length: ambientIndexCount }, (): RuntimeAmbientIndex => Object.freeze({
    sampleCount: reader.u16(),
    firstSample: reader.u16(),
  })))
  const ambientSampleCount = bounded(reader.u32(), MAX_AMBIENT_SAMPLES, "ambient sample count")
  const ambientSamples: RuntimeAmbientSample[] = []
  for (let index = 0; index < ambientSampleCount; index += 1) {
    const cube = Object.freeze(Array.from({ length: 6 }, () => readRgb(reader))) as unknown as readonly [Rgb, Rgb, Rgb, Rgb, Rgb, Rgb]
    const position = Object.freeze([reader.u8(), reader.u8(), reader.u8()]) as readonly [number, number, number]
    if (reader.u8() !== 0) throw new RuntimeMapError("ambient sample reserved byte is nonzero")
    ambientSamples.push(Object.freeze({ cube, position }))
  }
  for (const ambient of ambientIndexes) {
    if (ambient.firstSample + ambient.sampleCount > ambientSamples.length) {
      throw new RuntimeMapError("ambient sample range is invalid")
    }
  }

  const propLighting = Object.freeze({
    detailProps: reader.u32(),
    detailStyleSamples: reader.u32(),
    staticProps: reader.u32(),
    mapFlags: reader.u32(),
  })
  const profileMaterialCount = bounded(reader.u32(), MAX_PROFILE_MATERIALS, "profile material count")
  const profileMaterials: RuntimeProfileMaterial[] = []
  const materialIdentities = new Set<string>()
  for (let index = 0; index < profileMaterialCount; index += 1) {
    const logicalPath = utf8(reader, decoder, "profile material path")
    const shader = reader.u8()
    const features = reader.u8()
    const textureRole = reader.u8()
    if (reader.u8() !== 0) throw new RuntimeMapError("profile material reserved byte is nonzero")
    const texturePath = utf8(reader, decoder, "profile texture path")
    const width = reader.u32()
    const height = reader.u32()
    const format = reader.i32()
    const sourceSha256 = reader.take(32).slice()
    const sourceBytes = reader.sized().slice()
    const identity = logicalPath.toLowerCase()
    if (
      !knownShader(shader) || (features & ~0x3f) !== 0 || textureRole < 1 || textureRole > 5
      || width < 1 || height < 1 || width > MAX_ATLAS_DIMENSION || height > MAX_ATLAS_DIMENSION
      || !materialIdentities.add(identity)
    ) {
      throw new RuntimeMapError("profile material record is invalid")
    }
    const material = Object.freeze({
      logicalPath,
      shader,
      features,
      textureRole,
      texture: Object.freeze({ logicalPath: texturePath, width, height, format, sourceSha256, sourceBytes }),
    })
    validateVtf(material)
    profileMaterials.push(material)
  }

  const inputCount = bounded(reader.u32(), MAX_INPUTS, "consumed input count")
  const consumedInputs: RuntimeInput[] = []
  const inputIdentities = new Set<string>()
  let priorInput: { role: number; path: Uint8Array; sha256: Uint8Array } | undefined
  for (let index = 0; index < inputCount; index += 1) {
    const role = reader.u8()
    zeros(reader.take(3), "consumed input")
    const pathBytes = reader.sized()
    if (pathBytes.byteLength < 1 || pathBytes.byteLength > MAX_PATH_BYTES) {
      throw new RuntimeMapError("consumed input path byte length is invalid")
    }
    let logicalPath: string
    try {
      logicalPath = decoder.decode(pathBytes)
    } catch {
      throw new RuntimeMapError("consumed input path is not UTF-8")
    }
    const sha256 = reader.take(32).slice()
    if (sha256.every((value) => value === 0) || !inputIdentities.add(`${role}\0${logicalPath}`)) {
      throw new RuntimeMapError("consumed input identity is invalid")
    }
    if (priorInput) {
      const order = role - priorInput.role || compareBytes(pathBytes, priorInput.path) || compareBytes(sha256, priorInput.sha256)
      if (order < 0) throw new RuntimeMapError("consumed inputs are not sorted")
    }
    priorInput = { role, path: pathBytes.slice(), sha256 }
    consumedInputs.push(Object.freeze({ role, logicalPath, sha256 }))
  }

  const profile: HdrProfile = Object.freeze({
    version: 1,
    encoding: "linear-rgb-f32",
    outputRole,
    compilerIdentity,
    bspSha256,
    configurationSha256,
    lightingClosureSha256,
    members,
    lightmappedFaces,
    directionalFaces,
    surfaces: Object.freeze(surfaces),
    worldLights: Object.freeze(worldLights),
    ambientIndexes,
    ambientSamples: Object.freeze(ambientSamples),
    propLighting,
    profileMaterials: Object.freeze(profileMaterials),
    consumedInputs: Object.freeze(consumedInputs),
    requirements: profileRequirements(profileMaterials),
  })

  const expectedMemberCounts = [surfaceCount, lightingSampleCount, worldLightCount, ambientIndexCount, ambientSampleCount, 1]
  for (let index = 0; index < expectedMemberCounts.length; index += 1) {
    if (members[index]!.itemCount !== expectedMemberCounts[index]) {
      throw new RuntimeMapError("HDR lighting member item count differs")
    }
  }
  if (
    members[0]!.decodedBytes !== surfaceCount * 56
    || members[1]!.decodedBytes !== lightingSampleCount * 4
    || members[2]!.decodedBytes !== worldLightCount * 88
    || members[3]!.decodedBytes !== ambientIndexCount * 4
    || members[4]!.decodedBytes !== ambientSampleCount * 28
    || members[5]!.decodedBytes !== 4
    || members[7]!.itemCount !== propLighting.detailProps
    || members[8]!.itemCount !== propLighting.detailStyleSamples
    || members[9]!.itemCount !== propLighting.staticProps
  ) {
    throw new RuntimeMapError("HDR lighting member byte or prop counts differ")
  }
  return profile
}

export function packRuntimeLightmapLayout(
  surfaces: readonly Readonly<{ face: number; width: number; height: number }>[],
  gutter: number,
): RuntimeLightmapLayout {
  if (!Number.isSafeInteger(gutter) || gutter < 0 || gutter > 1) {
    throw new RuntimeMapError("lightmap gutter is invalid")
  }
  const ordered = surfaces.map((surface) => {
    const width = surface.width + gutter * 2
    const height = surface.height + gutter * 2
    if (!Number.isSafeInteger(surface.face) || surface.face < 0 || !Number.isSafeInteger(surface.width)
      || !Number.isSafeInteger(surface.height) || surface.width < 1 || surface.height < 1
      || width > MAX_ATLAS_DIMENSION || height > MAX_ATLAS_DIMENSION) {
      throw new RuntimeMapError("lightmap dimensions are invalid")
    }
    return Object.freeze({ ...surface, packedWidth: width, packedHeight: height })
  }).sort((left, right) => right.packedHeight - left.packedHeight
    || right.packedWidth - left.packedWidth || left.face - right.face)
  if (new Set(ordered.map((surface) => surface.face)).size !== ordered.length) {
    throw new RuntimeMapError("duplicate lightmap face identity")
  }
  const widest = Math.max(1, ...ordered.map((surface) => surface.packedWidth))
  let width = 1
  while (width < widest) width *= 2
  let selected: RuntimeLightmapLayout | undefined
  for (; width <= MAX_ATLAS_DIMENSION; width *= 2) {
    let x = gutter === 0 ? 1 : gutter
    let y = gutter
    let rowHeight = 1
    const placements = new Map<number, LightmapPlacement>()
    let fits = true
    for (const surface of ordered) {
      if (x + surface.packedWidth > width) {
        x = gutter
        y += rowHeight
        rowHeight = 0
      }
      if (x + surface.packedWidth > width) {
        fits = false
        break
      }
      placements.set(surface.face, Object.freeze({
        face: surface.face,
        x: x + gutter,
        y: y + gutter,
        width: surface.width,
        height: surface.height,
      }))
      x += surface.packedWidth
      rowHeight = Math.max(rowHeight, surface.packedHeight)
    }
    const height = y + rowHeight + gutter
    if (!fits || height > MAX_ATLAS_DIMENSION) continue
    if (!selected || width * height < selected.width * selected.height) {
      selected = Object.freeze({ width, height, gutter, placements })
    }
  }
  if (!selected) throw new RuntimeMapError("lightmap atlas exceeds its limit")
  return selected
}

function packLightmaps(records: readonly LightmapRecord[], gutter: number): RuntimeLightmapLayout {
  const layout = packRuntimeLightmapLayout(records.map((record) => Object.freeze({
    face: record.surface.face,
    width: record.surface.lightmapWidth,
    height: record.surface.lightmapHeight,
  })), gutter)
  for (const record of records) {
    const placement = layout.placements.get(record.surface.face)!
    for (let vertex = 0; vertex < record.uv.length / 2; vertex += 1) {
      record.batch.lightmapUv.set(record.uvStart + vertex * 2,
        (placement.x + record.uv[vertex * 2]! + 0.5) / layout.width)
      record.batch.lightmapUv.set(record.uvStart + vertex * 2 + 1,
        (placement.y + record.uv[vertex * 2 + 1]! + 0.5) / layout.height)
    }
  }
  return layout
}

function styleScalarMap(surfaces: readonly SurfaceLighting[], values?: readonly Readonly<{ style: number; scalar: number }>[]): ReadonlyMap<number, number> {
  const required = new Set<number>()
  for (const surface of surfaces) {
    for (let index = 0; index < surface.styleCount; index += 1) required.add(surface.styles[index]!)
  }
  if (!values) {
    if ([...required].some((style) => style !== 0)) {
      throw new RuntimeMapError("explicit light-style scalars are required")
    }
    return new Map([[0, 1]])
  }
  const result = new Map<number, number>()
  for (const value of values) {
    if (
      !Number.isSafeInteger(value.style) || value.style < 0 || value.style > 63
      || !Number.isFinite(value.scalar) || value.scalar < 0 || result.has(value.style)
    ) {
      throw new RuntimeMapError("light-style scalar input is invalid")
    }
    result.set(value.style, value.scalar)
  }
  if ([...required].some((style) => !result.has(style))) throw new RuntimeMapError("a required light-style scalar is missing")
  return result
}

function quantizeIntegerHdr(value: number): number {
  return Math.floor(Math.max(0, Math.min(65_535, value * 4_096))) / 4_096
}

function setPixel(plane: Float32Array, width: number, x: number, y: number, value: Rgb): void {
  const at = (y * width + x) * 4
  plane[at] = value[0]
  plane[at + 1] = value[1]
  plane[at + 2] = value[2]
  plane[at + 3] = 1
}

function copyGutters(plane: Float32Array, atlasWidth: number, placement: LightmapPlacement, gutter: number): void {
  if (gutter === 0) return
  const { x, y, width, height } = placement
  const pixel = (sourceX: number, sourceY: number, targetX: number, targetY: number) => {
    const source = (sourceY * atlasWidth + sourceX) * 4
    const target = (targetY * atlasWidth + targetX) * 4
    plane.copyWithin(target, source, source + 4)
  }
  for (let offset = 0; offset < width; offset += 1) {
    pixel(x + offset, y, x + offset, y - 1)
    pixel(x + offset, y + height - 1, x + offset, y + height)
  }
  for (let offset = -1; offset <= height; offset += 1) {
    pixel(x, Math.max(y, Math.min(y + height - 1, y + offset)), x - 1, y + offset)
    pixel(x + width - 1, Math.max(y, Math.min(y + height - 1, y + offset)), x + width, y + offset)
  }
}

function readHdrSample(samples: Float32Array, index: number): [number, number, number] {
  const at = index * 3
  return [samples[at]!, samples[at + 1]!, samples[at + 2]!]
}

function addScaled(target: [number, number, number], source: Rgb, scalar: number): void {
  target[0] += source[0] * scalar
  target[1] += source[1] * scalar
  target[2] += source[2] * scalar
}

function composeHdrTexel(
  samples: Float32Array,
  surface: SurfaceLighting,
  texel: number,
  scalars: ReadonlyMap<number, number>,
): readonly [Rgb, Rgb, Rgb, Rgb] {
  if (surface.kind === "unlit") return [WHITE, WHITE, WHITE, WHITE]
  const layers: [[number, number, number], [number, number, number], [number, number, number], [number, number, number]] = [
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
  ]
  for (let styleIndex = 0; styleIndex < surface.styleCount; styleIndex += 1) {
    const scalar = scalars.get(surface.styles[styleIndex]!)!
    for (let layer = 0; layer < surface.layerCount; layer += 1) {
      const sample = surface.sampleStart
        + styleIndex * surface.layerCount * surface.samplesPerLayer
        + layer * surface.samplesPerLayer
        + texel
      addScaled(layers[layer]!, readHdrSample(samples, sample), scalar)
    }
  }
  if (surface.layerCount === 1) {
    const flat = layers[0].map(quantizeIntegerHdr) as [number, number, number]
    return [flat, WHITE, WHITE, WHITE]
  }
  for (let channel = 0; channel < 3; channel += 1) {
    const average = (layers[1][channel] + layers[2][channel] + layers[3][channel]) / 3
    const correction = average === 0 ? 0 : layers[0][channel] / average
    for (let layer = 1; layer < 4; layer += 1) layers[layer][channel] *= correction
  }
  return layers.map((layer) => layer.map(quantizeIntegerHdr) as [number, number, number]) as unknown as readonly [Rgb, Rgb, Rgb, Rgb]
}

export function buildRuntimeLightmap(
  map: Pick<RuntimeMap, "lighting" | "lightmapLayout">,
  values?: readonly Readonly<{ style: number; scalar: number }>[],
): RuntimeLightmap {
  if (map.lighting.profile === "ldr") throw new RuntimeMapError("LDR lightmap replacement is not style-addressable")
  const { descriptor, samples } = map.lighting
  const styleScalars = styleScalarMap(descriptor.surfaces, values)
  const { width, height, placements, gutter } = map.lightmapLayout
  const flat = new Float32Array(width * height * 4)
  const directional = [
    new Float32Array(flat.length),
    new Float32Array(flat.length),
    new Float32Array(flat.length),
  ] as [Float32Array, Float32Array, Float32Array]
  for (const surface of descriptor.surfaces) {
    const placement = placements.get(surface.face)
    if (!placement) continue
    for (let texel = 0; texel < placement.width * placement.height; texel += 1) {
      const values = composeHdrTexel(samples, surface, texel, styleScalars)
      const x = placement.x + texel % placement.width
      const y = placement.y + Math.floor(texel / placement.width)
      setPixel(flat, width, x, y, values[0])
      for (let layer = 0; layer < 3; layer += 1) setPixel(directional[layer]!, width, x, y, values[layer + 1]!)
    }
    copyGutters(flat, width, placement, gutter)
    for (const plane of directional) copyGutters(plane, width, placement, gutter)
  }
  return Object.freeze({
    width,
    height,
    profile: "hdr",
    flat,
    directional: Object.freeze(directional),
    styleScalars,
  })
}

function buildLdrLightmap(
  lighting: Uint8Array,
  lightingSampleCount: number,
  layout: RuntimeLightmapLayout,
  records: readonly LightmapRecord[],
): RuntimeLightmap {
  const rgba = new Float32Array(layout.width * layout.height * 4)
  rgba.set([1, 1, 1, 1])
  for (const record of records) {
    const placement = layout.placements.get(record.surface.face)!
    const samples = placement.width * placement.height
    const source = record.surface.lightOffset >= 0 ? record.surface.lightOffset / 4 : -1
    if (source >= 0 && (!Number.isInteger(source) || source + samples > lightingSampleCount)) {
      throw new RuntimeMapError("lightmap sample range is invalid")
    }
    for (let sample = 0; sample < samples; sample += 1) {
      const target = ((placement.y + Math.floor(sample / placement.width)) * layout.width + placement.x + sample % placement.width) * 4
      if (source < 0) {
        rgba.set([1, 1, 1, 1], target)
      } else {
        const encoded = (source + sample) * 4
        const exponentByte = lighting[encoded + 3]!
        rgba[target] = sourceLdrEncodedChannel(lighting[encoded]!, exponentByte)
        rgba[target + 1] = sourceLdrEncodedChannel(lighting[encoded + 1]!, exponentByte)
        rgba[target + 2] = sourceLdrEncodedChannel(lighting[encoded + 2]!, exponentByte)
        rgba[target + 3] = 1
      }
    }
    copyGutters(rgba, layout.width, placement, layout.gutter)
  }
  return Object.freeze({
    width: layout.width,
    height: layout.height,
    profile: "ldr",
    flat: rgba,
    styleScalars: new Map([[0, 1]]),
  })
}

function sourceRound(value: number): number {
  const lower = Math.floor(value)
  const fraction = value - lower
  return fraction < 0.5 ? lower : fraction > 0.5 ? lower + 1 : lower % 2 === 0 ? lower : lower + 1
}

const ldrChannels = new Float32Array(65_536)
const ldrChannelsInitialized = new Uint8Array(65_536)
const SOURCE_LDR_OVERBRIGHT = 2 ** 2.2

function sourceLdrEffectiveChannel(linear: number): number {
  const index = Math.max(0, Math.min(4091, sourceRound(linear * 1024)))
  const gamma = Math.pow(index / 1024, 1 / 2.2) * 0.5
  const quantized = Math.max(0, Math.min(255, sourceRound(gamma * 255))) / 255
  const decoded = quantized <= 0.04045
    ? quantized / 12.92
    : ((quantized + 0.055) / 1.055) ** 2.4
  return decoded * SOURCE_LDR_OVERBRIGHT
}

function sourceLdrEncodedChannel(channel: number, exponentByte: number): number {
  const identity = exponentByte * 256 + channel
  if (ldrChannelsInitialized[identity]) return ldrChannels[identity]!
  const exponent = exponentByte > 127 ? exponentByte - 256 : exponentByte
  const value = sourceLdrEffectiveChannel(channel * 2 ** exponent / 255)
  ldrChannels[identity] = value
  ldrChannelsInitialized[identity] = 1
  return value
}

export function sourceLdrLightmapIrradiance(
  linear: readonly [number, number, number],
): readonly [number, number, number] {
  if (linear.some((value) => !Number.isFinite(value) || value < 0)) {
    throw new RuntimeMapError("LDR lightmap radiance is invalid")
  }
  return Object.freeze(linear.map(sourceLdrEffectiveChannel)) as unknown as readonly [number, number, number]
}

export function parseRuntimeMap(input: Uint8Array): RuntimeMap {
  if (input.byteLength < 37 || input.byteLength > MAX_PAYLOAD_BYTES) {
    throw new RuntimeMapError("runtime map byte length is invalid")
  }
  const reader = new Reader(input)
  const decoder = new TextDecoder("utf-8", { fatal: true })
  if (new TextDecoder().decode(reader.take(4)) !== "PSMP") throw new RuntimeMapError("runtime map identity is invalid")
  const schema = reader.u32()
  if (schema !== 6 && schema !== 7 && schema !== 8 && schema !== 9) throw new RuntimeMapError("runtime map schema is invalid")
  const bspVersion = reader.u32()
  const mapRevision = reader.u32()
  const lightingProfile = reader.u8()
  if (((schema === 6 || schema === 9) && lightingProfile !== 0) || ((schema === 7 || schema === 8) && lightingProfile !== 1)) {
    throw new RuntimeMapError("runtime map lighting profile differs from its schema")
  }
  const materialCount = bounded(reader.u32(), MAX_MATERIALS, "material count")
  const surfaceCount = bounded(reader.u32(), MAX_SURFACES, "surface count")
  const lightingSampleCount = bounded(reader.u32(), MAX_VERTICES, "lighting sample count")
  const entityCount = bounded(reader.u32(), MAX_SURFACES, "entity count")
  const materials: RuntimeMaterial[] = []
  for (let index = 0; index < materialCount; index += 1) {
    materials.push({
      logicalPath: utf8(reader, decoder, "runtime material path"),
      width: reader.i32(),
      height: reader.i32(),
      shader: 1,
      features: 0,
      textureRole: 0,
    })
    if (materials[index]!.width < 1 || materials[index]!.height < 1) {
      throw new RuntimeMapError("runtime map material record is invalid")
    }
  }

  const batches = new Map<number, MutableBatch>()
  const brushTables = new Map<number, Map<number, MutableBatch>>()
  const brushCounts = new Map<number, number>()
  const commonSurfaces: CommonSurface[] = []
  const lightmapRecords: LightmapRecord[] = []
  let totalVertices = 0
  let totalTriangles = 0
  let drawableSurfaces = 0
  let displacementSurfaces = 0
  const displacements: RuntimeDisplacement[] = []
  for (let index = 0; index < surfaceCount; index += 1) {
    const face = reader.u32()
    const model = reader.u32()
    const material = reader.u32()
    reader.i32()
    const draw = reader.u8()
    const vertexCount = bounded(reader.u32(), MAX_VERTICES, "surface vertex count")
    const triangleCount = bounded(reader.u32(), MAX_TRIANGLES, "surface triangle count")
    if (model>4095||material >= materialCount || draw > 1 || vertexCount < 3) throw new RuntimeMapError("runtime map surface record is invalid")
    totalVertices = bounded(totalVertices + vertexCount, MAX_VERTICES, "total vertex count")
    totalTriangles = bounded(totalTriangles + triangleCount, MAX_TRIANGLES, "total triangle count")
    const positions = reader.f32Array(vertexCount * 3)
    const normals = reader.f32Array(vertexCount * 3)
    const uv = reader.f32Array(vertexCount * 2)
    const lightmapUv = reader.f32Array(vertexCount * 2)
    const displacementAlpha = schema === 8 || schema === 9 ? reader.f32Array(vertexCount) : new Float32Array(vertexCount)
    const indices = reader.u32Array(triangleCount * 3)
    if (indices.some((value) => value >= vertexCount)) throw new RuntimeMapError("runtime map triangle index is invalid")
    const lightOffset = reader.i32()
    const styles = Object.freeze([reader.u8(), reader.u8(), reader.u8(), reader.u8()]) as readonly [number, number, number, number]
    const lightmapWidth = Math.max(1, reader.i32() + 1)
    const lightmapHeight = Math.max(1, reader.i32() + 1)
    if (schema === 8 || schema === 9) {
      const displacement = reader.u8(), power = reader.u8()
      zeros(reader.take(2), "runtime displacement reserved")
      if (displacement > 1 || (displacement === 0 && power !== 0)) throw new RuntimeMapError("runtime displacement disposition is invalid")
      if (displacement === 1) {
        if (power < 2 || power > 4) throw new RuntimeMapError("runtime displacement power is invalid")
        const source = reader.u32(); reader.i32(); reader.f32(); reader.u32(); const mapFace = reader.u16(); zeros(reader.take(2), "runtime displacement face reserved")
        if (mapFace !== face) throw new RuntimeMapError("runtime displacement parent face differs")
        reader.f32(); reader.f32(); reader.f32(); reader.u32(); reader.u32()
        for (let word = 0; word < 10; word += 1) reader.u32()
        reader.take(48 + 40)
        const tags = bounded(reader.u32(), MAX_TRIANGLES, "runtime displacement triangle tags")
        reader.take(tags * 2)
        const minimum = [Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY]
        const maximum = [Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY]
        for (let vertex = 0; vertex < vertexCount; vertex += 1) {
          for (let axis = 0; axis < 3; axis += 1) {
            const value = positions[vertex * 3 + axis]!
            minimum[axis] = Math.min(minimum[axis]!, value)
            maximum[axis] = Math.max(maximum[axis]!, value)
          }
        }
        displacements.push(Object.freeze({
          source, face, model, material, power, positions, normals, lightmapUv, indices,
          bounds: Object.freeze([Object.freeze(minimum) as Rgb, Object.freeze(maximum) as Rgb]),
          lightOffset, styles, lightmapWidth, lightmapHeight,
        }))
        displacementSurfaces += 1
      }
    }
    const common = Object.freeze({ face, lightOffset, styles, lightmapWidth, lightmapHeight })
    commonSurfaces.push(common)
    if(draw===0)continue
    let table = batches
    if (model !== 0) {
      table = brushTables.get(model) ?? new Map<number, MutableBatch>()
      brushTables.set(model, table)
    }
    const batch = table.get(material) ?? createMutableBatch()
    table.set(material, batch)
    const base = batch.positions.length / 3
    batch.positions.append(positions)
    batch.normals.append(normals)
    batch.uv.append(uv)
    const uvStart = batch.lightmapUv.length
    batch.lightmapUv.fill(0, lightmapUv.length)
    batch.displacementAlpha.append(displacementAlpha)
    batch.vertexFaces.fill(face, vertexCount)
    lightmapRecords.push({ surface: common, batch, uvStart, uv: lightmapUv })
    batch.indices.appendOffset(indices, base)
    batch.faces.fill(face, triangleCount)
    if(model===0)drawableSurfaces+=1;else brushCounts.set(model,(brushCounts.get(model)??0)+1)
  }

  let lighting: RuntimeLighting
  let ldrBytes: Uint8Array | undefined
  if (schema === 6 || schema === 9) {
    ldrBytes = reader.take(lightingSampleCount * 4).slice()
    lighting = Object.freeze({ profile: "ldr", samples: ldrBytes })
  } else {
    lighting = Object.freeze({ profile: "hdr", samples: reader.f32Array(lightingSampleCount * 3), descriptor: undefined as never })
  }
  const entityBytes = reader.sized().slice()
  const resolvedCount = reader.u32()
  if (resolvedCount !== materials.length) throw new RuntimeMapError("runtime material payload count is invalid")
  for (let index = 0; index < resolvedCount; index += 1) {
    materials[index] = resolvedMaterial(reader, decoder, materials[index]!, schema === 8 || schema === 9)
  }

  const models: RuntimeModel[] = []
  const modelCount = bounded(reader.u32(), 4_096, "runtime model count")
  for (let modelIndex = 0; modelIndex < modelCount; modelIndex += 1) {
    const logicalPath = utf8(reader, decoder, "runtime model path")
    const modelMaterialCount = bounded(reader.u32(), MAX_MATERIALS, "model material count")
    const modelMaterials: RuntimeMaterial[] = []
    for (let material = 0; material < modelMaterialCount; material += 1) {
      const materialPath = utf8(reader, decoder, "runtime model material path")
      modelMaterials.push(resolvedMaterial(reader, decoder, { logicalPath: materialPath, width: 1, height: 1 }, schema === 8 || schema === 9))
    }
    const primitiveCount = bounded(reader.u32(), 65_536, "model primitive count")
    const primitives: RuntimeModelPrimitive[] = []
    for (let primitive = 0; primitive < primitiveCount; primitive += 1) {
      const material = reader.u32()
      const vertices = bounded(reader.u32(), MAX_VERTICES, "model vertex count")
      const triangles = bounded(reader.u32(), MAX_TRIANGLES, "model triangle count")
      if (material >= modelMaterialCount) throw new RuntimeMapError("model material index is invalid")
      const positions = reader.f32Array(vertices * 3)
      const normals = reader.f32Array(vertices * 3)
      const uv = reader.f32Array(vertices * 2)
      const indices = reader.u32Array(triangles * 3)
      if (indices.some((value) => value >= vertices)) throw new RuntimeMapError("model triangle index is invalid")
      primitives.push(Object.freeze({ material, positions, normals, uv, indices }))
    }
    models.push(Object.freeze({ logicalPath, materials: Object.freeze(modelMaterials), primitives: Object.freeze(primitives) }))
  }
  const modelOccurrences: RuntimeModelOccurrence[] = []
  const occurrenceCount = bounded(reader.u32(), MAX_SURFACES, "model occurrence count")
  for (let index = 0; index < occurrenceCount; index += 1) {
    const entity = reader.u32()
    const model = reader.u32()
    if (model >= models.length) throw new RuntimeMapError("model occurrence index is invalid")
    const position = Object.freeze([reader.f32(), reader.f32(), reader.f32()]) as readonly [number, number, number]
    const angles = Object.freeze([reader.f32(), reader.f32(), reader.f32()]) as readonly [number, number, number]
    modelOccurrences.push(Object.freeze({ entity, model, position, angles }))
  }

  if (schema === 7 || schema === 8) {
    const descriptor = parseHdrProfile(reader, decoder, commonSurfaces, lightingSampleCount)
    lighting = Object.freeze({ profile: "hdr", samples: lighting.samples as Float32Array, descriptor })
  }
  if (reader.offset !== input.byteLength) throw new RuntimeMapError("runtime map has trailing bytes")

  const lightmapLayout = packLightmaps(lightmapRecords, schema === 7 || schema === 8 || schema === 9 ? 1 : 0)
  const lightingKinds = lighting.profile === "hdr"
    ? new Map(lighting.descriptor.surfaces.map((surface) => [surface.face, surface.kind === "unlit" ? 0 : surface.kind === "flat" ? 1 : surface.kind === "directional-normal" ? 2 : 3]))
    : new Map(commonSurfaces.map((surface) => [surface.face, surface.lightOffset < 0 ? 0 : 1]))
  const freeze = (table: ReadonlyMap<number, MutableBatch>): RuntimeBatch[] => [...table.entries()]
    .sort(([left], [right]) => left - right)
    .map(([material, batch]) => Object.freeze({
      material,
      positions: batch.positions.finish(),
      normals: batch.normals.finish(),
      uv: batch.uv.finish(),
      lightmapUv: batch.lightmapUv.finish(),
      displacementAlpha: batch.displacementAlpha.finish(),
      lightmapKind: Float32Array.from(batch.vertexFaces.finish(), (face) => lightingKinds.get(face) ?? 0),
      indices: batch.indices.finish(),
      faces: batch.faces.finish(),
    }))
  const frozenBatches = freeze(batches)
  const brushModels = Object.freeze([...brushTables].sort(([left], [right]) => left - right)
    .map(([index, table]) => Object.freeze({
      index,
      batches: Object.freeze(freeze(table)),
      drawableSurfaces: brushCounts.get(index) ?? 0,
    })))
  const staticHdrStyles = lighting.profile === "hdr"
    && lighting.descriptor.surfaces.every((surface) => surface.styles.slice(0, surface.styleCount).every((style) => style === 0))
  const lightmap = lighting.profile === "ldr"
    ? buildLdrLightmap(ldrBytes!, lightingSampleCount, lightmapLayout, lightmapRecords)
    : staticHdrStyles ? buildRuntimeLightmap({ lighting, lightmapLayout }) : undefined
  return Object.freeze({
    schema,
    bspVersion,
    mapRevision,
    lightingProfile: lightingProfile as 0 | 1,
    materials: Object.freeze(materials.map((material) => Object.freeze(material))),
    batches: Object.freeze(frozenBatches),
    brushModels,
    lightingSampleCount,
    lighting,
    entityCount,
    entityBytes,
    drawableSurfaces,
    displacementSurfaces,
    displacements: Object.freeze(displacements),
    models: Object.freeze(models),
    modelOccurrences: Object.freeze(modelOccurrences),
    lightmapLayout,
    lightmap,
  })
}

export async function validateRuntimeMapHashes(map: RuntimeMap): Promise<void> {
  if (map.lighting.profile !== "hdr") return
  await Promise.all([
    validateLightingClosure(map.lighting.descriptor),
    ...map.lighting.descriptor.profileMaterials.map(async (material) => {
      if (!equalBytes(await digest(material.texture.sourceBytes), material.texture.sourceSha256)) {
        throw new RuntimeMapError("profile texture SHA-256 differs")
      }
    }),
  ])
}

export async function parseRuntimeMapVerified(input: Uint8Array): Promise<RuntimeMap> {
  const map = parseRuntimeMap(input)
  await validateRuntimeMapHashes(map)
  return map
}
