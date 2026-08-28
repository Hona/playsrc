import os from "node:os"
import type { CDPSession } from "@playwright/test"

/** Read-only control: records actual transfers and host pressure, never changes
 * cache state, scheduling, request interception or the application's load. */
export async function selectionLoadingControl(session: CDPSession) {
  const requests: any[] = [], pressure: any[] = [], boundaries: any[] = []
  const pending = new Map<string, any>()
  await session.send("Network.enable")
  session.on("Network.requestWillBeSent", event => {
    const url = new URL(event.request.url)
    // No headers, credentials, query strings or response bodies are retained.
    const row = { id: event.requestId, path: url.pathname, origin: url.origin, type: event.type,
      started: event.timestamp, wallTime: event.wallTime, method: event.request.method }
    requests.push(row); pending.set(event.requestId, row)
  })
  session.on("Network.responseReceived", event => {
    const row = pending.get(event.requestId)
    if (row) Object.assign(row, { responseAt: event.timestamp, status: event.response.status,
      diskCache: event.response.fromDiskCache ?? false, serviceWorker: event.response.fromServiceWorker ?? false,
      protocol: event.response.protocol, timing: event.response.timing })
  })
  session.on("Network.requestServedFromCache", event => {
    const row = pending.get(event.requestId); if (row) row.servedFromCache = true
  })
  session.on("Network.loadingFinished", event => {
    const row = pending.get(event.requestId); if (row) Object.assign(row, { finished: event.timestamp, encodedBytes: event.encodedDataLength })
  })
  session.on("Network.loadingFailed", event => {
    const row = pending.get(event.requestId); if (row) Object.assign(row, { failed: event.timestamp, failure: event.errorText })
  })
  const sample = () => pressure.push({ epoch: Date.now(), freeBytes: os.freemem(), totalBytes: os.totalmem(),
    cpus: os.cpus().map(cpu => cpu.times) })
  sample()
  const timer = setInterval(sample, 1000)
  return {
    boundary(name: string) { boundaries.push({ name, epoch: Date.now() }); sample() },
    stop() { clearInterval(timer); sample(); return { requests, pressure, boundaries,
      scope: "Host cumulative CPU ticks/free physical memory and actual browser requests; not attribution to a particular external process" } },
  }
}
