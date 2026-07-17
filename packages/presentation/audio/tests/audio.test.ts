import { describe, expect, test } from "bun:test"
import { AudioError, createAudioSystem } from "../src"

class Parameter { value = 0 }
class Node {
  onended: (() => void) | null = null
  buffer?: AudioBuffer
  loop = false
  readonly gain = new Parameter()
  readonly pan = new Parameter()
  starts = 0
  stops = 0
  connect(): this { return this }
  disconnect(): void {}
  start(): void { this.starts += 1 }
  stop(): void { this.stops += 1 }
}

test("audio refuses missing resources without creating a source", () => {
  let sourceCalls = 0
  const context = {
    state: "running",
    destination: new Node(),
    createBufferSource: () => { sourceCalls += 1; return new Node() },
    createGain: () => new Node(),
    createStereoPanner: () => new Node(),
    resume: async () => {},
    close: async () => {},
  } as unknown as AudioContext
  const audio = createAudioSystem(context, [])
  expect(() => audio.play({ voice: 1, resource: "missing.wav", gain: 1, pan: 0, loop: false }))
    .toThrow(AudioError)
  expect(sourceCalls).toBe(0)
  expect(audio.activeVoices()).toEqual([])
})

describe("browser audio graph", () => {
  test("starts, replaces, stops, and closes supplied exact buffers", async () => {
    const sources: Node[] = []
    const buffer = {
      length: 1,
      numberOfChannels: 1,
      sampleRate: 44_100,
      duration: 1 / 44_100,
    } as AudioBuffer
    const context = {
      state: "suspended",
      destination: new Node(),
      createBufferSource: () => { const value = new Node(); sources.push(value); return value },
      createGain: () => new Node(),
      createStereoPanner: () => new Node(),
      resume: async function () { this.state = "running" },
      close: async function () { this.state = "closed" },
    } as unknown as AudioContext
    const audio = createAudioSystem(context, [{ identity: "sound/test.wav", buffer }])
    await audio.resume()
    audio.play({ voice: 2, resource: "SOUND/TEST.WAV", gain: 0.5, pan: -0.25, loop: false })
    audio.play({ voice: 2, resource: "sound/test.wav", gain: 1, pan: 0, loop: true })
    expect(sources).toHaveLength(2)
    expect(sources[0]!.stops).toBe(1)
    expect(audio.activeVoices()).toEqual([2])
    audio.stop(2)
    expect(audio.activeVoices()).toEqual([])
    await audio.close()
    await audio.close()
    expect(context.state).toBe("closed")
  })
})
