// Injected only into a sampled dedicated Worker, never bundled with the game.
export function installWorkerTaskProfiler(host: any = globalThis, identity = "test"): void {
  if (host.__playsrcWorkerTasks) throw new Error("Worker task profiler already installed")
  const limit = 16_384
  const state = {
    active: true, limit, dropped: 0, sequence: 0, timeOrigin: host.performance.timeOrigin,
    tasks: [] as any[], current: null as any,
  }
  const mark = (name: string) => {
    host.performance.mark(name)
    // The trace owns the event; do not retain thousands of PerformanceEntries too.
    host.performance.clearMarks(name)
  }
  const sync = (phase: string) => {
    const name = `playsrc-worker-clock:${identity}:${phase}`
    const before = host.performance.now()
    mark(name)
    const after = host.performance.now()
    return { name, before, after }
  }
  const started = sync("start")
  const originalMessage = host.onmessage
  const originalPost = host.postMessage
  host.onmessage = function(this: any, event: any) {
    if (!state.active || !Number.isSafeInteger(event.data?.id)) return originalMessage.call(this, event)
    if (state.tasks.length >= limit) { state.dropped += 1; return originalMessage.call(this, event) }
    const request = event.data
    const sequence = ++state.sequence
    const prefix = `playsrc-worker-task:${identity}:${sequence}:${request.id}`
    const task = {
      sequence, requestId: request.id, kind: request.kind, generation: request.generation ?? null,
      queuedAt: request.queuedAt ?? null, nowSeconds: request.nowSeconds ?? null,
      started: host.performance.now(), finished: null as number | null,
      startMark: `${prefix}:start`, endMark: `${prefix}:end`, responses: [] as any[], memory: [] as any[], observes: [] as any[],
    }
    state.tasks.push(task)
    state.current = task
    mark(task.startMark)
    try { return originalMessage.call(this, event) }
    finally {
      task.finished = host.performance.now()
      mark(task.endMark)
      state.current = null
    }
  }
  host.postMessage = function(this: any, message: any, ...rest: any[]) {
    const responseMessage = message?.kind === "reply-control" ? message.response : message
    const task = state.current
    if (!state.active || !task) return originalPost.call(this, message, ...rest)
    // Read byte lengths before transfer detaches ownership. Never retain payloads/views.
    const shared = typeof SharedArrayBuffer === "function" && responseMessage?.output instanceof SharedArrayBuffer
    const response = {
      requestId: responseMessage?.id, kind: responseMessage?.kind,
      bytes: shared ? 0 : responseMessage?.outputs?.reduce((sum: number, output: ArrayBuffer) => sum + output.byteLength, 0)
        ?? responseMessage?.output?.byteLength ?? responseMessage?.payload?.byteLength ?? 0,
      sharedBytes: shared ? responseMessage.byteLength ?? 0 : 0,
      timings: responseMessage?.timings ?? null,
      started: host.performance.now(), finished: null as number | null,
    }
    task.responses.push(response)
    try { return originalPost.call(this, message, ...rest) }
    finally { response.finished = host.performance.now() }
  }
  host.__playsrcWorkerProfileReply = (response: any) => {
    if (state.active && state.current) state.current.responses.push(response)
  }
  host.__playsrcWorkerProfileObserve = (span: any) => {
    if (state.active && state.current) state.current.observes.push(span)
  }
  // The game supplies only allocation counters, not module/heap references.
  host.__playsrcWorkerProfileMemory = (linearBytes: number, liveBytes: number, highWaterBytes: number) => {
    // usize counters cross the WASM32 ABI as signed JS i32 results.
    if (state.active && state.current) state.current.memory.push({ at: host.performance.now(), linearBytes, liveBytes: liveBytes >>> 0, highWaterBytes: highWaterBytes >>> 0 })
  }
  host.__playsrcWorkerTasks = {
    stop() {
      state.active = false
      const ended = sync("end")
      host.onmessage = originalMessage
      host.postMessage = originalPost
      delete host.__playsrcWorkerProfileMemory
      delete host.__playsrcWorkerProfileReply
      delete host.__playsrcWorkerProfileObserve
      return { timeOrigin: state.timeOrigin, limit, dropped: state.dropped, tasks: state.tasks, clocks: [started, ended] }
    },
  }
}
