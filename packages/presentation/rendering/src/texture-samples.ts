export function sourceTextureSamples(
  source: Uint8Array,
  format: number | null,
  encoding: "u8" | "f16",
): Uint8Array | Uint16Array {
  if (format === 2 || format === 3) {
    if (encoding !== "u8" || source.byteLength % 3 !== 0) {
      throw new Error("authored three-channel texture samples are invalid")
    }
    const rgba = new Uint8Array(source.byteLength / 3 * 4)
    const reversed = format === 3
    for (let input = 0, output = 0; input < source.byteLength; input += 3, output += 4) {
      rgba[output] = source[input + (reversed ? 2 : 0)]!
      rgba[output + 1] = source[input + 1]!
      rgba[output + 2] = source[input + (reversed ? 0 : 2)]!
      rgba[output + 3] = 255
    }
    return rgba
  }
  if (encoding === "u8") return source
  if (source.byteLength % Uint16Array.BYTES_PER_ELEMENT !== 0) {
    throw new Error("authored half-float texture samples are invalid")
  }
  return source.byteOffset % Uint16Array.BYTES_PER_ELEMENT === 0
    ? new Uint16Array(source.buffer, source.byteOffset, source.byteLength / Uint16Array.BYTES_PER_ELEMENT)
    : new Uint16Array(source.slice().buffer)
}
