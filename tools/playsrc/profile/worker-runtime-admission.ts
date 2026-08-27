type WorkerRuntimeTransport = {
  send(method: string, params?: Record<string, unknown>): Promise<unknown>
  on(method: string, callback: (event: any) => void): unknown
  off(method: string, callback: (event: any) => void): unknown
}

/** Target attachment is not execution-context admission. In particular, do not
 * evaluate in a newly attached module Worker before Runtime has announced its
 * context. The returned ID must be supplied to subsequent Runtime.evaluate calls;
 * a destroyed context is an error, never permission to evaluate in another one.
 * Keep this function self-contained for native CDP controller serialization.
 */
export function admitWorkerExecutionContext(transport: WorkerRuntimeTransport, milliseconds = 5_000): Promise<number> {
  if (!Number.isSafeInteger(milliseconds) || milliseconds < 1 || milliseconds > 5_000) return Promise.reject(new Error("Worker context admission deadline is invalid"))
  return new Promise((resolve, reject) => {
    let contextId: number | undefined, enabled = false, settled = false
    const finish = (error?: Error) => {
      if (settled || !error && (!enabled || contextId === undefined)) return
      settled = true
      clearTimeout(timer)
      for (const [method, callback] of listeners) transport.off(method, callback)
      if (error) reject(error)
      else resolve(contextId!)
    }
    const listeners: Array<[string, (event: any) => void]> = [
      ["Runtime.executionContextCreated", event => {
        const context = event.context
        if (context?.auxData?.isDefault === false) return
        if (!Number.isSafeInteger(context?.id) || context.id < 1) { finish(new Error("Worker execution context ID is invalid")); return }
        if (contextId !== undefined && contextId !== context.id) { finish(new Error("Worker execution context changed during admission")); return }
        contextId = context.id
        finish()
      }],
      ["Runtime.executionContextDestroyed", event => { if (event.executionContextId === contextId) finish(new Error("Worker execution context destroyed during admission")) }],
      ["Runtime.executionContextsCleared", () => { if (contextId !== undefined) finish(new Error("Worker execution contexts cleared during admission")) }],
      ["Inspector.detached", () => finish(new Error("Worker detached during context admission"))],
      ["Inspector.targetCrashed", () => finish(new Error("Worker crashed during context admission"))],
    ]
    const timer = setTimeout(() => finish(new Error("Worker execution context admission exceeded its deadline")), milliseconds)
    for (const [method, callback] of listeners) transport.on(method, callback)
    const fail = (error: unknown) => finish(error instanceof Error ? error : new Error(String(error)))
    try { void transport.send("Runtime.enable").then(() => { enabled = true; finish() }, fail) }
    catch (error) { fail(error) }
  })
}
