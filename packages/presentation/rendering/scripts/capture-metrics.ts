#!/usr/bin/env bun

import { createHash } from "node:crypto"
import { inflateSync } from "node:zlib"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"

const REGIONS = Object.freeze([
  Object.freeze({ name: "ceiling", x: 400, y: 120, width: 320, height: 100 }),
  Object.freeze({ name: "forward-wall", x: 400, y: 270, width: 320, height: 180 }),
  Object.freeze({ name: "floor", x: 180, y: 500, width: 160, height: 130 }),
])

function decodePng(bytes: Uint8Array): Readonly<{ width: number; height: number; rgb: Uint8Array }> {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  if (!signature.every((value, index) => bytes[index] === value)) throw new Error("capture PNG signature is invalid")
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  let offset = 8
  let width = 0
  let height = 0
  let channels = 0
  const parts: Uint8Array[] = []
  while (offset + 12 <= bytes.length) {
    const length = view.getUint32(offset)
    const type = new TextDecoder().decode(bytes.subarray(offset + 4, offset + 8))
    const start = offset + 8
    const end = start + length
    if (end + 4 > bytes.length) throw new Error("capture PNG chunk is truncated")
    if (type === "IHDR") {
      if (length !== 13 || bytes[start + 8] !== 8) throw new Error("capture PNG header is invalid")
      width = view.getUint32(start)
      height = view.getUint32(start + 4)
      channels = bytes[start + 9] === 2 ? 3 : bytes[start + 9] === 6 ? 4 : 0
    } else if (type === "IDAT") {
      parts.push(bytes.subarray(start, end))
    } else if (type === "IEND") {
      break
    }
    offset = end + 4
  }
  if (width < 1 || height < 1 || width > 4096 || height > 4096 || channels === 0) {
    throw new Error("capture PNG dimensions or color type are invalid")
  }
  const inflated = inflateSync(Buffer.concat(parts))
  const stride = width * channels
  if (inflated.length !== height * (stride + 1)) throw new Error("capture PNG scanline length differs")
  const samples = new Uint8Array(height * stride)
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[y * (stride + 1)]!
    if (filter > 4) throw new Error("capture PNG filter is invalid")
    for (let x = 0; x < stride; x += 1) {
      const at = y * stride + x
      const left = x >= channels ? samples[at - channels]! : 0
      const above = y > 0 ? samples[at - stride]! : 0
      const upperLeft = y > 0 && x >= channels ? samples[at - stride - channels]! : 0
      const estimate = left + above - upperLeft
      const leftDistance = Math.abs(estimate - left)
      const aboveDistance = Math.abs(estimate - above)
      const upperLeftDistance = Math.abs(estimate - upperLeft)
      const paeth = leftDistance <= aboveDistance && leftDistance <= upperLeftDistance ? left
        : aboveDistance <= upperLeftDistance ? above
        : upperLeft
      const predictor = filter === 0 ? 0 : filter === 1 ? left : filter === 2 ? above
        : filter === 3 ? Math.floor((left + above) / 2)
        : paeth
      samples[at] = (inflated[y * (stride + 1) + 1 + x]! + predictor) & 0xff
    }
  }
  const rgb = new Uint8Array(width * height * 3)
  for (let source = 0, target = 0; source < samples.length; source += channels, target += 3) {
    rgb[target] = samples[source]!
    rgb[target + 1] = samples[source + 1]!
    rgb[target + 2] = samples[source + 2]!
  }
  return { width, height, rgb }
}

function mainRegion(image: ReturnType<typeof decodePng>, region: typeof REGIONS[number]) {
  if (region.x + region.width > image.width || region.y + region.height > image.height) {
    throw new Error(`capture region ${region.name} exceeds the image`)
  }
  const output = new Uint8Array(region.width * region.height * 3)
  let at = 0
  let warmParticlePixels = 0
  let luma = 0
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      const index = (y * image.width + x) * 3
      const red = image.rgb[index]!
      const green = image.rgb[index + 1]!
      const blue = image.rgb[index + 2]!
      output[at++] = red
      output[at++] = green
      output[at++] = blue
      if (red >= 220 && green >= 140 && blue <= 100) warmParticlePixels += 1
      luma += red * 0.2126 + green * 0.7152 + blue * 0.0722
    }
  }
  return Object.freeze({
    ...region,
    sha256: createHash("sha256").update(output).digest("hex"),
    warmParticlePixels,
    meanLuma: Number((luma / (region.width * region.height)).toFixed(3)),
  })
}

const identity = process.argv[2]
if (process.argv.length !== 3 || !identity || !/^[0-9a-f]{64}$/u.test(identity)) {
  throw new Error("Usage: bun packages/presentation/rendering/scripts/capture-metrics.ts <capture-sha256>")
}
const configuration = await loadLocalConfig()
const capture = path.join(configuration.sourceCacheDir, "evidence", "browser", "jump_beef", `${identity}.png`)
const bytes = await readFile(capture)
if (createHash("sha256").update(bytes).digest("hex") !== identity) throw new Error("capture SHA-256 differs")
const image = decodePng(bytes)
const report = Object.freeze({
  schema: "playsrc-rendering-headed-capture-v1",
  target: "jump_beef",
  capture,
  sha256: identity,
  byteLength: bytes.length,
  width: image.width,
  height: image.height,
  regions: REGIONS.map((region) => mainRegion(image, region)),
})
const directory = path.join(configuration.sourceCacheDir, "evidence", "tf2-browser-performance")
await mkdir(directory, { recursive: true })
const output = path.join(directory, `renderer-capture-${identity}.json`)
await writeFile(output, `${JSON.stringify(report, null, 2)}\n`)
console.log(JSON.stringify({ output, ...report }, null, 2))
