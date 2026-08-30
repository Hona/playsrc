/** Observe the client's existing reply hook without attaching a Worker debugger,
 * replacing a timer, touching the mailbox, or retaining any payload/heap view. */
export function installDeliveryRpcObserver(host: any = globalThis, kinds?: readonly string[]) {
  const NativeWorker = host.Worker
  const limit = 16_384
  let active = false, started = 0, dropped = 0, records: any[] = [], workers = 0
  host.Worker = class extends NativeWorker {
    constructor(url: any, options: any) {
      super(url, options)
      if (!String(url).includes("gameplay-worker")) return
      const worker = ++workers, pending = new Map<number, { kind: string; at: number }>()
      const post = this.postMessage.bind(this)
      this.postMessage = (message: any, ...rest: any[]) => {
        if (Number.isSafeInteger(message?.id)) {
          if (pending.size < limit) pending.set(message.id, { kind: message.kind, at: host.performance.now() })
          else if (active) dropped++
        }
        return post(message, ...rest)
      }
      this.__playsrcProfileReply = (response: any) => {
        const request = pending.get(response?.id)
        pending.delete(response?.id)
        if (!active || !request || !response.timings || kinds && !kinds.includes(request.kind)) return
        if (records.length >= limit) { dropped++; return }
        const at = host.performance.now(), timings: Record<string, number | null> = {}
        for (const key of ["queueMilliseconds", "inputCopyMilliseconds", "transactMilliseconds", "outputCopyMilliseconds", "totalMilliseconds",
          "wasmLinearMemoryBytes", "wasmAllocatorLiveBytes", "wasmAllocatorHighWaterBytes"]) {
          timings[key] = typeof response.timings[key] === "number" ? response.timings[key] : null
        }
        const outputBytes = typeof response.byteLength === "number" ? response.byteLength : response.output instanceof ArrayBuffer ? response.output.byteLength : undefined
        records.push({ worker, id: response.id, kind: request.kind, sent: request.at, received: at, censoredStart: request.at < started, outputBytes,
          elapsedMilliseconds: at - request.at, timings })
      }
    }
  }
  host.__playsrcDeliveryRpc = {
    start(at: number) { started = at; records = []; dropped = 0; active = true },
    stop() { active = false; return { records, dropped, workers, scope: "Client request-to-existing-reply-hook, with original Worker timing fields; no Worker debugger or payload retention" } },
  }
}
