const MAX_RESOURCES = 4096
const MAX_VOICES = 128
const IDENTITY = /^[\x21-\x7e]{1,512}$/

export * from "./source"

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
  nodes: readonly AudioNode[]
  stereo?: Readonly<{ left: AudioParam; right: AudioParam }>
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
  playNeutral(voice: import("./source").NeutralVoice): void
  updateNeutral(voice: import("./source").NeutralVoice): void
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
    for (const node of current.nodes) node.disconnect()
  }

  function requireRunning(): void {
    if (closed || context.state === "closed") throw new AudioError("Closed", "audio context is closed")
    if (context.state !== "running") throw new AudioError("Suspended", "audio context is suspended")
  }

  function requireCapacity(voice: number): void {
    if (!voices.has(voice) && voices.size >= MAX_VOICES) {
      throw new AudioError("Capacity", "active voice count exceeds its limit")
    }
  }

  function commit(voiceIdentity: number, source: AudioBufferSourceNode, nodes: readonly AudioNode[], start: () => void, stereo?: Voice["stereo"]): void {
    const voice = Object.freeze({ source, nodes: Object.freeze([...nodes]), stereo })
    source.onended = () => {
      if (voices.get(voiceIdentity) !== voice) return
      voices.delete(voiceIdentity)
      for (const node of nodes) node.disconnect()
    }
    try {
      start()
    } catch {
      source.onended = null
      for (const node of nodes) node.disconnect()
      throw new AudioError("BrowserFailure", "audio source start failed")
    }
    stop(voiceIdentity)
    voices.set(voiceIdentity, voice)
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
      requireRunning()
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
      const buffer = registry.get(canonical(request.resource))
      if (!buffer) throw new AudioError("MissingResource", `audio resource ${request.resource} is missing`)
      requireCapacity(request.voice)
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
      commit(request.voice, source, [source, gain, pan], () => source!.start())
    },
    playNeutral(voice): void {
      requireRunning()
      if (
        !voice
        || !Number.isSafeInteger(voice.identity)
        || voice.identity < 1
        || !IDENTITY.test(voice.resource)
        || !Number.isFinite(voice.leftGain)
        || voice.leftGain < 0
        || !Number.isFinite(voice.rightGain)
        || voice.rightGain < 0
        || !Number.isFinite(voice.playbackRate)
        || voice.playbackRate <= 0
        || !Number.isFinite(voice.startTimeSeconds)
        || !Number.isFinite(voice.offsetSeconds)
        || voice.offsetSeconds < 0
      ) {
        throw new AudioError("MalformedEvent", "neutral audio voice is invalid")
      }
      const buffer = registry.get(canonical(voice.resource))
      if (!buffer) throw new AudioError("MissingResource", `audio resource ${voice.resource} is missing`)
      if (
        typeof context.createChannelSplitter !== "function"
        || typeof context.createChannelMerger !== "function"
      ) {
        throw new AudioError("BrowserFailure", "channel-exact browser graph is unavailable")
      }
      requireCapacity(voice.identity)
      let source: AudioBufferSourceNode | undefined
      let stereo: Voice["stereo"]
      const nodes: AudioNode[] = []
      try {
        source = context.createBufferSource()
        nodes.push(source)
        const left = context.createGain()
        const right = context.createGain()
        stereo = Object.freeze({ left: left.gain, right: right.gain })
        const merger = context.createChannelMerger(2)
        nodes.push(left, right, merger)
        source.buffer = buffer
        source.playbackRate.value = voice.playbackRate
        source.loop = voice.loopStartSeconds !== null
        if (voice.loopStartSeconds !== null) source.loopStart = voice.loopStartSeconds
        const when = Math.max(context.currentTime, voice.startTimeSeconds)
        for (const [parameter, gain] of [[left.gain, voice.leftGain], [right.gain, voice.rightGain]] as const) {
          if (voice.envelope) {
            parameter.setValueAtTime(gain * voice.envelope.from, when)
            parameter.linearRampToValueAtTime(gain * voice.envelope.to, when + voice.envelope.seconds)
          } else parameter.value = gain
        }
        if (buffer.numberOfChannels === 1) {
          source.connect(left)
          source.connect(right)
        } else {
          const splitter = context.createChannelSplitter(2)
          nodes.push(splitter)
          source.connect(splitter)
          splitter.connect(left, 0)
          splitter.connect(right, 1)
        }
        left.connect(merger, 0, 0)
        right.connect(merger, 0, 1)
        merger.connect(context.destination)
      } catch {
        for (const node of nodes) node.disconnect()
        throw new AudioError("BrowserFailure", "neutral audio graph creation failed")
      }
      if (!source) throw new AudioError("BrowserFailure", "neutral audio graph creation was incomplete")
      const when = Math.max(context.currentTime, voice.startTimeSeconds)
      commit(voice.identity, source, nodes, () => source!.start(when, voice.offsetSeconds), stereo)
    },
    updateNeutral(voice): void {
      requireRunning()
      if (!Number.isSafeInteger(voice.identity) || voice.identity < 1 || ![voice.leftGain, voice.rightGain].every(value => Number.isFinite(value) && value >= 0)) throw new AudioError("MalformedEvent", "neutral audio update is invalid")
      const current = voices.get(voice.identity)
      if (!current) return
      if (!current.stereo) throw new AudioError("MalformedEvent", "neutral audio update targets a different graph")
      const now = context.currentTime
      for (const [parameter, gain] of [[current.stereo.left, voice.leftGain], [current.stereo.right, voice.rightGain]] as const) {
        parameter.cancelScheduledValues(now)
        if (voice.envelope) {
          const progress = Math.max(0, Math.min(1, (now - voice.startTimeSeconds) / voice.envelope.seconds))
          parameter.setValueAtTime(gain * (voice.envelope.from + (voice.envelope.to - voice.envelope.from) * progress), now)
          if (progress < 1) parameter.linearRampToValueAtTime(gain * voice.envelope.to, voice.startTimeSeconds + voice.envelope.seconds)
        } else parameter.setValueAtTime(gain, now)
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
