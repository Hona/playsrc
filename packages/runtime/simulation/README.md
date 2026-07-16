# Simulation

## Sample

```ts
import { createSimulation } from "@playsrc/simulation"

const simulation = createSimulation({ map, game, ruleset })
simulation.submitCommands(commands)
simulation.advance(1)
const snapshot = simulation.readSnapshot()
```

```rust
let mut simulation = playsrc_simulation::Simulation::new(map, game, ruleset)?;
simulation.submit_commands(commands);
simulation.advance(1)?;
let snapshot = simulation.snapshot();
```

## Objective

Provide the single deterministic authority that advances composed Source gameplay state.

## Responsibilities

- Own tick progression, command submission, scheduling, state transitions, events, and snapshots.
- Coordinate world, entity, movement, physics, game, and ruleset behavior in a defined order.
- Run the same gameplay implementation in browser workers and native servers.

## Non-Responsibilities

- Reimplementing movement, physics, entities, games, or rulesets.
- Owning network transport, replay authority, or presentation.
- Calling WASM once per tiny domain operation.

## Relationships

Composes world and runtime packages with one game and one game-owned ruleset; networking transports its commands and snapshots.

## Completion

Complete when gameplay advances through one bounded authority with deterministic ordering and coarse native/WASM interfaces.
