# Audio

## Sample

```ts
import { createAudioSystem } from "@playsrc/audio"

const audio = createAudioSystem(audioContext, decodedResources)
await audio.resume()
audio.play({ voice: 1, resource: "sound/weapons/rocket_shoot.wav", gain: 1, pan: 0, loop: false })
```

## Objective

Resolve and present Source sound definitions and audio state.

## Responsibilities

- Parse and classify sound scripts, references, channels, levels, variation, and playback parameters.
- Own spatialization, mixing inputs, playback lifetime, and browser audio resources.
- Consume explicit world, gameplay, and replay audio events.
- Create bounded Web Audio source, gain, split, merge, and direct-resource pan graphs only from caller-supplied exact decoded buffers; missing resources create no node or substitute sound.
- Present exact stock rocket/sticky launch and explosion WAVE resources selected by game-owned versioned fire/explosion events after one explicit browser resume gesture.
- Compile caller-parsed sound entries with Source channel, level/attenuation, volume, pitch, decorators, ordered no-repeat waves, base/override, delay, and owner-only semantics; produce neutral voice gain/pan/mixer/DSP dispositions before browser construction.
- Build channel-exact mono/stereo Web Audio graphs for neutral voices, schedule offsets and loop starts, replace only matching voice identities, and release every node once on end, stop, reset, replacement, or close.

## Non-Responsibilities

- Deciding which game event should emit a game-specific sound.
- Advancing gameplay or reconstructing missing events.
- Finding resources outside configured content interfaces.

## Relationships

Consumes `content` and game-owned sound events; applications provide browser lifecycle and user settings.

## Completion

Complete when the declared Source sound behavior family is classified, presented, and supported by credible evidence.
