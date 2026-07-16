# Replay

## Sample

```ts
import { openReplay } from "@playsrc/replay"

const replay = await openReplay(demo, game)
const snapshot = replay.seek(tick)
```

```rust
let mut replay = playsrc_replay::Replay::open(demo, game)?;
let snapshot = replay.seek(tick)?;
```

## Objective

Produce authoritative replay state from parsed Source demo records.

## Responsibilities

- Own replay timelines, decoded state progression, seeking, and playback state.
- Apply game-specific demo interpretation without resimulating gameplay.
- Expose replay snapshots and events to presentation modules.

## Non-Responsibilities

- Parsing demo containers.
- Becoming a second gameplay simulation.
- Rendering or controlling product UI.

## Relationships

Consumes `demo`, networking formats, and game-owned recorded-state interpretation; supplies state to presentation and replay applications.

## Completion

Complete when declared recordings can produce bounded, seekable, authoritative replay state with credible evidence.
