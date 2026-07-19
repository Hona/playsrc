# Movement

## Sample

```rust
let result = playsrc_movement::step(&collision, state, input, configuration, game_policy)?;
```

## Objective

Advance generic Source player movement from commands and world queries.

## Responsibilities

- Implement shared ground, air, water/current/swim/ledge-exit, ladder, crouch, jump, stair, observer, fly, noclip, base-velocity, conveyor, and player-side mover behavior.
- Consume deterministic collision queries and explicit movement configuration.
- Produce movement state suitable for both authority and prediction.
- Advance one admitted fixed-tick command through a single versioned movement state and result containing movement/observer/toss modes, command and absolute angle state, support identity/plane/friction, base and support velocity, water/current/ledge-exit state, ladder normal, fall/jump latch, reversible crouch state/fraction/view, contacts, events, sweep and point-content transcripts, mover result, wish/jump/step outputs, and exact stuck-sequence state.
- Execute client/surface/annular speed constraints, projected ground and capped-air acceleration, stop-speed friction, split/full gravity, support-relative jumping and landing, four-bump/five-plane clipping, normalized two-plane creases, generic and high-first step policies, blocked unduck retry, swim and ladder projection, observer follow/fixed/roaming behavior, default/bounce toss response, and accelerated/direct noclip without displacement collision.
- Consume game-supplied hulls, views, speed/air/bunnyhop/backward/jump values, eligibility, step strategy, and accepted mode-transition dispositions. Immutable tracer extensions supply point contents, surface climbability, support/conveyor velocity, mover displacement/filtering, observer targets, and the monotonic clock required only by incremental stuck recovery.
- Emit `StepResult::tick_trace` as the browser-neutral per-command comparison record: position, velocity, wish state, support, selected hull, crouch fraction/view, contacts, events, and mover disposition.
- Reject malformed selected state, command, configuration, hull, velocity, coordinate, and trace values before publication with command number, operation, classification, field, exact offending bits, and configured-limit bits.
- Advance ordered current-transform pusher requests through `PusherSnapshot` without mutating Entity state. One producer handles linear and angular root motion plus rigidly attached pusher members, clamps every final transform, rotates supported/intersecting subjects from the root start transform to its proposed transform, tests subjects in reverse supplied collision-enumeration order, rolls back the complete hierarchy/subject proposal on the first blocker, and emits progress, completion, blocking, hierarchy transforms, subject carry/angular displacement, support/base velocity, and ordered blocker end/start/stay facts.
- Serialize bounded `PUSH` state and `PRES` result records at version 2. Pusher hierarchy, subject, contact, subject-move, and byte limits reject the complete transition before publication.

## Non-Responsibilities

- TF2, CS:S, or legacy CS:GO movement differences.
- Weapon impulses, ruleset timing, or rigid-body simulation.
- Owning collision geometry.

## Relationships

Consumes one immutable `Tracer`; Simulation schedules movement; each game module projects its movement policy and consumes the returned state/events. `advance_transform_pushers` is the current pusher implementation; `advance_linear_pushers` submits the existing linear request shape to that same state machine. Both consume one immutable Collision snapshot and pure caller predicates, while Entity owns request lifecycle, transform publication, outputs, reversal, damage, and rollback acceptance. `State::snapshot_bytes` retains the version-1 TF2 integration record; complete selected-mode comparison uses `StepResult` and `TickTrace` until Simulation owns a replacement snapshot schema. Generic fly custom/slide responses return `Unsupported`: the pinned generic handler does not define a pure slide response and delegates custom response to mutable touch code.

## Completion

Complete when the declared generic Source movement family is implemented and game variation occurs through explicit game-owned behavior.
