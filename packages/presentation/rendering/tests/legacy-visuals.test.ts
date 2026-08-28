import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { LegacyVisuals,PixelFeedbackLedger,type LegacyVisualFrame } from "../src/legacy-visuals"

test("a newly submitted proxy cannot overwrite an unread completed sample count",()=>{
  const ledger=new PixelFeedbackLedger()
  const pending={source:6,submission:1,visible:-1,possible:-1,clipFraction:1}
  ledger.submit(pending)
  expect(ledger.consume()).toEqual([pending])
  const complete={...pending,visible:17,possible:32}
  ledger.complete(complete)
  const next={...pending,submission:2,clipFraction:0.75}
  ledger.submit(next)
  // The real call order is GPU completion -> render the next frame -> prepare
  // native visibility. Losing the completed record here starves the fader.
  expect(ledger.snapshot()).toEqual([complete])
  expect(ledger.consume()).toEqual([complete])
  expect(ledger.consume()).toEqual([next])
  ledger.complete({...next,visible:0,possible:28})
  expect(ledger.consume()[0]).toMatchObject({submission:2,visible:0,possible:28,clipFraction:0.75})
  ledger.clear()
  expect(ledger.consume()).toEqual([])
})

test("native world sprites, ignore-depth sprites and overlays retain separate draw owners",()=>{
  const original=()=>{},backend={device:{},finishRender:original}
  const first=new THREE.MeshBasicMaterial(),second=new THREE.MeshBasicMaterial()
  const pool=new LegacyVisuals(backend,[[first,second]])
  const quad=(source:number,layer:0|1|2,frame=0)=>({source,layer,material:0,frame,hdrScale:1,origin:new Float32Array([10,20,30]),positions:new Float32Array([9,19,30,9,21,30,11,21,30,11,19,30]),uv:new Float32Array([0,1,0,0,1,0,1,1]),color:new Float32Array(16).fill(1)})
  const input:LegacyVisualFrame={proxies:[],quads:[quad(1,0),quad(2,1),quad(3,2)]}
  pool.update(input)
  expect(pool.world.children).toHaveLength(1);expect(pool.noDepthClip.children).toHaveLength(1);expect(pool.group.children).toHaveLength(1)
  const mesh=pool.world.children[0] as THREE.Mesh
  expect(mesh.geometry.boundingSphere!.center.toArray()).toEqual([10,20,30])
  pool.update({proxies:[],quads:[quad(1,0,1)]})
  expect(mesh.material).toBe(second);expect(pool.noDepthClip.children[0]!.visible).toBe(false)
  pool.dispose();expect(backend.finishRender).toBe(original)
  first.dispose();second.dispose()
})

test("legacy pipelines compile every authored frame for world and late-pass contexts before use",async()=>{
  const backend={device:{},finishRender:()=>{}},materials=[new THREE.MeshBasicMaterial(),new THREE.MeshBasicMaterial()]
  const pool=new LegacyVisuals(backend,[materials]),world=new THREE.Scene(),camera=new THREE.PerspectiveCamera(),scenes:THREE.Scene[]=[]
  const renderer={getRenderTarget:()=>null,setRenderTarget:()=>{},compileAsync:async(group:THREE.Group,_camera:THREE.Camera,scene:THREE.Scene)=>{
    expect(group.children).toHaveLength(2)
    const mesh=group.children[0] as THREE.Mesh
    expect(Object.keys(mesh.geometry.attributes).sort()).toEqual(["legacyColor","legacyFog","legacyHdr","position","uv"])
    scenes.push(scene)
  }}
  await pool.prepareMaterials(renderer as unknown as THREE.WebGPURenderer,camera,world)
  expect(scenes).toEqual([world,pool.noDepth,pool.group]);pool.dispose();materials.forEach(material=>material.dispose())
})
