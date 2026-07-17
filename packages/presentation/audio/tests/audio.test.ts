import { describe, expect, test } from "bun:test"
import {
  AudioError,
  createAudioSystem,
  MixerRegistry,
  SoundRegistry,
  SourceAudioError,
  SourceAudioWorld,
  sourceDistanceGain,
  type Listener,
  type SoundScriptNode,
  type StartSound,
} from "../src"

class Parameter { value = 0 }
class Node {
  onended: (() => void) | null = null
  buffer?: AudioBuffer
  loop = false
  readonly gain = new Parameter()
  readonly pan = new Parameter()
  readonly playbackRate = new Parameter()
  loopStart = 0
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

  test("builds and releases a channel-exact neutral voice graph", () => {
    const nodes: Node[] = []
    const make = () => { const node = new Node(); nodes.push(node); return node }
    const buffer = {
      length: 44_100,
      numberOfChannels: 2,
      sampleRate: 44_100,
      duration: 1,
    } as AudioBuffer
    const context = {
      state: "running",
      currentTime: 2,
      destination: make(),
      createBufferSource: make,
      createGain: make,
      createStereoPanner: make,
      createChannelSplitter: make,
      createChannelMerger: make,
      resume: async () => {},
      close: async () => {},
    } as unknown as AudioContext
    const voice = new SourceAudioWorld(new SoundRegistry([targetDocument]), { maxActiveVoices: 16 })
      .start(start({ scheduledTimeSeconds: 3 })).voice
    const audio = createAudioSystem(context, [{ identity: voice.resource, buffer }])
    audio.playNeutral(voice)
    expect(audio.activeVoices()).toEqual([1])
    audio.stop(1)
    expect(audio.activeVoices()).toEqual([])
    expect(nodes.every((node) => node.disconnect instanceof Function)).toBe(true)
  })
})

function scalar(key: string, value: string): SoundScriptNode {
  return Object.freeze({ key, value })
}

function entry(key: string, value: readonly SoundScriptNode[]): SoundScriptNode {
  return Object.freeze({ key, value })
}

const targetDocument = Object.freeze({
  logicalPath: "scripts/game_sounds_weapons.txt",
  mode: "base" as const,
  preload: false,
  entries: Object.freeze([
    entry("Weapon_RPG.Single", [
      scalar("channel", "CHAN_WEAPON"),
      scalar("volume", "1.0"),
      scalar("soundlevel", "SNDLVL_94dB"),
      scalar("wave", ")weapons/rocket_shoot.wav"),
    ]),
    entry("BaseExplosionEffect.Sound", [
      scalar("channel", "CHAN_WEAPON"),
      scalar("volume", "1.0"),
      scalar("soundlevel", "SNDLVL_95dB"),
      scalar("pitch", "PITCH_NORM"),
      entry("rndwave", [
        scalar("wave", ")weapons/explode2.wav"),
        scalar("wave", ")weapons/explode3.wav"),
        scalar("wave", ")weapons/explode1.wav"),
      ]),
    ]),
  ]),
})

const listener: Listener = Object.freeze({
  identity: 1,
  revision: 7,
  origin: Object.freeze([0, 0, 0]),
  forward: Object.freeze([1, 0, 0]),
  right: Object.freeze([0, -1, 0]),
  masterGain: 1,
  categoryGain: 1,
  muted: false,
})

function start(overrides: Partial<StartSound> = {}): StartSound {
  return Object.freeze({
    voiceIdentity: 1,
    definition: "BaseExplosionEffect.Sound",
    source: Object.freeze({
      kind: "world" as const,
      identity: 10,
      ownerIdentity: null,
      origin: Object.freeze([36, 0, 0]),
      radius: 0,
      sourceClass: "",
    }),
    listener,
    samples: Object.freeze({ volume: 0, pitch: 0, wave: 0, soundLevel: 0 }),
    resourceDurationSeconds: 8,
    resourceLoopStartSeconds: null,
    resourceChannels: 2,
    resourceAvailable: () => true,
    scheduledTimeSeconds: 5,
    delaySeconds: 0,
    mixerGain: 0.72,
    userGain: 1,
    doNotOverwrite: false,
    ...overrides,
  })
}

