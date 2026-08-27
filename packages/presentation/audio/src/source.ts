export type SoundScriptNode = Readonly<{
  key: string
  value: string | readonly SoundScriptNode[]
}>

export type SoundScriptDocument = Readonly<{
  logicalPath: string
  mode: "base" | "override"
  preload: boolean
  entries: readonly SoundScriptNode[]
}>

export type Interval = Readonly<{ start: number; range: number }>

export type SoundDecorator =
  | "stream"
  | "voice"
  | "sentence"
  | "dry"
  | "doppler"
  | "directional"
  | "distance-variant"
  | "omni"
  | "spatial-stereo"
  | "fast-pitch"

export type SoundWave = Readonly<{
  token: string
  undecorated: string
  resource: string | null
  decorators: readonly SoundDecorator[]
}>

export type SoundDefinition = Readonly<{
  identity: string
  source: string
  override: boolean
  preload: boolean
  channel: number
  volume: Interval
  pitch: Interval
  soundLevel: Interval
  delayMilliseconds: number
  ownerOnly: boolean
  waves: readonly SoundWave[]
  unknownFields: readonly string[]
}>

export type RegistryLimits = Readonly<{
  maxDocuments: number
  maxDefinitions: number
  maxWavesPerDefinition: number
}>

export type SoundSamples = Readonly<{
  volume: number
  pitch: number
  wave: number
  soundLevel: number
}>

export type Listener = Readonly<{
  identity: number
  revision: number
  origin: readonly [number, number, number]
  forward: readonly [number, number, number]
  right: readonly [number, number, number]
  masterGain: number
  categoryGain: number
  muted: boolean
}>

export type SoundSource = Readonly<{
  kind: "world" | "entity" | "local-listener" | "ui"
  identity: number
  ownerIdentity: number | null
  origin: readonly [number, number, number] | null
  radius: number
  sourceClass: string
}>

export type StartSound = Readonly<{
  envelope?: Readonly<{ from: number; to: number; seconds: number }>
  voiceIdentity: number
  definition: string
  source: SoundSource
  listener: Listener
  samples: SoundSamples
  overrides?: Readonly<{ volume?: number; pitch?: number }>
  resourceDurationSeconds: number
  resourceLoopStartSeconds: number | null
  resourceChannels: number
  resourceAvailable(resource: string): boolean
  scheduledTimeSeconds: number
  delaySeconds: number
  mixerGain: number
  userGain: number
  doNotOverwrite: boolean
}>

export type NeutralVoice = Readonly<{
  envelope?: Readonly<{ from: number; to: number; seconds: number }>
  identity: number
  definition: string
  wave: number
  resource: string
  decorators: readonly SoundDecorator[]
  sourceIdentity: number
  channel: number
  volume: number
  pitch: number
  playbackRate: number
  soundLevel: number
  startTimeSeconds: number
  offsetSeconds: number
  loopStartSeconds: number | null
  leftGain: number
  rightGain: number
  sourceDistance: number
  distanceGain: number
  mixerGain: number
  dsp: "dry" | "room"
  listenerRevision: number
}>

export type SourceAudioLimits = Readonly<{ maxActiveVoices: number }>

export type MixerRule = Readonly<{
  group: string
  pathSubstring: string
  sourceClassSubstring: string
  channel: number | null
  minimumLevel: number | null
  maximumLevel: number | null
  priority: number
  ducked: boolean
  causesDucking: boolean
  duckTarget: number
  duckerThreshold: number
}>

export type Mixer = Readonly<{
  identity: string
  gains: ReadonlyMap<string, number>
}>

export class MixerRegistry {
  readonly #rules: readonly MixerRule[]
  readonly #mixers: ReadonlyMap<string, Mixer>

