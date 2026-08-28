import { expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import { installStaticPackageRouting } from "../profile/static-package-routing"

test("browser Fetch routes unchanged page and nested worker bytes through one owner",async()=>{
  const calls:any[]=[],events=new EventEmitter()
  const browser={on:events.on.bind(events),off:events.off.bind(events),send:async(method:string,params:any)=>{calls.push({method,params});return {}}} as any
  const bytes=Buffer.from('exact generated helper bytes')
  const route=await installStaticPackageRouting(browser,async()=>({status:200,headers:{"content-type":"text/javascript"},body:bytes}))
  expect(calls[0].method).toBe('Fetch.enable')
  events.emit('Fetch.requestPaused',{requestId:'helper',request:{url:'https://playsrc.online/tf2/assets/helper.js'},resourceType:'Script'})
  await new Promise(r=>setTimeout(r,0))
  expect(calls[1]).toEqual({method:'Fetch.fulfillRequest',params:{requestId:'helper',responseCode:200,responseHeaders:[{name:'content-type',value:'text/javascript'}],body:bytes.toString('base64')}})
  route.check();await route.close()
  expect(calls.at(-1).method).toBe('Fetch.disable')
})

test("unfulfilled immutable package requests fail the gate instead of silently hanging startup",async()=>{
  const events=new EventEmitter(),calls:string[]=[]
  const browser={on:events.on.bind(events),off:events.off.bind(events),send:async(method:string)=>{calls.push(method);return {}}} as any
  const route=await installStaticPackageRouting(browser,async()=>{throw new Error('immutable object missing')})
  events.emit('Fetch.requestPaused',{requestId:'helper',request:{url:'https://playsrc.online/tf2/assets/helper.js'}})
  await new Promise(r=>setTimeout(r,0))
  expect(()=>route.check()).toThrow('immutable object missing')
  expect(calls).toContain('Fetch.failRequest');await route.close()
})
