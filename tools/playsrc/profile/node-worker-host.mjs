import { Worker as ThreadWorker, isMainThread, parentPort, workerData } from "node:worker_threads"

// Standard message/transfer adaptation for generated Rayon helpers, not a
// browser or game emulation. No generated helper bytes are patched.
export function installNodeWorkerHost() {
  const previous = { self: globalThis.self, Worker: globalThis.Worker }
  globalThis.self = new EventTarget()
  globalThis.Worker = class extends EventTarget {
    constructor(url) {
      super()
      this.thread = new ThreadWorker(new URL("../profile/node-worker-host.mjs", import.meta.url), { workerData: { url: String(url) } })
      this.thread.on("message", data => this.dispatchEvent(new MessageEvent("message", { data })))
      this.thread.on("error", error => { console.error(error); this.dispatchEvent(Object.assign(new Event("error"), { error })) })
    }
    postMessage(data, transfer = []) { this.thread.postMessage(data, transfer) }
    terminate() { return this.thread.terminate() }
  }
  return () => { globalThis.self = previous.self; globalThis.Worker = previous.Worker }
}

if (!isMainThread) {
  const target = new EventTarget(), pending = []
  let ready = false
  globalThis.self = target
  globalThis.postMessage = (data, transfer = []) => parentPort.postMessage(data, transfer)
  parentPort.on("message", data => ready ? target.dispatchEvent(new MessageEvent("message", { data })) : pending.push(data))
  await import(workerData.url)
  ready = true
  for (const data of pending) target.dispatchEvent(new MessageEvent("message", { data }))
}
