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
  lightmapUv: Float32Array
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
  lightmap?: Readonly<{ width: number; height: number; rgba: Float32Array }>
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
  lightmapUv: number[]
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
    lightmapUv: [],
    indices: [],
    faces: [],
  }))
  let totalVertices = 0
  let totalTriangles = 0
  let drawableSurfaces = 0
  const lightmapRecords: Array<{
    batch: MutableBatch
    start: number
    uv: number[]
    offset: number
    width: number
    height: number
  }> = []
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
    const lightmapUv = Array.from({ length: vertexCount * 2 }, () => reader.f32())
    const indices = Array.from({ length: triangleCount * 3 }, () => reader.u32())
    if (indices.some((value) => value >= vertexCount)) {
      throw new RuntimeMapError("runtime map triangle index is invalid")
    }
    const lightOffset = reader.i32()
    reader.take(4)
    const lightmapWidth = Math.max(1, reader.i32() + 1)
    const lightmapHeight = Math.max(1, reader.i32() + 1)
    if (draw === 0 || model !== 0) continue
    const batch = batches[material]!
    const base = batch.positions.length / 3
    for (const value of positions) batch.positions.push(value)
    for (const value of normals) batch.normals.push(value)
    for (const value of uv) batch.uv.push(value)
    const lightmapStart = batch.lightmapUv.length
    for (let value = 0; value < lightmapUv.length; value += 1) batch.lightmapUv.push(0)
    lightmapRecords.push({
      batch,
      start: lightmapStart,
      uv: lightmapUv,
      offset: lightOffset,
      width: lightmapWidth,
      height: lightmapHeight,
    })
    for (const value of indices) batch.indices.push(value + base)
    for (let triangle = 0; triangle < triangleCount; triangle += 1) batch.faces.push(face)
    drawableSurfaces += 1
  }
  const lighting = reader.take(lightingSampleCount * 4)
  let lightmap: RuntimeMap["lightmap"]
  if (lightmapRecords.length > 0) {
    const atlasWidth = 4096
    let x = 1
    let y = 0
    let rowHeight = 1
    const placements: Array<{ x: number; y: number }> = []
    for (const record of lightmapRecords) {
      if (record.width < 1 || record.height < 1 || record.width > atlasWidth) {
        throw new RuntimeMapError("lightmap dimensions are invalid")
      }
      if (x + record.width > atlasWidth) {
        x = 0
        y += rowHeight
        rowHeight = 0
      }
      placements.push({ x, y })
      x += record.width
      rowHeight = Math.max(rowHeight, record.height)
    }
    const atlasHeight = y + rowHeight
    if (atlasHeight < 1 || atlasHeight > 4096) throw new RuntimeMapError("lightmap atlas exceeds its limit")
    const rgba = new Float32Array(atlasWidth * atlasHeight * 4)
    rgba.set([1, 1, 1, 1])
    for (const [recordIndex, record] of lightmapRecords.entries()) {
      const placement = placements[recordIndex]!
      const samples = record.width * record.height
      const source = record.offset >= 0 ? record.offset / 4 : -1
      if (source >= 0 && (!Number.isInteger(source) || source + samples > lightingSampleCount)) {
        throw new RuntimeMapError("lightmap sample range is invalid")
      }
      for (let sample = 0; sample < samples; sample += 1) {
        const targetX = placement.x + sample % record.width
        const targetY = placement.y + Math.floor(sample / record.width)
        const target = (targetY * atlasWidth + targetX) * 4
        if (source < 0) {
          rgba.set([1, 1, 1, 1], target)
        } else {
          const encoded = (source + sample) * 4
          const exponentByte = lighting[encoded + 3]!
          const exponent = exponentByte > 127 ? exponentByte - 256 : exponentByte
          const scale = 2 ** exponent / 255
          rgba[target] = lighting[encoded]! * scale
          rgba[target + 1] = lighting[encoded + 1]! * scale
          rgba[target + 2] = lighting[encoded + 2]! * scale
          rgba[target + 3] = 1
        }
      }
      for (let vertex = 0; vertex < record.uv.length / 2; vertex += 1) {
        record.batch.lightmapUv[record.start + vertex * 2] =
          (placement.x + record.uv[vertex * 2]! + 0.5) / atlasWidth
        record.batch.lightmapUv[record.start + vertex * 2 + 1] =
          (placement.y + record.uv[vertex * 2 + 1]! + 0.5) / atlasHeight
      }
    }
    lightmap = Object.freeze({ width: atlasWidth, height: atlasHeight, rgba })
  }
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
    lightmapUv: new Float32Array(batch.lightmapUv),
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
    lightmap,
  })
}
