import {expect,test} from "bun:test"
import {authoredMipUpload} from "../src/source-texture-layout"

test("quality-selected 8x2 cable normal mips use exact native decoded planes without padding or LOD changes",()=>{
  const planes=[{width:16,height:4,rgba:new Uint8Array(32)},...[[8,2],[4,1],[2,1],[1,1]].map(([width,height])=>({width:width!,height:height!,rgba:new Uint8Array(width!>4?16:8),decodedRgba:new Uint8Array(width!*height!*4).fill(width!)}))]
  const full=authoredMipUpload(13,"u8",planes);expect(full.layout.compressed).not.toBeNull();expect(full.mipmaps[0]!.data).toBe(planes[0]!.rgba)
  for(let lod=1;lod<planes.length;lod++){
    const selected=authoredMipUpload(13,"u8",planes.slice(lod));expect(selected.layout.compressed).toBeNull();expect(selected.mipmaps).toHaveLength(planes.length-lod)
    expect(selected.mipmaps[0]!.width).toBe(planes[lod]!.width);expect(selected.mipmaps[0]!.height).toBe(planes[lod]!.height)
    expect(selected.mipmaps[0]!.data).toBe(planes[lod]!.decodedRgba)
  }
  expect(()=>authoredMipUpload(13,"u8",[{width:8,height:2,rgba:new Uint8Array(16)}])).toThrow("decoded")
})
