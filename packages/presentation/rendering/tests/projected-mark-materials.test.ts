import {test,expect} from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import {ProjectedMarkMaterials} from "../src/projected-mark-materials"

test("projected fragments share shader graphs but preserve opaque material ordering",()=>{
  const calls:string[]=[];let disposed=0
  const materials=new ProjectedMarkMaterials(identity=>{
    calls.push(identity);const material=new THREE.MeshBasicNodeMaterial({transparent:identity==="blended"});material.colorNode=TSL.vec4(1,1,1,1)
    material.addEventListener("dispose",()=>disposed++);return material
  })
  const a=materials.fragment("opaque-a"),b=materials.fragment("opaque-b"),c=materials.fragment("opaque-a")
  expect(a.id).toBeLessThan(b.id);expect(b.id).toBeLessThan(c.id)
  expect(a).not.toBe(c);expect(a.colorNode).toBe(c.colorNode);expect(a.customProgramCacheKey()).toBe(c.customProgramCacheKey())
  const blended=materials.fragment("blended");expect(materials.fragment("blended")).toBe(blended)
  expect(calls).toEqual(["opaque-a","opaque-b","blended"])
  let clones=0;for(const material of [a,b,c])material.addEventListener("dispose",()=>clones++)
  materials.dispose();materials.dispose();expect(disposed).toBe(3);expect(clones).toBe(3)
})
