// Computational WASM acceptance, not a browser or presentation benchmark.
import path from "node:path"
import {readFile,writeFile} from "node:fs/promises"
import {encodeResourceBatch,parseResourceGraph} from "@playsrc/asset-store/graph"
import {decodeSnapshot,encodeCommand,type Command} from "../../../games/tf2/browser/src/codec"
import {loadLocalConfig} from "./config"
import {acquireMap} from "./targets"
import {buildTf2Wasm} from "./tf2-wasm-build"
import {parseGameplayReplay,replayWorkClockBytes} from "../profile/gameplay-replay"

const [target,graphIdentity,at="1",mode]=process.argv.slice(2),addTick=Number(at),selfReplay=mode==="--self-replay"
if(!target||!graphIdentity||!/^[0-9a-f]{64}$/.test(graphIdentity)||!Number.isInteger(addTick)||addTick<1||addTick>4096||mode!==undefined&&!selfReplay)throw new Error("Usage: verify-bot-progression <target> <retained graph sha256> [bot add tick] [--self-replay]")
const config=await loadLocalConfig(),map=await acquireMap(config,target)
const wasmPath=await buildTf2Wasm(config,false),wasm=await readFile(wasmPath)
const graphBytes=await readFile(path.join(config.sourceCacheDir,"browser-bundles/immutable-roots",graphIdentity))
const hash=(bytes:Uint8Array)=>new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
if(hash(graphBytes)!==graphIdentity)throw new Error("Graph identity differs")
const graph=parseResourceGraph(JSON.parse(graphBytes.toString()))
if(graph.target!==target)throw new Error("Graph target differs")
const {instance}=await WebAssembly.instantiate(wasm,{playsrc_metrics:{monotonic_milliseconds:()=>performance.now()}})
const e=instance.exports as Record<string,any>
const copy=(bytes:Uint8Array)=>{const pointer=e.playsrc_alloc(bytes.length)>>>0;new Uint8Array(e.memory.buffer,pointer,bytes.length).set(bytes);return pointer}
const sections:{pointer:number,length:number}[]=[]
for(const descriptor of graph.chunks.filter(chunk=>chunk.roles.includes("gameplay"))){
  const bytes=await readFile(path.join(config.sourceCacheDir,`browser-bundles/${target}.graph/objects`,descriptor.encodedSha256))
  const batch=encodeResourceBatch([{descriptor,bytes}]),pointer=copy(batch)
  if(e.playsrc_resource_decode(pointer,batch.length)!==1)throw new Error("Resource authentication failed")
  e.playsrc_free(pointer,batch.length)
  const length=e.playsrc_resource_length();sections.push({pointer:e.playsrc_resource_take()>>>0,length})
}
const tableBytes=new Uint8Array(sections.length*8),tableView=new DataView(tableBytes.buffer)
sections.forEach((section,i)=>{tableView.setUint32(i*8,section.pointer,true);tableView.setUint32(i*8+4,section.length,true)})
const table=copy(tableBytes),digest=e.playsrc_alloc(32)>>>0
e.playsrc_resource_sections_hash(table,sections.length,digest)
const configurationHash=new Uint8Array(e.memory.buffer,digest,32).slice()
const bsp=await readFile(path.join(config.sourceCacheDir,map.decoded.cachePath))
if(hash(bsp)!==map.decoded.sha256)throw new Error("BSP identity differs")
const source=copy(bsp),handle=e.playsrc_compile_map(source,bsp.length,1,table,sections.length,digest,1)
if(e.playsrc_result_error(handle)!==0)throw new Error(`Map compilation failed: ${e.playsrc_result_error(handle)}`)
e.playsrc_result_release(handle);e.playsrc_presentation_release(handle)
e.playsrc_free(source,bsp.length);e.playsrc_free(table,tableBytes.length);e.playsrc_free(digest,32)
const base:Command={forward:0,side:0,yawDegrees:0,pitchDegrees:0,jump:false,crouch:false,fire:false,detonate:false}
const observe=(owner:number,now:number,command:Uint8Array,suspended=0,acknowledged=0n)=>{
  const pointer=copy(command),success=e.playsrc_simulation_observe(owner,now,pointer,command.length,suspended,acknowledged)
  e.playsrc_free(pointer,command.length)
  if(success!==1)throw new Error(`Simulation observe failed: ${e.playsrc_simulation_error()}`)
}
const snapshotBytes=(owner:number)=>{
  const length=e.playsrc_snapshot_length(owner),output=e.playsrc_alloc(length)>>>0
  if(e.playsrc_snapshot_copy(owner,output,length)!==length)throw new Error("Snapshot copy failed")
  const bytes=new Uint8Array(e.memory.buffer,output,length).slice();e.playsrc_free(output,length);return bytes
}
if(selfReplay){
  if(e.playsrc_gameplay_replay_begin(handle)!==1)throw new Error("Replay capture rejected")
  e.playsrc_gameplay_replay_mark(handle,0)
  observe(handle,0,new Uint8Array(encodeCommand({...base,selectClass:3,selectTeam:2})))
}
const history:any[]=[],started=performance.now()
let advanced=0,won=false,now=0
while(advanced<12000){
  if(performance.now()-started>150000)throw new Error("WASM progression exceeded bounded runtime")
  const command=new Uint8Array(encodeCommand({...base,...advanced===0?{selectClass:3 as const,selectTeam:2 as const}:{},...advanced+1===addTick?{bot:{action:"add" as const,count:15,class:1 as const,team:2 as const,difficulty:1 as const}}:{}}))
  let steps=advanced===0?1:Math.min(selfReplay?4:64,12000-advanced)
  if(advanced+1===addTick)steps=1
  if(advanced+1<addTick)steps=Math.min(steps,addTick-1-advanced)
  const pointer=copy(command)
  const success=selfReplay?e.playsrc_simulation_observe(handle,now+=steps*0.015,pointer,command.length,0,0n):e.playsrc_game_advance(handle,pointer,command.length,steps)
  if(success!==1){
    const length=e.playsrc_game_advance_error_length()
    if(length>65536)throw new Error("Gameplay error detail exceeds its bound")
    const detail=e.playsrc_alloc(Math.max(1,length))>>>0
    if(e.playsrc_game_advance_error_copy(detail,length)!==length)throw new Error("Gameplay error detail copy failed")
    const message=new TextDecoder().decode(new Uint8Array(e.memory.buffer,detail,length));e.playsrc_free(detail,Math.max(1,length))
    throw new Error(`WASM gameplay transaction failed after tick ${advanced}, steps=${steps}, code=${e.playsrc_game_advance_error()}, detail=${message}`)
  }
  e.playsrc_free(pointer,command.length);advanced+=steps
  const snapshot=decodeSnapshot(snapshotBytes(handle));advanced=Number(snapshot.tick)
  history.push({tick:snapshot.tick,round:snapshot.round,points:snapshot.controlPoints?.points.map(point=>point.owner),bots:snapshot.bots.map(bot=>({identity:bot.identity,position:bot.position,velocity:bot.velocity,area:bot.area,captures:bot.captures}))})
  if(snapshot.round.winningTeam===2){won=true;break}
}
let replayVerification:unknown
if(selfReplay){
  e.playsrc_gameplay_replay_mark(handle,1)
  if(e.playsrc_gameplay_replay_stop(handle)!==1)throw new Error("Replay capture incomplete")
  const length=e.playsrc_gameplay_replay_length(handle),pointer=e.playsrc_alloc(length)>>>0
  if(e.playsrc_gameplay_replay_copy(handle,0,pointer,length)!==length)throw new Error("Replay capture copy failed")
  const bytes=Buffer.from(new Uint8Array(e.memory.buffer,pointer,length));e.playsrc_free(pointer,length)
  const journal=parseGameplayReplay(bytes)
  await writeFile(path.join(config.sourceCacheDir,"evidence/map-runtime",`${target}-wasm-bot-progression-${addTick}.replay.bin`),bytes)
  e.playsrc_dispose(handle)
  if(!journal.initialEquipment)throw new Error("Replay initial equipment is absent")
  const restore=Buffer.concat([Buffer.from([0]),journal.initialEquipment]),restorePointer=copy(restore)
  if(e.playsrc_equipment_update(0,restorePointer,restore.length)!==1)throw new Error("Replay initial equipment restore failed")
  e.playsrc_free(restorePointer,restore.length)
  const source=copy(bsp),table=copy(tableBytes),digest=copy(configurationHash)
  const replayHandle=e.playsrc_compile_map(source,bsp.length,1,table,sections.length,digest,1)
  if(e.playsrc_result_error(replayHandle)!==0)throw new Error("Replay map compilation failed")
  e.playsrc_free(source,bsp.length);e.playsrc_free(table,tableBytes.length);e.playsrc_free(digest,32)
  e.playsrc_result_release(replayHandle);e.playsrc_presentation_release(replayHandle)
  const clocks=replayWorkClockBytes(journal.records,journal.version),clockPointer=copy(clocks.length?clocks:new Uint8Array(1))
  if(e.playsrc_gameplay_replay_clock_input(replayHandle,clockPointer,clocks.length)!==1)throw new Error("Replay clock inputs rejected")
  e.playsrc_free(clockPointer,Math.max(clocks.length,1))
  if(e.playsrc_gameplay_replay_begin(replayHandle)!==1)throw new Error("Replay verification capture rejected")
  let publications=0
  for(const record of journal.records){
    const data=record.bytes
    if(record.kind===1)observe(replayHandle,data.readDoubleLE(0),data.subarray(24),data.readUInt32LE(8),data.readBigUInt64LE(12))
    else if(record.kind===3){
      const count=e.playsrc_simulation_output_length(replayHandle),pointer=e.playsrc_simulation_output_pointer(replayHandle)>>>0
      if(hash(new Uint8Array(e.memory.buffer,pointer,count))!==data.toString("hex"))throw new Error(`Replay publication differs at ${publications}`)
      publications++
    }else if(record.kind===7)e.playsrc_gameplay_replay_mark(replayHandle,data.readUInt32LE(0))
    else if(![2,8].includes(record.kind))throw new Error("Unexpected mutation in scripted progression")
  }
  if(e.playsrc_gameplay_replay_clock_remaining(replayHandle)!==0||e.playsrc_gameplay_replay_stop(replayHandle)!==1)throw new Error("Replay work-clock consumption or completion differs")
  const count=e.playsrc_gameplay_replay_length(replayHandle),out=e.playsrc_alloc(count)>>>0
  e.playsrc_gameplay_replay_copy(replayHandle,0,out,count)
  const actual=parseGameplayReplay(Buffer.from(new Uint8Array(e.memory.buffer,out,count)))
  e.playsrc_free(out,count)
  const expectedTicks=journal.records.filter(record=>record.kind===2),actualTicks=actual.records.filter(record=>record.kind===2)
  if(actualTicks.length!==expectedTicks.length||actualTicks.some((record,index)=>!record.bytes.subarray(16).equals(expectedTicks[index]!.bytes.subarray(16))))throw new Error("Replay tick publication/command/clock transcript differs")
  replayVerification={publications,ticks:actualTicks.length,workClockValues:clocks.length/8,sha256:hash(bytes)}
  e.playsrc_dispose(replayHandle)
}
if(!selfReplay)e.playsrc_dispose(handle)
for(const section of sections)if(e.playsrc_resource_release(section.pointer,section.length)!==1)throw new Error("Resource owner release failed")
const result={target,graphIdentity,wasmSha256:hash(wasm),addTick,won,scope:selfReplay?"capture-recompile-replay":"advance-and-snapshot-decode",milliseconds:performance.now()-started,replayVerification,history}
await writeFile(path.join(config.sourceCacheDir,"evidence/map-runtime",`${target}-wasm-bot-progression-${addTick}.json`),JSON.stringify(result,(_,value)=>typeof value==="bigint"?value.toString():value))
console.log(JSON.stringify({...result,history:undefined,last:history.at(-1)},(_,value)=>typeof value==="bigint"?value.toString():value))
if(!won)process.exitCode=1
