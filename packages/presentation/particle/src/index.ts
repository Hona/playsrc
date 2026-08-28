const OUTPUT_MAGIC = 0x5250_5350 // "PSPR"
const OUTPUT_VERSION = 5
const OUTPUT_HEADER_BYTES = 40
const OUTPUT_RECORD_BYTES = 436
/** Standard native PSPR admission, shared with the final frame consumer. */
export const PARTICLE_RENDER_OUTPUT_LIMITS = Object.freeze({ maxOutputBytes: 64 * 1024 * 1024, maxRenderItems: 65_536 })
const HEX_BYTES = Object.freeze(Array.from({ length: 256 }, (_, value) => value.toString(16).padStart(2, "0")))

export type PcfResource = Readonly<{
  logicalPath: string
  bytes: Uint8Array
}>

export type ParticleBatch = Readonly<{
  /** Compact versioned event/advance bytes consumed as one complete Rust phase. */
  bytes: Uint8Array
}>

export type ParticleKernelSession = Readonly<{
  materials: readonly string[]
  transact(batch: Uint8Array): Uint8Array
  reset(bytes: Uint8Array): void
  dispose(): void
}>

export type ParticleKernel = Readonly<{
  /** Loads all supplied PCF bytes in one bounded Rust registry operation. */
  load(resources: readonly PcfResource[]): ParticleKernelSession
}>

export type ParticleAdapterLimits = Readonly<{
  maxOutputBytes: number
  maxRenderItems: number
}>

export type ParticleRenderItem = Readonly<{
  sky: boolean
  identity: number
  effectIdentity: number
  particleIdentity: number
  rendererIndex: number
  primitive: "sprite" | "trail" | "rope"
  visibility?: Readonly<{ identity: bigint; vertices: Float32Array; clipFraction: number }>
  mesh?: Readonly<{ positions: Float32Array; uv: Float32Array; colors: Uint8Array; indices: Uint32Array }>
  systemUuid: string
  material: string
  position: readonly [number, number, number]
  previousPosition: readonly [number, number, number]
  radius: number
  rollRadians: number
  yawRadians: number
  color: number
  opacity: number
  sequence: number
  secondarySequence: number
  trailLength: number
  trailLengthScale: number
  trailEndPosition: readonly [number, number, number]
  trailWidth: number
  sortKey: number
  ageSeconds: number
  lifetimeSeconds: number
  animationRate: number
  secondaryAnimationRate: number
  stepSeconds: number
  trailMinLength: number
  trailMaxLength: number
  trailFadeInSeconds: number
  orientationType: number
  animationFitLifetime: boolean
  animationRateAsFps: boolean
  materialShader: "sprite-card" | "mesh-sprite"
  textureColorSpace: "srgb-texture-linear-tint"
  blendSource: "zero" | "one" | "source-alpha" | "one-minus-source-alpha"
  blendDestination: "zero" | "one" | "source-alpha" | "one-minus-source-alpha"
  stableTieIdentity: bigint
  primarySheet: ParticleSheetSample | null
  secondarySheet: ParticleSheetSample | null
}>

export type ParticleRenderOutput = Readonly<{
  bounds: Readonly<{
    minimum: readonly [number, number, number]
    maximum: readonly [number, number, number]
  }> | null
  items: readonly ParticleRenderItem[]
}>

export type ParticleSheetSample = Readonly<{
  current: readonly (readonly [number, number, number, number])[]
  next: readonly (readonly [number, number, number, number])[]
  blend: number
}>

export class ParticleAdapterError extends Error {
  constructor(
    readonly code: "MalformedInput" | "MalformedOutput" | "BoundExceeded" | "InvalidState",
    message: string,
  ) {
    super(message)
    this.name = "ParticleAdapterError"
  }
}

