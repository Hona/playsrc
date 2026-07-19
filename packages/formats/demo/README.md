# Demo

## Implementation

`rust/` exposes caller-profiled, caller-bounded whole-buffer and chunk-scheduled parsing. Successful values retain the complete source bytes, fixed header fields and float bits, command metadata, payload ranges, and both complete and SourceTV stream-flush terminal encodings.

Run the checked controlled capture and protocol-24 verification with:

```sh
bun packages/formats/demo/scripts/verify-controlled-tf2-demo.ts
```

## Objective

Parse Source 1 demo containers and records as bounded data.

## Responsibilities

- Validate demo headers, command records, lengths, ticks, and encoded payloads.
- Preserve ordering and protocol data required by replay consumers.
- Report malformed, unsupported, and unknown records explicitly.
- Verify one immutable capture against `playsrc.local.json` content identities and checked expected framing/state counts.

## Non-Responsibilities

- Owning replay state, seeking policy, or playback timing.
- Resimulating gameplay.
- Rendering recorded state.

## Relationships

Supplies parsed records to `replay`; game and networking modules interpret game-specific recorded state.

## Completion

Complete when the declared demo container and record families are bounded and verified by fixed inputs, expected observable outputs, and declared comparison methods.
