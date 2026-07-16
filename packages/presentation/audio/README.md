# Audio

## Sample

```ts
import { createAudioSystem } from "@playsrc/audio"

const audio = createAudioSystem(audioContext, content)
await audio.play("Weapon_RocketLauncher.Single", origin)
```

## Objective

Resolve and present Source sound definitions and audio state.

## Responsibilities

- Parse and classify sound scripts, references, channels, levels, variation, and playback parameters.
- Own spatialization, mixing inputs, playback lifetime, and browser audio resources.
- Consume explicit world, gameplay, and replay audio events.

## Non-Responsibilities

- Deciding which game event should emit a game-specific sound.
- Advancing gameplay or reconstructing missing events.
- Finding resources outside configured content interfaces.

## Relationships

Consumes `content` and game-owned sound events; applications provide browser lifecycle and user settings.

## Completion

Complete when the declared Source sound behavior family is classified, presented, and supported by credible evidence.