export function createParticleSystem(
  kernel: ParticleKernel,
  resources: readonly PcfResource[],
  limits: ParticleAdapterLimits = PARTICLE_RENDER_OUTPUT_LIMITS,
): Readonly<{
  advance(batch: ParticleBatch): ParticleRenderOutput
  reset(bytes: Uint8Array): void
  dispose(): void
}> {
  validateLimits(limits)
  validateResources(resources)
  const session = kernel.load(resources)
  validateMaterials(session.materials)
  let disposed = false
  return Object.freeze({
    advance(batch: ParticleBatch): ParticleRenderOutput {
      if (disposed) throw new ParticleAdapterError("InvalidState", "particle adapter is disposed")
      if (!(batch.bytes instanceof Uint8Array) || batch.bytes.byteLength === 0) {
        throw new ParticleAdapterError("MalformedInput", "particle batch must contain bytes")
      }
      return decodeParticleRenderOutput(session.transact(batch.bytes), session.materials, limits)
    },
    reset(bytes: Uint8Array): void {
      if (disposed || !(bytes instanceof Uint8Array) || bytes.byteLength === 0) {
        throw new ParticleAdapterError("InvalidState", "particle reset bytes or state are invalid")
      }
      session.reset(bytes)
    },
    dispose(): void {
      if (!disposed) session.dispose()
      disposed = true
    },
  })
}

