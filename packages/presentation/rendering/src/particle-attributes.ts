export type ParticleAppearance = Readonly<{
  color: number
  opacity: number
  primarySheet: Readonly<{
    current: readonly (readonly number[])[]
    next: readonly (readonly number[])[]
    blend: number
  }> | null
}>

export type ParticleAppearanceArrays = Readonly<{
  uv: Float32Array
  uvNext: Float32Array
  sheetBlend: Float32Array
  colors: Float32Array
}>

export type ParticleAttributeUpdate = { start: number; end: number }

export function createParticleAttributeUpdates(): ParticleAttributeUpdate[] {
  return Array.from({ length: 4 }, () => ({ start: Number.POSITIVE_INFINITY, end: 0 }))
}

export function resetParticleAttributeUpdates(updates: readonly ParticleAttributeUpdate[]): void {
  for (let index = 0; index < updates.length; index += 1) {
    updates[index]!.start = Number.POSITIVE_INFINITY
    updates[index]!.end = 0
  }
}

export function writeParticleAppearance(
  item: ParticleAppearance,
  arrays: ParticleAppearanceArrays,
  index: number,
  updates: readonly ParticleAttributeUpdate[],
  tint = 1,
): void {
  const sheet = item.primarySheet!
  const current = sheet.current[0]!
  const next = sheet.next[0]!
  const currentLeft = Math.fround(current[0]!)
  const currentTop = Math.fround(current[1]!)
  const currentRight = Math.fround(current[2]!)
  const currentBottom = Math.fround(current[3]!)
  const nextLeft = Math.fround(next[0]!)
  const nextTop = Math.fround(next[1]!)
  const nextRight = Math.fround(next[2]!)
  const nextBottom = Math.fround(next[3]!)
  const red = Math.fround(((item.color >> 16) & 255) / 255 * tint)
  const green = Math.fround(((item.color >> 8) & 255) / 255 * tint)
  const blue = Math.fround((item.color & 255) / 255 * tint)
  const opacity = Math.fround(item.opacity * tint)
  const blend = Math.fround(sheet.blend)

  for (let vertex = 0; vertex < 4; vertex += 1) {
    const uvOffset = index * 8 + vertex * 2
    const colorOffset = index * 16 + vertex * 4
    const right = vertex === 1 || vertex === 2
    const bottom = vertex >= 2
    write(arrays.uv, uvOffset, right ? currentRight : currentLeft, updates[0]!)
    write(arrays.uv, uvOffset + 1, bottom ? currentBottom : currentTop, updates[0]!)
    write(arrays.uvNext, uvOffset, right ? nextRight : nextLeft, updates[1]!)
    write(arrays.uvNext, uvOffset + 1, bottom ? nextBottom : nextTop, updates[1]!)
    write(arrays.sheetBlend, index * 4 + vertex, blend, updates[2]!)
    write(arrays.colors, colorOffset, red, updates[3]!)
    write(arrays.colors, colorOffset + 1, green, updates[3]!)
    write(arrays.colors, colorOffset + 2, blue, updates[3]!)
    write(arrays.colors, colorOffset + 3, opacity, updates[3]!)
  }
}

function write(array: Float32Array, offset: number, value: number, update: ParticleAttributeUpdate): void {
  if (Object.is(array[offset], value)) return
  array[offset] = value
  if (offset < update.start) update.start = offset
  if (offset >= update.end) update.end = offset + 1
}
