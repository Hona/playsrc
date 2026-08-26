import * as THREE from "three/webgpu"

export const MAX_STATIC_PROPS_PER_BATCH = 512

export type StaticPropBatchInput = Readonly<{
  source: number
  geometry: THREE.BufferGeometry
  matrix: THREE.Matrix4
}>

export type StaticPropBatch = Readonly<{
  mesh: THREE.Mesh
  sources: Uint32Array
  setVisible(index: number, visible: boolean): boolean
  update(): void
  sourceAtFace(face: number): number | undefined
}>

export function createStaticPropBatch(
  occurrences: readonly StaticPropBatchInput[],
  material: THREE.Material,
): StaticPropBatch {
  if (occurrences.length < 2 || occurrences.length > MAX_STATIC_PROPS_PER_BATCH) {
    throw new Error("static-prop batch occurrence count is invalid")
  }

  const first = occurrences[0]!.geometry
  const names = Object.keys(first.attributes).sort()
  const vertices = occurrences.reduce((total, occurrence) => total + occurrence.geometry.getAttribute("position").count, 0)
  const indexCount = occurrences.reduce((total, occurrence) => {
    const index = occurrence.geometry.getIndex()
    if (!index || index.count % 3 !== 0) throw new Error("static-prop batch geometry indices are invalid")
    return total + index.count
  }, 0)
  const geometry = new THREE.BufferGeometry()
  for (const name of names) {
    const source = first.getAttribute(name)
    if (!(source instanceof THREE.BufferAttribute)) throw new Error("static-prop batch attribute is unsupported")
    const Constructor = source.array.constructor as new (size: number) => typeof source.array
    geometry.setAttribute(name, new THREE.BufferAttribute(new Constructor(vertices * source.itemSize), source.itemSize, source.normalized))
  }

  const complete = new Uint32Array(indexCount)
  const selected = new Uint32Array(indexCount)
  const offsets = new Uint32Array(occurrences.length + 1)
  const sources = new Uint32Array(occurrences.length)
  const faceSources = new Uint32Array(indexCount / 3)
  const visible = new Uint8Array(occurrences.length)
  const point = new THREE.Vector3()
  const normal = new THREE.Matrix3()
  let vertexOffset = 0
  let indexOffset = 0

  for (let occurrenceIndex = 0; occurrenceIndex < occurrences.length; occurrenceIndex += 1) {
    const occurrence = occurrences[occurrenceIndex]!
    const sourceGeometry = occurrence.geometry
    if (Object.keys(sourceGeometry.attributes).sort().join("\0") !== names.join("\0")) {
      throw new Error("static-prop batch geometry attributes differ")
    }
    const vertexCount = sourceGeometry.getAttribute("position").count
    normal.getNormalMatrix(occurrence.matrix)
    for (const name of names) {
      const source = sourceGeometry.getAttribute(name)
      const destination = geometry.getAttribute(name)
      if (!(source instanceof THREE.BufferAttribute) || source.itemSize !== destination.itemSize || source.normalized !== destination.normalized) {
        throw new Error("static-prop batch geometry attribute format differs")
      }
      if (name === "position" || name === "normal" || name === "tangent") {
        for (let vertex = 0; vertex < vertexCount; vertex += 1) {
          point.fromBufferAttribute(source, vertex)
          if (name === "position") point.applyMatrix4(occurrence.matrix)
          else point.applyMatrix3(normal).normalize()
          destination.setXYZ(vertexOffset + vertex, point.x, point.y, point.z)
          if (name === "tangent" && source.itemSize === 4) destination.setW(vertexOffset + vertex, source.getW(vertex))
        }
      } else {
        destination.array.set(source.array, vertexOffset * source.itemSize)
      }
    }
    const index = sourceGeometry.getIndex()!
    offsets[occurrenceIndex] = indexOffset
    sources[occurrenceIndex] = occurrence.source
    for (let at = 0; at < index.count; at += 1) complete[indexOffset + at] = vertexOffset + index.getX(at)
    indexOffset += index.count
    vertexOffset += vertexCount
  }
  offsets[occurrences.length] = indexOffset

  const index = new THREE.BufferAttribute(selected, 1)
  geometry.setIndex(index)
  geometry.setDrawRange(0, 0)
  geometry.computeBoundingSphere()
  const mesh = new THREE.Mesh(geometry, material)
  mesh.matrixAutoUpdate = false
  mesh.frustumCulled = false
  mesh.visible = false
  let dirty = false

  return Object.freeze({
    mesh,
    sources,
    setVisible(occurrence: number, value: boolean): boolean {
      if (occurrence < 0 || occurrence >= visible.length) throw new Error("static-prop batch occurrence identity is invalid")
      const next = Number(value)
      if (visible[occurrence] === next) return false
      visible[occurrence] = next
      dirty = true
      return true
    },
    update(): void {
      if (!dirty) return
      let count = 0
      for (let occurrence = 0; occurrence < visible.length; occurrence += 1) {
        if (visible[occurrence] === 0) continue
        const start = offsets[occurrence]!
        const end = offsets[occurrence + 1]!
        selected.set(complete.subarray(start, end), count)
        faceSources.fill(sources[occurrence]!, count / 3, (count + end - start) / 3)
        count += end - start
      }
      geometry.setDrawRange(0, count)
      mesh.visible = count !== 0
      if (count !== 0) {
        index.clearUpdateRanges()
        index.addUpdateRange(0, count)
        index.needsUpdate = true
      }
      dirty = false
    },
    sourceAtFace(face: number): number | undefined {
      return face >= 0 && face < geometry.drawRange.count / 3 ? faceSources[face] : undefined
    },
  })
}
