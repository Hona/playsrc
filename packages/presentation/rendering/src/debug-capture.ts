export type DebugPlaneKind = "color" | "depth" | "normal" | "material-id" | "primitive-id" | "object-id"

export type DebugPlane = Readonly<{
  kind: DebugPlaneKind
  width: number
  height: number
  components: 1 | 3 | 4
  scalar: "u8" | "f32" | "u32"
  bytes: Uint8Array
  sha256: string
}>

export type DebugCapture = Readonly<{
  identity: string
  frameOrdinal: number
  sceneIdentity: string
  width: number
  height: number
  planes: ReadonlyMap<DebugPlaneKind, DebugPlane>
}>

export type DebugPlaneInput = Readonly<{
  kind: DebugPlaneKind
  components: 1 | 3 | 4
  scalar: "u8" | "f32" | "u32"
  bytes: Uint8Array
}>

export type AlignedCaptureSide = Readonly<{
  runtime: string
  operatingSystem: string
  gpu: string
  driver: string
  planes: readonly Readonly<{ kind: DebugPlaneKind; sha256: string }>[]
}>

export type AlignedCaptureManifest = Readonly<{
  version: 1
  identity: string
  contentBuild: string
  mapLogicalPath: string
  mapSha256: string
  authoritySnapshot: string
  presentationTimestamp: number
  camera: Readonly<{
    position: readonly [number, number, number]
    angles: readonly [number, number, number]
    projection: readonly number[]
  }>
  viewport: readonly [number, number]
  lightingProfile: "ldr" | "hdr"
  exposure: number
  assetClosureSha256: string
  target: AlignedCaptureSide
  browser: AlignedCaptureSide
  comparisons: readonly PlaneComparison[]
}>

export type PlaneComparison =
  | Readonly<{ kind: "color"; maximumAbsolute: number; meanAbsolute: number }>
  | Readonly<{ kind: "depth"; maximumAbsolute: number }>
  | Readonly<{ kind: "normal"; minimumDot: number }>
  | Readonly<{ kind: "material-id" | "primitive-id" | "object-id"; exact: true }>

export type PlaneComparisonResult = Readonly<{
  kind: DebugPlaneKind
  passed: boolean
  maximumAbsolute?: number
  meanAbsolute?: number
  minimumDot?: number
  mismatchedValues?: number
}>

export class DebugCaptureError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "DebugCaptureError"
  }
}

const HASH = /^[0-9a-f]{64}$/
const REQUIRED_PLANES = Object.freeze(["color", "depth", "normal", "material-id", "primitive-id", "object-id"] as const)

async function digest(bytes: Uint8Array): Promise<string> {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes).buffer))
  return Array.from(hash, (value) => value.toString(16).padStart(2, "0")).join("")
}

function bytesPerScalar(scalar: DebugPlaneInput["scalar"]): number {
  return scalar === "u8" ? 1 : 4
}

function expectedShape(kind: DebugPlaneKind): Pick<DebugPlaneInput, "components" | "scalar"> {
  if (kind === "color") return { components: 4, scalar: "u8" }
  if (kind === "normal") return { components: 3, scalar: "f32" }
  if (kind === "depth") return { components: 1, scalar: "f32" }
  return { components: 1, scalar: "u32" }
}

export async function createDebugCapture(input: Readonly<{
  identity: string
  frameOrdinal: number
  sceneIdentity: string
  width: number
  height: number
  planes: readonly DebugPlaneInput[]
}>): Promise<DebugCapture> {
  if (
    !input.identity || !input.sceneIdentity
    || !Number.isSafeInteger(input.frameOrdinal) || input.frameOrdinal < 1
    || !Number.isSafeInteger(input.width) || input.width < 1
    || !Number.isSafeInteger(input.height) || input.height < 1
    || input.planes.length !== REQUIRED_PLANES.length
  ) throw new DebugCaptureError("debug capture identity or dimensions are invalid")
  const planes = new Map<DebugPlaneKind, DebugPlane>()
  for (const source of input.planes) {
    const shape = expectedShape(source.kind)
    if (
      planes.has(source.kind)
      || source.components !== shape.components
      || source.scalar !== shape.scalar
      || source.bytes.length !== input.width * input.height * source.components * bytesPerScalar(source.scalar)
    ) throw new DebugCaptureError("debug capture plane is invalid")
    const bytes = source.bytes.slice()
    planes.set(source.kind, Object.freeze({ ...source, width: input.width, height: input.height, bytes, sha256: await digest(bytes) }))
  }
  if (REQUIRED_PLANES.some((kind) => !planes.has(kind))) throw new DebugCaptureError("debug capture plane set is incomplete")
  return Object.freeze({ ...input, planes })
}

function finiteVector(value: readonly number[], length?: number): boolean {
  return (length === undefined || value.length === length) && value.every(Number.isFinite)
}

function validateSide(side: AlignedCaptureSide): void {
  if (!side.runtime || !side.operatingSystem || !side.gpu || !side.driver || side.planes.length !== REQUIRED_PLANES.length) {
    throw new DebugCaptureError("aligned capture environment is incomplete")
  }
  const planes = new Map(side.planes.map((plane) => [plane.kind, plane.sha256]))
  if (planes.size !== REQUIRED_PLANES.length || REQUIRED_PLANES.some((kind) => !HASH.test(planes.get(kind) ?? ""))) {
    throw new DebugCaptureError("aligned capture plane identities are incomplete")
  }
}

