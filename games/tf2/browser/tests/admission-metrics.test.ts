import { expect, test } from "bun:test"
import { decodeAdmissionMetrics } from "../src/admission-metrics"

test("diagnostic wire preserves actor tick clocks and allocation counters above four GiB", () => {
  const bytes = new DataView(new ArrayBuffer(56))
  bytes.setUint32(0, 2, true); bytes.setUint32(4, 24, true)
  const values = [375n, 1234567890n, 1200000n, 300n, 9000000000n, 4096n]
  values.forEach((value, index) => bytes.setBigUint64(8 + index * 8, value, true))
  expect(decodeAdmissionMetrics(bytes)).toEqual([{ stage: 2, actor: 24, tick: 375, at: 1234.56789, heapBytes: 1200000, allocations: 300, allocatedBytes: 9000000000, value: 4096 }])
  bytes.setBigUint64(48, BigInt(Number.MAX_SAFE_INTEGER) + 1n, true)
  expect(() => decodeAdmissionMetrics(bytes)).toThrow("not exact")
})

test("truncated or oversized admission records cannot masquerade as complete evidence", () => {
  expect(() => decodeAdmissionMetrics(new DataView(new ArrayBuffer(55)))).toThrow("bound")
  expect(() => decodeAdmissionMetrics(new DataView(new ArrayBuffer(56 * 8193)))).toThrow("bound")
  expect(() => decodeAdmissionMetrics(new DataView(new ArrayBuffer(56)))).toThrow("stage")
  expect(decodeAdmissionMetrics(new DataView(new ArrayBuffer(0)))).toEqual([])
})
