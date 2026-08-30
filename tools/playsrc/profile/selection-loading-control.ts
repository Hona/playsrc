import os from "node:os"
import type { CDPSession } from "@playwright/test"

/** Read-only control: records actual transfers and host pressure, never changes
 * cache state, scheduling, request interception or the application's load. */
export async function selectionLoadingControl(session: CDPSession) {
  const requests: any[] = [], pressure: any[] = [], boundaries: any[] = []
  const pending = new Map<string, any>()
  const dropped = { requests: 0, pressure: 0, boundaries: 0 }
  let stopped = false
  await session.send("Network.enable")
  const onRequest = (event: any) => {
    if (stopped) return
    if (requests.length >= 4096) { dropped.requests++; return }
    const url = new URL(event.request.url)
    // No headers, credentials, query strings or response bodies are retained.
    // A data: URL's pathname IS its encoded body. Keep metadata only, including
    // for authored HUD PNGs; inline media must not enter a diagnostic record.
    const network = url.protocol === "http:" || url.protocol === "https:"
    const row = { id: event.requestId, path: network ? url.pathname.slice(0, 4096) : null,
      pathTruncated: network && url.pathname.length > 4096, origin: url.origin, scheme: url.protocol,
      inlineUrlCharacters: network ? null : event.request.url.length, type: event.type,
      started: event.timestamp, wallTime: event.wallTime, method: event.request.method }
    requests.push(row); pending.set(event.requestId, row)
  }
  const onResponse = (event: any) => {
    const row = pending.get(event.requestId)
    if (row) Object.assign(row, { responseAt: event.timestamp, status: event.response.status,
      diskCache: event.response.fromDiskCache ?? false, serviceWorker: event.response.fromServiceWorker ?? false,
      protocol: event.response.protocol, timing: event.response.timing })
  }
  const onCache = (event: any) => {
    const row = pending.get(event.requestId); if (row) row.servedFromCache = true
  }
  const onFinished = (event: any) => {
    const row = pending.get(event.requestId); if (row) Object.assign(row, { finished: event.timestamp, encodedBytes: event.encodedDataLength })
    pending.delete(event.requestId)
  }
  const onFailed = (event: any) => {
    const row = pending.get(event.requestId); if (row) Object.assign(row, { failed: event.timestamp, failure: event.errorText })
    pending.delete(event.requestId)
  }
  const listeners = { "Network.requestWillBeSent": onRequest, "Network.responseReceived": onResponse,
    "Network.requestServedFromCache": onCache, "Network.loadingFinished": onFinished, "Network.loadingFailed": onFailed }
  for (const [name, listener] of Object.entries(listeners)) session.on(name as any, listener)
  const sample = () => {
    if (pressure.length >= 256) { dropped.pressure++; return }
    pressure.push({ epoch: Date.now(), freeBytes: os.freemem(), totalBytes: os.totalmem(),
     cpus: os.cpus().map(cpu => cpu.times) })
  }
  sample()
  const timer = setInterval(sample, 1000)
  let result: any
  return {
    boundary(name: string) {
      if (stopped) throw new Error("Loading control already stopped")
      if (boundaries.length >= 32) { dropped.boundaries++; return }
      boundaries.push({ name, epoch: Date.now() }); sample()
    },
    stop() {
      if (stopped) return result
      stopped = true; clearInterval(timer); sample()
      for (const [name, listener] of Object.entries(listeners)) session.off(name as any, listener)
      const unfinishedRequests = pending.size; pending.clear()
      return result = { requests, pressure, boundaries, dropped, unfinishedRequests,
        complete: !Object.values(dropped).some(Boolean) && !requests.some(row => row.pathTruncated),
        scope: "Bounded host cumulative CPU ticks/free physical memory and actual request metadata; not external-process attribution. No inline media bodies. Incomplete retention is explicit." }
    },
  }
}
