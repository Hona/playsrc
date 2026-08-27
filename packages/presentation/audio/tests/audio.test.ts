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

class Parameter {
  value = 0
  readonly automation: Array<readonly [string, number, number]> = []
  setValueAtTime(value: number, when: number): void { this.automation.push(["set", value, when]) }
  linearRampToValueAtTime(value: number, when: number): void { this.automation.push(["ramp", value, when]) }
}
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
  disconnects = 0
  connect(): this { return this }
  disconnect(): void { this.disconnects += 1 }
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

test("audio preload closure comes only from selected definitions, never from a script filename", () => {
  const registry = new SoundRegistry([{
    logicalPath: "scripts/game_sounds_vo.txt", mode: "base", preload: false,
    entries: [
      { key: "Announcer.RoundEnds60seconds", value: [{ key: "wave", value: "vo/announcer_ends_60sec.mp3" }] },
      { key: "Game.Overtime", value: [{ key: "rndwave", value: [
        { key: "wave", value: "#vo/announcer_overtime.mp3" },
        { key: "wave", value: "vo/announcer_overtime.mp3" },
        { key: "wave", value: "vo/announcer_overtime2.mp3" },
      ] }] },
    ],
  }])
  expect(registry.resources()).toEqual(["sound/vo/announcer_ends_60sec.mp3", "sound/vo/announcer_overtime.mp3", "sound/vo/announcer_overtime2.mp3"])
  expect(Object.isFrozen(registry.resources())).toBe(true)
  expect(registry.resources()).not.toContain("sound/vo/intel_enemystolen.mp3")
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
    expect(nodes.slice(1).every((node) => node.disconnects === 1)).toBe(true)
  })

  test("patch destruction stops and disconnects both channels, preserving bot patches and the complete winddown", () => {
    const nodes: Node[] = [], sources: Node[] = []
    const make = () => { const node = new Node(); nodes.push(node); return node }
    const context = {
      state: "running", currentTime: 2, destination: make(),
      createBufferSource: () => { const node = make(); sources.push(node); return node },
      createGain: make, createStereoPanner: make, createChannelSplitter: make, createChannelMerger: make,
      resume: async () => {}, close: async () => {},
    } as unknown as AudioContext
    const entries = [["Fire", "CHAN_STATIC"], ["FireLoop", "CHAN_WEAPON"], ["WindDown", "CHAN_STATIC"]].map(([name, channel]) =>
      entry(`Weapon_FlameThrower.${name}`, [scalar("channel", channel!), scalar("wave", `)test/${name}.wav`)]))
    const world = new SourceAudioWorld(new SoundRegistry([{ ...targetDocument, entries }]), { maxActiveVoices: 128 })
    const audio = createAudioSystem(context, ["Fire", "FireLoop", "WindDown"].map(name => ({
      identity: `sound/test/${name}.wav`, buffer: { length: 44100, sampleRate: 44100, numberOfChannels: 2, duration: 1 } as AudioBuffer,
    })))
    let identity = 0
    const emit = (name: string, sourceIdentity: number) => {
      const event = start({ voiceIdentity: ++identity, definition: `Weapon_FlameThrower.${name}`, samples: { volume: 0, pitch: 0, soundLevel: 0, wave: 0 },
        source: { kind: "entity", identity: sourceIdentity, ownerIdentity: sourceIdentity, origin: [0, 0, 0], radius: 0, sourceClass: "tf_weapon" },
        resourceLoopStartSeconds: name === "FireLoop" ? 0 : null, scheduledTimeSeconds: 2,
        ...(name === "WindDown" ? {} : { envelope: { from: name === "Fire" ? 1 : 0, to: name === "Fire" ? 0 : 1, seconds: 3.5 } }),
      })
      const result = world.start(event)
      for (const old of result.replaced) audio.stop(old)
      audio.playNeutral(result.voice)
      return result.voice.identity
    }
    const botStart = emit("Fire", 2), botLoop = emit("FireLoop", 2)
    const botSources = sources.slice()
    for (let repeat = 0; repeat < 10; repeat++) {
      const localStart = emit("Fire", 1), localLoop = emit("FireLoop", 1)
      const firing = sources.slice(-2)
      const winddown = emit("WindDown", 1)
      const tail = sources.at(-1)!
      // CHAN_STATIC does not replace the older start. Destruction is explicit,
      // by sound-patch source + definition, not by new event voice identity.
      expect(audio.activeVoices()).toEqual([botStart, botLoop, localStart, localLoop, winddown])
      for (const name of ["FireLoop", "Fire"]) {
        for (const stopped of world.stopDefinition(1, `Weapon_FlameThrower.${name}`)) audio.stop(stopped)
      }
      expect(firing.every(source => source.stops === 1 && source.disconnects === 1)).toBe(true)
      expect(botSources.every(source => source.stops === 0 && source.disconnects === 0)).toBe(true)
      expect(tail.stops).toBe(0)
      expect(audio.activeVoices()).toEqual([botStart, botLoop, winddown])
      tail.onended!()
      world.stop(winddown)
      expect(tail.disconnects).toBe(1)
      expect(audio.activeVoices()).toEqual([botStart, botLoop])
      expect(world.voices().filter(voice => voice.loopStartSeconds !== null)).toHaveLength(1)
    }
    expect(nodes.some(node => node.gain.automation.some(([kind, value, when]) => kind === "ramp" && value > 0 && when === 5.5))).toBe(true)
    world.reset(); audio.reset()
    expect(audio.activeVoices()).toEqual([])
    expect(sources.every(source => source.disconnects === 1)).toBe(true)
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

  test("suppresses a farther fifth duplicate without replacing its four nearer Source voices", () => {
    const document = Object.freeze({
      ...targetDocument,
      entries: Object.freeze([
        entry("Test.Duplicate", [
          scalar("channel", "CHAN_AUTO"),
          scalar("soundlevel", "SNDLVL_95dB"),
          scalar("wave", "weapons/explode2.wav"),
        ]),
      ]),
    })
    const world = new SourceAudioWorld(new SoundRegistry([document]), { maxActiveVoices: 16 })
    const request = (identity: number, distance: number) => start({
      voiceIdentity: identity,
      definition: "Test.Duplicate",
      source: Object.freeze({ ...start().source, identity: identity + 10, origin: Object.freeze([distance, 0, 0]) }),
    })
    for (let identity = 1; identity <= 4; identity += 1) world.start(request(identity, identity * 20))
    try {
      world.start(request(5, 200))
      throw new Error("farther duplicate was not suppressed")
    } catch (error) {
      expect(error).toBeInstanceOf(SourceAudioError)
      expect((error as SourceAudioError).code).toBe("Suppressed")
    }
    expect(world.voices().map(voice => voice.identity)).toEqual([1, 2, 3, 4])
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
