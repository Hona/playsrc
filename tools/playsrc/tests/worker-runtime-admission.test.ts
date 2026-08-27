import { expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import { admitWorkerExecutionContext } from "../profile/worker-runtime-admission"

function transport() {
  const events = new EventEmitter()
  let enable!: () => void
  const commands: string[] = []
  const runtime = Object.assign(events, { send(method: string) { commands.push(method); return new Promise<void>(resolve => { enable = resolve }) } })
  return { runtime, commands, enable: () => enable(), created: (id: number) => runtime.emit("Runtime.executionContextCreated", { context: { id } }) }
}

test("admission joins Runtime.enable and its context event in either order", async () => {
  for (const contextFirst of [true, false]) {
    const t = transport(), ready = admitWorkerExecutionContext(t.runtime)
    let settled = false
    void ready.then(() => { settled = true })
    if (contextFirst) t.created(7); else t.enable()
    await Promise.resolve()
    expect(settled).toBe(false)
    if (contextFirst) t.enable(); else t.created(7)
    expect(await ready).toBe(7)
    expect(t.commands).toEqual(["Runtime.enable"])
    expect(t.runtime.eventNames()).toEqual([])
  }
})

test("isolated worlds cannot admit default Worker evaluation", async () => {
  const t = transport(), ready = admitWorkerExecutionContext(t.runtime)
  t.runtime.emit("Runtime.executionContextCreated", { context: { id: 2, auxData: { isDefault: false } } })
  t.created(9); t.enable()
  expect(await ready).toBe(9)
})

test("closed, crashed and invalidated contexts reject without fallback", async () => {
  for (const [event, value] of [
    ["Inspector.detached", {}], ["Inspector.targetCrashed", {}],
    ["Runtime.executionContextsCleared", {}], ["Runtime.executionContextDestroyed", { executionContextId: 4 }],
    ["Runtime.executionContextCreated", { context: { id: 5 } }],
  ] as const) {
    const t = transport(), ready = admitWorkerExecutionContext(t.runtime)
    t.created(4); t.runtime.emit(event, value)
    await expect(ready).rejects.toThrow()
    expect(t.runtime.eventNames()).toEqual([])
    t.enable()
  }
})

test("missing context has a finite deadline and removes listeners", async () => {
  const t = transport(), ready = admitWorkerExecutionContext(t.runtime, 5)
  t.enable()
  await expect(ready).rejects.toThrow("deadline")
  expect(t.runtime.eventNames()).toEqual([])
})

test("Runtime.enable errors remain errors even after a context event", async () => {
  const t = transport()
  t.runtime.send = () => { t.created(3); return Promise.reject(new Error("transport failed")) }
  await expect(admitWorkerExecutionContext(t.runtime)).rejects.toThrow("transport failed")
  expect(t.runtime.eventNames()).toEqual([])
})

test("serialized native controller uses the same context gate", async () => {
  const native = eval(`(${admitWorkerExecutionContext.toString()})`) as typeof admitWorkerExecutionContext
  const t = transport(), ready = native(t.runtime)
  t.created(11); t.enable()
  expect(await ready).toBe(11)
})

test("synchronous transport failures also remove the admission listeners", async () => {
  const t = transport()
  t.runtime.send = () => { throw new Error("closed socket") }
  await expect(admitWorkerExecutionContext(t.runtime)).rejects.toThrow("closed socket")
  expect(t.runtime.eventNames()).toEqual([])
})
