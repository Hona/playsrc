import { inflateSync } from "node:zlib"

export type DecodedScreenshot = Readonly<{ width: number; height: number; channels: number; pixels: Uint8Array }>

export function decodeScreenshot(bytes: Buffer): DecodedScreenshot {
  if (!bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    throw new Error("headed browser screen sample is not PNG")
  }
  let width = 0
  let height = 0
  let colorType = -1
  const chunks: Buffer[] = []
  for (let offset = 8; offset + 12 <= bytes.length;) {
    const length = bytes.readUInt32BE(offset)
    if (length > bytes.length - offset - 12) throw new Error("headed browser PNG chunk is truncated")
    const kind = bytes.subarray(offset + 4, offset + 8).toString("ascii")
    const value = bytes.subarray(offset + 8, offset + 8 + length)
    if (kind === "IHDR") {
      if (value.length !== 13 || value[8] !== 8 || value[10] !== 0 || value[11] !== 0 || value[12] !== 0) {
        throw new Error("headed browser PNG format is unsupported")
      }
      width = value.readUInt32BE(0)
      height = value.readUInt32BE(4)
      colorType = value[9]!
    } else if (kind === "IDAT") chunks.push(value)
    offset += length + 12
  }
  if (width < 1 || height < 1 || ![2, 6].includes(colorType)) {
    throw new Error("headed browser PNG dimensions or color type differ")
  }
  const channels = colorType === 2 ? 3 : 4
  const stride = width * channels
  const values = inflateSync(Buffer.concat(chunks))
  if (values.length !== (stride + 1) * height) throw new Error("headed browser PNG scanline length differs")
  const pixels = new Uint8Array(stride * height)
  for (let row = 0; row < height; row += 1) {
    const source = row * (stride + 1)
    const filter = values[source]!
    if (filter > 4) throw new Error("headed browser PNG scanline filter is invalid")
    for (let column = 0; column < stride; column += 1) {
      const destination = row * stride + column
      const left = column >= channels ? pixels[destination - channels]! : 0
      const above = row > 0 ? pixels[destination - stride]! : 0
      const upperLeft = row > 0 && column >= channels ? pixels[destination - stride - channels]! : 0
      let predictor = 0
      if (filter === 1) predictor = left
      else if (filter === 2) predictor = above
      else if (filter === 3) predictor = Math.floor((left + above) / 2)
      else if (filter === 4) {
        const estimate = left + above - upperLeft
        const leftDistance = Math.abs(estimate - left)
        const aboveDistance = Math.abs(estimate - above)
        const upperLeftDistance = Math.abs(estimate - upperLeft)
        predictor = leftDistance <= aboveDistance && leftDistance <= upperLeftDistance
          ? left
          : aboveDistance <= upperLeftDistance ? above : upperLeft
      }
      pixels[destination] = (values[source + column + 1]! + predictor) & 255
    }
  }
  return Object.freeze({ width, height, channels, pixels })
}
