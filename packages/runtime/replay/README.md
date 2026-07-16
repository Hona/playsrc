# Replay

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
