import type { CDPSession } from "@playwright/test"

type Response = { status: number; headers: Record<string,string>; body: Buffer } | null

/** Browser-target Fetch owns page and worker-created requests alike. Worker
 * targets do not implement Fetch, and page-only routing misses Rayon scripts.
 * This does not pause scripts, change URLs/bytes or override autoplay/security. */
export async function installStaticPackageRouting(browser: Pick<CDPSession,"send"|"on"|"off">, response: (url:string)=>Promise<Response>) {
  const records:any[]=[],pending=new Set<Promise<void>>()
  let failure:unknown,closing=false
  const listener=(event:any)=>{
    const task=(async()=>{
      try {
        const value=await response(event.request.url)
        records.push({url:event.request.url,status:value?.status??null,resourceType:event.resourceType,frameId:event.frameId??null})
        if(value)await browser.send("Fetch.fulfillRequest",{requestId:event.requestId,responseCode:value.status,
          responseHeaders:Object.entries(value.headers).map(([name,value])=>({name,value})),body:value.body.toString("base64")})
        else await browser.send("Fetch.continueRequest",{requestId:event.requestId})
      }catch(error){if(!closing)failure??=error;await browser.send("Fetch.failRequest",{requestId:event.requestId,errorReason:"Aborted"}).catch(()=>{})}
    })();pending.add(task);void task.finally(()=>pending.delete(task))
  }
  browser.on("Fetch.requestPaused",listener)
  await browser.send("Fetch.enable",{patterns:[{urlPattern:"*",requestStage:"Request"}]})
  return {
    records,
    check(){if(failure)throw new Error(`Exact static package routing failed: ${String(failure)}`)},
    async close(){closing=true;await browser.send("Fetch.disable").catch(()=>{});browser.off("Fetch.requestPaused",listener);await Promise.allSettled(pending)},
  }
}
