const DEGREES_TO_RADIANS = Math.PI / 180

/** The Source quad order is clockwise. Three's front faces are counterclockwise. */
export function writeParticleQuadIndices(indices: Uint16Array | Uint32Array): void {
  for (let offset = 0, vertex = 0; offset < indices.length; offset += 6, vertex += 4) {
    indices[offset] = vertex; indices[offset + 1] = vertex + 2; indices[offset + 2] = vertex + 1
    indices[offset + 3] = vertex; indices[offset + 4] = vertex + 3; indices[offset + 5] = vertex + 2
  }
}

export type ParticleQuad = Readonly<{
  primitive: "sprite" | "trail"
  position: readonly [number, number, number]
  trailEndPosition: readonly [number, number, number]
  trailWidth: number
  radius: number
  rollRadians: number
  orientationType?: number
  yawRadians?: number
  materialShader?: "sprite-card" | "mesh-sprite"
}>

export type ParticleQuadCamera = Readonly<{
  position: readonly [number, number, number]
  yawDegrees: number
  pitchDegrees: number
}>

export type ParticleQuadWriter = (item: ParticleQuad, positions: Float32Array, offset: number) => number

export function createParticleQuadWriter(camera: ParticleQuadCamera): ParticleQuadWriter {
  const yaw = camera.yawDegrees * DEGREES_TO_RADIANS
  const pitch = camera.pitchDegrees * DEGREES_TO_RADIANS
  const rightX = Math.sin(yaw)
  const rightY = -Math.cos(yaw)
  const upX = Math.sin(pitch) * Math.cos(yaw)
  const upY = Math.sin(pitch) * Math.sin(yaw)
  const upZ = Math.cos(pitch)
  return (item, positions, offset) => writeQuad(item, camera, positions, offset, rightX, rightY, upX, upY, upZ)
}

export function writeParticleQuad(
  item: ParticleQuad,
  camera: ParticleQuadCamera,
  positions: Float32Array,
  offset: number,
): void {
  createParticleQuadWriter(camera)(item, positions, offset)
}

function writeQuad(
  item: ParticleQuad,
  camera: ParticleQuadCamera,
  positions: Float32Array,
  offset: number,
  cameraRightX: number,
  cameraRightY: number,
  cameraUpX: number,
  cameraUpY: number,
  cameraUpZ: number,
): number {
  const centerX = item.position[0]
  const centerY = item.position[1]
  const centerZ = item.position[2]

  if (item.primitive === "trail") {
    const deltaX = item.trailEndPosition[0] - centerX
    const deltaY = item.trailEndPosition[1] - centerY
    const deltaZ = item.trailEndPosition[2] - centerZ
    const eyeX = centerX - camera.position[0]
    const eyeY = centerY - camera.position[1]
    const eyeZ = centerZ - camera.position[2]
    let tangentX = eyeY * deltaZ - eyeZ * deltaY
    let tangentY = eyeZ * deltaX - eyeX * deltaZ
    let tangentZ = eyeX * deltaY - eyeY * deltaX
    const inverseLength = 1 / (Math.sqrt(tangentX * tangentX + tangentY * tangentY + tangentZ * tangentZ) || 1)
    tangentX *= inverseLength
    tangentY *= inverseLength
    tangentZ *= inverseLength
    const halfWidth = item.trailWidth * 0.5
    const firstX = centerX + tangentX * halfWidth
    const firstY = centerY + tangentY * halfWidth
    const firstZ = centerZ + tangentZ * halfWidth
    const secondX = centerX - tangentX * halfWidth
    const secondY = centerY - tangentY * halfWidth
    const secondZ = centerZ - tangentZ * halfWidth
    positions[offset] = firstX
    positions[offset + 1] = firstY
    positions[offset + 2] = firstZ
    positions[offset + 3] = secondX
    positions[offset + 4] = secondY
    positions[offset + 5] = secondZ
    positions[offset + 6] = secondX + deltaX
    positions[offset + 7] = secondY + deltaY
    positions[offset + 8] = secondZ + deltaZ
    positions[offset + 9] = firstX + deltaX
    positions[offset + 10] = firstY + deltaY
    positions[offset + 11] = firstZ + deltaZ
    return 1
  }

  const worldOriented = item.orientationType === 2
  let rightX = worldOriented ? 1 : cameraRightX
  let rightY = worldOriented ? 0 : cameraRightY
  let upX = worldOriented ? 0 : cameraUpX
  let upY = worldOriented ? -1 : cameraUpY
  let upZ = worldOriented ? 0 : cameraUpZ
  let radius = item.radius, tint = 1
  if (item.orientationType === 1) {
    const mesh = item.materialShader === "mesh-sprite"
    const deltaX = centerX - camera.position[0], deltaY = centerY - camera.position[1], deltaZ = centerZ - camera.position[2]
    const distance = Math.hypot(deltaX, deltaY, deltaZ)
    const basisX = mesh ? camera.position[0] : deltaX, basisY = mesh ? camera.position[1] : deltaY
    const inverse = 1 / (Math.hypot(basisX, basisY) || 1)
    const yaw = mesh ? 0 : item.yawRadians ?? 0, cosine = Math.cos(yaw), sine = Math.sin(yaw)
    rightX = (-basisY * cosine + basisX * sine) * inverse
    rightY = (basisX * cosine + basisY * sine) * inverse
    upX = 0; upY = 0; upZ = mesh ? 1 : -1
    if (mesh ? distance < radius * 0.5 : distance <= radius * 0.5) { radius = 0; tint = 0 }
    else if (!mesh && radius > 0 && distance < radius * 2) {
      const t = Math.min(1, (distance - radius * 0.5) / (radius * 0.5))
      tint = t * t * (3 - 2 * t)
    }
  }
  const cosine = Math.cos(item.rollRadians)
  const sine = Math.sin(item.rollRadians)
  const rolledRightX = rightX * cosine + upX * sine
  const rolledRightY = rightY * cosine + upY * sine
  const rolledRightZ = upZ * sine
  const rolledUpX = upX * cosine - rightX * sine
  const rolledUpY = upY * cosine - rightY * sine
  const rolledUpZ = upZ * cosine

  positions[offset] = centerX - rolledRightX * radius + rolledUpX * radius
  positions[offset + 1] = centerY - rolledRightY * radius + rolledUpY * radius
  positions[offset + 2] = centerZ - rolledRightZ * radius + rolledUpZ * radius
  positions[offset + 3] = centerX + rolledRightX * radius + rolledUpX * radius
  positions[offset + 4] = centerY + rolledRightY * radius + rolledUpY * radius
  positions[offset + 5] = centerZ + rolledRightZ * radius + rolledUpZ * radius
  positions[offset + 6] = centerX + rolledRightX * radius - rolledUpX * radius
  positions[offset + 7] = centerY + rolledRightY * radius - rolledUpY * radius
  positions[offset + 8] = centerZ + rolledRightZ * radius - rolledUpZ * radius
  positions[offset + 9] = centerX - rolledRightX * radius - rolledUpX * radius
  positions[offset + 10] = centerY - rolledRightY * radius - rolledUpY * radius
  positions[offset + 11] = centerZ - rolledRightZ * radius - rolledUpZ * radius
  return tint
}
