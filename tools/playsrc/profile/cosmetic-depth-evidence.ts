import type { Camera } from "@playsrc/rendering"
import type { ParticleRenderItem } from "@playsrc/particle"
import { decodeScreenshot } from "./screenshot-pixels"

/** Compare actual pre-particle GPU colors/depth with the same completed frame. */
export function cosmeticDepthEvidence(input: {
  camera: Camera; particles: readonly ParticleRenderItem[]; width: number; height: number
  format: string; colorSpace: string; before: Uint8Array; depth: Uint8Array; after: Buffer
}) {
  const image = decodeScreenshot(input.after)
  if (image.width !== input.width || image.height !== input.height) throw new Error("Depth capture dimensions differ")
  const yaw = input.camera.yawDegrees * Math.PI / 180, pitch = input.camera.pitchDegrees * Math.PI / 180
  const forward = [Math.cos(pitch) * Math.cos(yaw), Math.cos(pitch) * Math.sin(yaw), -Math.sin(pitch)]
  const right = [Math.sin(yaw), -Math.cos(yaw), 0], up = [Math.sin(pitch) * Math.cos(yaw), Math.sin(pitch) * Math.sin(yaw), Math.cos(pitch)]
  const focal = input.height / (2 * Math.tan(input.camera.verticalFovDegrees * Math.PI / 360))
  const project = (particle: ParticleRenderItem) => {
    const d = particle.position.map((v, i) => v - input.camera.position[i]!)
    const dot = (v: number[]) => d.reduce((sum, p, i) => sum + p * v[i]!, 0)
    const z = dot(forward)
    return { particle, x: input.width / 2 + dot(right) / z * focal, y: input.height / 2 - dot(up) / z * focal,
      radius: particle.radius / z * focal, depth: (input.camera.far * z / (input.camera.far - input.camera.near) - input.camera.far * input.camera.near / (input.camera.far - input.camera.near)) / 192 }
  }
  const quads = input.particles.filter(p => p.effectIdentity >= 0x6000_0000 && p.primitive === "sprite" && p.opacity > 0).map(project)
  const bytes = new DataView(input.before.buffer, input.before.byteOffset, input.before.byteLength)
  const half = (bits: number) => { const e = (bits >> 10) & 31, m = bits & 1023, sign = bits & 32768 ? -1 : 1; return sign * (e === 31 ? m ? NaN : Infinity : e === 0 ? m * 2 ** -24 : (1 + m / 1024) * 2 ** (e - 15)) }
  const display = (value: number) => Math.round(Math.max(0, Math.min(1, input.colorSpace === "srgb-linear" ? value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055 : value)) * 255)
  const before = (pixel: number, channel: number) => input.format === "rgba16float" ? display(half(bytes.getUint16(pixel * 8 + channel * 2, true)))
    : display(input.before[pixel * 4 + (input.format === "bgra8unorm" ? 2 - channel : channel)]! / 255)
  let covered = 0, occluded = 0, unchanged = 0, visibleChanged = 0
  const examples: { x: number; y: number; scene: number; nearest: number; difference: number }[] = []
  // The first-person launcher occupies the right side. The left actor is unobstructed by HUD/viewmodel passes.
  for (let y = 80; y < Math.min(input.height * 0.7, input.height); y++) for (let x = 40; x < input.width / 2 - 20; x++) {
    const overlapping = quads.filter(q => {
      const dx = (x + 0.5 - q.x) / q.radius, dy = (q.y - y - 0.5) / q.radius
      const c = Math.cos(q.particle.rollRadians), s = Math.sin(q.particle.rollRadians)
      return Math.abs(dx * c + dy * s) <= 1.01 && Math.abs(-dx * s + dy * c) <= 1.01
    })
    if (!overlapping.length) continue
    covered++
    const pixel = y * input.width + x, scene = input.depth[pixel * 4 + 3]! / 255
    const difference = Math.max(...[0,1,2].map(c => Math.abs(before(pixel, c) - image.pixels[pixel * image.channels + c]!)))
    const nearest = Math.min(...overlapping.map(q => q.depth))
    if (scene < 0.9 && nearest > scene + 4 / 255) {
      occluded++; if (difference <= 2) unchanged++
      if (examples.length < 16) examples.push({ x, y, scene, nearest, difference })
    } else if (nearest < scene - 4 / 255 && difference > 15) visibleChanged++
  }
  return { covered, occluded, unchanged, visibleChanged, examples }
}
