import { expect,test } from "bun:test"
import { encodeLegacyVisualQuery,decodeLegacyVisualViews,type LegacyVisualView } from "../src/legacy-visuals"

const view:LegacyVisualView={kind:0,position:[1,2,3],yawDegrees:90,pitchDegrees:0,verticalFovDegrees:60,aspectRatio:16/9,near:7,far:30000,presentationTimeSeconds:2,viewportHeight:720,
  pixelVisibility:[{source:31,submission:7,visible:-1,possible:-1,clipFraction:0.5}]}

test("PLVQ transports camera and exact pending raster feedback without another clock",()=>{
  const bytes=encodeLegacyVisualQuery([view]),data=new DataView(bytes.buffer)
  expect(bytes.length).toBe(96)
  expect(new TextDecoder().decode(bytes.subarray(0,4))).toBe("PLVQ")
  expect(data.getUint32(8,true)).toBe(1)
  expect(data.getUint32(20,true)).toBe(720)
  expect(data.getUint32(76,true)).toBe(31)
  expect(data.getUint32(80,true)).toBe(7)
  expect(data.getInt32(84,true)).toBe(-1)
  expect(data.getFloat32(92,true)).toBe(0.5)
  expect(()=>encodeLegacyVisualQuery([{...view,viewportHeight:0}])).toThrow()
  expect(()=>encodeLegacyVisualQuery([{...view,position:[NaN,0,0]}])).toThrow()
})

test("PLVF render views decode owned native proxy, frame, HDR and UV data",()=>{
  const bytes=new Uint8Array(20+16+88+176),data=new DataView(bytes.buffer)
  const magic=new TextEncoder().encode("PLVF")
  bytes.set(magic);data.setUint32(4,5,true);data.setUint32(8,1,true);data.setUint32(12,0,true);data.setUint32(16,bytes.length-20,true)
  bytes.set(magic,20);data.setUint32(24,7,true);data.setUint32(28,1,true);data.setUint32(32,1,true)
  data.setUint32(36,31,true);data.setFloat32(40,0.5,true)
  for(let i=0;i<20;i++)data.setFloat32(44+i*4,i/20,true)
  data.setUint32(124,31,true);data.setUint32(128,2,true);data.setUint32(132,3,true);data.setUint32(136,1,true);data.setFloat32(140,0.75,true)
  for(let i=0;i<39;i++)data.setFloat32(144+i*4,i/39,true)
  const result=decodeLegacyVisualViews(bytes)
  expect(result[0]!.proxies[0]).toMatchObject({source:31,clipFraction:0.5})
  expect(result[0]!.quads[0]).toMatchObject({source:31,material:2,frame:3,hdrScale:0.75})
  expect(result[0]!.quads[0]!.uv).toHaveLength(8)
  const retained=result[0]!.quads[0]!.positions.slice()
  data.setFloat32(156,10,true)
  expect(result[0]!.quads[0]!.positions).toEqual(retained)
  expect(()=>decodeLegacyVisualViews(bytes.subarray(0,bytes.length-1))).toThrow()
  data.setFloat32(140,NaN,true)
  expect(()=>decodeLegacyVisualViews(bytes)).toThrow()
})

test("legacy water views retain render order and cannot alias view identities",()=>{
  const bytes=encodeLegacyVisualQuery([{...view,kind:1},{...view,kind:2,viewportHeight:512},{...view,kind:3},{...view,kind:0}])
  const data=new DataView(bytes.buffer)
  expect(data.getUint32(8,true)).toBe(4)
  expect([0,1,2,3].map(index=>data.getUint32(12+index*84,true))).toEqual([1,2,3,0])
  expect(()=>encodeLegacyVisualQuery([view,{...view,kind:0}])).toThrow()
})
