import * as THREE from "three/webgpu"
import {sourceTextureSamples} from "./texture-samples"

/** Storage interpretation shared by actual authored upload and compiler-only
 * acceptance. The latter has no pixel allocation or device submission. */
export function sourceTextureLayout(sourceFormat: number | null, scalarEncoding: "u8" | "f16") {
  const compressed = sourceFormat === 13 || sourceFormat === 20 ? THREE.RGBA_S3TC_DXT1_Format
    : sourceFormat === 14 ? THREE.RGBA_S3TC_DXT3_Format : sourceFormat === 15 ? THREE.RGBA_S3TC_DXT5_Format : null
  if (sourceFormat !== null && ![0, 1, 2, 3, 11, 12, 16, 24].includes(sourceFormat) && compressed === null) return null
  return { compressed, format: compressed ?? THREE.RGBAFormat,
    type: compressed === null && scalarEncoding === "f16" ? THREE.HalfFloatType : THREE.UnsignedByteType }
}

/** A block-compressed texture may have sub-block mip levels, but WebGPU cannot
 * allocate one as the top level. Native decoded planes retain that exact LOD. */
export function authoredMipUpload(sourceFormat:number|null,scalarEncoding:"u8"|"f16",planes:readonly Readonly<{width:number;height:number;rgba:Uint8Array;decodedRgba?:Uint8Array}>[]){
  const layout=sourceTextureLayout(sourceFormat,scalarEncoding),base=planes[0]
  if(!layout||!base)throw new Error("Authored texture upload layout is unavailable")
  const decoded=layout.compressed!==null&&(base.width%4!==0||base.height%4!==0)
  const mipmaps=planes.map(plane=>{
    const data=decoded?plane.decodedRgba:sourceTextureSamples(plane.rgba,sourceFormat,scalarEncoding)
    if(!data||decoded&&data.byteLength!==plane.width*plane.height*4)throw new Error("Native decoded compressed mip is unavailable")
    return Object.freeze({data,width:plane.width,height:plane.height})
  })
  return {layout:{...layout,compressed:decoded?null:layout.compressed},mipmaps}
}
