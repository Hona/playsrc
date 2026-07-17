const MAX_RESOURCES = 4096
const MAX_VOICES = 128
const IDENTITY = /^[\x21-\x7e]{1,512}$/

export type AudioResource = Readonly<{
  identity: string
  buffer: AudioBuffer
}>

export type PlayRequest = Readonly<{
  voice: number
  resource: string
  gain: number
  pan: number
  loop: boolean
}>

type Voice = Readonly<{
  source: AudioBufferSourceNode
  gain: GainNode
  pan: StereoPannerNode
}>

export class AudioError extends Error {
  constructor(
    readonly code:
      | "MalformedResource"
      | "MissingResource"
      | "MalformedEvent"
      | "Capacity"
      | "Suspended"
      | "Closed"
      | "BrowserFailure",
    message: string,
  ) {
    super(message)
    this.name = "AudioError"
  }
}

function canonical(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => letter.toLowerCase())
}

function validBuffer(value: AudioBuffer): boolean {
  return typeof value === "object"
    && value !== null
    && Number.isSafeInteger(value.length)
    && value.length >= 0
    && Number.isSafeInteger(value.numberOfChannels)
    && value.numberOfChannels >= 1
    && Number.isFinite(value.sampleRate)
    && value.sampleRate > 0
    && Number.isFinite(value.duration)
    && value.duration >= 0
}

export function createAudioSystem(context: AudioContext, resources: readonly AudioResource[]): Readonly<{
  resume(): Promise<void>
  play(request: PlayRequest): void
  stop(voice: number): void
  reset(): void
  close(): Promise<void>
  activeVoices(): readonly number[]
}> {
  if (
    !context
    || typeof context.resume !== "function"
    || typeof context.close !== "function"
    || typeof context.createBufferSource !== "function"
    || typeof context.createGain !== "function"
    || typeof context.createStereoPanner !== "function"
    || !context.destination
  ) {
    throw new AudioError("BrowserFailure", "audio context interface is unavailable")
  }
  if (resources.length > MAX_RESOURCES) throw new AudioError("Capacity", "audio resource count exceeds its limit")
  const registry = new Map<string, AudioBuffer>()
  for (const resource of resources) {
    const identity = canonical(resource.identity)
    if (!IDENTITY.test(resource.identity) || !validBuffer(resource.buffer) || registry.has(identity)) {
      throw new AudioError("MalformedResource", "audio resource is invalid")
    }
    registry.set(identity, resource.buffer)
  }
  const voices = new Map<number, Voice>()
  let closed = false

  function stop(voice: number): void {
    if (!Number.isSafeInteger(voice) || voice < 1) {
      throw new AudioError("MalformedEvent", "audio voice identity is invalid")
    }
    const current = voices.get(voice)
    if (!current) return
    voices.delete(voice)
    current.source.onended = null
    try {
      current.source.stop()
    } catch {
      // A naturally ended source is already stopped; its graph still requires disconnection.
    }
    current.source.disconnect()
    current.gain.disconnect()
    current.pan.disconnect()
  }

  return Object.freeze({
    async resume(): Promise<void> {
      if (closed || context.state === "closed") throw new AudioError("Closed", "audio context is closed")
      try {
        await context.resume()
      } catch {
        throw new AudioError("BrowserFailure", "audio context resume failed")
      }
      if (context.state !== "running") throw new AudioError("Suspended", "audio context did not enter running state")
    },
    play(request: PlayRequest): void {
      if (closed || context.state === "closed") throw new AudioError("Closed", "audio context is closed")
      if (
        !request
        || !Number.isSafeInteger(request.voice)
        || request.voice < 1
        || !IDENTITY.test(request.resource)
        || !Number.isFinite(request.gain)
        || request.gain < 0
        || !Number.isFinite(request.pan)
        || request.pan < -1
        || request.pan > 1
      ) {
        throw new AudioError("MalformedEvent", "audio play request is invalid")
      }
      if (context.state !== "running") throw new AudioError("Suspended", "audio context is suspended")
      const buffer = registry.get(canonical(request.resource))
      if (!buffer) throw new AudioError("MissingResource", `audio resource ${request.resource} is missing`)
      if (!voices.has(request.voice) && voices.size >= MAX_VOICES) {
        throw new AudioError("Capacity", "active voice count exceeds its limit")
      }
      stop(request.voice)
      let source: AudioBufferSourceNode | undefined
      let gain: GainNode | undefined
      let pan: StereoPannerNode | undefined
      try {
        source = context.createBufferSource()
        gain = context.createGain()
        pan = context.createStereoPanner()
        source.buffer = buffer
        source.loop = request.loop
        gain.gain.value = request.gain
        pan.pan.value = request.pan
        source.connect(gain).connect(pan).connect(context.destination)
      } catch {
        source?.disconnect()
        gain?.disconnect()
        pan?.disconnect()
        throw new AudioError("BrowserFailure", "audio graph creation failed")
      }
      if (!source || !gain || !pan) throw new AudioError("BrowserFailure", "audio graph creation was incomplete")
      const voice = Object.freeze({ source, gain, pan })
      voices.set(request.voice, voice)
      source.onended = () => {
        if (voices.get(request.voice) !== voice) return
        voices.delete(request.voice)
        source.disconnect()
        gain.disconnect()
        pan.disconnect()
      }
      try {
        source.start()
      } catch {
        voices.delete(request.voice)
        source.onended = null
        source.disconnect()
        gain.disconnect()
        pan.disconnect()
        throw new AudioError("BrowserFailure", "audio source start failed")
      }
    },
    stop,
    reset(): void {
      for (const voice of [...voices.keys()]) stop(voice)
    },
    async close(): Promise<void> {
      if (closed) return
      closed = true
      for (const voice of [...voices.keys()]) stop(voice)
      try {
        if (context.state !== "closed") await context.close()
      } catch {
        throw new AudioError("BrowserFailure", "audio context close failed")
      }
    },
    activeVoices(): readonly number[] {
      return Object.freeze([...voices.keys()].sort((left, right) => left - right))
    },
  })
}
