const MAX_PAYLOAD_BYTES = 536_870_912
const MAX_MATERIALS = 65_536
const MAX_SURFACES = 1_000_000
const MAX_VERTICES = 16_777_216
const MAX_TRIANGLES = 16_777_216

export type RuntimeMaterial = Readonly<{
  logicalPath: string
  width: number
  height: number
}>

export type RuntimeBatch = Readonly<{
  material: number
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
  faces: Uint32Array
}>

export type RuntimeMap = Readonly<{
  bspVersion: number
  mapRevision: number
  lightingProfile: 0 | 1
  materials: readonly RuntimeMaterial[]
  batches: readonly RuntimeBatch[]
  lightingSampleCount: number
  entityCount: number
  entityBytes: Uint8Array
  drawableSurfaces: number
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
    return this.take(1)[0]!
  }
  u32(): number {
    const offset = this.offset
    this.take(4)
    return this.view.getUint32(offset, true)
  }
  i32(): number {
    const offset = this.offset
    this.take(4)
    return this.view.getInt32(offset, true)
  }
  f32(): number {
    const offset = this.offset
    this.take(4)
    const value = this.view.getFloat32(offset, true)
    if (!Number.isFinite(value)) throw new RuntimeMapError("runtime map contains a non-finite scalar")
    return value
  }
  sized(): Uint8Array {
    return this.take(this.u32())
  }
}

type MutableBatch = {
  positions: number[]
  normals: number[]
  uv: number[]
  indices: number[]
  faces: number[]
}

function bounded(value: number, maximum: number, field: string): number {
  if (value > maximum) throw new RuntimeMapError(`${field} exceeds its limit`)
  return value
}

export function parseRuntimeMap(input: Uint8Array): RuntimeMap {
  if (input.byteLength < 37 || input.byteLength > MAX_PAYLOAD_BYTES) {
    throw new RuntimeMapError("runtime map byte length is invalid")
  }
  const reader = new Reader(input)
  if (new TextDecoder().decode(reader.take(4)) !== "PSMP" || reader.u32() !== 1) {
    throw new RuntimeMapError("runtime map identity is invalid")
  }
  const bspVersion = reader.u32()
  const mapRevision = reader.u32()
  const lightingProfile = reader.u8()
  if (lightingProfile !== 0 && lightingProfile !== 1) {
    throw new RuntimeMapError("runtime map lighting profile is invalid")
  }
  const materialCount = bounded(reader.u32(), MAX_MATERIALS, "material count")
  const surfaceCount = bounded(reader.u32(), MAX_SURFACES, "surface count")
  const lightingSampleCount = bounded(reader.u32(), MAX_VERTICES, "lighting sample count")
  const entityCount = bounded(reader.u32(), MAX_SURFACES, "entity count")
  const decoder = new TextDecoder("utf-8", { fatal: true })
  const materials: RuntimeMaterial[] = []
  for (let index = 0; index < materialCount; index += 1) {
    let logicalPath: string
    try {
      logicalPath = decoder.decode(reader.sized())
    } catch {
      throw new RuntimeMapError("runtime map material path is not UTF-8")
    }
    const width = reader.i32()
    const height = reader.i32()
    if (!logicalPath || width < 1 || height < 1) {
      throw new RuntimeMapError("runtime map material record is invalid")
    }
    materials.push(Object.freeze({ logicalPath, width, height }))
  }
  const batches = Array.from({ length: materialCount }, (): MutableBatch => ({
    positions: [],
    normals: [],
    uv: [],
    indices: [],
    faces: [],
  }))
  let totalVertices = 0
  let totalTriangles = 0
  let drawableSurfaces = 0
  for (let index = 0; index < surfaceCount; index += 1) {
    const face = reader.u32()
    const model = reader.u32()
    const material = reader.u32()
    reader.i32()
    const draw = reader.u8()
    const vertexCount = bounded(reader.u32(), MAX_VERTICES, "surface vertex count")
    const triangleCount = bounded(reader.u32(), MAX_TRIANGLES, "surface triangle count")
    if (material >= materialCount || draw > 1 || vertexCount < 3) {
      throw new RuntimeMapError("runtime map surface record is invalid")
    }
    totalVertices = bounded(totalVertices + vertexCount, MAX_VERTICES, "total vertex count")
    totalTriangles = bounded(totalTriangles + triangleCount, MAX_TRIANGLES, "total triangle count")
    const positions = Array.from({ length: vertexCount * 3 }, () => reader.f32())
    const normals = Array.from({ length: vertexCount * 3 }, () => reader.f32())
    const uv = Array.from({ length: vertexCount * 2 }, () => reader.f32())
    for (let value = 0; value < vertexCount * 2; value += 1) reader.f32()
    const indices = Array.from({ length: triangleCount * 3 }, () => reader.u32())
    if (indices.some((value) => value >= vertexCount)) {
      throw new RuntimeMapError("runtime map triangle index is invalid")
    }
    reader.i32()
    reader.take(4)
    reader.i32()
    reader.i32()
    if (draw === 0 || model !== 0) continue
    const batch = batches[material]!
    const base = batch.positions.length / 3
    for (const value of positions) batch.positions.push(value)
    for (const value of normals) batch.normals.push(value)
    for (const value of uv) batch.uv.push(value)
    for (const value of indices) batch.indices.push(value + base)
    for (let triangle = 0; triangle < triangleCount; triangle += 1) batch.faces.push(face)
    drawableSurfaces += 1
  }
  reader.take(lightingSampleCount * 4)
  const entityBytes = reader.sized().slice()
  if (reader.offset !== input.byteLength) throw new RuntimeMapError("runtime map has trailing bytes")
  const frozenBatches = batches.flatMap((batch, material): RuntimeBatch[] => batch.indices.length === 0 ? [] : [Object.freeze({
    material,
    positions: new Float32Array(batch.positions),
    normals: new Float32Array(batch.normals),
    uv: new Float32Array(batch.uv),
    indices: new Uint32Array(batch.indices),
    faces: new Uint32Array(batch.faces),
  })])
  return Object.freeze({
    bspVersion,
    mapRevision,
    lightingProfile,
    materials: Object.freeze(materials),
    batches: Object.freeze(frozenBatches),
    lightingSampleCount,
    entityCount,
    entityBytes,
    drawableSurfaces,
  })
}
