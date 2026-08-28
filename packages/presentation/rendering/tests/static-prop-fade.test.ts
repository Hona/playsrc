import { describe, expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { createStaticPropFadeVariant, selectStaticPropFadePass, distanceFadeOpacity, quantizeStaticPropOpacity, screenFadeOpacity } from "../src/static-prop-fade"

describe("authored Source static-prop fade boundaries", () => {
  test("a fully opaque console writes depth while another shared occurrence fades",()=>{
    const authored=new THREE.MeshBasicNodeMaterial({transparent:false,depthTest:true,depthWrite:true,blending:THREE.NoBlending})
    const faded=createStaticPropFadeVariant(authored)
    const near={mesh:new THREE.Mesh(),authored,faded},far={mesh:new THREE.Mesh(),authored,faded}
    selectStaticPropFadePass([near],1);selectStaticPropFadePass([far],254/255)
    expect(near.mesh.material).toBe(authored);expect(authored.depthWrite).toBe(true);expect(authored.transparent).toBe(false)
    expect(far.mesh.material).toBe(faded);expect(faded.depthTest).toBe(true);expect(faded.depthWrite).toBe(false);expect(faded.blending).toBe(THREE.NormalBlending)
    selectStaticPropFadePass([far],1);expect(far.mesh.material).toBe(authored)
    authored.dispose();faded.dispose()
  })

  test("full opacity does not make an authored translucent or additive material opaque",()=>{
    const authored=new THREE.MeshBasicNodeMaterial({transparent:true,depthWrite:false,blending:THREE.CustomBlending,blendSrc:THREE.SrcAlphaFactor,blendDst:THREE.OneFactor})
    const faded=createStaticPropFadeVariant(authored),binding={mesh:new THREE.Mesh(),authored,faded}
    selectStaticPropFadePass([binding],1);expect((binding.mesh.material as THREE.Material).transparent).toBe(true)
    selectStaticPropFadePass([binding],0.5);expect(faded.blending).toBe(THREE.CustomBlending);expect(faded.blendDst).toBe(THREE.OneFactor)
    authored.dispose();faded.dispose()
  })
  test("preserves squared-distance admission at minimum, between thresholds, and maximum", () => {
    expect(distanceFadeOpacity(0, 10, 20)).toBe(1)
    expect(distanceFadeOpacity(100, 10, 20)).toBe(1)
    expect(distanceFadeOpacity(250, 10, 20)).toBe(0.5)
    expect(distanceFadeOpacity(400, 10, 20)).toBe(0)
    expect(distanceFadeOpacity(401, 10, 20)).toBe(0)
  })

  test("preserves authored screen-width fade and disabled upper thresholds", () => {
    const authoredMinimumDistanceField = 20
    const authoredMaximumDistanceField = 10
    expect(screenFadeOpacity(15, authoredMaximumDistanceField, authoredMinimumDistanceField)).toBe(0.5)
    expect(screenFadeOpacity(9, 10, 20)).toBe(0)
    expect(screenFadeOpacity(10, 10, 20)).toBe(0)
    expect(screenFadeOpacity(15, 10, 20)).toBe(0.5)
    expect(screenFadeOpacity(20, 10, 20)).toBe(1)
    expect(screenFadeOpacity(30, 10, -1)).toBe(1)
  })

  test("retains Source 8-bit truncation and endpoint clamps", () => {
    expect(quantizeStaticPropOpacity(-1)).toBe(0)
    expect(quantizeStaticPropOpacity(0.5)).toBe(127 / 255)
    expect(quantizeStaticPropOpacity(1)).toBe(1)
    expect(quantizeStaticPropOpacity(2)).toBe(1)
  })
})
