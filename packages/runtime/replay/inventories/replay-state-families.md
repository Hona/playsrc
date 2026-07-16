# Replay State Family Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

| Field | Current value |
|---|---|
| Authority identity | Valve Source SDK 2013 client demo, replay, interpolation, string-table, receive-table, entity-update, and game-event contracts; accepted TF2 `tf2-demo3-net24` Demo profile; accepted Networking codec inventory; accepted TF2 recorded-state decoder inventory; controlled TF2 target recordings. |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; Demo inventory Candidate; Networking and TF2 reviewed commits Missing; target recording build Missing. |
| Generator command | Missing. |
| Output path | `packages/runtime/replay/inventories/replay-state-families.md` |
| Item count | 40 candidate items; 0 accepted items. |

Every item is Replay-owned. `Required seam` identifies an input producer or output consumer and never transfers Replay timeline, progression, playback, index, checkpoint, snapshot, or event-order authority.

## Session And Selection

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `session.open` | Open exactly one successful immutable Demo value under required limits and create no session from raw bytes or a failed Demo result. | Demo supplies parsed records and source identity. | Unsupported |
| `session.accepted-metadata` | Preserve source hash, profile, game selection, header metadata, observed record range, and terminal identity while treating declared duration, frames, and ticks as advisory. | Demo supplies exact metadata and records. | Unsupported |
| `session.decoder-selection` | Select exactly one caller-named shared-codec and game-decoder identity whose declared capabilities cover the profile and selected game. | Networking and the selected game supply accepted registries. | Missing |
| `session.unsupported-protocol` | Reject an unsupported profile or decoder capability before sign-on without selecting another protocol or decoder. | Demo and decoder registries supply immutable profile identities. | Unsupported |
| `session.missing-decoder` | Return one structured `Missing` result for absent, duplicate, or incomplete decoder registration and allocate no authoritative state. | Applications supply the explicit selected game and decoder set. | Unsupported |
| `session.lifecycle` | Transition through `Opening`, `Signon`, `Ready`, and exactly one `Ended` or `Failed` state with an orthogonal Playing or Paused state only while Ready. | Presentation and applications observe state without mutating it. | Unsupported |
| `session.end-of-stream` | Commit all valid prior operations, emit one terminal transition at the terminal DEM record, freeze the clock, and make repeated advance inert. | Demo supplies the terminal record. | Unsupported |

## Sign-On And Setup State

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `setup.signon-state` | Apply decoded sign-on transitions in source order and prohibit Ready before the selected decoder's complete setup predicate passes. | Networking decodes sign-on messages. | Unsupported |
| `setup.data-tables` | Version and retain decoded data-table definitions, class links, field declarations, and replacement identity before dependent baselines or fields. | Networking decodes tables; games bind game fields. | Unsupported |
| `setup.string-tables` | Version and retain table identity, ordered entries, exact user data, creation tick, and mutations before dependent operations. | Networking decodes table wire data; games interpret named tables. | Unsupported |
| `setup.entity-baselines` | Version and retain class and entity baselines and reject a delta whose exact base is absent. | Networking decodes baselines; games supply field schemas. | Unsupported |

## Recorded Progression And Events

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `progression.source-order` | Apply outer records, inner messages, and decoder emissions in lexicographic ordinal order without coalescing duplicates. | Demo and decoders supply all three ordinals. | Unsupported |
| `progression.packet-application` | Commit all decoded operations from one packet in order or retain the complete prior snapshot on failure. | Networking decodes packet messages. | Unsupported |
| `progression.entity-creation` | Atomically establish entity index, generation, class, baseline, and initial fields at one source cursor. | Networking emits creation; the selected game supplies class meaning. | Unsupported |
| `progression.entity-deletion` | Delete one live generation, invalidate references before index reuse, and retain the deletion cursor. | Networking emits deletion operations. | Unsupported |
| `progression.field-update` | Patch one declared field on one live generation, retain unchanged fields, and reject an unknown class, field, quantization, or target generation. | Networking decodes values; the selected game owns field semantics. | Unsupported |
| `progression.console-command` | Apply only typed decoder operations or an explicit inert classification and never execute recorded command text. | The selected game owns command meaning. | Missing |
| `progression.game-event` | Append each typed selected-game event once at its exact operation position and source cursor. | Networking decodes event framing; the selected game owns event meaning. | Missing |
| `progression.audio-event` | Append ordered audio start, update, and stop operations with exact target and parameters; never infer audio from state. | The selected game maps events; Audio consumes them. | Missing |
| `progression.particle-event` | Append ordered particle create, control, and stop operations; never infer an effect from entity disappearance. | The selected game maps events; Particle consumes them. | Missing |

