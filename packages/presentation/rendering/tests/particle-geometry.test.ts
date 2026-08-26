import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { createParticleQuadWriter, writeParticleQuad, type ParticleQuad, type ParticleQuadCamera } from "../src/particle-geometry"

function previousQuad(item: ParticleQuad, camera: ParticleQuadCamera): Float32Array {
  const positions = new Float32Array(12)
  if (item.primitive === "trail") {
    const delta = new THREE.Vector3().fromArray(item.trailEndPosition).sub(new THREE.Vector3().fromArray(item.position))
    const center = new THREE.Vector3().fromArray(item.position)
    const tangent = center.clone().sub(new THREE.Vector3().fromArray(camera.position)).cross(delta).normalize()
    const width = item.trailWidth * 0.5
    const vertices = [center.clone().addScaledVector(tangent, width), center.clone().addScaledVector(tangent, -width)]
    vertices.push(vertices[1]!.clone().add(delta), vertices[0]!.clone().add(delta))
    vertices.forEach((vertex, index) => vertex.toArray(positions, index * 3))
  } else {
    const yaw = THREE.MathUtils.degToRad(camera.yawDegrees)
    const pitch = THREE.MathUtils.degToRad(camera.pitchDegrees)
    const right = new THREE.Vector3(Math.sin(yaw), -Math.cos(yaw), 0)
    const up = new THREE.Vector3(Math.sin(pitch) * Math.cos(yaw), Math.sin(pitch) * Math.sin(yaw), Math.cos(pitch))
    const cosine = Math.cos(item.rollRadians)
    const sine = Math.sin(item.rollRadians)
    const rolledRight = right.clone().multiplyScalar(cosine).addScaledVector(up, sine)
    const rolledUp = up.clone().multiplyScalar(cosine).addScaledVector(right, -sine)
    const center = new THREE.Vector3().fromArray(item.position)
    for (const [index, [x, y]] of ([[-1, 1], [1, 1], [1, -1], [-1, -1]] as const).entries()) {
      center.clone().addScaledVector(rolledRight, x * item.radius).addScaledVector(rolledUp, y * item.radius).toArray(positions, index * 3)
    }
  }
  return positions
}

const camera: ParticleQuadCamera = { position: [5328, 3376, -3067.96875], yawDegrees: 161.75, pitchDegrees: -18.5 }
const base: ParticleQuad = {
  primitive: "sprite",
  position: [5310.125, 3394.625, -3058.25],
  trailEndPosition: [5298.5, 3382.25, -3049.75],
  trailWidth: 11.75,
  radius: 23.5,
  rollRadians: 0.72,
}

describe("allocation-free Source Particle geometry", () => {
  test("preserves exact binary32 camera-facing explosion and smoke sprite vertices", () => {
    const actual = new Float32Array(12)
    writeParticleQuad(base, camera, actual, 0)
    expect(actual).toEqual(previousQuad(base, camera))
  })

  test("retains exact binary32 sprite, world-oriented, and trail vertices across one shared camera basis", () => {
    const items = [base, { ...base, orientationType: 2 }, { ...base, primitive: "trail" as const }]
    const retained = new Float32Array(items.length * 12)
    const independent = new Float32Array(items.length * 12)
    const write = createParticleQuadWriter(camera)
    items.forEach((item, index) => {
      write(item, retained, index * 12)
      writeParticleQuad(item, camera, independent, index * 12)
    })
    expect(retained).toEqual(independent)
  })

  test("keeps authored orientation-two Medi Gun sprites on the Source world XY plane", () => {
    const oriented: ParticleQuad = { ...base, position: [2, 3, 4], radius: 2, rollRadians: 0, orientationType: 2 }
    const first = new Float32Array(12)
    const second = new Float32Array(12)
    writeParticleQuad(oriented, camera, first, 0)
    writeParticleQuad(oriented, { position: [100, 200, 300], yawDegrees: -41, pitchDegrees: 63 }, second, 0)
    expect([...first]).toEqual([0, 1, 4, 4, 1, 4, 4, 5, 4, 0, 5, 4])
    expect(second).toEqual(first)
  })

  test("preserves exact binary32 rocket trail endpoints and camera-facing width", () => {
    const trail = { ...base, primitive: "trail" as const }
    const actual = new Float32Array(16)
    writeParticleQuad(trail, camera, actual, 2)
    expect(actual.slice(2, 14)).toEqual(previousQuad(trail, camera))
  })

  test("retains zero-width collinear trails without NaN or a guessed camera offset", () => {
    const trail: ParticleQuad = {
      ...base,
      primitive: "trail",
      position: [1, 0, 0],
      trailEndPosition: [2, 0, 0],
    }
    const actual = new Float32Array(12)
    writeParticleQuad(trail, { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0 }, actual, 0)
    expect([...actual]).toEqual([1, 0, 0, 1, 0, 0, 2, 0, 0, 2, 0, 0])
  })
})
