const DEGREES_TO_RADIANS = Math.PI / 180

export type ParticleQuad = Readonly<{
  primitive: "sprite" | "trail"
  position: readonly [number, number, number]
  trailEndPosition: readonly [number, number, number]
  trailWidth: number
  radius: number
  rollRadians: number
  orientationType?: number
}>

export type ParticleQuadCamera = Readonly<{
  position: readonly [number, number, number]
  yawDegrees: number
  pitchDegrees: number
}>

export function writeParticleQuad(
  item: ParticleQuad,
  camera: ParticleQuadCamera,
  positions: Float32Array,
  offset: number,
): void {
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
    return
  }

  const yaw = camera.yawDegrees * DEGREES_TO_RADIANS
  const pitch = camera.pitchDegrees * DEGREES_TO_RADIANS
  const worldOriented = item.orientationType === 2
  const rightX = worldOriented ? 1 : Math.sin(yaw)
  const rightY = worldOriented ? 0 : -Math.cos(yaw)
  const upX = worldOriented ? 0 : Math.sin(pitch) * Math.cos(yaw)
  const upY = worldOriented ? -1 : Math.sin(pitch) * Math.sin(yaw)
  const upZ = worldOriented ? 0 : Math.cos(pitch)
  const cosine = Math.cos(item.rollRadians)
  const sine = Math.sin(item.rollRadians)
  const rolledRightX = rightX * cosine + upX * sine
  const rolledRightY = rightY * cosine + upY * sine
  const rolledRightZ = upZ * sine
  const rolledUpX = upX * cosine - rightX * sine
  const rolledUpY = upY * cosine - rightY * sine
  const rolledUpZ = upZ * cosine
  const radius = item.radius

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
}
