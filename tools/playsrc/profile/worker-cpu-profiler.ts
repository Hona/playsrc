import type { CDPSession, Page } from "@playwright/test"
import { EventEmitter } from "node:events"
import type { CpuProfile } from "./gameui-profile"
import { installWorkerTaskProfiler } from "./worker-task-profiler"
import { admitWorkerExecutionContext } from "./worker-runtime-admission"

// Non-flattened CDP sessions let the browser address a real dedicated Worker;
// page Profiler.start samples only the renderer, not its Workers.
export class WorkerCdpSession extends EventEmitter {
  #next = 0
  #pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>()
  constructor(readonly browser: CDPSession | WorkerCdpSession, readonly sessionId: string) {
    super()
    browser.on("Target.receivedMessageFromTarget", this.#receive)
    browser.on("Target.detachedFromTarget", this.#detached)
  }
  #detached = (event: { sessionId: string }) => {
    if (event.sessionId === this.sessionId) this.emit("Inspector.detached", event)
  }
  #receive = (event: { sessionId: string; message: string }) => {
    if (event.sessionId !== this.sessionId) return
    const message = JSON.parse(event.message)
    if (message.method) { this.emit(message.method, message.params); return }
    const pending = this.#pending.get(message.id)
    if (!pending) return
    this.#pending.delete(message.id)
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(JSON.stringify(message.error)))
    else pending.resolve(message.result)
  }
  send(method: string, params: Record<string, unknown> = {}): Promise<any> {
    const id = ++this.#next
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id)
        reject(new Error(`Worker CDP ${method} exceeded 5 seconds`))
      }, 5_000)
      this.#pending.set(id, { resolve, reject, timer })
      void this.browser.send("Target.sendMessageToTarget", { sessionId: this.sessionId, message: JSON.stringify({ id, method, params }) }).catch(error => {
        clearTimeout(timer)
        this.#pending.delete(id)
        reject(error)
      })
    })
  }
  async close(): Promise<void> {
    this.emit("Inspector.detached", {})
    this.browser.off("Target.receivedMessageFromTarget", this.#receive)
    this.browser.off("Target.detachedFromTarget", this.#detached)
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer)
      pending.reject(new Error("Worker CDP session closed"))
    }
    this.#pending.clear()
    await this.browser.send("Target.detachFromTarget", { sessionId: this.sessionId })
  }
}

export type WorkerCpuCapture = Readonly<{
  target: { targetId: string; type: string; url: string; browserContextId?: string }
  samplingIntervalMicroseconds: number
  executionContextId?: number
  deadlineStopped?: boolean
  profile: CpuProfile
  execution: {
    timeOrigin: number; limit: number; dropped: number
    clocks: Array<{ name: string; before: number; after: number }>
    tasks: Array<{ sequence: number; requestId: number; kind: string; started: number; finished: number; startMark: string; endMark: string; responses: any[]; memory: any[] }>
  }
}>

export async function startWorkerCpuCapture(browser: CDPSession, pageCdp: CDPSession, page: Page) {
  const { targetInfo: owner } = await pageCdp.send("Target.getTargetInfo")
  const urls = new Set(page.workers().map(worker => worker.url()))
  const { targetInfos } = await browser.send("Target.getTargets")
  const targets = targetInfos.filter(target => target.type === "worker" && urls.has(target.url)
    && target.browserContextId === owner.browserContextId)
  const gameplay = targets.filter(target => target.url.includes("gameplay-worker"))
  if (gameplay.length !== 1) throw new Error(`Expected one exact gameplay Worker target, found ${gameplay.length}`)
  if (targets.length > 32) throw new Error("Worker CPU target bound exceeded")
  // Rayon helpers can be synchronously parked in WASM/Atomics.wait and cannot
  // service Runtime.evaluate. Sample the actual gameplay Worker; all other
  // browser/renderer/Worker/GPU threads remain in the native trace.
  const attached: Array<{ target: typeof targets[number]; session: WorkerCdpSession; contextId?: number }> = []
  try {
    for (const target of gameplay) {
      const { sessionId } = await browser.send("Target.attachToTarget", { targetId: target.targetId, flatten: false })
      const session = new WorkerCdpSession(browser, sessionId)
      const entry: typeof attached[number] = { target, session }
      attached.push(entry)
      entry.contextId = await admitWorkerExecutionContext(session)
      await session.send("Profiler.enable")
      await session.send("Profiler.setSamplingInterval", { interval: 1_000 })
    }
    let stopped: Promise<WorkerCpuCapture[]> | undefined
    let deadline: ReturnType<typeof setTimeout> | undefined
    let deadlineStopped = false
    const stop = (): Promise<WorkerCpuCapture[]> => {
      if (stopped) return stopped
      if (deadline) clearTimeout(deadline)
      stopped = Promise.all(attached.map(async ({ target, session, contextId }) => {
        const { profile } = await session.send("Profiler.stop")
        const result = await session.send("Runtime.evaluate", { expression: "globalThis.__playsrcWorkerTasks.stop()", returnByValue: true, contextId })
        if (result.exceptionDetails) throw new Error(`Worker profiler collection failed: ${JSON.stringify(result.exceptionDetails)}`)
        return { target, executionContextId: contextId, samplingIntervalMicroseconds: 1_000, profile, execution: result.result.value, deadlineStopped }
      }))
      return stopped
    }
    return {
      unsampledTargets: targets.filter(target => !gameplay.includes(target)),
      async start() {
        deadline = setTimeout(() => { deadlineStopped = true; void stop().catch(() => undefined) }, 12_000)
        await Promise.all(attached.map(async ({ target, session, contextId }) => {
          const expression = `(${installWorkerTaskProfiler.toString()})(globalThis, ${JSON.stringify(target.targetId)})`
          const result = await session.send("Runtime.evaluate", { expression, contextId })
          if (result.exceptionDetails) throw new Error(`Worker profiler injection failed: ${JSON.stringify(result.exceptionDetails)}`)
          await session.send("Profiler.start")
        }))
      },
      stop,
      async close() {
        if (deadline) clearTimeout(deadline)
        await Promise.all(attached.map(({ session }) => session.close()))
      },
    }
  } catch (error) {
    await Promise.allSettled(attached.map(({ session }) => session.close()))
    throw error
  }
}
