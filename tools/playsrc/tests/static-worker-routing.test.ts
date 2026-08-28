import { expect, test } from "bun:test"
import { configureStaticWorkerRoute, STATIC_WORKER_ATTACH } from "../profile/static-worker-routing"

test("nested worker Fetch ownership is installed before script resumption",async()=>{
  const calls:Array<{method:string;params:unknown}>=[]
  await configureStaticWorkerRoute({send:async(method,params)=>{calls.push({method,params});return {}}})
  expect(calls.map(call=>call.method)).toEqual(["Fetch.enable","Target.setAutoAttach","Runtime.runIfWaitingForDebugger"])
  expect(calls[1]!.params).toEqual(STATIC_WORKER_ATTACH)
  expect(STATIC_WORKER_ATTACH.waitForDebuggerOnStart).toBe(true)
})

test("routing failure is not mistaken for a resumed worker",async()=>{
  const calls:string[]=[]
  await expect(configureStaticWorkerRoute({send:async method=>{calls.push(method);throw new Error("Fetch unavailable")}})).rejects.toThrow("Fetch unavailable")
  expect(calls).toEqual(["Fetch.enable"])
})
