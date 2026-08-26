import { describe, expect, test } from "bun:test"
import { installWorkerTaskProfiler } from "../profile/worker-task-profiler"

function worker() {
  let at = 0
  const marks: string[] = []
  const sent: any[] = []
  const host: any = {
    performance: { timeOrigin: 1234, now: () => ++at, mark: (name: string) => marks.push(name), clearMarks() {} },
    onmessage(event: any) {
      host.__playsrcWorkerProfileMemory?.(64, 30, 40)
      host.postMessage({ id: event.data.id, kind: "simulation", output: event.data.output, timings: { transactMilliseconds: 131 } }, [event.data.output])
      host.__playsrcWorkerProfileMemory?.(128, 35, 42)
      return 42
    },
    postMessage(message: any, transfer: any[]) {
      sent.push(structuredClone(message, { transfer }))
    },
  }
  return { host, marks, sent }
}

describe("injected Worker task profiling", () => {
  test("retains task IDs, clocks, execution, transfer bytes and memory without retaining/detaching a second view", () => {
    const { host, marks, sent } = worker()
    const original = host.onmessage
    installWorkerTaskProfiler(host, "worker-1")
    const output = new ArrayBuffer(48)
    expect(host.onmessage({ data: { id: 7, kind: "observe", queuedAt: 2000, output } })).toBe(42)
    expect(output.byteLength).toBe(0)
    expect(sent[0].output.byteLength).toBe(48)
    const result = host.__playsrcWorkerTasks.stop()
    expect(result).toMatchObject({ timeOrigin: 1234, dropped: 0, tasks: [{ requestId: 7, kind: "observe", responses: [{ requestId: 7, bytes: 48, timings: { transactMilliseconds: 131 } }], memory: [{ linearBytes: 64 }, { linearBytes: 128 }] }] })
    expect(result.tasks[0].responses[0]).not.toHaveProperty("output")
    expect(result.tasks[0].responses[0].finished).toBeGreaterThan(result.tasks[0].responses[0].started)
    expect(marks).toContain("playsrc-worker-task:worker-1:1:7:start")
    expect(host.onmessage).toBe(original)
    expect(host.__playsrcWorkerProfileMemory).toBeUndefined()
  })

  test("bounds records without dropping or changing real Worker messages", () => {
    const { host, sent } = worker()
    installWorkerTaskProfiler(host)
    for (let id = 1; id <= 16_385; id++) host.onmessage({ data: { id, kind: "observe", output: new ArrayBuffer(1) } })
    const result = host.__playsrcWorkerTasks.stop()
    expect(result.tasks).toHaveLength(16_384)
    expect(result.dropped).toBe(1)
    expect(sent).toHaveLength(16_385)
  })

  test("preserves throws and closes the trace task", () => {
    const { host } = worker()
    host.onmessage = () => { throw new Error("original failure") }
    installWorkerTaskProfiler(host)
    expect(() => host.onmessage({ data: { id: 1, kind: "observe" } })).toThrow("original failure")
    expect(host.__playsrcWorkerTasks.stop().tasks[0].finished).toBeNumber()
  })

  test("injection is self-contained after serialization", () => {
    const { host } = worker()
    new Function("host", `(${installWorkerTaskProfiler.toString()})(host, 'serialized')`)(host)
    expect(host.__playsrcWorkerTasks.stop().clocks[0].name).toBe("playsrc-worker-clock:serialized:start")
  })
})
