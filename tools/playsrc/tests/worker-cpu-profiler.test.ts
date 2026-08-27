import { describe, expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import { startWorkerCpuCapture, WorkerCdpSession } from "../profile/worker-cpu-profiler"
import { admitWorkerExecutionContext } from "../profile/worker-runtime-admission"

describe("actual Worker CDP sampling transport", () => {
  test("context admission is scoped to its Worker and fails on native detach", async () => {
    const browser: any = new EventEmitter()
    browser.send = async (method: string, params: any) => {
      if (method === "Target.sendMessageToTarget") {
        const message = JSON.parse(params.message)
        browser.emit("Target.receivedMessageFromTarget", { sessionId: params.sessionId, message: JSON.stringify({ id: message.id, result: {} }) })
      }
      return {}
    }
    const session = new WorkerCdpSession(browser, "game")
    const ready = admitWorkerExecutionContext(session)
    browser.emit("Target.receivedMessageFromTarget", { sessionId: "helper", message: JSON.stringify({ method: "Runtime.executionContextCreated", params: { context: { id: 9 } } }) })
    browser.emit("Target.detachedFromTarget", { sessionId: "game" })
    await expect(ready).rejects.toThrow("detached")
    await session.close()
    expect(browser.listenerCount("Target.detachedFromTarget")).toBe(0)
  })
  test("samples the exact gameplay Worker without evaluating synchronously parked WASM helpers", async () => {
    const browser: any = new EventEmitter()
    const targets = [
      { targetId: "game", type: "worker", browserContextId: "context", url: "http://local/gameplay-worker.ts" },
      { targetId: "helper", type: "worker", browserContextId: "context", url: "http://local/workerHelpers.js" },
      { targetId: "other-page", type: "worker", browserContextId: "different", url: "http://local/gameplay-worker.ts" },
    ]
    const evaluated: string[] = []
    const commands: any[] = []
    browser.send = async (method: string, params: any) => {
      if (method === "Target.getTargets") return { targetInfos: targets }
      if (method === "Target.attachToTarget") return { sessionId: params.targetId }
      if (method === "Target.sendMessageToTarget") {
        const message = JSON.parse(params.message)
        commands.push(message)
        if (message.method === "Runtime.enable") browser.emit("Target.receivedMessageFromTarget", { sessionId: params.sessionId,
          message: JSON.stringify({ method: "Runtime.executionContextCreated", params: { context: { id: 17 } } }) })
        if (message.method === "Runtime.evaluate") evaluated.push(params.sessionId)
        queueMicrotask(() => browser.emit("Target.receivedMessageFromTarget", { sessionId: params.sessionId,
          message: JSON.stringify({ id: message.id, result: message.method === "Profiler.stop" ? { profile: { samples: [1] } } : { result: { value: { clocks: [], tasks: [] } } } }) }))
      }
      return {}
    }
    const controller = await startWorkerCpuCapture(browser, { send: async () => ({ targetInfo: { browserContextId: "context" } }) } as any,
      { workers: () => targets.slice(0, 2).map(target => ({ url: () => target.url })) } as any)
    await controller.start()
    const captured = await controller.stop()
    await controller.close()
    expect(evaluated).toEqual(["game", "game"])
    expect(commands[0].method).toBe("Runtime.enable")
    expect(commands.filter(value => value.method === "Runtime.evaluate").map(value => value.params.contextId)).toEqual([17, 17])
    expect(captured.map(value => value.target.targetId)).toEqual(["game"])
    expect(controller.unsampledTargets.map(value => value.targetId)).toEqual(["helper"])
  })

  test("isolates nested sessions and joins out-of-order command IDs", async () => {
    const browser: any = new EventEmitter()
    const sent: any[] = []
    browser.send = async (method: string, params: any) => { sent.push({ method, params }); return {} }
    const session = new WorkerCdpSession(browser, "worker-1")
    const a = session.send("Profiler.start")
    const b = session.send("Profiler.stop")
    browser.emit("Target.receivedMessageFromTarget", { sessionId: "other-worker", message: JSON.stringify({ id: 1, result: { wrong: true } }) })
    browser.emit("Target.receivedMessageFromTarget", { sessionId: "worker-1", message: JSON.stringify({ id: 2, result: { profile: { samples: [7] } } }) })
    browser.emit("Target.receivedMessageFromTarget", { sessionId: "worker-1", message: JSON.stringify({ id: 1, result: {} }) })
    expect(await a).toEqual({})
    expect(await b).toEqual({ profile: { samples: [7] } })
    expect(JSON.parse(sent[0].params.message)).toEqual({ id: 1, method: "Profiler.start", params: {} })
    await session.close()
    expect(browser.listenerCount("Target.receivedMessageFromTarget")).toBe(0)
  })
  test("preserves Worker errors and rejects outstanding requests on close", async () => {
    const browser: any = new EventEmitter()
    browser.send = async () => ({})
    const session = new WorkerCdpSession(browser, "worker")
    const failed = session.send("Profiler.start")
    browser.emit("Target.receivedMessageFromTarget", { sessionId: "worker", message: JSON.stringify({ id: 1, error: { message: "cannot sample" } }) })
    await expect(failed).rejects.toThrow("cannot sample")
    const pending = session.send("Profiler.stop")
    const failure = pending.catch(error => error)
    await session.close()
    expect((await failure).message).toContain("session closed")
  })
})
