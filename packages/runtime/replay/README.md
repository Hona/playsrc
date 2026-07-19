# Replay

## Implementation

`rust/` exposes `ReplaySession` over caller-decoded recorded operations. It owns lifecycle, exact rational playback, pause/rate/step, canonical tick occurrences, event order, complete checkpoints, seek/rewind, identity-bound snapshot/restore, deterministic continuation, and immutable interpolation inputs. The interface contains no gameplay Simulation dependency; `AuthorityAudit::simulation_calls` remains zero by construction and verification.

## Objective

Produce authoritative replay state from parsed Source demo records.

## Responsibilities

- Own replay timelines, decoded state progression, seeking, and playback state.
- Apply typed operations emitted by the selected game-owned decoder without owning game-specific meaning.
- Expose replay snapshots and events to presentation modules.

## Non-Responsibilities

- Parsing demo containers.
- Becoming a second gameplay simulation.
- Rendering or controlling product UI.

## Relationships

Consumes `demo`, networking formats, and game-owned recorded-state interpretation; supplies state to presentation and replay applications.

## Completion

Complete when declared recordings can produce bounded, seekable, authoritative replay state with credible evidence.