  constructor(rules: readonly MixerRule[], mixers: readonly Mixer[]) {
    if (rules.length > 80 || mixers.length > 32) throw error("MalformedDocument", "mixer registry exceeds Source limits")
    const groups = new Set<string>()
    for (const rule of rules) {
      if (!identity(rule.group) || rule.group.length > 31 || rule.pathSubstring.length > 31
        || rule.sourceClassSubstring.length > 31 || !Number.isInteger(rule.priority)
        || rule.priority < 0 || rule.priority > 100 || !nonnegative(rule.duckTarget)
        || !nonnegative(rule.duckerThreshold)) {
        throw error("MalformedDocument", "mixer rule is malformed")
      }
      groups.add(canonical(rule.group))
    }
    if (groups.size > 64) throw error("MalformedDocument", "mixer group count exceeds Source limits")
    const byName = new Map<string, Mixer>()
    for (const mixer of mixers) {
      if (!identity(mixer.identity) || byName.has(canonical(mixer.identity))) {
        throw error("MalformedDocument", "mixer identity is malformed or duplicated")
      }
      const gains = new Map<string, number>()
      for (const [group, gain] of mixer.gains) {
        if (!groups.has(canonical(group)) || !nonnegative(gain)) {
          throw error("MalformedDocument", "mixer gain references an unknown group")
        }
        gains.set(canonical(group), gain)
      }
      byName.set(canonical(mixer.identity), Object.freeze({ identity: mixer.identity, gains }))
    }
    this.#rules = Object.freeze([...rules])
    this.#mixers = byName
  }

  resolve(
    mixerIdentity: string,
    resource: string,
    sourceClass: string,
    channel: number,
    soundLevel: number,
    duckGains: ReadonlyMap<string, number> = new Map(),
  ): Readonly<{ groups: readonly string[]; selectedGroup: string | null; gain: number }> {
    const mixer = this.#mixers.get(canonical(mixerIdentity))
    if (!mixer) throw error("MalformedEvent", `sound mixer ${mixerIdentity} is missing`)
    const groups: string[] = []
    for (const rule of this.#rules) {
      if (rule.pathSubstring && !canonical(resource).includes(canonical(rule.pathSubstring))) continue
      if (rule.sourceClassSubstring && !canonical(sourceClass).includes(canonical(rule.sourceClassSubstring))) continue
      if (rule.channel !== null && rule.channel !== channel) continue
      if (rule.minimumLevel !== null && soundLevel < rule.minimumLevel) continue
      if (rule.maximumLevel !== null && soundLevel > rule.maximumLevel) continue
      groups.push(rule.group)
      if (groups.length === 8) break
    }
    let duck = 1
    for (const group of groups) duck = Math.min(duck, duckGains.get(canonical(group)) ?? 1)
    for (const group of groups) {
      const gain = mixer.gains.get(canonical(group))
      if (gain !== undefined) return Object.freeze({ groups: Object.freeze(groups), selectedGroup: group, gain: gain * duck })
    }
    return Object.freeze({ groups: Object.freeze(groups), selectedGroup: null, gain: duck })
  }
}

const DEFAULT_REGISTRY_LIMITS: RegistryLimits = Object.freeze({
  maxDocuments: 256,
  maxDefinitions: 65_534,
  maxWavesPerDefinition: 1_024,
})

const CHANNELS = new Map<string, number>([
  ["chan_replace", -1],
  ["chan_auto", 0],
  ["chan_weapon", 1],
  ["chan_voice", 2],
  ["chan_item", 3],
  ["chan_body", 4],
  ["chan_stream", 5],
  ["chan_static", 6],
  ["chan_voice2", 7],
])

const LEVELS = new Map<string, number>([
  ["sndlvl_none", 0], ["sndlvl_20db", 20], ["sndlvl_25db", 25],
  ["sndlvl_30db", 30], ["sndlvl_35db", 35], ["sndlvl_40db", 40],
  ["sndlvl_45db", 45], ["sndlvl_50db", 50], ["sndlvl_55db", 55],
  ["sndlvl_idle", 60], ["sndlvl_60db", 60], ["sndlvl_65db", 65],
  ["sndlvl_static", 66], ["sndlvl_70db", 70], ["sndlvl_norm", 75],
  ["sndlvl_75db", 75], ["sndlvl_talking", 80], ["sndlvl_80db", 80],
  ["sndlvl_85db", 85], ["sndlvl_90db", 90], ["sndlvl_95db", 95],
  ["sndlvl_100db", 100], ["sndlvl_105db", 105], ["sndlvl_110db", 110],
  ["sndlvl_120db", 120], ["sndlvl_130db", 130], ["sndlvl_gunfire", 140],
  ["sndlvl_140db", 140], ["sndlvl_150db", 150], ["sndlvl_180db", 180],
])

