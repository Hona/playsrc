import * as THREE from "three/webgpu"

/** Storage interpretation shared by actual authored upload and compiler-only
 * acceptance. The latter has no pixel allocation or device submission. */
export function sourceTextureLayout(sourceFormat: number | null, scalarEncoding: "u8" | "f16") {
  const compressed = sourceFormat === 13 || sourceFormat === 20 ? THREE.RGBA_S3TC_DXT1_Format
    : sourceFormat === 14 ? THREE.RGBA_S3TC_DXT3_Format : sourceFormat === 15 ? THREE.RGBA_S3TC_DXT5_Format : null
  if (sourceFormat !== null && ![0, 1, 2, 3, 11, 12, 16, 24].includes(sourceFormat) && compressed === null) return null
  return { compressed, format: compressed ?? THREE.RGBAFormat,
    type: compressed === null && scalarEncoding === "f16" ? THREE.HalfFloatType : THREE.UnsignedByteType }
}
