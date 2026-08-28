import { expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { runInNewContext } from "node:vm"

function device() {
  const capacity = 8192, buffer = new SharedArrayBuffer(32 + capacity * 8)
  const control = new Int32Array(buffer, 0, 8), samples = new Float32Array(buffer, 32)
  const messages: any[] = []
  let Processor: any
  runInNewContext(readFileSync(new URL("../src/output-worklet.js", import.meta.url), "utf8"), {
    SharedArrayBuffer, Int32Array, Float32Array, Int16Array, Atomics, sampleRate: 44100,
    AudioWorkletProcessor: class { port = { onmessage: undefined, postMessage: (value: unknown) => messages.push(value) } },
    registerProcessor(name: string, value: unknown) { expect(name).toBe("playsrc-output"); Processor = value },
  })
  const processor = new Processor({ processorOptions: { buffer, capacity } })
  const render = () => {
    const channels = [new Float32Array(128), new Float32Array(128)]
    expect(processor.process([], [channels])).toBe(true)
    return channels
  }
  return { control, samples, processor, messages, render, capacity }
}

test("the device preserves interleaved PCM, wraps counters and records only its own rendered output", () => {
  const d = device(), first = 0xffff_fff0
  Atomics.store(d.control, 0, first | 0); Atomics.store(d.control, 1, (first + 32) | 0); Atomics.store(d.control, 4, 1)
  for (let frame = 0; frame < 32; frame++) { const at = ((first + frame) & (d.capacity - 1)) * 2; d.samples[at] = frame / 32768; d.samples[at + 1] = -frame / 32768 }
  d.processor.port.onmessage({ data: { captureFrames: 32 } })
  const [left, right] = d.render()
  expect([...left!.subarray(0, 32)]).toEqual(Array.from({ length: 32 }, (_, index) => index / 32768))
  expect([...right!.subarray(0, 32)]).toEqual(Array.from({ length: 32 }, (_, index) => -index / 32768))
  expect([...left!.subarray(32)]).toEqual(new Array(96).fill(0))
  expect(Atomics.load(d.control, 0)).toBe(16)
  expect(Atomics.load(d.control, 3)).toBe(96)
  expect(d.messages[0].startRead).toBe(first)
  expect([...new Int16Array(d.messages[0].capture)]).toEqual(Array.from({ length: 32 }, (_, index) => [index, index === 0 ? 0 : -index]).flat())
})

test("an inactive device neither consumes queued samples nor counts underruns, and captures reject map replacement", () => {
  const d = device()
  d.samples.fill(0.5); Atomics.store(d.control, 1, 128)
  expect([...d.render()[0]!]).toEqual(new Array(128).fill(0))
  expect(Atomics.load(d.control, 0)).toBe(0)
  expect(Atomics.load(d.control, 3)).toBe(0)
  d.processor.port.onmessage({ data: { captureFrames: 128 } })
  Atomics.store(d.control, 2, 1)
  d.render()
  expect(d.messages[0].error).toBe("Audio capture crossed map ownership")
})

test("capture reports device gaps explicitly instead of losing the PCM on underrun", () => {
  const d = device()
  d.samples.fill(0.5); Atomics.store(d.control, 1, 32); Atomics.store(d.control, 4, 1)
  d.processor.port.onmessage({ data: { captureFrames: 128 } })
  d.render()
  expect(d.messages[0].underruns).toBe(96)
  expect([...new Uint32Array(d.messages[0].gaps, 0, d.messages[0].gapCount)]).toEqual([32, 96])
  expect([...new Int16Array(d.messages[0].capture).subarray(64)]).toEqual(new Array(192).fill(0))
})

test("the consumer acknowledges map retirement before queued storage is reusable", () => {
  const d = device()
  d.samples.fill(0.5); Atomics.store(d.control, 1, 512)
  Atomics.store(d.control, 2, 1)
  expect(Atomics.load(d.control, 6)).toBe(0)
  expect([...d.render()[0]!]).toEqual(new Array(128).fill(0))
  expect(Atomics.load(d.control, 0)).toBe(512)
  expect(Atomics.load(d.control, 6)).toBe(1)
  d.samples.fill(-0.25, 1024, 1280)
  Atomics.store(d.control, 1, 640); Atomics.store(d.control, 4, 1)
  expect([...d.render()[0]!]).toEqual(new Array(128).fill(-0.25))
  expect(Atomics.load(d.control, 0)).toBe(640)
})
