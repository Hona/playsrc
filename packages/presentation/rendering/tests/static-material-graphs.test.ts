import { expect, test } from "bun:test"
import { createStaticCompilerParityOwner, THREE, TSL } from "./fixtures/static-compiler-parity"

test("same-plane static templates reuse compiler state without merging another texture or draw fade", () => {
  const owner=createStaticCompilerParityOwner(),geometry=new THREE.BoxGeometry()
  geometry.setAttribute("staticLighting",new THREE.Uint8BufferAttribute(new Uint8Array(geometry.getAttribute("position").count*4).fill(255),4,true))
  const textures=[new THREE.Texture(),new THREE.Texture()],bases=textures.map(texture=>TSL.texture(texture,TSL.uv()))
  const state={alphaModulation:1,blendEnabled:false,alphaOwnership:{opacity:false},fragmentDiscard:{kind:"none"},depthWrite:true,depthTest:true} as any
  for(let index=0;index<3;index++)owner.admit(`opaque${index}`,`template${index}`,`material${index}`,geometry,bases[index%2],state,false,false,THREE.FrontSide)
  for(let index=0;index<3;index++)owner.admit(`fading${index}`,`fade-template${index}`,`material${index}`,geometry,bases[0],state,false,true,THREE.FrontSide)
  const result=owner.finish()
  expect(result.draws).toBe(9)
  expect(result.dedicatedCompilerStates).toBe(9)
  expect(result.sharedCompilerStates).toBe(4)
  expect(result.retiredCompilerStates).toBe(0)
  geometry.dispose();for(const texture of textures)texture.dispose()
})
