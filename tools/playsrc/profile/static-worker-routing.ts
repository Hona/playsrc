import type { CDPSession } from "@playwright/test"
import { WorkerCdpSession } from "./worker-cpu-profiler"
import { admitWorkerExecutionContext } from "./worker-runtime-admission"

type Transport = CDPSession | WorkerCdpSession
export const STATIC_WORKER_ATTACH = { autoAttach: true, waitForDebuggerOnStart: true, flatten: false,
  filter: [{ type: "worker", exclude: false }, { exclude: true }] }

export async function configureStaticWorkerRoute(session: Pick<WorkerCdpSession,"send">) {
  await session.send("Fetch.enable",{patterns:[{urlPattern:"*",requestStage:"Request"}]})
  await session.send("Target.setAutoAttach",STATIC_WORKER_ATTACH)
  await session.send("Runtime.runIfWaitingForDebugger")
}

/** Worker-created requests do not pass through Playwright's page Fetch owner.
 * Install the worker's own request route before resuming it, including nested
 * Rayon helpers. No script, generated glue, memory or request URL is rewritten. */
export async function installStaticWorkerRouting(page: CDPSession, response: (url: string) => Promise<{ status: number; headers: Record<string,string>; body: Buffer } | null>) {
  const sessions: WorkerCdpSession[] = [], records: any[] = [], pending = new Set<Promise<void>>()
  let failure: unknown
  const attach = (parent: Transport, event: any) => {
    if(event.targetInfo.type!=="worker")return
    const session = new WorkerCdpSession(parent,event.sessionId);sessions.push(session)
    const record:any={target:event.targetInfo,at:Date.now(),requests:[]};records.push(record)
    let detached=false
    session.on("Inspector.detached",()=>{detached=true;record.detachedAt=Date.now()})
    session.on("Target.attachedToTarget",value=>attach(session,value))
    session.on("Fetch.requestPaused",event=>{
      const task=(async()=>{
        try {
          const value=await response(event.request.url)
          record.requests.push({url:event.request.url,status:value?.status??null})
          if(detached)return
          if(value)await session.send("Fetch.fulfillRequest",{requestId:event.requestId,responseCode:value.status,
            responseHeaders:Object.entries(value.headers).map(([name,value])=>({name,value})),body:value.body.toString("base64")})
          else await session.send("Fetch.continueRequest",{requestId:event.requestId})
        }catch(error){if(!detached)failure??=error;await session.send("Fetch.failRequest",{requestId:event.requestId,errorReason:"Aborted"}).catch(()=>{})}
      })();pending.add(task);void task.finally(()=>pending.delete(task))
    })
    const startup=(async()=>{
      try {
        await configureStaticWorkerRoute(session)
        if(event.targetInfo.url.includes("gameplay-worker"))record.executionContextId=await admitWorkerExecutionContext(session)
        else record.contextAdmission="No evaluation in synchronously parked helper workers"
      }catch(error){record.error=String(error);if(!detached)failure??=error;await session.send("Runtime.runIfWaitingForDebugger").catch(()=>{})}
    })();pending.add(startup);void startup.finally(()=>pending.delete(startup))
  }
  const listener=(event:any)=>attach(page,event)
  page.on("Target.attachedToTarget",listener)
  await page.send("Target.setAutoAttach",STATIC_WORKER_ATTACH)
  return {
    records,
    check(){if(failure)throw new Error(`Exact static Worker routing failed: ${String(failure)}`)},
    async close(){page.off("Target.attachedToTarget",listener);await page.send("Target.setAutoAttach",{autoAttach:false,waitForDebuggerOnStart:false,flatten:false}).catch(()=>{});await Promise.allSettled(sessions.map(session=>session.close()));await Promise.allSettled(pending)},
  }
}
