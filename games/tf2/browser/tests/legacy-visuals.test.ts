import { expect,test } from "bun:test"
import { encodeLegacyVisualQuery,decodeLegacyVisualViews,type LegacyVisualView } from "../src/legacy-visuals"

const view:LegacyVisualView={kind:0,position:[1,2,3],yawDegrees:90,pitchDegrees:0,verticalFovDegrees:60,aspectRatio:16/9,near:7,far:30000,presentationTimeSeconds:2,viewportHeight:720,
  pixelVisibility:[{source:31,submission:7,visible:-1,possible:-1,clipFraction:0.5}]}
const quality={screenWidth:1280,samples:1 as const}

test("PLVQ transports camera and exact pending raster feedback without another clock",()=>{
  const bytes=encodeLegacyVisualQuery([view],quality),data=new DataView(bytes.buffer)
  expect(bytes.length).toBe(104)
  expect(new TextDecoder().decode(bytes.subarray(0,4))).toBe("PLVQ")
  expect(data.getUint32(8,true)).toBe(1)
  expect(data.getUint32(12,true)).toBe(1280);expect(data.getUint32(16,true)).toBe(1)
  expect(data.getUint32(28,true)).toBe(720)
  expect(data.getUint32(84,true)).toBe(31)
  expect(data.getUint32(88,true)).toBe(7)
  expect(data.getInt32(92,true)).toBe(-1)
  expect(data.getFloat32(100,true)).toBe(0.5)
  expect(()=>encodeLegacyVisualQuery([{...view,viewportHeight:0}],quality)).toThrow()
  expect(()=>encodeLegacyVisualQuery([{...view,position:[NaN,0,0]}],quality)).toThrow()
})

test("PLVF render views decode owned native proxy, frame, HDR and UV data",()=>{
  const bytes=new Uint8Array(20+20+88+176),data=new DataView(bytes.buffer)
  const magic=new TextEncoder().encode("PLVF")
  bytes.set(magic);data.setUint32(4,5,true);data.setUint32(8,1,true);data.setUint32(12,0,true);data.setUint32(16,bytes.length-20,true)
  bytes.set(magic,20);data.setUint32(24,8,true);data.setUint32(28,1,true);data.setUint32(32,1,true)
  data.setUint32(40,31,true);data.setFloat32(44,0.5,true)
  for(let i=0;i<20;i++)data.setFloat32(48+i*4,i/20,true)
  data.setUint32(128,31,true);data.setUint32(132,2,true);data.setUint32(136,3,true);data.setUint32(140,1,true);data.setFloat32(144,0.75,true)
  for(let i=0;i<39;i++)data.setFloat32(148+i*4,i/39,true)
  const result=decodeLegacyVisualViews(bytes)
  expect(result[0]!.proxies[0]).toMatchObject({source:31,clipFraction:0.5})
  expect(result[0]!.quads[0]).toMatchObject({source:31,material:2,frame:3,hdrScale:0.75})
  expect(result[0]!.quads[0]!.uv).toHaveLength(8)
  const retained=result[0]!.quads[0]!.positions.slice()
  data.setFloat32(160,10,true)
  expect(result[0]!.quads[0]!.positions).toEqual(retained)
  expect(()=>decodeLegacyVisualViews(bytes.subarray(0,bytes.length-1))).toThrow()
  data.setFloat32(144,NaN,true)
  expect(()=>decodeLegacyVisualViews(bytes)).toThrow()
})

test("legacy water views retain render order and cannot alias view identities",()=>{
  const bytes=encodeLegacyVisualQuery([{...view,kind:1},{...view,kind:2,viewportHeight:512},{...view,kind:3},{...view,kind:0}],quality)
  const data=new DataView(bytes.buffer)
  expect(data.getUint32(8,true)).toBe(4)
  expect([0,1,2,3].map(index=>data.getUint32(20+index*84,true))).toEqual([1,2,3,0])
  expect(()=>encodeLegacyVisualQuery([view,{...view,kind:0}],quality)).toThrow()
})

test("native cable mesh records retain normalized vertex bytes and reject bad index/range bounds",()=>{
  const bytes=new Uint8Array(20+20+16+4+4*24+6*4),data=new DataView(bytes.buffer),magic=new TextEncoder().encode("PLVF")
  bytes.set(magic);data.setUint32(4,5,true);data.setUint32(8,1,true);data.setUint32(16,bytes.length-20,true)
  bytes.set(magic,20);data.setUint32(24,8,true);data.setUint32(36,1,true)
  data.setUint32(40,2,true);data.setUint32(44,1,true);data.setUint32(48,4,true);data.setUint32(52,6,true);data.setUint32(56,101,true)
  for(let vertex=0;vertex<4;vertex++){data.setFloat32(60+vertex*24,vertex,true);bytes.set([10,20,30,255],80+vertex*24)}
  ;[0,1,2,1,3,2].forEach((value,index)=>data.setUint32(156+index*4,value,true))
  const result=decodeLegacyVisualViews(bytes)[0]!.meshes[0]!
  expect(result.material).toBe(2);expect([...result.sources]).toEqual([101]);expect(result.positions).toHaveLength(12);expect([...result.color.subarray(0,4)]).toEqual([10,20,30,255])
  data.setUint32(156,4,true);expect(()=>decodeLegacyVisualViews(bytes)).toThrow("index")
  data.setUint32(48,262144,true);expect(()=>decodeLegacyVisualViews(bytes)).toThrow("record range")
})
