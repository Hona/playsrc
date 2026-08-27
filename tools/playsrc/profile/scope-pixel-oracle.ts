import { tf2AuthoredScope } from "../../../games/tf2/browser/src/ui-resources/scope"
import { tf2ScopeGeometry } from "../../../games/tf2/browser/src/hud-integration/scope"
import { evaluateSourceRefractPixel } from "../../../packages/presentation/rendering/src/source-refract"
import { decodeScreenshot, type DecodedScreenshot } from "./screenshot-pixels"
import { inflateSync } from "node:zlib"

const linear = (c: number) => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4
const srgb = (c: number) => Math.round(255 * (c <= .0031308 ? 12.92 * c : 1.055 * Math.max(0, c) ** (1 / 2.4) - .055))

function sample(image: DecodedScreenshot, u: number, v: number, color: boolean): [number, number, number, number] {
  const x = u * image.width - .5, y = v * image.height - .5
  const left = Math.floor(x), top = Math.floor(y), fx = x - left, fy = y - top
  const output = [0, 0, 0, 0] as [number, number, number, number]
  for (const [dx, dy, weight] of [[0, 0, (1 - fx) * (1 - fy)], [1, 0, fx * (1 - fy)], [0, 1, (1 - fx) * fy], [1, 1, fx * fy]]) {
    const offset = (Math.max(0, Math.min(image.height - 1, top + dy!)) * image.width + Math.max(0, Math.min(image.width - 1, left + dx!))) * image.channels
    for (let channel = 0; channel < 4; channel++) {
      const value = channel === 3 && image.channels === 3 ? 1 : image.pixels[offset + channel]! / 255
      output[channel]! += (color && channel < 3 ? linear(value) : value) * weight!
    }
  }
  return output
}

/** Same-frame world readback is the oracle's input, not a generated backdrop.
 * Compare the actual GPU material result, including refraction, four blur taps,
 * sRGB tint sampling, normal alpha and blending. Never just test transparency. */
export function scopePixelOracle(before: DecodedScreenshot, after: DecodedScreenshot, quality: { picmip: number; trilinear: number; anisotropy: number }) {
  if (before.width !== after.width || before.height !== after.height) throw new Error("scope witness dimensions differ")
  if (quality.trilinear !== 0 || quality.anisotropy !== 1) throw new Error("scope pixel oracle requires the configured bilinear sampler")
  const g = tf2ScopeGeometry(before.width, before.height)
  const quads = [
    [g.left, g.top, g.middleX - g.left, g.middleY - g.top, 1, 1],
    [g.middleX - 1, g.top, g.right - g.middleX + 1, g.middleY - g.top + 1, -1, 1],
    [g.middleX, g.middleY, g.right - g.middleX, g.bottom - g.middleY, -1, -1],
    [g.left, g.middleY, g.middleX - g.left, g.bottom - g.middleY, 1, -1],
  ] as const
  const textures = Object.fromEntries((["normal", "tint"] as const).map(role => {
    const source = tf2AuthoredScope[role]
    return [role, source.mips.map((mip, level) => source.encoding === "png" ? decodeScreenshot(Buffer.from(mip.data.split(",")[1]!, "base64")) : {
      width: Math.max(1, source.width >> level), height: Math.max(1, source.height >> level), channels: 4, pixels: inflateSync(Buffer.from(mip.data, "base64")),
    })]
  }))
  const points = []
  for (const yFraction of [.25, .36, .42, .58, .64, .75]) {
    for (const xFraction of [.28, .34, .42, .46, .54, .56, .66, .72]) {
      const x = Math.floor(before.width * xFraction), y = Math.floor(before.height * yFraction)
      const index = y < g.middleY ? x < g.middleX ? 0 : 1 : x < g.middleX ? 3 : 2
      const [left, top, width, height, sx, sy] = quads[index]!
      const span = 1 - 1 / 256
      const u = .5 + ((.5 / 256 + (x + .5 - left) / width * span) - .5) * sx
      const v = .5 + ((.5 / 256 + (y + .5 - top) / height * span) - .5) * sy
      const lookup = (role: "normal" | "tint") => {
        const source = tf2AuthoredScope[role]
        const offset = source.noLod ? 0 : Math.max(0, quality.picmip)
        const level = Math.max(offset, Math.min(source.mips.length - 1, Math.round(Math.log2(Math.max(source.width * span / width, source.height * span / height)))))
        return sample(textures[role]![level]!, u, v, role === "tint")
      }
      const normal = lookup("normal"), tint = lookup("tint")
      const coordinate = [(x + .5) / before.width, (y + .5) / before.height] as const
      const material = evaluateSourceRefractPixel({
        state: { refractAmount: .1, refractTint: [1, 1, 1], blurAmount: 1, ignoreDepth: true },
        coordinate, normal, tintTexture: [tint[0], tint[1], tint[2]], sample: uv => sample(before, uv[0], uv[1], true),
      })
      const world = sample(before, ...coordinate, true)
      const expected = [0, 1, 2].map(channel => srgb(material.rgba[channel]! * normal[3] + world[channel]! * (1 - normal[3])))
      const offset = (y * after.width + x) * after.channels
      const actual = Array.from(after.pixels.slice(offset, offset + 3))
      points.push({ x, y, quadrant: index, normal, tint, warpedCoordinate: material.warpedCoordinate, normalAlpha: normal[3], expected, actual, difference: Math.max(...expected.map((value, channel) => Math.abs(value - actual[channel]!))) })
    }
  }
  return { maximumDifference: Math.max(...points.map(point => point.difference)), points }
}
