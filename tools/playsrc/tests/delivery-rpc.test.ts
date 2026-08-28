import { expect, test } from "bun:test"
import { installDeliveryRpcObserver } from "../profile/delivery-rpc"
test("RPC diagnosis keeps the real send/reply contract and never retains payloads", () => {
  let now = 0
  const sent: any[] = []
  const host: any = { performance: { now: () => now }, Worker: class { postMessage(...args: any[]) { sent.push(args) } } }
  installDeliveryRpcObserver(host)
  const worker = new host.Worker("/gameplay-worker.ts", { type: "module" })
  const command = { id: 1, kind: "observe", bytes: new ArrayBuffer(16) }, transfer = [command.bytes]
  worker.postMessage(command, transfer)
  now = 2; host.__playsrcDeliveryRpc.start(now)
  now = 5; worker.__playsrcProfileReply({ id: 1, output: new ArrayBuffer(1000), timings: { queueMilliseconds: 1, totalMilliseconds: 3 } })
  const result = host.__playsrcDeliveryRpc.stop()
  expect(sent[0]).toEqual([command, transfer])
  expect(result.records).toHaveLength(1)
  expect(result.records[0]).toMatchObject({ kind: "observe", sent: 0, received: 5, elapsedMilliseconds: 5, censoredStart: true })
  expect(result.records[0]).not.toHaveProperty("output")
  expect(result.records[0]).not.toHaveProperty("bytes")
  expect(host.__playsrcProfile).toBeUndefined()
  expect(host.__playsrcFrameProfiler).toBeUndefined()
})
