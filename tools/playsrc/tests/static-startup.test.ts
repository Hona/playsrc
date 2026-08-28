import { expect, test } from "bun:test"
import { mkdtemp, mkdir, writeFile, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { deflateSync } from "node:zlib"
import releaseJson from "../../../apps/web/tf2/releases/current.json"
import { createDeployedBrowserConfiguration, parseTf2Release } from "../../../apps/web/tf2/src/deployment"
import { staticStartupPackage, staticStartupRouter, startupDigest } from "../profile/static-startup-package"
import { captureStaticStartup, staticStartupReceipt, assertStaticStartupReceipt, startupPixelEvidence, type StartupObservation } from "../profile/static-startup-gate"
import { fetchImmutableObject } from "@playsrc/asset-store/browser"

// Synthetic PNGs are unit inputs only, never retained as browser evidence.
function png(phase: number) {
  const width=64,height=32,raw=Buffer.alloc((width*3+1)*height)
  for(let y=0;y<height;y++)for(let x=0;x<width*3;x++)raw[y*(width*3+1)+x+1]=(x+y+phase)%255+1
  const chunk=(kind:string,data:Buffer)=>{const out=Buffer.alloc(data.length+12);out.writeUInt32BE(data.length);out.write(kind,4);data.copy(out,8);return out}
  const ihdr=Buffer.alloc(13);ihdr.writeUInt32BE(width);ihdr.writeUInt32BE(height,4);ihdr[8]=8;ihdr[9]=2
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',ihdr),chunk('IDAT',deflateSync(raw)),chunk('IEND',Buffer.alloc(0))])
}

test("exact static package catches a page/Worker/WASM mismatch before publication and never rewrites a checked artifact", async()=>{
  const root=await mkdtemp(path.join(tmpdir(),'playsrc-static-startup-'))
  try {
    const current=path.join(root,'current'),previous=path.join(root,'previous')
    await mkdir(path.join(current,'tf2/assets'),{recursive:true});await mkdir(path.join(previous,'tf2/assets'),{recursive:true})
    const wasm=Buffer.from([0,97,115,109,1,0,0,0]),wasmFile=path.join(root,'selected.wasm');await writeFile(wasmFile,wasm)
    const release=parseTf2Release({...releaseJson,objects:{...releaseJson.objects,wasm:{...releaseJson.objects.wasm,byteLength:String(wasm.length),sha256:startupDigest(wasm)}}})
    const configuration=createDeployedBrowserConfiguration(release,'a'.repeat(64))
    const generation={applicationBuild:configuration.applicationBuild,wasmSha256:configuration.wasm.sha256,resourceRoots:Object.fromEntries(configuration.targets.map(t=>[t.target,t.objects.resources.sha256]))}
    const source=`export const generation=${JSON.stringify(generation)};\n/*playsrc-generation:${JSON.stringify(generation)}*/`
    await writeFile(path.join(current,'tf2/assets/index-test.js'),source);await writeFile(path.join(current,'tf2/assets/gameplay-worker-test.js'),source)
    await writeFile(path.join(current,'tf2/index.html'),'<script src="/tf2/assets/index-test.js"></script>')
    await writeFile(path.join(current,'tf2/playsrc-config.json'),JSON.stringify(configuration))
    await writeFile(path.join(current,'release.json'),JSON.stringify({applicationBuild:configuration.applicationBuild,release}))
    await writeFile(path.join(current,'_headers'),'/*\n  Cross-Origin-Embedder-Policy: require-corp\n  Cross-Origin-Opener-Policy: same-origin\n')
    await writeFile(path.join(previous,'tf2/index.html'),'<script src="/tf2/assets/index-previous.js"></script>')
    await writeFile(path.join(previous,'tf2/assets/index-previous.js'),'export const previous=true;')
    const router=await staticStartupRouter({directory:current,previousDirectory:previous,assetDir:root,wasmFile})
    expect((await router.response('https://playsrc.online/tf2'))!.body.toString()).toContain('index-test')
    expect(router.upgradeNavigations).toBe(0)
    router.warmUpgrade()
    expect((await router.response('https://playsrc.online/tf2'))!.body.toString()).toContain('index-previous')
    expect(router.previousEntryUsed).toBe(true)
    expect((await router.response('https://playsrc.online/tf2'))!.body.toString()).toContain('index-test')
    expect(router.upgradeNavigations).toBe(2)
    expect((await router.response(`https://assets.playsrc.online/objects/sha256/${configuration.wasm.sha256}`))!.body).toEqual(wasm)
    const fetched=await fetchImmutableObject(configuration.assetOrigin,configuration.wasm,undefined,(async url=>{
      const served=(await router.response(String(url)))!
      const response=new Response(served.body,{status:served.status,headers:served.headers})
      Object.defineProperty(response,'url',{value:String(url)})
      return response
    }) as typeof fetch)
    expect(fetched).toEqual(new Uint8Array(wasm))
    await router.verifyUnchanged()
    const bad=source.replaceAll(configuration.wasm.sha256,'b'.repeat(64))
    await writeFile(path.join(current,'tf2/assets/gameplay-worker-test.js'),bad)
    await expect(staticStartupPackage(current)).rejects.toThrow('generation differs')
    await expect(router.response('https://playsrc.online/tf2/assets/gameplay-worker-test.js')).rejects.toThrow('changed')
    expect(await readFile(wasmFile)).toEqual(wasm)
  } finally {await rm(root,{recursive:true,force:true})}
})

test("startup gate rejects the real silent-failure shape even with no console exception", async()=>{
  const native=async()=>({at:Date.now(),physical:true,unlocked:true,foreground:true,visible:true,minimized:false,idleMilliseconds:2500,browserPid:1,windowId:2,targetId:'test'})
  await expect(captureStaticStartup({native,navigate:async()=>{},read:async()=>({phase:'Failed',detail:'Application generation upgrade did not converge',visible:true,focused:true,timeOrigin:0,at:0,frame:0,teamSelection:false,classSelection:false,unexpectedInput:0,movie:null}),screenshot:async()=>png(1),action:async()=>{},wait:async()=>{}},'jump_beef')).rejects.toThrow('did not converge')
})

test("receipt requires advancing movie pixels, menu and completed game frames for cold and warm upgrade", async()=>{
  let tick=0,playing=false,image=0,cache="stored"
  const capture=await captureStaticStartup({
    native:async()=>({at:Date.now(),physical:true,unlocked:true,foreground:true,visible:true,minimized:false,idleMilliseconds:2500,browserPid:1,windowId:2,targetId:'test'}),
    navigate:async mode=>{tick=0;playing=false;cache=mode==='cold'?'stored':'hit'},
    read:async()=>({phase:playing?'Ready':tick>3?'MainMenu':'Startup',detail:'fixture',startupState:tick>3?'Completed':'Playing',visible:true,focused:true,timeOrigin:1,at:tick,frame:playing?tick:0,cache,consoleVisible:false,gameUi:playing?'in-game':'main-menu',playerClass:playing?3:0,tick:String(tick+1),teamSelection:false,classSelection:false,unexpectedInput:0,movie:playing?null:{time:4.9+tick*.6,paused:false,muted:false,width:1440,height:1080}} as StartupObservation),
    screenshot:async()=>png(++image),action:async action=>{if(action==='open-map')playing=true},wait:async()=>{tick++;await new Promise(r=>setTimeout(r,1))},
  },'jump_beef')
  const identity={packageSha256:'a'.repeat(64),wasmSha256:'b'.repeat(64),previousPackageSha256:'c'.repeat(64),previousEntryUsed:true,upgradeNavigations:2,bootFailure:{phase:'Failed',visible:true,text:'Unavailable configuration',pixels:startupPixelEvidence(png(20)),native:capture.native[0]!}}
  const receipt=staticStartupReceipt(identity,capture)
  assertStaticStartupReceipt(receipt,identity)
  for(const change of [
    (r:any)=>r.packageSha256='d'.repeat(64),
    (r:any)=>r.previousEntryUsed=false,
    (r:any)=>r.capture.runs[1].movie[1].pixels=r.capture.runs[1].movie[0].pixels,
    (r:any)=>r.capture.runs[0].guard.valid=false,
    (r:any)=>r.capture.runs[0].playable.state.frame=r.capture.runs[0].playable.firstFrame,
    (r:any)=>r.capture.native[4].windowId=3,
    (r:any)=>r.capture.runs[1].playable.state.cache='stored',
    (r:any)=>r.bootFailure.visible=false,
    (r:any)=>r.upgradeNavigations=1,
  ]){const bad=structuredClone(receipt);change(bad);expect(()=>assertStaticStartupReceipt(bad,identity)).toThrow()}
})

test("a particle admission failure after valid movie/menu pixels cannot produce release acceptance", async()=>{
  let tick=0,loading=false,image=0
  const attempt=captureStaticStartup({
    native:async()=>({at:Date.now(),physical:true,unlocked:true,foreground:true,visible:true,minimized:false,idleMilliseconds:2500,browserPid:1,windowId:2,targetId:'test'}),
    navigate:async()=>{},
    read:async()=>({phase:loading?'Failed':tick>3?'MainMenu':'Startup',detail:loading?'CompileFailed:10':'fixture',startupState:tick>3?'Completed':'Playing',visible:true,focused:true,timeOrigin:1,at:tick,frame:0,teamSelection:false,classSelection:false,unexpectedInput:0,movie:loading?null:{time:4.9+tick*.6,paused:false,muted:false,width:1440,height:1080}}),
    screenshot:async()=>png(++image),action:async action=>{if(action==='open-map')loading=true},wait:async()=>{tick++},
  },'pl_upward')
  await expect(attempt).rejects.toThrow('CompileFailed:10')
  const failure=await attempt.catch(error=>error)
  expect(failure.startupEvidence.runs[0].menu.state.phase).toBe('MainMenu')
  expect(failure.startupEvidence.runs[0].playable).toBeUndefined()
  expect(failure.startupEvidence.terminal.detail).toBe('CompileFailed:10')
})