const ATTENUATIONS = new Map<string, number>([
  ["attn_none", 0], ["attn_norm", 0.8], ["attn_idle", 2],
  ["attn_static", 1.25], ["attn_ricochet", 1.5], ["attn_gunfire", 0.27],
])

const DECORATORS = new Map<string, SoundDecorator>([
  ["*", "stream"], ["?", "voice"], ["!", "sentence"], ["#", "dry"],
  [">", "doppler"], ["<", "directional"], ["^", "distance-variant"],
  ["@", "omni"], [")", "spatial-stereo"], ["}", "fast-pitch"],
])

export class SourceAudioError extends Error {
  constructor(
    readonly code:
      | "MalformedDocument"
      | "MalformedDefinition"
      | "MissingDefinition"
      | "MissingResource"
      | "MalformedEvent"
      | "Audience"
      | "Capacity"
      | "Suppressed",
    message: string,
  ) {
    super(message)
    this.name = "SourceAudioError"
  }
}

export class SoundRegistry {
  readonly #definitions = new Map<string, SoundDefinition>()
  readonly #savedBase = new Map<string, SoundDefinition>()

  constructor(
    documents: readonly SoundScriptDocument[],
    limits: RegistryLimits = DEFAULT_REGISTRY_LIMITS,
  ) {
    validateRegistryLimits(limits)
    if (documents.length > limits.maxDocuments) throw error("MalformedDocument", "document count exceeds its limit")
    for (const document of documents) {
      if (!logicalPath(document.logicalPath)) throw error("MalformedDocument", "sound document path is malformed")
      for (const entry of document.entries) {
        if (this.#definitions.size >= limits.maxDefinitions) throw error("MalformedDocument", "definition count exceeds its limit")
        const key = canonical(entry.key)
        if (!identity(entry.key) || !Array.isArray(entry.value)) throw error("MalformedDocument", "sound entry is malformed")
        if (document.mode === "base" && this.#definitions.has(key)) continue
        const definition = parseDefinition(entry, document, limits)
        const previous = this.#definitions.get(key)
        if (document.mode === "override" && previous && !previous.override && !this.#savedBase.has(key)) {
          this.#savedBase.set(key, previous)
        }
        this.#definitions.set(key, definition)
      }
    }
  }

  get(name: string): SoundDefinition | undefined {
    return this.#definitions.get(canonical(name))
  }

  definitions(): readonly SoundDefinition[] {
    return Object.freeze([...this.#definitions.values()])
  }

  resources(): readonly string[] {
    const resources = new Set<string>()
    for (const definition of this.#definitions.values()) {
      for (const wave of definition.waves) if (wave.resource !== null) resources.add(wave.resource)
    }
    return Object.freeze([...resources])
  }

  clearOverrides(): void {
    for (const [key, value] of this.#definitions) {
      if (value.override) this.#definitions.delete(key)
    }
    for (const [key, value] of this.#savedBase) this.#definitions.set(key, value)
    this.#savedBase.clear()
  }
}

export class SourceAudioWorld {
  readonly #registry: SoundRegistry
  readonly #limits: SourceAudioLimits
  readonly #voices = new Map<number, NeutralVoice>()
  readonly #spatialInputs = new Map<number, Readonly<{ source: SoundSource; userGain: number }>>()

  constructor(registry: SoundRegistry, limits: SourceAudioLimits) {
    if (!positive(limits.maxActiveVoices)) throw error("Capacity", "voice limit must be positive")
    this.#registry = registry
    this.#limits = limits
  }

  start(event: StartSound): Readonly<{ voice: NeutralVoice; replaced: readonly number[] }> {
    validateStart(event)
    const definition = this.#registry.get(event.definition)
    if (!definition) throw error("MissingDefinition", `sound ${event.definition} is missing`)
    if (definition.ownerOnly && event.source.ownerIdentity !== event.listener.identity) {
      throw error("Audience", "owner-only sound does not target the listener")
    }
    const selected = definition.waves[event.samples.wave]
    if (!selected) throw error("MalformedEvent", "selected sound wave ordinal is outside its definition")
    if (selected.resource === null || !event.resourceAvailable(selected.resource)) {
      throw error("MissingResource", "selected sound wave resource is unavailable")
    }
    const resource = selected.resource
    const volume = event.overrides?.volume ?? sampleInterval(definition.volume, event.samples.volume)
    const pitch = Math.trunc(event.overrides?.pitch ?? sampleInterval(definition.pitch, event.samples.pitch))
    const soundLevel = Math.trunc(sampleInterval(definition.soundLevel, event.samples.soundLevel))
    if (!Number.isFinite(volume)||volume<0||volume>1||pitch <= 0 || pitch > 255 || soundLevel < 0 || soundLevel > 511) {
      throw error("MalformedEvent", "resolved pitch or sound level is outside its encoded range")
    }
    const channel = selected.decorators.includes("stream") ? 5 : definition.channel
    const replaced = this.#replacement(channel, event)
    const looped = event.resourceLoopStartSeconds !== null
    this.#enforceConcurrency(resource, looped, event, replaced)
    const retained = this.#voices.size - replaced.length
    if (retained >= this.#limits.maxActiveVoices) throw error("Capacity", "active voice count exceeds its limit")
    const spatial = spatialGains(event.source, event.listener, soundLevel)
    const offsetSeconds = event.delaySeconds < 0
      ? looped
        ? (-event.delaySeconds) % event.resourceDurationSeconds
        : -event.delaySeconds
      : 0
    if (!looped && offsetSeconds >= event.resourceDurationSeconds) {
      throw error("Suppressed", "negative delay skips the complete non-looping resource")
    }
    const mixerGain = event.mixerGain
    const finalGain = volume * event.userGain * event.listener.masterGain * event.listener.categoryGain
      * (event.listener.muted ? 0 : 1)
    const voice = Object.freeze({
      ...(event.envelope ? { envelope: event.envelope } : {}),
      identity: event.voiceIdentity,
      definition: definition.identity,
      wave: event.samples.wave,
      resource,
      decorators: selected.decorators,
      sourceIdentity: event.source.identity,
      channel,
      volume,
      pitch,
      playbackRate: pitch / 100,
      soundLevel,
      startTimeSeconds: event.scheduledTimeSeconds + Math.max(0, event.delaySeconds)
        + definition.delayMilliseconds / 1_000,
      offsetSeconds,
      loopStartSeconds: event.resourceLoopStartSeconds,
      leftGain: finalGain * spatial.left * mixerGain,
      rightGain: finalGain * spatial.right * mixerGain,
      sourceDistance: spatial.sourceDistance,
      distanceGain: spatial.distance,
      mixerGain,
      dsp: selected.decorators.includes("dry") ? "dry" as const : "room" as const,
      listenerRevision: event.listener.revision,
    })
    for (const identity of replaced) this.stop(identity)
    this.#voices.set(voice.identity, voice)
    this.#spatialInputs.set(voice.identity, Object.freeze({ source: event.source, userGain: event.userGain }))
    return Object.freeze({ voice, replaced: Object.freeze(replaced) })
  }

  stop(identity: number): boolean {
    if (!uint(identity)) throw error("MalformedEvent", "voice identity is invalid")
    this.#spatialInputs.delete(identity)
    return this.#voices.delete(identity)
  }

  stopSourceChannel(sourceIdentity: number, channel: number): readonly number[] {
    if (!uint(sourceIdentity) || !Number.isInteger(channel)) throw error("MalformedEvent", "stop key is invalid")
    const stopped: number[] = []
    for (const [identity, voice] of this.#voices) {
      if (voice.sourceIdentity === sourceIdentity && (channel === -1 || voice.channel === channel)) {
        this.stop(identity)
        stopped.push(identity)
      }
    }
    return Object.freeze(stopped)
  }

  stopDefinition(sourceIdentity: number, definition: string): readonly number[] {
    if (!uint(sourceIdentity)) throw error("MalformedEvent", "stop source is invalid")
    const stopped: number[] = []
    for (const [identity, voice] of this.#voices) {
      if (voice.sourceIdentity === sourceIdentity && canonical(voice.definition) === canonical(definition)) {
        this.stop(identity)
        stopped.push(identity)
      }
    }
    return Object.freeze(stopped)
  }

  reset(): readonly number[] {
    const stopped = [...this.#voices.keys()]
    this.#voices.clear()
    this.#spatialInputs.clear()
    return Object.freeze(stopped)
  }

  voices(): readonly NeutralVoice[] {
    return Object.freeze([...this.#voices.values()])
  }

  refreshSpatial(listener: Listener, locate: (source: SoundSource) => SoundSource["origin"] | undefined): readonly NeutralVoice[] {
    if (!validListener(listener)) throw error("MalformedEvent", "sound listener is invalid")
    const changed: NeutralVoice[] = []
    for (const [identity, voice] of this.#voices) {
      const input = this.#spatialInputs.get(identity)!
      const located = locate(input.source)
      if (located !== undefined && located !== null && !vector(located)) throw error("MalformedEvent", "sound source position is invalid")
      let source = input.source
      if (located !== undefined && (located === null ? source.origin !== null : source.origin === null || located.some((value, axis) => value !== source.origin![axis]))) {
        source = Object.freeze({ ...source, origin: located })
        this.#spatialInputs.set(identity, Object.freeze({ ...input, source }))
      }
      const spatial = spatialGains(source, listener, voice.soundLevel)
      const gain = voice.volume * input.userGain * listener.masterGain * listener.categoryGain * (listener.muted ? 0 : 1) * voice.mixerGain
      const leftGain = gain * spatial.left, rightGain = gain * spatial.right
      if (leftGain === voice.leftGain && rightGain === voice.rightGain && spatial.sourceDistance === voice.sourceDistance && spatial.distance === voice.distanceGain) continue
      const next = Object.freeze({ ...voice, leftGain, rightGain, sourceDistance: spatial.sourceDistance, distanceGain: spatial.distance, listenerRevision: listener.revision })
      this.#voices.set(identity, next); changed.push(next)
    }
    return Object.freeze(changed)
  }

  #replacement(channel: number, event: StartSound): number[] {
    if (channel === 0 || channel === 6) return []
    const matches = [...this.#voices.values()].filter((voice) =>
      voice.sourceIdentity === event.source.identity
      && (channel === -1 ? ![2, 5, 7].includes(voice.channel) : voice.channel === channel))
    if (matches.length > 0 && event.doNotOverwrite) throw error("Suppressed", "matching channel is protected")
    return matches.map(({ identity }) => identity)
  }

  #enforceConcurrency(resource: string, looped: boolean, event: StartSound, replaced: number[]): void {
    const active = [...this.#voices.values()].filter((voice) =>
      voice.resource === resource && !replaced.includes(voice.identity))
    const limit = looped ? 3 : 4
    if (active.length < limit) return
    const quietest = active.reduce((left, right) =>
      Math.max(left.leftGain, left.rightGain) <= Math.max(right.leftGain, right.rightGain) ? left : right)
    const nextDistance = event.source.origin ? distance(event.source.origin, event.listener.origin) : 0
    if (event.source.origin && nextDistance > quietest.sourceDistance && quietest.channel !== 1) {
      throw error("Suppressed", "farther duplicate sound is suppressed")
    }
    replaced.push(quietest.identity)
  }
}

export function sourceDistanceGain(soundLevel: number, distanceUnits: number): number {
  if (!Number.isFinite(distanceUnits) || distanceUnits < 0 || !Number.isInteger(soundLevel)) {
    throw error("MalformedEvent", "distance gain inputs are invalid")
  }
  if (soundLevel >= 256) {
    const decoded = soundLevel - 256
    const attenuation = decoded > 50 ? 20 / (decoded - 50) : 4
    return Math.max(0, Math.min(1, 1 - distanceUnits * attenuation / 1_000))
  }
  if (soundLevel === 0) return 1
  const distanceMultiplier = (10 ** (60 / 20) / 10 ** (soundLevel / 20)) / 36
  const foliageLoss = 4 * (distanceUnits / 1_200)
  const relativeDistance = distanceUnits * distanceMultiplier * 10 ** (foliageLoss / 20)
  let gain = relativeDistance > 0.1 ? 1 / relativeDistance : 10
  if (gain > 0.5) {
    const power = soundLevel > 90 ? mix(2.5, 0.8, (soundLevel - 90) / (140 - 90)) : 2.5
    const crossover = -1 / (0.5 ** power * (0.5 - 1))
    gain = 1 - 1 / (crossover * gain ** power)
  }
  if (gain < 0.01) gain = Math.max(0.001, 0.01 * (2 - relativeDistance * 0.01))
  return gain
}

function parseDefinition(
  entry: SoundScriptNode,
  document: SoundScriptDocument,
  limits: RegistryLimits,
): SoundDefinition {
  const children = entry.value as readonly SoundScriptNode[]
  let channel = 0
  let volume = interval(1, 0)
  let pitch = interval(100, 0)
  let soundLevel = interval(75, 0)
  let delayMilliseconds = 0
  let ownerOnly = false
  const waves: SoundWave[] = []
  const unknownFields: string[] = []
  for (const child of children) {
    const key = canonical(child.key)
    if (key === "rndwave") {
      if (!Array.isArray(child.value)) throw error("MalformedDefinition", "rndwave must be an object")
      for (const wave of child.value) {
        if (canonical(wave.key) !== "wave" || typeof wave.value !== "string") {
          throw error("MalformedDefinition", "rndwave contains a non-wave entry")
        }
        waves.push(parseWave(wave.value))
      }
      continue
    }
    if (typeof child.value !== "string") throw error("MalformedDefinition", `${child.key} must be scalar`)
    if (key === "channel") channel = parseChannel(child.value)
    else if (key === "volume") volume = parseVolume(child.value)
    else if (key === "pitch") pitch = parsePitch(child.value)
    else if (key === "soundlevel") soundLevel = parseSoundLevel(child.value)
    else if (key === "attenuation" || key === "compatibilityattenuation") {
      soundLevel = parseAttenuation(child.value, key === "compatibilityattenuation")
    } else if (key === "wave") waves.push(parseWave(child.value))
    else if (key === "delay_msec") delayMilliseconds = Math.max(0, strictInteger(child.value))
    else if (key === "play_to_owner_only") ownerOnly = strictInteger(child.value) !== 0
    else unknownFields.push(child.key)
  }
  if (waves.length === 0 || waves.length > limits.maxWavesPerDefinition) {
    throw error("MalformedDefinition", "sound wave count is invalid")
  }
  return Object.freeze({
    identity: entry.key,
    source: document.logicalPath,
    override: document.mode === "override",
    preload: document.preload,
    channel,
    volume,
    pitch,
    soundLevel,
    delayMilliseconds,
    ownerOnly,
    waves: Object.freeze(waves),
    unknownFields: Object.freeze(unknownFields),
  })
}

function parseWave(token: string): SoundWave {
  if (!token || token.length > 1_024 || /[\u0000-\u001f\u007f]/.test(token)) {
    throw error("MalformedDefinition", "wave token is malformed")
  }
  const decorators: SoundDecorator[] = []
  let offset = 0
  while (offset < token.length) {
    const decorator = DECORATORS.get(token[offset]!)
    if (!decorator) break
    decorators.push(decorator)
    offset += 1
  }
  const undecorated = token.slice(offset).replace(/\\/g, "/")
  if (!logicalPath(undecorated)) throw error("MalformedDefinition", "undecorated wave path is malformed")
  return Object.freeze({
    token,
    undecorated,
    resource: decorators.includes("sentence") ? null : `sound/${undecorated}`.toLowerCase(),
    decorators: Object.freeze(decorators),
  })
}

function parseChannel(value: string): number {
  const named = CHANNELS.get(canonical(value))
  if (named !== undefined) return named
  if (/^chan_/i.test(value)) throw error("MalformedDefinition", `unknown channel ${value}`)
  return strictInteger(value)
}

function parseVolume(value: string): Interval {
  if (canonical(value) === "vol_norm") return interval(1, 0)
  return parseInterval(value)
}

function parsePitch(value: string): Interval {
  const named = new Map([["pitch_norm", 100], ["pitch_low", 95], ["pitch_high", 120]])
    .get(canonical(value))
  return named === undefined ? parseInterval(value) : interval(named, 0)
}

function parseSoundLevel(value: string): Interval {
  const key = canonical(value)
  const named = LEVELS.get(key)
  if (named !== undefined) return interval(named, 0)
  const custom = /^sndlvl_(\d+)(?:db)?$/i.exec(value)
  if (custom) {
    const level = Number(custom[1])
    if (level > 0 && level <= 180) return interval(level, 0)
  }
  if (value.includes(",") || /^[-+]?\d+(?:\.\d+)?$/.test(value.trim())) return parseInterval(value)
  throw error("MalformedDefinition", `unknown sound level ${value}`)
}

function parseAttenuation(value: string, compatibility: boolean): Interval {
  const named = ATTENUATIONS.get(canonical(value))
  const source = named === undefined ? parseInterval(value) : interval(named, 0)
  const start = attenuationToLevel(source.start)
  const end = attenuationToLevel(source.start + source.range)
  const encodedStart = compatibility ? start + 256 : start
  const encodedEnd = compatibility ? end + 256 : end
  return interval(encodedStart, encodedEnd - encodedStart)
}

function attenuationToLevel(value: number): number {
  return Math.trunc(value === 0 ? 0 : 50 + 20 / value)
}

function parseInterval(value: string): Interval {
  const values = value.split(",")
  if (values.length > 2) throw error("MalformedDefinition", `interval ${value} has too many values`)
  const start = strictNumber(values[0]!)
  const end = values.length === 2 ? strictNumber(values[1]!) : start
  return interval(start, end - start)
}

function interval(start: number, range: number): Interval {
  if (!Number.isFinite(start) || !Number.isFinite(range)) throw error("MalformedDefinition", "interval is non-finite")
  return Object.freeze({ start, range })
}

function sampleInterval(value: Interval, sample: number): number {
  validateSample(sample)
  return value.start + value.range * sample
}

function spatialGains(source: SoundSource, listener: Listener, soundLevel: number): { left: number; right: number; distance: number; sourceDistance: number } {
  if (source.kind === "ui" || source.kind === "local-listener" || source.origin === null || soundLevel === 0) {
    return { left: 1, right: 1, distance: 1, sourceDistance: 0 }
  }
  const delta = subtract(source.origin, listener.origin)
  const length = Math.hypot(...delta)
  const direction = length > 0 ? scale(delta, 1 / length) : [0, 0, 0] as const
  const mono = source.radius > 0 && length < source.radius
    ? 1 - Math.max(0, length - source.radius * 0.5) / (source.radius * 0.5)
    : 0
  const pan = dot(listener.right, direction) * (1 - mono)
  const distanceGain = sourceDistanceGain(soundLevel, length)
  return { left: distanceGain * (1 - pan) / 2, right: distanceGain * (1 + pan) / 2, distance: distanceGain, sourceDistance: length }
}

function validateStart(event: StartSound): void {
  if (event.envelope && (![event.envelope.from, event.envelope.to].every(value => Number.isFinite(value) && value >= 0 && value <= 1)
    || !Number.isFinite(event.envelope.seconds) || event.envelope.seconds <= 0)) throw error("MalformedEvent", "sound envelope is invalid")
  if (!event || typeof event.samples !== "object" || event.samples === null
    || !uint(event.voiceIdentity) || !identity(event.definition)
    || !(uint(event.source.identity) || event.source.kind === "world" && event.source.identity === 0)
    || !validListener(event.listener) || !Number.isFinite(event.resourceDurationSeconds)
    || event.resourceDurationSeconds <= 0 || ![1, 2].includes(event.resourceChannels)
    || !Number.isFinite(event.scheduledTimeSeconds) || !Number.isFinite(event.delaySeconds)
    || !nonnegative(event.source.radius) || !nonnegative(event.mixerGain) || !nonnegative(event.userGain)
    || (event.resourceLoopStartSeconds !== null && (!nonnegative(event.resourceLoopStartSeconds)
      || event.resourceLoopStartSeconds >= event.resourceDurationSeconds))) {
    throw error("MalformedEvent", "start event is malformed")
  }
  validateSample(event.samples.volume)
  validateSample(event.samples.pitch)
  validateSample(event.samples.soundLevel)
  if (!Number.isSafeInteger(event.samples.wave) || event.samples.wave < 0) {
    throw error("MalformedEvent", "selected sound wave ordinal is invalid")
  }
  if (event.source.kind !== "ui" && event.source.kind !== "local-listener" && event.source.origin === null) {
    throw error("MalformedEvent", "positioned source has no origin")
  }
}

function validListener(listener: Listener): boolean {
  return uint(listener.identity) && Number.isSafeInteger(listener.revision) && listener.revision >= 0
    && vector(listener.origin) && normal(listener.forward) && normal(listener.right)
    && Math.abs(dot(listener.forward, listener.right)) <= 1e-4
    && nonnegative(listener.masterGain) && nonnegative(listener.categoryGain)
}

function validateRegistryLimits(limits: RegistryLimits): void {
  if (!positive(limits.maxDocuments) || !positive(limits.maxDefinitions) || !positive(limits.maxWavesPerDefinition)) {
    throw error("MalformedDocument", "registry limits must be positive")
  }
}

function validateSample(value: number): void {
  if (!Number.isFinite(value) || value < 0 || value >= 1) throw error("MalformedEvent", "random sample is outside [0, 1)")
}

function strictNumber(value: string): number {
  if (!/^\s*[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?\s*$/.test(value)) {
    throw error("MalformedDefinition", `number ${value} is malformed`)
  }
  const result = Number(value)
  if (!Number.isFinite(result)) throw error("MalformedDefinition", `number ${value} is non-finite`)
  return result
}

function strictInteger(value: string): number {
  if (!/^\s*[-+]?\d+\s*$/.test(value)) throw error("MalformedDefinition", `integer ${value} is malformed`)
  const result = Number(value)
  if (!Number.isSafeInteger(result)) throw error("MalformedDefinition", `integer ${value} is outside its range`)
  return result
}

function canonical(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => letter.toLowerCase())
}

function identity(value: string): boolean {
  return typeof value === "string" && /^[\x21-\x7e]{1,512}$/.test(value)
}

function logicalPath(value: string): boolean {
  return identity(value) && !value.startsWith("/") && !value.includes("\\")
    && value.split("/").every((component) => component.length > 0 && component !== "." && component !== "..")
}

function positive(value: number): boolean { return Number.isSafeInteger(value) && value > 0 }
function uint(value: number): boolean { return Number.isSafeInteger(value) && value > 0 && value <= 0xffff_ffff }
function nonnegative(value: number): boolean { return Number.isFinite(value) && value >= 0 }
function vector(value: readonly number[]): boolean { return value.length === 3 && value.every(Number.isFinite) }
function normal(value: readonly number[]): boolean { return vector(value) && Math.abs(dot(value, value) - 1) <= 1e-4 }
function dot(left: readonly number[], right: readonly number[]): number { return left[0]! * right[0]! + left[1]! * right[1]! + left[2]! * right[2]! }
function subtract(left: readonly [number, number, number], right: readonly [number, number, number]): readonly [number, number, number] { return [left[0] - right[0], left[1] - right[1], left[2] - right[2]] }
function scale(value: readonly [number, number, number], amount: number): readonly [number, number, number] { return [value[0] * amount, value[1] * amount, value[2] * amount] }
function distance(left: readonly [number, number, number], right: readonly [number, number, number]): number { return Math.hypot(...subtract(left, right)) }
function mix(left: number, right: number, fraction: number): number { return left + (right - left) * fraction }
function error(code: SourceAudioError["code"], message: string): SourceAudioError { return new SourceAudioError(code, message) }
