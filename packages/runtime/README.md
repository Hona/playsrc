# Runtime Packages

## Objective

Advance, transport, or reconstruct Source gameplay and replay state.

## Responsibilities

- Own deterministic movement, physics, simulation, networking, and replay behavior.
- Keep shared behavior usable by browser workers and native servers.
- Expose state for presentation without transferring authority.

## Non-Responsibilities

- Game-specific mechanics and rulesets.
- GPU rendering or product UI.

Each child is an independently useful package. This directory is only a navigational group.

## Packages

| Package | Exact responsibility |
|---|---|
| [`movement/`](movement/) | Shared ground, air, water, ladder, crouch, jump, stair, velocity, and player-hull movement behavior. |
| [`physics/`](physics/) | Rigid bodies, integration, contacts, constraints, sleeping, and physical material response. |
| [`simulation/`](simulation/) | Tick progression, commands, scheduling, state transitions, events, snapshots, and gameplay authority. |
| [`networking/`](networking/) | Commands, snapshots, acknowledgements, encoding, replication, prediction reconciliation, bounds, and backpressure. |
| [`replay/`](replay/) | Recorded-state decoding, replay timelines, seeking, playback state, snapshots, and events without gameplay resimulation. |
