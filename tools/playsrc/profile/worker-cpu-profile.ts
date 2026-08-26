import type { CDPSession } from "@playwright/test"

export async function startGameplayWorkerProfile(browser: CDPSession) {
  const { targetInfos } = await browser.send("Target.getTargets")
  const target = targetInfos.find(target => target.type === "worker" && target.url.includes("gameplay-worker"))
  if (!target) throw new Error("Gameplay Worker CPU profiling target is absent")
  const { sessionId } = await browser.send("Target.attachToTarget", { targetId: target.targetId, flatten: false })
  let sequence = 0
  const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>()
  const receive = (event: { sessionId: string; message: string }) => {
    if (event.sessionId !== sessionId) return
    const message = JSON.parse(event.message)
    const request = pending.get(message.id)
    if (!request) return
    pending.delete(message.id)
    if (message.error) request.reject(new Error(message.error.message))
    else request.resolve(message.result)
  }
  browser.on("Target.receivedMessageFromTarget", receive)
  const send = async (method: string, params = {}) => {
    const id = ++sequence
    const result = new Promise<any>((resolve, reject) => { pending.set(id, { resolve, reject }) })
    await browser.send("Target.sendMessageToTarget", { sessionId, message: JSON.stringify({ id, method, params }) })
    return result
  }
  await send("Profiler.enable")
  await send("Profiler.setSamplingInterval", { interval: 1000 })
  await send("Profiler.start")
  return async () => {
    try { return (await send("Profiler.stop")).profile }
    finally {
      browser.off("Target.receivedMessageFromTarget", receive)
      await browser.send("Target.detachFromTarget", { sessionId })
    }
  }
}