export function decodeParticleRenderOutput(
  bytes: Uint8Array,
  materials: readonly string[],
  limits: ParticleAdapterLimits = PARTICLE_RENDER_OUTPUT_LIMITS,
): ParticleRenderOutput {
  validateLimits(limits)
  validateMaterials(materials)
  if (!(bytes instanceof Uint8Array) || bytes.byteLength < OUTPUT_HEADER_BYTES || bytes.byteLength > limits.maxOutputBytes) {
    throw new ParticleAdapterError("BoundExceeded", "particle output byte length is invalid")
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (view.getUint32(0, true) !== OUTPUT_MAGIC || view.getUint32(4, true) !== OUTPUT_VERSION) {
    throw new ParticleAdapterError("MalformedOutput", "particle output identity is invalid")
  }
  const count = view.getUint32(8, true)
  if (count > limits.maxRenderItems) {
    throw new ParticleAdapterError("BoundExceeded", "particle render item count exceeds its limit")
  }
  const expected = OUTPUT_HEADER_BYTES + count * OUTPUT_RECORD_BYTES
  if (!Number.isSafeInteger(expected) || expected > bytes.byteLength) {
    throw new ParticleAdapterError("MalformedOutput", "particle output records do not frame its bytes")
  }
  const boundsState = view.getUint32(12, true)
  if (boundsState > 1) {
    throw new ParticleAdapterError("MalformedOutput", "particle bounds state is invalid")
  }
  const minimum = tuple3(view, 16)
  const maximum = tuple3(view, 28)
  if (
    !finite3(minimum)
    || !finite3(maximum)
    || (boundsState === 0
      ? minimum.some(value => value !== 0) || maximum.some(value => value !== 0)
      : minimum.some((value, component) => value > maximum[component]!))
  ) {
    throw new ParticleAdapterError("MalformedOutput", "particle bounds are invalid")
  }
  const bounds = boundsState === 0 ? null : Object.freeze({ minimum, maximum })
  const output: ParticleRenderItem[] = new Array(count)
  const sheets = new SheetImagesCache(view)
  for (let index = 0; index < count; index += 1) {
    const offset = OUTPUT_HEADER_BYTES + index * OUTPUT_RECORD_BYTES
    const primitive = bytes[offset + 14]
    if ((primitive !== 0 && primitive !== 1 && primitive !== 2) || bytes[offset + 15]! > 3) {
      throw new ParticleAdapterError("MalformedOutput", "particle primitive or reserved byte is invalid")
    }
    const materialIndex = view.getUint32(offset + 32, true)
    const material = materials[materialIndex]
    if (material === undefined) {
      throw new ParticleAdapterError("MalformedOutput", "particle material index is invalid")
    }
    const position = tuple3(view, offset + 36)
    const previousPosition = tuple3(view, offset + 48)
    const radius = view.getFloat32(offset + 60, true)
    const rollRadians = view.getFloat32(offset + 64, true)
    const color = view.getUint32(offset + 68, true)
    const opacity = view.getFloat32(offset + 72, true)
    const sequence = view.getInt32(offset + 76, true)
    const trailLength = view.getFloat32(offset + 80, true)
    const sortKey = view.getFloat32(offset + 84, true)
    const ageSeconds = view.getFloat32(offset + 88, true)
    const lifetimeSeconds = view.getFloat32(offset + 92, true)
    const animationRate = view.getFloat32(offset + 96, true)
    const trailMinLength = view.getFloat32(offset + 100, true)
    const trailMaxLength = view.getFloat32(offset + 104, true)
    const trailFadeInSeconds = view.getFloat32(offset + 108, true)
    const orientationType = view.getInt32(offset + 112, true)
    const flags = view.getUint32(offset + 116, true)
    const secondarySequence = view.getInt32(offset + 120, true)
    const sheetFlags = view.getUint32(offset + 124, true)
    const primarySheet = (sheetFlags & 1) === 0 ? null : sheetSample(view, sheets, offset + 128, offset + 132, offset + 196)
    const secondarySheet = (sheetFlags & 2) === 0 ? null : sheetSample(view, sheets, offset + 260, offset + 264, offset + 328)
    const materialShader = bytes[offset + 392]
    const colorSpace = bytes[offset + 393]
    const blendSource = blendFactor(bytes[offset + 394]!)
    const blendDestination = blendFactor(bytes[offset + 395]!)
    const secondaryAnimationRate = view.getFloat32(offset + 396, true)
    const stepSeconds = view.getFloat32(offset + 400, true)
    const trailEndPosition = tuple3(view, offset + 404)
    const trailWidth = view.getFloat32(offset + 416, true)
    const trailLengthScale = view.getFloat32(offset + 420, true)
    const yawRadians = view.getFloat32(offset + 432, true)
    if (
      !finite3(position)
      || !finite3(previousPosition)
      || !finite3(trailEndPosition)
      || !Number.isFinite(radius)
      || !Number.isFinite(rollRadians)
      || !Number.isFinite(yawRadians)
      || !Number.isFinite(opacity)
      || !Number.isFinite(trailLength)
      || !Number.isFinite(sortKey)
      || !Number.isFinite(ageSeconds)
      || !Number.isFinite(lifetimeSeconds)
      || !Number.isFinite(animationRate)
      || !Number.isFinite(trailMinLength)
      || !Number.isFinite(trailMaxLength)
      || !Number.isFinite(trailFadeInSeconds)
      || !Number.isFinite(secondaryAnimationRate)
      || !Number.isFinite(stepSeconds)
      || !Number.isFinite(trailWidth)
      || !Number.isFinite(trailLengthScale)
      || radius < 0
      || opacity < 0
      || opacity > 1
      || trailLength < 0
      || ageSeconds < 0
      || lifetimeSeconds <= 0
      || trailMinLength < 0
      || trailMaxLength < 0
      || trailFadeInSeconds < 0
      || secondaryAnimationRate < 0
      || stepSeconds <= 0
      || trailWidth < 0
      || trailLengthScale < 0
      || orientationType < 0 || orientationType > 2
      || (flags & ~3) !== 0
      || (sheetFlags & ~3) !== 0
      || (materialShader !== 0 && materialShader !== 1)
      || colorSpace !== 0
      || primarySheet === null
    ) {
      throw new ParticleAdapterError("MalformedOutput", "particle output contains an invalid scalar")
    }
    output[index] = Object.freeze({
      identity: view.getUint32(offset, true),
      effectIdentity: view.getUint32(offset + 4, true),
      particleIdentity: view.getUint32(offset + 8, true),
      rendererIndex: view.getUint16(offset + 12, true),
      primitive: primitive === 0 ? "sprite" : primitive === 1 ? "trail" : "rope",
      sky: (bytes[offset + 15]! & 1) !== 0,
      systemUuid: uuid(bytes, offset + 16),
      material,
      position,
      previousPosition,
      radius,
      rollRadians,
      yawRadians,
      color: color & 0xff_ffff,
      opacity,
      sequence,
      secondarySequence,
      trailLength,
      trailLengthScale,
      trailEndPosition,
      trailWidth,
      sortKey,
      ageSeconds,
      lifetimeSeconds,
      animationRate,
      secondaryAnimationRate,
      stepSeconds,
      trailMinLength,
      trailMaxLength,
      trailFadeInSeconds,
      orientationType,
      animationFitLifetime: (flags & 1) !== 0,
      animationRateAsFps: (flags & 2) !== 0,
      materialShader: materialShader === 0 ? "sprite-card" : "mesh-sprite",
      textureColorSpace: "srgb-texture-linear-tint",
      blendSource,
      blendDestination,
      stableTieIdentity: view.getBigUint64(offset + 424, true),
      primarySheet,
      secondarySheet,
    })
  }
  let at = expected
  let verticesRemaining = limits.maxRenderItems * 4
  for (let index = 0; index < output.length; index++) {
    let item = output[index]!
    if ((bytes[OUTPUT_HEADER_BYTES + index * OUTPUT_RECORD_BYTES + 15]! & 2) !== 0) {
      if (at + 72 > bytes.byteLength) throw new ParticleAdapterError("MalformedOutput", "visibility proxy is truncated")
      const identity = view.getBigUint64(at, true), vertices = new Float32Array(15), clipFraction = view.getFloat32(at + 68, true)
      for (let index = 0; index < 15; index++) vertices[index] = view.getFloat32(at + 8 + index * 4, true)
      if (!vertices.every(Number.isFinite) || !Number.isFinite(clipFraction) || clipFraction < 0 || clipFraction > 1) throw new ParticleAdapterError("MalformedOutput", "visibility proxy is invalid")
      item = output[index] = Object.freeze({ ...item, visibility: Object.freeze({ identity, vertices, clipFraction }) })
      at += 72
    }
    if (item.primitive !== "rope") { verticesRemaining -= 4; continue }
    if (at + 8 > bytes.byteLength) throw new ParticleAdapterError("MalformedOutput", "rope header is truncated")
    const vertices = view.getUint32(at, true), indicesCount = view.getUint32(at + 4, true)
    at += 8
    if (vertices < 4 || vertices % 2 !== 0 || vertices > verticesRemaining || indicesCount !== (vertices / 2 - 1) * 6) {
      throw new ParticleAdapterError("BoundExceeded", "rope geometry count is invalid")
    }
    verticesRemaining -= vertices
    if (at + vertices * 24 + indicesCount * 4 > bytes.byteLength) throw new ParticleAdapterError("MalformedOutput", "rope geometry is truncated")
    const positions = new Float32Array(vertices * 3), uv = new Float32Array(vertices * 2)
    for (const array of [positions, uv]) for (let component = 0; component < array.length; component++, at += 4) {
      const value = view.getFloat32(at, true)
      if (!Number.isFinite(value)) throw new ParticleAdapterError("MalformedOutput", "rope vertex is nonfinite")
      array[component] = value
    }
    const colors = bytes.slice(at, at + vertices * 4); at += vertices * 4
    const indices = new Uint32Array(indicesCount)
    for (let component = 0; component < indicesCount; component++, at += 4) {
      const value = view.getUint32(at, true)
      if (value >= vertices) throw new ParticleAdapterError("MalformedOutput", "rope index is out of range")
      indices[component] = value
    }
    output[index] = Object.freeze({ ...item, mesh: Object.freeze({ positions, uv, colors, indices }) })
  }
  if (verticesRemaining < 0 || at !== bytes.byteLength) throw new ParticleAdapterError("MalformedOutput", "particle geometry does not frame its output")
  return Object.freeze({ bounds, items: Object.freeze(output) })
}

function blendFactor(value: number): ParticleRenderItem["blendSource"] {
  switch (value) {
    case 0: return "zero"
    case 1: return "one"
    case 2: return "source-alpha"
    case 3: return "one-minus-source-alpha"
    default: throw new ParticleAdapterError("MalformedOutput", "particle blend factor is invalid")
  }
}

function validateResources(resources: readonly PcfResource[]): void {
  const identities = new Set<string>()
  for (const resource of resources) {
    if (
      !logicalPath(resource.logicalPath)
      || !(resource.bytes instanceof Uint8Array)
      || resource.bytes.byteLength === 0
      || identities.has(resource.logicalPath.toLowerCase())
    ) {
      throw new ParticleAdapterError("MalformedInput", "PCF resource is malformed or duplicated")
    }
    identities.add(resource.logicalPath.toLowerCase())
  }
}

function validateMaterials(materials: readonly string[]): void {
  if (!Array.isArray(materials) || materials.some((material) => !logicalPath(material))) {
    throw new ParticleAdapterError("MalformedOutput", "particle material registry is invalid")
  }
}

function validateLimits(limits: ParticleAdapterLimits): void {
  if (!positive(limits.maxOutputBytes) || !positive(limits.maxRenderItems)) {
    throw new ParticleAdapterError("BoundExceeded", "particle adapter limits must be positive integers")
  }
}

function logicalPath(value: string): boolean {
  return typeof value === "string"
    && value.length > 0
    && value.length <= 1_024
    && !value.startsWith("/")
    && !value.includes("\\")
    && value.split("/").every((component) => component.length > 0 && component !== "." && component !== "..")
    && !/[\u0000-\u001f\u007f]/.test(value)
}

function positive(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0
}

function tuple3(view: DataView, offset: number): readonly [number, number, number] {
  return Object.freeze([
    view.getFloat32(offset, true),
    view.getFloat32(offset + 4, true),
    view.getFloat32(offset + 8, true),
  ])
}

function finite3(value: readonly [number, number, number]): boolean {
  return Number.isFinite(value[0]) && Number.isFinite(value[1]) && Number.isFinite(value[2])
}

function sheetSample(
  view: DataView,
  sheets: SheetImagesCache,
  blendOffset: number,
  currentOffset: number,
  nextOffset: number,
): ParticleSheetSample {
  const blend = view.getFloat32(blendOffset, true)
  const current = sheets.read(currentOffset)
  const next = sheets.read(nextOffset)
  if (!Number.isFinite(blend) || blend < 0 || blend > 1) {
    throw new ParticleAdapterError("MalformedOutput", "particle sheet blend is invalid")
  }
  return Object.freeze({ current, next, blend })
}

type SheetImages = ParticleSheetSample["current"]

/** Authored frame rectangles recur across particles. Intern only within this
 * immutable packet: no global cache, retained packet, float canonicalization or
 * hash-only equality. Returned arrays still own frozen copies of the values. */
class SheetImagesCache {
  readonly #view: DataView
  readonly #buckets = new Map<number, Array<{ offset: number; images: SheetImages }>>()
  #size = 0
  constructor(view: DataView) { this.#view = view }
  read(offset: number): SheetImages {
    const view = this.#view
    let hash = 0x811c9dc5
    for (let at = 0; at < 64; at += 4) hash = Math.imul(hash ^ view.getUint32(offset + at, true), 0x01000193)
    const bucket = this.#buckets.get(hash)
    if (bucket) for (const entry of bucket) {
      let equal = true
      for (let at = 0; at < 64; at += 4) if (view.getUint32(offset + at, true) !== view.getUint32(entry.offset + at, true)) { equal = false; break }
      if (equal) return entry.images
    }
    const images = sheetImages(view, offset)
    if (this.#size < 512 && (!bucket || bucket.length < 4)) {
      if (bucket) bucket.push({ offset, images })
      else this.#buckets.set(hash, [{ offset, images }])
      this.#size++
    }
    return images
  }
}

function sheetImages(view: DataView, offset: number): SheetImages {
  const output: (readonly [number, number, number, number])[] = new Array(4)
  for (let image = 0; image < 4; image += 1) {
    const start = offset + image * 16
    const left = view.getFloat32(start, true), top = view.getFloat32(start + 4, true)
    const right = view.getFloat32(start + 8, true), bottom = view.getFloat32(start + 12, true)
    if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
      throw new ParticleAdapterError("MalformedOutput", "particle sheet rectangle is invalid")
    }
    output[image] = Object.freeze([left, top, right, bottom])
  }
  return Object.freeze(output)
}

function uuid(bytes: Uint8Array, offset: number): string {
  let identity = ""
  for (let index = 0; index < 16; index += 1) identity += HEX_BYTES[bytes[offset + index]!]!
  return identity
}
