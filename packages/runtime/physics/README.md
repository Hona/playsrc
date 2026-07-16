# Physics

## Sample

```ts
import { createPhysicsWorld } from "@playsrc/physics"

const physics = createPhysicsWorld(collision)
physics.step(tickInterval)
```

```rust
let mut physics = playsrc_physics::World::new(collision);
physics.step(tick_interval)?;
```

## Objective

Advance Source-style rigid bodies, constraints, and physical interactions deterministically.

## Responsibilities

- Own rigid-body state, integration, contacts, constraints, sleeping, and physical material response.
- Consume collision shapes without redefining collision parsing or queries.
- Expose physical events and state to entities and simulation.

## Non-Responsibilities

- Player movement, game damage rules, or visual ragdoll presentation.
- Parsing PHY files.
- Owning gameplay authority outside physical state.

## Relationships

Consumes `collision` and PHY-derived data; simulation coordinates physics with entities and game behavior.

## Completion

Complete when the declared physical behavior family is deterministic, integrated, and supported by credible evidence.
