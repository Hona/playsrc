import * as THREE from "three/webgpu"

export type DynamicAttributeUpdate = Readonly<{
  changed: boolean
  offset: number
  count: number
  bytes: number
}>

export function synchronizeDynamicAttribute(
  attribute: THREE.BufferAttribute,
  source: Float32Array,
): DynamicAttributeUpdate {
  const destination = attribute.array
  if (!(destination instanceof Float32Array) || destination.length !== source.length) {
    throw new RangeError("Dynamic model attribute differs from its authored pose")
  }

  const current = new Uint32Array(destination.buffer, destination.byteOffset, destination.length)
  const next = new Uint32Array(source.buffer, source.byteOffset, source.length)
  let first = 0
  while (first < next.length && current[first] === next[first]) first += 1
  if (first === next.length) return { changed: false, offset: 0, count: 0, bytes: 0 }

  let end = next.length
  while (end > first && current[end - 1] === next[end - 1]) end -= 1
  destination.set(source.subarray(first, end), first)
  attribute.clearUpdateRanges()
  attribute.addUpdateRange(first, end - first)
  attribute.needsUpdate = true
  return { changed: true, offset: first, count: end - first, bytes: (end - first) * Float32Array.BYTES_PER_ELEMENT }
}
