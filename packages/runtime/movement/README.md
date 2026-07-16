# Movement

## Sample

```rust
let next_player = playsrc_movement::advance(&collision, player, command, parameters)?;
```

## Objective

Advance generic Source player movement from commands and world queries.

## Responsibilities

- Implement shared ground, air, water, ladder, crouch, jump, stair, and velocity behavior.
- Consume deterministic collision queries and explicit movement configuration.
- Produce movement state suitable for both authority and prediction.
- Advance fixed-tick standing/crouched walk state through friction, ground/air acceleration, jump latch, split gravity, multi-plane clipping, ground probes, and 18-unit step route selection.

## Non-Responsibilities

- TF2, CS:S, or legacy CS:GO movement differences.
- Weapon impulses, ruleset timing, or rigid-body simulation.
- Owning collision geometry.

## Relationships

Consumes `collision`; simulation schedules movement; each game module owns its movement differences and game state integration.

## Completion

Complete when the declared generic Source movement family is implemented and game variation occurs through explicit game-owned behavior.
