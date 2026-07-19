# Networking

## Implementation

`rust/` implements the transport-independent TF2 protocol-24 recorded payload seam. `RecordedStateCodec` atomically applies LSB-first message framing, DEM/send tables, class binding, string tables and class baselines, event schemas/events, outer user-message handoff, and full/delta packet entities. Caller-supplied limits bound every retained family. Reserved or unsupported message identities stop without resynchronization.

## Objective

Transport commands and replicated state for online multiplayer without owning gameplay.

## Responsibilities

- Define transport-neutral command, snapshot, acknowledgement, and synchronization behavior.
- Encode game-owned replicated state and support prediction reconciliation.
- Bound message sizes, queues, rates, and backpressure.
- Reuse one recorded-state codec for DEM payloads and future live transport adapters.

## Non-Responsibilities

- Advancing gameplay or deciding game-owned replication fields.
- Matchmaking, server orchestration, or a specific browser transport.
- Replay parsing or rendering.

## Relationships

Connects clients to the simulation authority using schemas owned by simulation and game modules; deployed servers and clients provide transports.

## Completion

Complete when the declared multiplayer protocol behavior is bounded, deterministic where required, and integrated with one gameplay authority.
