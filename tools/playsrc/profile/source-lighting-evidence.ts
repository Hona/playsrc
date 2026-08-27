import { decodeScreenshot } from "./screenshot-pixels"

type LightingSample = Readonly<{ x: number; y: number; material: string; worldNormal: readonly number[]; worldPosition: readonly number[] }>

// Configured Upward RED spawn: two downward ceiling spotlights illuminate the
// upper launcher surface, not its downward-facing sight edge. Pair real pixels
// with the posed triangle normals so exposure-only changes cannot pass this.
export function upwardSpawnLightingEvidence(png: Buffer, lighting: Readonly<{
  geometry: Readonly<{ samples: readonly LightingSample[] }>
  viewmodel: Readonly<{ localLights: readonly Readonly<{ kind: string; direction: readonly number[]; position: readonly number[] }>[] }>
}>) {
  const upper = lighting.geometry.samples.find(sample => sample.x === 0.7 && sample.y === -0.4)
  const lower = lighting.geometry.samples.find(sample => sample.x === 0.7 && sample.y === -0.2)
  if (!upper || !lower || upper.material !== lower.material || !upper.material.includes("c_rocketlauncher")
    || upper.worldNormal[2]! <= 0.8 || lower.worldNormal[2]! >= 0
    || !lighting.viewmodel.localLights.some(light => light.kind === "spot" && light.direction[2]! < -0.99
      && light.position[2]! > Math.max(upper.worldPosition[2]!, lower.worldPosition[2]!))) {
    throw new Error("configured Upward ceiling-light/launcher-face evidence is unavailable")
  }
  const image = decodeScreenshot(png)
  const luminance = (sample: LightingSample) => {
    const x = Math.round((sample.x + 1) * image.width / 2), y = Math.round((1 - sample.y) * image.height / 2)
    const offset = (y * image.width + x) * image.channels
    return image.pixels[offset]! * 0.2126 + image.pixels[offset + 1]! * 0.7152 + image.pixels[offset + 2]! * 0.0722
  }
  return { upperNormal: upper.worldNormal, lowerNormal: lower.worldNormal, upperLuminance: luminance(upper), lowerLuminance: luminance(lower) }
}
