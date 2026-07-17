# Movement

## Sample

```rust
let result = playsrc_movement::step(&collision, state, input, configuration, game_policy)?;
```

## Objective

Advance generic Source player movement from commands and world queries.

## Responsibilities

- Implement shared ground, air, water, ladder, crouch, jump, stair, and velocity behavior.
- Consume deterministic collision queries and explicit movement configuration.
- Produce movement state suitable for both authority and prediction.
- Advance one admitted fixed-tick command through a single versioned movement state and result containing walk/noclip mode, support identity/plane/friction, fall/jump latch, reversible crouch state/fraction/view, contacts, events, query transcript, wish/jump/step outputs, and deterministic trapped recovery.
- Execute command-angle basis and normalization, speed selection, surface friction, projected ground/30-unit-capped air acceleration, split gravity, jump/landing transitions, four-bump/five-plane clipping, normalized two-plane creases, slopes, high/low 18-unit steps, blocked unduck retry, and accelerated/direct noclip without displacement collision.
- Consume game-supplied hulls, views, speed/air/bunnyhop/backward/jump values, eligibility, step strategy, and accepted mode-transition dispositions without embedding a game-specific movement loop.
- Reject malformed selected state, command, configuration, hull, velocity, coordinate, and trace values before publication with command number, operation, classification, field, exact offending bits, and configured-limit bits.

## Non-Responsibilities

- TF2, CS:S, or legacy CS:GO movement differences.
- Weapon impulses, ruleset timing, or rigid-body simulation.
- Owning collision geometry.

## Relationships

Consumes one `Tracer`; Simulation schedules movement; each game module projects its movement policy and consumes the returned state/events. `State::snapshot_bytes` is the version-1 exact authority/prediction comparison surface.

## Completion

Complete when the declared generic Source movement family is implemented and game variation occurs through explicit game-owned behavior.
