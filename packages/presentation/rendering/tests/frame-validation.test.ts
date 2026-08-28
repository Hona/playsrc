import { expect, test } from "bun:test"
import type { Frame } from "../src/index"
import { invalidFrameEnvelope } from "../src/frame-validation"
import { decodeParticleRenderOutput } from "../../particle/src"

const frame = (): Frame => ({effects:[],camera:{position:[-2592,-1680,65.03125095367432],yawDegrees:315,pitchDegrees:0,verticalFovDegrees:59.84044400898544,near:7,far:28377.919921875},deltaSeconds:0.015})
test("frame admission identifies the rejected scalar rather than losing nonfinite values in generic text",()=>{
  expect(invalidFrameEnvelope(frame(),4096)).toBeUndefined()
  expect(invalidFrameEnvelope({...frame(),deltaSeconds:NaN},4096)).toEqual({field:"deltaSeconds",value:"NaN",requirement:"finite"})
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,position:[NaN,0,0]}},4096)?.field).toBe("camera.position[0]")
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,far:7}},4096)?.field).toBe("camera.far")
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,verticalFovDegrees:180}},4096)?.field).toBe("camera.verticalFovDegrees")
  expect(invalidFrameEnvelope({...frame(),deltaSeconds:-0.015},4096)?.field).toBe("deltaSeconds")
  expect(invalidFrameEnvelope({...frame(),exposureHistogram:new Uint32Array(15)},4096)?.field).toBe("exposureHistogram.length")
  expect(invalidFrameEnvelope({...frame(),effects:Array(4097)},4096)?.field).toBe("sceneItems.total")
})

// Authored protocol fixture, not a captured image or a simulated gameplay pass.
// Its records pass the same PSPR decoder used by the Worker consumer.
function particles(count:number) {
  const bytes=new Uint8Array(40+count*436),view=new DataView(bytes.buffer)
  view.setUint32(0,0x5250_5350,true);view.setUint32(4,5,true);view.setUint32(8,count,true)
  for(let index=0;index<count;index++){
    const at=40+index*436
    view.setUint32(at,index+1,true);view.setUint32(at+4,1,true)
    view.setFloat32(at+60,1,true);view.setFloat32(at+72,1,true)
    view.setFloat32(at+92,0.2,true);view.setFloat32(at+400,0.015,true)
    view.setUint32(at+124,1,true)
    bytes[at+394]=2;bytes[at+395]=3
    for(const sheet of [132,196])for(let image=0;image<4;image++){
      view.setFloat32(at+sheet+image*16+8,1,true);view.setFloat32(at+sheet+image*16+12,1,true)
    }
  }
  return decodeParticleRenderOutput(bytes,["particle/blood1/blood_goop3_spray.vmt"]).items
}

test("a decoded particle population is not charged against the scene-object budget",()=>{
  const input:Frame={...frame(),particles:particles(4096),brushModels:{sourceIdentity:1n,registryIdentity:1n,tick:6742n,entityRevision:15027n,collisionRevision:1n,
    models:[{sourceIndex:1,model:1,worldPosition:[0,0,0],worldAngles:[0,0,0],renderMode:0,color:[255,255,255,255],renderFx:0,effects:0,draw:true,mover:null}]}}
  // Minimal cross-domain overflow: remove this brush or one particle and the
  // former aggregate 4096 guard passes. Both producers independently admit it.
  expect(input.particles).toHaveLength(4096)
  expect(invalidFrameEnvelope(input,4096)).toBeUndefined()
  expect(invalidFrameEnvelope({...input,particles:particles(4099)},4096)).toBeUndefined()
  // Configured Viaduct's observed envelope: 4078 snow particles, 39 models,
  // 22 brushes. This tests counts only, not model payload admission or pixels.
  expect(invalidFrameEnvelope({...input,particles:particles(4078),models:Array(39),brushModels:{...input.brushModels!,models:Array(22)}},4096)).toBeUndefined()
})

test("independent budgets still reject overflow and retired/replacement frames do not inherit population state",()=>{
  const admitted={...frame(),particles:particles(4099)}
  expect(invalidFrameEnvelope(admitted,4096)).toBeUndefined()
  expect(invalidFrameEnvelope({...admitted,effects:Array(4097)},4096)?.field).toBe("sceneItems.total")
  expect(invalidFrameEnvelope({...frame(),particles:Array(65537)},4096)?.field).toBe("particles.length")
  expect(invalidFrameEnvelope({...admitted,camera:{...frame().camera,position:[NaN,0,0]}},4096)?.field).toBe("camera.position[0]")
  expect(invalidFrameEnvelope(frame(),4096)).toBeUndefined()
  expect(invalidFrameEnvelope(admitted,4096)).toBeUndefined()
  const malformed=new Uint8Array(40),view=new DataView(malformed.buffer)
  view.setUint32(0,0x5250_5350,true);view.setUint32(4,5,true);view.setUint32(8,65537,true)
  expect(()=>decodeParticleRenderOutput(malformed,["particle/blood1/blood_goop3_spray.vmt"])).toThrow("particle render item count exceeds its limit")
})
