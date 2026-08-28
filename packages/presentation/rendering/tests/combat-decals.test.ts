import {expect,test} from "bun:test"
import * as THREE from "three/webgpu"
import {CombatDecals,type CombatDecalInput} from "../src/combat-decals"

function decal(identity:number):CombatDecalInput{return {identity,face:identity,reference:`impact:${identity}`,positions:new Float32Array(9),normals:new Float32Array(9),uv:new Float32Array(6),indices:new Uint32Array([0,1,2])}}

test("impacts share one authored shader and expiration frees only their geometry",()=>{
  const group=new THREE.Group(),material=new THREE.MeshBasicNodeMaterial();let created=0,disposed=0,geometries=0
  material.addEventListener("dispose",()=>disposed++)
  const decals=new CombatDecals(group,"materials/decals/decals_mod2x.vmt",()=>{created++;return material})
  decals.update([decal(1),decal(2)],2)
  const first=decals.records.get(1)!.mesh;first.geometry.addEventListener("dispose",()=>geometries++)
  expect(decals.records.get(2)!.mesh.material).toBe(first.material)
  decals.update([decal(2),decal(3)],2)
  expect(created).toBe(1);expect(disposed).toBe(0);expect(geometries).toBe(1)
  expect([...decals.records.keys()]).toEqual([2,3]);expect(group.children).toHaveLength(2)
  expect(decals.records.get(3)!.mesh.userData.materialIdentity).toBe("materials/decals/decals_mod2x.vmt")
  decals.dispose();decals.dispose();expect(disposed).toBe(1);expect(group.children).toHaveLength(0)
})

test("zero admission and duplicate events do not allocate a new material",()=>{
  let created=0;const decals=new CombatDecals(new THREE.Group(),"authored",()=>{created++;return new THREE.MeshBasicNodeMaterial()})
  decals.update([decal(1)],0);expect(created).toBe(0)
  decals.update([decal(1),decal(1)],2);expect(created).toBe(1);expect(decals.records.size).toBe(1)
  decals.dispose()
})
