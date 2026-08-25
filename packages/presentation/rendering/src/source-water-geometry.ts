import { SourceWaterError } from "./source-water"

type Vector3 = readonly [number, number, number]

export type SourceWaterGeometryInput = Readonly<{
  positions: Float32Array
  normals: Float32Array
  uv: Float32Array
  indices: Uint32Array
  faces: Uint32Array
  surfacePlanes: ReadonlyMap<number, readonly [number, number, number, number]>
}>

export type SourceWaterTangentAttributes = Readonly<{
  tangentS: Float32Array
  tangentT: Float32Array
}>

function subtract(left: Vector3, right: Vector3): Vector3 {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

function cross(left: Vector3, right: Vector3): Vector3 {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ]
}

function dot(left: Vector3, right: Vector3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

function normalize(value: Vector3, face: number): Vector3 {
  const length = Math.hypot(...value)
  if (!Number.isFinite(length) || length === 0) {
    throw new SourceWaterError(`Water surface ${face} has no recoverable authored texture basis`)
  }
  return [value[0] / length, value[1] / length, value[2] / length]
}

function vertex(values: Float32Array, index: number): Vector3 {
  return [values[index * 3]!, values[index * 3 + 1]!, values[index * 3 + 2]!]
}

function gradient(
  firstEdge: Vector3,
  secondEdge: Vector3,
  area: Vector3,
  firstDelta: number,
  secondDelta: number,
  face: number,
): Vector3 {
  const first = cross(secondEdge, area)
  const second = cross(area, firstEdge)
  const squared = dot(area, area)
  if (!Number.isFinite(squared) || squared === 0) {
    throw new SourceWaterError(`Water surface ${face} contains a degenerate triangle`)
  }
  return normalize([
    (first[0] * firstDelta + second[0] * secondDelta) / squared,
    (first[1] * firstDelta + second[1] * secondDelta) / squared,
    (first[2] * firstDelta + second[2] * secondDelta) / squared,
  ], face)
}

export function sourceWaterTangentAttributes(input: SourceWaterGeometryInput): SourceWaterTangentAttributes {
  const vertexCount = input.positions.length / 3
  if (
    !Number.isSafeInteger(vertexCount)
    || vertexCount < 3
    || input.normals.length !== input.positions.length
    || input.uv.length !== vertexCount * 2
    || input.indices.length !== input.faces.length * 3
    || input.indices.length < 3
    || !input.positions.every(Number.isFinite)
    || !input.normals.every(Number.isFinite)
    || !input.uv.every(Number.isFinite)
  ) {
    throw new SourceWaterError("Water surface geometry is malformed")
  }

  const tangentS = new Float32Array(input.positions.length)
  const tangentT = new Float32Array(input.positions.length)
  const assigned = new Uint8Array(vertexCount)
  for (let triangle = 0; triangle < input.faces.length; triangle += 1) {
    const face = input.faces[triangle]!
    const plane = input.surfacePlanes.get(face)
    if (!plane || !plane.every(Number.isFinite)) {
      throw new SourceWaterError(`Water surface ${face} has no authored oriented plane`)
    }
    const first = input.indices[triangle * 3]!
    const second = input.indices[triangle * 3 + 1]!
    const third = input.indices[triangle * 3 + 2]!
    if (first >= vertexCount || second >= vertexCount || third >= vertexCount) {
      throw new SourceWaterError(`Water surface ${face} has an out-of-range vertex index`)
    }
    const firstEdge = subtract(vertex(input.positions, second), vertex(input.positions, first))
    const secondEdge = subtract(vertex(input.positions, third), vertex(input.positions, first))
    const area = cross(firstEdge, secondEdge)
    const sourceS = gradient(
      firstEdge,
      secondEdge,
      area,
      input.uv[second * 2]! - input.uv[first * 2]!,
      input.uv[third * 2]! - input.uv[first * 2]!,
      face,
    )
    const sourceT = gradient(
      firstEdge,
      secondEdge,
      area,
      input.uv[second * 2 + 1]! - input.uv[first * 2 + 1]!,
      input.uv[third * 2 + 1]! - input.uv[first * 2 + 1]!,
      face,
    )
    const reverse = dot([plane[0], plane[1], plane[2]], cross(sourceS, sourceT)) > 0
    for (const index of [first, second, third]) {
      if (assigned[index]) continue
      const normal = vertex(input.normals, index)
      const firstTangent = normalize(cross(normal, sourceT), face)
      const secondTangent = normalize(cross(firstTangent, normal), face)
      tangentS.set(reverse ? firstTangent.map((component) => -component) : firstTangent, index * 3)
      tangentT.set(secondTangent, index * 3)
      assigned[index] = 1
    }
  }
  if (assigned.some((value) => value === 0)) {
    throw new SourceWaterError("Water surface contains a vertex without an authored triangle")
  }
  return Object.freeze({ tangentS, tangentT })
}
