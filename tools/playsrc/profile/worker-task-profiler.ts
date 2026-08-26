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
      startMark: `${prefix}:start`, endMark: `${prefix}:end`, responses: [] as any[], memory: [] as any[],
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
    const task = state.current
    if (!state.active || !task) return originalPost.call(this, message, ...rest)
    // Read byte lengths before transfer detaches ownership. Never retain payloads/views.
    const response = {
      requestId: message?.id, kind: message?.kind,
      bytes: message?.outputs?.reduce((sum: number, output: ArrayBuffer) => sum + output.byteLength, 0)
        ?? message?.output?.byteLength ?? message?.payload?.byteLength ?? 0,
      timings: message?.timings ?? null,
      started: host.performance.now(), finished: null as number | null,
    }
    task.responses.push(response)
    try { return originalPost.call(this, message, ...rest) }
    finally { response.finished = host.performance.now() }
  }
  // The game supplies only allocation counters, not module/heap references.
  host.__playsrcWorkerProfileMemory = (linearBytes: number, liveBytes: number, highWaterBytes: number) => {
    if (state.active && state.current) state.current.memory.push({ at: host.performance.now(), linearBytes, liveBytes, highWaterBytes })
  }
  host.__playsrcWorkerTasks = {
    stop() {
      state.active = false
      const ended = sync("end")
      host.onmessage = originalMessage
      host.postMessage = originalPost
      delete host.__playsrcWorkerProfileMemory
      return { timeOrigin: state.timeOrigin, limit, dropped: state.dropped, tasks: state.tasks, clocks: [started, ended] }
    },
  }
}