## Timeline, Playback, And Seeking

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `timeline.tick-identity` | Preserve command tick, decoded server tick, occurrence, record ordinal, and source range as distinct fields of each canonical cursor. | Demo and Networking supply source and server tick domains. | Unsupported |
| `timeline.playback-clock` | Scale exact non-negative elapsed nanoseconds by a positive rational rate, commit all crossed authoritative boundaries, and retain exact remainder. | Applications supply elapsed duration; presentation reads position. | Unsupported |
| `timeline.pause-resume` | Pause without consuming elapsed duration or emitting events and resume from the same cursor and accumulator. | Applications invoke controls. | Unsupported |
| `timeline.rate` | Validate bounded positive numerator and denominator, preserve accumulated remainder on change, and expose one current rational rate. | Applications select a rate within Replay limits. | Unsupported |
| `timeline.step` | While paused, cross a bounded signed count of indexed canonical ticks, reconstruct backward movement, and return one discontinuity without transient event delivery. | Applications invoke step controls. | Unsupported |
| `timeline.seek` | Resolve an exact cursor or the last occurrence of one server tick, reject an absent domain, restore state, and return the target snapshot without clamping. | Applications and tools invoke seek. | Unsupported |
| `timeline.rewind` | Reconstruct every backward target from a complete checkpoint at or before the target and replay operations forward. | Checkpoints and the record index supply restart positions. | Unsupported |
| `timeline.record-index` | Retain one bounded entry for every outer record plus its tick and operation/event boundaries; fail rather than omit an unindexable record. | Demo supplies ordered record identities and ranges. | Unsupported |
| `timeline.checkpoints` | Store complete identity-bound state at the initial Ready cursor and configured tick interval within count and byte limits. | Decoders serialize their complete continuation state. | Unsupported |
| `timeline.deterministic-reconstruction` | Produce byte-identical canonical state, event cursor, and next transition whenever the same cursor is reached under identical identities. | Evidence compares uninterrupted and seek histories. | Unsupported |

## Snapshots And Presentation Inputs

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `snapshot.authoritative-state` | Expose immutable complete setup, entity, game-decoder, cursor, ordering, and event-cursor state sufficient for exact continuation. | Games provide decoder snapshot data; consumers read only. | Unsupported |
| `snapshot.interpolation-inputs` | Select prior and next authoritative snapshots, exact fraction, per-field decoder policy, and discontinuity without inventing samples or extrapolated truth. | Games declare field policy; presentation performs interpolation. | Unsupported |
| `snapshot.presentation` | Return one immutable presentation tuple plus only the event range crossed since the caller's prior event cursor. | Rendering, Particle, and Audio consume the tuple. | Unsupported |
| `snapshot.discontinuity` | Mark create, delete, reset, no-interpolation, seek, step, and terminal boundaries so no consumer blends or continues effects across them implicitly. | Demo flags and decoders identify recorded discontinuities. | Unsupported |

## Failures, Bounds, Authority, And Integration

| Stable identity | Replay-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `failure.malformed-propagation` | Retain exact source and decoder location, classification, and last committed snapshot when malformed or failed decoding stops progression. | Demo and decoders supply structured failures. | Unsupported |
| `bounds.resident-memory` | Charge every retained state, table, entity, field, event, index, checkpoint, snapshot, and diagnostic byte before allocation and fail atomically above the limit. | Callers supply the required limits value. | Unsupported |
| `bounds.index-and-work` | Reject out-of-range cursors, over-bound index/checkpoint counts, seek replay work, steps, and rate components without clamping or partial state. | Applications receive structured bound failures. | Unsupported |
| `coverage.complete-classification` | Assign every record and decoder operation exactly one coverage classification and stop on required Unsupported, Unknown, Malformed, or Missing state. | Tools and game roadmaps consume the audit. | Unsupported |
| `authority.no-gameplay-resimulation` | Never submit recorded user commands or inferred inputs to Simulation, Movement, Physics, Entity gameplay transitions, weapons, projectiles, objectives, or rules. | Simulation and game packages remain uncalled by Replay progression. | Unsupported |
| `integration.sole-replay-authority` | Supply one current timeline, progression, playback, seek, snapshot, and event interface to every producer and consumer with no duplicate authority or fallback. | The recorded-replay application owner is Missing. | Missing |

## Generation Contract

The future checked-in generator must read a manifest pinning the official SDK paths and revision, accepted Demo profiles, accepted Networking codec families, accepted selected-game decoder families, controlled recording identities, and the root Owner Registry. It must emit these identities in section order, reproduce this file byte-for-byte on repeated runs, and retain `Unsupported`, `Unknown`, `Malformed`, and `Missing` discoveries. Generation fails on an omitted or duplicate identity, missing authority, unowned game semantic, unclassified record or operation, stale capture, or item-count mismatch.