describe("Source sound registry and neutral voice state", () => {
  test("resolves exact target fields and producer-selected original wave ordinals", () => {
    const registry = new SoundRegistry([targetDocument])
    const rocket = registry.get("weapon_rpg.single")!
    expect(rocket.channel).toBe(1)
    expect(rocket.soundLevel).toEqual({ start: 94, range: 0 })
    expect(rocket.waves[0]).toEqual({
      token: ")weapons/rocket_shoot.wav",
      undecorated: "weapons/rocket_shoot.wav",
      resource: "sound/weapons/rocket_shoot.wav",
      decorators: ["spatial-stereo"],
    })

    const world = new SourceAudioWorld(registry, { maxActiveVoices: 16 })
    const resources: string[] = []
    for (const [ordinal, wave] of [0, 1, 2, 0].entries()) {
      resources.push(world.start(start({
        voiceIdentity: ordinal + 1,
        source: Object.freeze({ ...start().source, identity: ordinal + 10 }),
        samples: Object.freeze({ volume: 0, pitch: 0, wave, soundLevel: 0 }),
      })).voice.resource)
    }
    expect(resources).toEqual([
      "sound/weapons/explode2.wav",
      "sound/weapons/explode3.wav",
      "sound/weapons/explode1.wav",
      "sound/weapons/explode2.wav",
    ])
  })

  test("applies channel replacement, scheduled skip, distance, linear pan, and mixer stages", () => {
    const world = new SourceAudioWorld(new SoundRegistry([targetDocument]), { maxActiveVoices: 16 })
    const first = world.start(start()).voice
    const second = world.start(start({ voiceIdentity: 2, delaySeconds: -1 })).voice
    expect(world.voices().map(({ identity }) => identity)).toEqual([2])
    expect(second.offsetSeconds).toBe(1)
    expect(second.playbackRate).toBe(1)
    expect(second.dsp).toBe("room")
    expect(second.leftGain).toBeCloseTo(second.rightGain, 7)
    expect(second.leftGain).toBeCloseTo(sourceDistanceGain(95, 36) * 0.5 * 0.72, 7)
    expect(first.resource).toBe("sound/weapons/explode2.wav")
  })

  test("rejects unknown levels, unavailable selected resources, and out-of-range ordinals", () => {
    expect(() => new SoundRegistry([Object.freeze({
      ...targetDocument,
      entries: [entry("bad", [scalar("soundlevel", "LOUDISH"), scalar("wave", "bad.wav")])],
    })])).toThrow(SourceAudioError)
    const world = new SourceAudioWorld(new SoundRegistry([targetDocument]), { maxActiveVoices: 16 })
    expect(() => world.start(start({ resourceAvailable: () => false }))).toThrow(SourceAudioError)
    expect(() => world.start(start({
      resourceAvailable: (resource) => resource.endsWith("explode3.wav"),
      samples: Object.freeze({ volume: 0, pitch: 0, wave: 0, soundLevel: 0 }),
    }))).toThrow(SourceAudioError)
    expect(() => world.start(start({
      samples: Object.freeze({ volume: 0, pitch: 0, wave: 3, soundLevel: 0 }),
    }))).toThrow(SourceAudioError)
    expect(world.voices()).toEqual([])
    const selected = world.start(start({
      samples: Object.freeze({ volume: 0, pitch: 0, wave: 2, soundLevel: 0 }),
    })).voice
    expect(selected.wave).toBe(2)
    expect(selected.resource).toBe("sound/weapons/explode1.wav")
  })

  test("selects the first configured mixer gain from ordered matching groups", () => {
    const rule = (group: string, minimumLevel: number | null, maximumLevel: number | null) => Object.freeze({
      group,
      pathSubstring: "",
      sourceClassSubstring: "",
      channel: null,
      minimumLevel,
      maximumLevel,
      priority: 50,
      ducked: false,
      causesDucking: false,
      duckTarget: 1,
      duckerThreshold: 0.4,
    })
    const mixers = new MixerRegistry(
      [rule("Loud", 91, 100), rule("All", null, null)],
      [{ identity: "Default_Mix", gains: new Map([["All", 0.72]]) }],
    )
    expect(mixers.resolve(
      "default_mix", "sound/weapons/explode2.wav", "", 1, 95,
    )).toEqual({ groups: ["Loud", "All"], selectedGroup: "All", gain: 0.72 })
  })
})