export function validateAlignedCaptureManifest(manifest: AlignedCaptureManifest): AlignedCaptureManifest {
  if (
    manifest.version !== 1 || !manifest.identity || !manifest.contentBuild
    || !manifest.mapLogicalPath.startsWith("maps/") || !manifest.mapLogicalPath.endsWith(".bsp")
    || !HASH.test(manifest.mapSha256) || !HASH.test(manifest.assetClosureSha256)
    || !manifest.authoritySnapshot || !Number.isFinite(manifest.presentationTimestamp)
    || !finiteVector(manifest.camera.position, 3) || !finiteVector(manifest.camera.angles, 3)
    || !finiteVector(manifest.camera.projection, 16)
    || manifest.viewport.length !== 2 || manifest.viewport.some((value) => !Number.isSafeInteger(value) || value < 1)
    || !Number.isFinite(manifest.exposure) || manifest.exposure <= 0
    || manifest.comparisons.length !== REQUIRED_PLANES.length
  ) throw new DebugCaptureError("aligned capture manifest is invalid")
  validateSide(manifest.target)
  validateSide(manifest.browser)
  const comparisons = new Map<DebugPlaneKind, PlaneComparison>()
  for (const comparison of manifest.comparisons) {
    if (comparisons.has(comparison.kind)) throw new DebugCaptureError("aligned capture comparison is duplicated")
    if (comparison.kind === "color" && (
      !Number.isFinite(comparison.maximumAbsolute) || comparison.maximumAbsolute < 0
      || !Number.isFinite(comparison.meanAbsolute) || comparison.meanAbsolute < 0
    )) throw new DebugCaptureError("color comparison tolerance is invalid")
    if (comparison.kind === "depth" && (!Number.isFinite(comparison.maximumAbsolute) || comparison.maximumAbsolute < 0)) {
      throw new DebugCaptureError("depth comparison tolerance is invalid")
    }
    if (comparison.kind === "normal" && (!Number.isFinite(comparison.minimumDot) || comparison.minimumDot < -1 || comparison.minimumDot > 1)) {
      throw new DebugCaptureError("normal comparison tolerance is invalid")
    }
    comparisons.set(comparison.kind, comparison)
  }
  if (REQUIRED_PLANES.some((kind) => !comparisons.has(kind))) throw new DebugCaptureError("aligned capture comparisons are incomplete")
  return manifest
}

function float32(bytes: Uint8Array): Float32Array {
  if (bytes.byteOffset % 4 === 0) return new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4)
  return new Float32Array(bytes.slice().buffer)
}

function uint32(bytes: Uint8Array): Uint32Array {
  if (bytes.byteOffset % 4 === 0) return new Uint32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4)
  return new Uint32Array(bytes.slice().buffer)
}

export function compareDebugPlanes(
  target: DebugPlane,
  browser: DebugPlane,
  comparison: PlaneComparison,
): PlaneComparisonResult {
  if (
    target.kind !== browser.kind || target.kind !== comparison.kind
    || target.width !== browser.width || target.height !== browser.height
    || target.components !== browser.components || target.scalar !== browser.scalar
    || target.bytes.length !== browser.bytes.length
  ) throw new DebugCaptureError("debug planes are not aligned")
  if (comparison.kind === "color") {
    let maximumAbsolute = 0, total = 0
    for (let index = 0; index < target.bytes.length; index += 1) {
      const difference = Math.abs(target.bytes[index]! - browser.bytes[index]!) / 255
      maximumAbsolute = Math.max(maximumAbsolute, difference)
      total += difference
    }
    const meanAbsolute = total / target.bytes.length
    return Object.freeze({ kind: comparison.kind, passed: maximumAbsolute <= comparison.maximumAbsolute && meanAbsolute <= comparison.meanAbsolute, maximumAbsolute, meanAbsolute })
  }
  if (comparison.kind === "depth") {
    const left = float32(target.bytes), right = float32(browser.bytes)
    let maximumAbsolute = 0
    for (let index = 0; index < left.length; index += 1) {
      if (!Number.isFinite(left[index]!) || !Number.isFinite(right[index]!)) throw new DebugCaptureError("depth plane contains a non-finite value")
      maximumAbsolute = Math.max(maximumAbsolute, Math.abs(left[index]! - right[index]!))
    }
    return Object.freeze({ kind: comparison.kind, passed: maximumAbsolute <= comparison.maximumAbsolute, maximumAbsolute })
  }
  if (comparison.kind === "normal") {
    const left = float32(target.bytes), right = float32(browser.bytes)
    let minimumDot = 1
    for (let index = 0; index < left.length; index += 3) {
      const l = Math.hypot(left[index]!, left[index + 1]!, left[index + 2]!)
      const r = Math.hypot(right[index]!, right[index + 1]!, right[index + 2]!)
      if (l <= 0 || r <= 0) throw new DebugCaptureError("normal plane contains a zero vector")
      minimumDot = Math.min(minimumDot, (
        left[index]! * right[index]! + left[index + 1]! * right[index + 1]! + left[index + 2]! * right[index + 2]!
      ) / (l * r))
    }
    return Object.freeze({ kind: comparison.kind, passed: minimumDot >= comparison.minimumDot, minimumDot })
  }
  const left = uint32(target.bytes), right = uint32(browser.bytes)
  let mismatchedValues = 0
  for (let index = 0; index < left.length; index += 1) mismatchedValues += Number(left[index] !== right[index])
  return Object.freeze({ kind: comparison.kind, passed: mismatchedValues === 0, mismatchedValues })
}
