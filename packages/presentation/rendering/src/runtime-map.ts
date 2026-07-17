const MAX_PAYLOAD_BYTES = 536_870_912
const MAX_MATERIALS = 65_536
const MAX_SURFACES = 1_000_000
const MAX_VERTICES = 16_777_216
const MAX_TRIANGLES = 16_777_216

export type RuntimeMaterial = Readonly<{
  logicalPath: string
  width: number
  height: number
  shader: number
  features: number
  baseTexture?: Readonly<{
    logicalPath: string
    width: number
    height: number
    rgba: Uint8Array
  }>
}>

export type RuntimeBatch = Readonly<{
  material: number
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
  faces: Uint32Array
}>
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
  models: readonly RuntimeModel[]
  modelOccurrences: readonly RuntimeModelOccurrence[]
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

function resolvedMaterial(
  reader: Reader,
  decoder: TextDecoder,
  base: Pick<RuntimeMaterial, "logicalPath" | "width" | "height">,
): RuntimeMaterial {
  const shader = reader.u8()
  const features = reader.u8()
  const hasTexture = reader.u8()
  if (reader.u8() !== 0 || hasTexture > 1) {
    throw new RuntimeMapError("runtime material payload is invalid")
  }
  let baseTexture: RuntimeMaterial["baseTexture"]
  if (hasTexture === 1) {
    let logicalPath: string
    try {
      logicalPath = decoder.decode(reader.sized())
    } catch {
      throw new RuntimeMapError("runtime texture path is not UTF-8")
    }
    const width = reader.u32()
    const height = reader.u32()
    const rgba = reader.sized().slice()
    if (!logicalPath || width < 1 || height < 1 || width * height * 4 !== rgba.byteLength) {
      throw new RuntimeMapError("runtime texture payload is invalid")
    }
    baseTexture = Object.freeze({ logicalPath, width, height, rgba })
  }
  return Object.freeze({ ...base, shader, features, baseTexture })
}

export function parseRuntimeMap(input: Uint8Array): RuntimeMap {
  if (input.byteLength < 37 || input.byteLength > MAX_PAYLOAD_BYTES) {
    throw new RuntimeMapError("runtime map byte length is invalid")
  }
  const reader = new Reader(input)
  if (new TextDecoder().decode(reader.take(4)) !== "PSMP") {
    throw new RuntimeMapError("runtime map identity is invalid")
  }
  const schema = reader.u32()
  if (schema !== 1 && schema !== 2 && schema !== 3) {
    throw new RuntimeMapError("runtime map schema is invalid")
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
    materials.push({ logicalPath, width, height, shader: 0, features: 0 })
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
  if (schema >= 2) {
    const resolvedCount = reader.u32()
    if (resolvedCount !== materials.length) {
      throw new RuntimeMapError("runtime material payload count is invalid")
    }
    for (let index = 0; index < resolvedCount; index += 1) {
      materials[index] = resolvedMaterial(reader, decoder, materials[index]!)
    }
  }
  const models: RuntimeModel[] = []
  const modelOccurrences: RuntimeModelOccurrence[] = []
  if (schema === 3) {
    const modelCount = bounded(reader.u32(), 4096, "runtime model count")
    for (let modelIndex = 0; modelIndex < modelCount; modelIndex += 1) {
      let logicalPath: string
      try {
        logicalPath = decoder.decode(reader.sized())
      } catch {
        throw new RuntimeMapError("runtime model path is not UTF-8")
      }
      const materialCount = bounded(reader.u32(), MAX_MATERIALS, "model material count")
      const modelMaterials: RuntimeMaterial[] = []
      for (let material = 0; material < materialCount; material += 1) {
        let materialPath: string
        try {
          materialPath = decoder.decode(reader.sized())
        } catch {
          throw new RuntimeMapError("runtime model material path is not UTF-8")
        }
        modelMaterials.push(resolvedMaterial(reader, decoder, {
          logicalPath: materialPath,
          width: 1,
          height: 1,
        }))
      }
      const primitiveCount = bounded(reader.u32(), 65_536, "model primitive count")
      const primitives: RuntimeModelPrimitive[] = []
      for (let primitive = 0; primitive < primitiveCount; primitive += 1) {
        const material = reader.u32()
        const vertices = bounded(reader.u32(), MAX_VERTICES, "model vertex count")
        const triangles = bounded(reader.u32(), MAX_TRIANGLES, "model triangle count")
        if (material >= materialCount) throw new RuntimeMapError("model material index is invalid")
        const positions = Float32Array.from({ length: vertices * 3 }, () => reader.f32())
        const normals = Float32Array.from({ length: vertices * 3 }, () => reader.f32())
        const uv = Float32Array.from({ length: vertices * 2 }, () => reader.f32())
        const indices = Uint32Array.from({ length: triangles * 3 }, () => reader.u32())
        if (indices.some((index) => index >= vertices)) {
          throw new RuntimeMapError("model triangle index is invalid")
        }
        primitives.push(Object.freeze({ material, positions, normals, uv, indices }))
      }
      models.push(Object.freeze({
        logicalPath,
        materials: Object.freeze(modelMaterials),
        primitives: Object.freeze(primitives),
      }))
    }
    const occurrenceCount = bounded(reader.u32(), MAX_SURFACES, "model occurrence count")
    for (let index = 0; index < occurrenceCount; index += 1) {
      const entity = reader.u32()
      const model = reader.u32()
      if (model >= models.length) throw new RuntimeMapError("model occurrence index is invalid")
      const position = Object.freeze([reader.f32(), reader.f32(), reader.f32()]) as readonly [number, number, number]
      const angles = Object.freeze([reader.f32(), reader.f32(), reader.f32()]) as readonly [number, number, number]
      modelOccurrences.push(Object.freeze({ entity, model, position, angles }))
    }
  }
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
    materials: Object.freeze(materials.map((material) => Object.freeze(material))),
    batches: Object.freeze(frozenBatches),
    lightingSampleCount,
    entityCount,
    entityBytes,
    drawableSurfaces,
    models: Object.freeze(models),
    modelOccurrences: Object.freeze(modelOccurrences),
  })
}
