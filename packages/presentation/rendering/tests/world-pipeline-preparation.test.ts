import {test,expect} from "bun:test"
import * as THREE from "three/webgpu"
import {createWorldClipGroup,prepareWorldViewPipelines} from "../src/world-pipeline-preparation"

test("world shaders retain one plane slot across main and actual water attachments",async()=>{
  const root=new THREE.ClippingGroup(),camera=new THREE.PerspectiveCamera(),scene=new THREE.Scene()
  const main=new THREE.RenderTarget(),reflection=new THREE.RenderTarget(),refraction=new THREE.RenderTarget()
  root.enabled=false;let target:THREE.RenderTarget|null=main
  const calls:unknown[]=[]
  const renderer={getRenderTarget:()=>target,setRenderTarget:(value:THREE.RenderTarget|null)=>{target=value},compileAsync:async(object:THREE.Object3D)=>{expect(object).toBe(root);calls.push([root.enabled,target])}}
  await prepareWorldViewPipelines(renderer as unknown as THREE.WebGPURenderer,root,camera,scene,root,[reflection,refraction])
  expect(calls).toEqual([[true,main],[true,reflection],[true,refraction]])
  expect(target).toBe(main);expect(root.enabled).toBe(false)
  main.dispose();reflection.dispose();refraction.dispose()
})

test("the dormant world clip plane rejects no finite fragment after camera transforms",()=>{
  const group=createWorldClipGroup(),other=createWorldClipGroup()
  expect(group.enabled).toBe(true);expect(group.clippingPlanes).toHaveLength(1)
  const dormant=group.clippingPlanes[0]!
  expect(dormant).not.toBe(other.clippingPlanes[0])
  const matrix=new THREE.Matrix4().makeRotationX(0.4).setPosition(1200,-3300,200)
  dormant.applyMatrix4(matrix)
  for(const point of [new THREE.Vector3(0,0,0),new THREE.Vector3(-10000,20000,30000)])expect(dormant.normal.dot(point)>-dormant.constant).toBe(false)
})

test("failed water preparation restores clipping and target; dry maps do not add variants",async()=>{
  const root=new THREE.ClippingGroup(),camera=new THREE.PerspectiveCamera(),scene=new THREE.Scene(),water=new THREE.RenderTarget()
  let target:THREE.RenderTarget|null=null,calls=0;root.enabled=true
  const renderer={getRenderTarget:()=>target,setRenderTarget:(value:THREE.RenderTarget|null)=>{target=value},compileAsync:async()=>{calls++;if(target===water)throw Error("pipeline failure")}}
  await expect(prepareWorldViewPipelines(renderer as unknown as THREE.WebGPURenderer,root,camera,scene,root,[water])).rejects.toThrow("pipeline failure")
  expect(root.enabled).toBe(true);expect(target).toBeNull()
  calls=0;await prepareWorldViewPipelines(renderer as unknown as THREE.WebGPURenderer,root,camera,scene,root,[])
  expect(calls).toBe(1);expect(root.enabled).toBe(true);water.dispose()
})
