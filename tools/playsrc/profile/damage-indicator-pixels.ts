import { decodeScreenshot } from "./screenshot-pixels"

export type DamageQuad = Readonly<{ x: number; y: number; width: number; height: number; rotation: number }>

/** Audit actual composited game pixels. The texture's alpha-weighted red mass
 * points along +V, not -V: the dense red edge should face the screen centre.
 * No texture replacement or HUD mutation is used to obtain these pixels. */
export function damageIndicatorPixels(png: Buffer, texturePng: Buffer, quad: DamageQuad, viewport: { width: number; height: number }) {
  const image = decodeScreenshot(png), texture = decodeScreenshot(texturePng)
  let textureMass = 0, textureY = 0
  for (let y = 0; y < texture.height; y++) for (let x = 0; x < texture.width; x++) {
    const i = (y * texture.width + x) * texture.channels
    const mass = texture.pixels[i]! * (texture.pixels[i + 3]! / 255)
    textureMass += mass
    textureY += (y + .5) * mass
  }
  const scale = image.width / viewport.width
  const cx = quad.x + quad.width / 2, cy = quad.y + quad.height / 2
  const radius = Math.hypot(quad.width, quad.height) / 2
  let mass = 0, sumX = 0, sumY = 0, redPixels = 0
  for (let y = Math.max(0, Math.floor((cy - radius) * scale)); y < Math.min(image.height, Math.ceil((cy + radius) * scale)); y++) {
    for (let x = Math.max(0, Math.floor((cx - radius) * scale)); x < Math.min(image.width, Math.ceil((cx + radius) * scale)); x++) {
      const dx = (x + .5) / scale - cx, dy = (y + .5) / scale - cy
      // Restrict to the drawn quad, excluding unrelated RED world geometry.
      const u = dx * Math.cos(quad.rotation) + dy * Math.sin(quad.rotation)
      const v = -dx * Math.sin(quad.rotation) + dy * Math.cos(quad.rotation)
      if (Math.abs(u) > quad.width / 2 || Math.abs(v) > quad.height / 2) continue
      const i = (y * image.width + x) * image.channels
      const weight = Math.max(0, image.pixels[i]! - Math.max(image.pixels[i + 1]!, image.pixels[i + 2]!) - 50)
      if (weight) redPixels++
      mass += weight; sumX += dx * weight; sumY += dy * weight
    }
  }
  const dx = sumX / mass, dy = sumY / mass
  const inwardX = viewport.width / 2 - cx, inwardY = viewport.height / 2 - cy
  return { redPixels, mass, centroidOffset: [dx, dy],
    inwardCosine: (dx * inwardX + dy * inwardY) / (Math.hypot(dx, dy) * Math.hypot(inwardX, inwardY)),
    texture: { width: texture.width, height: texture.height, alphaWeightedV: textureY / textureMass / texture.height }, scale }
}
