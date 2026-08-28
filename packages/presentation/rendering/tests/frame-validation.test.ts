import { expect, test } from "bun:test"
import type { Frame } from "../src/index"
import { invalidFrameEnvelope } from "../src/frame-validation"

const frame = (): Frame => ({effects:[],camera:{position:[-2592,-1680,65.03125095367432],yawDegrees:315,pitchDegrees:0,verticalFovDegrees:59.84044400898544,near:7,far:28377.919921875},deltaSeconds:0.015})
test("frame admission identifies the rejected scalar rather than losing nonfinite values in generic text",()=>{
  expect(invalidFrameEnvelope(frame(),4096)).toBeUndefined()
  expect(invalidFrameEnvelope({...frame(),deltaSeconds:NaN},4096)).toEqual({field:"deltaSeconds",value:"NaN",requirement:"finite"})
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,position:[NaN,0,0]}},4096)?.field).toBe("camera.position[0]")
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,far:7}},4096)?.field).toBe("camera.far")
  expect(invalidFrameEnvelope({...frame(),camera:{...frame().camera,verticalFovDegrees:180}},4096)?.field).toBe("camera.verticalFovDegrees")
  expect(invalidFrameEnvelope({...frame(),deltaSeconds:-0.015},4096)?.field).toBe("deltaSeconds")
  expect(invalidFrameEnvelope({...frame(),exposureHistogram:new Uint32Array(15)},4096)?.field).toBe("exposureHistogram.length")
  expect(invalidFrameEnvelope({...frame(),effects:Array(4097)},4096)?.field).toBe("items.total")
})
