# Demo

## Sample

```ts
import { parseDemo } from "@playsrc/demo"

const demo = parseDemo(bytes)
```

```rust
let demo = playsrc_demo::parse(&bytes)?;
```

## Objective

Parse Source 1 demo containers and records as bounded data.

## Responsibilities

- Validate demo headers, command records, lengths, ticks, and encoded payloads.
- Preserve ordering and protocol data required by replay consumers.
- Report malformed, unsupported, and unknown records explicitly.

## Non-Responsibilities

- Owning replay state, seeking policy, or playback timing.
- Resimulating gameplay.
- Rendering recorded state.

## Relationships

Supplies parsed records to `replay`; game and networking modules interpret game-specific recorded state.

## Completion

Complete when the declared demo container and record families are bounded and supported by fair vectors.
