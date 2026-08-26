const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, index) => {
  let value = index
  for (let bit = 0; bit < 8; bit += 1) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1
  return value >>> 0
})

function chunk(name: string, data: Uint8Array): Uint8Array {
  const output = new Uint8Array(data.length + 12)
  const view = new DataView(output.buffer)
  view.setUint32(0, data.length)
  for (let index = 0; index < 4; index += 1) output[index + 4] = name.charCodeAt(index)
  output.set(data, 8)
  let crc = 0xffffffff
  for (let index = 4; index < output.length - 4; index += 1) crc = CRC_TABLE[(crc ^ output[index]!) & 255]! ^ (crc >>> 8)
  view.setUint32(output.length - 4, (crc ^ 0xffffffff) >>> 0)
  return output
}

// Encode the authored straight-alpha bytes directly. Canvas.toDataURL first
// unpremultiplies its backing store, which loses low-alpha RGB precision.
export async function encodeVguiRasterPng(width: number, height: number, rgba: Uint8ClampedArray): Promise<Uint8Array> {
  if (!Number.isSafeInteger(width) || width < 1 || !Number.isSafeInteger(height) || height < 1 || rgba.length !== width * height * 4) {
    throw new Error("VGUI raster PNG dimensions differ from its pixels")
  }
  const stride = width * 4
  const scanlines = new Uint8Array((stride + 1) * height)
  for (let row = 0; row < height; row += 1) scanlines.set(rgba.subarray(row * stride, (row + 1) * stride), row * (stride + 1) + 1)
  const compressed = new Uint8Array(await new Response(new Blob([scanlines]).stream().pipeThrough(new CompressionStream("deflate"))).arrayBuffer())
  const header = new Uint8Array(13)
  const dimensions = new DataView(header.buffer)
  dimensions.setUint32(0, width)
  dimensions.setUint32(4, height)
  header[8] = 8
  header[9] = 6
  const chunks = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("sRGB", new Uint8Array([0])), chunk("IDAT", compressed), chunk("IEND", new Uint8Array())]
  const result = new Uint8Array(chunks.reduce((total, item) => total + item.length, 0))
  let offset = 0
  for (const item of chunks) { result.set(item, offset); offset += item.length }
  return result
}

export async function vguiRasterDataUrl(width: number, height: number, rgba: Uint8ClampedArray): Promise<string> {
  const bytes = await encodeVguiRasterPng(width, height, rgba)
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(new Blob([bytes], { type: "image/png" }))
  })
}
